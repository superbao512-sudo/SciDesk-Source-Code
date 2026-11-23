//! Project-only metadata lookup. Never ingests records or creates journal deliveries.
use super::*;
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct Record{pub title:String,pub authors:Vec<String>,pub journal:Option<String>,pub published_at:Option<String>,pub year:Option<i64>,pub doi:Option<String>,pub arxiv_id:Option<String>,pub url:String,pub source:String,pub matched_keywords:Vec<String>}
#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
struct Query{source:String,journal_id:Option<String>,keywords:Vec<String>,match_logic:String,query_keyword:Option<String>,from_year:i64,to_year:i64,offset:u32,rows:u32}
impl Query{
 fn validate(&mut self)->Result<(),String>{self.keywords=self.keywords.iter().map(|s|normalize_phrase(s)).collect();let year=Utc::now().format("%Y").to_string().parse::<i64>().unwrap();if self.keywords.is_empty()||self.keywords.len()>8||self.keywords.iter().any(|s|s.is_empty()||s.chars().count()>120){return Err("请填写 1–8 个关键词或短语，每个不超过 120 字".into())}if !matches!(self.source.as_str(),"crossref"|"arxiv")||!matches!(self.match_logic.as_str(),"any"|"all"){return Err("检索来源或匹配方式无效".into())}if self.from_year<1900||self.to_year>year||self.from_year>self.to_year||self.rows==0||self.rows>100||self.offset>300||self.offset%100!=0{return Err("年份范围或检索分页参数无效".into())}Ok(())}
}
fn records(body:&str,q:&Query,id:&str,name:&str)->Result<(Vec<Record>,usize),String>{
 let candidates=if q.source=="arxiv"{validate_arxiv_response(body)?;parse_arxiv(body)}else{parse_crossref_for_journal(body,id,name)?};
 let fetched=if q.source=="arxiv"{Regex::new(r"<entry\b[^>]*>").unwrap().find_iter(body).count()}else{serde_json::from_str::<Value>(body).ok().and_then(|v|v["message"]["items"].as_array().map(Vec::len)).unwrap_or(0)};
 let items=candidates.into_iter().filter_map(|c|{let year=c.year.or_else(||c.published_at.as_deref()?.get(..4)?.parse().ok())?;if year<q.from_year||year>q.to_year{return None}let hay=normalize_phrase(&format!("{} {}",c.title,c.abstract_text.as_deref().unwrap_or("")));let matched=q.keywords.iter().filter(|k|keyword_hit(&hay,k)).cloned().collect::<Vec<_>>();if matched.is_empty()||q.match_logic=="all"&&matched.len()!=q.keywords.len(){return None}let url=c.article_url?;Some(Record{title:c.title,authors:c.authors.into_iter().map(|a|a.display).collect(),journal:c.journal,published_at:c.published_at,year:Some(year),doi:c.doi,arxiv_id:c.arxiv_id,url,source:c.source,matched_keywords:matched})}).collect();Ok((items,fetched))
}
pub(crate) async fn page(pool:&SqlitePool,args:Value)->Result<Value,String>{
 let mut q:Query=serde_json::from_value(args).map_err(|e|e.to_string())?;q.validate()?;
 let client=Client::builder().connect_timeout(std::time::Duration::from_secs(8)).timeout(std::time::Duration::from_secs(25)).build().map_err(|e|e.to_string())?;
 let (body,id,name)=if q.source=="crossref"{
  let id=q.journal_id.as_deref().ok_or("请选择期刊")?;let journal=read_journal(pool,id).await?;if !journal.subscription.as_ref().is_some_and(|s|s.enabled){return Err("期刊已不在期刊库，请刷新列表".into())}
  let issn=journal.issn_online.as_ref().or(journal.issn_print.as_ref()).ok_or("期刊缺少 ISSN")?;
  let term=if q.match_logic=="all"{q.keywords.join(" ")}else{let term=normalize_phrase(q.query_keyword.as_deref().ok_or("缺少当前检索关键词")?);if !q.keywords.contains(&term){return Err("当前关键词不在检索条件中".into())}term};
  let mut url=Url::parse(&format!("{CROSSREF_ENDPOINT}/{issn}/works")).map_err(|e|e.to_string())?;url.query_pairs_mut().append_pair("filter",&format!("type:journal-article,from-pub-date:{}-01-01,until-pub-date:{}-12-31",q.from_year,q.to_year)).append_pair("query.bibliographic",&term).append_pair("rows",&q.rows.to_string()).append_pair("offset",&q.offset.to_string()).append_pair("select","DOI,title,author,type,issued,published,published-online,published-print,abstract,URL,publisher,indexed");
  (fetch_with_retry(&client,url,&crossref_user_agent()).await?,journal.id,journal.name)
 }else{
  let enabled:i64=sqlx::query_scalar("SELECT COUNT(*) FROM source_registry WHERE id='arxiv' AND enabled=1").fetch_one(pool).await.map_err(|e|e.to_string())?;if enabled==0{return Err("arXiv 来源未启用".into())}
  static LAST:OnceLock<Mutex<Option<std::time::Instant>>>=OnceLock::new();let mut last=LAST.get_or_init(||Mutex::new(None)).lock().await;if let Some(t)=*last{let elapsed=t.elapsed();if elapsed<std::time::Duration::from_secs(3){tokio::time::sleep(std::time::Duration::from_secs(3)-elapsed).await}}
  let terms=q.keywords.iter().map(|p|format!("all:\"{}\"",p.replace(['"','\\']," "))).collect::<Vec<_>>().join(if q.match_logic=="all"{" AND "}else{" OR "});let query=format!("({terms}) AND submittedDate:[{}01010000 TO {}12312359]",q.from_year,q.to_year);
  let mut result=Err("arXiv 未返回结果".to_owned());for endpoint in ARXIV_ENDPOINTS{let mut url=arxiv_query_url(endpoint,&query,None,q.offset);let pairs=url.query_pairs().filter(|(k,_)|k!="max_results").map(|(k,v)|(k.into_owned(),v.into_owned())).collect::<Vec<_>>();url.set_query(None);url.query_pairs_mut().extend_pairs(pairs).append_pair("max_results",&q.rows.to_string());match fetch_with_retry(&client,url,USER_AGENT).await{Ok(body)=>match validate_arxiv_response(&body){Ok(())=>{result=Ok(body);break},Err(e)=>result=Err(e)},Err(e)=>result=Err(e)}tokio::time::sleep(std::time::Duration::from_secs(3)).await;}
  *last=Some(std::time::Instant::now());(result?,String::new(),"arXiv".into())
 };
 let (items,fetched)=records(&body,&q,&id,&name)?;Ok(json!({"items":items,"fetched":fetched,"hasMore":fetched>=q.rows as usize,"sourceName":name}))
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn year_and_complete_phrase_filter_preserve_real_metadata(){let mut q=Query{source:"arxiv".into(),journal_id:None,keywords:vec!["chiral phonons".into()],match_logic:"any".into(),query_keyword:None,from_year:2020,to_year:2025,offset:0,rows:100};q.validate().unwrap();let xml=r#"<feed><entry><id>https://arxiv.org/abs/2401.12345v2</id><title>Chiral phonons in a material</title><summary>Actual fixture abstract</summary><published>2024-01-02T00:00:00Z</published><author><name>Fixture Author</name></author></entry></feed>"#;let (rows,n)=records(xml,&q,"","arXiv").unwrap();assert_eq!(n,1);assert_eq!(rows[0].authors,vec!["Fixture Author"]);assert_eq!(rows[0].arxiv_id.as_deref(),Some("2401.12345"));q.from_year=2025;assert!(records(xml,&q,"","arXiv").unwrap().0.is_empty());q.from_year=2020;q.keywords=vec!["phonon heat transport".into()];assert!(records(xml,&q,"","arXiv").unwrap().0.is_empty());q.from_year=1899;assert!(q.validate().is_err());}
}
