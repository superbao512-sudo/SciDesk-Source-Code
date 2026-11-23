use super::*;
use crate::journals::ProjectPaper;
impl Service{
 pub(super) async fn literature(&self,op:&str,args:Value)->Result<Value,String>{
  let _guard=if op=="literature_write"{Some(self.lock().await?)}else{None};
  let project=self.get(&text(&args,"id")?).await?;let relative=args["relative"].as_str().unwrap_or("");let dir=self.resolve(&project,relative)?;if !dir.is_dir(){return Err("保存位置必须是项目内的文件夹".into())}
  let filename=text(&args,"filename")?;validate_name(&filename)?;let format=text(&args,"format")?;if !matches!(format.as_str(),"md"|"json")||!filename.to_lowercase().ends_with(&format!(".{format}")){return Err("文件格式必须与 .md 或 .json 扩展名一致".into())}
  let target=dir.join(&filename);self.protect(&target)?;if target.exists(){return Err("同名文件已存在，请更换文件名；原文件未覆盖".into())}
  if op=="literature_prepare"{return Ok(json!({"path":display(&target)}))}
  let records:Vec<ProjectPaper>=serde_json::from_value(args["records"].clone()).map_err(err)?;if records.len()>200{return Err("单次最多保存 200 篇文献基础信息".into())}
  for row in &records{let url=url::Url::parse(&row.url).map_err(|_|"文献链接无效")?;if row.title.trim().is_empty()||row.title.chars().count()>500||row.authors.len()>5000||!matches!(row.source.as_str(),"crossref"|"arxiv")||url.scheme()!="https"||!url.username().is_empty()||url.password().is_some()||!matches!(url.host_str(),Some("doi.org"|"arxiv.org")){return Err("文献基础信息或原文链接无效".into())}}
  let now=chrono::Utc::now().to_rfc3339();let report=json!({"schemaVersion":1,"createdAt":now,"projectName":project.name,"query":args["query"],"warnings":args["warnings"],"count":records.len(),"records":records});
  let bytes=if format=="json"{serde_json::to_vec_pretty(&report).map_err(err)?}else{markdown(&project.name,&now,&args["query"],&args["warnings"],&records).into_bytes()};if bytes.len()>4*1024*1024{return Err("检索结果文件超过 4 MB，请减少篇数".into())}
  let temporary=dir.join(format!(".scidesk-literature-{}.tmp",token()));
  let result=(||{let mut f=fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(err)?;f.write_all(&bytes).and_then(|_|f.sync_all()).map_err(err)?;drop(f);project_platform::rename_no_replace(&temporary,&target)})();
  if let Err(e)=result{let _=fs::remove_file(&temporary);return Err(format!("保存失败，原文件未覆盖：{e}"))}
  Ok(json!({"path":display(&target),"relative":if relative.is_empty(){filename}else{format!("{relative}/{filename}")},"count":records.len(),"bytes":bytes.len()}))
 }
}
fn escape(s:&str)->String{s.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace(['\r','\n']," ").replace('\\',"\\\\").replace('[',"\\[").replace(']',"\\]").replace('*',"\\*").replace('_',"\\_").replace('`',"\\`").replace('#',"\\#")}
fn markdown(project:&str,now:&str,query:&Value,warnings:&Value,rows:&[ProjectPaper])->String{
 let mut text=format!("# 项目文献检索\n\n项目：{}\n\n检索时间：{}\n\n实际篇数：{}\n\n## 检索条件\n\n",escape(project),now,rows.len());
 for (label,key) in [("关键词","keywords"),("来源","sourceNames"),("起始年份","fromYear"),("结束年份","toYear"),("目标篇数上限","limit"),("匹配方式","matchLogic")]{let value=&query[key];let value=if let Some(a)=value.as_array(){a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("；")}else if let Some(s)=value.as_str(){s.to_string()}else{value.to_string()};text.push_str(&format!("- {label}：{}\n",escape(&value)));}
 if let Some(w)=warnings.as_array().filter(|a|!a.is_empty()){text.push_str("\n## 未完成来源与提示\n\n");for item in w{text.push_str(&format!("- {}\n",escape(item.as_str().unwrap_or(""))))}}
 text.push_str("\n## 文献基础信息\n\n");if rows.is_empty(){text.push_str("未找到符合本次条件的文献。\n")}
 for (i,row) in rows.iter().enumerate(){text.push_str(&format!("### {}. {}\n\n- 作者：{}\n- 期刊 / 来源：{}\n- 发表日期：{}\n- DOI：{}\n- arXiv ID：{}\n- 来源：{}\n- 命中关键词：{}\n- 原文：<{}>\n\n",i+1,escape(&row.title),escape(&row.authors.join(", ")),escape(row.journal.as_deref().unwrap_or("arXiv")),escape(row.published_at.as_deref().unwrap_or("来源未提供日期")),escape(row.doi.as_deref().unwrap_or("来源未提供")),escape(row.arxiv_id.as_deref().unwrap_or("无")),escape(&row.source),escape(&row.matched_keywords.join("；")),row.url.replace(['<','>'],"")));}text
}
