use super::*;
use sqlx::Row;
fn relocated(value:&str,old:&Path,new:&Path,groups:&[PathBuf])->Option<String>{
 let path=Path::new(value);if !path.is_absolute()||!within(path,old){return None}
 // Only redirect content that is actually part of this migration. Existing
 // target modules win, and external references remain untouched.
 let normalized=display(path).replace('\\',"/");let base=display(old).replace('\\',"/");let relative=normalized.get(base.trim_end_matches('/').len()..)?.trim_start_matches('/');
 let relative=PathBuf::from(relative);if !groups.iter().any(|g|within(&old.join(&relative),&old.join(g))){return None}Some(display(&new.join(relative)))
}
async fn columns(db:&mut SqliteConnection,table:&str)->Result<Vec<String>>{Ok(sqlx::query(&format!("PRAGMA table_info({table})")).fetch_all(db).await.map_err(error)?.iter().map(|row|row.get("name")).collect())}
async fn field(db:&mut SqliteConnection,table:&str,column:&str,key_column:Option<&str>,old:&Path,new:&Path,groups:&[PathBuf])->Result<()>{
 let schema=columns(db,table).await?;if !schema.iter().any(|n|n==column)||!schema.iter().any(|n|n=="id"){return Ok(())}
 let rows=sqlx::query(&format!("SELECT id,{column} FROM {table} WHERE {column} IS NOT NULL")).fetch_all(&mut *db).await.map_err(error)?;
 for row in rows{let id:String=row.get("id");let value:String=row.get(column);if let Some(updated)=relocated(&value,old,new,groups){
  sqlx::query(&format!("UPDATE {table} SET {column}=? WHERE id=?")).bind(&updated).bind(&id).execute(&mut *db).await.map_err(error)?;
  if let Some(key_column)=key_column.filter(|c|schema.iter().any(|n|n==c)){sqlx::query(&format!("UPDATE {table} SET {key_column}=? WHERE id=?")).bind(key(Path::new(&updated))).bind(&id).execute(&mut *db).await.map_err(error)?;}
 }}Ok(())
}
pub async fn copied_databases(stage:&Path,old:&Path,new:&Path,groups:&[PathBuf])->Result<()>{
 for relative in ["database/scidesk.db","database/manuals/manuals.db","database/learning/learning.db","database/tools/tools.db"]{
  let path=stage.join(relative);if !path.is_file(){continue}let mut db=SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(path)).await.map_err(error)?;
  sqlx::query("BEGIN IMMEDIATE").execute(&mut db).await.map_err(error)?;
  match relative{
   "database/scidesk.db"=>{field(&mut db,"projects","root_path",Some("root_key"),old,new,groups).await?;field(&mut db,"projects","cover_path",None,old,new,groups).await?;
    if !columns(&mut db,"settings").await?.is_empty(){let rows=sqlx::query("SELECT key,value FROM settings WHERE key LIKE 'home.background%'").fetch_all(&mut db).await.map_err(error)?;for row in rows{let key:String=row.get("key");let value:String=row.get("value");let decoded=serde_json::from_str::<String>(&value).ok();let text=decoded.as_deref().unwrap_or(&value);if let Some(updated)=relocated(text,old,new,groups){let updated=if decoded.is_some(){json!(updated).to_string()}else{updated};sqlx::query("UPDATE settings SET value=? WHERE key=?").bind(updated).bind(key).execute(&mut db).await.map_err(error)?;}}}
   },
   "database/manuals/manuals.db"=>field(&mut db,"links","path",Some("path_key"),old,new,groups).await?,
   "database/learning/learning.db"=>field(&mut db,"backups","path",Some("path_key"),old,new,groups).await?,
   "database/tools/tools.db"=>{if columns(&mut db,"tools").await?.iter().any(|n|n=="local_json"){let rows=sqlx::query("SELECT id,local_json FROM tools WHERE local_json IS NOT NULL").fetch_all(&mut db).await.map_err(error)?;for row in rows{let id:String=row.get("id");let text:String=row.get("local_json");let mut value:Value=serde_json::from_str(&text).map_err(error)?;if let Some(updated)=value["root"].as_str().and_then(|p|relocated(p,old,new,groups)){value["root"]=json!(updated);sqlx::query("UPDATE tools SET local_json=? WHERE id=?").bind(value.to_string()).bind(id).execute(&mut db).await.map_err(error)?;}}}},
   _=>{}
  }
  sqlx::query("COMMIT").execute(&mut db).await.map_err(error)?;db.close().await.map_err(error)?;
 }
 Ok(())
}
