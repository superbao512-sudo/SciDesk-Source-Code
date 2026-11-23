//! Shared real-file drag/drop operations. No database blobs or filesystem deletion fallback.
use std::{collections::HashSet, fs, path::{Path,PathBuf}};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use base64::Engine;
use std::io::Read;
use tauri::Emitter;
use crate::project_service::{canonical,display,is_link,key,within};
#[cfg(windows)] mod native;
#[cfg(windows)] pub(crate) use native::{clipboard_paths,copy_to_clipboard};
#[cfg(not(windows))] pub(crate) fn clipboard_paths()->Result<Vec<PathBuf>,String>{Err("文件剪贴板需要 Windows".into())}
#[cfg(not(windows))] pub(crate) fn copy_to_clipboard(_: &[PathBuf])->Result<(),String>{Err("文件剪贴板需要 Windows".into())}
pub fn window_event(window:&tauri::Window,event:&tauri::WindowEvent){
 #[cfg(windows)]
 if window.label()=="main"{if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop{paths,position,..})=event{
  use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState,VK_SHIFT,VK_CONTROL};
  let shift=unsafe{GetAsyncKeyState(VK_SHIFT.0 as i32)}<0;let control=unsafe{GetAsyncKeyState(VK_CONTROL.0 as i32)}<0;
  let _=window.emit("scidesk://file-drop",json!({"paths":paths,"position":{"x":position.x,"y":position.y},"mode":if shift&&!control{"move"}else{"copy"}}));
 }}
 #[cfg(not(windows))] let _=(window,event);
}
#[derive(Clone)] pub struct Guard { app:PathBuf, data:PathBuf, allowed:Vec<PathBuf> }
impl Guard {
 pub fn new(allowed:Vec<PathBuf>)->Result<Self,String>{
  let data=crate::runtime_paths::data_root()?;
  #[cfg(debug_assertions)] let allowed={let mut allowed=allowed;if std::env::var_os("SCIDESK_DESKTOP_TEST_ROOT").is_some(){if let Some(root)=data.parent(){allowed.push(root.to_owned());}}allowed};
  Ok(Self{app:crate::project_service::application_root()?,data,allowed})
 }
 pub fn check(&self,path:&Path)->Result<PathBuf,String>{
  let actual=canonical(path)?;
  for parent in path.ancestors(){let meta=fs::symlink_metadata(parent).map_err(|e|e.to_string())?;if is_link(&meta){return Err("文件拖放不经过符号链接或目录联接".into())}}
  if key(path)!=key(&actual){return Err("本地位置发生变化，请刷新后重新拖动".into())}
  let reference=Path::new(env!("SCIDESK_PROJECT_ROOT")).parent().ok_or("参考目录无效")?.join("SciDesk");
  if within(&actual,&reference)||within(&reference,&actual){return Err("第一版 SciDesk 及其父目录受只读保护".into())}
  let user_content=["Projects","Exports"].iter().any(|folder|within(&actual,&self.data.join(folder)));
  if (within(&actual,&self.data)||within(&self.data,&actual))&&!user_content{return Err("数据库、设置、缓存等内部数据不能拖入或拖出".into())}
  let permitted=self.allowed.iter().any(|p|within(&actual,p))||user_content||within(&actual,&self.app.join("SciDeskData/Projects"));
  #[cfg(test)] let permitted=permitted||within(&actual,&Path::new(env!("SCIDESK_PROJECT_ROOT")).join("development"));
  if !permitted&&(within(&actual,&self.app)||within(&self.app,&actual)){return Err("程序文件等内部目录不能作为文件拖放来源或目标".into())}
  Ok(actual)
 }
}
pub fn paths(args:&Value)->Result<Vec<PathBuf>,String>{let paths:Vec<String>=serde_json::from_value(args["paths"].clone()).map_err(|_|"缺少拖入的本地路径")?;if paths.is_empty()||paths.len()>256{return Err("每次拖放需要 1–256 项".into())}Ok(paths.into_iter().map(PathBuf::from).collect())}
pub fn inspect(path:&Path)->Result<(),String>{
 let mut stack=vec![(path.to_path_buf(),0)];let mut count=0;
 while let Some((path,depth))=stack.pop(){count+=1;if count>20000||depth>64{return Err("目录超过 20000 项或 64 层，请分批拖放".into())}let meta=fs::symlink_metadata(&path).map_err(|e|e.to_string())?;if is_link(&meta)||!meta.is_dir()&&!meta.is_file(){return Err(format!("不能拖放链接或特殊文件：{}",display(&path)))}if meta.is_dir(){for child in fs::read_dir(path).map_err(|e|e.to_string())?{stack.push((child.map_err(|e|e.to_string())?.path(),depth+1));}}}Ok(())
}
#[derive(Serialize,Deserialize,Debug)] pub struct Item {pub source:String,pub destination:String,pub status:String,pub message:String}
#[derive(Serialize)] pub struct Report {pub items:Vec<Item>,pub cancelled:bool}
fn plan(guard:&Guard,destination:&Path,sources:&[PathBuf],keep_both:bool,duplicate_in_place:bool)->Result<Vec<Item>,String>{
 if sources.is_empty()||sources.len()>256{return Err("每次拖放需要 1–256 项".into())}
 let destination=guard.check(destination)?;if !destination.is_dir(){return Err("拖放目标必须是已存在的文件夹".into())}
 let mut checked=vec![];let mut seen=HashSet::new();for source in sources{let path=guard.check(source)?;if seen.insert(key(&path)){inspect(&path)?;checked.push(path);}}
 let mut reserved=HashSet::new();let mut result=vec![];
 for source in &checked{
  if checked.iter().any(|p|key(p)!=key(source)&&within(source,p)){continue} // A selected directory already carries its children.
  if source.is_dir()&&within(&destination,source){return Err("不能将文件夹放进它自身或其子目录".into())}
  let name=source.file_name().and_then(|s|s.to_str()).ok_or("不能拖放磁盘根目录")?;crate::project_service::validate_name(name)?;
  let mut target=destination.join(name);let mut status="ready";let mut message=String::new();
  if key(&target)==key(source)&&!duplicate_in_place{status="skipped";message="来源已经位于该文件夹".into();}
  else if target.exists()||reserved.contains(&key(&target)){
   if keep_both{
    let stem=if source.is_dir(){name.to_owned()}else{source.file_stem().unwrap_or_default().to_string_lossy().into_owned()};let ext=if source.is_dir(){String::new()}else{source.extension().map(|s|format!(".{}",s.to_string_lossy())).unwrap_or_default()};
    let mut found=false;for n in 1..10000{target=destination.join(format!("{stem} ({n}){ext}"));if !target.exists()&&!reserved.contains(&key(&target)){found=true;break}}if !found{return Err("无法分配不重名的目标名称".into())}
   }else{status="skipped";message="目标已有同名条目，保留原文件".into();}
  }
  if !reserved.insert(key(&target))&&status=="ready"{status="skipped";message="拖放列表包含重复条目".into();}
  result.push(Item{source:display(source),destination:display(&target),status:status.into(),message});
 }
 Ok(result)
}
pub fn transfer(guard:Guard,destination:PathBuf,sources:Vec<PathBuf>,args:Value)->Result<Value,String>{
 let mode=args["mode"].as_str().ok_or("请选择复制或移动")?;if !matches!(mode,"copy"|"move"){return Err("请选择复制或移动".into())}
 let conflict=args["conflict"].as_str().unwrap_or("skip");if !matches!(conflict,"skip"|"keep"){return Err("重名处理方式无效".into())}
 let duplicate_in_place=mode=="copy"&&conflict=="keep"&&args["duplicateInPlace"]==true;
 let items=plan(&guard,&destination,&sources,conflict=="keep",duplicate_in_place)?;
 #[cfg(windows)] {native::transfer(guard,items,mode=="move",conflict=="keep",args["_native_window"].as_u64().unwrap_or(0) as usize).and_then(|v|serde_json::to_value(v).map_err(|e|e.to_string()))}
 #[cfg(not(windows))] {let _=(items,mode);Err("文件拖放需要 Windows 桌面版".into())}
}
pub fn prepare_drag(guard:Guard,paths:Vec<PathBuf>)->Result<Vec<PathBuf>,String>{
 if paths.is_empty()||paths.len()>256{return Err("每次拖出需要 1–256 项".into())}let paths=paths.iter().map(|p|guard.check(p)).collect::<Result<Vec<_>,_>>()?;for path in &paths{inspect(path)?;}
 Ok(paths)
}
pub fn drag(guard:Guard,paths:Vec<PathBuf>)->Result<Value,String>{
 // The directory preflight ran on a worker before entering the UI thread. Recheck
 // root identities immediately before building the OLE object, without blocking
 // the UI on a second recursive walk.
 let paths=paths.iter().map(|p|guard.check(p)).collect::<Result<Vec<_>,_>>()?;
 #[cfg(windows)] {native::drag(&paths).map(|(cancelled,effect)|json!({"cancelled":cancelled,"effect":effect}))}
 #[cfg(not(windows))] {Err("原生文件拖出需要 Windows 桌面版".into())}
}
// Compatibility adapter for the existing Ideas file-upload pipeline. Read-only,
// bounded and local; native drag/drop otherwise suppresses its HTML File event.
pub fn read_drop_file(args:Value)->Result<Value,String>{
 let path=PathBuf::from(args["path"].as_str().ok_or("缺少拖入文件路径")?);let path=Guard::new(vec![])?.check(&path)?;
 let meta=fs::symlink_metadata(&path).map_err(|e|e.to_string())?;let limit=crate::ideas::store::MAX_ATTACHMENT_BYTES;
 if !meta.is_file()||is_link(&meta)||meta.len()>limit as u64{return Err("想法附件须为不超过 20 MB 的普通文件".into())}
 let mut bytes=vec![];fs::File::open(&path).map_err(|e|e.to_string())?.take(limit as u64+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
 if bytes.len()>limit{return Err("想法附件不能超过 20 MB".into())}
 let mime=match path.extension().and_then(|s|s.to_str()).unwrap_or("").to_lowercase().as_str(){"png"=>"image/png","jpg"|"jpeg"=>"image/jpeg","webp"=>"image/webp","gif"=>"image/gif","bmp"=>"image/bmp","svg"=>"image/svg+xml","tif"|"tiff"=>"image/tiff",_=>"application/octet-stream"};
 Ok(json!({"name":path.file_name().unwrap().to_string_lossy(),"mime":mime,"base64":base64::engine::general_purpose::STANDARD.encode(bytes)}))
}
#[cfg(test)] mod tests;
