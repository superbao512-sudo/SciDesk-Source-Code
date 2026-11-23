use super::*;

impl Store {
 pub(super) async fn change_entry(&self, op:&str, args:&Value)->Result<Value> {
  let mut tx=self.pool.begin_with("BEGIN IMMEDIATE").await.map_err(err)?;
  let manual=text(args,"id")?;
  let link:Link=sqlx::query_as("SELECT id,manual_id,path,kind,status,message,scanned_at FROM links WHERE id=? AND manual_id=?")
   .bind(text(args,"linkId")?).bind(&manual).fetch_optional(&mut *tx).await.map_err(err)?.ok_or("手册关联已改变，请刷新")?;
  let relative=text(args,"relative")?;
  if relative.is_empty() && link.kind!="file" {return Err("请选择手册内容中的文件或子文件夹".into())}
  let source=resolve(&link,&relative)?;
  let guard=crate::file_transfer::Guard::new(vec![])?;
  guard.check(&source)?;
  let shown=text(args,"confirm_path")?;
  if key(Path::new(&shown))!=key(&source) || args["isDir"].as_bool()!=Some(source.is_dir()) {
   return Err("目标位置或类型已改变，请刷新并重新确认".into())
  }
  if !relative.is_empty() {
   let indexed:i64=sqlx::query_scalar("SELECT COUNT(*) FROM entries WHERE link_id=? AND relative=? AND is_dir=?")
    .bind(&link.id).bind(&relative).bind(source.is_dir()).fetch_one(&mut *tx).await.map_err(err)?;
   if indexed!=1{return Err("条目已改变，请重新扫描后再操作".into())}
  }
  let links:Vec<Link>=sqlx::query_as("SELECT id,manual_id,path,kind,status,message,scanned_at FROM links").fetch_all(&mut *tx).await.map_err(err)?;
  let mut affected=HashSet::new();
  let destination=if op=="rename_entry"{
   let name=text(args,"name")?;
   crate::project_service::validate_name(&name)?;
   let parent=source.parent().ok_or("不能重命名磁盘根目录")?;
   Some(parent.join(name))
  }else{None};
  // Stage metadata before touching disk. Conflicting associations then fail
  // before a rename, and a failed Shell operation rolls the transaction back.
  for item in links {
   let path=Path::new(&item.path);
   if within(&source,path)||within(path,&source){affected.insert(item.manual_id.clone());}
   if !within(path,&source){continue}
   if let Some(to)=&destination{
    let mut new_path=to.clone();
    new_path.extend(path.components().skip(source.components().count()));
    sqlx::query("UPDATE links SET path=?,path_key=? WHERE id=?")
     .bind(display(&new_path)).bind(key(&new_path)).bind(&item.id).execute(&mut *tx).await
     .map_err(|e|format!("无法更新手册关联，请检查目标路径是否已关联：{e}"))?;
   }else{
    sqlx::query("DELETE FROM entries WHERE link_id=?").bind(&item.id).execute(&mut *tx).await.map_err(err)?;
    sqlx::query("UPDATE links SET status='missing',message='已移入回收站' WHERE id=?").bind(&item.id).execute(&mut *tx).await.map_err(err)?;
   }
  }
  if let Some(to)=&destination{
   if source.as_os_str()!=to.as_os_str(){project_platform::rename_no_replace(&source,to).map_err(|e|format!("重命名失败：{e}"))?;}
  }else{
   let target=source.clone();
   tauri::async_runtime::spawn_blocking(move||{
    guard.check(&target)?;
    crate::file_transfer::inspect(&target)?;
    project_platform::recycle(&target)
   }).await.map_err(err)??;
  }
  if let Err(error)=tx.commit().await{
   if let Some(to)=destination{
    if source.as_os_str()!=to.as_os_str(){
     project_platform::rename_no_replace(&to,&source).map_err(|undo|format!("文件已改名为 {}，更新关联失败：{error}；恢复原名失败：{undo}",display(&to)))?;
    }
    return Err(format!("重命名未完成，已保留原名称：{error}"))
   }
   return Ok(json!({"warning":format!("文件已移入回收站，请重新扫描以更新手册关联：{error}")}))
  }
  for id in affected{
   if let Err(e)=self.scan_manual(&id).await{return Ok(json!({"warning":format!("本地操作已完成，请重新扫描手册：{e}")}))}
  }
  Ok(json!({}))
 }
}
