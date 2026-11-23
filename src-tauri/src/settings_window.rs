use std::{sync::{Mutex,atomic::{AtomicBool,Ordering}}};
use tauri::{Manager,AppHandle,LogicalSize,PhysicalPosition,WindowEvent};
use tauri_plugin_sql::DbInstances;
use crate::{boot_diagnostics,datasets,settings_core::{self,WindowState,Screen}};
use crate::window_geometry::{fit_client_size, MIN_WIDTH, MIN_HEIGHT};

pub fn fit_main_window(window: &tauri::WebviewWindow, width: f64, height: f64, center: bool) -> tauri::Result<()> {
 let monitor = if center { window.primary_monitor()? } else { window.current_monitor()? }.or(window.primary_monitor()?);
 if let Some(monitor) = monitor {
  let area = monitor.work_area();
  let scale = monitor.scale_factor();
  let inner = window.inner_size()?;
  let outer = window.outer_size()?;
  let frame_width = outer.width.saturating_sub(inner.width);
  let frame_height = outer.height.saturating_sub(inner.height);
  let (width, height) = fit_client_size(width, height, area.size.width, area.size.height, scale, frame_width, frame_height);
  window.set_min_size(Some(LogicalSize::new(MIN_WIDTH.min(width), MIN_HEIGHT.min(height))))?;
  window.set_size(LogicalSize::new(width, height))?;
  let outer = window.outer_size()?;
  let position = window.outer_position()?;
  let right = area.position.x + area.size.width.saturating_sub(outer.width) as i32;
  let bottom = area.position.y + area.size.height.saturating_sub(outer.height) as i32;
  let x = if center { area.position.x + (right - area.position.x) / 2 } else { position.x.clamp(area.position.x, right) };
  let y = if center { area.position.y + (bottom - area.position.y) / 2 } else { position.y.clamp(area.position.y, bottom) };
  window.set_position(PhysicalPosition::new(x, y))?;
 }
 Ok(())
}
#[derive(Default)]pub struct WindowMemory{normal:Mutex<Option<WindowState>>,closing:AtomicBool}
#[tauri::command]
pub async fn configure_main_window(app:AppHandle,state:tauri::State<'_,DbInstances>)->Result<(),String>{
 let pool=datasets::pool(&state).await?;
 let rows:Vec<(String,String)>=sqlx::query_as("SELECT key,value FROM settings WHERE key LIKE 'startup.%'").fetch_all(&pool).await.map_err(|e|e.to_string())?;
 let config=settings_core::parse_preferences(rows);let window=app.get_webview_window("main").ok_or("找不到主窗口")?;
 let screens:Vec<Screen>=window.available_monitors().map_err(|e|e.to_string())?.iter().map(|m|{let area=m.work_area();Screen{x:area.position.x,y:area.position.y,width:area.size.width,height:area.size.height,scale:m.scale_factor()}}).collect();
 let saved:Option<String>=sqlx::query_scalar("SELECT value FROM settings WHERE key='startup.window_state'").fetch_optional(&pool).await.map_err(|e|e.to_string())?;
 let restore=saved.as_ref().and_then(|s|serde_json::from_str::<WindowState>(s).ok()).filter(|s|config["startup.remember_window_state"]==true&&settings_core::visible_state(s,&screens));
 let should_maximize=config["startup.start_maximized"]==true||restore.as_ref().is_some_and(|s|s.maximized);
 if let Some(ref saved)=restore{window.set_position(PhysicalPosition::new(saved.x,saved.y)).map_err(|e|e.to_string())?;fit_main_window(&window,saved.width,saved.height,false).map_err(|e|e.to_string())?;}
 else{fit_main_window(&window,config["startup.default_width"].as_f64().unwrap(),config["startup.default_height"].as_f64().unwrap(),config["startup.center_window"]==true||saved.is_some()).map_err(|e|e.to_string())?;}
 if let (Ok(size),Ok(position),Ok(scale))=(window.inner_size(),window.outer_position(),window.scale_factor()){let size=size.to_logical::<f64>(scale);*app.state::<WindowMemory>().normal.lock().unwrap()=Some(WindowState{width:size.width,height:size.height,x:position.x,y:position.y,maximized:false});}
 if should_maximize{window.maximize().map_err(|e|e.to_string())?;}Ok(())
}
#[tauri::command]pub fn show_main_window(app:AppHandle)->Result<(),String>{
 boot_diagnostics::record("BOOT_14_SHOW_MAIN_INVOKED", "show_main_window entered");
 let window=app.get_webview_window("main").ok_or("找不到主窗口")?;
 if !window.is_visible().map_err(|e|e.to_string())? && !window.is_maximized().map_err(|e|e.to_string())? {
  let size=window.inner_size().map_err(|e|e.to_string())?.to_logical::<f64>(window.scale_factor().map_err(|e|e.to_string())?);
  fit_main_window(&window,size.width,size.height,false).map_err(|e|e.to_string())?;
 }
 window.show().map_err(|e|e.to_string())?;
 let visible=window.is_visible().map_err(|e|e.to_string())?;
 boot_diagnostics::record("BOOT_15_SHOW_MAIN_RETURNED", format!("result=ok visible={visible}"));
 if !visible{return Err("主窗口 show() 后仍不可见".into());}
 boot_diagnostics::record("BOOT_16_MAIN_WINDOW_VISIBLE", "visibility confirmed by window API");
 Ok(())
}
pub fn window_event(window:&tauri::Window,event:&WindowEvent){
 if window.label()!="main"{return}let app=window.app_handle().clone();let memory=app.state::<WindowMemory>();
 if matches!(event,WindowEvent::Moved(_)|WindowEvent::Resized(_))&&!window.is_maximized().unwrap_or(false)&&!window.is_minimized().unwrap_or(false){
  if let (Ok(size),Ok(position),Ok(scale))=(window.inner_size(),window.outer_position(),window.scale_factor()){let size=size.to_logical::<f64>(scale);if size.width>0.0&&size.height>0.0{*memory.normal.lock().unwrap()=Some(WindowState{width:size.width,height:size.height,x:position.x,y:position.y,maximized:false});}}
 }
 if let WindowEvent::CloseRequested{api,..}=event{
  if memory.closing.swap(true,Ordering::SeqCst){return}api.prevent_close();
  let mut saved=memory.normal.lock().unwrap().clone();if let Some(ref mut s)=saved{s.maximized=window.is_maximized().unwrap_or(false)}let target=window.clone();
  tauri::async_runtime::spawn(async move{let state=app.state::<DbInstances>();if let (Ok(pool),Some(saved))=(datasets::pool(&state).await,saved){let result=sqlx::query("INSERT INTO settings(key,value) VALUES('startup.window_state',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value").bind(serde_json::to_string(&saved).unwrap()).execute(&pool).await;if let Err(error)=result{eprintln!("Could not save window geometry: {error}")}}let _=target.close();});
 }
}
