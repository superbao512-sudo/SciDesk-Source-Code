use chrono::Utc;
use regex::Regex;
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sqlx::SqlitePool;
#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Snapshot{#[serde(default,skip_serializing_if="Option::is_none")] pub source:Option<String>,pub name:String,pub affiliation:String,pub interests:Vec<String>,pub works:Vec<Work>}
#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Work{pub title:String,pub authors:String,pub publication:String,pub year:Option<u32>}
fn text(html:&str)->String{
 let stripped=Regex::new("<[^>]*>").unwrap().replace_all(html," ");
 quick_xml::escape::unescape(&stripped).unwrap_or_else(|_|stripped.clone()).split_whitespace().collect::<Vec<_>>().join(" ")
}
fn attr(attrs:&str,key:&str)->Option<String>{Regex::new(&format!(r#"\b{}\s*=\s*["']([^"']*)["']"#,regex::escape(key))).ok()?.captures(attrs).map(|c|c[1].to_owned())}
fn elements(html:&str,tag:&str)->Vec<(String,String)>{
 Regex::new(&format!(r"(?is)<{tag}\b([^>]*)>(.*?)</{tag}\s*>" )).unwrap().captures_iter(html).map(|c|(c[1].to_owned(),c[2].to_owned())).collect()
}
fn has_class(attrs:&str,class:&str)->bool{attr(attrs,"class").is_some_and(|v|v.split_whitespace().any(|c|c==class))}
pub fn parse(html:&str)->Result<Snapshot,String>{
 if html.len()>3_000_000{return Err("主页响应过大，未更新缓存".into())}

 // Name is an explicit profile marker; generic Google/login/error pages never count as success.
 let name=Regex::new(r#"(?is)<div\b[^>]*\bid=["']gsc_prf_in["'][^>]*>(.*?)</div>"#).unwrap().captures(html).map(|c|text(&c[1])).filter(|v|!v.is_empty()).ok_or("未读取到公开 Scholar 个人主页；可能网络不可用、需要验证或页面结构已变化。已有缓存保留")?;
 let affiliation=Regex::new(r#"(?is)<div\b([^>]*\bclass=["'][^"']*\bgsc_prf_il\b[^"']*["'][^>]*)>(.*?)</div>"#).unwrap().captures_iter(html).find(|c|attr(&c[1],"id").is_none()).map(|c|text(&c[2])).unwrap_or_default();
 let interests=elements(html,"a").iter().filter(|(a,_)|has_class(a,"gsc_prf_inta")).map(|(_,h)|text(h)).filter(|s|!s.is_empty()).collect::<Vec<_>>();
 let mut works=Vec::new();
 for (attrs,row) in elements(html,"tr"){
  if !has_class(&attrs,"gsc_a_tr"){continue}
  let title=elements(&row,"a").iter().find(|(a,_)|has_class(a,"gsc_a_at")).map(|(_,s)|text(s)).unwrap_or_default();if title.is_empty(){continue}
  let gray=elements(&row,"div").iter().filter(|(a,_)|has_class(a,"gs_gray")).map(|(_,s)|text(s)).collect::<Vec<_>>();
  let year=elements(&row,"span").iter().filter(|(a,_)|has_class(a,"gsc_a_h")).find_map(|(_,s)|text(s).parse::<u32>().ok()).filter(|y|(1900..=Utc::now().format("%Y").to_string().parse().unwrap_or(9999)).contains(y));
  works.push(Work{title,authors:gray.first().cloned().unwrap_or_default(),publication:gray.get(1).cloned().unwrap_or_default(),year});if works.len()==100{break}
 }
 Ok(Snapshot{source:None,name,affiliation,interests,works})
}
fn user_id(input: &str) -> Result<String, String> {
    let invalid = "请粘贴带有 user 参数的 Google Scholar 公开个人主页链接";
    let url = url::Url::parse(input.trim()).map_err(|_| invalid)?;
    let domain = url.host_str().and_then(|h| h.strip_prefix("scholar.google."));
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some()
        || url.port().is_some() || url.path().trim_end_matches('/') != "/citations"
        || !domain.is_some_and(|suffix| suffix == "com" || suffix.len() == 2
            || ["co.", "com."].iter().any(|prefix| suffix.strip_prefix(prefix).is_some_and(|cc| cc.len() == 2))) {
        return Err(invalid.into());
    }
    let mut users = url.query_pairs().filter(|(key, _)| key == "user");
    let (_, id) = users.next().ok_or(invalid)?;
    if users.next().is_some() || id.len() != 12 || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-') {
        return Err(invalid.into());
    }
    Ok(id.into_owned())
}

fn profile_url(id: &str) -> String {
    format!("https://scholar.google.com/citations?hl=zh-CN&view_op=list_works&user={id}")
}

async fn bound_id(pool: &SqlitePool) -> Result<String,String> {
    sqlx::query_scalar::<_,Option<String>>("SELECT scholar_user_id FROM scholar_profile WHERE id=1")
        .fetch_one(pool).await.map_err(|e|e.to_string())?.ok_or_else(||"请先连接 Google Scholar 主页".into())
}
pub async fn bound_url(pool: &SqlitePool) -> Result<String,String> {
    Ok(profile_url(&bound_id(pool).await?))
}
pub async fn bind(pool: &SqlitePool, input: &str) -> Result<Value,String> {
    let id = user_id(input)?;
    sqlx::query("UPDATE scholar_profile SET scholar_user_id=?, snapshot_json=NULL, last_attempt_at=NULL, last_success_at=NULL, last_error=NULL WHERE id=1 AND scholar_user_id IS NOT ?")
        .bind(&id).bind(&id).execute(pool).await.map_err(|e|e.to_string())?;
    get(pool).await
}
pub async fn get(pool:&SqlitePool)->Result<Value,String>{
    let (id,snapshot,attempt,success,error):(Option<String>,Option<String>,Option<String>,Option<String>,Option<String>)=
        sqlx::query_as("SELECT scholar_user_id,snapshot_json,last_attempt_at,last_success_at,last_error FROM scholar_profile WHERE id=1")
            .fetch_one(pool).await.map_err(|e|e.to_string())?;
    // Old snapshots have no confirmed owner. Only display metadata for a saved binding.
    if id.is_none() {return Ok(json!({"url":null,"snapshot":null,"lastAttemptAt":null,"lastSuccessAt":null,"error":null}))}
    Ok(json!({"url":id.as_deref().map(profile_url),"snapshot":snapshot.and_then(|s|serde_json::from_str::<Snapshot>(&s).ok()),"lastAttemptAt":attempt,"lastSuccessAt":success,"error":error}))
}
pub async fn import_saved(pool:&SqlitePool,args:&Value)->Result<Value,String>{
 let id=user_id(args["url"].as_str().unwrap_or(""))?;
 let mut snapshot:Snapshot=serde_json::from_value(args["snapshot"].clone()).map_err(|_|"主页文件的字段无效")?;
 if snapshot.name.trim().is_empty()||snapshot.works.iter().any(|w|w.title.trim().is_empty()){return Err("主页文件缺少姓名或论文标题".into())}
 snapshot.source=Some("html".into());
 let data=serde_json::to_string(&snapshot).map_err(|e|e.to_string())?;
 if data.len()>2_000_000{return Err("主页元数据超过 2 MB".into())}
 let now=Utc::now().to_rfc3339();
 let saved=sqlx::query("UPDATE scholar_profile SET scholar_user_id=?,snapshot_json=?,last_success_at=?,last_attempt_at=?,last_error=NULL WHERE id=1 AND (scholar_user_id IS NULL OR scholar_user_id=?)")
  .bind(&id).bind(data).bind(&now).bind(&now).bind(&id).execute(pool).await.map_err(|e|e.to_string())?;
 if saved.rows_affected()==0{return Err("文件与当前绑定的学术主页不一致".into())}
 get(pool).await
}
pub async fn update(pool:&SqlitePool)->Result<Value,String>{
 let id=bound_id(pool).await?;
 let now=Utc::now().to_rfc3339();
 let started=sqlx::query("UPDATE scholar_profile SET last_attempt_at=? WHERE id=1 AND scholar_user_id=?")
     .bind(&now).bind(&id).execute(pool).await.map_err(|e|e.to_string())?;
 if started.rows_affected()==0 {return get(pool).await}
 let result=async{
  let client=reqwest::Client::builder().connect_timeout(std::time::Duration::from_secs(8)).timeout(std::time::Duration::from_secs(15)).redirect(reqwest::redirect::Policy::limited(3)).build().map_err(|e|e.to_string())?;
  let response=client.get(format!("{}&cstart=0&pagesize=100",profile_url(&id))).header(reqwest::header::USER_AGENT,concat!("SciDesk/",env!("CARGO_PKG_VERSION")," (manual public profile refresh)")).send().await.map_err(|e|if e.is_timeout(){"连接 Google Scholar 超时，请检查系统代理或稍后再试".to_owned()}else{"无法连接 Google Scholar，请检查网络和系统代理；已保存的主页链接与缓存会保留".to_owned()})?;
  if response.status()==reqwest::StatusCode::TOO_MANY_REQUESTS{return Err("Google Scholar 暂时限制了此网络的请求（HTTP 429）。请稍后更新，或打开 Scholar 原页查看；已有缓存保留".into())}
  if !response.status().is_success(){return Err(format!("Scholar 返回 HTTP {}，请稍后重试；已有缓存保留",response.status()))}
  if response.content_length().is_some_and(|n|n>3_000_000){return Err("主页响应过大".into())}
  let html=response.text().await.map_err(|e|e.to_string())?;parse(&html)
 }.await;
 persist_result(pool,&id,result,&now).await
}
pub(super) async fn persist_result(pool:&SqlitePool,id:&str,result:Result<Snapshot,String>,now:&str)->Result<Value,String>{
 match result{
  Ok(snapshot)=>{sqlx::query("UPDATE scholar_profile SET snapshot_json=?,last_success_at=?,last_error=NULL WHERE id=1 AND scholar_user_id=? AND last_attempt_at=?").bind(serde_json::to_string(&snapshot).map_err(|e|e.to_string())?).bind(now).bind(id).bind(now).execute(pool).await.map_err(|e|e.to_string())?;},
  Err(error)=>{sqlx::query("UPDATE scholar_profile SET last_error=? WHERE id=1 AND scholar_user_id=? AND last_attempt_at=?").bind(error).bind(id).bind(now).execute(pool).await.map_err(|e|e.to_string())?;}
 }get(pool).await
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn parse_only_profile_metadata_and_keep_phrases(){let html=r#"<div class="profile-wrapper"><div id="gsc_prf_in">Fixture Scientist</div><div class="gsc_prf_il"><a>Fixture Institute</a></div></div><a class="gsc_prf_inta gs_ibl">Phonon Heat Transport</a><tr class="gsc_a_tr"><td><a class="gsc_a_at">Fixture &amp; Science</a><div class="gs_gray">Test Author</div><div class="gs_gray">Fixture Journal</div></td><td><span class="gsc_a_h gsc_a_hc gs_ibl">2026</span></td></tr>"#;let s=parse(html).unwrap();assert_eq!(s.affiliation,"Fixture Institute");assert_eq!(s.interests,vec!["Phonon Heat Transport"]);assert_eq!(s.works[0].title,"Fixture & Science");assert_eq!(s.works[0].year,Some(2026));assert!(parse("<html>Sorry, unusual traffic</html>").is_err());}
}
