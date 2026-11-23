use super::*;

#[cfg(windows)]
#[test]
fn rename_and_recycle_refresh_shared_manual_links_without_detaching(){
 tauri::async_runtime::block_on(async{
  let base=PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/verification/manual-file-actions").join(id());
  let docs=base.join("documents");let folder=docs.join("原目录");
  fs::create_dir_all(&folder).unwrap();
  let original=folder.join("教材.md");fs::write(&original,"test content").unwrap();
  let store=Store::open(&base.join("metadata")).await.unwrap();
  let import=|path:&Path,kind:&str|json!({"path":display(path),"kind":kind,"name":"测试手册"});
  let a=store.run("import",import(&docs,"folder")).await.unwrap()["id"].as_str().unwrap().to_owned();
  let b=store.run("import",import(&original,"file")).await.unwrap()["id"].as_str().unwrap().to_owned();
  let al=store.links(&a).await.unwrap().remove(0);let bl=store.links(&b).await.unwrap().remove(0);
  let mut op=json!({"id":a,"linkId":al.id,"relative":"原目录","isDir":true,"confirm_path":display(&folder),"name":"新目录"});
  store.run("rename_entry",op.clone()).await.unwrap();
  let renamed=docs.join("新目录");let file=renamed.join("教材.md");
  assert_eq!(fs::read_to_string(&file).unwrap(),"test content");
  assert_eq!(key(Path::new(&store.links(&b).await.unwrap()[0].path)),key(&file));
  let token=base.file_name().unwrap().to_str().unwrap();
  let file_name=format!("回收验证-{token}.md");
  op=json!({"id":b,"linkId":bl.id,"relative":"","confirm_path":display(&file),"isDir":false,"name":file_name});
  fs::write(renamed.join("已有.md"),"existing").unwrap();
  for invalid in ["../越界","已有.md"]{
   let mut conflict=op.clone();conflict["name"]=json!(invalid);
   assert!(store.run("rename_entry",conflict).await.is_err());
   assert_eq!(fs::read_to_string(&file).unwrap(),"test content");
  }
  store.run("rename_entry",op.clone()).await.unwrap();
  let changed=renamed.join(&file_name);
  assert!(store.run("trash_entry",op.clone()).await.is_err()); // pre-rename confirmation
  assert!(changed.is_file());
  op["confirm_path"]=json!(display(&changed));
  let mut wrong=op.clone();wrong["isDir"]=json!(true);
  assert!(store.run("trash_entry",wrong).await.is_err());
  store.run("trash_entry",op).await.unwrap();
  assert!(!changed.exists());assert_eq!(store.links(&b).await.unwrap()[0].status,"missing");
  let folder_name=format!("文件夹回收验证-{token}");
  let args=json!({"id":a,"linkId":al.id,"relative":"新目录","confirm_path":display(&renamed),"isDir":true,"name":folder_name});
  store.run("rename_entry",args).await.unwrap();
  let deleted_folder=docs.join(&folder_name);
  store.run("trash_entry",json!({"id":a,"linkId":al.id,"relative":folder_name,"isDir":true,"confirm_path":display(&deleted_folder)})).await.unwrap();
  assert!(!deleted_folder.exists());
  assert!(store.run("browse_root",json!({"id":a})).await.unwrap()["entries"].as_array().unwrap().is_empty());
  assert_eq!(store.links(&a).await.unwrap().len(),1);
  println!("Recycle Bin verification names: {file_name} | {folder_name}");
  store.pool.close().await;
 })
}

#[test]
fn manual_drop_associations_and_file_transfers_refresh_the_real_index(){tauri::async_runtime::block_on(async{
 let base=PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/verification/manual-drop-tests").join(id());fs::create_dir_all(base.join("documents/subfolder")).unwrap();fs::create_dir(base.join("incoming")).unwrap();fs::write(base.join("incoming/新手册.txt"),b"actual manual contents").unwrap();
 let store=Store::open(&base.join("metadata")).await.unwrap();let created=store.run("save",json!({"name":"Drag fixture"})).await.unwrap();let manual=created["id"].as_str().unwrap();
 let linked=store.run("drop_links",json!({"id":manual,"paths":[display(&base.join("documents"))]})).await.unwrap();assert_eq!(linked["items"][0]["status"],"completed");let link=store.links(manual).await.unwrap().remove(0);
 let copied=store.run("drop_files",json!({"id":manual,"linkId":link.id,"relative":"subfolder","paths":[display(&base.join("incoming/新手册.txt"))],"mode":"copy","conflict":"skip"})).await.unwrap();assert_eq!(copied["items"][0]["status"],"completed");
 let listing=store.run("browse",json!({"id":manual,"linkId":link.id,"relative":"subfolder"})).await.unwrap();assert_eq!(listing["entries"][0]["name"],"新手册.txt");assert!(base.join("incoming/新手册.txt").is_file());
 assert!(store.run("drop_files",json!({"id":manual,"linkId":link.id,"relative":"../incoming","paths":[display(&base.join("incoming/新手册.txt"))],"mode":"move"})).await.is_err());
 let new_folder=json!({"id":manual,"linkId":link.id,"relative":"subfolder","name":"计算示例"});
 store.run("create_folder",new_folder.clone()).await.unwrap();
 assert!(store.run("create_folder",new_folder).await.is_err());
 let listing=store.run("browse",json!({"id":manual,"linkId":link.id,"relative":"subfolder"})).await.unwrap();
 assert_eq!(listing["entries"][0]["name"],"计算示例");
 for (op,args) in [
  ("create_folder",json!({"id":manual,"linkId":link.id,"relative":"subfolder","name":"../上级"})),
  ("paste_files",json!({"id":manual,"linkId":link.id,"relative":"../incoming"})),
  ("copy_files",json!({"id":manual,"entries":[{"linkId":link.id,"relative":"../incoming"}]})),
 ]{assert!(store.run(op,args).await.is_err())}
 store.run("delete",json!({"id":manual})).await.unwrap();assert_eq!(fs::read(base.join("documents/subfolder/新手册.txt")).unwrap(),b"actual manual contents");store.pool.close().await;fs::remove_dir_all(base).unwrap();
})}

#[test]
fn legacy_manual_upgrade_preserves_metadata_links_and_original_files(){tauri::async_runtime::block_on(async{
 let base=PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/manual-tests").join(id());let root=base.join("metadata");fs::create_dir_all(&root).unwrap();let original=base.join("legacy.md");fs::write(&original,"legacy original").unwrap();
 let pool=SqlitePoolOptions::new().connect_with(SqliteConnectOptions::new().filename(root.join("manuals.db")).create_if_missing(true).foreign_keys(true)).await.unwrap();
 for statement in SCHEMA.split(';').filter(|s|!s.trim().is_empty()){sqlx::query(statement).execute(&pool).await.unwrap();}
 sqlx::query("INSERT INTO manuals(id,name,category,description) VALUES('first','Legacy first','Guide','Keep description'),('second','Legacy second','','')").execute(&pool).await.unwrap();
 sqlx::query("INSERT INTO links(id,manual_id,path,path_key,kind) VALUES('legacy-link','first',?,?,'file')").bind(display(&canonical(&original).unwrap())).bind(key(&original)).execute(&pool).await.unwrap();pool.close().await;
 let store=Store::open(&root).await.unwrap();let rows=store.run("list",json!({})).await.unwrap();assert_eq!(rows[0]["id"],"first");assert_eq!(rows[1]["id"],"second");assert_eq!(rows[0]["description"],"Keep description");assert_eq!(rows[0]["icon_name"],"book-open");assert_eq!(rows[0]["name_font_size"],18);assert_eq!(rows[0]["links"],1);
 assert_eq!(store.links("first").await.unwrap()[0].id,"legacy-link");assert_eq!(fs::read_to_string(&original).unwrap(),"legacy original");store.pool.close().await;
 let reopened=Store::open(&root).await.unwrap();assert_eq!(reopened.run("list",json!({})).await.unwrap(),rows);reopened.pool.close().await;
});}

#[test]
fn hall_order_icon_color_and_rename_settings_persist_independently_of_original_paths(){tauri::async_runtime::block_on(async{
 let base=PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/manual-tests").join(id());let root=base.join("metadata");let store=Store::open(&root).await.unwrap();let mut ids=vec![];
 for name in ["A","B","C"]{ids.push(store.run("save",json!({"name":name})).await.unwrap()["id"].as_str().unwrap().to_owned());}
 let original=base.join("original.txt");fs::write(&original,"original content").unwrap();store.run("attach",json!({"id":ids[0],"path":display(&original),"kind":"file"})).await.unwrap();
 store.run("reorder",json!({"ids":[ids[2],ids[0],ids[1]]})).await.unwrap();store.run("set_appearance",json!({"id":ids[0],"icon_name":"atom","manual_color":"#9333ea"})).await.unwrap();store.run("rename",json!({"id":ids[0],"name":"Renamed A","font_size":22})).await.unwrap();
 store.run("save",json!({"id":ids[0],"name":"Renamed A","description":"Updated description"})).await.unwrap();let snapshot=store.run("list",json!({})).await.unwrap();assert_eq!(snapshot[0]["id"],ids[2]);assert_eq!(snapshot[1]["icon_name"],"atom");assert_eq!(snapshot[1]["manual_color"],"#9333ea");assert!(snapshot.as_array().unwrap().iter().all(|m|m["name_font_size"]==22));
 for invalid in [json!({"ids":[ids[0],ids[0],ids[2]]}),json!({"ids":[ids[0],ids[1]]})]{assert!(store.run("reorder",invalid).await.is_err());}
 for invalid in [json!({"id":ids[0],"icon_name":"invalid"}),json!({"id":ids[0],"manual_color":"red"})]{assert!(store.run("set_appearance",invalid).await.is_err());}
 for size in [13,29]{assert!(store.run("rename",json!({"id":ids[0],"name":"Invalid change","font_size":size})).await.is_err());}assert_eq!(store.run("list",json!({})).await.unwrap(),snapshot);
 let links=store.links(&ids[0]).await.unwrap();assert_eq!(links[0].path,display(&canonical(&original).unwrap()));store.pool.close().await;
 let reopened=Store::open(&root).await.unwrap();assert_eq!(reopened.run("list",json!({})).await.unwrap(),snapshot);assert_eq!(fs::read_to_string(&original).unwrap(),"original content");reopened.pool.close().await;
});}

#[test]
fn hall_import_creates_associated_manual_atomically_and_can_add_to_existing(){tauri::async_runtime::block_on(async{
 let base=PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/manual-tests").join(id());let source=base.join("source");fs::create_dir_all(&source).unwrap();fs::write(source.join("guide.md"),"original guide").unwrap();let store=Store::open(&base.join("metadata")).await.unwrap();
 assert!(store.run("import",json!({"name":"No orphan","kind":"folder","path":display(&base.join("missing"))})).await.is_err());assert!(store.run("list",json!({})).await.unwrap().as_array().unwrap().is_empty());
 sqlx::query("CREATE TRIGGER reject_import BEFORE INSERT ON links BEGIN SELECT RAISE(ABORT,'injected association failure'); END").execute(&store.pool).await.unwrap();
 let input=json!({"name":"Imported manual","category":"software","description":"Import from hall","kind":"folder","path":display(&source)});
 assert!(store.run("import",input.clone()).await.is_err());assert!(store.run("list",json!({})).await.unwrap().as_array().unwrap().is_empty());sqlx::query("DROP TRIGGER reject_import").execute(&store.pool).await.unwrap();
 let manual=store.run("import",input).await.unwrap()["id"].as_str().unwrap().to_owned();let links=store.links(&manual).await.unwrap();assert_eq!(links.len(),1);
 assert_eq!(store.run("browse",json!({"id":manual,"linkId":links[0].id,"relative":""})).await.unwrap()["entries"][0]["name"],"guide.md");
 let add=json!({"id":manual,"kind":"file","path":display(&source.join("guide.md"))});store.run("import",add.clone()).await.unwrap();assert!(store.run("import",add).await.is_err());assert_eq!(store.links(&manual).await.unwrap().len(),2);
 store.run("delete",json!({"id":manual})).await.unwrap();assert_eq!(fs::read_to_string(source.join("guide.md")).unwrap(),"original guide");store.pool.close().await;
});}

#[test]
fn local_associations_persist_rescan_and_delete_only_metadata(){tauri::async_runtime::block_on(async{
 let base=PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/manual-tests").join(id());let sources=base.join("documents");fs::create_dir_all(sources.join("nested")).unwrap();
 fs::write(sources.join("guide.md"),"# Original guide").unwrap();fs::write(sources.join("nested/book.pdf"),b"fixture document bytes").unwrap();let single=base.join("standalone.txt");fs::write(&single,"original single file").unwrap();
 let root=base.join("metadata");let store=Store::open(&root).await.unwrap();
 let manual=store.run("save",json!({"name":"Scientific guide","category":"software","description":"Actual local references"})).await.unwrap()["id"].as_str().unwrap().to_owned();
 for (path,kind) in [(&sources,"folder"),(&single,"file")]{store.run("attach",json!({"id":manual,"path":display(path),"kind":kind})).await.unwrap();}
 store.pool.close().await;let store=Store::open(&root).await.unwrap();let detail=store.run("get",json!({"id":manual})).await.unwrap();assert_eq!(detail["links"].as_array().unwrap().len(),2);assert_eq!(detail["description"],"Actual local references");
 let links=store.links(&manual).await.unwrap();let folder=links.iter().find(|l|l.kind=="folder").unwrap();let file=links.iter().find(|l|l.kind=="file").unwrap();
 let root_entries=store.run("browse_root",json!({"id":manual})).await.unwrap();let names=root_entries["entries"].as_array().unwrap().iter().map(|e|e["name"].as_str().unwrap()).collect::<Vec<_>>();assert_eq!(names,vec!["nested","guide.md","standalone.txt"]);assert!(!names.contains(&"documents"));assert_eq!(store.run("enter",json!({"id":manual})).await.unwrap()["opened"],false);
 let entries=store.run("browse",json!({"id":manual,"linkId":folder.id,"relative":""})).await.unwrap();assert_eq!(entries["entries"].as_array().unwrap().len(),2);
 let nested=store.run("browse",json!({"id":manual,"linkId":folder.id,"relative":"nested"})).await.unwrap();assert_eq!(nested["entries"][0]["name"],"book.pdf");
 for relative in ["../standalone.txt","C:/Windows","nested/../../standalone.txt"]{assert!(store.run("open",json!({"id":manual,"linkId":folder.id,"relative":relative})).await.is_err());}
 // Only this freshly created disposable fixture is removed to simulate an external deletion.
 fs::remove_file(sources.join("guide.md")).unwrap();store.run("scan",json!({"id":manual})).await.unwrap();
 let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM entries WHERE link_id=? AND relative='guide.md'").bind(&folder.id).fetch_one(&store.pool).await.unwrap();assert_eq!(count,0);
 store.run("detach",json!({"id":manual,"linkId":file.id})).await.unwrap();assert_eq!(fs::read_to_string(&single).unwrap(),"original single file");
 let moved=base.join("moved-documents");fs::rename(&sources,&moved).unwrap();let detail=store.run("scan",json!({"id":manual})).await.unwrap();assert_eq!(detail["links"][0]["status"],"missing");
 assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM entries").fetch_one(&store.pool).await.unwrap(),0);
 store.run("relink",json!({"id":manual,"linkId":folder.id,"kind":"folder","path":display(&moved)})).await.unwrap();
 store.run("save",json!({"id":manual,"name":"Renamed guide","category":"textbook","description":"Updated description"})).await.unwrap();assert_eq!(store.run("get",json!({"id":manual})).await.unwrap()["name"],"Renamed guide");
 store.run("delete",json!({"id":manual})).await.unwrap();assert!(store.run("list",json!({})).await.unwrap().as_array().unwrap().is_empty());
 for table in ["manuals","links","entries"]{assert_eq!(sqlx::query_scalar::<_,i64>(&format!("SELECT COUNT(*) FROM {table}")).fetch_one(&store.pool).await.unwrap(),0);}
 assert_eq!(fs::read(moved.join("nested/book.pdf")).unwrap(),b"fixture document bytes");assert_eq!(fs::read_to_string(&single).unwrap(),"original single file");store.pool.close().await;
});}

#[test]
fn validation_and_duplicate_links_do_not_modify_original_files(){tauri::async_runtime::block_on(async{
 let base=PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/manual-tests").join(id());let store=Store::open(&base.join("metadata")).await.unwrap();let file=base.join("manual.txt");fs::write(&file,"untouched").unwrap();
 assert!(store.run("save",json!({"name":" "})).await.is_err());let manual=store.run("save",json!({"name":"Validation"})).await.unwrap()["id"].as_str().unwrap().to_owned();
 assert!(store.run("attach",json!({"id":manual,"path":display(&file),"kind":"folder"})).await.is_err());
 assert!(store.run("attach",json!({"id":manual,"path":"relative.txt","kind":"file"})).await.is_err());
 let args=json!({"id":manual,"path":display(&file),"kind":"file"});store.run("attach",args.clone()).await.unwrap();assert!(store.run("attach",args).await.is_err());
 let main_version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&store.pool).await.unwrap();assert_eq!(main_version,2);assert_eq!(fs::read_to_string(file).unwrap(),"untouched");store.pool.close().await;
});}
