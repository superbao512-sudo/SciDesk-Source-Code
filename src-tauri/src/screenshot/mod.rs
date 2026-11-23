//! Session-only screenshot tool. Pixels never enter SQLite, logs or network services.
#[cfg(windows)] mod win;
mod hotkey;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path,PathBuf}, sync::{Arc,Mutex}};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WebviewUrl, WindowEvent};
const MAX_PIXELS:u64=80_000_000;
const MAX_PNG:usize=64*1024*1024;
#[derive(Clone,Copy,Debug,Serialize,Deserialize,PartialEq)]
pub struct Rect{pub x:i32,pub y:i32,pub width:u32,pub height:u32}
impl Rect{
 pub fn intersection(self,other:Self)->Option<Self>{
  let x=self.x.max(other.x);let y=self.y.max(other.y);
  let right=(self.x as i64+self.width as i64).min(other.x as i64+other.width as i64);
  let bottom=(self.y as i64+self.height as i64).min(other.y as i64+other.height as i64);
  (right>x as i64&&bottom>y as i64).then(||Self{x,y,width:(right-x as i64) as u32,height:(bottom-y as i64) as u32})
 }
}
#[derive(Clone,Serialize)]pub struct WindowInfo{pub id:usize,pub title:String,pub rect:Rect}
#[derive(Clone)]pub struct Frame{pub rect:Rect,pub png:Vec<u8>,pub monitors:Vec<Rect>,pub windows:Vec<WindowInfo>}
#[derive(Clone,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]pub struct Settings{pub enabled:bool,pub shortcut:String,pub delay:u64}
impl Default for Settings{fn default()->Self{Self{enabled:true,shortcut:"Ctrl+Shift+F6".into(),delay:0}}}
struct Session{label:String,frame:Option<Arc<Frame>>,restore_main:bool,ready:bool}
pub struct State{
 active:Mutex<Option<Session>>,pins:Mutex<HashMap<String,Vec<u8>>>,pending_image:Mutex<Option<Vec<u8>>>,
 settings:Mutex<Settings>,registration:Mutex<Option<String>>,settings_path:PathBuf,
 hotkey:Mutex<Option<std::sync::mpsc::Sender<Settings>>>,
}
impl State{
 pub fn new(root:&Path)->Self{
  let settings_path=root.join("screenshot-settings.json");
  let stored=std::fs::read(&settings_path).ok().and_then(|bytes|serde_json::from_slice::<Settings>(&bytes).ok());
  Self{active:Mutex::new(None),pins:Mutex::new(HashMap::new()),pending_image:Mutex::new(None),settings:Mutex::new(stored.unwrap_or_default()),registration:Mutex::new(None),settings_path,hotkey:Mutex::new(None)}
 }
}
pub fn initialize(app:&AppHandle){hotkey::initialize(app);}
pub fn checked_pixels(width:u32,height:u32)->Result<usize,String>{
 if width==0||height==0||width as u64*height as u64>MAX_PIXELS{return Err("截图尺寸无效或超过 8000 万像素限制".into())}Ok(width as usize*height as usize*4)
}
pub fn encode_png(width:u32,height:u32,pixels:&[u8])->Result<Vec<u8>,String>{
 if checked_pixels(width,height)?!=pixels.len(){return Err("像素长度无效".into())}
 let mut result=Vec::new();{
  let mut encoder=png::Encoder::new(&mut result,width,height);encoder.set_color(png::ColorType::Rgba);encoder.set_depth(png::BitDepth::Eight);encoder.set_compression(png::Compression::Fast);
  encoder.write_header().and_then(|mut writer|writer.write_image_data(pixels)).map_err(|e|e.to_string())?;
 }if result.len()>MAX_PNG{return Err("图片超过 64 MB 限制".into())}Ok(result)
}
pub fn decode_png(bytes:&[u8])->Result<(u32,u32,Vec<u8>),String>{
 if bytes.len()>MAX_PNG{return Err("图片超过 64 MB 限制".into())}
 let mut decoder=png::Decoder::new(std::io::Cursor::new(bytes));decoder.set_transformations(png::Transformations::EXPAND|png::Transformations::STRIP_16);
 decoder.set_limits(png::Limits{bytes:MAX_PIXELS as usize*8});
 let mut reader=decoder.read_info().map_err(|e|format!("PNG 无效：{e}"))?;
 let (width,height)=(reader.info().width,reader.info().height);checked_pixels(width,height)?;
 let size=reader.output_buffer_size().ok_or("PNG 缓冲区过大")?;if size>MAX_PIXELS as usize*4{return Err("PNG 缓冲区过大".into())}
 let mut buffer=vec![0;size];let info=reader.next_frame(&mut buffer).map_err(|e|e.to_string())?;
 let mut rgba=Vec::with_capacity(width as usize*height as usize*4);
 match info.color_type{
  png::ColorType::Rgba=>rgba.extend_from_slice(&buffer[..info.buffer_size()]),
  png::ColorType::Rgb=>for p in buffer[..info.buffer_size()].chunks_exact(3){rgba.extend_from_slice(&[p[0],p[1],p[2],255]);},
  png::ColorType::Grayscale=>for &v in &buffer[..info.buffer_size()]{rgba.extend_from_slice(&[v,v,v,255]);},
  png::ColorType::GrayscaleAlpha=>for p in buffer[..info.buffer_size()].chunks_exact(2){rgba.extend_from_slice(&[p[0],p[0],p[0],p[1]]);},
  _=>return Err("不支持的 PNG 格式".into()),
 }Ok((width,height,rgba))
}
fn dibv5(width:u32,height:u32,pixels:&[u8])->Result<Vec<u8>,String>{
 let size=checked_pixels(width,height)?;if size!=pixels.len(){return Err("像素长度无效".into())}
 let mut dib=vec![0u8;124+size];
 for (at,value) in [(0,124u32),(4,width),(8,(-(height as i32)) as u32),(16,3),(20,size as u32),(40,0x00ff0000),(44,0x0000ff00),(48,0x000000ff),(52,0xff000000),(56,0x73524742)]{dib[at..at+4].copy_from_slice(&value.to_le_bytes());}
 dib[12..14].copy_from_slice(&1u16.to_le_bytes());dib[14..16].copy_from_slice(&32u16.to_le_bytes());
 for (src,dst) in pixels.chunks_exact(4).zip(dib[124..].chunks_exact_mut(4)){dst.copy_from_slice(&[src[2],src[1],src[0],src[3]]);}Ok(dib)
}
fn decode_dib(bytes:&[u8])->Result<Vec<u8>,String>{
 if bytes.len()<40{return Err("DIB 图片头无效".into())}
 let u32_at=|at:usize|u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
 let header=u32_at(0) as usize;let width=u32_at(4) as i32;let signed_height=u32_at(8) as i32;let height=signed_height.unsigned_abs();
 let depth=u16::from_le_bytes(bytes[14..16].try_into().unwrap());let compression=u32_at(16);
 if width<=0||header<40||header>bytes.len()||!matches!(depth,24|32)||!matches!(compression,0|3){return Err("仅支持未压缩的 24/32 位剪贴板图片".into())}
 checked_pixels(width as u32,height)?;
 let offset=header+if header==40&&compression==3{12}else{0};
 if compression==3&&(bytes.len()<52||[u32_at(40),u32_at(44),u32_at(48)]!=[0xff0000,0xff00,0xff]){return Err("剪贴板图片使用不支持的像素掩码".into())}
 let stride=(width as usize*depth as usize).div_ceil(32)*4;if offset+stride*height as usize>bytes.len(){return Err("剪贴板图片数据不完整".into())}
 let mut rgba=vec![0;checked_pixels(width as u32,height)?];
 for y in 0..height as usize{let source_y=if signed_height>0{height as usize-1-y}else{y};for x in 0..width as usize{let p=offset+source_y*stride+x*(depth as usize/8);let d=(y*width as usize+x)*4;rgba[d..d+4].copy_from_slice(&[bytes[p+2],bytes[p+1],bytes[p],255]);}}
 encode_png(width as u32,height,&rgba)
}
fn write_saved_png(path:&Path,png:&[u8])->Result<(),String>{
 if !path.extension().is_some_and(|e|e.eq_ignore_ascii_case("png")){return Err("请使用 .png 扩展名".into())}
 write_saved_file(path,png)
}
fn write_saved_file(path:&Path,bytes:&[u8])->Result<(),String>{
 let parent=std::fs::canonicalize(path.parent().ok_or("保存目录无效")?).map_err(|e|e.to_string())?;
 let lower=parent.to_string_lossy().trim_start_matches("\\\\?\\").replace('/',"\\").to_lowercase();
 for source in ["e:\\scidesk","d:\\work app\\information assistant\\信息化助手"]{if lower==source||lower.starts_with(&format!("{source}\\")){return Err("不能写入只读参考项目".into())}}
 let destination=parent.join(path.file_name().ok_or("文件名无效")?);
 if std::fs::symlink_metadata(&destination).is_ok_and(|m|{#[cfg(windows)]{use std::os::windows::fs::MetadataExt;m.file_attributes()&0x400!=0}#[cfg(not(windows))]{m.file_type().is_symlink()}}){return Err("不能覆盖链接文件".into())}
 let temporary=parent.join(format!(".scidesk-shot-{}.tmp",uuid::Uuid::new_v4()));
 let mut owned=false;
 let result=(||{use std::io::Write;let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(|e|e.to_string())?;owned=true;file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);crate::project_platform::replace_record(&temporary,&destination)})();
 if result.is_err()&&owned{let _=std::fs::remove_file(&temporary);}result
}
fn frame_value(frame:&Frame)->Value{json!({"rect":frame.rect,"image":format!("data:image/png;base64,{}",STANDARD.encode(&frame.png)),"monitors":frame.monitors,"windows":frame.windows})}
// Pin bytes have already passed decode_png; read dimensions without decoding on every wheel event.
fn pin_size(png:&[u8])->(u32,u32){(u32::from_be_bytes(png[16..20].try_into().unwrap()),u32::from_be_bytes(png[20..24].try_into().unwrap()))}
fn current(app:&AppHandle,label:&str)->Result<Arc<Frame>,String>{
 app.state::<State>().active.lock().unwrap().as_ref().filter(|s|s.label==label).and_then(|s|s.frame.clone()).ok_or_else(||"截图会话已结束".into())
}
pub fn finish(app:&AppHandle){
 let session=app.state::<State>().active.lock().unwrap().take();
 if let Some(session)=session{if let Some(window)=app.get_webview_window(&session.label){let _=window.close();}if session.restore_main{if let Some(main)=app.get_webview_window("main"){let _=main.show();let _=main.set_focus();}}}
}
fn report(app:&AppHandle,error:&str){if let Some(main)=app.get_webview_window("main"){let _=main.emit("screenshot:error",error);}}
pub async fn begin(app:AppHandle,delay:u64)->Result<(),String>{
 if !matches!(delay,0|3|5){return Err("延时仅支持 0、3、5 秒".into())}
 let main=app.get_webview_window("main").ok_or("主窗口尚未就绪")?;
 let restore_main=main.is_visible().unwrap_or(false)&&!main.is_minimized().unwrap_or(false);
 let label=format!("capture-{}",uuid::Uuid::new_v4());
 {let state=app.state::<State>();let mut active=state.active.lock().unwrap();if active.is_some(){return Err("截图正在进行".into())}*active=Some(Session{label:label.clone(),frame:None,restore_main,ready:false});}
 let result=async{
  if restore_main{main.hide().map_err(|e|e.to_string())?;}
  tokio::time::sleep(std::time::Duration::from_millis(delay*1000+220)).await;
  #[cfg(windows)]let frame=tauri::async_runtime::spawn_blocking(win::capture).await.map_err(|e|e.to_string())??;
  #[cfg(not(windows))]return Err::<(),String>("高级截图目前需要 Windows 桌面环境".into());
  #[cfg(windows)]{
   {let state=app.state::<State>();let mut active=state.active.lock().unwrap();let session=active.as_mut().filter(|s|s.label==label).ok_or("截图已取消")?;session.frame=Some(Arc::new(frame));}
   let handle=app.clone();let capture_label=label.clone();let profile=crate::runtime_paths::data_root()?.join("cache/webview");
   let (send,receive)=tokio::sync::oneshot::channel();
   app.run_on_main_thread(move||{
    let result=tauri::WebviewWindowBuilder::new(&handle,&capture_label,WebviewUrl::App("index.html".into())).data_directory(profile)
     .title("SciDesk 高级截图").decorations(false).shadow(false).always_on_top(true).skip_taskbar(true).visible(false).resizable(false).background_color(tauri::window::Color(18,21,28,255)).build().map(|_|()).map_err(|e|e.to_string());
    let _=send.send(result);
   }).map_err(|e|e.to_string())?;
   receive.await.map_err(|e|e.to_string())??;
   let watchdog=app.clone();let pending=label.clone();
   tauri::async_runtime::spawn(async move{tokio::time::sleep(std::time::Duration::from_secs(30)).await;let stalled=watchdog.state::<State>().active.lock().unwrap().as_ref().is_some_and(|s|s.label==pending&&!s.ready);if stalled{finish(&watchdog);report(&watchdog,"截图界面加载超时，已恢复主窗口");}});
   Ok(())
  }
 }.await;
 if result.is_err(){finish(&app);}result
}
pub fn window_event(window:&tauri::Window,event:&WindowEvent){
 if !matches!(event,WindowEvent::Destroyed){return}
 let app=window.app_handle();let Some(state)=app.try_state::<State>() else{return};
 if window.label().starts_with("pin-"){state.pins.lock().unwrap().remove(window.label());}
 if window.label().starts_with("capture-"){
  let session={let mut active=state.active.lock().unwrap();if active.as_ref().is_some_and(|s|s.label==window.label()){active.take()}else{None}};
  if session.is_some_and(|s|s.restore_main){if let Some(main)=app.get_webview_window("main"){let _=main.show();let _=main.set_focus();}}
 }
}
fn submitted(args:&Value,frame:&Frame)->Result<Vec<u8>,String>{
 let raw=args["image"].as_str().ok_or("缺少图片")?;if raw.len()>MAX_PNG*4/3+128{return Err("图片过大".into())}
 let png=STANDARD.decode(raw.strip_prefix("data:image/png;base64,").ok_or("只接受 PNG 图片")?).map_err(|e|e.to_string())?;
 let (width,height,_)=decode_png(&png)?;if width>frame.rect.width||height>frame.rect.height{return Err("导出尺寸超出本次截图".into())}Ok(png)
}
#[tauri::command]
pub async fn screenshot_command(app:AppHandle,window:WebviewWindow,op:String,args:Value)->Result<Value,String>{
 let label=window.label().to_owned();
 if label=="main"{
  return match op.as_str(){
   "start"=>{let delay=args["delay"].as_u64().unwrap_or(0);begin(app,delay).await?;Ok(Value::Null)},
   "digitizer_image"=>Ok(app.state::<State>().pending_image.lock().unwrap().take().map(|bytes|json!(format!("data:image/png;base64,{}",STANDARD.encode(bytes)))).unwrap_or(Value::Null)),
   "digitizer_copy"=>{let text=args["text"].as_str().filter(|s|s.len()<=16*1024*1024).ok_or("文本过大或无效")?.to_owned();#[cfg(windows)]{let owner=window.hwnd().map_err(|e|e.to_string())?.0 as usize;tauri::async_runtime::spawn_blocking(move||win::copy_text(&text,owner)).await.map_err(|e|e.to_string())??;}Ok(Value::Null)},
   "digitizer_clipboard"=>{#[cfg(windows)]{let owner=window.hwnd().map_err(|e|e.to_string())?.0 as usize;let png=tauri::async_runtime::spawn_blocking(move||win::read_image(owner)).await.map_err(|e|e.to_string())??;Ok(json!(format!("data:image/png;base64,{}",STANDARD.encode(png))))}#[cfg(not(windows))]Err("读取图片需要 Windows 剪贴板".into())},
   "digitizer_save"=>{
    let extension=args["extension"].as_str().filter(|s|matches!(*s,"csv"|"tsv"|"txt"|"json")).ok_or("导出格式无效")?.to_owned();
    let text=args["contents"].as_str().filter(|s|s.len()<=64*1024*1024).ok_or("导出内容过大")?.to_owned();
    let name=args["name"].as_str().unwrap_or("data").replace(['\\','/',':','*','?','"','<','>','|'],"_");let name=name.chars().take(150).collect::<String>();
    #[cfg(windows)]{let owner=window.hwnd().map_err(|e|e.to_string())?.0 as usize;let saved=tauri::async_runtime::spawn_blocking(move||win::save_text(&text,owner,&extension,&name)).await.map_err(|e|e.to_string())??;Ok(json!({"path":saved}))}
    #[cfg(not(windows))]Err("保存需要 Windows".into())
   },
   "settings"=>{let state=app.state::<State>();let settings=state.settings.lock().unwrap().clone();let error=state.registration.lock().unwrap().clone();Ok(json!({"settings":settings,"error":error}))},
   "configure"=>{let settings:Settings=serde_json::from_value(args).map_err(|e|e.to_string())?;hotkey::validate(&settings)?;let state=app.state::<State>();let sender=state.hotkey.lock().unwrap().clone().ok_or("快捷键服务不可用")?;
    let parent=state.settings_path.parent().unwrap();std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;let temporary=parent.join(format!(".screenshot-settings-{}.tmp",uuid::Uuid::new_v4()));std::fs::write(&temporary,serde_json::to_vec_pretty(&settings).unwrap()).map_err(|e|e.to_string())?;
    if let Err(e)=crate::project_platform::replace_record(&temporary,&state.settings_path){let _=std::fs::remove_file(temporary);return Err(e)}
    *state.settings.lock().unwrap()=settings.clone();sender.send(settings).map_err(|e|e.to_string())?;Ok(Value::Null)},
   _=>Err("主窗口不支持此截图操作".into()),
  }
 }
 if label.starts_with("pin-"){
  let (png,width,height)={let state=app.state::<State>();let pins=state.pins.lock().unwrap();let bytes=pins.get(&label).ok_or("贴图已关闭")?;let (width,height)=pin_size(bytes);(if op=="pin_frame"{Some(bytes.clone())}else{None},width,height)};
  return match op.as_str(){
   "pin_frame"=>Ok(json!({"image":format!("data:image/png;base64,{}",STANDARD.encode(png.unwrap())),"width":width,"height":height})),
   "pin_ready"=>{let monitor=window.current_monitor().map_err(|e|e.to_string())?;let scale=monitor.map(|m|{let size=m.work_area().size;(size.width as f64*0.65/width as f64).min(size.height as f64*0.65/height as f64).min(1.)}).unwrap_or(1.);window.set_size(tauri::PhysicalSize::new((width as f64*scale).max(48.) as u32,(height as f64*scale).max(32.) as u32)).map_err(|e|e.to_string())?;window.show().map_err(|e|e.to_string())?;Ok(json!({"scale":scale}))},
   "pin_drag"=>{window.start_dragging().map_err(|e|e.to_string())?;Ok(Value::Null)},
   "pin_resize"=>{#[cfg(windows)]win::resize(&window)?;Ok(Value::Null)},
   "pin_zoom"=>{let scale=args["scale"].as_f64().filter(|s|s.is_finite()&&*s>=0.1&&*s<=4.0).ok_or("缩放范围为 10%–400%")?;window.set_size(tauri::PhysicalSize::new((width as f64*scale).max(48.) as u32,(height as f64*scale).max(32.) as u32)).map_err(|e|e.to_string())?;Ok(Value::Null)},
   "pin_close"=>{window.close().map_err(|e|e.to_string())?;Ok(Value::Null)},
   _=>Err("不支持的贴图操作".into()),
  }
 }
 let frame=current(&app,&label)?;
 match op.as_str(){
  "frame"=>Ok(frame_value(&frame)),
  "ready"=>{
   #[cfg(windows)]win::position(&window,frame.rect)?;
   window.show().map_err(|e|e.to_string())?;window.set_focus().map_err(|e|e.to_string())?;
   if let Some(session)=app.state::<State>().active.lock().unwrap().as_mut(){session.ready=true;}Ok(Value::Null)
  },
  "cancel"=>{finish(&app);Ok(Value::Null)},
  "window"=>{
   let id=args["id"].as_u64().ok_or("窗口编号无效")? as usize;
   #[cfg(windows)]{
    let (next,rect)=tokio::time::timeout(std::time::Duration::from_secs(4),tauri::async_runtime::spawn_blocking(move||win::capture_window(&frame,id))).await.map_err(|_|"窗口捕获超时，请改用区域截图")?.map_err(|e|e.to_string())??;
    {let state=app.state::<State>();let mut active=state.active.lock().unwrap();let session=active.as_mut().filter(|s|s.label==label).ok_or("截图已结束")?;session.frame=Some(Arc::new(next.clone()));}
    Ok(json!({"frame":frame_value(&next),"selection":rect}))
   }
   #[cfg(not(windows))]{let _=id;Err("需要 Windows".into())}
  },
  "copy"|"save"|"pin"|"digitize"=>{
   let png=submitted(&args,&frame)?;
   if op=="digitize"{*app.state::<State>().pending_image.lock().unwrap()=Some(png);finish(&app);if let Some(main)=app.get_webview_window("main"){let _=main.emit("screenshot:digitize",());}return Ok(Value::Null)}
   if op=="pin"{
    let (width,height,_)=decode_png(&png)?;let name=format!("pin-{}",uuid::Uuid::new_v4());
    {let state=app.state::<State>();let mut pins=state.pins.lock().unwrap();let pixels=pins.values().map(|p|{let (w,h)=pin_size(p);w as u64*h as u64}).sum::<u64>();if pins.len()>=12||pixels+width as u64*height as u64>MAX_PIXELS||pins.values().map(Vec::len).sum::<usize>()+png.len()>MAX_PNG*2{return Err("贴图已达到本会话内存上限，请先关闭部分贴图".into())}pins.insert(name.clone(),png);}
    let handle=app.clone();let pin_label=name.clone();let profile=crate::runtime_paths::data_root()?.join("cache/webview");let (send,receive)=tokio::sync::oneshot::channel();
    app.run_on_main_thread(move||{let result=tauri::WebviewWindowBuilder::new(&handle,&pin_label,WebviewUrl::App("index.html".into())).data_directory(profile).title("SciDesk 贴图").decorations(false).shadow(false).always_on_top(true).skip_taskbar(true).visible(false).resizable(true).min_inner_size(48.,32.).inner_size(width.min(900) as f64,height.min(700) as f64).center().build().map(|_|()).map_err(|e|e.to_string());let _=send.send(result);}).map_err(|e|e.to_string())?;
    if let Err(e)=receive.await.map_err(|e|e.to_string())?{app.state::<State>().pins.lock().unwrap().remove(&name);return Err(e)}finish(&app);return Ok(json!({"pinned":true}))
   }
   #[cfg(windows)]{
    let owner=window.hwnd().map_err(|e|e.to_string())?.0 as usize;
    if op=="copy"{tauri::async_runtime::spawn_blocking(move||win::copy(&png,owner)).await.map_err(|e|e.to_string())??;finish(&app);if let Some(main)=app.get_webview_window("main"){let _=main.emit("screenshot:result","图片已复制到 Windows 剪贴板");}Ok(json!({"copied":true}))}
    else {let saved=tauri::async_runtime::spawn_blocking(move||win::save(&png,owner)).await.map_err(|e|e.to_string())??;if let Some(ref path)=saved{finish(&app);if let Some(main)=app.get_webview_window("main"){let _=main.emit("screenshot:result",format!("截图已保存：{path}"));}}Ok(json!({"path":saved}))}
   }
   #[cfg(not(windows))]Err("需要 Windows".into())
  },
  _=>Err("不支持的截图操作".into()),
 }
}
#[cfg(test)]mod tests;
