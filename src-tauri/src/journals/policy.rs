use super::engine;
use chrono::{Duration,Utc};
use serde_json::{json,Value};
use sqlx::SqlitePool;
pub const DAYS:i64=14;
pub const LIMIT:i64=10;

/// Admission and delivery are separate: matching configuration never creates a push.
/// One transaction publishes at most ten distinct unseen papers across all journals.
pub async fn publish(pool:&SqlitePool,source:&str,journal:Option<&str>)->Result<u32,String>{
 let mut tx=pool.begin().await.map_err(|e|e.to_string())?;
 let since=(Utc::now()-Duration::days(DAYS)).format("%Y-%m-%d").to_string();let today=Utc::now().format("%Y-%m-%d").to_string();
 let rows:Vec<(String,String,f64)>=sqlx::query_as(
  "WITH eligible AS (SELECT p.id,pm.topic_id,pm.score,p.published_at,ROW_NUMBER() OVER(PARTITION BY p.id ORDER BY pm.score DESC,pm.topic_id) AS rn FROM papers p JOIN paper_matches pm ON pm.paper_id=p.id JOIN topics t ON t.id=pm.topic_id AND t.enabled=1 WHERE pm.tier IN ('high','medium') AND COALESCE(json_extract(pm.reason_json,'$.configured_keywords'),0)>=3 AND date(p.published_at)>=date(?) AND date(p.published_at)<=date(?) AND (?='all' OR EXISTS(SELECT 1 FROM paper_source_records ps WHERE ps.paper_id=p.id AND ps.source=?)) AND (? IS NULL OR p.journal_id=? OR EXISTS(SELECT 1 FROM json_each(p.metadata_json,'$.crossref_journal_ids') j WHERE j.value=?)) AND NOT EXISTS(SELECT 1 FROM push_items old WHERE old.paper_id=p.id) AND (NOT EXISTS(SELECT 1 FROM paper_source_records cr WHERE cr.paper_id=p.id AND cr.source='crossref') OR EXISTS(SELECT 1 FROM paper_source_records ax WHERE ax.paper_id=p.id AND ax.source='arxiv') OR EXISTS(SELECT 1 FROM topic_journal_subscriptions ts JOIN journal_subscriptions js ON js.id=ts.subscription_id AND js.enabled=1 AND js.sync_enabled=1 WHERE ts.topic_id=pm.topic_id AND ts.enabled=1 AND (js.journal_id=p.journal_id OR EXISTS(SELECT 1 FROM json_each(p.metadata_json,'$.crossref_journal_ids') j WHERE j.value=js.journal_id))))) SELECT id,topic_id,score FROM eligible WHERE rn=1 ORDER BY date(published_at) DESC,score DESC,published_at DESC,id LIMIT 10")
  .bind(since).bind(today).bind(source).bind(source).bind(journal).bind(journal).bind(journal).fetch_all(&mut *tx).await.map_err(|e|e.to_string())?;
 let now=Utc::now().to_rfc3339();
 sqlx::query("DELETE FROM journal_feed_batch").execute(&mut *tx).await.map_err(|e|e.to_string())?;
 for (paper,topic,score) in &rows{
  let id=uuid::Uuid::new_v4().to_string();
  sqlx::query("INSERT INTO push_items(id,paper_id,topic_id,match_score) VALUES(?,?,?,?)").bind(&id).bind(paper).bind(topic).bind(score).execute(&mut *tx).await.map_err(|e|e.to_string())?;
  sqlx::query("INSERT INTO journal_feed_batch(paper_id,push_item_id,delivered_at) VALUES(?,?,?)").bind(paper).bind(id).bind(&now).execute(&mut *tx).await.map_err(|e|e.to_string())?;
 }
 sqlx::query("UPDATE journal_feed_batch_state SET updated_at=?,count=? WHERE id=1").bind(now).bind(rows.len() as i64).execute(&mut *tx).await.map_err(|e|e.to_string())?;
 tx.commit().await.map_err(|e|e.to_string())?;Ok(rows.len() as u32)
}
pub async fn ensure_topics(pool:&SqlitePool)->Result<(),String>{
 let ids:Vec<String>=sqlx::query_scalar("SELECT id FROM topics WHERE enabled=1").fetch_all(pool).await.map_err(|e|e.to_string())?;
 for id in ids {let topic=engine::read_topic(pool,&id).await?;let count=topic.keywords.iter().filter(|k|k.enabled&&k.kind!="exclude").map(|k|engine::keyword_identity(&k.phrase)).collect::<std::collections::HashSet<_>>().len();if count>=3{return Ok(())}}
 Err("请先配置一个至少含 3 个启用正向关键词的主题；排除词不计入数量".into())
}
pub async fn recompute(pool:&SqlitePool)->Result<(),String>{
 let mut tx=pool.begin().await.map_err(|e|e.to_string())?;
 let ids:Vec<String>=sqlx::query_scalar("SELECT id FROM papers").fetch_all(&mut *tx).await.map_err(|e|e.to_string())?;
 for id in ids{engine::match_paper(&mut tx,&id).await?;}
 tx.commit().await.map_err(|e|e.to_string())?;Ok(())
}
pub async fn info(pool:&SqlitePool)->Result<Value,String>{
 let updated:Option<String>=sqlx::query_scalar("SELECT updated_at FROM journal_feed_batch_state WHERE id=1").fetch_one(pool).await.map_err(|e|e.to_string())?;
 Ok(json!({"days":DAYS,"limit":LIMIT,"updatedAt":updated}))
}
