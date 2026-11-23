//! Read-only public metadata search shared by the two discovery views.
use super::*;
use super::search::Record;
use chrono::NaiveDate;
use std::time::Duration;

#[derive(Default, Serialize)]
pub(crate) struct Results {
    pub items: Vec<Record>,
    pub warnings: Vec<String>,
    #[serde(skip)]
    pub successes: usize,
}

pub(crate) fn terms(input: &[String]) -> Result<Vec<String>, String> {
    let mut terms = Vec::new();
    for value in input {
        let term = normalize_phrase(value);
        if term.chars().count() > 120 { return Err("每个关键词最多 120 个字符".into()); }
        if !term.is_empty() && !terms.contains(&term) { terms.push(term); }
    }
    if terms.is_empty() || terms.len() > 8 { return Err("请输入 1–8 个关键词或短语".into()); }
    Ok(terms)
}

fn matches(candidates: Vec<Candidate>, terms: &[String], since: NaiveDate, today: NaiveDate) -> Vec<Record> {
    candidates.into_iter().filter_map(|c| {
        let published = c.published_at.as_deref()?.get(..10)?;
        let date = NaiveDate::parse_from_str(published,"%Y-%m-%d").ok()?;
        if date < since || date > today { return None; }
        let content = normalize_phrase(&format!("{} {}", c.title,c.abstract_text.as_deref().unwrap_or("")));
        let matched_keywords:Vec<_> = terms.iter().filter(|term|keyword_hit(&content,term)).cloned().collect();
        if matched_keywords.is_empty() {return None;}
        Some(Record {title:c.title, authors:c.authors.into_iter().map(|a|a.display).collect(), journal:c.journal,
            published_at:Some(published.into()), year:c.year, doi:c.doi, arxiv_id:c.arxiv_id, url:c.article_url?,
            source:c.source, matched_keywords})
    }).collect()
}

async fn crossref(client:&Client, term:&str, since:NaiveDate, today:NaiveDate) -> Result<Vec<Candidate>,String> {
    let mut url=Url::parse("https://api.crossref.org/works").unwrap();
    url.query_pairs_mut().append_pair("query.bibliographic",term)
        .append_pair("filter",&format!("type:journal-article,from-pub-date:{since},until-pub-date:{today}"))
        .append_pair("rows","40")
        .append_pair("select","type,DOI,title,author,container-title,issued,published,published-online,published-print,abstract");
    let body=fetch_with_retry(client,url,&crossref_user_agent()).await?;
    parse_crossref_for_journal(&body,"","")
}

pub(crate) fn merge(items:Vec<Record>) -> Vec<Record> {
    let mut merged:Vec<Record>=Vec::new();
    for row in items {
        let existing=merged.iter_mut().find(|r| r.url==row.url
            || (row.doi.is_some() && row.doi==r.doi)
            || (row.arxiv_id.is_some() && row.arxiv_id==r.arxiv_id)
            || (row.year==r.year && row.authors.first().is_some() && row.authors.first()==r.authors.first() && normalize_phrase(&row.title)==normalize_phrase(&r.title)));
        if let Some(old)=existing {
            let mut hits=old.matched_keywords.clone();
            for term in &row.matched_keywords {if !hits.contains(term){hits.push(term.clone());}}
            if old.source=="arxiv" && row.source=="crossref" {
                let preprint=old.arxiv_id.clone();
                *old=row;
                old.arxiv_id=preprint;
            }
            old.matched_keywords=hits;
        } else { merged.push(row); }
    }
    merged.sort_by(|a,b|b.published_at.cmp(&a.published_at).then_with(||a.title.cmp(&b.title)));
    merged
}

pub(crate) async fn lookup(terms:&[String],days:i64) -> Result<Results,String> {
    let client=Client::builder().connect_timeout(Duration::from_secs(5)).timeout(Duration::from_secs(10)).build().map_err(|e|e.to_string())?;
    let now=Utc::now();
    let since=now-chrono::Duration::days(days);
    let today=now.date_naive();
    let since_date=since.date_naive();
    let journals=async {
        let mut results=Vec::new();
        // Two simultaneous Crossref queries keep UI latency bounded without bursting across all terms.
        for pair in terms.chunks(2) {
            let transport=&client;
            let run=|term| async move {
                let result=tokio::time::timeout(Duration::from_secs(13),crossref(transport,term,since_date,today)).await
                    .map_err(|_|"请求超时".to_string()).and_then(|r|r);
                (format!("Crossref · {term}"), result)
            };
            if pair.len()==2 {
                let (a,b)=tokio::join!(run(&pair[0]),run(&pair[1]));results.extend([a,b]);
            }else{results.push(run(&pair[0]).await);}
        }
        results
    };
    let preprints=async {
        let query=terms.iter().map(|p|format!("all:\"{}\"",p.replace(['"','\\']," "))).collect::<Vec<_>>().join(" OR ");
        let body=tokio::time::timeout(Duration::from_secs(13),fetch_arxiv(&client,&query,Some(&since),0)).await
            .map_err(|_|"请求超时".to_string())??;
        validate_arxiv_response(&body)?;
        Ok::<_,String>(parse_arxiv(&body))
    };
    let (mut responses,preprints)=tokio::join!(journals,preprints);
    responses.push(("arXiv".into(),preprints));
    let mut result=Results::default();
    for (source,response) in responses {
        match response {
            Ok(rows)=>{result.successes+=1; result.items.extend(matches(rows,terms,since_date,today));},
            Err(error)=>result.warnings.push(format!("{source}：{error}")),
        }
    }
    result.items=merge(result.items);
    Ok(result)
}
