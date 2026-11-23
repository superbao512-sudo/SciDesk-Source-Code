use std::path::PathBuf;
use tauri::WebviewWindow;
use webview2_com::{Microsoft::Web::WebView2::Win32::ICoreWebView2_7,PrintToPdfCompletedHandler};
use windows::core::{Interface,HSTRING};

pub async fn export(window:WebviewWindow,target:PathBuf)->Result<(),String>{
 let temp=target.with_file_name(format!(".scidesk-{}.pdf",uuid::Uuid::new_v4()));
 let output=HSTRING::from(temp.to_string_lossy().as_ref());
 let (start,started)=tokio::sync::oneshot::channel();
 let (finish,finished)=tokio::sync::oneshot::channel();
 window.with_webview(move |platform|{
  let handler=PrintToPdfCompletedHandler::create(Box::new(move |error,ok|{
   let result=error.map_err(|e|e.to_string()).and_then(|_|if ok{Ok(())}else{Err("WebView2 无法生成 PDF".to_owned())});
   let _=finish.send(result);Ok(())
  }));
  let result=(||unsafe{platform.controller().CoreWebView2()?.cast::<ICoreWebView2_7>()?.PrintToPdf(&output,None,&handler)})();
  let _=start.send(result.map_err(|e|e.to_string()));
 }).map_err(|e|e.to_string())?;
 let result=async{
  started.await.map_err(|e|e.to_string())??;
  finished.await.map_err(|e|e.to_string())??;
  crate::project_platform::rename_no_replace(&temp,&target)
 }.await;
 if result.is_err(){let _=std::fs::remove_file(temp);}
 result
}
