//! Non-destructive upgrade from the previous v2 user directory. Original data is
//! retained. SQLite is backed up through a read transaction, including live WAL.
use std::{fs,io::{Read,Write},path::{Path,PathBuf}};
use serde_json::{Value,json};
use sqlx::{Connection,SqliteConnection,sqlite::SqliteConnectOptions};
use crate::project_service::{display,is_link,key,within};
mod rebase;
type Result<T>=std::result::Result<T,String>;
const MARKER:&str="storage-migration-v1.json";
fn error(e:impl std::fmt::Display)->String{e.to_string()}
pub fn writable_root(root:&Path)->Result<()>{
 crate::personal_seed::prepare_project_directory(root)?;
 let probe=root.join(format!(".write-check-{}",uuid::Uuid::new_v4()));
 let file=fs::OpenOptions::new().write(true).create_new(true).open(&probe).map_err(|e|format!("数据目录不可写 {}：{e}。请将软件安装到当前用户可写的目录。",display(root)))?;
 file.sync_all().map_err(error)?;drop(file);fs::remove_file(probe).map_err(error)
}
fn ordinary(path:&Path)->Result<fs::Metadata>{let m=fs::symlink_metadata(path).map_err(error)?;if is_link(&m)||!m.is_file()&&!m.is_dir(){return Err(format!("迁移不经过链接或特殊文件：{}",display(path)))}Ok(m)}
fn sidecar(path:&Path)->bool{
 let Some(name)=path.file_name().and_then(|s|s.to_str())else{return false};
 let Some(base)=["-wal","-shm","-journal"].iter().find_map(|s|name.strip_suffix(s))else{return false};
 let mut header=[0u8;16];fs::File::open(path.with_file_name(base)).and_then(|mut f|f.read_exact(&mut header)).is_ok()&&&header==b"SQLite format 3\0"
}
pub fn prepare_paths(root:&Path)->Result<()>{
 for relative in ["database","database/learning","database/manuals","database/tools","ideas","journals","tools","Projects","Exports/Screenshots","Exports/PlotDigitizer","cache/webview","logs"]{crate::personal_seed::prepare_project_directory(&root.join(relative))?;}
 for relative in ["database/scidesk.db","database/learning/learning.db","database/manuals/manuals.db","database/tools/tools.db","ideas/ideas.db","journals/journals.db"]{let path=root.join(relative);if path.exists(){let meta=ordinary(&path)?;if !meta.is_file(){return Err(format!("元数据文件位置异常：{}",display(&path)))}}}Ok(())
}
async fn database_copy(source:&Path,target:&Path)->Result<()>{
 let mut source_db=SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(source).read_only(true).busy_timeout(std::time::Duration::from_secs(15))).await.map_err(error)?;
 let result=sqlx::query("VACUUM INTO ?").bind(display(target)).execute(&mut source_db).await.map_err(error);source_db.close().await.map_err(error)?;result?;
 let mut copied=SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(target).read_only(true)).await.map_err(error)?;
 let check:String=sqlx::query_scalar("PRAGMA quick_check").fetch_one(&mut copied).await.map_err(error)?;copied.close().await.map_err(error)?;if check!="ok"{return Err(format!("数据库备份校验失败：{check}"))}Ok(())
}
async fn copy_tree(source:&Path,target:&Path)->Result<()>{
 let mut stack=vec![(source.to_owned(),target.to_owned(),0)];let mut count=0usize;
 while let Some((from,to,depth))=stack.pop(){
  count+=1;if count>200000||depth>128{return Err("旧数据目录超过 200000 项或 128 层，已停止迁移并保留原目录".into())}
  let metadata=ordinary(&from)?;
  if metadata.is_dir(){fs::create_dir(&to).map_err(error)?;for entry in fs::read_dir(from).map_err(error)?{let entry=entry.map_err(error)?;let name=entry.file_name();if sidecar(&entry.path()){continue}stack.push((entry.path(),to.join(name),depth+1));}}
  else{
   let mut header=[0u8;16];let read=fs::File::open(&from).and_then(|mut f|f.read(&mut header)).map_err(error)?;
   if read==16&&&header==b"SQLite format 3\0"{database_copy(&from,&to).await?;}
   else{let mut input=fs::File::open(&from).map_err(error)?;let mut output=fs::OpenOptions::new().write(true).create_new(true).open(&to).map_err(error)?;std::io::copy(&mut input,&mut output).and_then(|_|output.sync_all()).map_err(error)?;if let Ok(modified)=metadata.modified(){output.set_times(fs::FileTimes::new().set_modified(modified)).map_err(error)?;}
    let after=fs::metadata(&from).map_err(error)?;if after.len()!=metadata.len()||after.modified().ok()!=metadata.modified().ok(){return Err(format!("迁移时文件仍在变化，请关闭旧版后重试：{}",display(&from)))}
   }
  }
 }
 Ok(())
}
pub async fn migrate_legacy(source:&Path,destination:&Path)->Result<Value>{
 writable_root(destination)?;
 if destination.join(MARKER).is_file(){return Ok(json!({"status":"already_migrated"}))}
 if !source.join("database/scidesk.db").is_file(){return Ok(json!({"status":"no_legacy_database"}))}
 ordinary(source)?;ordinary(&source.join("database"))?;
 let source=fs::canonicalize(source).map_err(error)?;let destination=fs::canonicalize(destination).map_err(error)?;
 let reference=Path::new(env!("SCIDESK_PROJECT_ROOT")).parent().ok_or("参考目录错误")?.join("SciDesk");
 if within(&source,&reference)||within(&source,&destination)||within(&destination,&source){return Err("旧数据与新数据目录重叠，或指向受保护的第一版目录，已停止迁移".into())}
 let operations=source.join("database/operations");if operations.exists()&&fs::read_dir(&operations).map_err(error)?.any(|e|e.is_ok_and(|e|e.path().extension().is_some_and(|s|s=="json"))){return Err("旧版仍有未完成的项目操作，请先在旧版中恢复；原目录已保留".into())}
 let mut groups=vec![];
 for entry in fs::read_dir(&source).map_err(error)?{
  let entry=entry.map_err(error)?;let name=entry.file_name();if name==MARKER||name.to_string_lossy().starts_with(".migration-"){continue}
  if name=="database"{for child in fs::read_dir(entry.path()).map_err(error)?{let child=child.map_err(error)?;let name=child.file_name();if sidecar(&child.path()){continue}groups.push(PathBuf::from("database").join(name));}}
  else{groups.push(PathBuf::from(name));}
 }
 groups.sort();let pending=groups.iter().filter(|rel|fs::symlink_metadata(destination.join(rel)).is_err()).cloned().collect::<Vec<_>>();
 let stage=destination.join(format!(".migration-{}",uuid::Uuid::new_v4()));fs::create_dir(&stage).map_err(error)?;
 for relative in &pending{let to=stage.join(relative);if let Some(parent)=to.parent(){crate::personal_seed::prepare_project_directory(parent)?;}copy_tree(&source.join(relative),&to).await.map_err(|e|format!("旧数据未删除。迁移暂存于 {}。{e}",display(&stage)))?;}
 // Path changes occur only inside unpublished copies, never in either live DB.
 rebase::copied_databases(&stage,&source,&destination,&pending).await?;
 let mut migrated=vec![];let mut preserved=vec![];
 for relative in groups{
  let to=destination.join(&relative);if !pending.contains(&relative){preserved.push(display(&relative));continue}
  crate::personal_seed::prepare_project_directory(to.parent().unwrap())?;
  match crate::project_platform::rename_no_replace(&stage.join(&relative),&to){Ok(())=>migrated.push(display(&relative)),Err(_) if to.exists()=>preserved.push(display(&relative)),Err(e)=>return Err(e)}
 }
 let report=json!({"version":1,"source":display(&source),"destination":display(&destination),"migrated":migrated,"preserved":preserved,"source_retained":true});
 let temporary=destination.join(format!(".migration-report-{}.tmp",uuid::Uuid::new_v4()));let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(error)?;file.write_all(&serde_json::to_vec_pretty(&report).map_err(error)?).and_then(|_|file.sync_all()).map_err(error)?;drop(file);
 if let Err(e)=crate::project_platform::rename_no_replace(&temporary,&destination.join(MARKER)){if !destination.join(MARKER).is_file(){return Err(e)}}
 // Only remove empty scaffolding. A concurrent publication loser retains its
 // unpublished backup rather than recursively deleting any data.
 let _=fs::remove_dir(stage.join("database"));let _=fs::remove_dir(&stage);Ok(report)
}
pub fn fatal(message:&str)->!{
 eprintln!("{message}");
 #[cfg(windows)] unsafe{use windows::{core::PCWSTR,Win32::UI::WindowsAndMessaging::{MessageBoxW,MB_ICONERROR,MB_OK}};let message=message.encode_utf16().chain(Some(0)).collect::<Vec<_>>();let title="SciDesk 数据目录错误".encode_utf16().chain(Some(0)).collect::<Vec<_>>();MessageBoxW(None,PCWSTR(message.as_ptr()),PCWSTR(title.as_ptr()),MB_ICONERROR|MB_OK);}
 std::process::exit(1)
}
#[cfg(test)] mod tests;
