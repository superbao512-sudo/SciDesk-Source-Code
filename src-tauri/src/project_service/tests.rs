use super::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

#[tokio::test]
async fn file_drop_uses_registered_directory_and_rejects_forged_drag_paths(){
 let f=Fixture::new().await;let project=f.create("drop-target").await;let incoming=f.base.join("incoming");fs::create_dir(&incoming).unwrap();fs::write(incoming.join("科研.txt"),b"unchanged source").unwrap();
 let result=f.call("drop_files",json!({"id":project.id,"relative":"","paths":[display(&incoming.join("科研.txt"))],"mode":"copy","conflict":"skip"})).await;assert_eq!(result["items"][0]["status"],"completed");assert_eq!(fs::read(Path::new(&project.root_path).join("科研.txt")).unwrap(),b"unchanged source");assert!(incoming.join("科研.txt").is_file());
 assert!(f.service.execute("drop_files",json!({"id":project.id,"relative":"../","paths":[display(&incoming.join("科研.txt"))],"mode":"move"})).await.is_err());
 assert!(f.service.execute("drag_paths",json!({"id":project.id,"relatives":["../incoming/科研.txt"]})).await.is_err());assert!(incoming.join("科研.txt").is_file());f.close().await;
}

// Every filesystem fixture is fresh and stays below this checkout's ignored development directory.
struct Fixture { base: PathBuf, db: PathBuf, service: Service }
impl Fixture {
    async fn new() -> Self {
        let base = PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/project-tests").join(token());
        let app = base.join("app");
        fs::create_dir_all(&app).unwrap();
        let db = base.join("metadata.db");
        let pool = Self::pool(&db).await;
        migrate(&pool).await.unwrap();
        Self { service: Service::new(pool, app, base.join("operations")), base, db }
    }
    async fn pool(db: &Path) -> SqlitePool {
        SqlitePoolOptions::new().max_connections(4).connect_with(
            SqliteConnectOptions::new().filename(db).create_if_missing(true).foreign_keys(true)
                .busy_timeout(std::time::Duration::from_secs(3))
        ).await.unwrap()
    }
    async fn call(&self, op: &str, args: Value) -> Value { self.service.execute(op, args).await.unwrap() }
    async fn create(&self, name: &str) -> Project {
        serde_json::from_value(self.call("register", json!({"name":name})).await).unwrap()
    }
    async fn import(&self, name: &str, path: &Path) -> Project {
        serde_json::from_value(self.call("register", json!({"name":name,"imported":true,"path":display(path)})).await).unwrap()
    }
    async fn fails(&self, op: &str, args: Value) -> String { self.service.execute(op,args).await.unwrap_err() }
    async fn close(&self) { self.service.pool.close().await; }
}

#[test]
fn registration_metadata_sort_selection_survive_restart() { tauri::async_runtime::block_on(async {
    let f = Fixture::new().await;
    let a:Project=serde_json::from_value(f.call("register",json!({"name":"中文物理项目","description":"初始研究目标"})).await).unwrap();
    assert_eq!(a.description,"初始研究目标");
    assert_eq!(Path::new(&a.root_path), f.service.default_root().join("中文物理项目"));
    assert!(Path::new(&a.root_path).is_dir());
    let imported = f.base.join("外部已有目录"); fs::create_dir(&imported).unwrap();
    fs::write(imported.join("研究数据.txt"), "原始数据不能改变").unwrap();
    let b = f.import("仅登记", &imported).await;
    assert_eq!(fs::read_to_string(imported.join("研究数据.txt")).unwrap(), "原始数据不能改变");
    f.fails("register",json!({"name":"重复","imported":true,"path":display(&imported).to_uppercase()})).await;
    f.call("update",json!({"id":a.id,"description":"中文研究目标\n分阶段验证计算结果","icon_name":"atom","project_color":"#9333ea","status":"idea","progress":76,"progress_enabled":true})).await;
    f.call("select",json!({"id":a.id})).await;
    f.call("reorder",json!({"ids":[b.id,a.id]})).await;
    f.fails("reorder",json!({"ids":[a.id,a.id]})).await;
    f.close().await;
    let reopened = Service::new(Fixture::pool(&f.db).await,f.service.app_root.clone(),f.service.journal.clone());
    migrate(&reopened.pool).await.unwrap();
    let state = reopened.execute("bootstrap",json!({})).await.unwrap();
    assert_eq!(state["selected_id"],a.id);
    assert_eq!(state["projects"][0]["id"],b.id);
    assert_eq!(state["projects"][1]["icon_name"],"atom");
    assert_eq!(state["projects"][1]["description"],"中文研究目标\n分阶段验证计算结果");
    assert_eq!(state["projects"][1]["progress"],76);
    assert_eq!(state["projects"][1]["status"],"idea");
    assert_eq!(state["projects"][1]["progress_enabled"],true);
    assert_eq!(state["projects"][1]["project_color"],"#9333ea");
    reopened.pool.close().await;
}); }

#[test]
fn v1_migration_preserves_existing_home_metadata() { tauri::async_runtime::block_on(async {
    let f=Fixture::new().await;
    let db=f.base.join("version-one.db");let pool=Fixture::pool(&db).await;
    sqlx::raw_sql(include_str!("../../migrations/001_home_foundation.sql")).execute(&pool).await.unwrap();
    sqlx::raw_sql("PRAGMA user_version=1; INSERT INTO projects(id,name,root_path) VALUES('prior','保留','C:/existing'); INSERT INTO settings(key,value) VALUES('home.message','保持首页');").execute(&pool).await.unwrap();
    migrate(&pool).await.unwrap();migrate(&pool).await.unwrap();
    let row:(String,String)=sqlx::query_as("SELECT name,icon_name FROM projects WHERE id='prior'").fetch_one(&pool).await.unwrap();
    assert_eq!(row,("保留".into(),"folder".into()));
    let message:String=sqlx::query_scalar("SELECT value FROM settings WHERE key='home.message'").fetch_one(&pool).await.unwrap();assert_eq!(message,"保持首页");
    let check:String=sqlx::query_scalar("PRAGMA quick_check").fetch_one(&pool).await.unwrap();assert_eq!(check,"ok");
    pool.close().await;f.close().await;
}); }

#[test]
fn real_entries_templates_and_refresh_preserve_existing_data() { tauri::async_runtime::block_on(async {
    let f=Fixture::new().await;let p=f.create("文件操作").await;let root=Path::new(&p.root_path);
    f.call("create_entry",json!({"id":p.id,"kind":"folder","name":"子目录"})).await;
    f.call("create_entry",json!({"id":p.id,"relative":"子目录","kind":"file","name":"测试.txt"})).await;
    fs::write(root.join("子目录/测试.txt"),"非 ASCII 内容").unwrap();
    f.call("rename_entry",json!({"id":p.id,"relative":"子目录/测试.txt","name":"新名字.txt"})).await;
    assert_eq!(fs::read_to_string(root.join("子目录/新名字.txt")).unwrap(),"非 ASCII 内容");
    f.fails("create_entry",json!({"id":p.id,"relative":"子目录","kind":"file","name":"新名字.txt"})).await;
    fs::write(root.join("外部新增.csv"),"1,2,3").unwrap();
    let listing=f.call("list_files",json!({"id":p.id})).await;
    assert!(listing["entries"].as_array().unwrap().iter().any(|e|e["name"]=="外部新增.csv"&&e["size"]==5));
    fs::create_dir(root.join("研究思路")).unwrap();fs::write(root.join("研究思路/已有.txt"),"保留").unwrap();fs::write(root.join("文献调研"),"同名文件").unwrap();
    let result=f.call("quick_create",json!({"id":p.id,"names":["研究思路","文献调研","过程文件","图表数据","文章写作","自定义目录"]})).await;
    assert_eq!(result["created"].as_array().unwrap().len(),4);assert_eq!(result["skipped"],json!(["研究思路"]));assert_eq!(result["failed"][0]["name"],"文献调研");
    assert_eq!(fs::read_to_string(root.join("研究思路/已有.txt")).unwrap(),"保留");assert_eq!(fs::read_to_string(root.join("文献调研")).unwrap(),"同名文件");
    f.fails("quick_create",json!({"id":p.id,"names":["重复","重复"]})).await;
    assert!(!root.join("重复").exists());f.close().await;
}); }

#[test]
fn names_paths_links_and_protected_directories_are_rejected() { tauri::async_runtime::block_on(async {
    for name in ["", ".", "..", "CON", "COM1.txt", "LPT²", "尾空格 ", "尾句点.", "a/b", "a\\b", "a:b", "a\u{0}b"] { assert!(validate_name(name).is_err(),"{name:?}"); }
    validate_name("中文 科研（1）.txt").unwrap();
    let f=Fixture::new().await;let p=f.create("边界").await;let root=Path::new(&p.root_path);
    for relative in ["../outside", "C:\\Windows", "\\\\server\\share", "..\\outside"] {f.fails("list_files",json!({"id":p.id,"relative":relative})).await;}
    for op in ["trash_entry","rename_entry"] {f.fails(op,json!({"id":p.id,"relative":"","name":"非法根删除","confirm_path":p.root_path})).await;}
    assert!(root.exists());
    let old=f.base.join("SciDesk");fs::create_dir(&old).unwrap();fs::write(old.join("只读.txt"),"不能触碰").unwrap();
    f.fails("register",json!({"name":"第一版","imported":true,"path":display(&old)})).await;
    f.fails("register",json!({"name":"源码","imported":true,"path":display(&f.service.app_root)})).await;
    fs::create_dir(root.join("nested")).unwrap();f.fails("register",json!({"name":"嵌套","imported":true,"path":display(&root.join("nested"))})).await;
    #[cfg(windows)] {
        // A junction is a real reparse point and does not require Developer Mode privileges.
        let link=root.join("目录联接");let target=f.base.join("outside");fs::create_dir(&target).unwrap();
        let status=std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command","New-Item -ItemType Junction -Path $env:SCIDESK_TEST_LINK -Target $env:SCIDESK_TEST_TARGET -ErrorAction Stop | Out-Null"]).env("SCIDESK_TEST_LINK",&link).env("SCIDESK_TEST_TARGET",&target).status().unwrap();
        assert!(status.success(),"junction fixture creation failed");f.fails("list_files",json!({"id":p.id,"relative":"目录联接"})).await;fs::remove_dir(&link).unwrap();
        let second=Fixture::new().await;let old=second.base.join("SciDesk");fs::create_dir(&old).unwrap();
        let status=std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command","New-Item -ItemType Junction -Path $env:SCIDESK_TEST_LINK -Target $env:SCIDESK_TEST_TARGET -ErrorAction Stop | Out-Null"]).env("SCIDESK_TEST_LINK",second.service.app_root.join("SciDeskData")).env("SCIDESK_TEST_TARGET",&old).status().unwrap();assert!(status.success());
        second.fails("register",json!({"name":"不得写入联接目标"})).await;assert!(!old.join("Projects").exists());fs::remove_dir(second.service.app_root.join("SciDeskData")).unwrap();second.close().await;
    }
    assert_eq!(fs::read_to_string(old.join("只读.txt")).unwrap(),"不能触碰");f.close().await;
}); }

#[test]
fn folder_rename_conflict_display_only_and_database_rollback() { tauri::async_runtime::block_on(async {
    let f=Fixture::new().await;let p=f.create("原目录").await;fs::write(Path::new(&p.root_path).join("保留.txt"),"文件内容").unwrap();
    let conflict=f.service.default_root().join("已有目录");fs::create_dir(&conflict).unwrap();
    f.fails("rename_project",json!({"id":p.id,"name":"已有目录"})).await;assert!(Path::new(&p.root_path).exists());
    f.call("rename_project",json!({"id":p.id,"name":"显示名称","display_only":true})).await;assert_eq!(f.service.get(&p.id).await.unwrap().root_path,p.root_path);
    sqlx::raw_sql("CREATE TRIGGER fail_rename BEFORE UPDATE OF root_path ON projects BEGIN SELECT RAISE(ABORT,'injected metadata failure'); END;").execute(&f.service.pool).await.unwrap();
    let error=f.fails("rename_project",json!({"id":p.id,"name":"应回滚"})).await;assert!(error.contains("目录已恢复原名"),"{error}");assert!(Path::new(&p.root_path).exists());assert!(!f.service.default_root().join("应回滚").exists());assert!(f.service.pending().unwrap().is_empty());
    sqlx::query("DROP TRIGGER fail_rename").execute(&f.service.pool).await.unwrap();
    f.call("rename_project",json!({"id":p.id,"name":"成功改名"})).await;
    let new=f.service.get(&p.id).await.unwrap();assert_eq!(new.name,"成功改名");assert!(!Path::new(&p.root_path).exists());assert_eq!(fs::read_to_string(Path::new(&new.root_path).join("保留.txt")).unwrap(),"文件内容");
    f.close().await;
}); }

#[test]
fn create_metadata_failure_rolls_back_only_new_empty_directory() { tauri::async_runtime::block_on(async {
    let f=Fixture::new().await;
    sqlx::raw_sql("CREATE TRIGGER fail_create BEFORE INSERT ON projects BEGIN SELECT RAISE(ABORT,'injected metadata failure'); END;").execute(&f.service.pool).await.unwrap();
    f.fails("register",json!({"name":"创建失败"})).await;assert!(!f.service.default_root().join("创建失败").exists());assert!(f.service.list().await.unwrap().is_empty());assert!(f.service.pending().unwrap().is_empty());
    let imported=f.base.join("导入保留");fs::create_dir(&imported).unwrap();fs::write(imported.join("existing.txt"),"原文件").unwrap();
    f.fails("register",json!({"name":"登记失败","imported":true,"path":display(&imported)})).await;assert_eq!(fs::read_to_string(imported.join("existing.txt")).unwrap(),"原文件");
    f.close().await;
}); }

#[test]
fn remove_registration_keeps_files_and_missing_root_can_be_relinked() { tauri::async_runtime::block_on(async {
    let f=Fixture::new().await;let p=f.create("外部移动").await;let moved=f.base.join("新位置");fs::rename(&p.root_path,&moved).unwrap();
    f.fails("list_files",json!({"id":p.id})).await;assert!(!Path::new(&p.root_path).exists());assert!(f.service.get(&p.id).await.is_ok());
    f.call("relink",json!({"id":p.id,"path":display(&moved)})).await;assert_eq!(f.call("list_files",json!({"id":p.id})).await["path"],display(&canonical(&moved).unwrap()));
    fs::write(moved.join("不能删除.txt"),"移除登记仍然存在").unwrap();
    f.call("remove",json!({"id":p.id})).await;assert!(f.service.list().await.unwrap().is_empty());assert_eq!(fs::read_to_string(moved.join("不能删除.txt")).unwrap(),"移除登记仍然存在");
    f.close().await;
}); }

#[test]
fn validation_errors_leave_metadata_unchanged() { tauri::async_runtime::block_on(async {
    let f=Fixture::new().await;let p=f.create("校验").await;
    for fields in [json!({"progress":101}),json!({"progress":-1}),json!({"progress":1.5}),json!({"progress":"50"}),json!({"status":"unknown"}),json!({"icon_name":"triangle"}),json!({"project_color":"invalid"})] {
        let mut args=fields;args["id"]=json!(p.id);f.fails("update",args).await;
    }
    assert_eq!(serde_json::to_value(f.service.get(&p.id).await.unwrap()).unwrap(),serde_json::to_value(&p).unwrap());
    f.fails("trash_project",json!({"id":p.id,"confirm_path":"different path"})).await;assert!(Path::new(&p.root_path).is_dir());
    f.close().await;
}); }

#[test]
fn pending_rename_and_create_are_explicitly_recoverable() { tauri::async_runtime::block_on(async {
    let f=Fixture::new().await;let p=f.create("恢复前").await;let new=f.service.default_root().join("恢复后");let mut desired=p.clone();desired.name="恢复后".into();desired.root_path=display(&new);
    let pending=Pending{kind:"rename".into(),project:desired.clone(),old_path:p.root_path.clone(),new_path:display(&new),fs_done:false};
    f.service.record(&pending).unwrap();project_platform::rename_no_replace(Path::new(&p.root_path),&new).unwrap();
    assert_eq!(f.call("bootstrap",json!({})).await["pending"].as_array().unwrap().len(),1);
    f.call("recover",json!({"id":p.id})).await;assert_eq!(f.service.get(&p.id).await.unwrap().root_path,display(&new));assert!(f.service.pending().unwrap().is_empty());
    desired.id=token();desired.name="创建待登记".into();desired.root_path=display(&f.service.default_root().join("创建待登记"));
    let pending=Pending{kind:"create".into(),project:desired.clone(),old_path:String::new(),new_path:desired.root_path.clone(),fs_done:false};f.service.record(&pending).unwrap();fs::create_dir(&desired.root_path).unwrap();
    f.call("recover",json!({"id":desired.id})).await;assert_eq!(f.service.get(&desired.id).await.unwrap().root_path,desired.root_path);assert!(f.service.pending().unwrap().is_empty());
    f.close().await;
}); }

#[cfg(windows)]
#[test]
fn windows_recycle_file_subfolder_project_and_metadata_failure_recovery() { tauri::async_runtime::block_on(async {
    let f=Fixture::new().await;let p=f.create("仅供回收站测试").await;let root=Path::new(&p.root_path);
    fs::write(root.join("待回收.txt"),"isolated test fixture").unwrap();fs::create_dir(root.join("待回收子目录")).unwrap();fs::write(root.join("待回收子目录/fixture.txt"),"isolated test fixture").unwrap();
    { use std::os::windows::fs::OpenOptionsExt;let held=fs::OpenOptions::new().read(true).share_mode(0).open(root.join("待回收.txt")).unwrap();
        f.fails("rename_entry",json!({"id":p.id,"relative":"待回收.txt","name":"不能改名.txt"})).await;
        f.fails("trash_entry",json!({"id":p.id,"relative":"待回收.txt","confirm_path":display(&root.join("待回收.txt"))})).await;
        assert!(root.join("待回收.txt").is_file());assert!(f.service.get(&p.id).await.is_ok());drop(held);}
    for name in ["待回收.txt","待回收子目录"] {f.call("trash_entry",json!({"id":p.id,"relative":name,"confirm_path":display(&root.join(name))})).await;assert!(!root.join(name).exists());assert!(f.service.get(&p.id).await.is_ok());}
    sqlx::raw_sql("CREATE TRIGGER fail_trash_metadata BEFORE DELETE ON projects BEGIN SELECT RAISE(ABORT,'injected metadata failure'); END;").execute(&f.service.pool).await.unwrap();
    let error=f.fails("trash_project",json!({"id":p.id,"confirm_path":p.root_path})).await;assert!(error.contains("目录已进入回收站"),"{error}");assert!(!root.exists());assert!(f.service.get(&p.id).await.is_ok());
    let pending=f.service.pending().unwrap();assert_eq!(pending.len(),1);assert!(pending[0].fs_done);
    sqlx::query("DROP TRIGGER fail_trash_metadata").execute(&f.service.pool).await.unwrap();f.call("recover",json!({"id":p.id})).await;assert!(f.service.get(&p.id).await.is_err());assert!(f.service.pending().unwrap().is_empty());
    let missing=f.create("回收失败保留登记").await;fs::rename(&missing.root_path,f.base.join("外部移动目录")).unwrap();f.fails("trash_project",json!({"id":missing.id,"confirm_path":missing.root_path})).await;assert!(f.service.get(&missing.id).await.is_ok());
    f.close().await;
}); }

#[test]
fn descriptions_preserve_partial_updates_and_older_recovery_records() { tauri::async_runtime::block_on(async {
    let f=Fixture::new().await;let p=f.create("描述测试").await;
    f.call("update",json!({"id":p.id,"description":"研究目标\n计算方法"})).await;
    f.call("update",json!({"id":p.id,"icon_name":"atom"})).await;
    assert_eq!(f.service.get(&p.id).await.unwrap().description,"研究目标\n计算方法");
    for value in [json!("a".repeat(2001)),json!("invalid\0"),json!(123)] {
        f.fails("update",json!({"id":p.id,"description":value})).await;
        assert_eq!(f.service.get(&p.id).await.unwrap().description,"研究目标\n计算方法");
    }
    let mut old_project=serde_json::to_value(&p).unwrap();old_project.as_object_mut().unwrap().remove("description");
    let old_pending=json!({"kind":"rename","project":old_project,"old_path":p.root_path,"new_path":"unused","fs_done":false});
    let parsed:Pending=serde_json::from_value(old_pending).unwrap();assert_eq!(parsed.project.description,"");
    f.call("update",json!({"id":p.id,"description":""})).await;assert_eq!(f.service.get(&p.id).await.unwrap().description,"");
    f.close().await;
}); }

#[test]
fn project_literature_exports_metadata_without_overwriting_or_escaping() { tauri::async_runtime::block_on(async {
 let f=Fixture::new().await;let p=f.create("Literature fixture").await;
 let base=json!({"id":p.id,"relative":"","filename":"results.md","format":"md","query":{"keywords":["chiral phonons"],"fromYear":2020,"toYear":2026,"limit":10,"sourceNames":["Fixture Journal"],"matchLogic":"any"},"warnings":[],"records":[{"title":"Fixture <science>","authors":["Fixture Author"],"journal":"Fixture Journal","publishedAt":"2025-10-01","year":2025,"doi":"10.5555/fixture","arxivId":null,"url":"https://doi.org/10.5555/fixture","source":"crossref","matchedKeywords":["chiral phonons"]}]});
 f.call("literature_prepare",base.clone()).await;assert!(!Path::new(&p.root_path).join("results.md").exists());
 let result=f.call("literature_write",base.clone()).await;assert_eq!(result["count"],1);let path=Path::new(&p.root_path).join("results.md");let text=fs::read_to_string(&path).unwrap();assert!(text.contains("Fixture &lt;science&gt;"));assert!(text.contains("https://doi.org/10.5555/fixture"));
 assert!(f.fails("literature_write",base.clone()).await.contains("同名"));assert_eq!(fs::read_to_string(path).unwrap(),text);
 let mut outside=base.clone();outside["filename"]=json!("other.md");outside["relative"]=json!("../escape");assert!(f.service.execute("literature_write",outside).await.is_err());
 let mut json_request=base.clone();json_request["filename"]=json!("results.json");json_request["format"]=json!("json");f.call("literature_write",json_request).await;let stored:Value=serde_json::from_slice(&fs::read(Path::new(&p.root_path).join("results.json")).unwrap()).unwrap();assert_eq!(stored["records"][0]["doi"],"10.5555/fixture");assert_eq!(stored["count"],1);
 let mut invalid=base.clone();invalid["filename"]=json!("bad.md");invalid["records"][0]["url"]=json!("javascript:alert(1)");assert!(f.service.execute("literature_write",invalid).await.is_err());assert!(!Path::new(&p.root_path).join("bad.md").exists());
 assert_eq!(sqlx::query_scalar::<_,i64>("PRAGMA user_version").fetch_one(&f.service.pool).await.unwrap(),2);assert!(!Path::new(&p.root_path).join("journals.db").exists());f.close().await;
});}

#[cfg(debug_assertions)]
#[test]
fn verification_projects_require_an_explicit_independent_checkout_subdirectory() { tauri::async_runtime::block_on(async {
 let f=Fixture::new().await;let boundary=f.service.app_root.join("development/verification");let valid=boundary.join("isolated");fs::create_dir_all(&valid).unwrap();
 let make=||Service::new(f.service.pool.clone(),f.service.app_root.clone(),f.service.journal.clone());
 assert_eq!(make().default_root(),f.service.app_root.join("SciDeskData/Projects"));
 for invalid in [&f.base,&boundary,&boundary.join("missing")] {assert!(make().for_verification(invalid).is_err());}
 let service=make().for_verification(&valid).unwrap();assert_eq!(service.default_root(),canonical(&valid).unwrap().join("Projects"));
 let project:Project=serde_json::from_value(service.execute("register",json!({"name":"isolated project"})).await.unwrap()).unwrap();
 assert!(Path::new(&project.root_path).is_dir());assert!(within(Path::new(&project.root_path),&canonical(&valid).unwrap()));
 assert!(!f.service.app_root.join("SciDeskData").exists());
 let old=f.base.join("SciDesk");fs::create_dir(&old).unwrap();assert!(service.protect(&canonical(&old).unwrap()).is_err());
 assert!(service.protect(&canonical(&f.service.app_root).unwrap()).is_err());f.close().await;
});}
