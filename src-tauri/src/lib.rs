use tauri::Manager;
use tauri_plugin_sql::{DbInstances,DbPool};
mod boot_diagnostics;
mod runtime_paths;
mod personal_seed;
mod installed_storage;
mod file_transfer;
mod datasets;
mod settings_core;
mod settings_window;
mod window_geometry;
mod project_platform;
pub mod project_service;
mod manuals;
#[cfg(windows)] mod book_pdf;
mod learning;
mod tools;
mod screenshot;
pub mod ideas;
pub mod journals;

#[cfg(windows)]
fn apply_title_bar_background(window: &tauri::WebviewWindow) -> Result<(), String> {
 use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
 let hwnd = window.hwnd().map_err(|error| error.to_string())?;
 let enabled: i32 = 0;
 // Keep the native caption light; this switch cannot select a custom gray color.
 unsafe {
  DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, (&enabled as *const i32).cast(), std::mem::size_of_val(&enabled) as u32)
 }.map_err(|error| error.to_string())
}

mod updates;
mod translation;
#[tauri::command]
async fn project_command(window:tauri::WebviewWindow,state:tauri::State<'_,DbInstances>,op:String,mut args:serde_json::Value)->Result<serde_json::Value,String>{
 if window.label()!="main"{return Err("文件操作仅限主窗口".into())}
 if !args.is_object(){return Err("文件操作参数必须是对象".into())}
 #[cfg(windows)] {args["_native_window"]=serde_json::json!(window.hwnd().map_err(|e|e.to_string())?.0 as usize);}
 let root=project_service::application_root()?;
 let journal=runtime_paths::data_root()?.join("database/operations");
 let service=project_service::Service::new(datasets::pool(&state).await?,root,journal);
 #[cfg(debug_assertions)]
 let service=if std::env::var_os("SCIDESK_DESKTOP_TEST_ROOT").is_some(){service.for_verification(runtime_paths::data_root()?.parent().ok_or("测试根目录无效")?)?}else{service};
 #[cfg(windows)] if op=="manual_book_pdf"{
  let target=service.execute("manual_book_pdf_target",args).await?;
  book_pdf::export(window,std::path::PathBuf::from(target.as_str().ok_or("PDF 路径无效")?)).await?;
  return Ok(target)
 }
 if matches!(op.as_str(),"drag_files"|"manual_drag_files"){
  let resolved=service.execute(&op.replace("drag_files","drag_paths"),args).await?;
  let paths:Vec<std::path::PathBuf>=serde_json::from_value(resolved).map_err(|e|e.to_string())?;
  let (send,receive)=tokio::sync::oneshot::channel();
  // OLE owns a nested Windows message pump while dragging. Start it on the
  // source window's UI thread, where the actual mouse-down was received.
  window.app_handle().run_on_main_thread(move||{let result=file_transfer::Guard::new(paths.clone()).and_then(|guard|file_transfer::drag(guard,paths));let _=send.send(result);}).map_err(|e|e.to_string())?;
  receive.await.map_err(|_|"文件拖出任务意外中断".to_owned())?
 }else{service.execute(&op,args).await}
}
#[tauri::command]
async fn ideas_command(state:tauri::State<'_,ideas::IdeasState>,op:String,args:serde_json::Value)->ideas::store::Result<serde_json::Value>{state.0.command(&op,args).await}
#[tauri::command]
async fn journal_command(state:tauri::State<'_,journals::JournalState>,op:String,args:serde_json::Value)->Result<serde_json::Value,String>{state.0.as_ref().map_err(|e|format!("期刊存储不可用：{e}"))?.command(&op,args).await}
#[cfg_attr(mobile,tauri::mobile_entry_point)]
pub fn run(){
 let application_root=project_service::application_root().expect("application directory");
 let root=runtime_paths::data_root().unwrap_or_else(|e|installed_storage::fatal(&e));
 let database=root.join("database/scidesk.db");let cache=root.join("cache/webview");
 // WebView2 must never reuse the first version's profile or application data.
 std::env::set_var("WEBVIEW2_USER_DATA_FOLDER",&cache);
 tauri::Builder::default().plugin(tauri_plugin_sql::Builder::default().build())
 .manage(settings_window::WindowMemory::default()).on_window_event(|window,event|{settings_window::window_event(window,event);screenshot::window_event(window,event);file_transfer::window_event(window,event);})
 .setup(move|app|{
  installed_storage::writable_root(&root).map_err(std::io::Error::other)?;
  if !cfg!(debug_assertions) {
   if let Some(source)=runtime_paths::legacy_data_root(){let migrated=tauri::async_runtime::block_on(installed_storage::migrate_legacy(&source,&root)).map_err(std::io::Error::other)?;boot_diagnostics::record("STORAGE_MIGRATION",migrated.to_string());}
   let seeded=personal_seed::initialize(&app.path().resource_dir()?.join("personal-seed"),&root).map_err(std::io::Error::other)?;
   boot_diagnostics::record("PERSONAL_SEED_READY",seeded.join(","));
  }
  installed_storage::prepare_paths(&root).map_err(std::io::Error::other)?;
  boot_diagnostics::record("PROCESS_ENTRY",format!("data_root={}",root.display()));
  app.manage(screenshot::State::new(&root.join("tools")));
  screenshot::initialize(app.handle());
  tauri::async_runtime::block_on(async{
   let options=sqlx::sqlite::SqliteConnectOptions::new().filename(&database).create_if_missing(true).foreign_keys(true);
   let pool=sqlx::sqlite::SqlitePoolOptions::new().max_connections(4).connect_with(options).await?;
   project_service::migrate(&pool).await.map_err(std::io::Error::other)?;
   let integrity:String=sqlx::query_scalar("PRAGMA quick_check").fetch_one(&pool).await?;
   if integrity!="ok"{return Err(std::io::Error::other(integrity).into())}
   let ideas_root=root.join("ideas");
   let idea_store=ideas::store::Store::open(ideas_root,pool.clone()).await.map_err(|e|std::io::Error::other(e["message"].as_str().unwrap_or("想法存储初始化失败")))?;
   app.manage(ideas::IdeasState(ideas::server::Hub::new(idea_store)));
   let journal_root=if cfg!(debug_assertions) {
    if let Ok(path)=std::env::var("SCIDESK_JOURNAL_TEST_ROOT"){
     let path=std::fs::canonicalize(path)?;
     if !path.starts_with(std::fs::canonicalize(application_root.join("development/verification"))?){return Err(std::io::Error::other("期刊测试目录越界").into())}path.join("journals")
    }else{root.join("journals")}
   }else{root.join("journals")};
   let journals=journals::Service::open(&journal_root).await;
   if let Ok(service)=&journals {service.start_recommendations();}
   app.manage(journals::JournalState(journals));
   app.state::<DbInstances>().0.write().await.insert("sqlite:scidesk.db".into(),DbPool::Sqlite(pool));
   Ok::<(),Box<dyn std::error::Error>>(())
  })?;
  boot_diagnostics::record("DATABASE_READY",database.display().to_string());
  let handle=app.handle().clone();let config=app.config().app.windows[0].clone();let profile=cache.clone();
  handle.clone().run_on_main_thread(move||{
   boot_diagnostics::record("WINDOW_BUILD_REQUESTED","main");
   match tauri::WebviewWindowBuilder::from_config(&handle,&config).and_then(|builder|builder.data_directory(profile).build()){
    Ok(_window)=>{
     boot_diagnostics::record("WINDOW_BUILD_RETURNED","ok");
     #[cfg(windows)]
     match apply_title_bar_background(&_window){Ok(())=>boot_diagnostics::record("TITLE_BAR_BACKGROUND","native light caption applied"),Err(error)=>boot_diagnostics::record("TITLE_BAR_BACKGROUND_FAILED",error)}
    },
    Err(error)=>{boot_diagnostics::record("WINDOW_BUILD_FAILED",error.to_string());eprintln!("{error}");}
   }
  })?;Ok(())
 }).invoke_handler(tauri::generate_handler![updates::check_release,updates::open_release_page,translation::translate_with_provider,translation::translation_files,translation::translation_install,translation::translation_read,project_command,ideas_command,journal_command,screenshot::screenshot_command,boot_diagnostics::boot_log,settings_window::configure_main_window,settings_window::show_main_window])
 .run(tauri::generate_context!()).unwrap_or_else(|e|installed_storage::fatal(&format!("SciDesk 无法初始化同级 SciDeskData：{e}")));
}
