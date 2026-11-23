use super::*;

#[test]
fn tool_creates_separate_editable_files_and_registers_each_folder() {
 tauri::async_runtime::block_on(async {
  let base=PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/verification/manual-editor-tests").join(id());
  let projects=base.join("Projects");
  let store=Store::open(&base.join("metadata")).await.unwrap();
  let a=store.create_blank_book(&projects).await.unwrap();
  let mut args=a["scope"].clone();
  args["chapter"]=a["chapter"].clone();
  let first=store.run("book_read",args.clone()).await.unwrap();
  assert_eq!(first["text"],"");
  args["hash"]=first["hash"].clone();
  args["text"]=json!("# 新手册");
  store.run("book_save",args.clone()).await.unwrap();
  let b=store.create_blank_book(&projects).await.unwrap();
  assert_ne!(a["scope"],b["scope"]);
  assert_eq!(store.run("book_read",args).await.unwrap()["text"],"# 新手册");
  let listing=store.run("book_list",b["scope"].clone()).await.unwrap();
  assert_ne!(listing["title"],"未命名手册");
  assert_eq!(listing["chapters"],json!([b["chapter"]]));
  store.pool.close().await;
 })
}

#[test]
fn saves_order_and_external_edits_are_preserved() {
 tauri::async_runtime::block_on(async {
  let base=PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/verification/manual-editor-tests").join(id());
  let root=base.join("手册");
  fs::create_dir_all(&root).unwrap();
  fs::write(root.join("初稿.md"),b"\xef\xbb\xbf# Notes\r\n").unwrap();
  let store=Store::open(&base.join("metadata")).await.unwrap();
  let manual=store.run("import",json!({"name":"手册","kind":"folder","path":display(&root)})).await.unwrap();
  let link=store.links(manual["id"].as_str().unwrap()).await.unwrap().remove(0);
  let mut args=json!({"id":manual["id"],"linkId":link.id,"chapter":"初稿.md"});
  let read=store.run("book_read",args.clone()).await.unwrap();assert_eq!(read["text"],"# Notes\n");
  args["text"]=json!("第二稿\n");args["hash"]=read["hash"].clone();
  store.run("book_save",args.clone()).await.unwrap();
  assert_eq!(fs::read(root.join("初稿.md")).unwrap(),"\u{feff}第二稿\r\n".as_bytes());
  fs::write(root.join("初稿.md"),"外部修改").unwrap();
  assert!(store.run("book_save",args.clone()).await.is_err());
  assert_eq!(fs::read_to_string(root.join("初稿.md")).unwrap(),"外部修改");
  args["name"]=json!("算例.md");
  store.run("book_create",args.clone()).await.unwrap();assert!(store.run("book_create",args.clone()).await.is_err());
  args["order"]=json!(["算例.md","初稿.md"]);
  store.run("book_order",args.clone()).await.unwrap();
  assert_eq!(store.run("book_list",args.clone()).await.unwrap()["chapters"],args["order"]);
  args["name"]=json!("理论.md");store.run("book_rename",args.clone()).await.unwrap();
  assert_eq!(store.run("book_list",args.clone()).await.unwrap()["chapters"],json!(["算例.md","理论.md"]));
  args["chapter"]=json!("../理论.md");assert!(store.run("book_read",args.clone()).await.is_err());
  args["data"]=json!(STANDARD.encode(b"not a picture"));assert!(store.run("book_add_image",args.clone()).await.is_err());
  store.pool.close().await;
 })
}
