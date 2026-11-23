use sqlx::SqlitePool;
use tauri_plugin_sql::{DbInstances,DbPool};
// Startup compatibility only; scientific data operations are not migrated.
pub async fn pool(state:&tauri::State<'_,DbInstances>)->Result<SqlitePool,String>{
 let instances=state.0.read().await;let Some(DbPool::Sqlite(pool))=instances.get("sqlite:scidesk.db") else{return Err("数据库尚未打开".into())};Ok(pool.clone())
}
