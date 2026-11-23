//! One real-filesystem service used by Tauri and the browser development bridge.
use std::{collections::HashSet,fs,io::Write,path::{Component,Path,PathBuf},time::{SystemTime,UNIX_EPOCH}};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sqlx::{SqlitePool,Sqlite,Transaction};
use crate::project_platform;
mod literature;

pub const ICONS:&[&str]=&["folder","folder-open","file","files","book","book-open","notebook","graduation-cap","pencil","pen-tool","sticky-note","code","terminal","braces","database","chart-line","chart-column","chart-pie","table","atom","flask-conical","test-tube","microscope","dna","magnet","orbit","network","cpu","server","wrench","settings","lightbulb","telescope","globe","workflow","layers","binary","radio","calculator","compass"];
#[derive(Debug,Clone,Serialize,Deserialize,sqlx::FromRow)]
pub struct Project {pub id:String,pub name:String,#[serde(default)] pub description:String,pub root_path:String,pub icon_name:String,pub project_color:String,pub status:String,pub progress:i64,pub progress_enabled:bool,pub sort_order:i64,pub source_type:String,pub created_at:String,pub updated_at:String}
const COLUMNS:&str="id,name,description,root_path,icon_name,project_color,status,progress,progress_enabled,sort_order,source_type,created_at,updated_at";
#[derive(Debug,Clone,Serialize,Deserialize)]
struct Pending {kind:String,project:Project,old_path:String,new_path:String,fs_done:bool}
mod research;
pub struct Service {pub pool:SqlitePool,app_root:PathBuf,journal:PathBuf,verification_root:Option<PathBuf>}
pub fn application_root()->Result<PathBuf,String>{
 #[cfg(debug_assertions)]{Ok(PathBuf::from(env!("SCIDESK_PROJECT_ROOT")))}
 #[cfg(not(debug_assertions))]{std::env::current_exe().map_err(|e|e.to_string())?.parent().map(Path::to_path_buf).ok_or("无法确定应用目录".into())}
}
pub async fn migrate(pool:&SqlitePool)->Result<(),String>{
 let mut tx=pool.begin().await.map_err(err)?;
 let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&mut *tx).await.map_err(err)?;
 if version==0{sqlx::raw_sql(include_str!("../migrations/001_home_foundation.sql")).execute(&mut *tx).await.map_err(err)?;}
 if version<2{sqlx::raw_sql(include_str!("../migrations/002_project_management.sql")).execute(&mut *tx).await.map_err(err)?;sqlx::query("PRAGMA user_version=2").execute(&mut *tx).await.map_err(err)?;}
 if version>2{return Err("数据库版本高于当前程序，已停止写入".into())}
 tx.commit().await.map_err(err)
}
fn err(e:impl std::fmt::Display)->String{e.to_string()}
fn text(v:&Value,k:&str)->Result<String,String>{v.get(k).and_then(Value::as_str).map(str::to_owned).ok_or_else(||format!("缺少参数：{k}"))}
fn description(v:&Value,fallback:&str)->Result<String,String>{match v.get("description"){None=>Ok(fallback.to_owned()),Some(value)=>value.as_str().map(|s|s.trim().to_owned()).ok_or("项目描述必须是文本".into())}}
fn integer(v:&Value,k:&str,default:i64)->Result<i64,String>{match v.get(k){None=>Ok(default),Some(value)=>value.as_i64().ok_or_else(||format!("{k} 必须是整数"))}}
pub fn display(path:&Path)->String{let s=path.to_string_lossy();if let Some(rest)=s.strip_prefix(r"\\?\UNC\"){format!(r"\\{rest}")}else{s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()}}
pub(crate) fn key(path:&Path)->String{display(path).replace('\\',"/").trim_end_matches('/').to_lowercase()}
pub(crate) fn within(path:&Path,root:&Path)->bool{let p=key(path);let r=key(root);p==r||p.starts_with(&(r+"/"))}
pub fn validate_name(name:&str)->Result<(),String>{
 let base=name.split('.').next().unwrap_or("").trim_end().to_uppercase();
 let reserved=["CON","PRN","AUX","NUL"].contains(&base.as_str())||["COM","LPT"].iter().any(|p|base.strip_prefix(p).is_some_and(|tail|tail.chars().count()==1&&"123456789¹²³".contains(tail)));
 if name.is_empty()||name.trim()!=name||name.encode_utf16().count()>255||name.ends_with('.')||name.chars().any(|c|c.is_control()||"<>:\"/\\|?*".contains(c))||reserved||name=="."||name==".."{return Err("名称无效：不能含路径分隔符、保留设备名、控制字符或末尾空格和句点".into())}Ok(())
}
fn display_name(name:&str)->Result<(),String>{if name.trim().is_empty()||name.chars().count()>120||name.chars().any(char::is_control){Err("项目名称需要 1–120 个字符，且不能包含控制字符".into())}else{Ok(())}}
pub(crate) fn is_link(meta:&fs::Metadata)->bool{
 #[cfg(windows)]{use std::os::windows::fs::MetadataExt;meta.file_attributes()&0x400!=0}
 #[cfg(not(windows))]{meta.file_type().is_symlink()}
}
pub(crate) fn canonical(path:&Path)->Result<PathBuf,String>{if !path.is_absolute(){return Err("必须使用绝对路径".into())}fs::canonicalize(path).map(|p|PathBuf::from(display(&p))).map_err(|e|format!("目录或文件不可访问 {}：{e}",display(path)))}
fn token()->String{format!("{:x}-{:x}",SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(),std::process::id())}
fn on_thread<T:Send+'static>(f:impl FnOnce()->Result<T,String>+Send+'static)->Result<T,String>{std::thread::spawn(f).join().map_err(|_|"Windows 文件操作线程异常".to_string())?}

impl Service {
 pub fn new(pool:SqlitePool,app_root:PathBuf,journal:PathBuf)->Self{Self{pool,app_root,journal,verification_root:None}}
 #[cfg(debug_assertions)] pub fn for_verification(mut self,root:&Path)->Result<Self,String>{let root=canonical(root)?;let boundary=canonical(&self.app_root.join("development/verification"))?;if !within(&root,&boundary)||key(&root)==key(&boundary){return Err("项目验收目录必须位于第二版 development/verification 的独立子目录内".into())}self.verification_root=Some(root);Ok(self)}
 fn default_root(&self)->PathBuf{self.verification_root.as_ref().map(|p|p.join("Projects")).unwrap_or_else(||if cfg!(debug_assertions){self.app_root.join("SciDeskData/Projects")}else{self.journal.parent().unwrap().parent().unwrap().join("Projects")})}
 fn prepare_default_root(&self)->Result<(),String>{if !cfg!(debug_assertions){return crate::personal_seed::prepare_project_directory(&self.default_root())}if self.verification_root.is_some(){let path=self.default_root();return match fs::symlink_metadata(&path){Ok(m) if m.is_dir()&&!is_link(&m)=>Ok(()),Ok(_)=>Err("验收项目目录不是普通文件夹".into()),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>fs::create_dir(path).map_err(err),Err(e)=>Err(e.to_string())}}let mut path=canonical(&self.app_root)?;for part in ["SciDeskData","Projects"]{path.push(part);match fs::symlink_metadata(&path){Ok(meta) if meta.is_dir()&&!is_link(&meta)=>{},Ok(_)=>return Err("默认目录不是普通文件夹，请选择其他保存位置".into()),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>fs::create_dir(&path).map_err(|e|format!("默认目录不可写，请选择其他保存位置：{e}"))?,Err(e)=>return Err(e.to_string())}}Ok(())}
 fn protect(&self,path:&Path)->Result<(),String>{
  let old=self.app_root.parent().ok_or("应用路径无父目录")?.join("SciDesk");
  if within(path,&old)||within(&old,path){return Err("第一版 SciDesk 及其父目录受只读保护".into())}
  if within(&self.app_root,path)||(within(path,&self.app_root)&&!within(path,&self.default_root())){return Err("此位置属于第二版程序源码或运行文件，请选择独立的项目目录".into())}Ok(())
 }
 async fn lock(&self)->Result<Transaction<'_,Sqlite>,String>{let mut tx=self.pool.begin().await.map_err(err)?;sqlx::query("UPDATE settings SET value=value WHERE key='projects.operation_lock'").execute(&mut *tx).await.map_err(err)?;Ok(tx)}
 pub async fn list(&self)->Result<Vec<Project>,String>{sqlx::query_as(&format!("SELECT {COLUMNS} FROM projects WHERE deleted_at IS NULL ORDER BY sort_order,created_at,id")).fetch_all(&self.pool).await.map_err(err)}
 async fn get(&self,id:&str)->Result<Project,String>{sqlx::query_as(&format!("SELECT {COLUMNS} FROM projects WHERE id=? AND deleted_at IS NULL")).bind(id).fetch_optional(&self.pool).await.map_err(err)?.ok_or("项目登记不存在，请刷新列表".into())}
 async fn distinct(&self,path:&Path,except:&str)->Result<(),String>{for p in self.list().await?{if p.id!=except&&(within(path,Path::new(&p.root_path))||within(Path::new(&p.root_path),path)){return Err("此目录已登记，或与已登记项目相互包含；请选择独立目录".into())}}Ok(())}
 fn validate_project(p:&Project)->Result<(),String>{display_name(&p.name)?;if p.description.chars().count()>2000||p.description.chars().any(|c|c.is_control()&&!matches!(c,'\n'|'\r'|'\t')){return Err("项目描述不能超过 2000 个字符，且不能含无效控制字符".into())}if !ICONS.contains(&p.icon_name.as_str()){return Err("未知线条图标".into())}if p.project_color.len()!=7||!p.project_color.starts_with('#')||!p.project_color[1..].bytes().all(|b|b.is_ascii_hexdigit()){return Err("图标颜色必须为 #RRGGBB".into())}if !["active","completed","idea"].contains(&p.status.as_str())||!(0..=100).contains(&p.progress){return Err("项目状态或进度无效".into())}Ok(())}
 fn journal_file(&self,p:&Pending)->PathBuf{self.journal.join(format!("{}.json",p.project.id))}
 fn record(&self,p:&Pending)->Result<(),String>{fs::create_dir_all(&self.journal).map_err(err)?;let target=self.journal_file(p);if target.exists(){return Err("该项目有未完成的文件系统操作，请先重新同步".into())}let mut f=fs::OpenOptions::new().write(true).create_new(true).open(target).map_err(err)?;f.write_all(&serde_json::to_vec(p).map_err(err)?).and_then(|_|f.sync_all()).map_err(err)}
 fn done(&self,p:&Pending)->Result<(),String>{fs::remove_file(self.journal_file(p)).map_err(err)}
 fn mark_done(&self,p:&mut Pending)->Result<(),String>{p.fs_done=true;let temporary=self.journal.join(format!("{}.{}.tmp",p.project.id,token()));let mut f=fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(err)?;f.write_all(&serde_json::to_vec(p).map_err(err)?).and_then(|_|f.sync_all()).map_err(err)?;drop(f);project_platform::replace_record(&temporary,&self.journal_file(p))}
 fn pending(&self)->Result<Vec<Pending>,String>{if !self.journal.exists(){return Ok(vec![])}fs::read_dir(&self.journal).map_err(err)?.filter(|entry|entry.as_ref().map_or(true,|e|e.path().extension().is_some_and(|v|v=="json"))).map(|entry|{let entry=entry.map_err(err)?;serde_json::from_slice(&fs::read(entry.path()).map_err(err)?).map_err(|e|format!("操作恢复记录不可读，请保留 {}：{e}",entry.path().display()))}).collect()}
 async fn insert(p:&Project,tx:&mut Transaction<'_,Sqlite>)->Result<(),String>{sqlx::query("INSERT INTO projects(id,name,description,root_path,root_key,icon_name,project_color,status,progress,progress_enabled,sort_order,source_type) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)").bind(&p.id).bind(&p.name).bind(&p.description).bind(&p.root_path).bind(key(Path::new(&p.root_path))).bind(&p.icon_name).bind(&p.project_color).bind(&p.status).bind(p.progress).bind(p.progress_enabled).bind(p.sort_order).bind(&p.source_type).execute(&mut **tx).await.map_err(err)?;Ok(())}
 async fn select_in(id:&str,tx:&mut Transaction<'_,Sqlite>)->Result<(),String>{sqlx::query("INSERT INTO settings(key,value) VALUES('projects.selected_id',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value").bind(id).execute(&mut **tx).await.map_err(err)?;Ok(())}
 fn resolve(&self,p:&Project,relative:&str)->Result<PathBuf,String>{
  let registered=Path::new(&p.root_path);let root=canonical(registered)?;
  if key(&root)!=key(registered){return Err("项目目录已被替换或移动，请重新定位目录".into())}self.protect(&root)?;
  let rel=Path::new(relative);if rel.components().any(|c|!matches!(c,Component::Normal(_)|Component::CurDir)){return Err("只能浏览当前项目内部的相对路径".into())}
  let mut current=root.clone();for c in rel.components(){if let Component::Normal(s)=c{let name=s.to_str().ok_or("文件名编码不可读")?;validate_name(name)?;current.push(s);let meta=fs::symlink_metadata(&current).map_err(err)?;if is_link(&meta){return Err("为避免越界，不在项目内操作符号链接或目录联接".into())}}}
  let path=canonical(&current)?;if !within(&path,&root){return Err("路径超出了当前项目".into())}self.protect(&path)?;Ok(path)
 }
 pub async fn execute(&self,op:&str,args:Value)->Result<Value,String>{
 if let Some(op)=op.strip_prefix("tools_"){return crate::tools::command(&self.journal.parent().ok_or("工具数据目录无效")?.join("tools"),op,args).await}
 if let Some(op)=op.strip_prefix("learning_"){return crate::learning::command(&self.journal.parent().ok_or("学习数据目录无效")?.join("learning"),op,args).await}
 if let Some(action)=op.strip_prefix("research_"){return self.research(action,args).await}
 if op=="manual_book_new" {
  self.prepare_default_root()?;
  let metadata=self.journal.parent().ok_or("手册数据目录无效")?.join("manuals");
  return crate::manuals::new_book(&metadata,&self.default_root()).await;
 }
 if let Some(op)=op.strip_prefix("manual_"){return crate::manuals::command(&self.journal.parent().ok_or("手册数据目录无效")?.join("manuals"),op,args).await}
 match op{
  "literature_prepare"|"literature_write"=>self.literature(op,args).await,
  "bootstrap"=>{let projects=self.list().await?;let selected:Option<String>=sqlx::query_scalar("SELECT value FROM settings WHERE key='projects.selected_id'").fetch_optional(&self.pool).await.map_err(err)?;let selected=selected.filter(|id|projects.iter().any(|p|&p.id==id));let pending=self.pending()?.into_iter().map(|p|json!({"id":p.project.id,"kind":p.kind,"old_path":p.old_path,"new_path":p.new_path,"fs_done":p.fs_done})).collect::<Vec<_>>();Ok(json!({"projects":projects,"selected_id":selected,"default_root":display(&self.default_root()),"pending":pending}))},
  "pick_directory"=>{let start=args.get("start").and_then(Value::as_str).map(PathBuf::from);on_thread(move||project_platform::pick(start.as_deref())).map(|p|json!(p))},
  "select"=>{let id=text(&args,"id")?;self.get(&id).await?;let mut tx=self.lock().await?;Self::select_in(&id,&mut tx).await?;tx.commit().await.map_err(err)?;Ok(json!(true))},
  "register"=>self.register(args).await,
  "update"=>self.update(args).await,
  "rename_project"=>self.rename_project(args).await,
  "remove"|"trash_project"=>self.remove(op,args).await,
  "relink"=>{let p=self.get(&text(&args,"id")?).await?;let root=canonical(Path::new(&text(&args,"path")?))?;if !root.is_dir(){return Err("请选择文件夹".into())}self.protect(&root)?;let mut tx=self.lock().await?;self.distinct(&root,&p.id).await?;sqlx::query("UPDATE projects SET root_path=?,root_key=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(display(&root)).bind(key(&root)).bind(&p.id).execute(&mut *tx).await.map_err(err)?;tx.commit().await.map_err(err)?;Ok(json!(true))},
  "reorder"=>{let ids:Vec<String>=serde_json::from_value(args.get("ids").cloned().ok_or("缺少排序列表")?).map_err(err)?;let mut tx=self.lock().await?;let actual=self.list().await?;let wanted=actual.iter().map(|p|p.id.clone()).collect::<HashSet<_>>();if ids.len()!=wanted.len()||ids.iter().cloned().collect::<HashSet<_>>()!=wanted{return Err("项目列表已变化，请刷新后重新排序".into())}for (order,id) in ids.iter().enumerate(){sqlx::query("UPDATE projects SET sort_order=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(order as i64).bind(id).execute(&mut *tx).await.map_err(err)?;}tx.commit().await.map_err(err)?;Ok(json!(true))},
  "list_files"=>self.list_files(args).await,
  "read_drop_file"=>tauri::async_runtime::spawn_blocking(move||crate::file_transfer::read_drop_file(args)).await.map_err(err)?,
  "drop_files"=>{
   let p=self.get(&text(&args,"id")?).await?;let destination=self.resolve(&p,args["relative"].as_str().unwrap_or(""))?;
   let sources=crate::file_transfer::paths(&args)?;let guard=crate::file_transfer::Guard::new(vec![PathBuf::from(p.root_path)])?;
   tauri::async_runtime::spawn_blocking(move||crate::file_transfer::transfer(guard,destination,sources,args)).await.map_err(err)?
  },
  "drag_paths"=>{
   let p=self.get(&text(&args,"id")?).await?;let relatives:Vec<String>=serde_json::from_value(args["relatives"].clone()).map_err(err)?;
   if relatives.is_empty()||relatives.len()>256||relatives.iter().any(String::is_empty){return Err("请选择 1–256 个文件或子文件夹，不拖出项目根目录".into())}
   let paths=relatives.iter().map(|r|self.resolve(&p,r)).collect::<Result<Vec<_>,_>>()?;let guard=crate::file_transfer::Guard::new(vec![PathBuf::from(p.root_path)])?;
   let paths=tauri::async_runtime::spawn_blocking(move||crate::file_transfer::prepare_drag(guard,paths)).await.map_err(err)??;
   Ok(json!(paths))
  },
  "create_entry"|"rename_entry"|"trash_entry"|"open"|"reveal"=>self.file_op(op,args).await,
  "quick_create"=>self.quick_create(args).await,
  "recover"=>self.recover(&text(&args,"id")?).await,
  _=>Err("不支持的项目操作".into())
 }}
 async fn register(&self,args:Value)->Result<Value,String>{
  let name=text(&args,"name")?;let imported=args.get("imported").and_then(Value::as_bool).unwrap_or(false);
  let root=if imported{canonical(Path::new(&text(&args,"path")?))?}else{validate_name(&name)?;let parent=args.get("parent").and_then(Value::as_str).filter(|s|!s.is_empty()).map(PathBuf::from).unwrap_or_else(||self.default_root());if key(&parent)==key(&self.default_root()){self.prepare_default_root()?;}canonical(&parent)?.join(&name)};
  self.protect(&root)?;if imported&&!root.is_dir(){return Err("请选择现有文件夹".into())}if !imported&&root.exists(){return Err("同名文件或文件夹已存在，请更改名称或导入现有文件夹".into())}
  let mut p=Project{id:token(),name,description:description(&args,"")?,root_path:display(&root),icon_name:args.get("icon_name").and_then(Value::as_str).unwrap_or("folder").into(),project_color:args.get("project_color").and_then(Value::as_str).unwrap_or("#2563eb").into(),status:args.get("status").and_then(Value::as_str).unwrap_or("active").into(),progress:integer(&args,"progress",0)?,progress_enabled:args.get("progress_enabled").and_then(Value::as_bool).unwrap_or(false),sort_order:0,source_type:if imported{"imported"}else{"created"}.into(),created_at:String::new(),updated_at:String::new()};Self::validate_project(&p)?;
  let mut tx=self.lock().await?;self.distinct(&root,"").await?;p.sort_order=sqlx::query_scalar::<_,i64>("SELECT COALESCE(MAX(sort_order),-1)+1 FROM projects").fetch_one(&mut *tx).await.map_err(err)?;
  let pending=Pending{kind:"create".into(),project:p.clone(),old_path:String::new(),new_path:p.root_path.clone(),fs_done:false};
  if !imported{self.record(&pending)?;if let Err(error)=fs::create_dir(&root){let _=self.done(&pending);return Err(error.to_string())}}
  let save=async{Self::insert(&p,&mut tx).await?;Self::select_in(&p.id,&mut tx).await?;tx.commit().await.map_err(err)}.await;
  if let Err(error)=save{if !imported{if let Err(rollback)=fs::remove_dir(&root){return Err(format!("目录已创建，但登记失败：{error}；空目录回滚失败：{rollback}。已保留恢复记录，请重新同步。"))}let _=self.done(&pending);}return Err(error)}
  if !imported{self.done(&pending)?;}Ok(json!(self.get(&p.id).await?))
 }
 async fn update(&self,args:Value)->Result<Value,String>{let mut tx=self.lock().await?;let mut p=self.get(&text(&args,"id")?).await?;p.description=description(&args,&p.description)?;for field in ["icon_name","project_color","status"]{if let Some(value)=args.get(field).and_then(Value::as_str){match field{"icon_name"=>p.icon_name=value.into(),"project_color"=>p.project_color=value.into(),_=>p.status=value.into()}}}if args.get("progress").is_some(){p.progress=integer(&args,"progress",0)?}if let Some(b)=args.get("progress_enabled").and_then(Value::as_bool){p.progress_enabled=b}Self::validate_project(&p)?;sqlx::query("UPDATE projects SET description=?,icon_name=?,project_color=?,status=?,progress=?,progress_enabled=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(&p.description).bind(&p.icon_name).bind(&p.project_color).bind(&p.status).bind(p.progress).bind(p.progress_enabled).bind(&p.id).execute(&mut *tx).await.map_err(err)?;tx.commit().await.map_err(err)?;Ok(json!(true))}
 async fn rename_project(&self,args:Value)->Result<Value,String>{
  let mut tx=self.lock().await?;let p=self.get(&text(&args,"id")?).await?;let name=text(&args,"name")?;display_name(&name)?;let only=args.get("display_only").and_then(Value::as_bool).unwrap_or(false);
  if only{sqlx::query("UPDATE projects SET name=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(name).bind(p.id).execute(&mut *tx).await.map_err(err)?;tx.commit().await.map_err(err)?;return Ok(json!(true))}
  validate_name(&name)?;let old=self.resolve(&p,"")?;let new=old.parent().ok_or("不能重命名磁盘根目录")?.join(&name);self.protect(&new)?;if old==new{return Ok(json!(true))}if new.exists()&&key(&old)!=key(&new){return Err("目标名称已存在".into())}self.distinct(&new,&p.id).await?;
  let mut desired=p.clone();desired.name=name.clone();desired.root_path=display(&new);
  let pending=Pending{kind:"rename".into(),project:desired,old_path:display(&old),new_path:display(&new),fs_done:false};self.record(&pending)?;
  if let Err(error)=project_platform::rename_no_replace(&old,&new){let _=self.done(&pending);return Err(error)}
  let save=async{sqlx::query("UPDATE projects SET name=?,root_path=?,root_key=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(name).bind(display(&new)).bind(key(&new)).bind(&p.id).execute(&mut *tx).await.map_err(err)?;tx.commit().await.map_err(err)}.await;
  if let Err(error)=save{if let Err(rollback)=project_platform::rename_no_replace(&new,&old){return Err(format!("真实目录已改名，但登记更新失败：{error}；回滚失败：{rollback}。请使用重新同步恢复准确路径。"))}let _=self.done(&pending);return Err(format!("重命名登记失败，目录已恢复原名：{error}"))}self.done(&pending)?;Ok(json!(true))
 }
 async fn remove(&self,op:&str,args:Value)->Result<Value,String>{let mut tx=self.lock().await?;let p=self.get(&text(&args,"id")?).await?;let mut pending=Pending{kind:"trash".into(),project:p.clone(),old_path:p.root_path.clone(),new_path:String::new(),fs_done:false};if op=="trash_project"{if args.get("confirm_path").and_then(Value::as_str)!=Some(&p.root_path){return Err("请先确认完整项目路径".into())}let root=self.resolve(&p,"")?;self.record(&pending)?;let result=on_thread(move||project_platform::recycle(&root));if let Err(error)=result{let _=self.done(&pending);return Err(error)}if let Err(error)=self.mark_done(&mut pending){return Err(format!("回收操作已完成，但恢复记录更新失败：{error}；项目登记已保留，请检查路径后重新同步。"))}}
  let save=async{sqlx::query("DELETE FROM projects WHERE id=?").bind(&p.id).execute(&mut *tx).await.map_err(err)?;sqlx::query("DELETE FROM settings WHERE key='projects.selected_id' AND value=?").bind(&p.id).execute(&mut *tx).await.map_err(err)?;tx.commit().await.map_err(err)}.await;
  if let Err(error)=save{return Err(if op=="trash_project"{format!("目录已进入回收站，但登记更新失败：{error}。登记已保留，请重新同步。")}else{error})}if op=="trash_project"{self.done(&pending)?}Ok(json!(true))
 }
 async fn list_files(&self,args:Value)->Result<Value,String>{let p=self.get(&text(&args,"id")?).await?;let relative=args.get("relative").and_then(Value::as_str).unwrap_or("");let dir=self.resolve(&p,relative)?;let mut entries=vec![];for entry in fs::read_dir(&dir).map_err(|e|format!("无法读取目录：{e}（可使用重新定位目录）"))?{let e=entry.map_err(err)?;let meta=fs::symlink_metadata(e.path()).map_err(err)?;let name=e.file_name().to_string_lossy().into_owned();entries.push(json!({"name":name,"is_dir":meta.is_dir(),"is_link":is_link(&meta),"size":if meta.is_dir(){None}else{Some(meta.len())},"modified":meta.modified().ok().and_then(|t|t.duration_since(UNIX_EPOCH).ok()).map(|n|n.as_millis() as u64),"extension":e.path().extension().map(|s|s.to_string_lossy().into_owned()).unwrap_or_default()}))}Ok(json!({"path":display(&dir),"relative":relative,"entries":entries}))}
 async fn file_op(&self,op:&str,args:Value)->Result<Value,String>{let _guard=self.lock().await?;let p=self.get(&text(&args,"id")?).await?;let relative=args.get("relative").and_then(Value::as_str).unwrap_or("");let path=self.resolve(&p,relative)?;
  match op{"create_entry"=>{if !path.is_dir(){return Err("目标不是文件夹".into())}let name=text(&args,"name")?;validate_name(&name)?;let new=path.join(name);self.protect(&new)?;match args.get("kind").and_then(Value::as_str){Some("folder")=>fs::create_dir(new).map_err(err)?,Some("file")=>fs::OpenOptions::new().write(true).create_new(true).open(new).and_then(|file|file.sync_all()).map_err(err)?,_=>return Err("请选择文件或文件夹".into())}},
  "rename_entry"=>{if relative.is_empty(){return Err("请通过项目菜单重命名项目根目录".into())}let name=text(&args,"name")?;validate_name(&name)?;let new=path.parent().ok_or("没有父目录")?.join(name);self.protect(&new)?;project_platform::rename_no_replace(&path,&new)?},
  "trash_entry"=>{if relative.is_empty(){return Err("不能通过文件菜单删除项目根目录".into())}if args.get("confirm_path").and_then(Value::as_str)!=Some(display(&path).as_str()){return Err("请先确认完整文件路径".into())}on_thread(move||project_platform::recycle(&path))?;},
  "open"=>on_thread(move||project_platform::open(&path))?,
  "reveal"=>on_thread(move||project_platform::reveal(&path))?,_=>unreachable!()};Ok(json!(true))
 }
 async fn quick_create(&self,args:Value)->Result<Value,String>{let _guard=self.lock().await?;let p=self.get(&text(&args,"id")?).await?;let root=self.resolve(&p,"")?;let names:Vec<String>=serde_json::from_value(args.get("names").cloned().ok_or("缺少目录列表")?).map_err(err)?;if names.is_empty()||names.len()>100{return Err("请选择 1–100 个目录".into())}let mut unique=HashSet::new();for name in &names{validate_name(name)?;if !unique.insert(name.to_lowercase()){return Err("目录列表存在重复名称".into())}}
  let mut created=vec![];let mut skipped=vec![];let mut failed=vec![];for name in names{let target=root.join(&name);match fs::symlink_metadata(&target){Ok(m) if m.is_dir()&&!is_link(&m)=>skipped.push(name),Ok(_)=>failed.push(json!({"name":name,"error":"同名对象不是普通目录，未覆盖"})),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>match fs::create_dir(&target){Ok(())=>created.push(name),Err(e)=>failed.push(json!({"name":name,"error":e.to_string()}))},Err(e)=>failed.push(json!({"name":name,"error":e.to_string()}))}}Ok(json!({"created":created,"skipped":skipped,"failed":failed}))
 }
 async fn recover(&self,id:&str)->Result<Value,String>{let pending=self.pending()?.into_iter().find(|p|p.project.id==id).ok_or("没有待同步记录")?;let mut tx=self.lock().await?;match pending.kind.as_str(){
  "rename"=>{let old=Path::new(&pending.old_path);let new=Path::new(&pending.new_path);if old.is_dir()&&!new.exists(){tx.rollback().await.map_err(err)?;self.done(&pending)?;return Ok(json!(true))}if old.exists()||!new.is_dir(){return Err("目录状态存在歧义，请先人工检查原路径和新路径，再使用重新定位".into())}self.protect(new)?;self.distinct(new,id).await?;sqlx::query("UPDATE projects SET name=?,root_path=?,root_key=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(&pending.project.name).bind(&pending.new_path).bind(key(new)).bind(id).execute(&mut *tx).await.map_err(err)?;},
  "create"=>{let target=canonical(Path::new(&pending.new_path))?;self.protect(&target)?;if !target.is_dir(){return Err("已创建的目录不存在，请人工检查恢复记录".into())}if self.get(id).await.is_err(){self.distinct(&target,id).await?;Self::insert(&pending.project,&mut tx).await?;}},
  "trash"=>{if !pending.fs_done||Path::new(&pending.old_path).exists(){return Err("无法确认回收已完成，登记已保留；请检查磁盘目录后重新定位".into())}sqlx::query("DELETE FROM projects WHERE id=?").bind(id).execute(&mut *tx).await.map_err(err)?;},_=>return Err("未知恢复类型".into())};tx.commit().await.map_err(err)?;self.done(&pending)?;Ok(json!(true))}
}

#[cfg(test)]
mod tests;
