use std::{io::{self,Read},path::PathBuf};
use serde_json::{Value,json};
use scidesk_lib::project_service::{Service,application_root,migrate};
fn main(){
 let result=tauri::async_runtime::block_on(async{
  let database=std::env::args().nth(1).map(PathBuf::from).ok_or("缺少第二版数据库位置")?;
  let root=application_root()?;
  if !database.starts_with(&root){return Err("开发桥接数据库必须位于第二版目录内".into())}
  std::fs::create_dir_all(database.parent().ok_or("数据库路径无效")?).map_err(|e|e.to_string())?;
  let pool=sqlx::sqlite::SqlitePoolOptions::new().max_connections(4).connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&database).create_if_missing(true).foreign_keys(true).busy_timeout(std::time::Duration::from_secs(10))).await.map_err(|e|e.to_string())?;
  migrate(&pool).await?;
  let mut input=String::new();io::stdin().take(4194305).read_to_string(&mut input).map_err(|e|e.to_string())?;if input.len()>4194304{return Err("请求过大".into())}
  let request:Value=serde_json::from_str(&input).map_err(|e|e.to_string())?;
  let service=Service::new(pool.clone(),root,database.parent().unwrap().join("operations"));
  #[cfg(debug_assertions)] let service=if let Ok(path)=std::env::var("SCIDESK_TEST_ROOT"){service.for_verification(std::path::Path::new(&path))?}else{service};
  let result=service.execute(request["op"].as_str().ok_or("缺少操作名称")?,request.get("args").cloned().unwrap_or(json!({}))).await;
  pool.close().await;result
 });
 println!("{}",match result{Ok(value)=>json!({"ok":true,"value":value}),Err(error)=>json!({"ok":false,"error":error})});
}
