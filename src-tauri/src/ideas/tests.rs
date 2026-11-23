use super::store::*;
use serde_json::json;
use base64::{engine::general_purpose::STANDARD,Engine};

async fn isolated() -> Store {
    let root=std::path::PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/verification/ideas-tests").join(random_id());
    std::fs::create_dir_all(&root).unwrap();
    let pool=sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
    sqlx::raw_sql("CREATE TABLE projects(id TEXT,name TEXT,deleted_at TEXT,sort_order INTEGER)").execute(&pool).await.unwrap();
    Store::open(root,pool).await.unwrap()
}
fn mutation(id:&str,version:i64,patch:serde_json::Value)->serde_json::Value{json!({"op_id":random_id(),"id":id,"base_version":version,"patch":patch})}

#[tokio::test]
async fn persistence_idempotency_and_incremental_cursor(){
    let store=isolated().await;let id=random_id();let op=mutation(&id,0,json!({"title":"","content":"科研草稿\n第二段"}));
    let saved=store.mutate(op.clone()).await.unwrap();assert_eq!(store.mutate(op).await.unwrap(),saved);
    let snapshot=store.snapshot(None).await.unwrap();assert_eq!(snapshot["records"].as_array().unwrap().len(),1);
    let cursor=snapshot["cursor"].as_i64().unwrap();assert!(store.snapshot(Some(cursor)).await.unwrap()["records"].as_array().unwrap().is_empty());
    let reopened=Store::open(store.root.clone(),store.project_pool.clone()).await.unwrap();assert_eq!(reopened.record(&id).await.unwrap()["content"],"科研草稿\n第二段");
}
#[tokio::test]
async fn content_layout_merge_and_same_field_conflict(){
    let store=isolated().await;let id=random_id();store.mutate(mutation(&id,0,json!({"content":"初稿"}))).await.unwrap();
    store.mutate(mutation(&id,1,json!({"content":"手机编辑"}))).await.unwrap();
    let moved=store.mutate(mutation(&id,1,json!({"canvas_x":400,"width":500}))).await.unwrap();assert_eq!(moved["content"],"手机编辑");assert_eq!(moved["canvas_x"],400);
    let conflict=store.mutate(mutation(&id,1,json!({"content":"电脑旧草稿"}))).await.unwrap_err();assert_eq!(conflict["code"],"conflict");assert_eq!(conflict["attempted"]["content"],"电脑旧草稿");assert_eq!(store.record(&id).await.unwrap()["content"],"手机编辑");
    let n:i64=sqlx::query_scalar("SELECT count(*) FROM idea_conflicts").fetch_one(&store.pool).await.unwrap();assert_eq!(n,1);
}
#[tokio::test]
async fn deletion_edit_conflict_restore_and_stale_undo(){
    let store=isolated().await;let id=random_id();store.mutate(mutation(&id,0,json!({"content":"保留我"}))).await.unwrap();
    store.mutate(mutation(&id,1,json!({"deleted":true}))).await.unwrap();
    assert_eq!(store.mutate(mutation(&id,1,json!({"content":"离线编辑"}))).await.unwrap_err()["code"],"conflict");
    let restored=store.mutate(mutation(&id,2,json!({"deleted":false}))).await.unwrap();assert!(restored["deleted_at"].is_null());assert_eq!(restored["content"],"保留我");
    assert!(store.mutate(mutation(&id,1,json!({"deleted":true}))).await.is_err());
}
#[tokio::test]
async fn attachments_are_real_copies_checked_and_not_arbitrary_paths(){
    let store=isolated().await;let id=random_id();store.mutate(mutation(&id,0,json!({"type":"image"}))).await.unwrap();let aid=random_id();
    let bytes=b"\x89PNG\r\n\x1a\nfile-copy-fixture";let upload=json!({"id":aid,"idea_id":id,"name":"科研图.png","image":true,"base64":STANDARD.encode(bytes)});
    store.upload(upload.clone()).await.unwrap();store.upload(upload).await.unwrap();assert_eq!(STANDARD.decode(store.attachment(&aid).await.unwrap()["base64"].as_str().unwrap()).unwrap(),bytes);
    assert!(store.attachment("../../ideas.db").await.is_err());
    assert!(store.upload(json!({"id":random_id(),"idea_id":id,"name":"../escape.png","base64":"AA=="})).await.is_err());
    store.remove_attachment(&aid).await.unwrap();assert!(store.attachment(&aid).await.is_err());assert!(store.root.join("attachments").join(format!("{aid}.bin")).exists());
}
#[tokio::test]
async fn canvas_cas_optional_title_project_reference_and_validation(){
    let store=isolated().await;let id=random_id();store.mutate(mutation(&id,0,json!({"title":"","project_id":"removed-project","tags":["声子"]}))).await.unwrap();
    assert_eq!(store.record(&id).await.unwrap()["project_id"],"removed-project");assert!(store.projects().await.unwrap().as_array().unwrap().is_empty());
    store.canvas(json!({"x":25,"y":-10,"zoom":0.7,"base_version":0})).await.unwrap();assert_eq!(store.canvas(json!({"x":0,"y":0,"zoom":1,"base_version":0})).await.unwrap_err()["code"],"conflict");
    assert!(store.mutate(mutation(&id,1,json!({"url":"javascript:alert(1)"}))).await.is_err());assert!(store.mutate(mutation(&id,1,json!({"width":-1}))).await.is_err());
}

fn object(kind:&str,id:&str,version:i64,patch:serde_json::Value)->serde_json::Value{let mut a=mutation(id,version,patch);a["entity"]=json!(kind);a}
#[tokio::test]
async fn rooms_have_independent_views_and_default_delete_retains_ideas(){
 let s=isolated().await;let a=random_id();let b=random_id();let n=random_id();
 for id in [&a,&b]{s.mutate(object("room",id,0,json!({"name":"主题"}))).await.unwrap();}
 s.mutate(mutation(&n,0,json!({"content":"保留思路","room_id":a}))).await.unwrap();
 s.canvas(json!({"scope_id":a,"x":410,"y":50,"zoom":0.6,"base_version":0})).await.unwrap();
 assert_eq!(s.canvas_state(&b).await.unwrap()["x"],0);assert_eq!(s.canvas_state("hall").await.unwrap()["x"],0);
 let mut remove=object("room",&a,1,json!({"deleted":true}));remove["members"]=json!([{"id":n,"version":1}]);s.mutate(remove).await.unwrap();
 let r=s.record(&n).await.unwrap();assert!(r["room_id"].is_null());assert!(r["deleted_at"].is_null());assert_eq!(r["content"],"保留思路");
 let reopened=Store::open(s.root.clone(),s.project_pool.clone()).await.unwrap();assert_eq!(reopened.canvas_state(&a).await.unwrap()["x"],410.0);
}
#[tokio::test]
async fn relations_deduplicate_preserve_identity_and_suspend_with_deleted_endpoints(){
 let s=isolated().await;let a=random_id();let b=random_id();let room=random_id();let edge=random_id();
 s.mutate(object("room",&room,0,json!({"name":"跨包间"}))).await.unwrap();
 for id in [&a,&b]{s.mutate(mutation(id,0,json!({"content":"节点"}))).await.unwrap();}
 let op=object("edge",&edge,0,json!({"source_idea_id":a,"target_idea_id":b}));let saved=s.mutate(op.clone()).await.unwrap();assert_eq!(s.mutate(op).await.unwrap(),saved);
 assert_eq!(s.mutate(object("edge",&random_id(),0,json!({"source_idea_id":b,"target_idea_id":a}))).await.unwrap_err()["code"],"duplicate");
 assert!(s.mutate(object("edge",&random_id(),0,json!({"source_idea_id":a,"target_idea_id":a}))).await.is_err());
 assert!(s.mutate(object("edge",&random_id(),0,json!({"source_idea_id":a,"target_idea_id":random_id()}))).await.is_err());
 s.mutate(mutation(&a,1,json!({"room_id":room}))).await.unwrap();assert_eq!(s.record(&edge).await.unwrap()["version"],1);
 s.mutate(mutation(&a,2,json!({"deleted":true}))).await.unwrap();assert!(s.record(&edge).await.unwrap()["deleted_at"].is_null()); // retained, renderer suppresses inactive endpoints
 assert!(s.mutate(object("edge",&edge,1,json!({"label":"不能编辑失效端点"}))).await.is_err());
 s.mutate(mutation(&a,3,json!({"deleted":false}))).await.unwrap();s.mutate(object("edge",&edge,1,json!({"relation_type":"支持"}))).await.unwrap();
 s.mutate(object("edge",&edge,2,json!({"deleted":true}))).await.unwrap();assert!(s.record(&b).await.unwrap()["deleted_at"].is_null());
 assert_eq!(s.mutate(object("edge",&edge,2,json!({"label":"离线旧编辑"}))).await.unwrap_err()["code"],"conflict");
 s.mutate(object("edge",&edge,3,json!({"deleted":false}))).await.unwrap();
 assert_eq!(Store::open(s.root.clone(),s.project_pool.clone()).await.unwrap().record(&edge).await.unwrap()["relation_type"],"支持");
}
#[tokio::test]
async fn room_deletion_is_atomic_and_rejects_stale_member_confirmation(){
 let s=isolated().await;let room=random_id();let id=random_id();
 s.mutate(object("room",&room,0,json!({"name":"主题"}))).await.unwrap();s.mutate(mutation(&id,0,json!({"room_id":room}))).await.unwrap();
 let op=object("room",&room,1,json!({"deleted":true}));assert_eq!(s.mutate(op.clone()).await.unwrap_err()["code"],"room_changed");
 assert!(s.record(&room).await.unwrap()["deleted_at"].is_null());assert_eq!(s.record(&id).await.unwrap()["room_id"],room);
 let mut op=op;op["members"]=json!([{"id":id,"version":1}]);op["delete_mode"]=json!("delete");s.mutate(op).await.unwrap();
 assert!(!s.record(&id).await.unwrap()["deleted_at"].is_null());assert!(s.record(&id).await.unwrap()["room_id"].is_null());
}
#[tokio::test]
async fn v1_database_migration_retains_records_canvas_and_identity(){
 let s=isolated().await;let root=s.root.join("migration-v1");std::fs::create_dir_all(&root).unwrap();
 let p=sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(root.join("ideas.db")).create_if_missing(true)).await.unwrap();
 sqlx::raw_sql(include_str!("../../ideas_migrations/001_canvas.sql")).execute(&p).await.unwrap();let id=random_id();
 sqlx::query("INSERT INTO idea_records VALUES(?,?,?,?)").bind(&id).bind(json!({"id":id,"content":"已有内容","version":7,"deleted_at":null}).to_string()).bind(7).bind("{}").execute(&p).await.unwrap();
 sqlx::query("INSERT INTO idea_settings VALUES('identity','\"existing-space\"')").execute(&p).await.unwrap();p.close().await;
 let migrated=Store::open(root,s.project_pool.clone()).await.unwrap();let r=migrated.record(&id).await.unwrap();assert_eq!(r["content"],"已有内容");assert_eq!(r["version"],7);assert_eq!(r["_entity"],"idea");assert!(r["room_id"].is_null());assert_eq!(migrated.setting("identity").await.unwrap(),"existing-space");
}
#[tokio::test]
async fn edge_anchors_survive_canonical_order_and_style_changes_without_moving_endpoints(){
 let s=isolated().await;let a="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";let b="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";let id=random_id();
 for n in [a,b]{s.mutate(mutation(n,0,json!({"content":"节点"}))).await.unwrap();}
 let r=s.mutate(object("edge",&id,0,json!({"source_idea_id":b,"target_idea_id":a,"source_anchor":"bottom","target_anchor":"top","line_style":"dashdot","color":"#16a085"}))).await.unwrap();
 assert_eq!(r["source_idea_id"],a);assert_eq!(r["source_anchor"],"top");assert_eq!(r["target_anchor"],"bottom");
 let r=s.mutate(object("edge",&id,1,json!({"line_style":"dashed","color":"#9333ea"}))).await.unwrap();assert_eq!(r["source_anchor"],"top");assert_eq!(r["target_anchor"],"bottom");
 assert!(s.mutate(object("edge",&id,2,json!({"color":"url(evil)"}))).await.is_err());assert!(s.mutate(object("edge",&id,2,json!({"source_anchor":"center"}))).await.is_err());
 let r=Store::open(s.root.clone(),s.project_pool.clone()).await.unwrap().record(&id).await.unwrap();assert_eq!(r["line_style"],"dashed");assert_eq!(r["color"],"#9333ea");assert_eq!(s.record(a).await.unwrap()["version"],1);
}
