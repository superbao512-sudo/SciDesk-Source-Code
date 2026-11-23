use super::{engine::{discovery,search::Record},profile,Service};
use chrono::Utc;
use serde::Serialize;
use sqlx::{FromRow,SqlitePool};
use std::{collections::HashSet,sync::Arc,time::Duration};

#[derive(Default,Serialize)]
#[serde(rename_all="camelCase")]
pub(super) struct Feed {
    bound: bool,
    interests: Vec<String>,
    items: Vec<Record>,
    updated_at: Option<String>,
    warnings: Vec<String>,
    refreshing: bool,
}

#[derive(FromRow)]
struct Cache {items:String,warnings:String,updated_at:Option<String>,lease_until:i64}

async fn researcher(pool:&SqlitePool)->Result<(Option<String>,Option<profile::Snapshot>),String> {
    let (id,json):(Option<String>,Option<String>)=sqlx::query_as("SELECT scholar_user_id,snapshot_json FROM scholar_profile WHERE id=1")
        .fetch_one(pool).await.map_err(|e|e.to_string())?;
    let snapshot=json.map(|s|serde_json::from_str(&s)).transpose().map_err(|e|format!("主页缓存读取失败：{e}"))?;
    Ok((id,snapshot))
}

fn interests(snapshot:Option<&profile::Snapshot>)->Vec<String>{
    snapshot.into_iter().flat_map(|s|s.interests.iter()).filter(|s|!s.trim().is_empty()).take(8).cloned().collect()
}

pub(super) async fn cached(pool:&SqlitePool)->Result<Feed,String>{
    let (id,snapshot)=researcher(pool).await?;
    let mut feed=Feed{bound:id.is_some(),interests:interests(snapshot.as_ref()),..Default::default()};
    if let Some(id)=id {
        let cache=sqlx::query_as::<_,Cache>("SELECT items,warnings,updated_at,lease_until FROM scholar_recommendations WHERE scholar_id=? AND interests=?")
            .bind(id).bind(serde_json::json!(feed.interests).to_string()).fetch_optional(pool).await.map_err(|e|e.to_string())?;
        if let Some(c)=cache{
            feed.items=serde_json::from_str(&c.items).map_err(|e|e.to_string())?;
            feed.warnings=serde_json::from_str(&c.warnings).map_err(|e|e.to_string())?;
            feed.updated_at=c.updated_at;
            feed.refreshing=c.lease_until>Utc::now().timestamp();
        }
    }
    Ok(feed)
}

// Give each interest a place before a high-volume subject fills the ten new slots.
fn selection(rows:&[Record],terms:&[String])->Vec<Record>{
    let mut selected=Vec::new();
    let mut seen=HashSet::new();
    loop {
        let before=selected.len();
        for term in terms {
            if let Some(row)=rows.iter().find(|r|r.matched_keywords.contains(term)&&!seen.contains(&r.url)){
                seen.insert(row.url.clone());selected.push(row.clone());
                if selected.len()==10{return selected}
            }
        }
        if selected.len()==before{return selected}
    }
}

pub(super) async fn refresh(pool:&SqlitePool,force:bool)->Result<Feed,String>{
    let (id,snapshot)=researcher(pool).await?;
    let labels=interests(snapshot.as_ref());
    let (Some(id),Some(snapshot))=(id,snapshot) else{return cached(pool).await};
    if labels.is_empty(){return cached(pool).await}
    let terms=discovery::terms(&labels)?;
    let key=serde_json::json!(labels).to_string();
    let token=uuid::Uuid::new_v4().to_string();
    let now=Utc::now().timestamp();
    // A lease also coordinates multiple open windows/processes and expires after a crash.
    let claimed=sqlx::query("INSERT INTO scholar_recommendations(scholar_id,interests,claim,lease_until) VALUES(?,?,?,?) ON CONFLICT(scholar_id,interests) DO UPDATE SET claim=excluded.claim,lease_until=excluded.lease_until WHERE lease_until<=? AND (next_check<=? OR ?)")
        .bind(&id).bind(&key).bind(&token).bind(now+240).bind(now).bind(now).bind(force)
        .execute(pool).await.map_err(|e|e.to_string())?;
    if claimed.rows_affected()==0{return cached(pool).await}
    let stored:String=sqlx::query_scalar("SELECT items FROM scholar_recommendations WHERE scholar_id=? AND interests=?")
        .bind(&id).bind(&key).fetch_one(pool).await.map_err(|e|e.to_string())?;
    let previous:Vec<Record>=serde_json::from_str(&stored).map_err(|e|e.to_string())?;
    let own:HashSet<_>=snapshot.works.iter().map(|w|super::engine::normalize_phrase(&w.title)).collect();
    let mut results=discovery::lookup(&terms,14).await.unwrap_or_else(|e|discovery::Results{warnings:vec![e],..Default::default()});
    results.items.retain(|r|!own.contains(&super::engine::normalize_phrase(&r.title)));
    if results.items.is_empty() && previous.is_empty() && results.successes>0 {
        if let Ok(mut wider)=discovery::lookup(&terms,90).await{
            wider.items.retain(|r|!own.contains(&super::engine::normalize_phrase(&r.title)));
            results.items=wider.items;
            for warning in wider.warnings {if !results.warnings.contains(&warning){results.warnings.push(warning);}}
        }
    }
    let mut articles=selection(&results.items,&terms);
    articles.extend(previous);
    let mut merged=discovery::merge(articles);
    merged.truncate(100);
    let success=(results.successes>0).then(||Utc::now().to_rfc3339());
    let retry=if results.warnings.is_empty(){3600}else{600};
    sqlx::query("UPDATE scholar_recommendations SET items=?,warnings=?,updated_at=coalesce(?,updated_at),next_check=?,lease_until=0,claim=NULL WHERE scholar_id=? AND interests=? AND claim=?")
        .bind(serde_json::json!(merged).to_string()).bind(serde_json::json!(results.warnings).to_string())
        .bind(success).bind(Utc::now().timestamp()+retry).bind(id).bind(key).bind(token)
        .execute(pool).await.map_err(|e|e.to_string())?;
    cached(pool).await
}

impl Service{
    pub fn start_recommendations(self:&Arc<Self>){
        let weak=Arc::downgrade(self);
        tokio::spawn(async move{
            while let Some(service)=weak.upgrade(){
                if let Err(error)=refresh(&service.pool,false).await{eprintln!("期刊推荐更新失败：{error}");}
                drop(service);
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn row(title:&str,term:&str)->Record{
        Record{title:title.into(),authors:vec![],journal:None,published_at:Some("2026-11-01".into()),year:Some(2026),
            doi:None,arxiv_id:None,url:format!("https://doi.org/10.1000/{title}"),source:"crossref".into(),matched_keywords:vec![term.into()]}
    }
    #[test]
    fn a_narrow_interest_survives_the_batch_cap(){
        let mut rows:Vec<_>=(0..12).map(|i|row(&i.to_string(),"broad")).collect();
        rows.push(row("rare","narrow"));
        let batch=selection(&rows,&["broad".into(),"narrow".into()]);
        assert_eq!(batch.len(),10);
        assert_eq!(batch[1].title,"rare");
        assert_eq!(discovery::merge([batch.clone(),batch].concat()).len(),10);
    }
    #[tokio::test]
    async fn cached_recommendations_survive_restart_and_are_scoped_to_the_interests_and_researcher(){
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../development/verification/recommendation-unit").join(uuid::Uuid::new_v4().to_string());
        let service=Service::open(&root).await.unwrap();
        assert!(!refresh(&service.pool,false).await.unwrap().bound);
        let snapshot=json!({"name":"Fixture","affiliation":"","interests":["chiral phonons"],"works":[]});
        sqlx::query("UPDATE scholar_profile SET scholar_user_id='researcher-A',snapshot_json=?").bind(snapshot.to_string()).execute(&service.pool).await.unwrap();
        let terms=json!(["chiral phonons"]).to_string();
        sqlx::query("INSERT INTO scholar_recommendations(scholar_id,interests,items,next_check) VALUES('researcher-A',?,?,?)")
            .bind(terms).bind(json!([row("a","chiral phonons")]).to_string()).bind(Utc::now().timestamp()+3600).execute(&service.pool).await.unwrap();
        service.pool.close().await;
        let reopened=Service::open(&root).await.unwrap();
        assert_eq!(refresh(&reopened.pool,false).await.unwrap().items.len(),1);
        sqlx::query("UPDATE scholar_profile SET snapshot_json=json_set(snapshot_json,'$.interests',json('[\"other\"]'))").execute(&reopened.pool).await.unwrap();
        assert!(cached(&reopened.pool).await.unwrap().items.is_empty());
        sqlx::query("UPDATE scholar_profile SET scholar_user_id='researcher-B', snapshot_json=?").bind(snapshot.to_string()).execute(&reopened.pool).await.unwrap();
        assert!(cached(&reopened.pool).await.unwrap().items.is_empty());
        reopened.pool.close().await;
    }
}
