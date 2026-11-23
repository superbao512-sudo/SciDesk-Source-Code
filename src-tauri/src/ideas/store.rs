use std::{collections::BTreeMap, fs, io::Write, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use tokio::sync::Mutex;

pub const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
pub type Result<T> = std::result::Result<T, Value>;
pub fn error(code: &str, message: impl ToString) -> Value { json!({"code":code,"message":message.to_string()}) }
pub fn db_error(e: impl std::fmt::Display) -> Value { error("storage", e.to_string()) }
pub fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64 }
pub fn random_id() -> String { rand::random::<[u8; 16]>().iter().map(|b|format!("{b:02x}")).collect() }
pub fn secret() -> String { rand::random::<[u8; 32]>().iter().map(|b|format!("{b:02x}")).collect() }
pub fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
pub fn valid_id(id: &str) -> bool { (16..=80).contains(&id.len()) && id.bytes().all(|b|b.is_ascii_alphanumeric() || b==b'-' || b==b'_') }
pub fn string(v: &Value, key: &str) -> Result<String> { v[key].as_str().map(str::to_owned).ok_or_else(||error("invalid",format!("缺少文本参数 {key}"))) }
pub fn safe_url(url: &str) -> bool { url::Url::parse(url).is_ok_and(|u|matches!(u.scheme(),"http"|"https") && u.host_str().is_some() && u.username().is_empty() && u.password().is_none()) }

pub struct Store { pub pool: SqlitePool, pub root: PathBuf, pub project_pool: SqlitePool, pub write: Mutex<()> }
impl Store {
    pub async fn open(root: PathBuf, project_pool: SqlitePool) -> Result<Self> {
        // This root is selected by the application, never by a phone request.
        fs::create_dir_all(&root).map_err(db_error)?;
        Self::check_directory(&root)?;
        let attachments=root.join("attachments");fs::create_dir_all(&attachments).map_err(db_error)?;Self::check_directory(&attachments)?;
        let pool=sqlx::sqlite::SqlitePoolOptions::new().max_connections(4).connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(root.join("ideas.db")).create_if_missing(true).foreign_keys(true).journal_mode(sqlx::sqlite::SqliteJournalMode::Wal).busy_timeout(std::time::Duration::from_secs(10))).await.map_err(db_error)?;
        let mut tx=pool.begin().await.map_err(db_error)?;
        let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&mut *tx).await.map_err(db_error)?;
        if version==0 { sqlx::raw_sql(include_str!("../../ideas_migrations/001_canvas.sql")).execute(&mut *tx).await.map_err(db_error)?; }
        if version<=1 {sqlx::raw_sql(include_str!("../../ideas_migrations/002_rooms_relations.sql")).execute(&mut *tx).await.map_err(db_error)?;}
        if version<=2 {sqlx::raw_sql(include_str!("../../ideas_migrations/003_edge_appearance.sql")).execute(&mut *tx).await.map_err(db_error)?;}
        if version>3 { return Err(error("storage","想法数据库版本较新，已停止写入")); }
        tx.commit().await.map_err(db_error)?;
        sqlx::query("INSERT OR IGNORE INTO idea_settings(key,value) VALUES('identity',?)").bind(json!(random_id()).to_string()).execute(&pool).await.map_err(db_error)?;
        Ok(Self{pool,root,project_pool,write:Mutex::new(())})
    }
    fn check_directory(path: &Path) -> Result<()> {
        let meta=fs::symlink_metadata(path).map_err(db_error)?;
        #[cfg(windows)] let linked={use std::os::windows::fs::MetadataExt;meta.file_attributes()&0x400!=0};
        #[cfg(not(windows))] let linked=meta.file_type().is_symlink();
        if linked || !meta.is_dir() { return Err(error("path","附件数据目录不能是符号链接或目录联接")); } Ok(())
    }
    pub async fn projects(&self) -> Result<Value> {
        let rows:Vec<(String,String)>=sqlx::query_as("SELECT id,name FROM projects WHERE deleted_at IS NULL ORDER BY sort_order,name").fetch_all(&self.project_pool).await.map_err(db_error)?;
        Ok(json!(rows.into_iter().map(|(id,name)|json!({"id":id,"name":name})).collect::<Vec<_>>()))
    }
    pub async fn setting(&self,key:&str) -> Result<Value> {
        let text:String=sqlx::query_scalar("SELECT value FROM idea_settings WHERE key=?").bind(key).fetch_one(&self.pool).await.map_err(db_error)?;
        serde_json::from_str(&text).map_err(db_error)
    }
    pub async fn snapshot(&self, cursor: Option<i64>) -> Result<Value> {
        let mut tx=self.pool.begin().await.map_err(db_error)?;
        let seq:i64=sqlx::query_scalar("SELECT COALESCE(MAX(seq),0) FROM idea_changes").fetch_one(&mut *tx).await.map_err(db_error)?;
        let records:Vec<String>=if let Some(cursor)=cursor {
            sqlx::query_scalar("SELECT data FROM idea_records WHERE id IN (SELECT idea_id FROM idea_changes WHERE seq>?)").bind(cursor).fetch_all(&mut *tx).await.map_err(db_error)?
        } else { sqlx::query_scalar("SELECT data FROM idea_records").fetch_all(&mut *tx).await.map_err(db_error)? };
        let attachments=sqlx::query("SELECT id,idea_id,name,media_type,size,sha256,created_at,deleted_at FROM idea_attachments").fetch_all(&mut *tx).await.map_err(db_error)?;
        let data=records.iter().map(|s|serde_json::from_str::<Value>(s).map_err(db_error)).collect::<Result<Vec<_>>>()?;
        let files:Vec<Value>=attachments.iter().map(|r|json!({"id":r.get::<String,_>("id"),"idea_id":r.get::<String,_>("idea_id"),"name":r.get::<String,_>("name"),"media_type":r.get::<String,_>("media_type"),"size":r.get::<i64,_>("size"),"sha256":r.get::<String,_>("sha256"),"deleted_at":r.get::<Option<i64>,_>("deleted_at")})).collect();
        let canvas_rows:Vec<(String,String)>=sqlx::query_as("SELECT scope_id,data FROM idea_canvases").fetch_all(&mut *tx).await.map_err(db_error)?;
        let mut canvases=serde_json::Map::new();for (id,data) in canvas_rows {canvases.insert(id,serde_json::from_str(&data).map_err(db_error)?);}
        tx.commit().await.map_err(db_error)?;
        Ok(json!({"canvases":canvases,"records":data,"attachments":files,"cursor":seq,"identity":self.setting("identity").await?,"canvas":self.canvas_state("hall").await?,"projects":self.projects().await?,"limits":{"attachment_bytes":MAX_ATTACHMENT_BYTES}}))
    }
    pub async fn record(&self,id:&str) -> Result<Value> {
        let text:Option<String>=sqlx::query_scalar("SELECT data FROM idea_records WHERE id=?").bind(id).fetch_optional(&self.pool).await.map_err(db_error)?;
        text.map(|s|serde_json::from_str(&s).map_err(db_error)).unwrap_or_else(||Err(error("missing","想法不存在")))
    }
    pub(super) fn validate(patch:&Value) -> Result<()> {
        let object=patch.as_object().ok_or_else(||error("invalid","修改内容必须为对象"))?;
        for (key,value) in object {
            let valid=match key.as_str() {
                "type"=>value.as_str().is_some_and(|s|["text","image","link","file"].contains(&s)),
                "title"=>value.as_str().is_some_and(|s|s.chars().count()<=500),
                "content"=>value.as_str().is_some_and(|s|s.chars().count()<=200_000),
                "url"=>value.as_str().is_some_and(|s|s.is_empty()||s.len()<=8192&&safe_url(s)),
                "project_id"=>value.is_null()||value.as_str().is_some_and(|s|s.len()<=120),
                "tags"=>value.as_array().is_some_and(|a|a.len()<=40&&a.iter().all(|v|v.as_str().is_some_and(|s|s.chars().count()<=80))),
                "canvas_x"|"canvas_y"=>value.as_f64().is_some_and(|n|n.is_finite()&&n.abs()<=1_000_000.0),
                "width"=>value.as_f64().is_some_and(|n|(180.0..=1600.0).contains(&n)),
                "height"=>value.as_f64().is_some_and(|n|(120.0..=1800.0).contains(&n)),
                "z_index"=>value.as_i64().is_some_and(|n|(0..=1_000_000).contains(&n)),
                "deleted"=>value.is_boolean(),
                _=>false
            };
            if !valid {return Err(error("invalid",format!("字段 {key} 无效或不允许修改")));}
        } Ok(())
    }
    pub async fn mutate(&self,args:Value) -> Result<Value> {
        let id=string(&args,"id")?;let op=string(&args,"op_id")?;
        if !valid_id(&id)||!valid_id(&op){return Err(error("invalid","想法或操作 ID 无效"));}
        let base=args["base_version"].as_i64().filter(|n|*n>=0).ok_or_else(||error("invalid","缺少基础版本"))?;
        let kind=args["entity"].as_str().unwrap_or("idea");
        let patch=&args["patch"];super::objects::validate(kind,patch)?;
        let request_hash=hash(args.to_string().as_bytes());let _guard=self.write.lock().await;
        let mut tx=self.pool.begin().await.map_err(db_error)?;
        if let Some((digest,result))=sqlx::query_as::<_,(String,String)>("SELECT request_hash,result FROM idea_operations WHERE id=?").bind(&op).fetch_optional(&mut *tx).await.map_err(db_error)? {
            if digest!=request_hash{return Err(error("invalid","同一操作 ID 不能提交不同内容"));}return serde_json::from_str(&result).map_err(db_error);
        }
        let previous=sqlx::query_as::<_,(String,i64,String)>("SELECT data,version,field_versions FROM idea_records WHERE id=?").bind(&id).fetch_optional(&mut *tx).await.map_err(db_error)?;
        let stamp=now();
        let (mut record,version,mut fields)=if let Some((data,version,fields))=previous { (serde_json::from_str::<Value>(&data).map_err(db_error)?,version,serde_json::from_str::<BTreeMap<String,i64>>(&fields).map_err(db_error)?) }
        else {
            if base!=0{return Err(error("missing","想法不存在，已保留本地草稿"));}
            (super::objects::initial(kind,&id,stamp),0,BTreeMap::new())
        };
        if record["_entity"].as_str().unwrap_or("idea")!=kind{return Err(error("invalid","对象类型不一致"));}
        let conflicting=base>version || record["deleted_at"].is_number()&&base<version || patch.get("deleted").is_some()&&base<version || patch.as_object().unwrap().keys().any(|k|*fields.get(k).unwrap_or(&0)>base);
        if conflicting {
            let conflict=json!({"code":"conflict","message":"此想法已有其他修改，本地草稿已保留","current":record,"attempted":patch,"id":id,"base_version":base});
            sqlx::query("INSERT OR IGNORE INTO idea_conflicts(id,idea_id,attempted,current,created_at) VALUES(?,?,?,?,?)").bind(&op).bind(&id).bind(patch.to_string()).bind(record.to_string()).bind(stamp as i64).execute(&mut *tx).await.map_err(db_error)?;
            tx.commit().await.map_err(db_error)?;return Err(conflict);
        }
        for (key,value) in patch.as_object().unwrap(){if key=="deleted"{record["deleted_at"]=if value==true{json!(stamp)}else{Value::Null};}else{record[key]=value.clone();}fields.insert(key.clone(),version+1);}
        record["version"]=json!(version+1);record["updated_at"]=json!(stamp);
        super::objects::validate_links(&mut tx,&mut record).await?;
        if kind=="room" && patch["deleted"]==true {super::objects::delete_room(&mut tx,&record,&args).await?;}
        super::objects::write_record(&mut tx,&record,&fields).await?;
        if kind=="room" {sqlx::query("INSERT OR IGNORE INTO idea_canvases(scope_id,data) VALUES(?,?)").bind(&id).bind(json!({"x":0,"y":0,"zoom":1,"version":0}).to_string()).execute(&mut *tx).await.map_err(db_error)?;}
        sqlx::query("INSERT INTO idea_operations(id,request_hash,result) VALUES(?,?,?)").bind(op).bind(request_hash).bind(record.to_string()).execute(&mut *tx).await.map_err(db_error)?;
        tx.commit().await.map_err(db_error)?;Ok(record)
    }
    pub async fn canvas_state(&self,scope:&str)->Result<Value>{
        let text:String=sqlx::query_scalar("SELECT data FROM idea_canvases WHERE scope_id=?").bind(scope).fetch_one(&self.pool).await.map_err(db_error)?;serde_json::from_str(&text).map_err(db_error)
    }
    pub async fn canvas(&self,args:Value) -> Result<Value> {
        let _guard=self.write.lock().await;let scope=args["scope_id"].as_str().unwrap_or("hall");
        if scope!="hall" {let room=self.record(scope).await?;if room["_entity"]!="room"||!room["deleted_at"].is_null(){return Err(error("invalid","包间不存在或已删除"));}}
        let current=self.canvas_state(scope).await?;
        if args["base_version"]!=current["version"]{return Err(json!({"code":"conflict","message":"画布视口已有更新","current":current}));}
        let x=args["x"].as_f64();let y=args["y"].as_f64();let zoom=args["zoom"].as_f64();
        if !x.is_some_and(|n|n.is_finite()&&n.abs()<=1_000_000.0)||!y.is_some_and(|n|n.is_finite()&&n.abs()<=1_000_000.0)||!zoom.is_some_and(|n|(0.2..=3.0).contains(&n)){return Err(error("invalid","画布坐标或缩放无效"));}
        let next=json!({"x":x,"y":y,"zoom":zoom,"version":current["version"].as_i64().unwrap_or(0)+1});
        sqlx::query("UPDATE idea_canvases SET data=? WHERE scope_id=?").bind(next.to_string()).bind(scope).execute(&self.pool).await.map_err(db_error)?;Ok(next)
    }
    pub async fn upload(&self,args:Value) -> Result<Value> {
        let id=string(&args,"id")?;let idea_id=string(&args,"idea_id")?;let name=string(&args,"name")?;
        if !valid_id(&id)||!valid_id(&idea_id)||name.is_empty()||name.len()>512||name.chars().any(|c|c.is_control()||"/\\:".contains(c)){return Err(error("invalid","附件名称或 ID 无效"));}
        let encoded=string(&args,"base64")?;
        if encoded.len()>MAX_ATTACHMENT_BYTES.div_ceil(3)*4{return Err(error("size","附件超过 20 MiB 限制"));}
        let bytes=STANDARD.decode(encoded).map_err(|_|error("invalid","附件编码无效"))?;
        if bytes.is_empty()||bytes.len()>MAX_ATTACHMENT_BYTES{return Err(error("size","附件为空或超过 20 MiB 限制"));}
        let media=if bytes.starts_with(b"\x89PNG\r\n\x1a\n"){"image/png"}else if bytes.starts_with(&[255,216,255]){"image/jpeg"}else if bytes.starts_with(b"GIF8"){"image/gif"}else if bytes.starts_with(b"RIFF")&&bytes.get(8..12)==Some(b"WEBP"){"image/webp"}else{"application/octet-stream"};
        if args["image"]==true&&media=="application/octet-stream"{return Err(error("type","图片只支持 PNG、JPEG、GIF 和 WebP"));}
        let digest=hash(&bytes);let _guard=self.write.lock().await;
        let record=self.record(&idea_id).await?;if record["_entity"]!="idea"{return Err(error("invalid","附件只能添加到想法"));}if !record["deleted_at"].is_null(){return Err(error("conflict","想法已删除，附件草稿仍保留在本机"));}
        if let Some((old_idea,old_hash,deleted))=sqlx::query_as::<_,(String,String,Option<i64>)>("SELECT idea_id,sha256,deleted_at FROM idea_attachments WHERE id=?").bind(&id).fetch_optional(&self.pool).await.map_err(db_error)? {
            if old_idea!=idea_id||old_hash!=digest||deleted.is_some(){return Err(error("invalid","附件任务 ID 已用于其他内容或已删除"));}
            return Ok(json!({"id":id,"idea_id":idea_id,"name":name,"media_type":media,"size":bytes.len(),"sha256":digest}));
        }
        Self::check_directory(&self.root.join("attachments"))?;
        let path=self.root.join("attachments").join(format!("{id}.bin"));
        if path.exists(){return Err(error("storage","发现上次未登记的附件文件，请保留现场并重试新上传任务"));}
        let part=path.with_extension("part");let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&part).map_err(|_|error("storage","无法创建附件副本"))?;
        if file.write_all(&bytes).and_then(|_|file.sync_all()).is_err(){let _=fs::remove_file(&part);return Err(error("storage","附件保存失败，请重试"));}drop(file);
        fs::rename(&part,&path).map_err(|_|error("storage","附件副本提交失败，上传任务已保留"))?;
        let result=async {
            let mut tx=self.pool.begin().await.map_err(db_error)?;
            sqlx::query("INSERT INTO idea_attachments(id,idea_id,name,media_type,size,sha256,created_at) VALUES(?,?,?,?,?,?,?)").bind(&id).bind(&idea_id).bind(&name).bind(media).bind(bytes.len() as i64).bind(&digest).bind(now() as i64).execute(&mut *tx).await.map_err(db_error)?;
            sqlx::query("INSERT INTO idea_changes(idea_id,version) VALUES(?,?)").bind(&idea_id).bind(record["version"].as_i64().unwrap()).execute(&mut *tx).await.map_err(db_error)?;tx.commit().await.map_err(db_error)
        }.await;
        if let Err(e)=result{let _=fs::remove_file(path);return Err(e);}
        Ok(json!({"id":id,"idea_id":idea_id,"name":name,"media_type":media,"size":bytes.len(),"sha256":digest}))
    }
    pub async fn attachment(&self,id:&str) -> Result<Value> {
        if !valid_id(id){return Err(error("path","附件 ID 无效"));}
        let row=sqlx::query("SELECT name,media_type,size,sha256 FROM idea_attachments a JOIN idea_records r ON r.id=a.idea_id WHERE a.id=? AND a.deleted_at IS NULL AND json_extract(r.data,'$.deleted_at') IS NULL").bind(id).fetch_optional(&self.pool).await.map_err(db_error)?.ok_or_else(||error("missing","附件不存在或所属想法已删除"))?;
        Self::check_directory(&self.root.join("attachments"))?;
        let path=self.root.join("attachments").join(format!("{id}.bin"));let meta=fs::symlink_metadata(&path).map_err(|_|error("missing","附件副本不存在"))?;
        #[cfg(windows)] let linked={use std::os::windows::fs::MetadataExt;meta.file_attributes()&0x400!=0};
        #[cfg(not(windows))] let linked=meta.file_type().is_symlink();
        if linked||!meta.is_file()||meta.len()>MAX_ATTACHMENT_BYTES as u64{return Err(error("path","附件副本类型不安全或大小异常"));}
        let bytes=fs::read(path).map_err(|_|error("missing","附件副本无法读取"))?;
        if hash(&bytes)!=row.get::<String,_>("sha256"){return Err(error("integrity","附件校验失败，未返回损坏内容"));}
        Ok(json!({"id":id,"name":row.get::<String,_>("name"),"media_type":row.get::<String,_>("media_type"),"base64":STANDARD.encode(bytes)}))
    }
    pub async fn remove_attachment(&self,id:&str) -> Result<Value> {
        let _guard=self.write.lock().await;
        let mut tx=self.pool.begin().await.map_err(db_error)?;
        let owner:Option<String>=sqlx::query_scalar("SELECT idea_id FROM idea_attachments WHERE id=?").bind(id).fetch_optional(&mut *tx).await.map_err(db_error)?;
        let owner=owner.ok_or_else(||error("missing","附件不存在"))?;
        sqlx::query("UPDATE idea_attachments SET deleted_at=? WHERE id=?").bind(now() as i64).bind(id).execute(&mut *tx).await.map_err(db_error)?;
        sqlx::query("INSERT INTO idea_changes(idea_id,version) SELECT id,version FROM idea_records WHERE id=?").bind(owner).execute(&mut *tx).await.map_err(db_error)?;
        tx.commit().await.map_err(db_error)?;Ok(json!(true))
    }
}
