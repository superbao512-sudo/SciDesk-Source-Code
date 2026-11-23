use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest,Sha256};
use std::io::{Read,Write};

fn check_id(value:&str)->Result<(),String>{
 if value.len()!=64 || !value.bytes().all(|c|c.is_ascii_hexdigit()){return Err("PDF 来源标识无效".into())}
 Ok(())
}
fn read_limited(path:&Path,limit:u64)->Result<Vec<u8>,String>{
 let file=fs::File::open(path).map_err(err)?;
 let meta=file.metadata().map_err(err)?;
 if !meta.is_file()||meta.len()>limit{return Err(format!("文件类型无效或超过 {} MB",limit/1048576))}
 let mut data=Vec::new();file.take(limit+1).read_to_end(&mut data).map_err(err)?;
 if data.len() as u64>limit{return Err("文件超过大小限制".into())}Ok(data)
}
fn publish(path:&Path,bytes:&[u8],replace:bool)->Result<(),String>{
 let tmp=path.with_file_name(format!(".{}.tmp",uuid::Uuid::new_v4()));
 let mut f=fs::OpenOptions::new().create_new(true).write(true).open(&tmp).map_err(err)?;
 let result=(||{f.write_all(bytes).and_then(|_|f.sync_all()).map_err(err)?;drop(f);
  if replace{crate::project_platform::replace_record(&tmp,path)}else{crate::project_platform::rename_no_replace(&tmp,path)}
 })();
 if result.is_err(){let _=fs::remove_file(tmp);}result
}
impl Service {
 pub(super) async fn research(&self,op:&str,args:Value)->Result<Value,String>{
  let data=self.journal.parent().and_then(Path::parent).ok_or("工具目录无效")?;
  let reader=data.join("tools/pdf-reader");
  if op=="export_plot"{
   let format=text(&args,"format")?;
   let bytes=match format.as_str(){
    "svg"=>text(&args,"data")?.into_bytes(),
    "png"=>STANDARD.decode(text(&args,"data")?).map_err(err)?,
    _=>return Err("支持 SVG 和 PNG".into())
   };
   if bytes.len()>20*1024*1024{return Err("图形文件超过 20 MB".into())}
   let dir=data.join("Exports/QuickPlots");crate::personal_seed::prepare_project_directory(&dir)?;
   let path=dir.join(format!("曲线-{}-{}.{}",chrono::Local::now().format("%Y%m%d-%H%M%S"),&uuid::Uuid::new_v4().simple().to_string()[..8],format));
   publish(&path,&bytes,false)?;return Ok(json!(display(&path)))
  }
  if op=="note"{
   let key=text(&args,"key")?;check_id(&key)?;
   let dir=reader.join(key);if !dir.join("source.json").is_file(){return Err("请先打开原始 PDF".into())}
   let note=&args["note"];let num=note["page"].as_u64().filter(|&n|n>0).ok_or("页码无效")?;
   let quote=text(note,"quote")?;let comment=text(note,"comment")?;
   if quote.len()+comment.len()>100_000{return Err("批注超过长度限制".into())}
   let rect=note.get("rect").cloned().unwrap_or(Value::Null);
   if !rect.is_null(){
    let r:Vec<f64>=serde_json::from_value(rect.clone()).map_err(err)?;
    if r.len()!=4||r.iter().any(|v|!(0.0..=1.0).contains(v))||r[2]<=0.0||r[3]<=0.0||r[0]+r[2]>1.000001||r[1]+r[3]>1.000001{return Err("选区无效".into())}
   }
   if quote.trim().is_empty()&&comment.trim().is_empty()&&rect.is_null(){return Err("请先选择原文、框选插图或输入批注".into())}
   let id=uuid::Uuid::new_v4().to_string();
   let item=json!({"id":id,"page":num,"quote":quote,"comment":comment,"rect":rect});
   publish(&dir.join(format!("{id}.note.json")),&serde_json::to_vec(&item).map_err(err)?,false)?;return Ok(item)
  }
  if !matches!(op,"plot"|"pdf"){return Err("未知科研工具操作".into())}
  let source=if let Some(key)=args["key"].as_str(){
   if op!="pdf"{return Err("文件类型无效".into())}check_id(key)?;
   let meta:Value=serde_json::from_slice(&read_limited(&reader.join(key).join("source.json"),65536)?).map_err(err)?;
   PathBuf::from(text(&meta,"path")?)
  }else if let Some(id)=args["id"].as_str(){
   self.resolve(&self.get(id).await?,&text(&args,"relative")?)?
  }else if let Some(p)=args["path"].as_str(){PathBuf::from(p)}else{
   let selected=on_thread(crate::project_platform::pick_pdf)?;
   let Some(p)=selected else{return Ok(Value::Null)};PathBuf::from(p)
  };
  let path=crate::file_transfer::Guard::new(self.verification_root.iter().cloned().collect())?.check(&source)?;
  let name=path.file_name().ok_or("缺少文件名")?.to_string_lossy();
  let ext=path.extension().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
  if op=="plot"{
   if !matches!(ext.as_str(),"csv"|"tsv"|"dat"|"txt"){return Err("请选择 TXT、DAT、CSV 或 TSV 数据文件".into())}
   let bytes=read_limited(&path,8*1024*1024)?;let content=String::from_utf8(bytes).map_err(|_|"请将数据文件保存为 UTF-8 编码")?;
   return Ok(json!({"name":name,"text":content}))
  }
  if ext!="pdf"{return Err("请选择 PDF 文件".into())}
  let bytes=read_limited(&path,64*1024*1024)?;
  if !bytes.iter().take(1024).copied().collect::<Vec<_>>().windows(5).any(|s|s==b"%PDF-"){return Err("PDF 文件头无效".into())}
  let key=format!("{:x}",Sha256::digest(&bytes));
  if args["key"].as_str().is_some_and(|old|old!=key){return Err("原 PDF 内容已变化，请重新打开对应版本".into())}
  let dir=reader.join(&key);crate::personal_seed::prepare_project_directory(&dir)?;
  let identity=serde_json::to_vec(&json!({"path":display(&path)})).map_err(err)?;
  publish(&dir.join("source.json"),&identity,true)?;
  let mut notes=Vec::<Value>::new();
  for entry in fs::read_dir(&dir).map_err(err)?{let p=entry.map_err(err)?.path();if p.file_name().unwrap_or_default().to_string_lossy().ends_with(".note.json"){notes.push(serde_json::from_slice(&read_limited(&p,150_000)?).map_err(err)?)}}
  notes.sort_by_key(|n|n["page"].as_u64());
  Ok(json!({"key":key,"name":name,"base64":STANDARD.encode(bytes),"notes":notes}))
 }
}
