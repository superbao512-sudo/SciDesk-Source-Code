use super::*;
async fn pool()->SqlitePool{
 let pool=sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(":memory:").foreign_keys(true)).await.unwrap();
 sqlx::raw_sql(include_str!("../../../journal_migrations/001_journals.sql")).execute(&pool).await.unwrap();sqlx::raw_sql(include_str!("../../../journal_migrations/002_profile_batches.sql")).execute(&pool).await.unwrap();sqlx::raw_sql(include_str!("../../../journal_migrations/003_library_avatar.sql")).execute(&pool).await.unwrap();pool
}
async fn configured(pool:&SqlitePool)->(String,String){
 let journal=save_journal(pool,None,serde_json::from_value(json!({"name":"Fixture Journal","shortName":"FIX","issnPrint":"2469-9950","websiteUrl":"https://journals.example.org"})).unwrap()).await.unwrap();
 let topic=save_topic(pool,None,serde_json::from_value(json!({"name":"Fixture science","keywords":[{"phrase":"science","weight":10,"field":"both","kind":"boost"},{"phrase":"quantum","weight":5,"kind":"boost"},{"phrase":"transport","weight":5,"kind":"boost"},{"phrase":"excluded","weight":1,"kind":"exclude"}]})).unwrap()).await.unwrap();
 set_journal_topics(pool,&journal.id,vec![topic.id.clone()]).await.unwrap();(journal.id,topic.id)
}
fn body()->String{json!({"message":{"items":[{"type":"journal-article","title":["Fixture science"],"DOI":"10.5555/scidesk-journal-fixture","author":[{"given":"Test","family":"Author"}],"published":{"date-parts":[[Utc::now().format("%Y").to_string().parse::<i32>().unwrap(),Utc::now().format("%m").to_string().parse::<i32>().unwrap(),Utc::now().format("%d").to_string().parse::<i32>().unwrap()]]},"abstract":"<jats:p>Fixture science abstract.</jats:p>","URL":"https://doi.org/10.5555/scidesk-journal-fixture"}]}}).to_string()}
#[tokio::test]
async fn crossref_sync_matches_deduplicates_and_preserves_read_history(){
 let pool=pool().await;let (id,_)=configured(&pool).await;
 let target=journal_sync_targets(&pool).await.unwrap().remove(0);
 let mut fetch=|url:Url|{assert_eq!(url.host_str(),Some("api.crossref.org"));assert!(url.path().contains("2469-9950"));let params=url.query_pairs().collect::<std::collections::HashMap<_,_>>();assert_eq!(params.get("sort").map(|v|v.as_ref()),Some("published"));assert_eq!(params.get("offset").map(|v|v.as_ref()),Some("0"));assert!(!params.contains_key("cursor"),"Crossref rejects publication sorting with cursor pagination");async{Ok(body())}};
 let first=sync_crossref_journal_with_fetch(&pool,&target,&mut fetch,100,2).await.unwrap();assert_eq!((first.inserted,first.matched,first.pushed),(1,1,0));assert_eq!(first.status,"success");assert_eq!(super::super::policy::publish(&pool,"crossref",None).await.unwrap(),1);
 let feed=list_feed(&pool,PaperFeedListOptions::default()).await.unwrap();assert_eq!(feed.total,1);let item=&feed.items[0];assert_eq!(item.title,"Fixture science");assert_eq!(item.authors,vec!["Test Author"]);assert_eq!(item.abstract_text.as_deref(),Some("Fixture science abstract."));assert_eq!(item.relevance_level,"high");
 super::super::set_status(&pool,&item.push_item_id,"read").await.unwrap();
 let second=sync_crossref_journal_with_fetch(&pool,&target,&mut fetch,100,2).await.unwrap();assert_eq!(second.inserted,0);assert_eq!(second.pushed,0);
 assert_eq!(list_feed(&pool,PaperFeedListOptions::default()).await.unwrap().items[0].status,"read");
 cancel_journal_subscription(&pool,&id).await.unwrap();assert!(list_journals(&pool).await.unwrap().is_empty());
 assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM papers").fetch_one(&pool).await.unwrap(),1);
 assert_eq!(sqlx::query_scalar::<_,String>("SELECT status FROM push_items").fetch_one(&pool).await.unwrap(),"read");
}
#[tokio::test]
async fn provider_failure_does_not_advance_success_cursor(){
 let pool=pool().await;let (id,_)=configured(&pool).await;let old="2026-10-01T00:00:00Z";
 sqlx::query("UPDATE journal_subscriptions SET last_success_at=? WHERE journal_id=?").bind(old).bind(&id).execute(&pool).await.unwrap();
 let target=journal_sync_targets(&pool).await.unwrap().remove(0);let mut fetch=|_:Url|async{Err("HTTP 503 fixture failure".into())};
 let run=sync_crossref_journal_with_fetch(&pool,&target,&mut fetch,100,2).await.unwrap();assert_ne!(run.status,"success");assert_eq!(run.inserted,0);
 let journal=read_journal(&pool,&id).await.unwrap();let subscription=journal.subscription.unwrap();assert_eq!(subscription.last_success_at.as_deref(),Some(old));assert!(subscription.last_error.unwrap().contains("503"));
}
#[tokio::test]
async fn filter_pagination_and_exclusion_use_real_matching_records(){
 let pool=pool().await;let (id,topic)=configured(&pool).await;
 for index in 0..3{let mut row:Value=serde_json::from_str(&body()).unwrap();row["message"]["items"][0]["DOI"]=json!(format!("10.5555/fixture-{index}"));row["message"]["items"][0]["title"]=json!([if index==2{"Excluded science"}else if index==1{"Fixture science two"}else{"Fixture science one"}]);let candidates=parse_crossref_for_journal(&row.to_string(),&id,"Fixture Journal").unwrap();let mut tx=pool.begin().await.unwrap();let (paper,_)=ingest(&mut tx,&candidates[0]).await.unwrap();match_paper(&mut tx,&paper).await.unwrap();tx.commit().await.unwrap()}
 super::super::policy::publish(&pool,"crossref",None).await.unwrap();
 let first=list_feed(&pool,PaperFeedListOptions{limit:Some(1),..Default::default()}).await.unwrap();assert_eq!(first.total,2);assert!(first.next_cursor.is_some());
 let next=list_feed(&pool,PaperFeedListOptions{limit:Some(1),cursor:first.next_cursor,..Default::default()}).await.unwrap();assert_ne!(first.items[0].paper_id,next.items[0].paper_id);
 let filtered=list_feed(&pool,PaperFeedListOptions{search:Some("two".into()),journal_id:Some(id),topic_id:Some(topic),sort:Some("relevance".into()),..Default::default()}).await.unwrap();assert_eq!(filtered.total,1);assert!(filtered.items[0].title.ends_with("two"));
 assert!(list_feed(&pool,PaperFeedListOptions{status:Some("invalid".into()),..Default::default()}).await.is_err());
}
#[test]
fn arxiv_and_crossref_parser_keep_real_fields_and_reject_invalid_responses(){
 assert!(parse_crossref_for_journal("{}","fixture","Fixture").is_err());
 assert!(validate_arxiv_response("<feed><id>https://arxiv.org/api/errors</id><summary>failure</summary></feed>").is_err());
 let xml=r#"<feed><entry><id>https://arxiv.org/abs/2601.12345v2</id><title>Fixture science</title><summary>Fixture abstract</summary><published>2026-01-01T00:00:00Z</published><author><name>Test Author</name></author></entry></feed>"#;
 let rows=parse_arxiv(xml);assert_eq!(rows.len(),1);assert_eq!(rows[0].arxiv_version,Some(2));assert_eq!(rows[0].authors[0].display,"Test Author");
}

#[tokio::test]
async fn every_topic_requires_three_enabled_positive_distinct_keywords(){
 let pool=pool().await;
 for keywords in [json!([{"phrase":"Electride","weight":5,"kind":"boost"}]),json!([{"phrase":"Electride","weight":5,"kind":"boost"},{"phrase":"electrides","weight":5,"kind":"boost"},{"phrase":"Chiral Phonons","weight":5,"kind":"boost"}]),json!([{"phrase":"Electride","weight":5,"kind":"boost"},{"phrase":"Superconductivity","weight":5,"kind":"boost"},{"phrase":"Phonon Heat Transport","weight":5,"kind":"exclude"}]),json!([{"phrase":"Electride","weight":5,"kind":"boost"},{"phrase":"Superconductivity","weight":5,"kind":"boost"},{"phrase":"Chiral Phonons","weight":5,"kind":"boost","enabled":false}])]{
  assert!(save_topic(&pool,None,serde_json::from_value(json!({"name":"Fixture theme","keywords":keywords})).unwrap()).await.unwrap_err().contains("3"));
 }
 let topic=save_topic(&pool,None,serde_json::from_value(json!({"name":"Fixture theme","keywords":[{"phrase":"Electride","weight":2,"kind":"boost"},{"phrase":"Superconductivity","weight":8,"kind":"boost"},{"phrase":"Phonon Heat Transport","weight":6,"kind":"boost"},{"phrase":"Chiral Phonons","weight":10,"kind":"boost"}]})).unwrap()).await.unwrap();
 assert_eq!(topic.keywords.iter().find(|k|k.phrase=="superconductivity").unwrap().weight,8.0);
 assert!(keyword_hit("electrides exhibit superconductivity","Electride"));
 assert!(keyword_hit("a chiral phonon in a lattice","Chiral Phonons"));
 assert!(keyword_hit("phonon heat transport in crystals","Phonon Heat Transport"));
 assert!(!keyword_hit("phonon scattering and electronic transport","Phonon Heat Transport"));
 assert!(!keyword_hit("nonelectride alloys","Electride"));
}

#[tokio::test]
async fn required_keywords_score_without_boost_hits_and_keep_admission_rules(){
 let cases=[
  ("all", "both", "boost", "Electride superconductivity", "No optional phrase", 15.0, "high", 2),
  ("all", "both", "boost", "Electride", "Superconductivity", 12.5, "high", 2),
  ("any", "both", "boost", "Unrelated title", "Electride", 5.0, "medium", 1),
  ("all", "both", "required", "Electride superconductivity first-principles", "", 22.5, "high", 3),
  ("all", "both", "boost", "Electride electride superconductivity", "", 15.0, "high", 2),
  ("all", "both", "boost", "Electride first-principles", "", 0.0, "none", 0),
  ("any", "abstract", "boost", "Electride", "Unrelated abstract", 0.0, "none", 0),
  ("all", "both", "boost", "Electride superconductivity excluded", "", 0.0, "none", 2),
 ];
 for (logic,field,third_kind,title,abstract_text,score,tier,term_count) in cases {
  let pool=pool().await;
  let (journal,topic)=configured(&pool).await;
  save_topic(&pool,Some(&topic),serde_json::from_value(json!({
   "name":"Required keyword regression", "matchLogic":logic,
   "keywords":[
    {"phrase":"electride","weight":5,"field":field,"kind":"required"},
    {"phrase":"superconductivity","weight":5,"kind":"required"},
    {"phrase":"first principles","weight":5,"kind":third_kind},
    {"phrase":"excluded","weight":5,"kind":"exclude"}
   ]
  })).unwrap()).await.unwrap();
  let mut payload:Value=serde_json::from_str(&body()).unwrap();
  payload["message"]["items"][0]["title"]=json!([title]);
  payload["message"]["items"][0]["abstract"]=json!(abstract_text);
  let candidate=parse_crossref_for_journal(&payload.to_string(),&journal,"Fixture").unwrap().remove(0);
  let mut tx=pool.begin().await.unwrap();
  let (paper,_)=ingest(&mut tx,&candidate).await.unwrap();
  let matched=match_paper(&mut tx,&paper).await.unwrap().0;
  tx.commit().await.unwrap();
  let actual:(f64,String,String)=sqlx::query_as("SELECT score,tier,reason_json FROM paper_matches WHERE paper_id=? AND topic_id=?")
   .bind(&paper).bind(&topic).fetch_one(&pool).await.unwrap();
  assert_eq!((actual.0,actual.1.as_str()),(score,tier),"{logic}: {title} / {abstract_text}");
  assert_eq!(matched,tier!="none");
  let terms=matched_terms(&actual.2);
  assert_eq!(terms.len(),term_count);
  if tier!="none" {
   assert!(terms.iter().any(|term|term["phrase"]=="electride"));
   assert_eq!(super::super::policy::publish(&pool,"crossref",None).await.unwrap(),1);
   let page=list_feed(&pool,PaperFeedListOptions::default()).await.unwrap();
   assert_eq!(page.items.len(),1);
   assert_eq!(page.items[0].score,score);
   super::super::set_status(&pool,&page.items[0].push_item_id,"read").await.unwrap();
   let mut tx=pool.begin().await.unwrap();
   match_paper(&mut tx,&paper).await.unwrap();
   tx.commit().await.unwrap();
   assert_eq!(list_feed(&pool,PaperFeedListOptions::default()).await.unwrap().items[0].status,"read");
   assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM push_items").fetch_one(&pool).await.unwrap(),1);
  } else {
   assert_eq!(super::super::policy::publish(&pool,"crossref",None).await.unwrap(),0);
  }
 }
}

#[tokio::test]
async fn delivery_is_global_ten_latest_distinct_unseen_papers_and_keeps_history(){
 let pool=pool().await;let (journal,topic)=configured(&pool).await;
 let other=save_journal(&pool,None,serde_json::from_value(json!({"name":"Second fixture journal","shortName":"FIX2","issnPrint":"0031-9007","websiteUrl":"https://second.example.org"})).unwrap()).await.unwrap();
 set_journal_topics(&pool,&other.id,vec![topic]).await.unwrap();
 for index in 0..26{
  let mut row:Value=serde_json::from_str(&body()).unwrap();let item=&mut row["message"]["items"][0];
  item["DOI"]=json!(format!("10.5555/batch-fixture-{index}"));item["title"]=json!([format!("Fixture science record {index}")]);
  let date=Utc::now()-chrono::Duration::days(if index==24{60}else if index==25{-1}else{index/2});
  item["published"]=json!({"date-parts":[[date.format("%Y").to_string().parse::<u32>().unwrap(),date.format("%m").to_string().parse::<u32>().unwrap(),date.format("%d").to_string().parse::<u32>().unwrap()]]});
  let id=if index%2==0{&journal}else{&other.id};let candidates=parse_crossref_for_journal(&row.to_string(),id,"Fixture").unwrap();
  let mut tx=pool.begin().await.unwrap();let (paper,_)=ingest(&mut tx,&candidates[0]).await.unwrap();match_paper(&mut tx,&paper).await.unwrap();tx.commit().await.unwrap();
 }
 assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM push_items").fetch_one(&pool).await.unwrap(),0,"matching alone does not deliver");
 let mut seen=HashSet::new();
 for expected in [10,10,4,0]{
  assert_eq!(super::super::policy::publish(&pool,"crossref",None).await.unwrap(),expected);
  let page=list_feed(&pool,PaperFeedListOptions{limit:Some(100),..Default::default()}).await.unwrap();assert_eq!(page.items.len(),expected as usize);
  for pair in page.items.windows(2){assert!(pair[0].published_at>=pair[1].published_at)}
  for item in page.items{assert!(seen.insert(item.paper_id));assert!(item.published_at.unwrap()<=Utc::now().format("%Y-%m-%d").to_string());}
 }
 assert_eq!(seen.len(),24);
 assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM push_items").fetch_one(&pool).await.unwrap(),24);
 assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM papers").fetch_one(&pool).await.unwrap(),26);
}

#[tokio::test]
async fn arxiv_source_remains_eligible_when_a_shared_journal_is_removed(){
 let pool=pool().await;let (journal,_)=configured(&pool).await;let candidate=parse_crossref_for_journal(&body(),&journal,"Fixture").unwrap().remove(0);
 let mut tx=pool.begin().await.unwrap();let (paper,_)=ingest(&mut tx,&candidate).await.unwrap();
 sqlx::query("INSERT INTO paper_source_records(id,paper_id,source,source_key,raw_json) VALUES(?,?,'arxiv','2610.00001','{}')").bind(uuid::Uuid::new_v4().to_string()).bind(&paper).execute(&mut *tx).await.unwrap();tx.commit().await.unwrap();
 cancel_journal_subscription(&pool,&journal).await.unwrap();let mut tx=pool.begin().await.unwrap();assert!(match_paper(&mut tx,&paper).await.unwrap().0);tx.commit().await.unwrap();
 assert_eq!(super::super::policy::publish(&pool,"arxiv",None).await.unwrap(),1);
 let page=list_feed(&pool,PaperFeedListOptions{source:Some("arxiv".into()),..Default::default()}).await.unwrap();assert_eq!(page.items.len(),1);assert_eq!(page.items[0].paper_id,paper);
}
