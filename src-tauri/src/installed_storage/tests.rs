use super::*;
use sqlx::Row;
fn fixture()->(PathBuf,PathBuf){let root=PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/verification").join(format!("migration-{}",uuid::Uuid::new_v4()));let source=root.join("legacy");let target=root.join("install/SciDeskData");fs::create_dir_all(source.join("database")).unwrap();fs::create_dir_all(&target).unwrap();(source,target)}
async fn db(path:&Path)->SqliteConnection{fs::create_dir_all(path.parent().unwrap()).unwrap();SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(path).create_if_missing(true)).await.unwrap()}
async fn execute(db:&mut SqliteConnection,sql:&str){sqlx::query(sql).execute(db).await.unwrap();}
#[tokio::test]
async fn migrates_live_wal_and_owned_references_without_changing_source(){
 let (source,target)=fixture();let mut main=db(&source.join("database/scidesk.db")).await;
 execute(&mut main,"PRAGMA journal_mode=WAL").await;execute(&mut main,"PRAGMA wal_autocheckpoint=0").await;
 execute(&mut main,"CREATE TABLE projects(id TEXT PRIMARY KEY,root_path TEXT,root_key TEXT)").await;
 execute(&mut main,"CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT)").await;
 fs::create_dir_all(source.join("Projects/中文项目")).unwrap();fs::write(source.join("Projects/中文项目/科研.txt"),b"original scientific contents").unwrap();
 fs::write(source.join("Projects/中文项目/paper-journal"),b"keep non-SQLite sidecar-like names").unwrap();
 let managed=display(&source.join("Projects/中文项目"));let external=display(&source.parent().unwrap().join("external"));
 for (id,path) in [("managed",&managed),("external",&external)]{sqlx::query("INSERT INTO projects VALUES(?,?,?)").bind(id).bind(path).bind(key(Path::new(path))).execute(&mut main).await.unwrap();}
 sqlx::query("INSERT INTO settings VALUES('home.background.path',?)").bind(json!(source.join("Projects/中文项目/科研.txt").to_string_lossy()).to_string()).execute(&mut main).await.unwrap();
 for (relative,table) in [("database/manuals/manuals.db","links"),("database/learning/learning.db","backups")]{let mut conn=db(&source.join(relative)).await;execute(&mut conn,&format!("CREATE TABLE {table}(id TEXT,path TEXT,path_key TEXT)")).await;sqlx::query(&format!("INSERT INTO {table} VALUES('owned',?,'old-key')")).bind(&managed).execute(&mut conn).await.unwrap();conn.close().await.unwrap();}
 let mut tools=db(&source.join("database/tools/tools.db")).await;execute(&mut tools,"CREATE TABLE tools(id TEXT,local_json TEXT)").await;sqlx::query("INSERT INTO tools VALUES('local',?)").bind(json!({"root":managed,"entry":"main.exe"}).to_string()).execute(&mut tools).await.unwrap();tools.close().await.unwrap();
 let report=migrate_legacy(&source,&target).await.unwrap();assert_eq!(report["source_retained"],true);
 let mut copied=db(&target.join("database/scidesk.db")).await;
 let rows=sqlx::query("SELECT id,root_path FROM projects ORDER BY id").fetch_all(&mut copied).await.unwrap();assert_eq!(rows.len(),2);assert_eq!(rows[0].get::<String,_>("root_path"),external);assert_eq!(key(Path::new(&rows[1].get::<String,_>("root_path"))),key(&target.join("Projects/中文项目")));
 let original:String=sqlx::query_scalar("SELECT root_path FROM projects WHERE id='managed'").fetch_one(&mut main).await.unwrap();assert_eq!(original,managed);
 for (relative,table) in [("database/manuals/manuals.db","links"),("database/learning/learning.db","backups")]{let mut conn=db(&target.join(relative)).await;let (path,k):(String,String)=sqlx::query_as(&format!("SELECT path,path_key FROM {table}")).fetch_one(&mut conn).await.unwrap();assert_eq!(key(Path::new(&path)),key(&target.join("Projects/中文项目")));assert_eq!(k,key(Path::new(&path)));conn.close().await.unwrap();}
 let mut tools=db(&target.join("database/tools/tools.db")).await;let text:String=sqlx::query_scalar("SELECT local_json FROM tools").fetch_one(&mut tools).await.unwrap();let value=serde_json::from_str::<Value>(&text).unwrap();assert_eq!(key(Path::new(value["root"].as_str().unwrap())),key(&target.join("Projects/中文项目")));tools.close().await.unwrap();
 assert_eq!(fs::read(target.join("Projects/中文项目/科研.txt")).unwrap(),b"original scientific contents");assert!(target.join("Projects/中文项目/paper-journal").is_file());
 execute(&mut copied,"UPDATE projects SET root_path='newer user edit' WHERE id='managed'").await;
 assert_eq!(migrate_legacy(&source,&target).await.unwrap()["status"],"already_migrated");let changed:String=sqlx::query_scalar("SELECT root_path FROM projects WHERE id='managed'").fetch_one(&mut copied).await.unwrap();assert_eq!(changed,"newer user edit");copied.close().await.unwrap();main.close().await.unwrap();
}
#[tokio::test]
async fn existing_destination_wins_and_partial_modules_are_preserved(){
 let (source,target)=fixture();let mut old=db(&source.join("database/scidesk.db")).await;execute(&mut old,"CREATE TABLE records(value TEXT)").await;execute(&mut old,"INSERT INTO records VALUES('old')").await;old.close().await.unwrap();
 let mut existing=db(&target.join("database/scidesk.db")).await;execute(&mut existing,"CREATE TABLE records(value TEXT)").await;execute(&mut existing,"INSERT INTO records VALUES('current')").await;existing.close().await.unwrap();fs::create_dir_all(source.join("journals")).unwrap();fs::write(source.join("journals/avatar.png"),b"avatar").unwrap();
 migrate_legacy(&source,&target).await.unwrap();let mut existing=db(&target.join("database/scidesk.db")).await;let value:String=sqlx::query_scalar("SELECT value FROM records").fetch_one(&mut existing).await.unwrap();assert_eq!(value,"current");existing.close().await.unwrap();assert_eq!(fs::read(target.join("journals/avatar.png")).unwrap(),b"avatar");assert!(source.join("journals/avatar.png").is_file());
}
#[tokio::test]
async fn unfinished_operations_abort_without_publishing(){
 let (source,target)=fixture();let old=db(&source.join("database/scidesk.db")).await;old.close().await.unwrap();fs::create_dir(source.join("database/operations")).unwrap();fs::write(source.join("database/operations/pending.json"),b"{}").unwrap();assert!(migrate_legacy(&source,&target).await.is_err());assert!(!target.join("database/scidesk.db").exists());assert!(source.join("database/scidesk.db").is_file());
}
#[tokio::test]
async fn corrupt_database_aborts_before_any_live_module_is_published(){
 let (source,target)=fixture();fs::write(source.join("database/scidesk.db"),b"SQLite format 3\0not a real database").unwrap();fs::create_dir_all(source.join("Projects")).unwrap();fs::write(source.join("Projects/retained.txt"),b"retain original").unwrap();assert!(migrate_legacy(&source,&target).await.is_err());assert!(!target.join("database/scidesk.db").exists());assert!(!target.join("Projects").exists());assert_eq!(fs::read(source.join("Projects/retained.txt")).unwrap(),b"retain original");
}
#[cfg(windows)]
#[tokio::test]
async fn junction_content_is_rejected_before_publication(){
 let (source,target)=fixture();let old=db(&source.join("database/scidesk.db")).await;old.close().await.unwrap();let original=source.parent().unwrap().join("original");fs::create_dir(&original).unwrap();fs::write(original.join("untouched.txt"),b"original").unwrap();let link=source.join("linked");let status=std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command","New-Item -ItemType Junction -Path $env:SCIDESK_MIGRATION_LINK -Target $env:SCIDESK_MIGRATION_ORIGINAL -ErrorAction Stop | Out-Null"]).env("SCIDESK_MIGRATION_LINK",&link).env("SCIDESK_MIGRATION_ORIGINAL",&original).status().unwrap();assert!(status.success());assert!(migrate_legacy(&source,&target).await.is_err());assert!(!target.join("database/scidesk.db").exists());assert_eq!(fs::read(original.join("untouched.txt")).unwrap(),b"original");fs::remove_dir(link).unwrap();
}
