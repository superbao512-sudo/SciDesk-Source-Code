//! Learning metadata only. External sites and associated files are never modified.
use std::{fs,path::{Path,PathBuf}};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sqlx::{SqlitePool,sqlite::{SqliteConnectOptions,SqlitePoolOptions}};
use crate::{project_platform,project_service::{canonical,display,is_link,key}};
type Result<T> = std::result::Result<T,String>;
fn err(e:impl std::fmt::Display)->String{e.to_string()}
fn id()->String{uuid::Uuid::new_v4().to_string()}
fn text(v:&Value,k:&str)->Result<String>{v[k].as_str().map(str::to_owned).ok_or_else(||format!("缺少参数：{k}"))}
#[cfg(test)]
fn catalog()->Result<Value>{serde_json::from_str(include_str!("../learning_catalog.json")).map_err(err)}
fn valid_url(raw:&str)->bool{!raw.chars().any(|c|c.is_whitespace()||c.is_control())&&url::Url::parse(raw).is_ok_and(|u|matches!(u.scheme(),"http"|"https")&&u.host_str().is_some()&&u.username().is_empty()&&u.password().is_none())}
#[cfg(test)]
fn issues(row:&Value,book:&Value)->String{let mut errors=vec![];if row["title"].as_str().unwrap_or("").trim().is_empty(){errors.push("缺少标题")};if !valid_url(row["url"].as_str().unwrap_or("")){errors.push("无效或缺失网址")};if !book["categories"].as_array().is_some_and(|c|c.iter().any(|c|c["name"]==row["category1"]&&c["children"].as_array().is_some_and(|s|s.contains(&row["category2"])))){errors.push("无法识别分类")};errors.join("；")}
#[cfg(test)]
fn preview(book:&Value)->Value{
 let rows=book["records"].as_array().cloned().unwrap_or_default();let exceptions:Vec<Value>=rows.iter().filter(|r|r["decision"]!="否").filter_map(|r|{let issue=issues(r,book);(!issue.is_empty()).then(||json!({"source_id":r["source_id"],"title":r["title"],"url":r["url"],"issue":issue,"raw":r["raw"]}))}).collect();
 json!({"source":book["source"],"sha256":book["sha256"],"total":rows.len(),"core":rows.iter().filter(|r|r["decision"]=="是").count(),"optional":rows.iter().filter(|r|r["decision"]=="可选").count(),"excluded":rows.iter().filter(|r|r["decision"]=="否").count(),"duplicates":rows.iter().filter(|r|r["duplicate"].as_str().is_some_and(|d|!d.is_empty())).count(),"eligible_duplicates":rows.iter().filter(|r|r["decision"]!="否"&&r["duplicate"].as_str().is_some_and(|d|!d.is_empty())).count(),"invalid_urls":exceptions.iter().filter(|r|r["issue"].as_str().unwrap_or("").contains("网址")).count(),"unknown_categories":exceptions.iter().filter(|r|r["issue"].as_str().unwrap_or("").contains("分类")).count(),"exceptions":exceptions})
}
#[derive(Serialize,Deserialize,sqlx::FromRow)]
struct Category{id:String,parent_id:String,name:String,sort_order:i64}
#[derive(Serialize,Deserialize,sqlx::FromRow)]
struct Resource{id:String,title:String,url:String,category1:String,category2:String,property:String,tier:String,origin:String,source_id:Option<String>,folder:String,classification_note:String,duplicate:String,notes:String,tags:String,created_at:String,updated_at:String,removed:bool}
#[derive(Serialize,sqlx::FromRow)]
struct Backup{id:String,resource_id:String,path:String}
struct Store{pool:SqlitePool}
impl Store{
 async fn open(root:&Path)->Result<Self>{
  fs::create_dir_all(root).map_err(err)?;let pool=SqlitePoolOptions::new().max_connections(2).connect_with(SqliteConnectOptions::new().filename(root.join("learning.db")).create_if_missing(true).foreign_keys(true).busy_timeout(std::time::Duration::from_secs(10))).await.map_err(err)?;
  let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&pool).await.map_err(err)?;if version>1{pool.close().await;return Err("学习数据库版本高于当前程序".into())}
  if version==0{let mut tx=pool.begin().await.map_err(err)?;for statement in include_str!("../learning_migrations/001_learning.sql").split(';').filter(|s|!s.trim().is_empty()){sqlx::query(statement).execute(&mut *tx).await.map_err(err)?;}tx.commit().await.map_err(err)?;}Ok(Self{pool})
 }
 async fn categories(&self)->Result<Vec<Category>>{sqlx::query_as("SELECT * FROM categories ORDER BY sort_order,rowid").fetch_all(&self.pool).await.map_err(err)}
 async fn resource(&self,rid:&str)->Result<Resource>{sqlx::query_as("SELECT * FROM resources WHERE id=? AND removed=0").bind(rid).fetch_optional(&self.pool).await.map_err(err)?.ok_or("学习资源不存在或已移除".into())}
#[cfg(test)]
 async fn import(&self,book:Value)->Result<Value>{
  let mut report=preview(&book);let mut tx=self.pool.begin().await.map_err(err)?;
  // Acquire the write lock before checking batch/source identities.
  sqlx::query("UPDATE import_batches SET source=source WHERE 0").execute(&mut *tx).await.map_err(err)?;
  let first:i64=sqlx::query_scalar("SELECT COUNT(*) FROM import_batches").fetch_one(&mut *tx).await.map_err(err)?;
  if first==0{for (i,c) in book["categories"].as_array().ok_or("分类数据损坏")?.iter().enumerate(){
   let name=c["name"].as_str().ok_or("分类名称损坏")?;let cid=format!("excel:{i}");sqlx::query("INSERT OR IGNORE INTO categories(id,parent_id,name,sort_order) VALUES(?,'',?,?)").bind(&cid).bind(name).bind(i as i64).execute(&mut *tx).await.map_err(err)?;
   let parent:String=sqlx::query_scalar("SELECT id FROM categories WHERE parent_id='' AND name=?").bind(name).fetch_one(&mut *tx).await.map_err(err)?;
   for (j,s) in c["children"].as_array().ok_or("子分类数据损坏")?.iter().enumerate(){let sub=s.as_str().ok_or("子分类名称损坏")?;if !sub.is_empty(){sqlx::query("INSERT OR IGNORE INTO categories(id,parent_id,name,sort_order) VALUES(?,?,?,?)").bind(format!("{cid}:{j}")).bind(&parent).bind(sub).bind(j as i64).execute(&mut *tx).await.map_err(err)?;}}
  }}
  let mut created=0u64;let mut skipped=0u64;
  for row in book["records"].as_array().ok_or("来源数据损坏")?{
   let sid=row["source_id"].as_str().ok_or("缺少来源标识")?;let issue=if row["decision"]=="否"{String::new()}else{issues(row,&book)};
   sqlx::query("INSERT OR IGNORE INTO source_records(source_id,decision,raw_json,issue) VALUES(?,?,?,?)").bind(sid).bind(row["decision"].as_str().unwrap_or("")).bind(row["raw"].to_string()).bind(&issue).execute(&mut *tx).await.map_err(err)?;
   if !matches!(row["decision"].as_str(),Some("是"|"可选"))||!issue.is_empty(){continue}
   let result=sqlx::query("INSERT OR IGNORE INTO resources(id,title,url,category1,category2,property,tier,origin,source_id,folder,classification_note,duplicate) VALUES(?,?,?,?,?,?,?,'excel',?,?,?,?)").bind(format!("excel:{sid}")).bind(row["title"].as_str().unwrap_or("")).bind(row["url"].as_str().unwrap_or("")).bind(row["category1"].as_str().unwrap_or("")).bind(row["category2"].as_str().unwrap_or("")).bind(row["property"].as_str().unwrap_or("")).bind(if row["decision"]=="是"{"core"}else{"optional"}).bind(sid).bind(row["folder"].as_str().unwrap_or("")).bind(row["classification_note"].as_str().unwrap_or("")).bind(row["duplicate"].as_str().unwrap_or("")).execute(&mut *tx).await.map_err(err)?;created+=result.rows_affected();if result.rows_affected()==0{skipped+=1}
  }
  report["created"]=json!(created);report["skipped"]=json!(skipped);
  sqlx::query("INSERT INTO import_batches(sha256,source,report_json) VALUES(?,?,?) ON CONFLICT(sha256) DO UPDATE SET report_json=excluded.report_json").bind(book["sha256"].as_str().ok_or("缺少来源摘要")?).bind(book["source"].as_str().unwrap_or("")).bind(report.to_string()).execute(&mut *tx).await.map_err(err)?;tx.commit().await.map_err(err)?;Ok(report)
 }
 async fn run(&self,op:&str,args:Value)->Result<Value>{match op{
  "state"=>{
   let resources:Vec<Resource>=sqlx::query_as("SELECT * FROM resources WHERE removed=0 ORDER BY rowid").fetch_all(&self.pool).await.map_err(err)?;
   let backups:Vec<Backup>=sqlx::query_as("SELECT b.* FROM backups b JOIN resources r ON r.id=b.resource_id WHERE r.removed=0").fetch_all(&self.pool).await.map_err(err)?;
   let backups:Vec<Value>=backups.into_iter().map(|b|{let exists=Path::new(&b.path).is_file();let mut v=json!(b);v["available"]=json!(exists);v}).collect();
   let report:Option<String>=sqlx::query_scalar("SELECT report_json FROM import_batches ORDER BY rowid DESC LIMIT 1").fetch_optional(&self.pool).await.map_err(err)?;
   Ok(json!({"resources":resources,"categories":self.categories().await?,"backups":backups,"report":report.map(|r|serde_json::from_str::<Value>(&r)).transpose().map_err(err)?}))
  },
  "bookmarks"=>{
   let rows=args["rows"].as_array().ok_or("Missing bookmarks")?;
   let mut tx=self.pool.begin().await.map_err(err)?;
   // Acquire the writer lock before checking existing URLs.
   sqlx::query("UPDATE resources SET title=title WHERE 0").execute(&mut *tx).await.map_err(err)?;
   let mut urls:std::collections::HashSet<String>=sqlx::query_scalar("SELECT url FROM resources WHERE removed=0").fetch_all(&mut *tx).await.map_err(err)?.into_iter().collect();
   let existing:Vec<(String,String,String)>=sqlx::query_as("SELECT parent_id,name,id FROM categories").fetch_all(&mut *tx).await.map_err(err)?;
   let mut categories:std::collections::HashMap<_,_>=existing.into_iter().map(|(parent,name,id)|((parent,name),id)).collect();
   let mut added=0;
   for row in rows{
    let url=text(row,"url")?;
    if !valid_url(&url){return Err("Invalid bookmark URL".into())}
    if !urls.insert(url.clone()){continue}
    let title=text(row,"title")?;
    let first=text(row,"category1")?;
    let second=text(row,"category2")?;
    if first.trim().is_empty()||title.trim().is_empty(){return Err("Missing bookmark title or folder".into())}
    let mut parent=String::new();
    for name in [&first,&second]{
     if name.is_empty(){continue}
     let key=(parent.clone(),name.clone());
     if let Some(existing)=categories.get(&key){parent=existing.clone();continue}
     let new_id=id();
     sqlx::query("INSERT INTO categories(id,parent_id,name,sort_order) VALUES(?,?,?,(SELECT COALESCE(MAX(sort_order),-1)+1 FROM categories WHERE parent_id=?))").bind(&new_id).bind(&parent).bind(name).bind(&parent).execute(&mut *tx).await.map_err(err)?;
     categories.insert(key,new_id.clone());parent=new_id;
    }
    sqlx::query("INSERT INTO resources(id,title,url,category1,category2,tier,origin) VALUES(?,?,?,?,?,'core','manual')").bind(id()).bind(title).bind(url).bind(first).bind(second).execute(&mut *tx).await.map_err(err)?;
    added+=1;
   }
   tx.commit().await.map_err(err)?;
   Ok(json!({"added":added,"skipped":rows.len()-added}))
  },
  "save"=>{
   let title=text(&args,"title")?;let url=text(&args,"url")?;let cat1=text(&args,"category1")?;let cat2=text(&args,"category2")?;let property=text(&args,"property")?;let tier=text(&args,"tier")?;let notes=text(&args,"notes")?;let tags=text(&args,"tags")?;
   if title.trim().is_empty()||title.chars().count()>1000||!valid_url(&url)||url.len()>16384||!matches!(tier.as_str(),"core"|"optional")||notes.chars().count()>8000||tags.chars().count()>2000||property.chars().count()>250||[&title,&notes,&tags,&property].iter().any(|s|s.contains('\0')){return Err("请填写标题、有效的 HTTP/HTTPS 原始网址及核心/可选状态；内容超过长度限制".into())}
   let categories=self.categories().await?;let parent=categories.iter().find(|c|c.parent_id.is_empty()&&c.name==cat1).ok_or("请选择已有一级分类或先在分类管理中创建")?;if !cat2.is_empty()&&!categories.iter().any(|c|c.parent_id==parent.id&&c.name==cat2){return Err("二级分类不属于该一级分类".into())}
   let rid=args["id"].as_str().map(str::to_owned).unwrap_or_else(id);let duplicate:i64=sqlx::query_scalar("SELECT COUNT(*) FROM resources WHERE url=? AND id!=? AND removed=0").bind(&url).bind(&rid).fetch_one(&self.pool).await.map_err(err)?;
   if args["id"].is_string(){self.resource(&rid).await?;sqlx::query("UPDATE resources SET title=?,url=?,category1=?,category2=?,property=?,tier=?,notes=?,tags=?,updated_at=CURRENT_TIMESTAMP WHERE id=? AND removed=0").bind(title).bind(url).bind(cat1).bind(cat2).bind(property).bind(tier).bind(notes).bind(tags).bind(&rid).execute(&self.pool).await.map_err(err)?;}else{sqlx::query("INSERT INTO resources(id,title,url,category1,category2,property,tier,origin,notes,tags) VALUES(?,?,?,?,?,?,?,'manual',?,?)").bind(&rid).bind(title).bind(url).bind(cat1).bind(cat2).bind(property).bind(tier).bind(notes).bind(tags).execute(&self.pool).await.map_err(err)?;}
   Ok(json!({"id":rid,"same_url":duplicate}))
  },
  "remove"=>{let rid=text(&args,"id")?;let mut tx=self.pool.begin().await.map_err(err)?;sqlx::query("UPDATE resources SET removed=1,updated_at=CURRENT_TIMESTAMP WHERE id=?").bind(&rid).execute(&mut *tx).await.map_err(err)?;sqlx::query("DELETE FROM backups WHERE resource_id=?").bind(rid).execute(&mut *tx).await.map_err(err)?;tx.commit().await.map_err(err)?;Ok(json!(true))},
  "open_url"=>{let r=self.resource(&text(&args,"id")?).await?;if !valid_url(&r.url){return Err("网址无效，请先编辑".into())}open(PathBuf::from(r.url)).await?;Ok(json!(true))},
  "pick_file"=>{let result=tauri::async_runtime::spawn_blocking(project_platform::pick_learning_backup).await.map_err(err)??;Ok(json!(result))},
  "attach"|"reconnect"=>{
   let rid=text(&args,"id")?;self.resource(&rid).await?;let raw=text(&args,"path")?;let path=canonical(Path::new(&raw))?;if !path.is_file()||is_link(&fs::symlink_metadata(Path::new(&raw)).map_err(err)?){return Err("请选择存在的普通文件".into())}
   if op=="reconnect"{let result=sqlx::query("UPDATE backups SET path=?,path_key=? WHERE id=? AND resource_id=?").bind(display(&path)).bind(key(&path)).bind(text(&args,"backup_id")?).bind(rid).execute(&self.pool).await.map_err(err)?;if result.rows_affected()!=1{return Err("关联已不存在".into())}}else{sqlx::query("INSERT OR IGNORE INTO backups(id,resource_id,path,path_key) VALUES(?,?,?,?)").bind(id()).bind(rid).bind(display(&path)).bind(key(&path)).execute(&self.pool).await.map_err(err)?;}Ok(json!(true))
  },
  "detach"=>{sqlx::query("DELETE FROM backups WHERE id=? AND resource_id=?").bind(text(&args,"backup_id")?).bind(text(&args,"id")?).execute(&self.pool).await.map_err(err)?;Ok(json!(true))},
  "open_backup"=>{let rid=text(&args,"id")?;self.resource(&rid).await?;let path:String=sqlx::query_scalar("SELECT path FROM backups WHERE id=? AND resource_id=?").bind(text(&args,"backup_id")?).bind(rid).fetch_optional(&self.pool).await.map_err(err)?.ok_or("关联已不存在")?;let p=PathBuf::from(&path);if !p.is_file()||key(&canonical(&p)?)!=key(&p)||is_link(&fs::symlink_metadata(&p).map_err(err)?){return Err("文件已失效或位置改变，请重新关联".into())}open(p).await?;Ok(json!(true))},
  "category_save"|"category_move"|"category_remove"=>self.category(op,args).await,
  _=>Err("未知学习操作".into())
 }}
 async fn category(&self,op:&str,args:Value)->Result<Value>{
  let categories=self.categories().await?;let existing=args["id"].as_str().map(|id|categories.iter().find(|c|c.id==id).ok_or("分类不存在")).transpose()?;
  let mut tx=self.pool.begin().await.map_err(err)?;
  if op=="category_save"{
   let name=text(&args,"name")?.trim().to_owned();if name.is_empty()||name.chars().count()>120||name.chars().any(char::is_control){return Err("分类名称需为 1–120 字".into())}
   if let Some(old)=existing{
    sqlx::query("UPDATE categories SET name=? WHERE id=?").bind(&name).bind(&old.id).execute(&mut *tx).await.map_err(|_|"同级分类名称已存在".to_owned())?;
    if old.parent_id.is_empty(){sqlx::query("UPDATE resources SET category1=?,updated_at=CURRENT_TIMESTAMP WHERE category1=?").bind(&name).bind(&old.name).execute(&mut *tx).await.map_err(err)?;}else{let parent=categories.iter().find(|c|c.id==old.parent_id).ok_or("父分类不存在")?;sqlx::query("UPDATE resources SET category2=?,updated_at=CURRENT_TIMESTAMP WHERE category1=? AND category2=?").bind(&name).bind(&parent.name).bind(&old.name).execute(&mut *tx).await.map_err(err)?;}
   }else{let parent=args["parent_id"].as_str().unwrap_or("");if !parent.is_empty()&&!categories.iter().any(|c|c.id==parent&&c.parent_id.is_empty()){return Err("只支持两级分类".into())}sqlx::query("INSERT INTO categories(id,parent_id,name,sort_order) VALUES(?,?,?,(SELECT COALESCE(MAX(sort_order),-1)+1 FROM categories WHERE parent_id=?))").bind(id()).bind(parent).bind(name).bind(parent).execute(&mut *tx).await.map_err(|_|"同级分类名称已存在".to_owned())?;}
  }else{let old=existing.ok_or("请选择分类")?;if op=="category_move"{let peers:Vec<&Category>=categories.iter().filter(|c|c.parent_id==old.parent_id).collect();let index=peers.iter().position(|c|c.id==old.id).ok_or("分类不存在")? as i64;let delta=args["delta"].as_i64().filter(|d|*d==1||*d== -1).ok_or("排序方向无效")?;let target=index+delta;if target>=0&&(target as usize)<peers.len(){let mut ids:Vec<&str>=peers.iter().map(|c|c.id.as_str()).collect();ids.swap(index as usize,target as usize);for (i,id) in ids.iter().enumerate(){sqlx::query("UPDATE categories SET sort_order=? WHERE id=?").bind(i as i64).bind(id).execute(&mut *tx).await.map_err(err)?;}}}else{
    let count:i64=if old.parent_id.is_empty(){if categories.iter().any(|c|c.parent_id==old.id){return Err("先移除空的二级分类；有资源的分类不可删除".into())}sqlx::query_scalar("SELECT COUNT(*) FROM resources WHERE category1=? AND removed=0").bind(&old.name).fetch_one(&mut *tx).await.map_err(err)?}else{let p=categories.iter().find(|c|c.id==old.parent_id).ok_or("父分类不存在")?;sqlx::query_scalar("SELECT COUNT(*) FROM resources WHERE category1=? AND category2=? AND removed=0").bind(&p.name).bind(&old.name).fetch_one(&mut *tx).await.map_err(err)?};if count>0{return Err("此分类仍有资源，请先调整资源分类".into())}sqlx::query("DELETE FROM categories WHERE id=?").bind(&old.id).execute(&mut *tx).await.map_err(err)?;
   }}tx.commit().await.map_err(err)?;Ok(json!(true))
 }
}
async fn open(path:PathBuf)->Result<()>{tauri::async_runtime::spawn_blocking(move||project_platform::open(&path)).await.map_err(err)?}
pub(crate) async fn command(root:&Path,op:&str,args:Value)->Result<Value>{let store=Store::open(root).await?;let result=store.run(op,args).await;store.pool.close().await;result}

#[cfg(test)]
mod tests{
 use super::*;
 fn root()->PathBuf{PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("development/verification/learning-rust").join(id())}
 #[tokio::test]
 async fn real_catalog_import_is_exact_idempotent_and_preserves_edits(){
  let root=root();let store=Store::open(&root).await.unwrap();let book=catalog().unwrap();let report=store.import(book.clone()).await.unwrap();assert_eq!(report["created"],454);assert_eq!(report["core"],377);assert_eq!(report["optional"],77);assert_eq!(report["excluded"],155);assert_eq!(report["duplicates"],10);assert_eq!(report["eligible_duplicates"],0);assert_eq!(report["exceptions"],json!([]));assert_eq!(store.categories().await.unwrap().iter().filter(|c|c.parent_id.is_empty()).count(),18);
  for row in book["records"].as_array().unwrap(){let sid=row["source_id"].as_str().unwrap();let actual:Option<(String,String,String)>=sqlx::query_as("SELECT title,url,folder FROM resources WHERE source_id=?").bind(sid).fetch_optional(&store.pool).await.unwrap();if row["decision"]=="否"{assert!(actual.is_none())}else{let (title,url,folder)=actual.unwrap();assert_eq!(title,row["title"].as_str().unwrap());assert_eq!(url,row["url"].as_str().unwrap());assert_eq!(folder,row["folder"].as_str().unwrap())}}
  let mut r=store.run("state",json!({})).await.unwrap()["resources"][0].clone();r["title"]=json!("用户修改");r["notes"]=json!("保留笔记");store.run("save",r.clone()).await.unwrap();store.run("remove",json!({"id":store.run("state",json!({})).await.unwrap()["resources"][1]["id"]})).await.unwrap();let again=store.import(book).await.unwrap();assert_eq!(again["created"],0);assert_eq!(again["skipped"],454);assert_eq!(store.resource(r["id"].as_str().unwrap()).await.unwrap().title,"用户修改");store.pool.close().await;
  let reopened=Store::open(&root).await.unwrap();assert_eq!(reopened.run("state",json!({})).await.unwrap()["resources"].as_array().unwrap().len(),453);reopened.pool.close().await;
 }
 #[tokio::test]
 async fn distinct_same_url_bookmarks_survive_and_bad_rows_remain_auditable(){
  let store=Store::open(&root()).await.unwrap();let mut book=catalog().unwrap();let mut rows=vec![book["records"][0].clone(),book["records"][0].clone(),book["records"][1].clone()];rows[1]["source_id"]=json!("same-url-second-bookmark");rows[1]["title"]=json!("另一使用场景");rows[2]["source_id"]=json!("bad-row");rows[2]["url"]=json!("not-a-url");book["records"]=json!(rows);let r=store.import(book).await.unwrap();assert_eq!(r["created"],2);assert_eq!(r["exceptions"].as_array().unwrap().len(),1);let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM source_records").fetch_one(&store.pool).await.unwrap();assert_eq!(count,3);store.pool.close().await;
 }
 #[tokio::test]
 async fn failed_import_rolls_back_entire_batch(){
  let store=Store::open(&root()).await.unwrap();let mut book=catalog().unwrap();book["records"][2]["source_id"]=json!(null);assert!(store.import(book).await.is_err());let state=store.run("state",json!({})).await.unwrap();assert_eq!(state["resources"],json!([]));assert_eq!(state["categories"],json!([]));assert!(state["report"].is_null());store.pool.close().await;
 }
 #[tokio::test]
 async fn backup_associations_never_mutate_originals(){
  let root=root();let store=Store::open(&root).await.unwrap();store.import(catalog().unwrap()).await.unwrap();let r=store.run("state",json!({})).await.unwrap()["resources"][0].clone();let rid=r["id"].as_str().unwrap();let file=root.join("existing.md");fs::write(&file,b"original contents").unwrap();store.run("attach",json!({"id":rid,"path":display(&file)})).await.unwrap();let backup=store.run("state",json!({})).await.unwrap()["backups"][0].clone();assert_eq!(backup["available"],true);store.run("detach",json!({"id":rid,"backup_id":backup["id"]})).await.unwrap();assert_eq!(fs::read(&file).unwrap(),b"original contents");store.run("attach",json!({"id":rid,"path":display(&file)})).await.unwrap();let moved=root.join("renamed.md");fs::rename(&file,&moved).unwrap();assert_eq!(store.run("state",json!({})).await.unwrap()["backups"][0]["available"],false);store.run("reconnect",json!({"id":rid,"backup_id":store.run("state",json!({})).await.unwrap()["backups"][0]["id"],"path":display(&moved)})).await.unwrap();store.run("remove",json!({"id":rid})).await.unwrap();assert_eq!(fs::read(moved).unwrap(),b"original contents");assert_eq!(store.run("state",json!({})).await.unwrap()["backups"],json!([]));store.pool.close().await;
 }
 #[tokio::test]
 async fn category_rename_reorders_and_preserves_source(){
  let store=Store::open(&root()).await.unwrap();store.import(catalog().unwrap()).await.unwrap();let cats=store.categories().await.unwrap();let first=cats.iter().find(|c|c.parent_id.is_empty()).unwrap();store.run("category_save",json!({"id":first.id,"name":"自定义课程"})).await.unwrap();store.run("category_move",json!({"id":first.id,"delta":1})).await.unwrap();assert!(store.run("category_remove",json!({"id":first.id})).await.is_err());store.import(catalog().unwrap()).await.unwrap();assert_eq!(store.categories().await.unwrap().iter().filter(|c|c.parent_id.is_empty()).count(),18);assert!(store.run("state",json!({})).await.unwrap()["resources"].as_array().unwrap().iter().any(|r|r["category1"]=="自定义课程"));store.pool.close().await;
 }
 #[test]
 fn only_original_http_urls_may_open(){assert!(valid_url("https://example.org/p?a=1&b=2#x"));for raw in ["file:///E:/x","javascript:alert(1)"," https://example.org","https://user:password@example.org","http://","https://example.org/\nx"]{assert!(!valid_url(raw),"{raw}")}}
}
