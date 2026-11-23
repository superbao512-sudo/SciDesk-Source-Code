//! Versioned local tool packages. Import reads metadata only; launch requires a saved ID.
use super::*;
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{io::{Read,Seek,SeekFrom},process::{Command,Stdio}};
use crate::project_service::{canonical,display,is_link,key,within};

const MANIFEST:&str="scidesk-tool.json";
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest{
 schema_version:u32,id:String,name:String,version:String,
 #[serde(default)]description:String,
 runtime:String,entry:String,
 #[serde(default)]args:Vec<String>,
 #[serde(default)]icon:Option<String>,
}
#[derive(Clone,Serialize,Deserialize)]
struct Registration{root:String,manifest:Manifest,manifest_sha256:String,entry_sha256:String}
struct Package{registration:Registration,icon:Option<Vec<u8>>,token:String}
fn digest(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn resolve(root:&Path,relative:&str)->Result<PathBuf>{
 if relative.is_empty()||relative.len()>4096||relative.chars().any(char::is_control)||relative.split(['/', '\\']).any(|p|p.is_empty()||p=="."||p==".."||p.contains(':')){return Err("入口和图标必须是工具文件夹内的相对路径，不允许越界或目录联接".into())}
 let mut path=root.to_owned();for part in relative.split(['/', '\\']){path.push(part);let meta=fs::symlink_metadata(&path).map_err(|e|format!("工具文件不存在或不可读：{relative}：{e}"))?;if is_link(&meta){return Err("工具文件不使用符号链接或目录联接".into())}}
 let actual=canonical(&path)?;if !within(&actual,root)||!actual.is_file(){return Err("入口和图标必须是工具文件夹内的普通文件".into())}Ok(actual)
}
fn entry_hash(path:&Path,runtime:&str)->Result<String>{
 let meta=fs::metadata(path).map_err(err)?;if meta.len()>512*1024*1024{return Err("工具入口文件不能超过 512 MB".into())}let mut input=fs::File::open(path).map_err(err)?;
 if runtime=="exe"{let mut header=[0u8;64];input.read_exact(&mut header).map_err(|_|"入口不是有效的 Windows EXE")?;if &header[..2]!=b"MZ"{return Err("入口不是有效的 Windows EXE".into())}let offset=u32::from_le_bytes(header[60..64].try_into().unwrap()) as u64;if offset>meta.len().saturating_sub(4){return Err("EXE 的 PE 头无效".into())}input.seek(SeekFrom::Start(offset)).map_err(err)?;let mut signature=[0;4];input.read_exact(&mut signature).map_err(err)?;if &signature!=b"PE\0\0"{return Err("入口不是有效的 PE 程序".into())}}
 drop(input);let mut input=fs::File::open(path).map_err(err)?;let mut hash=Sha256::new();let mut buffer=[0u8;64*1024];loop{let count=input.read(&mut buffer).map_err(err)?;if count==0{break}hash.update(&buffer[..count]);}Ok(format!("{:x}",hash.finalize()))
}
fn read_package(raw:&str)->Result<Package>{
 let path=Path::new(raw);if !path.is_absolute(){return Err("请选择本地工具文件夹的绝对路径".into())}let root=canonical(path).map_err(|e|format!("工具目录已失效，请重新选择：{e}"))?;if !root.is_dir(){return Err("请选择工具文件夹".into())}
 let manifest_path=resolve(&root,MANIFEST)?;let meta=fs::metadata(&manifest_path).map_err(err)?;if meta.len()>64*1024{return Err("工具清单不能超过 64 KB".into())}let raw=fs::read(manifest_path).map_err(err)?;
 let m:Manifest=serde_json::from_slice(raw.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&raw)).map_err(|e|format!("工具清单格式错误：{e}"))?;
 if m.schema_version!=1{return Err("不支持此工具接口版本，当前支持 schema_version: 1".into())}
 if m.id.len()<3||m.id.len()>80||!m.id.as_bytes()[0].is_ascii_alphabetic()||!m.id.bytes().all(|c|c.is_ascii_alphanumeric()||matches!(c,b'.'|b'-'|b'_')){return Err("工具 id 需为 3–80 位英文字母、数字、点、横线或下划线，并以字母开头".into())}
 if m.name.trim().is_empty()||m.name.chars().count()>80||m.name.chars().any(char::is_control)||m.description.chars().count()>500||m.description.contains('\0')||m.version.trim().is_empty()||m.version.len()>80||m.version.chars().any(char::is_control){return Err("请检查工具名称、说明和版本".into())}
 if !matches!(m.runtime.as_str(),"exe"|"html")||m.args.len()>32||m.args.iter().any(|a|a.len()>4096||a.chars().any(char::is_control))||m.runtime=="html"&&!m.args.is_empty(){return Err("支持 exe、html 两种入口；html 不接受参数，exe 最多 32 项参数".into())}
 let entry=resolve(&root,&m.entry)?;let ext=entry.extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase();if m.runtime=="exe"&&ext!="exe"||m.runtime=="html"&&!matches!(ext.as_str(),"html"|"htm"){return Err("入口后缀需与 runtime 对应：.exe 或 .html/.htm".into())}
 let entry_sha256=entry_hash(&entry,&m.runtime)?;let icon=if let Some(name)=&m.icon{let path=resolve(&root,name)?;if fs::metadata(&path).map_err(err)?.len()>MAX_ICON as u64{return Err("工具图标不能超过 2 MB".into())}let bytes=fs::read(path).map_err(err)?;icon_format(&bytes).ok_or("工具图标需为 PNG、JPEG 或 WebP")?;Some(bytes)}else{None};
 let registration=Registration{root:display(&root),manifest:m,manifest_sha256:digest(&raw),entry_sha256};let token=digest(&serde_json::to_vec(&(&registration,icon.as_ref().map(|v|digest(v)))).map_err(err)?);Ok(Package{registration,icon,token})
}
pub(super) fn summary(tool:&Tool)->Value{
 if tool.kind!="local"{return Value::Null}match tool.local_json.as_ref().and_then(|s|serde_json::from_str::<Registration>(s).ok()){
  Some(r)=>{let available=Path::new(&r.root).is_dir()&&resolve(Path::new(&r.root),&r.manifest.entry).is_ok();json!({"root":r.root,"id":r.manifest.id,"version":r.manifest.version,"runtime":r.manifest.runtime,"entry":r.manifest.entry,"available":available})},None=>json!({"available":false})
 }
}
impl Store{
 pub(super) async fn preview_local(&self,root:String)->Result<Value>{
  let p=tauri::async_runtime::spawn_blocking(move||read_package(&root)).await.map_err(err)??;let existing:Option<String>=sqlx::query_scalar("SELECT id FROM tools WHERE manifest_id=? AND kind='local' AND removed=0").bind(&p.registration.manifest.id).fetch_optional(&self.pool).await.map_err(err)?;
  let icon=p.icon.as_ref().and_then(|bytes|icon_format(bytes).map(|(_,mime)|format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))));
  Ok(json!({"root":p.registration.root,"manifest":p.registration.manifest,"icon":icon,"token":p.token,"existing_id":existing}))
 }
 pub(super) async fn register_local(&self,root:String,token:String)->Result<Value>{
  let p=tauri::async_runtime::spawn_blocking(move||read_package(&root)).await.map_err(err)??;if p.token!=token{return Err("工具文件在预览后发生变化，请重新读取并确认".into())}
  let mut tx=self.pool.begin().await.map_err(err)?;sqlx::query("UPDATE tools SET name=name WHERE 0").execute(&mut *tx).await.map_err(err)?;let old:Option<(String,Option<String>)>=sqlx::query_as("SELECT id,icon_file FROM tools WHERE kind='local' AND manifest_id=? AND removed=0").bind(&p.registration.manifest.id).fetch_optional(&mut *tx).await.map_err(err)?;
  let id=old.as_ref().map(|r|r.0.clone()).unwrap_or_else(||uuid::Uuid::new_v4().to_string());let icon_file=if let Some(bytes)=p.icon{let (ext,_)=icon_format(&bytes).ok_or("工具图标格式无效")?;let filename=format!("tool-icon-{}.{}",uuid::Uuid::new_v4(),ext);let directory=self.root.join("icons");fs::create_dir_all(&directory).map_err(err)?;let path=directory.join(&filename);let written=(||->std::io::Result<()>{let mut out=fs::OpenOptions::new().write(true).create_new(true).open(&path)?;out.write_all(&bytes)?;out.sync_all()})();if let Err(e)=written{let _=fs::remove_file(path);return Err(err(e))}Some(filename)}else{None};
  let m=&p.registration.manifest;let data=serde_json::to_string(&p.registration).map_err(err)?;
  let write=if old.is_some(){sqlx::query("UPDATE tools SET name=?,description=?,icon_file=?,local_json=?,updated_at=CURRENT_TIMESTAMP WHERE id=? AND kind='local' AND removed=0").bind(&m.name).bind(&m.description).bind(&icon_file).bind(data).bind(&id).execute(&mut *tx).await}else{sqlx::query("INSERT INTO tools(id,name,description,url,icon_file,kind,local_json,manifest_id) VALUES(?,?,?,'',?,'local',?,?)").bind(&id).bind(&m.name).bind(&m.description).bind(&icon_file).bind(data).bind(&m.id).execute(&mut *tx).await};
  if let Err(e)=write{if let Some(name)=&icon_file{self.remove_owned_icon(name)}return Err(err(e))}if let Err(e)=tx.commit().await{if let Some(name)=&icon_file{self.remove_owned_icon(name)}return Err(err(e))}
  if let Some((_,Some(name)))=old{if icon_file.as_deref()!=Some(name.as_str()){self.remove_owned_icon(&name)}}Ok(json!({"id":id}))
 }
 pub(super) async fn open_awbf(&self)->Result<Value>{
  let prior:Option<String>=sqlx::query_scalar("SELECT id FROM tools WHERE manifest_id='research.awbf' ORDER BY removed,rowid LIMIT 1").fetch_optional(&self.pool).await.map_err(err)?;
  let id=prior.filter(|id|uuid::Uuid::parse_str(id).is_ok()).unwrap_or_else(||"research.awbf".into());
  let data=self.root.join("data").join(id);
  let pid=tauri::async_runtime::spawn_blocking(move||{
   let exe=std::env::current_exe().map_err(err)?;
   let root=exe.parent().ok_or("Missing application directory")?.join("awbf");
   #[cfg(debug_assertions)]
   let root=if root.is_dir(){root}else{Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/awbf")};
   let entry=root.join("AWBF-Desktop.exe");
   let digest=format!("{:X}",Sha256::digest(fs::read(&entry).map_err(err)?));
   if digest!="79D8F7A6CF9995F2B2AEDF4072D242D68AC186C0140F6DF43530B297C9AA0C24"{return Err("AWBF executable checksum mismatch".into())}
   fs::create_dir_all(&data).map_err(err)?;
   start_exe(&entry,&root,&[],&data)
  }).await.map_err(err)??;
  Ok(json!({"started":true,"runtime":"exe","pid":pid}))
 }
 pub(super) async fn open_local(&self,tool:Tool)->Result<Value>{
  let saved:Registration=serde_json::from_str(tool.local_json.as_deref().ok_or("本地工具注册信息无效")?).map_err(err)?;let current=tauri::async_runtime::spawn_blocking(move||read_package(&saved.root).map(|current|(saved,current.registration))).await.map_err(err)??;
  let (saved,current)=current;if saved.manifest_sha256!=current.manifest_sha256||saved.entry_sha256!=current.entry_sha256||key(Path::new(&saved.root))!=key(Path::new(&current.root)){return Err("工具清单或入口已更新，请重新加载工具并确认后再启动".into())}
  let root=PathBuf::from(&saved.root);let entry=resolve(&root,&saved.manifest.entry)?;
  if saved.manifest.runtime=="html"{tauri::async_runtime::spawn_blocking(move||crate::project_platform::open(&entry)).await.map_err(err)??;return Ok(json!({"started":true,"runtime":"html"}))}
  uuid::Uuid::parse_str(&tool.id).map_err(|_|"本地工具 ID 无效")?;let data=self.root.join("data").join(&tool.id);fs::create_dir_all(&data).map_err(err)?;
  let result=tauri::async_runtime::spawn_blocking(move||start_exe(&entry,&root,&saved.manifest.args,&data)).await.map_err(err)??;Ok(json!({"started":true,"runtime":"exe","pid":result}))
 }
}
fn start_exe(entry:&Path,root:&Path,args:&[String],data:&Path)->Result<u32>{
 let mut command=Command::new(entry);command.args(args).current_dir(root).env("SCIDESK_TOOL_API_VERSION","1").env("SCIDESK_TOOL_DATA_DIR",data).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
 #[cfg(windows)]{use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
 command.spawn().map(|child|child.id()).map_err(|e|format!("工具启动失败：{e}"))
}

#[cfg(test)]mod tests;
