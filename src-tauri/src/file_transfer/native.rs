use super::*;
use windows::{core::{implement,BOOL,HRESULT,PCWSTR,w},Win32::{Foundation::*,System::{Com::*,Memory::*,Ole::*,SystemServices::{MODIFIERKEYS_FLAGS,MK_LBUTTON},DataExchange::RegisterClipboardFormatW},UI::Shell::*}};
struct Ole;
impl Ole {fn new()->Result<Self,String>{unsafe{OleInitialize(None)}.map_err(|e|e.to_string())?;Ok(Self)}}
impl Drop for Ole{fn drop(&mut self){unsafe{OleUninitialize()}}}
fn wide(path:&Path)->Vec<u16>{use std::os::windows::ffi::OsStrExt;path.as_os_str().encode_wide().chain(Some(0)).collect()}
unsafe fn item(path:&Path)->windows::core::Result<IShellItem>{let name=wide(path);unsafe{SHCreateItemFromParsingName(PCWSTR(name.as_ptr()),None)}}
pub fn transfer(guard:Guard,mut items:Vec<Item>,moving:bool,keep:bool,owner:usize)->Result<Report,String>{
 let _ole=Ole::new()?;let mut cancelled=false;
 for entry in &mut items{
  if entry.status!="ready"{continue}if cancelled{entry.status="not_attempted".into();entry.message="前一项已取消，未执行".into();continue}
  let source=PathBuf::from(&entry.source);let destination=PathBuf::from(&entry.destination);
  let outcome=(||->Result<bool,String>{
   guard.check(&source)?;inspect(&source)?;guard.check(destination.parent().ok_or("目标目录无效")?)?;
   if destination.exists()&&!keep{entry.status="skipped".into();entry.message="目标出现同名条目，已跳过".into();return Ok(false)}
   unsafe{
    let operation:IFileOperation=CoCreateInstance(&FileOperation,None,CLSCTX_INPROC_SERVER).map_err(|e|e.to_string())?;
    // Direct drops never ask for confirmation or replacement. Keep both on races,
    // report errors inline, and retain Windows' cancellable progress window.
    let flags=FOF_ALLOWUNDO|FOF_NOCONFIRMMKDIR|FOFX_EARLYFAILURE|FOF_NOCONFIRMATION|FOF_NOERRORUI|FOF_RENAMEONCOLLISION;
    operation.SetOperationFlags(flags).map_err(|e|e.to_string())?;
    if owner!=0{operation.SetOwnerWindow(HWND(owner as _)).map_err(|e|e.to_string())?;}
    let from=item(&source).map_err(|e|e.to_string())?;let to=item(destination.parent().unwrap()).map_err(|e|e.to_string())?;let name=wide(Path::new(destination.file_name().unwrap()));
    if moving{operation.MoveItem(&from,&to,PCWSTR(name.as_ptr()),None).map_err(|e|e.to_string())?}else{operation.CopyItem(&from,&to,PCWSTR(name.as_ptr()),None).map_err(|e|e.to_string())?}
    let performed=operation.PerformOperations();let aborted=operation.GetAnyOperationsAborted().map_err(|e|e.to_string())?.as_bool();
    performed.map_err(|e|e.to_string())?;if aborted{return Ok(true)}
   }
   entry.status="completed".into();entry.message=if moving{"Windows 移动已完成"}else{"Windows 复制已完成"}.into();Ok(false)
  })();
  match outcome{Ok(true)=>{cancelled=true;entry.status="cancelled".into();entry.message="Windows 操作已取消或未完全完成，请查看刷新后的目录".into()},Ok(false)=>{},Err(error)=>{entry.status="failed".into();entry.message=error;cancelled=true;}}
 }
 Ok(Report{items,cancelled})
}
#[implement(IDropSource)] pub(super) struct Source;
impl IDropSource_Impl for Source_Impl{
 fn QueryContinueDrag(&self,escape:BOOL,keys:MODIFIERKEYS_FLAGS)->HRESULT{if escape.as_bool(){DRAGDROP_S_CANCEL}else if keys.0&MK_LBUTTON.0==0{DRAGDROP_S_DROP}else{HRESULT(0)}}
 fn GiveFeedback(&self,_:DROPEFFECT)->HRESULT{DRAGDROP_S_USEDEFAULTCURSORS}
}
struct Pidls(Vec<*mut Common::ITEMIDLIST>);
impl Drop for Pidls{fn drop(&mut self){for p in &self.0{unsafe{CoTaskMemFree(Some((*p).cast()))}}}}
pub(super) unsafe fn data_object(paths:&[PathBuf])->Result<IDataObject,String>{
 let mut pidls=Pidls(vec![]);
 for path in paths{let normalized=canonical(path)?;let text=wide(&normalized);let mut pidl=std::ptr::null_mut();unsafe{SHParseDisplayName(PCWSTR(text.as_ptr()),None,&mut pidl,0,None)}.map_err(|e|format!("解析拖出路径失败：{e}"))?;pidls.0.push(pidl);}
 let pointers=pidls.0.iter().map(|p|*p as *const Common::ITEMIDLIST).collect::<Vec<_>>();
 let array=unsafe{SHCreateShellItemArrayFromIDLists(&pointers)}.map_err(|e|format!("构建 Shell 文件集合失败：{e}"))?;
 let data:IDataObject=unsafe{array.BindToHandler(None,&BHID_DataObject)}.map_err(|e|format!("获取 Shell 拖放对象失败：{e}"))?;
 // The default is copy. Explorer still supports explicit Shift-move / Ctrl-copy.
 unsafe{
  let memory=GlobalAlloc(GMEM_MOVEABLE,4).map_err(|e|e.to_string())?;let buffer=GlobalLock(memory);
  if buffer.is_null(){let _=GlobalFree(Some(memory));return Err("不能分配拖放格式内存".into())}
  std::ptr::write_unaligned(buffer.cast::<u32>(),DROPEFFECT_COPY.0);let _=GlobalUnlock(memory);
  let format=FORMATETC{cfFormat:RegisterClipboardFormatW(w!("Preferred DropEffect")) as u16,dwAspect:DVASPECT_CONTENT.0,lindex:-1,tymed:TYMED_HGLOBAL.0 as u32,..Default::default()};
  let medium=STGMEDIUM{tymed:TYMED_HGLOBAL.0 as u32,u:STGMEDIUM_0{hGlobal:memory},..Default::default()};
  if let Err(error)=data.SetData(&format,&medium,true){let _=GlobalFree(Some(memory));return Err(format!("设置默认拖放效果失败：{error}"))}
 }
 Ok(data)
}
pub fn drag(paths:&[PathBuf])->Result<(bool,u32),String>{
 let _ole=Ole::new()?;let data=unsafe{data_object(paths)}?;let source:IDropSource=Source.into();let mut effect=DROPEFFECT_NONE;
 let result=unsafe{DoDragDrop(&data,&source,DROPEFFECT_COPY|DROPEFFECT_MOVE,&mut effect)};
 if result==DRAGDROP_S_CANCEL{return Ok((true,0))}result.ok().map_err(|e|e.to_string())?;Ok((false,effect.0))
}

pub(crate) fn copy_to_clipboard(paths:&[PathBuf])->Result<(),String>{
 let _ole=Ole::new()?;
 unsafe{
  let data=data_object(paths)?;
  OleSetClipboard(&data).map_err(|e|format!("无法写入文件剪贴板：{e}"))?;
  OleFlushClipboard().map_err(|e|format!("无法保留剪贴板文件：{e}"))
 }
}

pub(super) fn paths_from_object(data:&IDataObject)->Result<Vec<PathBuf>,String>{
 use std::{ffi::OsString,os::windows::ffi::OsStringExt};
 let format=FORMATETC{cfFormat:CF_HDROP.0,dwAspect:DVASPECT_CONTENT.0,lindex:-1,tymed:TYMED_HGLOBAL.0 as u32,..Default::default()};
 let mut medium=unsafe{data.GetData(&format)}.map_err(|_|"剪贴板中没有可粘贴的本地文件或文件夹".to_owned())?;
 let result=(||unsafe{
  let handle=HDROP(medium.u.hGlobal.0);
  let count=DragQueryFileW(handle,u32::MAX,None);
  if !(1..=256).contains(&count){return Err("每次可粘贴 1–256 项".to_owned())}
  let mut paths=Vec::with_capacity(count as usize);
  for i in 0..count{
   let len=DragQueryFileW(handle,i,None);
   if len==0||len>32767{return Err("剪贴板文件路径无效".to_owned())}
   let mut buf=vec![0;len as usize+1];
   if DragQueryFileW(handle,i,Some(&mut buf))!=len{return Err("无法读取完整文件路径".to_owned())}
   paths.push(PathBuf::from(OsString::from_wide(&buf[..len as usize])));
  }
  Ok(paths)
 })();
 unsafe{ReleaseStgMedium(&mut medium)};
 result
}

pub(crate) fn clipboard_paths()->Result<Vec<PathBuf>,String>{
 let _ole=Ole::new()?;
 let data=unsafe{OleGetClipboard()}.map_err(|e|format!("无法读取系统剪贴板：{e}"))?;
 paths_from_object(&data)
}
