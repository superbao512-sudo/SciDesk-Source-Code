//! User-managed website launchers. Icons are owned files; SQLite stores metadata only.
use std::{fs,io::Write,path::{Path,PathBuf}};
use base64::Engine;
use serde::Serialize;
use serde_json::{json,Value};
use sqlx::{SqlitePool,sqlite::{SqliteConnectOptions,SqlitePoolOptions}};
mod local;
type Result<T> = std::result::Result<T,String>;
const MAX_ICON:usize=2*1024*1024;
fn err(e:impl std::fmt::Display)->String{e.to_string()}
fn text(v:&Value,k:&str)->Result<String>{v[k].as_str().map(str::to_owned).ok_or_else(||format!("缺少参数：{k}"))}
fn valid_url(raw:&str)->bool{raw.len()<=8192&&!raw.chars().any(|c|c.is_whitespace()||c.is_control())&&url::Url::parse(raw).is_ok_and(|u|matches!(u.scheme(),"http"|"https")&&u.host_str().is_some()&&u.username().is_empty()&&u.password().is_none())}
fn icon_format(bytes:&[u8])->Option<(&'static str,&'static str)>{if bytes.starts_with(b"\x89PNG\r\n\x1a\n"){Some(("png","image/png"))}else if bytes.starts_with(&[0xff,0xd8,0xff]){Some(("jpg","image/jpeg"))}else if bytes.starts_with(b"RIFF")&&bytes.get(8..12)==Some(b"WEBP"){Some(("webp","image/webp"))}else{None}}
fn owned_icon(name:&str)->bool{let Some(tail)=name.strip_prefix("tool-icon-") else{return false};let Some((id,ext))=tail.rsplit_once('.') else{return false};uuid::Uuid::parse_str(id).is_ok()&&matches!(ext,"png"|"jpg"|"webp")}
#[derive(Serialize,sqlx::FromRow)]
struct Tool{id:String,name:String,description:String,url:String,icon_file:Option<String>,created_at:String,updated_at:String,kind:String,local_json:Option<String>,manifest_id:Option<String>}
struct Store{pool:SqlitePool,root:PathBuf}
impl Store{
 async fn open(root:&Path)->Result<Self>{
  fs::create_dir_all(root).map_err(err)?;let pool=SqlitePoolOptions::new().max_connections(2).connect_with(SqliteConnectOptions::new().filename(root.join("tools.db")).create_if_missing(true).foreign_keys(true).busy_timeout(std::time::Duration::from_secs(10))).await.map_err(err)?;
  let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&pool).await.map_err(err)?;if version>2{pool.close().await;return Err("工具数据库版本高于当前程序".into())}
  if version==0{let mut tx=pool.begin().await.map_err(err)?;for statement in include_str!("../tool_migrations/001_tools.sql").split(';').filter(|s|!s.trim().is_empty()){sqlx::query(statement).execute(&mut *tx).await.map_err(err)?;}tx.commit().await.map_err(err)?;}
  if version<2{let mut tx=pool.begin().await.map_err(err)?;for statement in include_str!("../tool_migrations/002_local_tools.sql").split(';').filter(|s|!s.trim().is_empty()){sqlx::query(statement).execute(&mut *tx).await.map_err(err)?;}tx.commit().await.map_err(err)?;}Ok(Self{pool,root:root.to_owned()})
 }
 async fn get(&self,id:&str)->Result<Tool>{sqlx::query_as("SELECT id,name,description,url,icon_file,created_at,updated_at,kind,local_json,manifest_id FROM tools WHERE id=? AND removed=0").bind(id).fetch_optional(&self.pool).await.map_err(err)?.ok_or("工具不存在或已移除".into())}
 fn icon(&self,name:&str)->Result<String>{
  if !owned_icon(name){return Err("图标文件名无效".into())}let path=self.root.join("icons").join(name);let meta=fs::symlink_metadata(&path).map_err(err)?;if !meta.is_file()||meta.len()>MAX_ICON as u64||crate::project_service::is_link(&meta){return Err("图标文件无效".into())}let bytes=fs::read(path).map_err(err)?;let (_,mime)=icon_format(&bytes).ok_or("图标格式无效")?;Ok(format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes)))
 }
 fn remove_owned_icon(&self,name:&str){if owned_icon(name){let _=fs::remove_file(self.root.join("icons").join(name));}}
 async fn run(&self,op:&str,args:Value)->Result<Value>{match op{
  "list"=>{let rows:Vec<Tool>=sqlx::query_as("SELECT id,name,description,url,icon_file,created_at,updated_at,kind,local_json,manifest_id FROM tools WHERE removed=0 AND NOT (id='densityflow' AND name='QEtoolkit' AND url='https://www.densityflow.com/' AND description='Quantum ESPRESSO 建模、格式转换与后处理' AND icon_file IS NULL) AND COALESCE(manifest_id,'') <> 'research.awbf' ORDER BY rowid").fetch_all(&self.pool).await.map_err(err)?;let rows:Vec<Value>=rows.into_iter().map(|r|{let icon=r.icon_file.as_deref().map(|name|self.icon(name));let warning=icon.as_ref().and_then(|v|v.as_ref().err()).cloned();let data=icon.and_then(std::result::Result::ok);let local=local::summary(&r);let mut result=json!(r);result.as_object_mut().unwrap().remove("local_json");result["local"]=local;result["icon"]=json!(data);result["icon_warning"]=json!(warning);result}).collect();Ok(json!(rows))},
  "open_awbf"=>self.open_awbf().await,
  "preview_local"=>self.preview_local(text(&args,"root")?).await,
  "register_local"=>self.register_local(text(&args,"root")?,text(&args,"token")?).await,
  "pick_local"=>{let result=tauri::async_runtime::spawn_blocking(crate::project_platform::pick_tool_folder).await.map_err(err)??;Ok(json!(result))},
  "save"=>{
   let name=text(&args,"name")?.trim().to_owned();let description=text(&args,"description")?.trim().to_owned();let url=text(&args,"url")?.trim().to_owned();if name.is_empty()||name.chars().count()>80||description.chars().count()>500||name.chars().any(char::is_control)||description.contains('\0')||!valid_url(&url){return Err("请填写名称（1–80 字）、说明（最多 500 字）和有效的 HTTP/HTTPS 网址".into())}
   let existing=if let Some(id)=args["id"].as_str(){Some(self.get(id).await?)}else{None};if existing.as_ref().is_some_and(|r|r.kind!="website"){return Err("本地工具请通过重新加载工具更新".into())}let id=existing.as_ref().map(|r|r.id.clone()).unwrap_or_else(||uuid::Uuid::new_v4().to_string());let mut file=existing.as_ref().and_then(|r|r.icon_file.clone());let mut created:Option<String>=None;
   if let Some(icon)=args.get("icon"){
    if icon.is_null(){file=None}else{
     let encoded=icon.as_str().ok_or("图标数据无效")?;if encoded.len()>MAX_ICON.div_ceil(3)*4+8{return Err("图标不能超过 2 MB".into())}let bytes=base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|_|"图标数据无效")?;if bytes.len()>MAX_ICON{return Err("图标不能超过 2 MB".into())}let (ext,_)=icon_format(&bytes).ok_or("请选择 PNG、JPEG 或 WebP 图片")?;
     let directory=self.root.join("icons");fs::create_dir_all(&directory).map_err(err)?;let filename=format!("tool-icon-{}.{}",uuid::Uuid::new_v4(),ext);let path=directory.join(&filename);
     let write=(||->std::io::Result<()>{let mut output=fs::OpenOptions::new().write(true).create_new(true).open(&path)?;output.write_all(&bytes)?;output.sync_all()})();if let Err(e)=write{let _=fs::remove_file(path);return Err(err(e))}file=Some(filename.clone());created=Some(filename);
    }
   }
   let result=if existing.is_some(){sqlx::query("UPDATE tools SET name=?,description=?,url=?,icon_file=?,updated_at=CURRENT_TIMESTAMP WHERE id=? AND removed=0").bind(name).bind(description).bind(url).bind(&file).bind(&id).execute(&self.pool).await}else{sqlx::query("INSERT INTO tools(id,name,description,url,icon_file) VALUES(?,?,?,?,?)").bind(&id).bind(name).bind(description).bind(url).bind(&file).execute(&self.pool).await};
   match result{Ok(result) if result.rows_affected()==1=>{if let Some(old)=existing.and_then(|r|r.icon_file){if file.as_deref()!=Some(old.as_str()){self.remove_owned_icon(&old)}}Ok(json!({"id":id}))},Ok(_)=>{if let Some(name)=created{self.remove_owned_icon(&name)}Err("工具已变化，请刷新后重试".into())},Err(e)=>{if let Some(name)=created{self.remove_owned_icon(&name)}Err(err(e))}}
  },
  "remove"=>{let id=text(&args,"id")?;let row=self.get(&id).await?;sqlx::query("UPDATE tools SET removed=1,icon_file=NULL,updated_at=CURRENT_TIMESTAMP WHERE id=?").bind(id).execute(&self.pool).await.map_err(err)?;if let Some(name)=row.icon_file{self.remove_owned_icon(&name)}Ok(json!(true))},
  "open"=>{let row=self.get(&text(&args,"id")?).await?;if row.kind=="local"{return self.open_local(row).await}if !valid_url(&row.url){return Err("工具网址无效，请先编辑".into())}tauri::async_runtime::spawn_blocking(move||crate::project_platform::open(Path::new(&row.url))).await.map_err(err)??;Ok(json!(true))},
  _=>Err("未知工具操作".into())
 }}
}
pub(crate) async fn command(root:&Path,op:&str,args:Value)->Result<Value>{let store=Store::open(root).await?;let result=store.run(op,args).await;store.pool.close().await;result}

#[cfg(test)]
mod tests{
 use super::*;
 fn root()->PathBuf{PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("development/verification/tools-rust").join(uuid::Uuid::new_v4().to_string())}
 fn values()->Value{json!({"name":"自定义工具","description":"简短说明","url":"https://example.org/tool?x=1#section"})}
 fn png()->Vec<u8>{base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aF9sAAAAASUVORK5CYII=").unwrap()}
 #[tokio::test]
 async fn initial_requested_site_and_custom_edits_survive_reopen_without_reseeding(){
  let root=root();let store=Store::open(&root).await.unwrap();let initial=store.run("list",json!({})).await.unwrap();assert!(initial.as_array().unwrap().is_empty());
  let saved=store.run("save",values()).await.unwrap();let id=saved["id"].as_str().unwrap();let mut changed=values();changed["id"]=json!(id);changed["name"]=json!("已编辑名称");store.run("save",changed).await.unwrap();store.pool.close().await;
  let store=Store::open(&root).await.unwrap();let rows=store.run("list",json!({})).await.unwrap();assert_eq!(rows.as_array().unwrap().len(),1);assert_eq!(rows[0]["name"],"已编辑名称");assert_eq!(rows[0]["url"],"https://example.org/tool?x=1#section");store.pool.close().await;
 }
 #[tokio::test]
 async fn icons_live_on_filesystem_and_removing_owned_copy_never_touches_upload_original(){
  let root=root();let store=Store::open(&root).await.unwrap();let original=root.join("用户原始图片.png");fs::write(&original,png()).unwrap();let mut v=values();v["icon"]=json!(base64::engine::general_purpose::STANDARD.encode(fs::read(&original).unwrap()));let saved=store.run("save",v).await.unwrap();let id=saved["id"].as_str().unwrap();let row=store.get(id).await.unwrap();let filename=row.icon_file.unwrap();assert!(root.join("icons").join(&filename).is_file());let rows=store.run("list",json!({})).await.unwrap();assert!(rows[0]["icon"].as_str().unwrap().starts_with("data:image/png;base64,"));
  store.run("remove",json!({"id":id})).await.unwrap();assert!(!root.join("icons").join(filename).exists());assert_eq!(fs::read(original).unwrap(),png());store.pool.close().await;
 }
 #[tokio::test]
 async fn failed_save_cleans_only_new_icon_and_invalid_images_write_nothing(){
  let root=root();let store=Store::open(&root).await.unwrap();let mut invalid=values();invalid["icon"]=json!(base64::engine::general_purpose::STANDARD.encode(b"not an image"));assert!(store.run("save",invalid).await.is_err());assert!(!root.join("icons").exists());
  sqlx::query("CREATE TRIGGER fail_tool BEFORE INSERT ON tools BEGIN SELECT RAISE(ABORT,'fixture failure'); END").execute(&store.pool).await.unwrap();let mut v=values();v["icon"]=json!(base64::engine::general_purpose::STANDARD.encode(png()));assert!(store.run("save",v).await.is_err());assert_eq!(fs::read_dir(root.join("icons")).unwrap().count(),0);assert_eq!(store.run("list",json!({})).await.unwrap().as_array().unwrap().len(),0);store.pool.close().await;
 }
 #[tokio::test]
 async fn missing_icons_fall_back_and_unchanged_edits_keep_existing_icon(){
  let root=root();let store=Store::open(&root).await.unwrap();let mut v=values();v["icon"]=json!(base64::engine::general_purpose::STANDARD.encode(png()));let saved=store.run("save",v).await.unwrap();let id=saved["id"].as_str().unwrap();let original=store.get(id).await.unwrap().icon_file.unwrap();let mut update=values();update["id"]=json!(id);store.run("save",update).await.unwrap();assert_eq!(store.get(id).await.unwrap().icon_file.as_deref(),Some(original.as_str()));fs::remove_file(root.join("icons").join(original)).unwrap();let rows=store.run("list",json!({})).await.unwrap();assert_eq!(rows[0]["id"],id);assert!(rows[0]["icon"].is_null());assert!(rows[0]["icon_warning"].is_string());store.pool.close().await;
 }
 #[test]
 fn urls_and_owned_names_reject_executable_schemes_and_path_escapes(){
  assert!(valid_url("https://www.densityflow.com/"));assert!(valid_url("http://example.org/a?x=1#b"));for value in ["file:///E:/file","javascript:alert(1)","https://user:pass@example.org"," https://example.org","https://example.org/\n","not a url"]{assert!(!valid_url(value))}for value in ["../original.png","tool-icon-../../original.png","tool-icon-not-uuid.png","tool-icon-00000000-0000-0000-0000-000000000000.svg"]{assert!(!owned_icon(value))}
 }
}
