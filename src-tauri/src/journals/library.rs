use super::engine;
use serde_json::{json,Value};
use sqlx::SqlitePool;
pub fn entries()->Result<Vec<Value>,String>{serde_json::from_str(include_str!("../../journal_catalog.json")).map_err(|e|e.to_string())}
/// Public source metadata requested by the user; no v1 data or paper seeds.
pub async fn initialize(pool:&SqlitePool)->Result<(),String>{
 for entry in entries()?{
  let aliases=entry["issnAliases"].to_string();
  let existing:Option<String>=sqlx::query_scalar("SELECT id FROM journals WHERE name=? COLLATE NOCASE OR issn_print IN(SELECT value FROM json_each(?)) OR issn_online IN(SELECT value FROM json_each(?)) LIMIT 1").bind(entry["name"].as_str()).bind(&aliases).bind(&aliases).fetch_optional(pool).await.map_err(|e|e.to_string())?;
  let journal=if let Some(id)=existing{engine::set_journal_subscription(pool,&id,true).await?}else{
   engine::save_journal(pool,None,serde_json::from_value(json!({"name":entry["name"],"shortName":entry["shortName"],"issnPrint":entry["issnPrint"],"issnOnline":entry["issnOnline"],"publisher":entry["publisher"],"websiteUrl":entry["websiteUrl"],"series":entry["group"]})).map_err(|e|e.to_string())?).await?
  };
  sqlx::query("UPDATE journals SET series=? WHERE id=? AND series='自定义'").bind(entry["group"].as_str()).bind(&journal.id).execute(pool).await.map_err(|e|e.to_string())?;
 }
 // All existing library entries are also sources; no follow state is exposed.
 let ids:Vec<String>=sqlx::query_scalar("SELECT id FROM journals").fetch_all(pool).await.map_err(|e|e.to_string())?;
 for id in ids{engine::set_journal_subscription(pool,&id,true).await?;}
 sqlx::query("INSERT OR IGNORE INTO topic_journal_subscriptions(topic_id,subscription_id,enabled) SELECT t.id,js.id,1 FROM topics t CROSS JOIN journal_subscriptions js WHERE t.enabled=1 AND js.enabled=1 AND js.sync_enabled=1").execute(pool).await.map_err(|e|e.to_string())?;
 super::policy::recompute(pool).await?;
 Ok(())
}
