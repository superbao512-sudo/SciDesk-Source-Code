//! One journal service shared by native Tauri and the localhost development bridge.
mod engine;
pub(crate) use engine::search::Record as ProjectPaper;
mod library;
mod avatar;
mod policy;
mod profile;
mod recommendations;
use serde_json::{json,Value};
use sqlx::{SqlitePool,sqlite::{SqliteConnectOptions,SqlitePoolOptions}};
use std::{path::Path,sync::Arc};
use tokio::sync::Mutex;

pub struct Service { root:std::path::PathBuf,pool:SqlitePool, writing:Mutex<()> }
pub struct JournalState(pub Result<Arc<Service>,String>);

impl Service {
    pub async fn open(root:&Path)->Result<Arc<Self>,String>{
        std::fs::create_dir_all(root).map_err(|e|e.to_string())?;
        let pool=SqlitePoolOptions::new().max_connections(4).connect_with(SqliteConnectOptions::new()
            .filename(root.join("journals.db")).create_if_missing(true).foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal).busy_timeout(std::time::Duration::from_secs(10)))
            .await.map_err(|e|e.to_string())?;
        let mut tx=pool.begin().await.map_err(|e|e.to_string())?;
        let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&mut *tx).await.map_err(|e|e.to_string())?;
        if version==0 {
            sqlx::raw_sql(include_str!("../../journal_migrations/001_journals.sql")).execute(&mut *tx).await.map_err(|e|e.to_string())?;
            sqlx::query("PRAGMA user_version=1").execute(&mut *tx).await.map_err(|e|e.to_string())?;
        } else if version>5 {return Err("期刊数据库版本不受支持，请保留数据并检查程序版本".into())}
        if version<2 {sqlx::raw_sql(include_str!("../../journal_migrations/002_profile_batches.sql")).execute(&mut *tx).await.map_err(|e|e.to_string())?;sqlx::query("PRAGMA user_version=2").execute(&mut *tx).await.map_err(|e|e.to_string())?;}
        if version<3 {sqlx::raw_sql(include_str!("../../journal_migrations/003_library_avatar.sql")).execute(&mut *tx).await.map_err(|e|e.to_string())?;sqlx::query("PRAGMA user_version=3").execute(&mut *tx).await.map_err(|e|e.to_string())?;}
        if version<4 {
            sqlx::raw_sql(include_str!("../../journal_migrations/004_scholar_binding.sql")).execute(&mut *tx).await.map_err(|e|e.to_string())?;
            sqlx::query("PRAGMA user_version=4").execute(&mut *tx).await.map_err(|e|e.to_string())?;
        }
        if version<5 {
            sqlx::raw_sql(include_str!("../../journal_migrations/005_recommendations.sql")).execute(&mut *tx).await.map_err(|e|e.to_string())?;
            sqlx::query("PRAGMA user_version=5").execute(&mut *tx).await.map_err(|e|e.to_string())?;
        }
        tx.commit().await.map_err(|e|e.to_string())?;
        engine::recover_interrupted_syncs(&pool).await?;
        if version<2 {policy::recompute(&pool).await?;}
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS journal_library_state(id INTEGER PRIMARY KEY CHECK(id=1),initialized INTEGER NOT NULL DEFAULT 0); INSERT OR IGNORE INTO journal_library_state(id) VALUES(1);").execute(&pool).await.map_err(|e|e.to_string())?;
        let initialized:i64=sqlx::query_scalar("SELECT initialized FROM journal_library_state WHERE id=1").fetch_one(&pool).await.map_err(|e|e.to_string())?;
        if initialized==0 {library::initialize(&pool).await?;sqlx::query("UPDATE journal_library_state SET initialized=1 WHERE id=1").execute(&pool).await.map_err(|e|e.to_string())?;}
        Ok(Arc::new(Self{root:root.to_path_buf(),pool,writing:Mutex::new(())}))
    }

    pub async fn command(&self,op:&str,args:Value)->Result<Value,String>{
        // Reads stay available during a network refresh. Mutations cannot race a refresh.
        let _guard=if matches!(op,"list"|"feed"|"catalog"|"topics"|"open"|"profile"|"library"|"policy"|"open_profile"|"avatar"|"open_journal"|"sources"|"project_search"|"recommendations"|"refresh_recommendations"|"keyword_search"){None}else{Some(self.writing.try_lock().map_err(|_|"期刊操作正在进行，请稍后重试".to_owned())?)};
        let id=||args["id"].as_str().filter(|s|!s.is_empty()).ok_or_else(||"缺少期刊或主题 ID".to_owned());
        match op {
            "recommendations"=>value(recommendations::cached(&self.pool).await?),
            "refresh_recommendations"=>value(recommendations::refresh(&self.pool,args["force"].as_bool().unwrap_or(false)).await?),
            "keyword_search"=>{
                let input:Vec<String>=serde_json::from_value(args["keywords"].clone()).map_err(|_|"请输入关键词")?;
                let terms=engine::discovery::terms(&input)?;
                let days=args["days"].as_i64().unwrap_or(365);
                if !matches!(days,30|365|1825){return Err("检索时间范围无效".into())}
                let mut result=engine::discovery::lookup(&terms,days).await?;
                result.items.truncate(50);
                value(result)
            },
            "project_search"=>engine::search::page(&self.pool,args).await,
            "sources"=>source_statuses(&self.pool).await,
            "open_journal"=>{let url=journal_website(&self.pool,id()?).await?;crate::project_platform::open(std::path::Path::new(&url))?;Ok(Value::Null)},
            "open_profile"=>{
                let url=profile::bound_url(&self.pool).await?;
                crate::project_platform::open(Path::new(&url))?;
                Ok(Value::Null)
            },
            "profile_bind"=>profile::bind(&self.pool,args["url"].as_str().unwrap_or("")).await,
            "avatar"=>avatar::get(&self.pool,&self.root).await,
            "save_avatar"=>avatar::save(&self.pool,&self.root,args["data"].as_str().unwrap_or("")).await,
            "profile"=>profile::get(&self.pool).await,
            "profile_import_html"=>profile::import_saved(&self.pool,&args).await,
            "profile_update"=>profile::update(&self.pool).await,
            "policy"=>policy::info(&self.pool).await,
            "library"=>Ok(serde_json::from_str(include_str!("../../journal_catalog.json")).map_err(|e|e.to_string())?),
            "open"=>{open_article(args["url"].as_str().unwrap_or(""))?;Ok(Value::Null)},
            "list"=>value(engine::list_journals(&self.pool).await?),
            "catalog"=>value(engine::query_catalog(&self.pool).await?),
            "feed"=>value(engine::list_feed(&self.pool,serde_json::from_value(args).map_err(|e|e.to_string())?).await?),
            "topics"=>{
                let ids:Vec<String>=sqlx::query_scalar("SELECT id FROM topics ORDER BY enabled DESC,name COLLATE NOCASE").fetch_all(&self.pool).await.map_err(|e|e.to_string())?;
                let mut rows=Vec::new();for id in ids {rows.push(engine::read_topic(&self.pool,&id).await?)} value(rows)
            },
            "save"=>{
                let input=serde_json::from_value(args["input"].clone()).map_err(|e|e.to_string())?;
                let existing=args["id"].as_str();
                let journal=engine::save_journal(&self.pool,existing,input).await?;
                // A saved journal belongs to My Journals through its subscription relationship.
                if existing.is_none()||!journal.subscription.as_ref().is_some_and(|s|s.enabled&&s.sync_enabled){value(engine::set_journal_subscription(&self.pool,&journal.id,true).await?)}else{value(journal)}
            },
            "subscription"=>value(engine::set_journal_subscription(&self.pool,id()?,args["enabled"].as_bool().ok_or("订阅状态无效")?).await?),
            "remove"=>{
                // Remove the user's subscription; retain canonical metadata, articles and reading history.
                value(engine::cancel_journal_subscription(&self.pool,id()?).await?)
            },
            "journal_topics"=>{
                let ids=serde_json::from_value(args["topicIds"].clone()).map_err(|e|e.to_string())?;
                let journal=engine::set_journal_topics(&self.pool,id()?,ids).await?;
                value(journal)
            },
            "save_topic"=>value(engine::save_topic(&self.pool,args["id"].as_str(),serde_json::from_value(args["input"].clone()).map_err(|e|e.to_string())?).await?),
            "remove_topic"=>{
                let mut tx=self.pool.begin().await.map_err(|e|e.to_string())?;
                sqlx::query("DELETE FROM topics WHERE id=?").bind(id()?).execute(&mut *tx).await.map_err(|e|e.to_string())?;
                tx.commit().await.map_err(|e|e.to_string())?;Ok(Value::Null)
            },
            "status"=>{set_status(&self.pool,id()?,args["status"].as_str().unwrap_or("")).await?;Ok(Value::Null)},
            "sync"=>{
                policy::ensure_topics(&self.pool).await?;
                let source=args["source"].as_str().unwrap_or("crossref");let journal=args["id"].as_str();
                let mut run=if let Some(id)=journal{engine::sync_one_journal(&self.pool,id).await?}else{engine::sync_sources(&self.pool,source).await?};
                if matches!(run.status.as_str(),"success"|"partial_success"){run.pushed=policy::publish(&self.pool,source,journal).await?;
                 sqlx::query("UPDATE journal_subscriptions SET last_pushed_count=(SELECT COUNT(*) FROM journal_feed_batch b JOIN papers p ON p.id=b.paper_id WHERE p.journal_id=journal_subscriptions.journal_id) WHERE last_attempt_at>=?").bind(&run.started_at).execute(&self.pool).await.map_err(|e|e.to_string())?;
                 sqlx::query("UPDATE sync_state SET last_pushed_count=(SELECT COUNT(*) FROM journal_feed_batch b WHERE EXISTS(SELECT 1 FROM paper_source_records ps WHERE ps.paper_id=b.paper_id AND ps.source=sync_state.source)) WHERE last_attempt_at>=?").bind(&run.started_at).execute(&self.pool).await.map_err(|e|e.to_string())?;
                }
                value(run)
            },
            _=>Err("不支持的期刊操作".into())
        }
    }
}
fn value(v:impl serde::Serialize)->Result<Value,String>{serde_json::to_value(v).map_err(|e|e.to_string())}
async fn set_status(pool:&SqlitePool,id:&str,status:&str)->Result<(),String>{
    if !matches!(status,"unread"|"read"|"dismissed"){return Err("阅读状态无效".into())}
    if sqlx::query("UPDATE push_items SET status=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(status).bind(id).execute(pool).await.map_err(|e|e.to_string())?.rows_affected()==0{return Err("推送记录不存在".into())}Ok(())
}
pub fn response(result:Result<Value,String>)->Value{match result{Ok(value)=>json!({"ok":true,"value":value}),Err(error)=>json!({"ok":false,"error":error})}}

#[cfg(test)]
mod tests;

fn open_article(url:&str)->Result<(),String>{
 let parsed=url::Url::parse(url).map_err(|_|"原文链接无效")?;
 if parsed.scheme()!="https"||!parsed.username().is_empty()||parsed.password().is_some()||parsed.port().is_some()||!matches!(parsed.host_str(),Some("doi.org"|"dx.doi.org"|"arxiv.org"|"export.arxiv.org")){return Err("只允许打开可信的 HTTPS 原文链接".into())}
 #[cfg(windows)]{use std::os::windows::process::CommandExt;std::process::Command::new("rundll32.exe").args(["url.dll,FileProtocolHandler",url]).creation_flags(0x08000000).spawn().map_err(|e|e.to_string())?;}
 #[cfg(not(windows))]return Err("当前平台未配置原文打开方式".into());
 #[cfg(windows)]Ok(())
}

async fn journal_website(pool:&SqlitePool,id:&str)->Result<String,String>{
 if id=="arxiv"{let exists:i64=sqlx::query_scalar("SELECT COUNT(*) FROM source_registry WHERE id='arxiv' AND kind='preprint'").fetch_one(pool).await.map_err(|e|e.to_string())?;if exists==1{return Ok("https://arxiv.org/".into())}}
 let journal=engine::read_journal(pool,id).await?;
 let website=journal.website_url.ok_or("此期刊未设置官网地址")?;
 let parsed=url::Url::parse(&website).map_err(|_|"期刊官网地址无效")?;
 if parsed.scheme()!="https"||parsed.host_str().is_none()||!parsed.username().is_empty()||parsed.password().is_some(){return Err("期刊官网必须是有效的 HTTPS 地址".into())}
 Ok(parsed.to_string())
}

async fn source_statuses(pool:&SqlitePool)->Result<Value,String>{
 let rows:Vec<(String,String,i64,String,Option<String>,Option<String>,Option<String>,i64,i64,i64)>=sqlx::query_as("SELECT r.id,r.display_name,r.enabled,COALESCE(s.status,'never'),s.last_success_at,s.last_attempt_at,s.last_error_message,COALESCE(s.last_inserted_count,0),COALESCE(s.last_matched_count,0),COALESCE(s.last_pushed_count,0) FROM source_registry r LEFT JOIN sync_state s ON s.source=r.id ORDER BY r.id").fetch_all(pool).await.map_err(|e|e.to_string())?;
 Ok(Value::Array(rows.into_iter().map(|(id,name,enabled,status,success,attempt,error,inserted,matched,pushed)|json!({"websiteUrl":if id=="arxiv"{"https://arxiv.org/"}else{"https://www.crossref.org/"},"subscription":{"id":id,"enabled":enabled!=0,"syncEnabled":enabled!=0,"lastStatus":status,"lastSuccessAt":success,"lastAttemptAt":attempt,"lastError":error,"lastInsertedCount":inserted,"lastMatchedCount":matched,"lastPushedCount":pushed},"id":id,"name":name})).collect()))
}
