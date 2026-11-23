//! Adapted from v1 paper_push.rs: provider parsing, deduplication, keyword-v2 matching and feed queries.
//! No v1 database, seeds, scheduler, notifications, AI or window commands are migrated.
pub(super) mod search;
pub(super) mod discovery;
use chrono::Utc;
use regex::Regex;
use quick_xml::{events::Event,Reader};
use reqwest::{header,Client,StatusCode,Url};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use sqlx::{QueryBuilder,Sqlite,SqlitePool,Transaction};
use std::{collections::{HashMap,HashSet},sync::{Arc,OnceLock}};
use tokio::sync::Mutex;
use unicode_normalization::UnicodeNormalization;
const ARXIV_ENDPOINTS:&[&str]=&["https://export.arxiv.org/api/query","https://arxiv.org/api/query"];
const CROSSREF_ENDPOINT:&str="https://api.crossref.org/journals";
const USER_AGENT:&str="SciDesk/0.3.0 (journal-feed-v2)";
const MATCH_ALGORITHM_VERSION:&str="keyword-v2";
static SYNC_MUTEX:OnceLock<Arc<Mutex<()>>>=OnceLock::new();
pub(super) async fn recover_interrupted_syncs(pool: &SqlitePool) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE sync_runs SET status='cancelled',error_type='cancelled',error_message='上次同步随应用退出而中断',finished_at=? WHERE status='running'")
        .bind(&now).execute(&mut *tx).await.map_err(|error| error.to_string())?;
    sqlx::query("UPDATE sync_state SET status='cancelled',last_error_type='cancelled',last_error_message='上次同步随应用退出而中断',updated_at=? WHERE status='running'")
        .bind(&now).execute(&mut *tx).await.map_err(|error| error.to_string())?;
    sqlx::query("UPDATE journal_subscriptions SET last_status='cancelled',last_error_type='cancelled',last_error='上次同步随应用退出而中断',updated_at=? WHERE last_status='running'")
        .bind(&now).execute(&mut *tx).await.map_err(|error| error.to_string())?;
    tx.commit().await.map_err(|error| error.to_string())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaperSyncRun {
    pub source: String,
    pub status: String,
    pub queries: u32,
    pub fetched: u32,
    pub parsed: u32,
    pub inserted: u32,
    pub updated: u32,
    pub matched: u32,
    pub pushed: u32,
    pub started_at: String,
    pub finished_at: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PaperFeedListOptions {
    pub topic_id: Option<String>,
    pub relevance: Option<String>,
    pub source: Option<String>,
    pub paper_type: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    pub search: Option<String>,
    pub sort: Option<String>,
    pub journal_id: Option<String>,
    pub paper_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperFeedItem {
    pub push_item_id: String,
    pub paper_id: String,
    pub topic_id: String,
    pub title: String,
    pub abstract_text: Option<String>,
    pub abstract_status: String,
    pub authors: Vec<String>,
    pub paper_type: String,
    pub source: String,
    pub journal_name: Option<String>,
    pub journal_short_name: Option<String>,
    pub journal_icon: Option<String>,
    pub published_at: Option<String>,
    pub first_seen_at: String,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub article_url: String,
    pub arxiv_url: Option<String>,
    pub score: f64,
    pub relevance_level: String,
    pub matched_terms: Vec<Value>,
    pub algorithm_version: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperFeedPage {
    pub items: Vec<PaperFeedItem>,
    pub total: i64,
    pub unread_count: i64,
    pub next_cursor: Option<String>,
}


#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperTopicKeyword {
    pub phrase: String,
    pub weight: f64,
    pub field: String,
    pub kind: String,
    pub enabled: bool,
    pub notes: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperTopic {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub match_logic: String,
    pub match_algorithm_version: String,
    pub created_at: String,
    pub updated_at: String,
    pub keywords: Vec<PaperTopicKeyword>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperTopicKeywordInput {
    pub phrase: String,
    pub weight: f64,
    pub field: Option<String>,
    pub kind: String,
    pub enabled: Option<bool>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperTopicInput {
    pub name: String,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub match_logic: Option<String>,
    pub keywords: Vec<PaperTopicKeywordInput>,
}


#[derive(Clone, Debug)]
struct Author { given: Option<String>, family: Option<String>, display: String, orcid: Option<String> }

#[derive(Clone, Debug)]
struct Candidate {
    title: String,
    abstract_text: Option<String>,
    authors: Vec<Author>,
    doi: Option<String>,
    arxiv_id: Option<String>,
    arxiv_version: Option<i64>,
    journal: Option<String>,
    journal_id: Option<String>,
    publisher: Option<String>,
    year: Option<i64>,
    published_at: Option<String>,
    source_updated_at: Option<String>,
    article_url: Option<String>,
    pdf_url: Option<String>,
    source: String,
    source_key: String,
    raw_json: String,
}

#[derive(sqlx::FromRow)]
struct FeedRow {
    push_item_id: String,
    paper_id: String,
    topic_id: String,
    title: String,
    abstract_text: Option<String>,
    abstract_status: String,
    authors_json: String,
    paper_type: String,
    source: String,
    journal_name: Option<String>,
    journal_short_name: Option<String>,
    published_at: Option<String>,
    first_seen_at: String,
    doi: Option<String>,
    arxiv_id: Option<String>,
    article_url: Option<String>,
    score: f64,
    tier: String,
    reason_json: String,
    algorithm_version: String,
    status: String,
    created_at: String,
}

#[derive(sqlx::FromRow, Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct FeedStatsRow {
    total: i64,
    unread: i64,
    high: i64,
    medium: i64,
    journal_articles: i64,
    preprints: i64,
}

fn clean_text(value: &str) -> String {
value.nfkc().collect::<String>().replace('\r', " ").replace('\n', " ").split_whitespace().collect::<Vec<_>>().join(" ")
}
fn strip_markup(value: &str) -> String {
    let stripped = Regex::new(r"(?s)<[^>]+>").unwrap().replace_all(value, " ");
    clean_text(&stripped)
}
pub(super) fn normalize_phrase(value: &str) -> String {
    clean_text(value).to_lowercase().chars().map(|c| match c { '‐'|'‑'|'‒'|'–'|'—'|'−'|'-'|'_' => ' ', _ => c }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}
fn doi(value: &str) -> Option<String> {
    let mut text = value.trim().to_lowercase();
    if text.is_empty() { return None; }
    if let Some(rest) = text.strip_prefix("doi:") { text = rest.trim().to_owned(); }
    for prefix in ["https://doi.org/", "http://doi.org/", "https://dx.doi.org/", "http://dx.doi.org/"] { if let Some(rest) = text.strip_prefix(prefix) { text = rest.to_owned(); break; } }
    text = text.trim_matches(|c: char| c == '/' || c.is_whitespace()).to_owned();
    (text.starts_with("10.") && text.contains('/') && !text.chars().any(char::is_whitespace)).then_some(text)
}
fn arxiv_id(value: &str) -> (Option<String>, Option<i64>) {
    let mut text = value.trim().to_owned();
    if let Some(pos) = text.rfind("/abs/") { text = text[pos + 5..].to_owned(); }
    if let Some(pos) = text.rfind("/pdf/") { text = text[pos + 5..].to_owned(); text = text.trim_end_matches(".pdf").to_owned(); }
    let (base, version) = text.rsplit_once('v').filter(|(_, v)| v.chars().all(|c| c.is_ascii_digit())).map(|(a,v)|(a.to_owned(),v.parse().ok())).unwrap_or((text,None));
    let valid=Regex::new(r"^(?:\d{4}\.\d{4,5}|[A-Za-z][A-Za-z0-9.-]*/\d{7})$").unwrap().is_match(&base);
    if !valid { (None,None) } else { (Some(base),version) }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperFeedTopic { pub id: String, pub name: String, pub match_count: i64, pub enabled: bool }
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperFeedJournal { pub id: String, pub name: String, pub short_name: String, pub article_count: i64 }
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperFeedCatalog { pub topics: Vec<PaperFeedTopic>, pub journals: Vec<PaperFeedJournal> }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperJournalSubscription {
    pub id: String,
    pub enabled: bool,
    pub sync_enabled: bool,
    pub last_status: String,
    pub last_success_at: Option<String>,
    pub last_attempt_at: Option<String>,
    pub last_error: Option<String>,
    pub last_error_type: Option<String>,
    pub last_query_count: i64,
    pub last_fetched_count: i64,
    pub last_parsed_count: i64,
    pub last_inserted_count: i64,
    pub last_updated_count: i64,
    pub last_matched_count: i64,
    pub last_pushed_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperJournal {
    pub id: String,
    pub name: String,
    pub series: String,
    pub short_name: String,
    pub issn_print: Option<String>,
    pub issn_online: Option<String>,
    pub publisher: Option<String>,
    pub website_url: Option<String>,
    pub subscription: Option<PaperJournalSubscription>,
    pub topic_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperJournalInput {
    pub name: String,
    pub series: Option<String>,
    pub short_name: Option<String>,
    pub issn_print: Option<String>,
    pub issn_online: Option<String>,
    pub publisher: Option<String>,
    pub website_url: Option<String>,
}
fn normalize_public_url(value: &str) -> Option<String> {
    let parsed = Url::parse(value.trim()).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    if !matches!(host.as_str(), "arxiv.org" | "export.arxiv.org" | "doi.org" | "dx.doi.org") { return None; }
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.port().is_some() { return None; }
    if parsed.scheme() == "https" { Some(parsed.to_string()) }
    else if parsed.scheme() == "http" { Some(format!("https://{}", value.trim().trim_start_matches("http://"))) }
    else { None }
}
fn doi_url(value: Option<&str>) -> Option<String> { value.and_then(doi).map(|value| format!("https://doi.org/{value}")) }
fn arxiv_url(value: Option<&str>) -> Option<String> { value.map(|value| format!("https://arxiv.org/abs/{value}")) }
fn canonical_article_url(article_url: Option<&str>, doi_value: Option<&str>, arxiv: Option<&str>) -> Option<String> {
    doi_url(doi_value).or_else(|| article_url.and_then(normalize_public_url)).or_else(|| arxiv_url(arxiv))
}
fn fingerprint(title: &str, first_author: &str, year: Option<i64>) -> String {
    let key = format!("{}|{}|{}", normalize_phrase(title), normalize_phrase(first_author), year.map(|v|v.to_string()).unwrap_or_default());
    format!("{:x}", Sha256::digest(key.as_bytes()))
}
fn xml_unescape(value: &str) -> String { quick_xml::escape::unescape(value).map(|text|text.into_owned()).unwrap_or_else(|_|value.to_owned()) }
fn tag_text(block: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(r"(?s)<(?:[A-Za-z0-9_]+:)?{name}[^>]*>(.*?)</(?:[A-Za-z0-9_]+:)?{name}>" )).ok()?;
    re.captures(block).and_then(|c| c.get(1)).map(|m| clean_text(&xml_unescape(&m.as_str().replace("<jats:p>", " ").replace("</jats:p>", " ")))).filter(|s| !s.is_empty())
}
fn all_tag_text(block: &str, name: &str) -> Vec<String> {
    let re = Regex::new(&format!(r"(?s)<(?:[A-Za-z0-9_]+:)?{name}[^>]*>(.*?)</(?:[A-Za-z0-9_]+:)?{name}>" )).unwrap();
    re.captures_iter(block).filter_map(|c| c.get(1)).map(|m| clean_text(&xml_unescape(m.as_str()))).filter(|s| !s.is_empty()).collect()
}

fn parse_arxiv(xml: &str) -> Vec<Candidate> {
    let entry = Regex::new(r"(?s)<entry\b[^>]*>(.*?)</entry>").unwrap();
    let href = Regex::new(r#"<link[^>]*href="([^"]+)"[^>]*/?>"#).unwrap();
    entry.captures_iter(xml).filter_map(|capture| {
        let block = capture.get(1)?.as_str();
        let id_url = tag_text(block,"id")?;
        let (arxiv,version) = arxiv_id(&id_url);
        let arxiv=arxiv?;
        let title = tag_text(block,"title")?;
        let abstract_text = tag_text(block,"summary");
        let authors = all_tag_text(block,"name").into_iter().map(|display| Author { given: None, family: None, display, orcid: None }).collect::<Vec<_>>();
        let doi_value = tag_text(block,"doi").and_then(|v| doi(&v));
        let journal = tag_text(block,"journal_ref");
        let published = tag_text(block,"published");
        let updated = tag_text(block,"updated");
        let article_url = doi_url(doi_value.as_deref()).or_else(|| arxiv_url(Some(&arxiv)));
        let pdf_url = href.captures_iter(block).find_map(|c| { let url=c.get(1)?.as_str(); url.contains("/pdf/").then(||normalize_public_url(url)).flatten() });
        let year = published.as_deref().and_then(|v| v.get(0..4)).and_then(|v| v.parse().ok());
        Some(Candidate { title, abstract_text, authors, doi: doi_value, arxiv_id: Some(arxiv.clone()), arxiv_version: version, journal, journal_id: None, publisher: None, year, published_at: published, source_updated_at: updated, article_url, pdf_url, source: "arxiv".into(), source_key: arxiv, raw_json: block.to_owned() })
    }).collect()
}

fn validate_arxiv_response(xml: &str) -> Result<(), String> {
    let body = xml.trim_start_matches('\u{feff}');
    let mut reader=Reader::from_str(body);
    let mut depth=0usize;
    let mut root_seen=false;
    loop {
        match reader.read_event().map_err(|error|format!("arXiv Atom 解析失败：{error}"))? {
            Event::Start(element)=>{
                if depth==0 {
                    if root_seen || element.local_name().as_ref()!="feed" { return Err("arXiv Atom 响应根节点无效".into()); }
                    root_seen=true;
                }
                depth+=1;
            },
            Event::End(_)=>{depth=depth.checked_sub(1).ok_or("arXiv Atom 节点不完整")?;},
            Event::Empty(element) if depth==0=>{
                if root_seen || element.local_name().as_ref()!="feed" {return Err("arXiv Atom 响应根节点无效".into());}
                root_seen=true;
            },
            Event::Eof=>break,
            _=>{},
        }
    }
    if !root_seen || depth!=0 {return Err("arXiv Atom 响应不完整".into());}
    if body.contains("/api/errors") || body.contains("<error") {
        let detail = tag_text(body, "summary")
            .or_else(|| tag_text(body, "message"))
            .unwrap_or_else(|| "arXiv 返回错误信封".into());
        return Err(format!("arXiv API 错误：{detail}"));
    }
    Ok(())
}

fn crossref_date(item: &Value) -> Option<String> {
    ["published-online","published-print","published","issued"].iter().find_map(|key| {
        let parts = item.get(*key)?.get("date-parts")?.get(0)?.as_array()?;
        let year = parts.first()?.as_i64()?;
        let month = parts.get(1).and_then(Value::as_i64);
        let day = parts.get(2).and_then(Value::as_i64);
        Some(match (month, day) {
            (Some(month), Some(day)) => format!("{year:04}-{month:02}-{day:02}"),
            (Some(month), None) => format!("{year:04}-{month:02}"),
            _ => format!("{year:04}"),
        })
    })
}
fn parse_crossref_for_journal(body: &str, journal_id: &str, journal_name: &str) -> Result<Vec<Candidate>, String> {
    let root: Value = serde_json::from_str(body).map_err(|e| format!("Crossref JSON 无效：{e}"))?;
    let items = root.get("message").and_then(|v| v.get("items")).and_then(Value::as_array).ok_or("Crossref 响应缺少 items")?;
    Ok(items.iter().filter(|item| item.get("type").and_then(Value::as_str).unwrap_or("") == "journal-article").filter_map(|item| {
        let title = item.get("title").and_then(|v| v.get(0)).and_then(Value::as_str).map(strip_markup).filter(|value|!value.is_empty())?;
        let doi_value = item.get("DOI").and_then(Value::as_str).and_then(doi);
        let authors = item.get("author").and_then(Value::as_array).map(|rows| rows.iter().filter_map(|row| { let given=row.get("given").and_then(Value::as_str).map(clean_text); let family=row.get("family").and_then(Value::as_str).map(clean_text); let display=match (&given,&family) {(Some(a),Some(b))=>format!("{a} {b}"),(None,Some(b))=>b.clone(),(Some(a),None)=>a.clone(),_=>String::new()}; (!display.is_empty()).then_some(Author{given,family,display,orcid:row.get("ORCID").and_then(Value::as_str).map(str::to_owned)}) }).collect()).unwrap_or_default();
        let abstract_text = item.get("abstract").and_then(Value::as_str).map(|v| clean_text(&Regex::new("<[^>]+>").unwrap().replace_all(v," "))).filter(|v| !v.is_empty());
        let published_at=crossref_date(item); let year=published_at.as_deref().and_then(|v|v.get(0..4)).and_then(|v|v.parse().ok());
        let source_key=doi_value.clone().or_else(||item.get("URL").and_then(Value::as_str).map(|value|{
            doi(value).unwrap_or_else(||value.trim().to_owned())
        }).filter(|value|!value.is_empty()))?;
        Some(Candidate { title, abstract_text, authors, doi: doi_value.clone(), arxiv_id: None, arxiv_version: None, journal: if journal_name.is_empty(){item["container-title"].get(0).and_then(Value::as_str).map(strip_markup)}else{Some(journal_name.to_owned())}, journal_id: (!journal_id.is_empty()).then(||journal_id.to_owned()), publisher:item.get("publisher").and_then(Value::as_str).map(str::to_owned), year, published_at, source_updated_at: item.get("indexed").and_then(|v|v.get("date-time")).and_then(Value::as_str).map(str::to_owned), article_url: doi_url(doi_value.as_deref()), pdf_url: None, source:"crossref".into(), source_key, raw_json:item.to_string() })
    }).collect())
}

fn crossref_query_url(issn: &str, filter: &str, offset: u32, rows: u32) -> Result<Url, String> {
    let mut url = Url::parse(&format!("{CROSSREF_ENDPOINT}/{issn}/works")).map_err(|error| format!("Crossref URL 无效：{error}"))?;
    url.query_pairs_mut()
        .append_pair("filter", filter)
        .append_pair("rows", &rows.to_string())
        .append_pair("select", "DOI,title,author,type,issued,published,published-online,published-print,abstract,URL,publisher,indexed")
        .append_pair("sort","published")
        .append_pair("order","desc")
        .append_pair("offset", &offset.to_string());
    Ok(url)
}

async fn fetch_with_retry(client: &Client, url: Url, user_agent: &str) -> Result<String,String> {
    let mut last = String::new();
    let arxiv=url.host_str().is_some_and(|host|host=="arxiv.org"||host=="export.arxiv.org");
    for attempt in 0..3u64 {
        let mut retry_delay=std::time::Duration::from_millis(750 * 2u64.pow(attempt as u32));
        if arxiv {retry_delay=retry_delay.max(std::time::Duration::from_secs(3));}
        let response = client.get(url.clone())
            .header(header::USER_AGENT,user_agent)
            .header(header::ACCEPT,"application/atom+xml, application/json")
            // This reqwest build does not enable compression codecs.
            .header(header::ACCEPT_ENCODING,"identity")
            .send().await;
        match response {
            Ok(response) if response.status().is_success() => match response.text().await {
                Ok(body) => return Ok(body),
                Err(error) => { last=format!("响应体解码失败：{error}"); }
            },
            Ok(response) => {
                let status=response.status();
                let retry_after=response.headers().get(header::RETRY_AFTER).and_then(|value|value.to_str().ok());
                if let Some(value)=retry_after {
                    let seconds=value.parse::<u64>().ok().or_else(||chrono::DateTime::parse_from_rfc2822(value).ok()
                        .map(|date|date.with_timezone(&Utc).signed_duration_since(Utc::now()).num_seconds().max(0) as u64));
                    if let Some(seconds)=seconds {
                        if seconds>60 { return Err(format!("HTTP {status}: Retry-After {seconds}s；稍后重试")); }
                        retry_delay=retry_delay.max(std::time::Duration::from_secs(seconds));
                    }
                }
                let body=response.text().await.unwrap_or_default();
                let detail=clean_text(&body).chars().take(240).collect::<String>();
                last=if detail.is_empty() {format!("HTTP {status}")} else {format!("HTTP {status}: {detail}")};
                if status != StatusCode::TOO_MANY_REQUESTS && !status.is_server_error() { break; }
            }
            Err(error) => { last=if error.is_timeout(){format!("timeout: {error}")}else{format!("network: {error:?}")}; }
        }
        if attempt < 2 { tokio::time::sleep(retry_delay).await; }
    }
    Err(last)
}

fn arxiv_query_url(endpoint: &str, query: &str, since: Option<&chrono::DateTime<Utc>>, start: u32) -> Url {
    let date_filter=since.map(|date|format!(" AND submittedDate:[{} TO 999912312359]",date.format("%Y%m%d%H%M")))
        .unwrap_or_default();
    let mut url=Url::parse(endpoint).expect("static arXiv endpoint");
    url.query_pairs_mut()
        .append_pair("search_query",&format!("({query}){date_filter}"))
        .append_pair("start",&start.to_string())
        .append_pair("max_results","50")
        .append_pair("sortBy","submittedDate")
        .append_pair("sortOrder","descending");
    url
}

async fn fetch_arxiv(client: &Client, query: &str, since: Option<&chrono::DateTime<Utc>>, start: u32) -> Result<String,String> {
    let mut errors=Vec::new();
    for endpoint in ARXIV_ENDPOINTS {
        if !errors.is_empty(){tokio::time::sleep(std::time::Duration::from_secs(3)).await;}
        match fetch_with_retry(client,arxiv_query_url(endpoint,query,since,start),USER_AGENT).await {
            Ok(body)=>match validate_arxiv_response(&body) {
                Ok(())=>return Ok(body),
                Err(error)=>errors.push(format!("{endpoint}: {error}")),
            },
            Err(error)=>errors.push(format!("{endpoint}: {error}")),
        }
    }
    Err(errors.join("; "))
}

async fn ingest(tx: &mut Transaction<'_,Sqlite>, candidate: &Candidate) -> Result<(String,bool),String> {
    let first_author=candidate.authors.first().map(|a|a.display.as_str()).unwrap_or("");
    let fp=fingerprint(&candidate.title,first_author,candidate.year);
    let mut existing: Option<String> = None;
    if let Some(value)=candidate.doi.as_deref() { existing=sqlx::query_scalar("SELECT id FROM papers WHERE lower(trim(doi))=?").bind(value).fetch_optional(&mut **tx).await.map_err(|e|e.to_string())?; }
    if existing.is_none() { if let Some(value)=candidate.arxiv_id.as_deref() { existing=sqlx::query_scalar("SELECT id FROM papers WHERE lower(trim(arxiv_id))=?").bind(value.to_lowercase()).fetch_optional(&mut **tx).await.map_err(|e|e.to_string())?; } }
    if existing.is_none() { existing=sqlx::query_scalar("SELECT id FROM papers WHERE fingerprint=?").bind(&fp).fetch_optional(&mut **tx).await.map_err(|e|e.to_string())?; }
    let id=existing.clone().unwrap_or_else(||uuid::Uuid::new_v4().to_string());
    let preserve_local = if existing.is_some() {
        sqlx::query_scalar::<_,Option<String>>("SELECT source_type FROM papers WHERE id=?")
            .bind(&id).fetch_one(&mut **tx).await.map_err(|e|e.to_string())?
            .is_some_and(|value| value == "local" || value == "manual")
    } else { false };
    let authors=candidate.authors.iter().map(|a|a.display.clone()).collect::<Vec<_>>().join("; ");
    let journal=candidate.journal.clone().unwrap_or_default();
    let abstract_value=candidate.abstract_text.clone();
    let stored_article_url=canonical_article_url(candidate.article_url.as_deref(),candidate.doi.as_deref(),candidate.arxiv_id.as_deref());
    let previous_metadata: Option<String> = sqlx::query_scalar("SELECT metadata_json FROM papers WHERE id=?")
        .bind(&id).fetch_optional(&mut **tx).await.map_err(|e|e.to_string())?;
    let mut metadata: Value = previous_metadata.and_then(|value|serde_json::from_str(&value).ok()).unwrap_or_else(||json!({}));
    if !metadata.is_object() { metadata=json!({}); }
    metadata["source"]=json!(candidate.source);
    metadata["source_key"]=json!(candidate.source_key);
    metadata["source_updated_at"]=json!(candidate.source_updated_at);
    let metadata=metadata.to_string();
    if existing.is_some() && !preserve_local {
        sqlx::query("UPDATE papers SET title=COALESCE(NULLIF(?,'') ,title),authors=COALESCE(NULLIF(?,'') ,authors),journal=COALESCE(NULLIF(?,'') ,journal),year=COALESCE(?,year),doi=COALESCE(NULLIF(?,'') ,doi),abstract=COALESCE(?,abstract),abstract_text=COALESCE(?,abstract_text),arxiv_id=COALESCE(?,arxiv_id),arxiv_version=COALESCE(?,arxiv_version),journal_id=COALESCE(journal_id,?),publisher=COALESCE(?,publisher),published_at=COALESCE(?,published_at),source_updated_at=COALESCE(?,source_updated_at),article_url=COALESCE(?,article_url),pdf_url=COALESCE(?,pdf_url),primary_source=COALESCE(primary_source,?),abstract_status=CASE WHEN ? IS NULL THEN abstract_status ELSE 'available' END,fingerprint=COALESCE(fingerprint,?),canonical_key=COALESCE(NULLIF(?,'') ,canonical_key),last_seen_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),metadata_json=COALESCE(?,metadata_json),updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?")
            .bind(if preserve_local { None } else { Some(candidate.title.clone()) })
            .bind(if preserve_local { None } else { Some(authors.clone()) })
            .bind(if preserve_local { None } else { Some(journal.clone()) })
            .bind(if preserve_local { None } else { candidate.year })
            .bind(candidate.doi.clone()).bind(abstract_value.clone()).bind(abstract_value).bind(candidate.arxiv_id.clone()).bind(candidate.arxiv_version).bind(candidate.journal_id.clone()).bind(candidate.publisher.clone()).bind(candidate.published_at.clone()).bind(candidate.source_updated_at.clone()).bind(stored_article_url.clone()).bind(candidate.pdf_url.clone()).bind(&candidate.source).bind(candidate.abstract_text.clone()).bind(&fp).bind(candidate.doi.as_ref().map(|value|format!("doi:{value}"))).bind(if preserve_local { None } else { Some(metadata) }).bind(&id).execute(&mut **tx).await.map_err(|e|e.to_string())?;
    } else if existing.is_none() {
        sqlx::query("INSERT INTO papers(id,title,authors,journal,year,doi,abstract,source_type,display_name,document_title,canonical_key,abstract_text,arxiv_id,arxiv_version,journal_id,publisher,published_at,source_updated_at,article_url,pdf_url,primary_source,abstract_status,fingerprint,first_seen_at,last_seen_at,metadata_json) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(&id).bind(&candidate.title).bind(&authors).bind(&journal).bind(candidate.year).bind(candidate.doi.clone().unwrap_or_default()).bind(candidate.abstract_text.clone().unwrap_or_default()).bind("remote").bind(&candidate.title).bind(&candidate.title).bind(candidate.doi.as_ref().map(|v|format!("doi:{v}")).or_else(||candidate.arxiv_id.as_ref().map(|v|format!("arxiv:{v}"))).unwrap_or_else(||format!("fingerprint:{fp}"))).bind(candidate.abstract_text.clone()).bind(candidate.arxiv_id.clone()).bind(candidate.arxiv_version).bind(candidate.journal_id.clone()).bind(candidate.publisher.clone()).bind(candidate.published_at.clone()).bind(candidate.source_updated_at.clone()).bind(stored_article_url).bind(candidate.pdf_url.clone()).bind(&candidate.source).bind(if candidate.abstract_text.is_some(){"available"}else{"missing"}).bind(&fp).bind(sqlx::query_scalar::<_,String>("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')").fetch_one(&mut **tx).await.map_err(|e|e.to_string())?).bind(sqlx::query_scalar::<_,String>("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')").fetch_one(&mut **tx).await.map_err(|e|e.to_string())?).bind(metadata).execute(&mut **tx).await.map_err(|e|e.to_string())?;
    }
    if !preserve_local {
        for (index,author) in candidate.authors.iter().enumerate() { sqlx::query("INSERT INTO paper_authors(paper_id,author_index,given_name,family_name,display_name,orcid) VALUES(?,?,?,?,?,?) ON CONFLICT(paper_id,author_index) DO UPDATE SET given_name=COALESCE(excluded.given_name,paper_authors.given_name),family_name=COALESCE(excluded.family_name,paper_authors.family_name),display_name=excluded.display_name,orcid=COALESCE(excluded.orcid,paper_authors.orcid)").bind(&id).bind(index as i64).bind(&author.given).bind(&author.family).bind(&author.display).bind(&author.orcid).execute(&mut **tx).await.map_err(|e|e.to_string())?; }
    }
    if candidate.source == "crossref" && !preserve_local {
        if let Some(journal_id) = candidate.journal_id.as_deref() {
            let metadata: String = sqlx::query_scalar("SELECT metadata_json FROM papers WHERE id=?")
                .bind(&id).fetch_one(&mut **tx).await.map_err(|e|e.to_string())?;
            let mut metadata: Value = serde_json::from_str(&metadata).unwrap_or_else(|_|json!({}));
            if !metadata.is_object() { metadata=json!({}); }
            let ids=metadata.as_object_mut().unwrap().entry("crossref_journal_ids").or_insert_with(||json!([]));
            if !ids.is_array() { *ids=json!([]); }
            let ids=ids.as_array_mut().unwrap();
            if !ids.iter().any(|value|value.as_str()==Some(journal_id)) { ids.push(json!(journal_id)); }
            sqlx::query("UPDATE papers SET metadata_json=? WHERE id=?").bind(metadata.to_string()).bind(&id)
                .execute(&mut **tx).await.map_err(|e|e.to_string())?;
        }
    }
    sqlx::query("INSERT INTO paper_source_records(id,paper_id,source,source_key,source_version,raw_json) VALUES(?,?,?,?,?,?) ON CONFLICT(source,source_key,source_version) DO UPDATE SET paper_id=excluded.paper_id,raw_json=excluded.raw_json,fetched_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')").bind(uuid::Uuid::new_v4().to_string()).bind(&id).bind(&candidate.source).bind(&candidate.source_key).bind(candidate.arxiv_version.unwrap_or(0)).bind(&candidate.raw_json).execute(&mut **tx).await.map_err(|e|e.to_string())?;
    Ok((id,existing.is_none()))
}

pub(super) fn keyword_identity(phrase:&str)->String{
 let normalized=normalize_phrase(phrase);
 if normalized.ends_with("phonons"){normalized.trim_end_matches('s').to_owned()}else if normalized=="electrides"{"electride".into()}else{normalized}
}
fn keyword_hit(text:&str,phrase:&str)->bool{
 let phrase=keyword_identity(phrase);if phrase.is_empty(){return false}
 if phrase.is_ascii(){let tail=if phrase.ends_with("phonon")||phrase=="electride"{"s?"}else{""};Regex::new(&format!("(?:^|[^a-z0-9]){}{tail}(?:$|[^a-z0-9])",regex::escape(&phrase))).is_ok_and(|re|re.is_match(text))}else{text.contains(&phrase)}
}

fn semantic_group(phrase: &str) -> String {
    match normalize_phrase(phrase).as_str() {
        "chiral phonon" | "chiral phonons" => "chiral phonon".into(),
        other => other.into(),
    }
}

fn matched_terms(reason: &str) -> Vec<Value> {
    serde_json::from_str::<Value>(reason).ok()
        .and_then(|value| value.get("matched").and_then(Value::as_array).cloned())
        .unwrap_or_default()
}

pub(super) async fn match_paper(tx: &mut Transaction<'_,Sqlite>, paper_id: &str) -> Result<(bool,bool),String> {
    let row: Option<(String,String)> = sqlx::query_as("SELECT title,COALESCE(abstract_text,abstract,'') FROM papers WHERE id=?").bind(paper_id).fetch_optional(&mut **tx).await.map_err(|e|e.to_string())?;
    let Some((title,abstract_text))=row else{return Ok((false,false))};
    let title=normalize_phrase(&title); let abstract_text=normalize_phrase(&abstract_text); let mut any=false;
    let extended:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pragma_table_info('topic_keywords') WHERE name='kind')")
        .fetch_one(&mut **tx).await.map_err(|error|error.to_string())?;
    let has_arxiv: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM paper_source_records WHERE paper_id=? AND source='arxiv')")
        .bind(paper_id).fetch_one(&mut **tx).await.map_err(|e|e.to_string())?;
    let crossref_paper: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM paper_source_records WHERE paper_id=? AND source='crossref')")
        .bind(paper_id).fetch_one(&mut **tx).await.map_err(|e|e.to_string())?;
    let mut topics:Vec<(String,String,String)>=sqlx::query_as(if extended {"SELECT id,required_terms_json,COALESCE(match_logic,'any') FROM topics WHERE enabled=1"} else {"SELECT id,required_terms_json,'any' FROM topics WHERE enabled=1"}).fetch_all(&mut **tx).await.map_err(|e|e.to_string())?;
    if extended && crossref_paper && !has_arxiv {
        let allowed:Vec<String>=sqlx::query_scalar("SELECT DISTINCT tjs.topic_id FROM topic_journal_subscriptions tjs JOIN journal_subscriptions js ON js.id=tjs.subscription_id AND js.enabled=1 AND js.sync_enabled=1 JOIN papers p ON p.id=? WHERE tjs.enabled=1 AND (js.journal_id=p.journal_id OR EXISTS(SELECT 1 FROM json_each(p.metadata_json,'$.crossref_journal_ids') membership WHERE membership.value=js.journal_id))")
            .bind(paper_id).fetch_all(&mut **tx).await.map_err(|e|e.to_string())?;
        topics.retain(|topic|allowed.contains(&topic.0));
    }
    for (topic_id,required_json,match_logic) in topics {
        let mut required:Vec<String>=serde_json::from_str(&required_json).unwrap_or_default();
        let keywords:Vec<(String,f64,String,String,i64)>=sqlx::query_as(if extended {"SELECT phrase,weight,field,kind,enabled FROM topic_keywords WHERE topic_id=? AND enabled=1 ORDER BY phrase"} else {"SELECT phrase,weight,field,'boost',1 FROM topic_keywords WHERE topic_id=? ORDER BY phrase"}).bind(&topic_id).fetch_all(&mut **tx).await.map_err(|e|e.to_string())?;
        let configured_keywords=keywords.iter().filter(|(_,_,_,kind,_)|kind!="exclude").map(|(phrase,_,_,_,_)|keyword_identity(phrase)).collect::<HashSet<_>>().len();
        required.extend(keywords.iter().filter(|(_,_,_,kind,_)|kind=="required").map(|(phrase,_,_,_,_)|phrase.clone()));
        required.sort(); required.dedup();
        let required_hits=required.iter().map(|term|{
            let normalized=normalize_phrase(term);
            let field=keywords.iter().find(|(phrase,_,_,kind,_)|kind=="required" && normalize_phrase(phrase)==normalized)
                .map(|(_,_,field,_,_)|field.as_str()).unwrap_or("both");
            (field=="both"||field=="title") && keyword_hit(&title,&normalized) || (field=="both"||field=="abstract") && keyword_hit(&abstract_text,&normalized)
        }).collect::<Vec<_>>();
        let required_gate=configured_keywords>=3 && if required.is_empty(){true}else if match_logic=="all"{required_hits.iter().all(|hit|*hit)}else{required_hits.iter().any(|hit|*hit)};
        if !required_gate {
            let reason=json!({"required_terms":required,"required_gate":false,"configured_keywords":configured_keywords,"matched":[],"algorithm_version":MATCH_ALGORITHM_VERSION}).to_string();
            sqlx::query("INSERT INTO paper_matches(paper_id,topic_id,score,tier,reason_json,algorithm_version) VALUES(?,?,0,'none',?,?) ON CONFLICT(paper_id,topic_id) DO UPDATE SET score=0,tier='none',reason_json=excluded.reason_json,algorithm_version=excluded.algorithm_version,matched_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')").bind(paper_id).bind(&topic_id).bind(reason).bind(MATCH_ALGORITHM_VERSION).execute(&mut **tx).await.map_err(|e|e.to_string())?;
            // Preserve read/dismissed history; tier=none hides this item from the Feed.
            continue;
        }
        let mut excluded=Vec::new(); let mut best: HashMap<(String,String),(String,f64,f64)> = HashMap::new();
        for (phrase,weight,field,kind,_) in keywords {
            let normalized=normalize_phrase(&phrase); let group=semantic_group(&normalized);
            let title_hit=(field=="both"||field=="title") && keyword_hit(&title,&normalized);
            let abstract_hit=(field=="both"||field=="abstract") && keyword_hit(&abstract_text,&normalized);
            if kind=="exclude" && (title_hit||abstract_hit) { excluded.push(json!({"phrase":normalized})); continue; }
            // Required phrases enforce admission above and also contribute relevance.
            if kind!="exclude" {
                for (matched_field,hit) in [("title",title_hit), ("abstract",abstract_hit)] {
                    if !hit { continue; }
                    let contribution=if matched_field=="title" {weight*1.5} else {weight};
                    let key=(group.clone(),matched_field.to_owned());
                    let replace=best.get(&key).map(|(old_phrase,_,old)| contribution>*old || (contribution==*old && normalized < *old_phrase)).unwrap_or(true);
                    if replace { best.insert(key,(normalized.clone(),weight,contribution)); }
                }
            }
        }
        let mut selected:Vec<(String,String,f64,f64)>=best.into_iter().map(|((_,field),(phrase,weight,contribution))|(phrase,field,weight,contribution)).collect();
        selected.sort_by(|a,b| a.1.cmp(&b.1).then_with(||a.0.cmp(&b.0)));
        let score:f64=selected.iter().map(|(_,_,_,contribution)|*contribution).sum();
        let matched:Vec<Value>=selected.iter().map(|(phrase,field,weight,contribution)|json!({"phrase":phrase,"field":field,"weight":weight,"contribution":contribution})).collect();
        let excluded_hit=!excluded.is_empty();
        let tier=if excluded_hit{"none"}else if score>=10.0{"high"}else if score>=5.0{"medium"}else{"none"};
        let reason=json!({"required_terms":required,"required_gate":true,"configured_keywords":configured_keywords,"excluded":excluded,"matched":matched,"algorithm_version":MATCH_ALGORITHM_VERSION});
        sqlx::query("INSERT INTO paper_matches(paper_id,topic_id,score,tier,reason_json,algorithm_version) VALUES(?,?,?,?,?,?) ON CONFLICT(paper_id,topic_id) DO UPDATE SET score=excluded.score,tier=excluded.tier,reason_json=excluded.reason_json,algorithm_version=excluded.algorithm_version,matched_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')").bind(paper_id).bind(&topic_id).bind(if excluded_hit{0.0}else{score}).bind(tier).bind(reason.to_string()).bind(MATCH_ALGORITHM_VERSION).execute(&mut **tx).await.map_err(|e|e.to_string())?;
        if tier!="none" {any=true;}
    }
    Ok((any,false))
}

fn sync_error_type(error: &str) -> &'static str {
    let lower=error.to_ascii_lowercase();
    if lower.contains("http 429") { "rate_limited" }
    else if lower.contains("http ") { "http_error" }
    else if lower.contains("timed out") || lower.contains("timeout") { "timeout" }
    else if lower.contains("json") || lower.contains("parse") || lower.contains("响应") || lower.contains("decoding response body") { "parse_error" }
    else if lower.contains("connect") || lower.contains("dns") || lower.contains("network") || lower.contains("sending request") { "network_error" }
    else { "failed" }
}

async fn persist_sync_run(pool: &SqlitePool, run_id: &str, run: &mut PaperSyncRun, status: &str, error_type: Option<&str>, error_message: Option<&str>, mark_success: bool) -> Result<(),String> {
    run.status=status.to_owned(); run.finished_at=Utc::now().to_rfc3339(); run.error=error_message.map(str::to_owned);
    let mut tx=pool.begin().await.map_err(|error|error.to_string())?;
    let extended:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='sync_runs')")
        .fetch_one(&mut *tx).await.map_err(|error|error.to_string())?;
    let error_json=error_message.map(|message|json!({"type":error_type,"message":message,"run":run}).to_string());
    if !extended {
        sqlx::query("UPDATE sync_state SET last_success_at=CASE WHEN ?=1 THEN ? ELSE last_success_at END,last_error_json=?,updated_at=? WHERE source=?")
            .bind(if mark_success{1i64}else{0}).bind(&run.finished_at).bind(error_json).bind(&run.finished_at).bind(&run.source)
            .execute(&mut *tx).await.map_err(|error|error.to_string())?;
        tx.commit().await.map_err(|error|error.to_string())?;
        return Ok(());
    }
    sqlx::query("UPDATE sync_runs SET status=?,query_count=?,fetched_count=?,parsed_count=?,inserted_count=?,updated_count=?,matched_count=?,pushed_count=?,error_type=?,error_message=?,finished_at=? WHERE id=?")
        .bind(status).bind(run.queries as i64).bind(run.fetched as i64).bind(run.parsed as i64).bind(run.inserted as i64).bind(run.updated as i64).bind(run.matched as i64).bind(run.pushed as i64).bind(error_type).bind(error_message).bind(&run.finished_at).bind(run_id).execute(&mut *tx).await.map_err(|e|e.to_string())?;
    sqlx::query("UPDATE sync_state SET status=?,last_success_at=CASE WHEN ?=1 THEN ? ELSE last_success_at END,last_error_json=?,last_error_type=?,last_error_message=?,last_query_count=?,last_fetched_count=?,last_parsed_count=?,last_inserted_count=?,last_updated_count=?,last_matched_count=?,last_pushed_count=?,updated_at=? WHERE source=?")
        .bind(status).bind(if mark_success {1i64}else{0}).bind(if mark_success {Some(run.finished_at.clone())}else{None})
        .bind(error_json).bind(error_type).bind(error_message).bind(run.queries as i64).bind(run.fetched as i64).bind(run.parsed as i64).bind(run.inserted as i64).bind(run.updated as i64).bind(run.matched as i64).bind(run.pushed as i64).bind(&run.finished_at).bind(&run.source).execute(&mut *tx).await.map_err(|e|e.to_string())?;
    tx.commit().await.map_err(|error|error.to_string())?;
    Ok(())
}

async fn source_is_enabled(pool: &SqlitePool, source: &str) -> Result<bool, String> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT EXISTS(SELECT 1 FROM source_registry r WHERE r.id=? AND r.enabled=1 AND ((r.id='arxiv' AND EXISTS(SELECT 1 FROM topics t WHERE t.enabled=1)) OR (r.id='crossref' AND EXISTS(SELECT 1 FROM journal_subscriptions js JOIN journals j ON j.id=js.journal_id WHERE js.enabled=1 AND js.sync_enabled=1 AND (NULLIF(j.issn_print,'') IS NOT NULL OR NULLIF(j.issn_online,'') IS NOT NULL)))))"
    ).bind(source).fetch_one(pool).await.map_err(|error| error.to_string())? != 0)
}

async fn run_source(pool:&SqlitePool, source:&str) -> Result<PaperSyncRun,String> {
    if !source_is_enabled(pool, source).await? {
        let now = Utc::now().to_rfc3339();
        return Ok(PaperSyncRun { source: source.to_owned(), status: "not_configured".into(), queries: 0, fetched: 0, parsed: 0, inserted: 0, updated: 0, matched: 0, pushed: 0, started_at: now.clone(), finished_at: now, error: None });
    }
    if source == "crossref" { return run_crossref_subscriptions(pool).await; }
    run_source_arxiv(pool, source).await
}

async fn run_source_arxiv(pool:&SqlitePool, source:&str) -> Result<PaperSyncRun,String> {
    let started=Utc::now(); let mut run=PaperSyncRun{source:source.into(),status:"running".into(),queries:0,fetched:0,parsed:0,inserted:0,updated:0,matched:0,pushed:0,started_at:started.to_rfc3339(),finished_at:String::new(),error:None};
    let run_id=uuid::Uuid::new_v4().to_string();
    let mut start_tx=pool.begin().await.map_err(|error|error.to_string())?;
    let extended:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='sync_runs')").fetch_one(&mut *start_tx).await.map_err(|error|error.to_string())?;
    if extended {
        sqlx::query("INSERT INTO sync_runs(id,source,status,started_at) VALUES(?,?,?,?)").bind(&run_id).bind(source).bind("running").bind(&run.started_at).execute(&mut *start_tx).await.map_err(|e|e.to_string())?;
    }
    sqlx::query(if extended {"UPDATE sync_state SET status='running',last_attempt_at=?,last_error_json=NULL,last_error_type=NULL,last_error_message=NULL,last_query_count=0,last_fetched_count=0,last_parsed_count=0,last_inserted_count=0,last_updated_count=0,last_matched_count=0,last_pushed_count=0,run_count=run_count+1,updated_at=? WHERE source=?"} else {"UPDATE sync_state SET last_attempt_at=?,last_error_json=NULL,run_count=run_count+1,updated_at=? WHERE source=?"}).bind(&run.started_at).bind(&run.started_at).bind(source).execute(&mut *start_tx).await.map_err(|e|e.to_string())?;
    start_tx.commit().await.map_err(|error|error.to_string())?;
    let client=Client::builder().connect_timeout(std::time::Duration::from_secs(10)).timeout(std::time::Duration::from_secs(30)).build().map_err(|e|e.to_string())?; let mut candidates=Vec::new();
    let mut item_errors=Vec::new();
    let result:Result<(),String>=async {
        if source=="arxiv" {
            let arxiv_since=Utc::now()-chrono::Duration::days(super::policy::DAYS);
            let phrases:Vec<String>=sqlx::query_scalar("SELECT DISTINCT k.phrase FROM topic_keywords k JOIN topics t ON t.id=k.topic_id WHERE t.enabled=1 AND k.enabled=1 AND k.kind<>'exclude' ORDER BY k.weight DESC LIMIT 30").fetch_all(pool).await.map_err(|e|e.to_string())?;
            let query=phrases.iter().map(|p|format!("all:\"{}\"",p.replace(['"','\\']," "))).collect::<Vec<_>>().join(" OR ");
            if !query.is_empty(){run.queries+=1;let text=fetch_arxiv(&client,&query,Some(&arxiv_since),0).await?;let count=Regex::new(r"<entry\b[^>]*>").unwrap().find_iter(&text).count();let rows=parse_arxiv(&text);run.fetched=count as u32;run.parsed=rows.len() as u32;if rows.len()<count{item_errors.push("arXiv 存在无法解析的条目".into())}candidates.extend(rows);}
        }
        Ok(())
    }.await;
    let provider_error=result.err();
    let mut seen=HashSet::new();
    for candidate in candidates {
        if !seen.insert((candidate.source_key.clone(),candidate.arxiv_version.unwrap_or(0))){continue;}
        let mut tx=match pool.begin().await { Ok(tx)=>tx, Err(error)=>{item_errors.push(error.to_string());continue;} };
        let result=async { let (id,is_new)=ingest(&mut tx,&candidate).await?; let (matched,pushed)=match_paper(&mut tx,&id).await?; tx.commit().await.map_err(|e|e.to_string())?; Ok::<(bool,bool,bool),String>((is_new,matched,pushed)) }.await;
        match result { Ok((is_new,matched,pushed))=>{if is_new{run.inserted+=1}else{run.updated+=1};if matched{run.matched+=1;}if pushed{run.pushed+=1;} }, Err(error)=>item_errors.push(error) }
    }
    if let Some(error)=provider_error {
        let error_type=sync_error_type(&error);
        let status=if run.inserted+run.updated>0 {"partial_success"}else{error_type};
        persist_sync_run(pool,&run_id,&mut run,status,Some(error_type),Some(&error),false).await?;
    } else if item_errors.is_empty() {
        persist_sync_run(pool,&run_id,&mut run,"success",None,None,true).await?;
    } else {
        let message=format!("条目解析、写入或匹配异常：{}",item_errors.join("; ").chars().take(500).collect::<String>());
        persist_sync_run(pool,&run_id,&mut run,"partial_success",Some("failed"),Some(&message),false).await?;
    }
    Ok(run)
}

#[derive(Clone, Debug)]
struct JournalSyncTarget {
    subscription_id: String,
    journal_id: String,
    name: String,
    issns: Vec<String>,
}

async fn journal_sync_targets(pool: &SqlitePool) -> Result<Vec<JournalSyncTarget>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT js.id,j.id,j.name,j.issn_print,j.issn_online,js.last_success_at FROM journal_subscriptions js JOIN journals j ON j.id=js.journal_id WHERE js.enabled=1 AND js.sync_enabled=1 ORDER BY j.name COLLATE NOCASE"
    ).fetch_all(pool).await.map_err(|error| error.to_string())?;
    Ok(rows.into_iter().filter_map(|(subscription_id,journal_id,name,print_issn,online_issn,_last_success_at)| {
        let mut issns: Vec<String> = Vec::new();
        for value in [print_issn,online_issn].into_iter().flatten() {
            if !issns.iter().any(|existing| existing.eq_ignore_ascii_case(&value)) { issns.push(value); }
        }
        (!issns.is_empty()).then_some(JournalSyncTarget { subscription_id,journal_id,name,issns })
    }).collect())
}

async fn set_journal_sync_running(pool: &SqlitePool, target: &JournalSyncTarget, started: &str) -> Result<(), String> {
    sqlx::query("UPDATE journal_subscriptions SET last_status='running',last_attempt_at=?,last_error=NULL,last_error_type=NULL,last_query_count=0,last_fetched_count=0,last_parsed_count=0,last_inserted_count=0,last_updated_count=0,last_matched_count=0,last_pushed_count=0,updated_at=? WHERE id=?")
        .bind(started).bind(started).bind(&target.subscription_id).execute(pool).await.map_err(|error| error.to_string())?;
    Ok(())
}

async fn persist_journal_sync(pool: &SqlitePool, target: &JournalSyncTarget, run: &PaperSyncRun, status: &str, error_type: Option<&str>, error_message: Option<&str>, mark_success: bool) -> Result<(), String> {
    sqlx::query("UPDATE journal_subscriptions SET last_status=?,last_success_at=CASE WHEN ?=1 THEN ? ELSE last_success_at END,last_error=?,last_error_type=?,last_query_count=?,last_fetched_count=?,last_parsed_count=?,last_inserted_count=?,last_updated_count=?,last_matched_count=?,last_pushed_count=?,updated_at=? WHERE id=?")
        .bind(status).bind(if mark_success {1i64} else {0}).bind(if mark_success {Some(run.finished_at.clone())} else {None})
        .bind(error_message).bind(error_type).bind(run.queries as i64).bind(run.fetched as i64).bind(run.parsed as i64).bind(run.inserted as i64).bind(run.updated as i64).bind(run.matched as i64).bind(run.pushed as i64).bind(&run.finished_at).bind(&target.subscription_id)
        .execute(pool).await.map_err(|error| error.to_string())?;
    Ok(())
}

async fn sync_crossref_journal(pool: &SqlitePool, target: &JournalSyncTarget) -> Result<PaperSyncRun, String> {
    let client=Client::builder().connect_timeout(std::time::Duration::from_secs(10)).timeout(std::time::Duration::from_secs(30)).build().map_err(|error|error.to_string())?;
    let mut fetch=|url: Url| {
        let client=client.clone();
        async move { fetch_with_retry(&client,url,&crossref_user_agent()).await }
    };
    sync_crossref_journal_with_fetch(pool,target,&mut fetch,30,1).await
}

fn crossref_user_agent() -> String {
    std::env::var("SCIDESK_CROSSREF_MAILTO").ok().filter(|value|!value.trim().is_empty()).map(|value|format!("{USER_AGENT}; mailto={value}")).unwrap_or_else(||USER_AGENT.to_owned())
}

async fn sync_crossref_journal_with_fetch<F, Fut>(pool: &SqlitePool, target: &JournalSyncTarget, fetch: &mut F, page_size: u32, max_pages: u32) -> Result<PaperSyncRun, String>
where F: FnMut(Url) -> Fut, Fut: std::future::Future<Output=Result<String,String>> {
    let started=Utc::now();
    let mut run=PaperSyncRun { source: format!("crossref:{}", target.journal_id), status:"running".into(), queries:0, fetched:0, parsed:0, inserted:0, updated:0, matched:0, pushed:0, started_at:started.to_rfc3339(), finished_at:String::new(), error:None };
    set_journal_sync_running(pool,target,&run.started_at).await?;
    let filter=format!("type:journal-article,from-pub-date:{},until-pub-date:{}",(Utc::now()-chrono::Duration::days(super::policy::DAYS)).format("%Y-%m-%d"),Utc::now().format("%Y-%m-%d"));
    let mut candidates=Vec::new(); let mut provider_errors=Vec::new(); let mut item_errors=Vec::new(); let mut write_errors=Vec::new();
    for issn in &target.issns {
        for page in 0..max_pages {
            run.queries+=1;
            let url=crossref_query_url(issn,&filter,page.saturating_mul(page_size),page_size)?;
            let text=match fetch(url).await { Ok(text)=>text, Err(error)=>{provider_errors.push(format!("ISSN {issn}: {error}")); break;} };
            let root:Value=match serde_json::from_str(&text) { Ok(root)=>root, Err(error)=>{provider_errors.push(format!("ISSN {issn}: Crossref JSON 无效：{error}")); break;} };
            let count=root.get("message").and_then(|message|message.get("items")).and_then(Value::as_array).map(Vec::len).unwrap_or(0);
            run.fetched+=count as u32;
            let rows=match parse_crossref_for_journal(&text,&target.journal_id,&target.name) { Ok(rows)=>rows, Err(error)=>{provider_errors.push(format!("ISSN {issn}: {error}")); break;} };
            if rows.len()<count { item_errors.push(format!("ISSN {issn}: {} 条论文解析失败，已跳过",count-rows.len())); }
            run.parsed+=rows.len() as u32; candidates.extend(rows);
            if count < page_size as usize { break; }
            if page+1==max_pages { break; }
        }
    }
    let mut seen=HashSet::new();
    for candidate in candidates {
        let identity=candidate.doi.clone().unwrap_or_else(||candidate.source_key.clone());
        if !seen.insert(identity) { continue; }
        let mut tx=match pool.begin().await { Ok(tx)=>tx, Err(error)=>{write_errors.push(error.to_string()); continue;} };
        let result=async { let (id,is_new)=ingest(&mut tx,&candidate).await?; let (matched,pushed)=match_paper(&mut tx,&id).await?; tx.commit().await.map_err(|error|error.to_string())?; Ok::<(bool,bool,bool),String>((is_new,matched,pushed)) }.await;
        match result { Ok((is_new,matched,pushed))=>{if is_new{run.inserted+=1}else{run.updated+=1};if matched{run.matched+=1};if pushed{run.pushed+=1};}, Err(error)=>write_errors.push(error) }
    }
    run.finished_at=Utc::now().to_rfc3339();
    let had_write_error=!write_errors.is_empty(); item_errors.extend(write_errors);
    let had_provider_error=!provider_errors.is_empty(); let had_item_error=!item_errors.is_empty();
    let provider_error=had_provider_error.then(||provider_errors.join("; "));
    let item_error=had_item_error.then(||item_errors.join("; ").chars().take(500).collect::<String>());
    let error_message=match (provider_error,item_error) { (Some(a),Some(b))=>Some(format!("{a}; {b}")), (Some(a),None)=>Some(a), (None,Some(b))=>Some(b), (None,None)=>None };
    let (status,error_type,mark_success)=if had_provider_error {
        (if run.inserted+run.updated>0 {"partial_success"} else {sync_error_type(error_message.as_deref().unwrap_or("failed"))},Some(sync_error_type(error_message.as_deref().unwrap_or("failed"))),false)
    } else if had_item_error { ("partial_success",Some(if had_write_error {"failed"} else {"parse_error"}),!had_write_error) } else { ("success",None,true) };
    run.status=status.into(); run.error=error_message.clone();
    persist_journal_sync(pool,target,&run,status,error_type,error_message.as_deref(),mark_success).await?;
    Ok(run)
}

async fn run_crossref_subscriptions(pool: &SqlitePool) -> Result<PaperSyncRun, String> {
    let started=Utc::now(); let mut aggregate=PaperSyncRun { source:"crossref".into(), status:"running".into(), queries:0, fetched:0, parsed:0, inserted:0, updated:0, matched:0, pushed:0, started_at:started.to_rfc3339(), finished_at:String::new(), error:None };
    let targets=journal_sync_targets(pool).await?;
    let run_id=uuid::Uuid::new_v4().to_string();
    let mut tx=pool.begin().await.map_err(|error|error.to_string())?;
    sqlx::query("INSERT INTO sync_runs(id,source,status,started_at) VALUES(?,'crossref','running',?)").bind(&run_id).bind(&aggregate.started_at).execute(&mut *tx).await.map_err(|error|error.to_string())?;
    sqlx::query("UPDATE sync_state SET status='running',last_attempt_at=?,run_count=run_count+1 WHERE source='crossref'").bind(&aggregate.started_at).execute(&mut *tx).await.map_err(|error|error.to_string())?;
    tx.commit().await.map_err(|error|error.to_string())?;
    if targets.is_empty() { aggregate.status="not_configured".into(); aggregate.finished_at=Utc::now().to_rfc3339(); persist_sync_run(pool,&run_id,&mut aggregate,"not_configured",Some("not_configured"),Some("没有启用的期刊订阅"),false).await?; return Ok(aggregate); }
    let mut errors=Vec::new(); let mut statuses=Vec::new();
    for target in targets {
        match sync_crossref_journal(pool,&target).await {
            Ok(run)=>{aggregate.queries+=run.queries;aggregate.fetched+=run.fetched;aggregate.parsed+=run.parsed;aggregate.inserted+=run.inserted;aggregate.updated+=run.updated;aggregate.matched+=run.matched;aggregate.pushed+=run.pushed;statuses.push(run.status);if let Some(error)=run.error{errors.push(format!("{}: {}",target.name,error));}},
            Err(error)=>errors.push(format!("{}: {error}",target.name)),
        }
    }
    aggregate.finished_at=Utc::now().to_rfc3339();
    aggregate.status=if errors.is_empty() && statuses.iter().all(|status|status=="success") {"success".into()} else if statuses.iter().any(|status|status=="success"||status=="partial_success") {"partial_success".into()} else {statuses.first().cloned().unwrap_or_else(||"failed".into())};
    aggregate.error=(!errors.is_empty()).then(||errors.join("; ").chars().take(1000).collect());
    let status=aggregate.status.clone(); let error=aggregate.error.clone();
    let mark_success=status=="success";
    persist_sync_run(pool,&run_id,&mut aggregate,&status,(!mark_success).then_some("failed"),error.as_deref(),mark_success).await?;
    Ok(aggregate)
}

pub(crate) async fn sync_one_journal(pool: &SqlitePool, journal_id: &str) -> Result<PaperSyncRun, String> {
    let lock=SYNC_MUTEX.get_or_init(||Arc::new(Mutex::new(()))).clone(); let _guard=lock.lock().await;
    if !source_is_enabled(pool, "crossref").await? { return Err("Crossref 来源未启用或没有有效期刊订阅".into()); }
    let target=journal_sync_targets(pool).await?.into_iter().find(|target|target.journal_id==journal_id).ok_or_else(||"期刊未启用同步订阅".to_owned())?;
    let run = sync_crossref_journal(pool,&target).await?;
    
    Ok(run)
}

fn validate_feed_options(options: &PaperFeedListOptions) -> Result<u32,String> {
    if let Some(value)=options.relevance.as_deref() { if !matches!(value,"high"|"medium") { return Err("relevance 必须是 high 或 medium".into()); } }
    if let Some(value)=options.source.as_deref() { if !matches!(value,"arxiv"|"crossref") { return Err("source 必须是 arxiv 或 crossref".into()); } }
    if let Some(value)=options.paper_type.as_deref() { if !matches!(value,"journal_article"|"preprint") { return Err("paperType 必须是 journal_article 或 preprint".into()); } }
    if let Some(value)=options.status.as_deref() { if !matches!(value,"unread"|"read"|"dismissed") { return Err("status 必须是 unread、read 或 dismissed".into()); } }
    if let Some(value)=options.sort.as_deref() { if !matches!(value,"time"|"relevance") { return Err("sort 必须是 time 或 relevance".into()); } }
    if options.search.as_ref().is_some_and(|value| value.chars().count()>200) { return Err("搜索长度不能超过 200 字符".into()); }
    Ok(options.limit.unwrap_or(10).clamp(1,10))
}

fn decode_cursor(value: Option<&str>) -> Result<Option<(String,String)>,String> {
    let Some(value)=value else { return Ok(None) };
    let Some((created,id))=value.split_once('|') else { return Err("cursor 无效".into()) };
    if created.is_empty() || id.is_empty() { return Err("cursor 无效".into()) }
    Ok(Some((created.to_owned(),id.to_owned())))
}

fn push_feed_filters(builder: &mut QueryBuilder<'_, Sqlite>, options: &PaperFeedListOptions, _cursor: Option<&(String,String)>) {
    for (column,value) in [("ranked_feed.topic_id",&options.topic_id),("ranked_feed.tier",&options.relevance)] {
        if let Some(value)=value { builder.push(" AND ").push(column).push(" = ").push_bind(value.clone()); }
    }
    if let Some(value)=&options.journal_id { builder.push(" AND EXISTS(SELECT 1 FROM papers jp WHERE jp.id=ranked_feed.paper_id AND (jp.journal_id=").push_bind(value.clone()).push(" OR EXISTS(SELECT 1 FROM json_each(jp.metadata_json,'$.crossref_journal_ids') membership WHERE membership.value=").push_bind(value.clone()).push(")))"); }
    if let Some(value)=&options.paper_id { builder.push(" AND ranked_feed.paper_id = ").push_bind(value.clone()); }
    if let Some(value)=&options.status { builder.push(" AND ranked_feed.status = ").push_bind(value.clone()); } else { builder.push(" AND ranked_feed.status <> 'dismissed'"); }
    if let Some(value)=&options.source { builder.push(" AND EXISTS(SELECT 1 FROM paper_source_records psf WHERE psf.paper_id=ranked_feed.paper_id AND psf.source=").push_bind(value.clone()).push(")"); }
    if let Some(value)=&options.paper_type { builder.push(" AND ranked_feed.paper_type = ").push_bind(value.clone()); }
    if let Some(value)=options.search.as_ref().filter(|value|!value.trim().is_empty()) {
        let needle=value.trim().to_lowercase();
        builder.push(" AND (instr(lower(ranked_feed.title),").push_bind(needle.clone()).push(")>0 OR instr(lower(coalesce(ranked_feed.doi,'')),").push_bind(needle.clone()).push(")>0 OR instr(lower(coalesce(ranked_feed.arxiv_id,'')),").push_bind(needle.clone()).push(")>0 OR EXISTS (SELECT 1 FROM paper_authors sa WHERE sa.paper_id=ranked_feed.paper_id AND instr(lower(sa.display_name),").push_bind(needle.clone()).push(")>0) OR EXISTS (SELECT 1 FROM json_each(ranked_feed.reason_json,'$.matched') mt WHERE instr(lower(json_extract(mt.value,'$.phrase')),").push_bind(needle.clone()).push(")>0) OR instr(lower(coalesce(ranked_feed.abstract_text,'')),").push_bind(needle).push(")>0)");
    }
}

fn push_feed_cursor(builder:&mut QueryBuilder<'_,Sqlite>,_options:&PaperFeedListOptions,cursor:Option<&(String,String)>){
 if let Some((published,id))=cursor{builder.push(" AND (ranked_feed.published_at < ").push_bind(published.clone()).push(" OR (ranked_feed.published_at = ").push_bind(published.clone()).push(" AND ranked_feed.push_item_id < ").push_bind(id.clone()).push("))");}
}

fn feed_select() -> &'static str {
    "SELECT pi.id AS push_item_id,p.id AS paper_id,pi.topic_id,p.title,NULLIF(COALESCE(NULLIF(p.abstract_text,''),NULLIF(p.abstract,'')),'') AS abstract_text,COALESCE(NULLIF(p.abstract_status,''),CASE WHEN NULLIF(COALESCE(NULLIF(p.abstract_text,''),NULLIF(p.abstract,'')),'') IS NULL THEN 'missing' ELSE 'available' END) AS abstract_status,COALESCE((SELECT json_group_array(display_name) FROM (SELECT pa.display_name FROM paper_authors pa WHERE pa.paper_id=p.id ORDER BY pa.author_index)),'[]') AS authors_json,CASE WHEN EXISTS (SELECT 1 FROM paper_source_records pst WHERE pst.paper_id=p.id AND pst.source='crossref') THEN 'journal_article' ELSE 'preprint' END AS paper_type,COALESCE(p.primary_source,(SELECT ps.source FROM paper_source_records ps WHERE ps.paper_id=p.id ORDER BY ps.fetched_at LIMIT 1),'arxiv') AS source,j.name AS journal_name,j.short_name AS journal_short_name,p.journal_id AS journal_id,p.published_at,COALESCE(p.first_seen_at,p.created_at) AS first_seen_at,NULLIF(p.doi,'') AS doi,NULLIF(p.arxiv_id,'') AS arxiv_id,p.article_url,pm.score AS score,pm.tier,pm.reason_json,pm.algorithm_version,pi.status,pi.created_at FROM push_items pi JOIN papers p ON p.id=pi.paper_id JOIN paper_matches pm ON pm.paper_id=pi.paper_id AND pm.topic_id=pi.topic_id JOIN topics active_topic ON active_topic.id=pi.topic_id AND active_topic.enabled=1 LEFT JOIN journals j ON j.id=p.journal_id WHERE pm.tier IN ('high','medium') AND COALESCE(json_extract(pm.reason_json,'$.configured_keywords'),0)>=3 AND date(p.published_at)>=date('now','-14 days') AND date(p.published_at)<=date('now') AND EXISTS(SELECT 1 FROM journal_feed_batch b WHERE b.push_item_id=pi.id) AND (NOT EXISTS (SELECT 1 FROM paper_source_records psr WHERE psr.paper_id=p.id AND psr.source='crossref') OR EXISTS (SELECT 1 FROM paper_source_records psa WHERE psa.paper_id=p.id AND psa.source='arxiv') OR EXISTS (SELECT 1 FROM topic_journal_subscriptions tjs JOIN journal_subscriptions js ON js.id=tjs.subscription_id AND js.enabled=1 AND js.sync_enabled=1 WHERE tjs.topic_id=pi.topic_id AND tjs.enabled=1 AND (js.journal_id=p.journal_id OR EXISTS(SELECT 1 FROM json_each(p.metadata_json,'$.crossref_journal_ids') membership WHERE membership.value=js.journal_id))))"
}

fn filtered_feed(options: &PaperFeedListOptions) -> QueryBuilder<'static, Sqlite> {
    let mut builder=QueryBuilder::<Sqlite>::new("WITH filtered AS (SELECT * FROM (");
    builder.push(feed_select()).push(") AS ranked_feed WHERE 1=1");
    push_feed_filters(&mut builder,options,None);
    builder.push("), deduplicated AS (SELECT *,ROW_NUMBER() OVER (PARTITION BY paper_id ORDER BY score DESC,created_at DESC,push_item_id DESC) AS paper_rank FROM filtered) ");
    builder
}

pub(crate) async fn query_feed_stats(pool: &SqlitePool, options: &PaperFeedListOptions) -> Result<FeedStatsRow,String> {
    validate_feed_options(options)?;
    let mut builder=filtered_feed(options);
    builder.push("SELECT COUNT(*) AS total,COALESCE(SUM(status='unread'),0) AS unread,COALESCE(SUM(tier='high'),0) AS high,COALESCE(SUM(tier='medium'),0) AS medium,COALESCE(SUM(paper_type='journal_article'),0) AS journal_articles,COALESCE(SUM(paper_type='preprint'),0) AS preprints FROM deduplicated WHERE paper_rank=1");
    builder.build_query_as::<FeedStatsRow>().fetch_one(pool).await.map_err(|e|e.to_string())
}

fn to_feed_item(row: FeedRow) -> Result<PaperFeedItem,String> {
    let article_url=canonical_article_url(row.article_url.as_deref(),row.doi.as_deref(),row.arxiv_id.as_deref()).ok_or_else(||format!("推送论文 {} 缺少合法 HTTPS 原文链接",row.paper_id))?;
    Ok(PaperFeedItem { push_item_id:row.push_item_id,paper_id:row.paper_id,topic_id:row.topic_id,title:strip_markup(&row.title),abstract_text:row.abstract_text,abstract_status:row.abstract_status,authors:serde_json::from_str(&row.authors_json).unwrap_or_default(),paper_type:row.paper_type,source:row.source,journal_name:row.journal_name,journal_short_name:row.journal_short_name,journal_icon:None,published_at:row.published_at,first_seen_at:row.first_seen_at,doi:row.doi.clone(),arxiv_id:row.arxiv_id.clone(),article_url,arxiv_url:arxiv_url(row.arxiv_id.as_deref()),score:row.score,relevance_level:row.tier,matched_terms:matched_terms(&row.reason_json),algorithm_version:row.algorithm_version,status:row.status,created_at:row.created_at})
}

pub(crate) async fn list_feed(pool: &SqlitePool, options: PaperFeedListOptions) -> Result<PaperFeedPage,String> {
    let limit=validate_feed_options(&options)?; let cursor=decode_cursor(options.cursor.as_deref())?; let stats=query_feed_stats(pool,&options).await?;
    let mut builder=filtered_feed(&options); builder.push("SELECT * FROM deduplicated AS ranked_feed WHERE paper_rank=1");
    let cursor_options=PaperFeedListOptions::default();
    push_feed_cursor(&mut builder,&cursor_options,cursor.as_ref()); builder.push(" ORDER BY ranked_feed.published_at DESC,ranked_feed.push_item_id DESC LIMIT ").push_bind((limit+1) as i64);
    let mut rows:Vec<FeedRow>=builder.build_query_as().fetch_all(pool).await.map_err(|e|e.to_string())?;
    let has_more=rows.len()>limit as usize; if has_more { rows.truncate(limit as usize); }
    let next_cursor=if has_more {rows.last().map(|row|format!("{}|{}",row.published_at.clone().unwrap_or_default(),row.push_item_id))}else{None};
    Ok(PaperFeedPage {items:rows.into_iter().map(to_feed_item).collect::<Result<Vec<_>,_>>()?,total:stats.total,unread_count:stats.unread,next_cursor})
}



pub(crate) async fn sync_sources(pool: &SqlitePool, source: &str) -> Result<PaperSyncRun,String> {
    if !["all","arxiv","crossref"].contains(&source) { return Err("source 必须是 arxiv、crossref 或 all".into()); }
    let lock=SYNC_MUTEX.get_or_init(||Arc::new(Mutex::new(()))).clone();
    let _guard=lock.lock().await;
    if source!="all" { let run = run_source(pool,source).await?;  return Ok(run); }
    let failed_run = |source: &str, error: String| {
        let now = Utc::now().to_rfc3339();
        PaperSyncRun { source:source.into(), status:"failed".into(), queries:0, fetched:0, parsed:0, inserted:0, updated:0, matched:0, pushed:0, started_at:now.clone(), finished_at:now, error:Some(error) }
    };
    // Always attempt the other enabled source, including a database/provider
    // setup error that prevents the first source from returning a run record.
    let mut first=run_source(pool,"arxiv").await.unwrap_or_else(|error| failed_run("arxiv", error));
    let second=run_source(pool,"crossref").await.unwrap_or_else(|error| failed_run("crossref", error));
    let first_error=first.error.clone(); let second_error=second.error.clone();
    first.source="all".into(); first.queries+=second.queries;first.fetched+=second.fetched;first.parsed+=second.parsed;
    first.inserted+=second.inserted;first.updated+=second.updated;first.matched+=second.matched;first.pushed+=second.pushed;first.finished_at=second.finished_at;
    first.status=if first.status=="not_configured" && second.status=="not_configured" {"not_configured".into()}
        else if matches!(first.status.as_str(),"success"|"not_configured") && matches!(second.status.as_str(),"success"|"not_configured") {"success".into()}
        else if first.status=="success"||second.status=="success"||first.status=="partial_success"||second.status=="partial_success" {"partial_success".into()}
        else {"failed".into()};
    first.error=match (first_error,second_error) {(Some(a),Some(b))=>Some(format!("arXiv: {a}; Crossref: {b}")),(Some(a),None)=>Some(format!("arXiv: {a}")),(None,Some(b))=>Some(format!("Crossref: {b}")),(None,None)=>None};
    if first.status=="not_configured" { first.error=Some("没有启用且有效的 SCI 来源或期刊订阅".into()); }
    
    Ok(first)
}

pub(crate) async fn query_catalog(pool: &SqlitePool) -> Result<PaperFeedCatalog,String> {
    let topics=sqlx::query_as::<_,(String,String,i64,i64)>("SELECT t.id,t.name,COUNT(pi.id),t.enabled FROM topics t LEFT JOIN push_items pi ON pi.topic_id=t.id AND pi.status<>'dismissed' GROUP BY t.id ORDER BY t.name").fetch_all(pool).await.map_err(|e|e.to_string())?.into_iter().map(|(id,name,match_count,enabled)|PaperFeedTopic{id,name,match_count,enabled:enabled!=0}).collect();
    let journals=sqlx::query_as::<_,(String,String,String,i64)>("SELECT j.id,j.name,j.short_name,COUNT(DISTINCT CASE WHEN active_topic.id IS NOT NULL THEN pi.paper_id END) FROM journals j JOIN journal_subscriptions js ON js.journal_id=j.id AND js.enabled=1 AND js.sync_enabled=1 LEFT JOIN papers p ON p.journal_id=j.id LEFT JOIN push_items pi ON pi.paper_id=p.id AND pi.status<>'dismissed' LEFT JOIN topics active_topic ON active_topic.id=pi.topic_id AND active_topic.enabled=1 GROUP BY j.id ORDER BY j.name").fetch_all(pool).await.map_err(|e|e.to_string())?.into_iter().map(|(id,name,short_name,article_count)|PaperFeedJournal{id,name,short_name,article_count}).collect();
    Ok(PaperFeedCatalog{topics,journals})
}

fn validate_topic_input(input: &PaperTopicInput) -> Result<(String, Vec<(String, f64, String, String, i64, String)>), String> {
    let name = clean_text(&input.name);
    if name.is_empty() || name.chars().count() > 120 { return Err("主题名称不能为空且不能超过 120 个字符".into()); }
    let logic = input.match_logic.as_deref().unwrap_or("any");
    if !matches!(logic, "all" | "any") { return Err("主题匹配逻辑无效".into()); }
    let mut keywords = Vec::new();
    let mut seen = HashSet::new();
    for keyword in &input.keywords {
        let phrase = normalize_phrase(&keyword.phrase);
        if phrase.is_empty() { continue; }
        if keyword.weight <= 0.0 || !keyword.weight.is_finite() { return Err(format!("关键词“{}”的权重必须大于 0", phrase)); }
        let field = keyword.field.as_deref().unwrap_or("both");
        if !matches!(field, "title" | "abstract" | "both") { return Err(format!("关键词“{}”的字段无效", phrase)); }
        if !matches!(keyword.kind.as_str(), "required" | "boost" | "exclude") { return Err(format!("关键词“{}”的类型无效", phrase)); }
        if !seen.insert(phrase.clone()) { return Err(format!("关键词“{}”重复", phrase)); }
        keywords.push((phrase, keyword.weight, field.to_owned(), keyword.kind.clone(), if keyword.enabled.unwrap_or(true) { 1 } else { 0 }, keyword.notes.clone().unwrap_or_default()));
    }
    if input.enabled.unwrap_or(true)&&keywords.iter().filter(|k|k.3!="exclude"&&k.4!=0).map(|k|keyword_identity(&k.0)).collect::<HashSet<_>>().len()<3{return Err("每个启用主题至少需要 3 个不同的正向关键词；排除词、停用词和单复数重复词不计入数量".into())}
    Ok((logic.to_owned(), keywords))
}

fn normalize_issn(value: Option<&str>, label: &str) -> Result<Option<String>, String> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else { return Ok(None) };
    let compact = raw.chars().filter(|character| *character != '-').collect::<String>();
    if compact.chars().count() != 8 || !compact.chars().enumerate().all(|(index, character)| {
        character.is_ascii_digit() || (index == 7 && matches!(character, 'x' | 'X'))
    }) {
        return Err(format!("{label} 格式无效，应为 XXXX-XXXX"));
    }
    let digits = compact.chars().map(|character| if character == 'x' || character == 'X' { 10 } else { character.to_digit(10).unwrap_or(99) as i32 }).collect::<Vec<_>>();
    let checksum: i32 = digits.iter().enumerate().map(|(index, digit)| digit * (8 - index as i32)).sum();
    if checksum % 11 != 0 { return Err(format!("{label} 校验位无效")); }
    Ok(Some(format!("{}-{}", &compact[..4], &compact[4..]).to_uppercase()))
}

fn validate_journal_input(input: &PaperJournalInput) -> Result<(String, String, Option<String>, Option<String>, Option<String>, Option<String>), String> {
    let name = clean_text(&input.name);
    if name.is_empty() || name.chars().count() > 200 { return Err("期刊名称不能为空且不能超过 200 个字符".into()); }
    let print_issn = normalize_issn(input.issn_print.as_deref(), "Print ISSN")?;
    let online_issn = normalize_issn(input.issn_online.as_deref(), "Online ISSN")?;
    if print_issn.is_none() && online_issn.is_none() { return Err("至少填写 Print ISSN 或 Online ISSN".into()); }
    let website = input.website_url.as_deref().map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned).ok_or_else(|| "请填写官方网站 HTTPS URL".to_owned())?;
    let parsed = Url::parse(&website).map_err(|_| "官方网站必须是有效的 HTTPS URL".to_owned())?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() { return Err("官方网站必须使用 HTTPS".into()); }
    let short_name = input.short_name.as_deref().map(clean_text).filter(|value| !value.is_empty()).unwrap_or_else(|| {
        name.split_whitespace().filter_map(|word| word.chars().next()).collect::<String>().to_uppercase()
    });
    let short_name = if short_name.is_empty() { name.chars().take(20).collect() } else { short_name.chars().take(40).collect() };
    Ok((name, short_name, print_issn, online_issn, input.publisher.as_deref().map(clean_text).filter(|value| !value.is_empty()), Some(website)))
}

pub(super) async fn read_journal(pool: &SqlitePool, journal_id: &str) -> Result<PaperJournal, String> {
    let row = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<String>, Option<String>, String, String)>(
        "SELECT id,name,short_name,issn_print,issn_online,publisher,website_url,created_at,updated_at FROM journals WHERE id=?"
    ).bind(journal_id).fetch_optional(pool).await.map_err(|error| error.to_string())?.ok_or_else(|| "期刊不存在".to_owned())?;
    let subscription = sqlx::query_as::<_, (String, i64, i64, String, Option<String>, Option<String>, Option<String>, Option<String>, i64, i64, i64, i64, i64, i64, i64)>(
        "SELECT id,enabled,sync_enabled,last_status,last_success_at,last_attempt_at,last_error,last_error_type,last_query_count,last_fetched_count,last_parsed_count,last_inserted_count,last_updated_count,last_matched_count,last_pushed_count FROM journal_subscriptions WHERE journal_id=?"
    ).bind(journal_id).fetch_optional(pool).await.map_err(|error| error.to_string())?.map(|row| PaperJournalSubscription {
        id: row.0, enabled: row.1 != 0, sync_enabled: row.2 != 0, last_status: row.3,
        last_success_at: row.4, last_attempt_at: row.5, last_error: row.6, last_error_type: row.7,
        last_query_count: row.8, last_fetched_count: row.9, last_parsed_count: row.10,
        last_inserted_count: row.11, last_updated_count: row.12, last_matched_count: row.13,
        last_pushed_count: row.14,
    });
    let topic_ids = if let Some(subscription) = subscription.as_ref() {
        sqlx::query_scalar::<_, String>("SELECT topic_id FROM topic_journal_subscriptions WHERE subscription_id=? AND enabled=1 ORDER BY topic_id").bind(&subscription.id).fetch_all(pool).await.map_err(|error| error.to_string())?
    } else { Vec::new() };
    let series:String=sqlx::query_scalar("SELECT series FROM journals WHERE id=?").bind(journal_id).fetch_one(pool).await.map_err(|e|e.to_string())?;
    Ok(PaperJournal { series,id: row.0, name: row.1, short_name: row.2, issn_print: row.3, issn_online: row.4, publisher: row.5, website_url: row.6, subscription, topic_ids, created_at: row.7, updated_at: row.8 })
}

pub(crate) async fn list_journals(pool: &SqlitePool) -> Result<Vec<PaperJournal>, String> {
    let ids = sqlx::query_scalar::<_, String>("SELECT id FROM journals WHERE EXISTS(SELECT 1 FROM journal_subscriptions js WHERE js.journal_id=journals.id) ORDER BY name COLLATE NOCASE").fetch_all(pool).await.map_err(|error| error.to_string())?;
    let mut result = Vec::with_capacity(ids.len());
    for id in ids { result.push(read_journal(pool, &id).await?); }
    Ok(result)
}

pub(crate) async fn save_journal(pool: &SqlitePool, journal_id: Option<&str>, input: PaperJournalInput) -> Result<PaperJournal, String> {
    let series=input.series.as_deref().map(str::trim).filter(|s|!s.is_empty()).map(str::to_owned);
    if series.as_ref().is_some_and(|s|s.chars().count()>80||s.chars().any(char::is_control)){return Err("系列名称不能超过 80 个字符或包含控制字符".into())}
    let (name, requested_short_name, print_issn, online_issn, publisher, website_url) = validate_journal_input(&input)?;
    let archived:Option<String>=if journal_id.is_none(){sqlx::query_scalar("SELECT j.id FROM journals j WHERE (j.name=? COLLATE NOCASE OR j.issn_print IN (?,?) OR j.issn_online IN (?,?)) LIMIT 1").bind(&name).bind(&print_issn).bind(&online_issn).bind(&print_issn).bind(&online_issn).fetch_optional(pool).await.map_err(|e|e.to_string())?}else{None};
    if let Some(id)=archived.as_deref(){let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM journal_subscriptions WHERE journal_id=?)").bind(id).fetch_one(pool).await.map_err(|e|e.to_string())?;if exists{return Err("期刊已在我的期刊中，请选择它进行编辑".into())}}
    let journal_id=journal_id.or(archived.as_deref());
    let id = journal_id.map(str::to_owned).unwrap_or_else(|| format!("journal-{}", uuid::Uuid::new_v4()));
    let mut short_name = requested_short_name;
    let short_name_base = short_name.clone();
    let mut suffix = 2u32;
    loop {
        let existing = sqlx::query_scalar::<_, String>("SELECT id FROM journals WHERE short_name=? LIMIT 1").bind(&short_name).fetch_optional(pool).await.map_err(|error| error.to_string())?;
        if existing.is_none() || existing.as_deref() == Some(id.as_str()) { break; }
        short_name = format!("{short_name_base}{suffix}"); suffix += 1;
    }
    if let Some(existing) = sqlx::query_scalar::<_, String>("SELECT id FROM journals WHERE (issn_print IS NOT NULL AND issn_print IN (?,?)) OR (issn_online IS NOT NULL AND issn_online IN (?,?)) LIMIT 1").bind(print_issn.clone()).bind(online_issn.clone()).bind(print_issn.clone()).bind(online_issn.clone()).fetch_optional(pool).await.map_err(|error| error.to_string())? {
        if existing != id { return Err("ISSN 已被其它期刊使用".into()); }
    }
    let mut tx=pool.begin().await.map_err(|e|e.to_string())?;
    let result = if journal_id.is_some() {
        sqlx::query("UPDATE journals SET name=?,short_name=?,issn_print=?,issn_online=?,publisher=?,website_url=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?").bind(&name).bind(&short_name).bind(print_issn).bind(online_issn).bind(publisher).bind(website_url).bind(&id).execute(&mut *tx).await.map_err(|error| error.to_string())?
    } else {
        sqlx::query("INSERT INTO journals(id,name,short_name,issn_print,issn_online,publisher,website_url,metadata_status,metadata_json,updated_at) VALUES(?,?,?,?,?,?,?,'local','{}',strftime('%Y-%m-%dT%H:%M:%fZ','now'))").bind(&id).bind(&name).bind(&short_name).bind(print_issn).bind(online_issn).bind(publisher).bind(website_url).execute(&mut *tx).await.map_err(|error| error.to_string())?
    };
    if journal_id.is_some() && result.rows_affected() == 0 { return Err("期刊不存在".into()); }
    if let Some(series)=series{sqlx::query("UPDATE journals SET series=? WHERE id=?").bind(series).bind(&id).execute(&mut *tx).await.map_err(|e|e.to_string())?;}
    sqlx::query("INSERT OR IGNORE INTO journal_subscriptions(id,journal_id,enabled,sync_enabled) VALUES(?,?,1,1)").bind(format!("subscription-{id}")).bind(&id).execute(&mut *tx).await.map_err(|e|e.to_string())?;
    sqlx::query("INSERT OR IGNORE INTO topic_journal_subscriptions(topic_id,subscription_id,enabled) SELECT t.id,js.id,1 FROM topics t JOIN journal_subscriptions js ON js.journal_id=? WHERE t.enabled=1 AND js.enabled=1").bind(&id).execute(&mut *tx).await.map_err(|e|e.to_string())?;
    tx.commit().await.map_err(|e|e.to_string())?;
    read_journal(pool, &id).await
}

pub(crate) async fn set_journal_subscription(pool: &SqlitePool, journal_id: &str, enabled: bool) -> Result<PaperJournal, String> {
    sqlx::query("SELECT id FROM journals WHERE id=?").bind(journal_id).fetch_optional(pool).await.map_err(|error| error.to_string())?.ok_or_else(|| "期刊不存在".to_owned())?;
    if enabled {
        sqlx::query("INSERT INTO journal_subscriptions(id,journal_id,enabled,sync_enabled) VALUES(?,?,1,1) ON CONFLICT(journal_id) DO UPDATE SET enabled=1,sync_enabled=1,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')").bind(format!("subscription-{journal_id}")).bind(journal_id).execute(pool).await.map_err(|error| error.to_string())?;
    } else {
        sqlx::query("UPDATE journal_subscriptions SET enabled=0,sync_enabled=0,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE journal_id=?").bind(journal_id).execute(pool).await.map_err(|error| error.to_string())?;
    }
    read_journal(pool, journal_id).await
}

pub(crate) async fn cancel_journal_subscription(pool: &SqlitePool, journal_id: &str) -> Result<PaperJournal, String> {
    sqlx::query("DELETE FROM journal_subscriptions WHERE journal_id=?").bind(journal_id).execute(pool).await.map_err(|error| error.to_string())?;
    read_journal(pool, journal_id).await
}

pub(crate) async fn set_journal_topics(pool: &SqlitePool, journal_id: &str, topic_ids: Vec<String>) -> Result<PaperJournal, String> {
    let subscription_id = sqlx::query_scalar::<_, String>("SELECT id FROM journal_subscriptions WHERE journal_id=?").bind(journal_id).fetch_optional(pool).await.map_err(|error| error.to_string())?.ok_or_else(|| "请先启用期刊订阅".to_owned())?;
    let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
    sqlx::query("DELETE FROM topic_journal_subscriptions WHERE subscription_id=?").bind(&subscription_id).execute(&mut *tx).await.map_err(|error| error.to_string())?;
    for topic_id in topic_ids {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM topics WHERE id=?)").bind(&topic_id).fetch_one(&mut *tx).await.map_err(|error| error.to_string())?;
        if !exists { return Err(format!("主题不存在: {topic_id}")); }
        sqlx::query("INSERT INTO topic_journal_subscriptions(topic_id,subscription_id,enabled) VALUES(?,?,1)").bind(topic_id).bind(&subscription_id).execute(&mut *tx).await.map_err(|error| error.to_string())?;
    }
    let paper_ids:Vec<String>=sqlx::query_scalar("SELECT id FROM papers ORDER BY id").fetch_all(&mut *tx).await.map_err(|e|e.to_string())?;
    for paper_id in paper_ids {match_paper(&mut tx,&paper_id).await?;}
    tx.commit().await.map_err(|error| error.to_string())?;
    read_journal(pool, journal_id).await
}

pub(super) async fn read_topic(pool: &SqlitePool, topic_id: &str) -> Result<PaperTopic, String> {
    let row = sqlx::query_as::<_, (String, String, String, i64, String, String, String, String)>(
        "SELECT id,name,COALESCE(description,''),enabled,COALESCE(match_logic,'any'),COALESCE(match_algorithm_version,'keyword-v2'),created_at,updated_at FROM topics WHERE id=?"
    ).bind(topic_id).fetch_optional(pool).await.map_err(|e| e.to_string())?
        .ok_or_else(|| "主题不存在".to_owned())?;
    let rows = sqlx::query_as::<_, (String, f64, String, String, i64, String)>(
        "SELECT phrase,weight,field,kind,enabled,notes FROM topic_keywords WHERE topic_id=? ORDER BY kind,phrase"
    ).bind(topic_id).fetch_all(pool).await.map_err(|e| e.to_string())?;
    Ok(PaperTopic { id: row.0, name: row.1, description: row.2, enabled: row.3 != 0, match_logic: row.4, match_algorithm_version: row.5, created_at: row.6, updated_at: row.7,
        keywords: rows.into_iter().map(|r| PaperTopicKeyword { phrase: r.0, weight: r.1, field: r.2, kind: r.3, enabled: r.4 != 0, notes: r.5 }).collect() })
}

pub(super) async fn save_topic(pool: &SqlitePool, topic_id: Option<&str>, input: PaperTopicInput) -> Result<PaperTopic, String> {
    let name = clean_text(&input.name);
    let (logic, keywords) = validate_topic_input(&input)?;
    let id = topic_id.map(str::to_owned).unwrap_or_else(|| format!("topic-{}", uuid::Uuid::new_v4()));
    let description = input.description.unwrap_or_default().trim().to_owned();
    let enabled = if input.enabled.unwrap_or(true) { 1 } else { 0 };
    let required_terms = serde_json::to_string(&keywords.iter().filter(|item| item.3 == "required" && item.4 != 0).map(|item| item.0.clone()).collect::<Vec<_>>()).map_err(|e| e.to_string())?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    if topic_id.is_some() {
        let changed = sqlx::query("UPDATE topics SET name=?,description=?,enabled=?,match_logic=?,required_terms_json=?,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?")
            .bind(&name).bind(&description).bind(enabled).bind(&logic).bind(&required_terms).bind(&id).execute(&mut *tx).await.map_err(|e| e.to_string())?.rows_affected();
        if changed == 0 { return Err("主题不存在".into()); }
        sqlx::query("DELETE FROM topic_keywords WHERE topic_id=?").bind(&id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    } else {
        sqlx::query("INSERT INTO topics(id,name,description,enabled,match_logic,match_algorithm_version,required_terms_json) VALUES(?,?,?,?,?,'keyword-v2',?)")
            .bind(&id).bind(&name).bind(&description).bind(enabled).bind(&logic).bind(&required_terms).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    for (phrase, weight, field, kind, keyword_enabled, notes) in keywords {
        sqlx::query("INSERT INTO topic_keywords(topic_id,phrase,weight,field,kind,enabled,notes) VALUES(?,?,?,?,?,?,?)")
            .bind(&id).bind(phrase).bind(weight).bind(field).bind(kind).bind(keyword_enabled).bind(notes).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    sqlx::query("INSERT OR IGNORE INTO topic_journal_subscriptions(topic_id,subscription_id,enabled) SELECT ?,id,1 FROM journal_subscriptions WHERE enabled=1 AND sync_enabled=1").bind(&id).execute(&mut *tx).await.map_err(|e|e.to_string())?;
    let paper_ids:Vec<String>=sqlx::query_scalar("SELECT id FROM papers ORDER BY id").fetch_all(&mut *tx).await.map_err(|e|e.to_string())?;
    for paper_id in paper_ids {match_paper(&mut tx,&paper_id).await?;}
    tx.commit().await.map_err(|e| e.to_string())?;
    read_topic(pool, &id).await
}


#[cfg(test)]
mod tests;
