use std::path::Path;

#[cfg(windows)]
mod win {
 use super::*;
 use windows::{core::{w,PCWSTR},Win32::{System::Com::*,UI::{Shell::*,WindowsAndMessaging::SW_SHOWNORMAL}}};
 struct Com;
 impl Drop for Com {fn drop(&mut self){unsafe{CoUninitialize()}}}
 fn apartment()->Result<Com,String>{unsafe{CoInitializeEx(None,COINIT_APARTMENTTHREADED).ok().map_err(|e|e.to_string())?;}Ok(Com)}
 fn wide(path:&Path)->Vec<u16>{use std::os::windows::ffi::OsStrExt;path.as_os_str().encode_wide().chain(Some(0)).collect()}
 pub fn rename_no_replace(from:&Path,to:&Path)->Result<(),String>{let from=wide(from);let to=wide(to);unsafe{windows::Win32::Storage::FileSystem::MoveFileW(PCWSTR(from.as_ptr()),PCWSTR(to.as_ptr()))}.map_err(|e|e.to_string())}
 pub fn replace_record(from:&Path,to:&Path)->Result<(),String>{use windows::Win32::Storage::FileSystem::{MoveFileExW,MOVEFILE_REPLACE_EXISTING,MOVEFILE_WRITE_THROUGH};let from=wide(from);let to=wide(to);unsafe{MoveFileExW(PCWSTR(from.as_ptr()),PCWSTR(to.as_ptr()),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH)}.map_err(|e|e.to_string())}
 pub fn recycle(path:&Path)->Result<(),String>{
  let _com=apartment()?;let path=wide(path);
  unsafe{
   let op:IFileOperation=CoCreateInstance(&FileOperation,None,CLSCTX_INPROC_SERVER).map_err(|e|e.to_string())?;
   op.SetOperationFlags(FOF_ALLOWUNDO|FOFX_RECYCLEONDELETE|FOFX_EARLYFAILURE|FOF_NOCONFIRMATION|FOF_NOERRORUI|FOF_SILENT).map_err(|e|e.to_string())?;
   let item:IShellItem=SHCreateItemFromParsingName(PCWSTR(path.as_ptr()),None).map_err(|e|e.to_string())?;
   op.DeleteItem(&item,None).map_err(|e|e.to_string())?;
   op.PerformOperations().map_err(|e|format!("无法移入回收站（未使用永久删除）：{e}"))?;
   if op.GetAnyOperationsAborted().map_err(|e|e.to_string())?.as_bool(){return Err("回收操作已取消".into())}
  }Ok(())
 }
 pub fn pick(start:Option<&Path>)->Result<Option<String>,String>{pick_local(start,true,"选择项目文件夹")}
 pub fn pick_file(start:Option<&Path>)->Result<Option<String>,String>{pick_local(start,false,"选择手册文件")}
 pub fn pick_pdf()->Result<Option<String>,String>{pick_local(None,false,"打开本地 PDF")}
 pub fn pick_learning_backup()->Result<Option<String>,String>{pick_local(None,false,"选择学习资源备份文件")}
 pub fn pick_tool_folder()->Result<Option<String>,String>{pick_local(None,true,"选择包含 scidesk-tool.json 的工具文件夹")}
 fn pick_local(start:Option<&Path>,folder:bool,title:&str)->Result<Option<String>,String>{
  let _com=apartment()?;
  unsafe{
   let dialog:IFileOpenDialog=CoCreateInstance(&FileOpenDialog,None,CLSCTX_INPROC_SERVER).map_err(|e|e.to_string())?;
   let options=FOS_FORCEFILESYSTEM|FOS_PATHMUSTEXIST|FOS_DONTADDTORECENT;
   dialog.SetOptions(if folder{options|FOS_PICKFOLDERS}else{options|FOS_FILEMUSTEXIST}).map_err(|e|e.to_string())?;
   let title:Vec<u16>=title.encode_utf16().chain(Some(0)).collect();dialog.SetTitle(PCWSTR(title.as_ptr())).map_err(|e|e.to_string())?;
   if let Some(start)=start.filter(|p|p.is_dir()){let text=wide(start);if let Ok(item)=SHCreateItemFromParsingName::<_,_,IShellItem>(PCWSTR(text.as_ptr()),None){let _=dialog.SetFolder(&item);}}
   if let Err(error)=dialog.Show(None){if error.code().0 as u32==0x800704c7{return Ok(None)}return Err(error.to_string())}
   let text=dialog.GetResult().and_then(|item|item.GetDisplayName(SIGDN_FILESYSPATH)).map_err(|e|e.to_string())?;
   let result=text.to_string().map_err(|e|e.to_string());CoTaskMemFree(Some(text.0.cast()));result.map(Some)
  }
 }
 pub fn open(path:&Path)->Result<(),String>{let path=wide(path);let code=unsafe{ShellExecuteW(None,w!("open"),PCWSTR(path.as_ptr()),PCWSTR::null(),PCWSTR::null(),SW_SHOWNORMAL)}.0 as isize;if code<=32{Err(format!("Windows 默认应用打开失败，错误码 {code}"))}else{Ok(())}}
 pub fn reveal(path:&Path)->Result<(),String>{use std::os::windows::process::CommandExt;std::process::Command::new("explorer.exe").arg(format!("/select,{}",path.display())).creation_flags(0x08000000).spawn().map(|_|()).map_err(|e|e.to_string())}
}
#[cfg(windows)]pub use win::*;
#[cfg(not(windows))]pub fn recycle(_: &Path)->Result<(),String>{Err("此平台未配置回收站；未执行永久删除".into())}
#[cfg(not(windows))]pub fn pick(_:Option<&Path>)->Result<Option<String>,String>{Err("目录选择器需要 Windows 桌面环境".into())}
#[cfg(not(windows))]pub fn pick_file(_:Option<&Path>)->Result<Option<String>,String>{Err("文件选择器需要 Windows 桌面环境".into())}
#[cfg(not(windows))]pub fn pick_pdf()->Result<Option<String>,String>{Err("文件选择器需要 Windows".into())}
#[cfg(not(windows))]pub fn pick_learning_backup()->Result<Option<String>,String>{Err("文件选择器需要 Windows 桌面环境".into())}
#[cfg(not(windows))]pub fn pick_tool_folder()->Result<Option<String>,String>{Err("工具文件夹选择需要 Windows 桌面环境".into())}
#[cfg(not(windows))]pub fn open(_: &Path)->Result<(),String>{Err("需要 Windows 默认应用".into())}
#[cfg(not(windows))]pub fn reveal(_: &Path)->Result<(),String>{Err("需要 Windows 文件资源管理器".into())}
#[cfg(not(windows))]pub fn rename_no_replace(from:&Path,to:&Path)->Result<(),String>{if to.exists(){return Err("目标名称已存在".into())}std::fs::rename(from,to).map_err(|e|e.to_string())}
#[cfg(not(windows))]pub fn replace_record(from:&Path,to:&Path)->Result<(),String>{std::fs::rename(from,to).map_err(|e|e.to_string())}
