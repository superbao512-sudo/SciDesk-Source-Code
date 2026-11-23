//! Local document associations; explicit file-drop operations use the shared Windows transfer service.
use std::{collections::HashSet,fs,path::{Path,PathBuf,Component},time::{SystemTime,UNIX_EPOCH}};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use sqlx::{SqlitePool,sqlite::{SqliteConnectOptions,SqlitePoolOptions}};
use crate::{project_platform,project_service::{canonical,display,is_link,key,within}};
type Result<T> = std::result::Result<T,String>;
mod file_actions;
mod editor;
fn err(e:impl std::fmt::Display)->String{e.to_string()}
fn id()->String{format!("{:x}-{:x}",SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(),std::process::id())}
fn text(v:&Value,k:&str)->Result<String>{v[k].as_str().map(str::to_owned).ok_or_else(||format!("缺少参数：{k}"))}
const SCHEMA:&str=r#"
CREATE TABLE IF NOT EXISTS manuals(id TEXT PRIMARY KEY,name TEXT NOT NULL,category TEXT NOT NULL DEFAULT '',description TEXT NOT NULL DEFAULT '',updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
CREATE TABLE IF NOT EXISTS links(id TEXT PRIMARY KEY,manual_id TEXT NOT NULL REFERENCES manuals(id) ON DELETE CASCADE,path TEXT NOT NULL,path_key TEXT NOT NULL,kind TEXT NOT NULL CHECK(kind IN ('folder','file')),status TEXT NOT NULL DEFAULT 'available',message TEXT NOT NULL DEFAULT '',scanned_at TEXT,UNIQUE(manual_id,path_key));
CREATE TABLE IF NOT EXISTS entries(link_id TEXT NOT NULL REFERENCES links(id) ON DELETE CASCADE,relative TEXT NOT NULL,parent TEXT NOT NULL,name TEXT NOT NULL,is_dir INTEGER NOT NULL,size INTEGER,modified INTEGER,PRIMARY KEY(link_id,relative));
CREATE INDEX IF NOT EXISTS entries_parent ON entries(link_id,parent);
PRAGMA user_version=1;
"#;
#[derive(Clone,Serialize,Deserialize,sqlx::FromRow)]
struct Link{id:String,manual_id:String,path:String,kind:String,status:String,message:String,scanned_at:Option<String>}
#[derive(Serialize,sqlx::FromRow)]
struct Entry{relative:String,parent:String,name:String,is_dir:bool,size:Option<i64>,modified:Option<i64>}
#[derive(Serialize,sqlx::FromRow)]
struct ManualRow{id:String,name:String,category:String,description:String,icon_name:String,manual_color:String,sort_order:i64,name_font_size:i64,links:i64,invalid:i64}
#[derive(Serialize)]
struct SourcedEntry{link_id:String,#[serde(flatten)]entry:Entry}
struct Store{pool:SqlitePool}
impl Store{
 async fn open(root:&Path)->Result<Self>{
  fs::create_dir_all(root).map_err(err)?;
  let pool=SqlitePoolOptions::new().max_connections(2).connect_with(SqliteConnectOptions::new().filename(root.join("manuals.db")).create_if_missing(true).foreign_keys(true).busy_timeout(std::time::Duration::from_secs(10))).await.map_err(err)?;
  let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&pool).await.map_err(err)?;
  if version>2{pool.close().await;return Err("手册数据库版本高于当前程序".into())}
  if version==0{let mut tx=pool.begin().await.map_err(err)?;for statement in SCHEMA.split(';').filter(|s|!s.trim().is_empty()){sqlx::query(statement).execute(&mut *tx).await.map_err(err)?;}tx.commit().await.map_err(err)?;}
  if version<2{let mut tx=pool.begin().await.map_err(err)?;for statement in include_str!("../manual_migrations/002_hall_controls.sql").split(';').filter(|s|!s.trim().is_empty()){sqlx::query(statement).execute(&mut *tx).await.map_err(err)?;}tx.commit().await.map_err(err)?;}
  Ok(Self{pool})
 }
 async fn rows(&self,id:Option<&str>)->Result<Vec<ManualRow>>{sqlx::query_as("SELECT m.id,m.name,m.category,m.description,m.icon_name,m.manual_color,m.sort_order,(SELECT name_font_size FROM preferences WHERE id=1) AS name_font_size,(SELECT COUNT(*) FROM links l WHERE l.manual_id=m.id) AS links,(SELECT COUNT(*) FROM links l WHERE l.manual_id=m.id AND status!='available') AS invalid FROM manuals m WHERE (? IS NULL OR m.id=?) ORDER BY m.sort_order,m.rowid").bind(id).bind(id).fetch_all(&self.pool).await.map_err(err)}
 async fn require(&self,manual:&str)->Result<()>{let exists:i64=sqlx::query_scalar("SELECT COUNT(*) FROM manuals WHERE id=?").bind(manual).fetch_one(&self.pool).await.map_err(err)?;if exists==0{Err("手册不存在或已删除".into())}else{Ok(())}}
 async fn link(&self,manual:&str,link:&str)->Result<Link>{sqlx::query_as("SELECT id,manual_id,path,kind,status,message,scanned_at FROM links WHERE id=? AND manual_id=?").bind(link).bind(manual).fetch_optional(&self.pool).await.map_err(err)?.ok_or("本地关联不存在".into())}
 async fn links(&self,manual:&str)->Result<Vec<Link>>{sqlx::query_as("SELECT id,manual_id,path,kind,status,message,scanned_at FROM links WHERE manual_id=? ORDER BY rowid").bind(manual).fetch_all(&self.pool).await.map_err(err)}
 async fn scan_link(&self,link:&Link)->Result<()>{
  let copy=link.clone();let snapshot=tauri::async_runtime::spawn_blocking(move||scan(&copy)).await.map_err(err)?;
  let (status,message,entries)=match snapshot{Ok(rows)=>("available","".to_owned(),rows),Err(e)=>(if !Path::new(&link.path).exists(){"missing"}else{"unavailable"},e,vec![])};
  let mut tx=self.pool.begin().await.map_err(err)?;
  let current:Option<String>=sqlx::query_scalar("SELECT path FROM links WHERE id=? AND manual_id=?").bind(&link.id).bind(&link.manual_id).fetch_optional(&mut *tx).await.map_err(err)?;
  if current.as_deref()!=Some(&link.path){return Err("本地关联已变化，请重新扫描".into())}
  sqlx::query("DELETE FROM entries WHERE link_id=?").bind(&link.id).execute(&mut *tx).await.map_err(err)?;
  for e in entries{sqlx::query("INSERT INTO entries(link_id,relative,parent,name,is_dir,size,modified) VALUES(?,?,?,?,?,?,?)").bind(&link.id).bind(e.relative).bind(e.parent).bind(e.name).bind(e.is_dir).bind(e.size).bind(e.modified).execute(&mut *tx).await.map_err(err)?;}
  sqlx::query("UPDATE links SET status=?,message=?,scanned_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(status).bind(message).bind(&link.id).execute(&mut *tx).await.map_err(err)?;
  tx.commit().await.map_err(err)
 }
 async fn scan_manual(&self,manual:&str)->Result<()>{self.require(manual).await?;for link in self.links(manual).await?{self.scan_link(&link).await?}Ok(())}
 async fn run(&self,op:&str,args:Value)->Result<Value>{match op{
  op if op.starts_with("book_")=>self.editor(op,args).await,
  "rename_entry"|"trash_entry"=>self.change_entry(op,&args).await,
  "list"=>Ok(json!(self.rows(None).await?)),
  "reorder"=>{
   let ids:Vec<String>=serde_json::from_value(args["ids"].clone()).map_err(err)?;let mut tx=self.pool.begin().await.map_err(err)?;
   sqlx::query("UPDATE preferences SET name_font_size=name_font_size WHERE id=1").execute(&mut *tx).await.map_err(err)?;
   let actual:Vec<String>=sqlx::query_scalar("SELECT id FROM manuals").fetch_all(&mut *tx).await.map_err(err)?;let wanted=actual.into_iter().collect::<HashSet<_>>();
   if ids.len()!=wanted.len()||ids.iter().cloned().collect::<HashSet<_>>()!=wanted{return Err("手册列表已变化，请刷新后重新排序".into())}
   for (index,id) in ids.iter().enumerate(){sqlx::query("UPDATE manuals SET sort_order=? WHERE id=?").bind(index as i64).bind(id).execute(&mut *tx).await.map_err(err)?;}tx.commit().await.map_err(err)?;Ok(json!(true))
  },
  "set_appearance"=>{let manual=text(&args,"id")?;let old:Option<(String,String)>=sqlx::query_as("SELECT icon_name,manual_color FROM manuals WHERE id=?").bind(&manual).fetch_optional(&self.pool).await.map_err(err)?;let old=old.ok_or("手册不存在或已删除")?;let (icon,color)=appearance(&args,(&old.0,&old.1))?;sqlx::query("UPDATE manuals SET icon_name=?,manual_color=? WHERE id=?").bind(icon).bind(color).bind(manual).execute(&self.pool).await.map_err(err)?;Ok(json!(true))},
  "rename"=>{let manual=text(&args,"id")?;let (name,_,_)=manual_fields(&args)?;let size=args["font_size"].as_i64().filter(|n|(14..=28).contains(n)).ok_or("手册名称字号必须是 14–28 的整数")?;let mut tx=self.pool.begin().await.map_err(err)?;let result=sqlx::query("UPDATE manuals SET name=?,updated_at=CURRENT_TIMESTAMP WHERE id=?").bind(name).bind(&manual).execute(&mut *tx).await.map_err(err)?;if result.rows_affected()!=1{return Err("手册不存在或已删除".into())}sqlx::query("UPDATE preferences SET name_font_size=? WHERE id=1").bind(size).execute(&mut *tx).await.map_err(err)?;tx.commit().await.map_err(err)?;Ok(json!(true))},
  "import"=>{
   let kind=text(&args,"kind")?;let path=canonical(Path::new(&text(&args,"path")?))?;validate_root(&path,&kind)?;
   let existing=args["id"].as_str();let manual=existing.map(str::to_owned).unwrap_or_else(id);let link_id=id();
   let link=Link{id:link_id.clone(),manual_id:manual.clone(),path:display(&path),kind:kind.clone(),status:"available".into(),message:String::new(),scanned_at:None};
   let entries=tauri::async_runtime::spawn_blocking(move||scan(&link)).await.map_err(err)??;
   let (name,category,description)=if existing.is_none(){manual_fields(&args)?}else{(String::new(),String::new(),String::new())};let (icon,color)=appearance(&args,("book-open","#4779bd"))?;
   let mut tx=self.pool.begin().await.map_err(err)?;
   if existing.is_some(){let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM manuals WHERE id=?").bind(&manual).fetch_one(&mut *tx).await.map_err(err)?;if count==0{return Err("目标手册不存在或已删除".into())}}
   else{sqlx::query("INSERT INTO manuals(id,name,category,description,icon_name,manual_color,sort_order) VALUES(?,?,?,?,?,?,(SELECT COALESCE(MAX(sort_order),-1)+1 FROM manuals))").bind(&manual).bind(name).bind(category).bind(description).bind(icon).bind(color).execute(&mut *tx).await.map_err(err)?;}
   sqlx::query("INSERT INTO links(id,manual_id,path,path_key,kind,scanned_at) VALUES(?,?,?,?,?,strftime('%Y-%m-%dT%H:%M:%fZ','now'))").bind(&link_id).bind(&manual).bind(display(&path)).bind(key(&path)).bind(kind).execute(&mut *tx).await.map_err(|e|format!("导入失败（可能已关联相同路径）：{e}"))?;
   for e in entries{sqlx::query("INSERT INTO entries(link_id,relative,parent,name,is_dir,size,modified) VALUES(?,?,?,?,?,?,?)").bind(&link_id).bind(e.relative).bind(e.parent).bind(e.name).bind(e.is_dir).bind(e.size).bind(e.modified).execute(&mut *tx).await.map_err(err)?;}
   tx.commit().await.map_err(err)?;Ok(json!({"id":manual}))
  },
  "save"=>{
   let (name,category,description)=manual_fields(&args)?;
   let manual=args["id"].as_str().map(str::to_owned).unwrap_or_else(id);
   if args["id"].is_string(){let old=self.rows(Some(&manual)).await?.pop().ok_or("手册不存在或已删除")?;let (icon,color)=appearance(&args,(&old.icon_name,&old.manual_color))?;sqlx::query("UPDATE manuals SET name=?,category=?,description=?,icon_name=?,manual_color=?,updated_at=CURRENT_TIMESTAMP WHERE id=?").bind(name).bind(category).bind(description).bind(icon).bind(color).bind(&manual).execute(&self.pool).await.map_err(err)?;}else{let (icon,color)=appearance(&args,("book-open","#4779bd"))?;sqlx::query("INSERT INTO manuals(id,name,category,description,icon_name,manual_color,sort_order) VALUES(?,?,?,?,?,?,(SELECT COALESCE(MAX(sort_order),-1)+1 FROM manuals))").bind(&manual).bind(name).bind(category).bind(description).bind(icon).bind(color).execute(&self.pool).await.map_err(err)?;}
   Ok(json!({"id":manual}))
  },
  "get"|"scan"=>{let manual=text(&args,"id")?;self.scan_manual(&manual).await?;let row=self.rows(Some(&manual)).await?.pop().ok_or("手册不存在或已删除")?;let mut result=serde_json::to_value(row).map_err(err)?;result["links"]=json!(self.links(&manual).await?);Ok(result)},
  "enter"=>{let manual=text(&args,"id")?;self.require(&manual).await?;let links=self.links(&manual).await?;if links.len()==1&&links[0].kind=="file"{if let Ok(path)=resolve(&links[0],""){if editor::markdown(&path){return Ok(json!({"opened":false,"markdownLink":links[0].id,"chapter":path.file_name().unwrap_or_default().to_string_lossy()}))}open_original(path).await?;return Ok(json!({"opened":true}))}}Ok(json!({"opened":false}))},
  "browse_root"=>{
   let manual=text(&args,"id")?;self.require(&manual).await?;let mut result=vec![];
   for link in self.links(&manual).await?{if link.status!="available"{continue}let root=match resolve(&link,""){Ok(root)=>root,Err(_)=>continue};
    if link.kind=="file"{let meta=fs::metadata(&root).map_err(err)?;result.push(SourcedEntry{link_id:link.id,entry:Entry{relative:String::new(),parent:String::new(),name:root.file_name().unwrap_or_default().to_string_lossy().into_owned(),is_dir:false,size:Some(meta.len() as i64),modified:meta.modified().ok().and_then(|t|t.duration_since(UNIX_EPOCH).ok()).map(|t|t.as_secs() as i64)}})}
    else{let entries:Vec<Entry>=sqlx::query_as("SELECT relative,parent,name,is_dir,size,modified FROM entries WHERE link_id=? AND parent=''").bind(&link.id).fetch_all(&self.pool).await.map_err(err)?;for entry in entries{if resolve(&link,&entry.relative).is_ok(){result.push(SourcedEntry{link_id:link.id.clone(),entry})}}}
   }result.sort_by(|a,b|b.entry.is_dir.cmp(&a.entry.is_dir).then_with(||a.entry.name.to_lowercase().cmp(&b.entry.name.to_lowercase())).then_with(||a.link_id.cmp(&b.link_id)));Ok(json!({"entries":result}))
  },
  "attach"|"relink"=>{
   let manual=text(&args,"id")?;self.require(&manual).await?;let kind=text(&args,"kind")?;let path=canonical(Path::new(&text(&args,"path")?))?;validate_root(&path,&kind)?;
   let link_id=if op=="relink"{let link=self.link(&manual,&text(&args,"linkId")?).await?;if link.kind!=kind{return Err("重新连接时请保持文件或文件夹类型一致".into())}sqlx::query("UPDATE links SET path=?,path_key=? WHERE id=?").bind(display(&path)).bind(key(&path)).bind(&link.id).execute(&self.pool).await.map_err(|e|format!("无法重新连接（可能已关联相同路径）：{e}"))?;link.id}else{let link=id();sqlx::query("INSERT INTO links(id,manual_id,path,path_key,kind) VALUES(?,?,?,?,?)").bind(&link).bind(&manual).bind(display(&path)).bind(key(&path)).bind(kind).execute(&self.pool).await.map_err(|e|format!("无法连接（可能已关联相同路径）：{e}"))?;link};
   self.scan_link(&self.link(&manual,&link_id).await?).await?;Ok(json!(true))
  },
  "detach"=>{sqlx::query("DELETE FROM links WHERE id=? AND manual_id=?").bind(text(&args,"linkId")?).bind(text(&args,"id")?).execute(&self.pool).await.map_err(err)?;Ok(json!(true))},
  "delete"=>{sqlx::query("DELETE FROM manuals WHERE id=?").bind(text(&args,"id")?).execute(&self.pool).await.map_err(err)?;Ok(json!(true))},
  "browse"|"open"=>{
   let link=self.link(&text(&args,"id")?,&text(&args,"linkId")?).await?;let relative=args["relative"].as_str().unwrap_or("");let path=resolve(&link,relative)?;
   if op=="open"{if !path.is_file(){return Err("只能打开关联中的原始文件".into())}if link.kind=="folder"{let indexed:i64=sqlx::query_scalar("SELECT COUNT(*) FROM entries WHERE link_id=? AND relative=? AND is_dir=0").bind(&link.id).bind(relative).fetch_one(&self.pool).await.map_err(err)?;if indexed==0{return Err("文件未在本地索引中，请重新扫描".into())}}tauri::async_runtime::spawn_blocking(move||project_platform::open(&path)).await.map_err(err)??;return Ok(json!(true))}
   if !path.is_dir(){return Err("该目录已失效，请重新扫描或连接".into())}
   let entries:Vec<Entry>=sqlx::query_as("SELECT relative,parent,name,is_dir,size,modified FROM entries WHERE link_id=? AND parent=? ORDER BY is_dir DESC,name COLLATE NOCASE").bind(&link.id).bind(relative).fetch_all(&self.pool).await.map_err(err)?;
   let entries=entries.into_iter().filter(|e|resolve(&link,&e.relative).is_ok()).collect::<Vec<_>>();Ok(json!({"entries":entries}))
  },
  "reveal"|"open_directory"|"create_folder"=>{
   let manual=text(&args,"id")?;
   let link=self.link(&manual,&text(&args,"linkId")?).await?;
   let path=resolve(&link,args["relative"].as_str().unwrap_or(""))?;
   if op=="reveal"{tauri::async_runtime::spawn_blocking(move||project_platform::reveal(&path)).await.map_err(err)??;}
   else{
    if !path.is_dir(){return Err("请选择手册中的本地文件夹".into())}
    if op=="open_directory"{open_original(path).await?;}
    else{
     let parent=crate::file_transfer::Guard::new(vec![])?.check(&path)?;
     let name=text(&args,"name")?;crate::project_service::validate_name(&name)?;
     fs::create_dir(parent.join(name)).map_err(err)?;
     self.scan_link(&link).await?;
    }
   }
   Ok(json!(true))
  },
  "drop_files"|"paste_files"=>{
   let manual=text(&args,"id")?;let link=self.link(&manual,&text(&args,"linkId")?).await?;
   if link.kind!="folder"{return Err("请选择手册中的实际文件夹作为目标".into())}
   let destination=resolve(&link,args["relative"].as_str().unwrap_or(""))?;let guard=crate::file_transfer::Guard::new(vec![])?;
   let (sources,args)=if op=="paste_files"{
    let paths=tauri::async_runtime::spawn_blocking(crate::file_transfer::clipboard_paths).await.map_err(err)??;
    (paths,json!({"mode":"copy","conflict":"keep","duplicateInPlace":true,"_native_window":args["_native_window"]}))
   }else{(crate::file_transfer::paths(&args)?,args)};
   let mut result=tauri::async_runtime::spawn_blocking(move||crate::file_transfer::transfer(guard,destination,sources,args)).await.map_err(err)??;
   if let Err(error)=self.scan_manual(&manual).await{result["scan_warning"]=json!(error)}Ok(result)
  },
  "drag_paths"|"copy_files"=>{
   let manual=text(&args,"id")?;self.require(&manual).await?;let entries=args["entries"].as_array().filter(|e|!e.is_empty()&&e.len()<=256).ok_or("请选择 1–256 个文件或文件夹")?;let mut paths=vec![];
   for entry in entries{let link=self.link(&manual,&text(entry,"linkId")?).await?;paths.push(resolve(&link,entry["relative"].as_str().unwrap_or(""))?);}
   let guard=crate::file_transfer::Guard::new(vec![])?;
   let copy=op=="copy_files";
   tauri::async_runtime::spawn_blocking(move||{
    let paths=crate::file_transfer::prepare_drag(guard,paths)?;
    if copy{crate::file_transfer::copy_to_clipboard(&paths)?;Ok(json!(paths.len()))}else{Ok(json!(paths))}
   }).await.map_err(err)?
  },
  "drop_links"=>{
   let manual=text(&args,"id")?;self.require(&manual).await?;let paths=crate::file_transfer::paths(&args)?;let mut items=vec![];
   for path in paths{let source=display(&path);let result=Box::pin(self.run("attach",json!({"id":manual,"path":source,"kind":if path.is_dir(){"folder"}else{"file"}}))).await;
    items.push(json!({"source":source,"destination":"手册本地关联","status":if result.is_ok(){"completed"}else{"failed"},"message":result.err().unwrap_or_else(||"已关联，保留原始文件".into())}));
   }Ok(json!({"items":items,"cancelled":false}))
  },
  "pick_folder"|"pick_file"=>{let folder=op=="pick_folder";let result=tauri::async_runtime::spawn_blocking(move||if folder{project_platform::pick(None)}else{project_platform::pick_file(None)}).await.map_err(err)??;Ok(json!(result))},
  _=>Err("未知手册操作".into())
 }}
}
fn manual_fields(args:&Value)->Result<(String,String,String)>{let name=text(args,"name")?.trim().to_owned();let category=args["category"].as_str().unwrap_or("").trim().to_owned();let description=args["description"].as_str().unwrap_or("").trim().to_owned();if name.is_empty()||name.chars().count()>120||category.chars().count()>80||description.chars().count()>2000||name.chars().any(char::is_control)||category.chars().any(char::is_control)||description.contains('\0'){return Err("请检查名称（1–120 字）、分类（最多 80 字）和说明（最多 2000 字）".into())}Ok((name,category,description))}
fn appearance(args:&Value,fallback:(&str,&str))->Result<(String,String)>{let icon=match args.get("icon_name"){None=>fallback.0,Some(v)=>v.as_str().ok_or("图标无效")?};let color=match args.get("manual_color"){None=>fallback.1,Some(v)=>v.as_str().ok_or("颜色无效")?};if !crate::project_service::ICONS.contains(&icon)||color.len()!=7||!color.starts_with('#')||!color[1..].bytes().all(|c|c.is_ascii_hexdigit()){return Err("请选择已有图标和有效的 #RRGGBB 颜色".into())}Ok((icon.into(),color.into()))}
async fn open_original(path:PathBuf)->Result<()>{tauri::async_runtime::spawn_blocking(move||project_platform::open(&path)).await.map_err(err)?}
fn validate_root(path:&Path,kind:&str)->Result<()>{let meta=fs::symlink_metadata(path).map_err(err)?;if is_link(&meta)||!matches!(kind,"folder"|"file")||kind=="folder"&&!meta.is_dir()||kind=="file"&&!meta.is_file(){return Err("请选择存在的普通文件或文件夹，不使用符号链接或目录联接".into())}Ok(())}
fn resolve(link:&Link,relative:&str)->Result<PathBuf>{
 let root=Path::new(&link.path);validate_root(root,&link.kind)?;if key(&canonical(root)?)!=key(root){return Err("关联位置已改变，请重新连接".into())}
 let rel=Path::new(relative);if rel.components().any(|c|!matches!(c,Component::Normal(_)))||link.kind=="file"&&!relative.is_empty(){return Err("只能访问当前手册关联内部的路径".into())}
 let mut path=root.to_path_buf();for c in rel.components(){path.push(c);let meta=fs::symlink_metadata(&path).map_err(err)?;if is_link(&meta){return Err("不访问符号链接或目录联接".into())}}
 let actual=canonical(&path)?;if !within(&actual,root){return Err("路径已超出本地关联".into())}Ok(actual)
}
fn scan(link:&Link)->Result<Vec<Entry>>{
 let root=resolve(link,"")?;if link.kind=="file"{return Ok(vec![])}let mut result=vec![];let mut pending=vec![(root,String::new(),0)];
 while let Some((dir,parent,depth))=pending.pop(){if depth>64{return Err("目录超过 64 层，未保留不完整索引；请选择更具体的文件夹".into())}
  for item in fs::read_dir(dir).map_err(err)?{let item=item.map_err(err)?;let meta=fs::symlink_metadata(item.path()).map_err(err)?;if is_link(&meta)||!meta.is_dir()&&!meta.is_file(){continue}if result.len()>=20000{return Err("超过 20000 个条目，未保留不完整索引；请选择更具体的文件夹".into())}
   let name=item.file_name().to_string_lossy().into_owned();let relative=if parent.is_empty(){name.clone()}else{format!("{parent}/{name}")};let is_dir=meta.is_dir();
   if is_dir{pending.push((item.path(),relative.clone(),depth+1))}result.push(Entry{relative,parent:parent.clone(),name,is_dir,size:if is_dir{None}else{Some(meta.len() as i64)},modified:meta.modified().ok().and_then(|t|t.duration_since(UNIX_EPOCH).ok()).map(|t|t.as_secs() as i64)});
  }
 }Ok(result)
}
pub(crate) async fn new_book(metadata:&Path,projects:&Path)->Result<Value>{
 let store=Store::open(metadata).await?;
 let result=store.create_blank_book(projects).await;
 store.pool.close().await;
 result
}

pub(crate) async fn command(root:&Path,op:&str,args:Value)->Result<Value>{let store=Store::open(root).await?;let result=store.run(op,args).await;store.pool.close().await;result}

#[cfg(test)]mod tests;
