use super::{Settings,State};
use tauri::{AppHandle,Manager};
pub fn parse(shortcut:&str)->Result<(u32,u32),String>{
 let mut modifiers=0u32;let mut key=None;
 for part in shortcut.split('+').map(str::trim){
  let bit=match part.to_ascii_lowercase().as_str(){"alt"=>Some(1),"ctrl"=>Some(2),"shift"=>Some(4),"win"=>Some(8),_=>None};
  if let Some(bit)=bit{if modifiers&bit!=0{return Err("快捷键包含重复修饰键".into())}modifiers|=bit;continue}
  if key.is_some(){return Err("请使用修饰键加一个字母、数字或 F1–F12".into())}
  let upper=part.to_ascii_uppercase();
  key=if upper.len()==1&&upper.chars().all(|c|c.is_ascii_uppercase()||c.is_ascii_digit()){Some(upper.as_bytes()[0] as u32)}else{upper.strip_prefix('F').and_then(|v|v.parse::<u32>().ok()).filter(|n|(1..=12).contains(n)).map(|n|111+n)};
  if key.is_none(){return Err("快捷键仅支持字母、数字或 F1–F12".into())}
 }
 let key=key.ok_or("快捷键缺少按键")?;
 if modifiers==0||modifiers==4{return Err("请包含 Ctrl、Alt 或 Win 修饰键".into())}
 if modifiers&8!=0{return Err("Win 组合键保留给系统，请使用 Ctrl 或 Alt".into())}
 if (modifiers==1&&key==115)||(modifiers==2&&key==87){return Err("此快捷键与关闭窗口操作冲突".into())}Ok((modifiers,key))
}
pub fn validate(settings:&Settings)->Result<(),String>{if !matches!(settings.delay,0|3|5){return Err("延时仅支持 0、3、5 秒".into())}parse(&settings.shortcut).map(|_|())}
pub fn initialize(app:&AppHandle){
 let (sender,receiver)=std::sync::mpsc::channel();let state=app.state::<State>();*state.hotkey.lock().unwrap()=Some(sender.clone());let initial=state.settings.lock().unwrap().clone();let handle=app.clone();
 std::thread::spawn(move||{
  #[cfg(windows)]{
   use windows::Win32::UI::{Input::KeyboardAndMouse::{RegisterHotKey,UnregisterHotKey,HOT_KEY_MODIFIERS,MOD_NOREPEAT},WindowsAndMessaging::{PeekMessageW,MSG,PM_REMOVE,WM_HOTKEY}};
   let mut registered=None;let mut next_id=4100;
   loop{
    match receiver.recv_timeout(std::time::Duration::from_millis(40)){
     Ok(settings)=>{
      if let Some(id)=registered.take(){unsafe{let _=UnregisterHotKey(None,id);}}
      let result=if !settings.enabled{Ok(())}else{parse(&settings.shortcut).and_then(|(mods,key)|unsafe{RegisterHotKey(None,next_id,HOT_KEY_MODIFIERS(mods)|MOD_NOREPEAT,key)}.map_err(|e|format!("快捷键 {} 注册失败，可能已被其他程序占用：{e}。仍可点击高级截图。",settings.shortcut)))};
      if result.is_ok(){if let Some(id)=registered.take(){unsafe{let _=UnregisterHotKey(None,id);}}if settings.enabled{registered=Some(next_id);next_id=if next_id==4100{4101}else{4100};}}
      else {if let Some(id)=registered.take(){unsafe{let _=UnregisterHotKey(None,id);}}}
      let error=result.err();*handle.state::<State>().registration.lock().unwrap()=error.clone();if let Some(error)=error{super::report(&handle,&error);}
     },
     Err(std::sync::mpsc::RecvTimeoutError::Disconnected)=>break,
     Err(std::sync::mpsc::RecvTimeoutError::Timeout)=>{},
    }
    unsafe{let mut msg=MSG::default();while PeekMessageW(&mut msg,None,0,0,PM_REMOVE).as_bool(){if msg.message==WM_HOTKEY&&registered==Some(msg.wParam.0 as i32){let app=handle.clone();let delay=app.state::<State>().settings.lock().unwrap().delay;tauri::async_runtime::spawn(async move{if let Err(e)=super::begin(app.clone(),delay).await{super::report(&app,&e);}});}}}
   }
   if let Some(id)=registered{unsafe{let _=UnregisterHotKey(None,id);}}
  }
  #[cfg(not(windows))]{let _=receiver;*handle.state::<State>().registration.lock().unwrap()=Some("全局截图快捷键需要 Windows".into());}
 });let _=sender.send(initial);
}
