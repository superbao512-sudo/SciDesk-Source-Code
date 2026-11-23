use super::*;
use base64::{engine::general_purpose::STANDARD,Engine};
use sha2::{Digest,Sha256};
use std::io::{Read,Write};

#[cfg(test)] mod tests;

const MD_LIMIT:usize=2*1024*1024;
const IMAGE_LIMIT:usize=12*1024*1024;
const ORDER:&str=".scidesk-chapters.json";

fn limited(path:&Path,limit:usize)->Result<Vec<u8>>{
 let mut data=Vec::new();
 fs::File::open(path).map_err(err)?.take(limit as u64+1).read_to_end(&mut data).map_err(err)?;
 if data.len()>limit{return Err(format!("文件过大：{}",display(path)))}
 Ok(data)
}
fn digest(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn atomic(target:&Path,data:&[u8],replace:bool)->Result<()>{
 let temp=target.with_file_name(format!(".scidesk-{}.tmp",uuid::Uuid::new_v4()));
 let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(err)?;
 let outcome=(||{file.write_all(data).and_then(|_|file.sync_all()).map_err(err)?;drop(file);
  if replace{project_platform::replace_record(&temp,target)}else{project_platform::rename_no_replace(&temp,target)}
 })();
 if outcome.is_err(){let _=fs::remove_file(temp);}outcome
}
pub(super) fn markdown(path:&Path)->bool{path.extension().is_some_and(|e|e.eq_ignore_ascii_case("md")||e.eq_ignore_ascii_case("markdown"))}
struct Book{root:PathBuf,single:Option<String>}
impl Book{
 fn new(link:&Link,base:&str)->Result<Self>{
  let path=resolve(link,base)?;crate::file_transfer::Guard::new(vec![])?.check(&path)?;
  if path.is_dir(){return Ok(Self{root:path,single:None})}
  if link.kind!="file"||!markdown(&path){return Err("请选择 Markdown 文件或本地文件夹".into())}
  Ok(Self{root:path.parent().ok_or("无法定位文件夹")?.into(),single:Some(path.file_name().unwrap().to_string_lossy().into())})
 }
 // Check every existing component, including the destination of a replacement.
 fn path(&self,relative:&str)->Result<PathBuf>{
  let mut path=self.root.clone();
  for part in relative.split('/'){
   crate::project_service::validate_name(part)?;
   path.push(part);
   match fs::symlink_metadata(&path){Ok(m) if is_link(&m)=>return Err("不能经过链接或目录联接".into()),Ok(_)=>{},Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},Err(e)=>return Err(err(e))}
  }
  Ok(path)
 }
 fn chapter(&self,s:&str)->Result<PathBuf>{
  if !markdown(Path::new(s))||self.single.as_deref().is_some_and(|only|only!=s){return Err("请选择本手册中的 Markdown 章节".into())}
  self.path(s)
 }
 fn chapters(&self)->Result<Vec<String>>{
  if let Some(s)=&self.single{return Ok(vec![s.clone()])}
  let mut chapters=vec![];let mut pending=vec![(self.root.clone(),0)];let mut count=0;
  while let Some((dir,depth))=pending.pop(){
   if depth>64{return Err("目录层数过多，请选择更具体的手册目录".into())}
   for e in fs::read_dir(dir).map_err(err)?{
    let e=e.map_err(err)?;let path=e.path();let m=fs::symlink_metadata(&path).map_err(err)?;
    count+=1;if count>20000{return Err("目录超过 20000 项，请选择更具体的手册目录".into())}
    let name=e.file_name().to_string_lossy().into_owned();
    if is_link(&m)||name.starts_with('.') {continue}
    if m.is_dir(){if !["assets","node_modules","手册导出"].contains(&name.as_str()){pending.push((path,depth+1));}}
    else if m.is_file()&&markdown(&path){chapters.push(path.strip_prefix(&self.root).map_err(err)?.to_string_lossy().replace('\\',"/"))}
   }
  }
  chapters.sort();
  let file=self.path(ORDER)?;
  if file.exists(){
   let order:Vec<String>=serde_json::from_slice(&limited(&file,2*1024*1024)?).map_err(|_|"章节顺序文件格式无效")?;
   let rank:std::collections::HashMap<_,_>=order.into_iter().enumerate().map(|(i,s)|(s,i)).collect();
   chapters.sort_by_key(|s|rank.get(s).copied().unwrap_or(usize::MAX));
  }
  Ok(chapters)
 }
 fn order(&self,order:&[String])->Result<()>{let file=self.path(ORDER)?;atomic(&file,&serde_json::to_vec_pretty(order).map_err(err)?,file.exists())}
 fn output(&self,extension:&str)->Result<PathBuf>{
  let dir=self.path("手册导出")?;fs::create_dir_all(&dir).map_err(err)?;
  let title=self.single.as_deref().and_then(|s|Path::new(s).file_stem()).or_else(||self.root.file_name()).unwrap_or_default().to_string_lossy();
  Ok(dir.join(format!("{}-{}-{}.{}",title,chrono::Local::now().format("%Y%m%d-%H%M%S"),&uuid::Uuid::new_v4().simple().to_string()[..6],extension)))
 }
}
fn image_format(bytes:&[u8])->Option<(&'static str,&'static str)>{
 if bytes.starts_with(b"\x89PNG\r\n\x1a\n"){Some(("png","image/png"))}
 else if bytes.starts_with(b"\xff\xd8\xff"){Some(("jpg","image/jpeg"))}
 else if bytes.starts_with(b"RIFF")&&bytes.get(8..12)==Some(b"WEBP"){Some(("webp","image/webp"))}
 else if bytes.starts_with(b"GIF87a")||bytes.starts_with(b"GIF89a"){Some(("gif","image/gif"))}
 else{None}
}

impl Store{
 pub(super) async fn create_blank_book(&self, projects: &Path) -> Result<Value> {
  let library = projects.join("教学手册");
  crate::personal_seed::prepare_project_directory(&library)?;
  crate::file_transfer::Guard::new(vec![])?.check(&library)?;
  let mut serial = 1_u32;
  let (name, root) = loop {
   let name = if serial == 1 { "未命名手册".to_string() } else { format!("未命名手册 ({serial})") };
   let candidate = library.join(&name);
   match fs::create_dir(&candidate) {
    Ok(()) => break (name, candidate),
    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => serial += 1,
    Err(error) => return Err(err(error)),
   }
  };
  let chapter = "第一章.md";
  let path = root.join(chapter);
  if let Err(error) = atomic(&path, b"", false) {
   let _ = fs::remove_dir(&root);
   return Err(error)
  }
  let manual = self.run("import", json!({"name":name,"path":display(&root),"kind":"folder"})).await
   .map_err(|error| format!("新文件已保存在 {}，加入手册失败：{error}", display(&path)))?;
  let id = text(&manual, "id")?;
  let link = self.links(&id).await?.into_iter().next().ok_or("新手册关联未找到")?;
  Ok(json!({"scope":{"id":id,"linkId":link.id},"chapter":chapter}))
 }

 pub(super) async fn editor(&self,op:&str,args:Value)->Result<Value>{
  if op=="book_pick"{
   let path=tauri::async_runtime::spawn_blocking(||project_platform::pick(None)).await.map_err(err)??;
   let Some(path)=path else{return Ok(Value::Null)};
   let root=crate::file_transfer::Guard::new(vec![])?.check(Path::new(&path))?;
   let existing:Option<(String,String)>=sqlx::query_as("SELECT manual_id,id FROM links WHERE path_key=? AND kind='folder' ORDER BY rowid LIMIT 1").bind(key(&root)).fetch_optional(&self.pool).await.map_err(err)?;
   if let Some((id,link))=existing{return Ok(json!({"id":id,"linkId":link}))}
   let result=Box::pin(self.run("import",json!({"name":root.file_name().unwrap_or_default().to_string_lossy(),"path":path,"kind":"folder"}))).await?;
   let id=text(&result,"id")?;let links=self.links(&id).await?;
   return Ok(json!({"id":id,"linkId":links[0].id}))
  }
  let id=text(&args,"id")?;let link=self.link(&id,&text(&args,"linkId")?).await?;
  let base=args["base"].as_str().unwrap_or("");
  let book=Book::new(&link,base)?;
  match op{
   "book_list"=>Ok(json!({"chapters":book.chapters()?,"title":book.single.as_deref().unwrap_or_else(||book.root.file_name().and_then(|n|n.to_str()).unwrap_or("教学手册")),"root":display(&book.root),"single":book.single.is_some()})),
   "book_read"|"book_save"=>{
    let path=book.chapter(&text(&args,"chapter")?)?;
    // Separate bridge processes use the same SQLite writer lock for compare-and-save.
    let lock=if op=="book_save"{Some(self.pool.begin_with("BEGIN IMMEDIATE").await.map_err(err)?)}else{None};
    let bytes=limited(&path,MD_LIMIT)?;let hash=digest(&bytes);
    let raw=std::str::from_utf8(&bytes).map_err(|_|"请使用 UTF-8 编码的 Markdown 文件")?;
    if op=="book_read"{return Ok(json!({"text":raw.trim_start_matches('\u{feff}').replace("\r\n","\n"),"hash":hash}))}
    if args["hash"]!=hash{return Err("文件已在其他位置修改。草稿已保留，请重新载入磁盘版本后合并。".into())}
    let mut content=text(&args,"text")?;
    if raw.contains("\r\n"){content=content.replace("\r\n","\n").replace('\n',"\r\n")}
    if raw.starts_with('\u{feff}'){content.insert(0,'\u{feff}')}
    if content.len()>MD_LIMIT{return Err("单章超过 2 MB".into())}
    atomic(&path,content.as_bytes(),true)?;drop(lock);Ok(json!({"hash":digest(content.as_bytes())}))
   },
   "book_create"=>{
    if book.single.is_some(){return Err("请打开手册文件夹以新增章节".into())}
    let name=text(&args,"name")?;crate::project_service::validate_name(&name)?;
    let path=book.chapter(&name)?;atomic(&path,format!("# {}\n\n",path.file_stem().unwrap().to_string_lossy()).as_bytes(),false)?;
    Ok(json!(name))
   },
   "book_order"|"book_rename"=>{
    if book.single.is_some(){return Err("请打开文件夹以整理章节".into())}
    let _lock=if op=="book_order"{Some(self.pool.begin_with("BEGIN IMMEDIATE").await.map_err(err)?)}else{None};
    let mut chapters=book.chapters()?;
    if op=="book_order"{
     let order:Vec<String>=serde_json::from_value(args["order"].clone()).map_err(err)?;
     if let Some(expected)=args.get("expected"){if *expected!=json!(chapters){return Err("章节顺序已变化，请重新打开手册".into())}}
     let mut sorted=order.clone();sorted.sort();chapters.sort();
     if sorted!=chapters{return Err("章节列表已变化，请重新打开手册".into())}
     book.order(&order)?;return Ok(json!(true))
    }
    let old=text(&args,"chapter")?;let name=text(&args,"name")?;crate::project_service::validate_name(&name)?;
    let from=book.chapter(&old)?;let new=Path::new(&old).with_file_name(&name).to_string_lossy().replace('\\',"/");book.chapter(&new)?;
    let relative=if base.is_empty(){old.clone()}else{format!("{base}/{old}")};
    self.scan_link(&link).await?;
    self.change_entry("rename_entry",&json!({"id":id,"linkId":link.id,"relative":relative,"confirm_path":display(&from),"isDir":false,"name":name})).await?;
    for ch in &mut chapters{if *ch==old{*ch=new.clone()}}
    book.order(&chapters)?;Ok(json!(new))
   },
   "book_image"=>{
    let name=text(&args,"path")?;
    let path=book.path(&name)?;
    let ext=path.extension().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
    if !["png","jpg","jpeg","gif","webp"].contains(&ext.as_str()){return Err("所引图片格式不支持".into())}
    let bytes=limited(&path,IMAGE_LIMIT)?;let (_,mime)=image_format(&bytes).ok_or("图片格式无法识别")?;
    Ok(json!(format!("data:{mime};base64,{}",STANDARD.encode(bytes))))
   },
   "book_add_image"=>{
    let encoded=args["data"].as_str().ok_or("缺少图片")?;
    if encoded.len()>IMAGE_LIMIT*4/3+4{return Err("图片超过 12 MB".into())}
    let bytes=STANDARD.decode(encoded).map_err(err)?;let(ext,_)=image_format(&bytes).ok_or("支持 PNG、JPEG、WebP、GIF")?;
    let dir=book.path("assets")?;fs::create_dir_all(&dir).map_err(err)?;
    let relative=format!("assets/{}.{}",uuid::Uuid::new_v4().simple(),ext);
    atomic(&book.path(&relative)?,&bytes,false)?;Ok(json!(relative))
   },
   "book_pdf_target"=>Ok(json!(display(&book.output("pdf")?))),
   "book_html"=>{
    let html=args["html"].as_str().ok_or("缺少排版内容")?;
    if html.len()>64*1024*1024{return Err("整本 HTML 超过 64 MB，请缩小插图".into())}
    let target=book.output("html")?;atomic(&target,html.as_bytes(),false)?;Ok(json!(display(&target)))
   },
   _=>Err("未知手册编辑操作".into())
  }
 }
}
