//! Windows capture/clipboard/save primitives. No external screenshot executable.
use super::{decode_png, encode_png, checked_pixels, Frame, Rect, WindowInfo};
use std::{os::windows::ffi::OsStrExt,path::Path, time::Duration};
use windows::{core::{w, PCWSTR, BOOL}, Win32::{
 Foundation::{HANDLE, HWND, LPARAM, RECT, GlobalFree, WPARAM},
 Graphics::{Gdi::*, Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWA_CLOAKED}},
 Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS},
 System::{Com::*, DataExchange::*, Memory::*, Threading::GetCurrentProcessId},
 UI::{HiDpi::*, Shell::*, WindowsAndMessaging::*},
}};

struct Dpi(isize);
impl Dpi {fn enter()->Self {Self(unsafe{SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)}.0 as isize)}}
impl Drop for Dpi {fn drop(&mut self){if self.0!=0 {unsafe{SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT(self.0 as _));}}}}
struct Surface {screen:HDC,dc:HDC,bitmap:HBITMAP,old:HGDIOBJ,bits:*mut u8,width:u32,height:u32}
impl Drop for Surface {fn drop(&mut self){unsafe{SelectObject(self.dc,self.old);let _=DeleteObject(self.bitmap.into());let _=DeleteDC(self.dc);ReleaseDC(None,self.screen);}}}
impl Surface {
 fn new(width:u32,height:u32)->Result<Self,String>{
  checked_pixels(width,height)?;
  unsafe {
   let screen=GetDC(None);if screen.is_invalid(){return Err("无法访问 Windows 桌面设备上下文".into())}
   let dc=CreateCompatibleDC(Some(screen));if dc.is_invalid(){ReleaseDC(None,screen);return Err("无法创建截图设备上下文".into())}
   let info=BITMAPINFO{bmiHeader:BITMAPINFOHEADER{biSize:std::mem::size_of::<BITMAPINFOHEADER>() as u32,biWidth:width as i32,biHeight:-(height as i32),biPlanes:1,biBitCount:32,biCompression:BI_RGB.0,..Default::default()},..Default::default()};
   let mut bits=std::ptr::null_mut();
   let bitmap=match CreateDIBSection(Some(screen),&info,DIB_RGB_COLORS,&mut bits,None,0){Ok(bitmap)=>bitmap,Err(e)=>{let _=DeleteDC(dc);ReleaseDC(None,screen);return Err(e.to_string())}};
   let old=SelectObject(dc,bitmap.into());
   Ok(Self{screen,dc,bitmap,old,bits:bits.cast(),width,height})
  }
 }
 fn rgba(&self)->Vec<u8>{
  unsafe{let _=GdiFlush();}
  let mut pixels=unsafe{std::slice::from_raw_parts(self.bits,(self.width*self.height*4) as usize)}.to_vec();
  for p in pixels.chunks_exact_mut(4){p.swap(0,2);p[3]=255;}pixels
 }
}
unsafe extern "system" fn monitor(_:HMONITOR,_:HDC,rect:*mut RECT,data:LPARAM)->BOOL{
 let list=&mut *(data.0 as *mut Vec<Rect>);let r=*rect;
 list.push(Rect{x:r.left,y:r.top,width:(r.right-r.left) as u32,height:(r.bottom-r.top) as u32});BOOL(1)
}
unsafe extern "system" fn window(hwnd:HWND,data:LPARAM)->BOOL{
 if !IsWindowVisible(hwnd).as_bool()||IsIconic(hwnd).as_bool(){return BOOL(1)}
 let mut pid=0;GetWindowThreadProcessId(hwnd,Some(&mut pid));if pid==GetCurrentProcessId(){return BOOL(1)}
 let mut cloaked=0u32;let _=DwmGetWindowAttribute(hwnd,DWMWA_CLOAKED,(&mut cloaked as *mut u32).cast(),4);if cloaked!=0{return BOOL(1)}
 let mut title=[0u16;512];let length=GetWindowTextW(hwnd,&mut title);if length==0{return BOOL(1)}
 let mut rect=RECT::default();if GetWindowRect(hwnd,&mut rect).is_err(){return BOOL(1)}
 let _=DwmGetWindowAttribute(hwnd,DWMWA_EXTENDED_FRAME_BOUNDS,(&mut rect as *mut RECT).cast(),std::mem::size_of::<RECT>() as u32);
 if rect.right-rect.left<32||rect.bottom-rect.top<32{return BOOL(1)}
 let list=&mut *(data.0 as *mut Vec<WindowInfo>);
 list.push(WindowInfo{id:hwnd.0 as usize,title:String::from_utf16_lossy(&title[..length as usize]),rect:Rect{x:rect.left,y:rect.top,width:(rect.right-rect.left) as u32,height:(rect.bottom-rect.top) as u32}});BOOL(1)
}
pub fn capture()->Result<Frame,String>{
 let _dpi=Dpi::enter();
 unsafe {
  let rect=Rect{x:GetSystemMetrics(SM_XVIRTUALSCREEN),y:GetSystemMetrics(SM_YVIRTUALSCREEN),width:GetSystemMetrics(SM_CXVIRTUALSCREEN) as u32,height:GetSystemMetrics(SM_CYVIRTUALSCREEN) as u32};
  let surface=Surface::new(rect.width,rect.height)?;
  BitBlt(surface.dc,0,0,rect.width as i32,rect.height as i32,Some(surface.screen),rect.x,rect.y,SRCCOPY|CAPTUREBLT).map_err(|e|format!("屏幕捕获失败：{e}"))?;
  let mut monitors=Vec::new();let _=EnumDisplayMonitors(None,None,Some(monitor),LPARAM(&mut monitors as *mut Vec<Rect> as isize));
  let mut windows=Vec::new();let _=EnumWindows(Some(window),LPARAM(&mut windows as *mut Vec<WindowInfo> as isize));
  Ok(Frame{rect,png:encode_png(rect.width,rect.height,&surface.rgba())?,monitors,windows})
 }
}
pub fn capture_window(frame:&Frame,id:usize)->Result<(Frame,Rect),String>{
 let _dpi=Dpi::enter();
 let target=frame.windows.iter().find(|w|w.id==id).ok_or("窗口不在本次截图列表中")?;
 unsafe{
  let hwnd=HWND(id as _);if !IsWindow(Some(hwnd)).as_bool()||IsIconic(hwnd).as_bool()||IsHungAppWindow(hwnd).as_bool(){return Err("此窗口已关闭、最小化或未响应，请重新截图".into())}
  let mut r=RECT::default();GetWindowRect(hwnd,&mut r).map_err(|e|e.to_string())?;
  let width=(r.right-r.left).max(0) as u32;let height=(r.bottom-r.top).max(0) as u32;
  let surface=Surface::new(width,height)?;
  if !PrintWindow(hwnd,surface.dc,PRINT_WINDOW_FLAGS(2)).as_bool(){return Err("此窗口不支持窗口捕获；请使用区域截图截取其可见部分".into())}
  let pixels=surface.rgba();
  // A successful PrintWindow may still return black for protected/GPU-only windows.
  if pixels.chunks_exact(4).all(|p|p[..3]==[0,0,0]){return Err("窗口返回黑色图像（可能受保护）；请使用区域截图，受保护内容无法截取".into())}
  let mut visible=RECT::default();
  if DwmGetWindowAttribute(hwnd,DWMWA_EXTENDED_FRAME_BOUNDS,(&mut visible as *mut RECT).cast(),std::mem::size_of::<RECT>() as u32).is_err(){visible=r;}
  let rect=Rect{x:visible.left,y:visible.top,width:(visible.right-visible.left).max(0) as u32,height:(visible.bottom-visible.top).max(0) as u32}.intersection(frame.rect).ok_or("窗口已移出桌面，请重新截图")?;
  let (_,_,mut desktop)=decode_png(&frame.png)?;
  for y in 0..rect.height {
   for x in 0..rect.width {
    let sx=rect.x-r.left+x as i32;let sy=rect.y-r.top+y as i32;
    if sx<0||sy<0||sx>=width as i32||sy>=height as i32{continue}
    let src=((sy as u32*width+sx as u32)*4) as usize;
    let dst=(((rect.y-frame.rect.y+y as i32) as u32*frame.rect.width+(rect.x-frame.rect.x+x as i32) as u32)*4) as usize;
    desktop[dst..dst+4].copy_from_slice(&pixels[src..src+4]);
   }
  }
  let mut next=frame.clone();next.png=encode_png(frame.rect.width,frame.rect.height,&desktop)?;
  let _=target;Ok((next,rect))
 }
}
pub fn position(window:&tauri::WebviewWindow,rect:Rect)->Result<(),String>{
 let hwnd=window.hwnd().map_err(|e|e.to_string())?;
 unsafe{SetWindowPos(hwnd,Some(HWND_TOPMOST),rect.x,rect.y,rect.width as i32,rect.height as i32,SWP_NOACTIVATE)}.map_err(|e|e.to_string())
}
pub fn resize(window:&tauri::WebviewWindow)->Result<(),String>{
 let hwnd=window.hwnd().map_err(|e|e.to_string())?;
 unsafe{windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture().map_err(|e|e.to_string())?;PostMessageW(Some(hwnd),WM_NCLBUTTONDOWN,WPARAM(HTBOTTOMRIGHT as usize),LPARAM(0))}.map_err(|e|e.to_string())
}

struct Clipboard;
impl Drop for Clipboard{fn drop(&mut self){unsafe{let _=CloseClipboard();}}}
unsafe fn clipboard_item(format:u32,bytes:&[u8])->Result<(),String>{
 let memory=GlobalAlloc(GMEM_MOVEABLE,bytes.len()).map_err(|e|e.to_string())?;
 let pointer=GlobalLock(memory);if pointer.is_null(){let _=GlobalFree(Some(memory));return Err("剪贴板内存分配失败".into())}
 std::ptr::copy_nonoverlapping(bytes.as_ptr(),pointer.cast(),bytes.len());let _=GlobalUnlock(memory);
 if let Err(e)=SetClipboardData(format,Some(HANDLE(memory.0))){let _=GlobalFree(Some(memory));return Err(e.to_string())}Ok(())
}
pub fn copy(png:&[u8],owner:usize)->Result<(),String>{
 let (width,height,rgba)=decode_png(png)?;
 let dib=super::dibv5(width,height,&rgba)?;
 unsafe{
  let mut opened=false;
  for _ in 0..10 {if OpenClipboard(Some(HWND(owner as _))).is_ok(){opened=true;break}std::thread::sleep(Duration::from_millis(30));}
  if !opened{return Err("剪贴板被其他程序占用，请重试".into())}let _guard=Clipboard;
  EmptyClipboard().map_err(|e|e.to_string())?;
  clipboard_item(17,&dib)?; // CF_DIBV5: actual image, not a filename or text.
  clipboard_item(RegisterClipboardFormatW(w!("PNG")),png)?;
 }Ok(())
}
struct Com;
impl Drop for Com{fn drop(&mut self){unsafe{CoUninitialize();}}}
fn default_save_folder(dialog:&IFileSaveDialog,kind:&str)->Result<(),String>{
 let folder=crate::runtime_paths::export_directory(kind)?;
 let wide=folder.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
 unsafe{let item:IShellItem=SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()),None).map_err(|e|format!("默认导出目录不可用：{e}"))?;dialog.SetFolder(&item).map_err(|e|e.to_string())}
}
pub fn copy_text(text:&str,owner:usize)->Result<(),String>{
 let bytes=text.encode_utf16().chain(Some(0)).flat_map(u16::to_le_bytes).collect::<Vec<_>>();
 unsafe{OpenClipboard(Some(HWND(owner as _))).map_err(|e|e.to_string())?;let _guard=Clipboard;EmptyClipboard().map_err(|e|e.to_string())?;clipboard_item(13,&bytes)}
}
pub fn read_image(owner:usize)->Result<Vec<u8>,String>{
 unsafe{
  OpenClipboard(Some(HWND(owner as _))).map_err(|e|e.to_string())?;let _guard=Clipboard;
  let png_format=RegisterClipboardFormatW(w!("PNG"));
  for format in [png_format,17,8]{
   if IsClipboardFormatAvailable(format).is_err(){continue}
   let memory=GetClipboardData(format).map_err(|e|e.to_string())?;let global=windows::Win32::Foundation::HGLOBAL(memory.0);let size=GlobalSize(global);if size==0||size>super::MAX_PNG*6{return Err("剪贴板图片过大或无效".into())}
   let pointer=GlobalLock(global);if pointer.is_null(){return Err("无法读取剪贴板图片".into())}
   let bytes=std::slice::from_raw_parts(pointer.cast::<u8>(),size).to_vec();let _=GlobalUnlock(global);
   if format==png_format{super::decode_png(&bytes)?;return Ok(bytes)}
   return super::decode_dib(&bytes);
  }Err("剪贴板中没有 PNG 或 DIB 图片，请先复制图片".into())
 }
}
pub fn save_text(text:&str,owner:usize,extension:&str,name:&str)->Result<Option<String>,String>{
 unsafe{
  CoInitializeEx(None,COINIT_APARTMENTTHREADED).ok().map_err(|e|e.to_string())?;let _guard=Com;
  let dialog:IFileSaveDialog=CoCreateInstance(&FileSaveDialog,None,CLSCTX_INPROC_SERVER).map_err(|e|e.to_string())?;
  dialog.SetOptions(FOS_FORCEFILESYSTEM|FOS_PATHMUSTEXIST|FOS_OVERWRITEPROMPT|FOS_DONTADDTORECENT).map_err(|e|e.to_string())?;
  dialog.SetTitle(w!("保存曲线数据提取文件")).map_err(|e|e.to_string())?;
  default_save_folder(&dialog,"PlotDigitizer")?;
  let ext:Vec<u16>=extension.encode_utf16().chain(Some(0)).collect();dialog.SetDefaultExtension(PCWSTR(ext.as_ptr())).map_err(|e|e.to_string())?;
  let name:Vec<u16>=name.encode_utf16().chain(Some(0)).collect();dialog.SetFileName(PCWSTR(name.as_ptr())).map_err(|e|e.to_string())?;
  if let Err(e)=dialog.Show(Some(HWND(owner as _))){if e.code().0 as u32==0x800704c7{return Ok(None)}return Err(e.to_string())}
  let path=dialog.GetResult().and_then(|item|item.GetDisplayName(SIGDN_FILESYSPATH)).map_err(|e|e.to_string())?;
  let selected=path.to_string().map_err(|e|e.to_string());CoTaskMemFree(Some(path.0.cast()));let selected=selected?;let path=Path::new(&selected);
  if !path.extension().is_some_and(|e|e.eq_ignore_ascii_case(extension)){return Err(format!("请使用 .{extension} 扩展名"))}
  super::write_saved_file(path,text.as_bytes())?;Ok(Some(selected))
 }
}
pub fn save(png:&[u8],owner:usize)->Result<Option<String>,String>{
 decode_png(png)?;
 unsafe{
  CoInitializeEx(None,COINIT_APARTMENTTHREADED).ok().map_err(|e|e.to_string())?;let _guard=Com;
  let dialog:IFileSaveDialog=CoCreateInstance(&FileSaveDialog,None,CLSCTX_INPROC_SERVER).map_err(|e|e.to_string())?;
  dialog.SetOptions(FOS_FORCEFILESYSTEM|FOS_PATHMUSTEXIST|FOS_OVERWRITEPROMPT|FOS_DONTADDTORECENT).map_err(|e|e.to_string())?;
  dialog.SetTitle(w!("保存截图")).map_err(|e|e.to_string())?;
  default_save_folder(&dialog,"Screenshots")?;
  dialog.SetDefaultExtension(w!("png")).map_err(|e|e.to_string())?;
  let name:Vec<u16>=format!("SciDesk-{}.png",chrono::Local::now().format("%Y%m%d-%H%M%S")).encode_utf16().chain(Some(0)).collect();
  dialog.SetFileName(PCWSTR(name.as_ptr())).map_err(|e|e.to_string())?;
  if let Err(e)=dialog.Show(Some(HWND(owner as _))){if e.code().0 as u32==0x800704c7{return Ok(None)}return Err(e.to_string())}
  let path=dialog.GetResult().and_then(|item|item.GetDisplayName(SIGDN_FILESYSPATH)).map_err(|e|e.to_string())?;
  let text=path.to_string().map_err(|e|e.to_string());CoTaskMemFree(Some(path.0.cast()));let text=text?;
  super::write_saved_png(Path::new(&text),png)?;Ok(Some(text))
 }
}
