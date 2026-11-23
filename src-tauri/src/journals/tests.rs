use super::*;
fn fixture_root()->std::path::PathBuf{std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("development/verification/journal-unit").join(uuid::Uuid::new_v4().to_string())}
fn input()->Value{json!({"name":"Fixture Journal","shortName":"FIX","issnPrint":"0000-0000","websiteUrl":"https://journals.example.org"})}
#[tokio::test]
async fn empty_database_persistence_removal_and_readdition(){
 let root=fixture_root();let service=Service::open(&root).await.unwrap();
 assert_eq!(service.command("list",json!({})).await.unwrap().as_array().unwrap().len(),23);
 assert_eq!(service.command("feed",json!({})).await.unwrap()["total"],0);
 let journal=service.command("save",json!({"input":input()})).await.unwrap();let id=journal["id"].as_str().unwrap();
 assert_eq!(journal["subscription"]["enabled"],true);
 assert!(service.command("save",json!({"input":input()})).await.unwrap_err().contains("已在"));
 service.pool.close().await;drop(service);
 let service=Service::open(&root).await.unwrap();assert_eq!(service.command("list",json!({})).await.unwrap().as_array().unwrap().iter().find(|j|j["id"]==id).unwrap()["id"],id);
 service.command("subscription",json!({"id":id,"enabled":false})).await.unwrap();
 assert_eq!(service.command("list",json!({})).await.unwrap().as_array().unwrap().iter().find(|j|j["id"]==id).unwrap()["subscription"]["enabled"],false);
 service.command("remove",json!({"id":id})).await.unwrap();assert_eq!(service.command("list",json!({})).await.unwrap().as_array().unwrap().len(),23);
 let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM journals").fetch_one(&service.pool).await.unwrap();assert_eq!(count,24);
 let restored=service.command("save",json!({"input":input()})).await.unwrap();assert_eq!(restored["id"],id);
 assert_eq!(sqlx::query_scalar::<_,String>("PRAGMA quick_check").fetch_one(&service.pool).await.unwrap(),"ok");
 service.pool.close().await;
}
#[tokio::test]
async fn validation_and_atomic_subscription_creation(){
 let root=fixture_root();let service=Service::open(&root).await.unwrap();
 let mut invalid=input();invalid["issnPrint"]=json!("1234-5678");assert!(service.command("save",json!({"input":invalid})).await.is_err());
 invalid=input();invalid["websiteUrl"]=json!("http://journals.example.org");assert!(service.command("save",json!({"input":invalid})).await.is_err());
 sqlx::raw_sql("CREATE TRIGGER fail_subscription BEFORE INSERT ON journal_subscriptions BEGIN SELECT RAISE(ABORT,'fixture write failure'); END;").execute(&service.pool).await.unwrap();
 assert!(service.command("save",json!({"input":input()})).await.unwrap_err().contains("fixture write failure"));
 assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM journals").fetch_one(&service.pool).await.unwrap(),23);
 assert!(service.command("not_an_operation",json!({})).await.is_err());service.pool.close().await;
}
#[tokio::test]
async fn configuration_rollback_on_matching_failure(){
 let root=fixture_root();let service=Service::open(&root).await.unwrap();
 sqlx::query("INSERT INTO papers(id,title) VALUES('fixture','Fixture science')").execute(&service.pool).await.unwrap();
 sqlx::raw_sql("CREATE TRIGGER fail_match BEFORE INSERT ON paper_matches BEGIN SELECT RAISE(ABORT,'fixture match failure'); END;").execute(&service.pool).await.unwrap();
 let result=service.command("save_topic",json!({"input":{"name":"Fixture topic","keywords":[{"phrase":"science","weight":10,"kind":"boost"},{"phrase":"quantum","weight":5,"kind":"boost"},{"phrase":"transport","weight":5,"kind":"boost"}]}})).await;
 assert!(result.unwrap_err().contains("fixture match failure"));
 assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM topics").fetch_one(&service.pool).await.unwrap(),0);service.pool.close().await;
}

#[tokio::test]
async fn profile_failure_retains_last_successful_snapshot_and_catalog_is_valid(){
 let service=Service::open(&fixture_root()).await.unwrap();
 let first="ExampleAAAAJ";
 let url=format!("https://scholar.google.com/citations?user={first}");
 assert!(service.command("profile",json!({})).await.unwrap()["url"].is_null());
 assert!(service.command("profile_update",json!({})).await.is_err());
 assert!(service.command("open_profile",json!({})).await.is_err());
 for invalid in ["https://scholar.google.com.evil.test/citations?user=ExampleAAAAJ","https://scholar.google.com/citations","https://scholar.google.com/citations?user=ExampleAAAAJ&user=AnotherAAAAJ","https://scholar.google.com/citations?user=bad%26value"] {
     assert!(service.command("profile_bind",json!({"url":invalid})).await.is_err());
 }
 let bound=service.command("profile_bind",json!({"url":url})).await.unwrap();
 assert_eq!(bound["url"],profile::bound_url(&service.pool).await.unwrap());
 let parsed=profile::parse("<div id=\"gsc_prf_in\">Fixture Scholar</div><a class=\"gsc_prf_inta\">Fixture science</a>").unwrap();
 let good="2026-10-01T00:00:00Z";
 let failed="2026-10-02T00:00:00Z";
 sqlx::query("UPDATE scholar_profile SET last_attempt_at=?").bind(good).execute(&service.pool).await.unwrap();
 profile::persist_result(&service.pool,first,Ok(parsed),good).await.unwrap();
 sqlx::query("UPDATE scholar_profile SET last_attempt_at=?").bind(failed).execute(&service.pool).await.unwrap();
 let state=profile::persist_result(&service.pool,first,Err("fixture network unavailable".into()),failed).await.unwrap();
 assert_eq!(state["snapshot"]["name"],"Fixture Scholar");assert_eq!(state["lastSuccessAt"],good);assert_eq!(state["error"],"fixture network unavailable");
 assert_eq!(state["snapshot"]["interests"],json!(["Fixture science"]));
 assert_eq!(service.command("profile_bind",json!({"url":url})).await.unwrap(),state);
 let other=service.command("profile_bind",json!({"url":"https://scholar.google.co.uk/citations?user=AnotherAAAAJ"})).await.unwrap();
 assert!(other["snapshot"].is_null()&&other["error"].is_null()&&other["lastSuccessAt"].is_null());
 assert_eq!(profile::persist_result(&service.pool,first,Err("late result".into()),failed).await.unwrap(),other);
 let reopened=Service::open(&service.root).await.unwrap();
 assert_eq!(reopened.command("profile",json!({})).await.unwrap(),other);
 reopened.pool.close().await;
 let entries=service.command("library",json!({})).await.unwrap();let entries=entries.as_array().unwrap();assert_eq!(entries.len(),23);
 let sources=service.command("list",json!({})).await.unwrap();assert_eq!(sources.as_array().unwrap().len(),23);
 assert!(sources.as_array().unwrap().iter().all(|j|j["subscription"]["enabled"]==true&&!j["series"].as_str().unwrap().is_empty()));
 service.pool.close().await;
}

#[tokio::test]
async fn upgrade_preserves_legacy_keywords_articles_and_read_history(){
 let root=fixture_root();std::fs::create_dir_all(&root).unwrap();
 let pool=SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(root.join("journals.db")).create_if_missing(true).foreign_keys(true)).await.unwrap();
 sqlx::raw_sql(include_str!("../../journal_migrations/001_journals.sql")).execute(&pool).await.unwrap();
 sqlx::raw_sql("INSERT INTO topics(id,name) VALUES('legacy','Legacy single-word theme'); INSERT INTO topic_keywords(topic_id,phrase,weight) VALUES('legacy','science',10); INSERT INTO papers(id,title) VALUES('paper','Legacy science'); INSERT INTO paper_matches(paper_id,topic_id,score,tier,reason_json) VALUES('paper','legacy',15,'high','{}'); INSERT INTO push_items(id,paper_id,topic_id,match_score,status) VALUES('push','paper','legacy',15,'read'); PRAGMA user_version=1;").execute(&pool).await.unwrap();pool.close().await;
 let service=Service::open(&root).await.unwrap();
 assert_eq!(service.command("topics",json!({})).await.unwrap()[0]["keywords"].as_array().unwrap().len(),1);
 assert_eq!(sqlx::query_scalar::<_,String>("SELECT status FROM push_items WHERE id='push'").fetch_one(&service.pool).await.unwrap(),"read");
 assert_eq!(service.command("feed",json!({})).await.unwrap()["total"],0);
 assert!(service.command("sync",json!({"source":"crossref"})).await.unwrap_err().contains("3"));
 service.pool.close().await;
}

#[tokio::test]
async fn custom_series_sources_and_avatar_survive_restart(){
 use base64::Engine;
 let root=fixture_root();let service=Service::open(&root).await.unwrap();
 let mut draft=input();draft["series"]=json!("Fixture custom series");
 let journal=service.command("save",json!({"input":draft})).await.unwrap();let id=journal["id"].as_str().unwrap().to_owned();
 assert_eq!(journal["series"],"Fixture custom series");assert_eq!(journal["subscription"]["enabled"],true);
 // A tiny valid PNG fixture; metadata stores only its generated filename.
 let data="iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg==";
 let avatar=service.command("save_avatar",json!({"data":data})).await.unwrap();assert!(avatar.as_str().unwrap().starts_with("data:image/png;base64,"));
 assert!(service.command("save_avatar",json!({"data":base64::engine::general_purpose::STANDARD.encode(b"<svg>script</svg>")})).await.is_err());
 service.pool.close().await;drop(service);
 let service=Service::open(&root).await.unwrap();assert_eq!(service.command("avatar",json!({})).await.unwrap(),avatar);
 let sources=service.command("list",json!({})).await.unwrap();let row=sources.as_array().unwrap().iter().find(|j|j["id"]==id).unwrap();assert_eq!(row["series"],"Fixture custom series");
 let stored:String=sqlx::query_scalar("SELECT avatar_file FROM scholar_profile WHERE id=1").fetch_one(&service.pool).await.unwrap();assert!(root.join(stored).is_file());service.pool.close().await;
}

#[tokio::test]
async fn journal_website_resolves_registered_https_address_without_opening_test_browser(){
 let service=Service::open(&fixture_root()).await.unwrap();
 let row=service.command("save",json!({"input":input()})).await.unwrap();let id=row["id"].as_str().unwrap();
 assert_eq!(journal_website(&service.pool,id).await.unwrap(),"https://journals.example.org/");
 assert_eq!(journal_website(&service.pool,"arxiv").await.unwrap(),"https://arxiv.org/");
 let sources=service.command("sources",json!({})).await.unwrap();let arxiv=sources.as_array().unwrap().iter().find(|r|r["id"]=="arxiv").unwrap();assert_eq!(arxiv["subscription"]["enabled"],true);assert_eq!(arxiv["websiteUrl"],"https://arxiv.org/");assert_eq!(service.command("list",json!({})).await.unwrap().as_array().unwrap().len(),24,"arXiv source does not create a fake ISSN journal");
 assert!(journal_website(&service.pool,"missing-journal").await.is_err());
 for url in ["file:///E:/fixture","javascript:alert(1)","https://user:secret@journals.example.org/"]{sqlx::query("UPDATE journals SET website_url=? WHERE id=?").bind(url).bind(id).execute(&service.pool).await.unwrap();assert!(journal_website(&service.pool,id).await.is_err());}
 service.pool.close().await;
}
