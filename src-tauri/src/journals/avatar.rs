use base64::Engine;
use serde_json::{json,Value};
use sqlx::SqlitePool;
use std::path::Path;
const MAX:usize=2*1024*1024;
fn format(bytes:&[u8])->Option<(&'static str,&'static str)>{
 if bytes.starts_with(b"\x89PNG\r\n\x1a\n"){Some(("png","image/png"))}else if bytes.starts_with(&[0xff,0xd8,0xff]){Some(("jpg","image/jpeg"))}else if bytes.starts_with(b"RIFF")&&bytes.get(8..12)==Some(b"WEBP"){Some(("webp","image/webp"))}else{None}
}
pub async fn get(pool:&SqlitePool,root:&Path)->Result<Value,String>{
 let filename:Option<String>=sqlx::query_scalar("SELECT avatar_file FROM scholar_profile WHERE id=1").fetch_one(pool).await.map_err(|e|e.to_string())?;
 let Some(filename)=filename else{return Ok(Value::Null)};
 if filename.contains(['/', '\\'])||!filename.starts_with("avatar-"){return Err("头像元数据无效".into())}
 let bytes=std::fs::read(root.join(&filename)).map_err(|e|format!("头像读取失败：{e}"))?;
 if bytes.len()>MAX{return Err("头像文件过大".into())}let (_,mime)=format(&bytes).ok_or("头像格式无效")?;
 Ok(json!(format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))))
}
pub async fn save(pool:&SqlitePool,root:&Path,data:&str)->Result<Value,String>{
 if data.len()>MAX.div_ceil(3)*4+8{return Err("头像不能超过 2 MB".into())}
 let bytes=base64::engine::general_purpose::STANDARD.decode(data).map_err(|_|"头像数据无效")?;
 if bytes.len()>MAX{return Err("头像不能超过 2 MB".into())}let (ext,_)=format(&bytes).ok_or("请选择 PNG、JPEG 或 WebP 图片")?;
 let filename=format!("avatar-{}.{}",uuid::Uuid::new_v4(),ext);let path=root.join(&filename);
 std::fs::write(&path,&bytes).map_err(|e|e.to_string())?;
 if let Err(error)=sqlx::query("UPDATE scholar_profile SET avatar_file=? WHERE id=1").bind(filename).execute(pool).await{
  let _=std::fs::remove_file(path);return Err(error.to_string())
 }get(pool,root).await
}
