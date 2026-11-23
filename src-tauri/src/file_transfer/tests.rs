use super::*;
#[cfg(windows)]
#[test]fn junction_sources_and_targets_are_rejected_without_touching_their_contents(){
 let (root,guard)=fixture();let link=root.join("junction");
 let status=std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command","New-Item -ItemType Junction -Path $env:SCIDESK_DROP_LINK -Target $env:SCIDESK_DROP_TARGET -ErrorAction Stop | Out-Null"]).env("SCIDESK_DROP_LINK",&link).env("SCIDESK_DROP_TARGET",root.join("source/中文目录")).status().unwrap();assert!(status.success());
 assert!(guard.check(&link).is_err());assert!(plan(&guard,&root.join("target"),&[link.clone()],false,false).is_err());assert!(plan(&guard,&link,&[root.join("source/中文目录/实验.txt")],false,false).is_err());
 fs::remove_dir(link).unwrap();assert_eq!(fs::read(root.join("source/中文目录/实验.txt")).unwrap(),b"original scientific fixture");fs::remove_dir_all(root).unwrap();
}
#[test]fn protected_paths_and_invalid_source_batches_never_publish_files(){
 let (root,guard)=fixture();let first=root.join("source/中文目录/实验.txt");
 assert!(plan(&guard,&root.join("target"),&[first,root.join("missing")],false,false).is_err());assert!(fs::read_dir(root.join("target")).unwrap().next().is_none());
 assert!(guard.check(Path::new(env!("SCIDESK_PROJECT_ROOT"))).is_err());assert!(read_drop_file(json!({"path":root.join("source")})).is_err());
 let private=root.join("private");fs::create_dir(&private).unwrap();fs::write(private.join("database.db"),b"private metadata fixture").unwrap();let guarded=Guard{app:root.join("app"),data:private.clone(),allowed:vec![private.clone()]};assert!(guarded.check(&private.join("database.db")).is_err());
 let read=read_drop_file(json!({"path":root.join("source/中文目录/实验.txt")})).unwrap();assert_eq!(read["base64"],base64::engine::general_purpose::STANDARD.encode(b"original scientific fixture"));fs::remove_dir_all(root).unwrap();
}
fn fixture()->(PathBuf,Guard){let root=PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/verification/file-transfer-tests").join(uuid::Uuid::new_v4().to_string());fs::create_dir_all(root.join("source/中文目录")).unwrap();fs::create_dir(root.join("target")).unwrap();fs::write(root.join("source/中文目录/实验.txt"),b"original scientific fixture").unwrap();(root,Guard::new(vec![]).unwrap())}
#[test]fn preflight_rejects_self_nesting_and_preserves_conflicts(){
 let (root,guard)=fixture();let source=root.join("source/中文目录");
 assert!(plan(&guard,&source,&[source.clone()],false,false).is_err());
 fs::write(root.join("target/实验.txt"),b"existing").unwrap();let input=source.join("实验.txt");
 let skip=plan(&guard,&root.join("target"),&[input.clone()],false,false).unwrap();assert_eq!(skip[0].status,"skipped");
 let keep=plan(&guard,&root.join("target"),&[input.clone()],true,false).unwrap();assert!(keep[0].destination.ends_with("实验 (1).txt"));
 assert_eq!(fs::read(root.join("target/实验.txt")).unwrap(),b"existing");assert_eq!(fs::read(input).unwrap(),b"original scientific fixture");fs::remove_dir_all(root).unwrap();
}
#[cfg(windows)]
#[test]fn windows_copy_move_keep_both_and_cancel_free_skips_use_real_files(){
 let (root,guard)=fixture();let destination=root.join("target");let original=root.join("source/中文目录");
 let copied=transfer(guard.clone(),destination.clone(),vec![original.clone()],json!({"mode":"copy","conflict":"skip"})).unwrap();assert_eq!(copied["items"][0]["status"],"completed");assert!(original.join("实验.txt").is_file());assert_eq!(fs::read(destination.join("中文目录/实验.txt")).unwrap(),b"original scientific fixture");
 let skipped=transfer(guard.clone(),destination.clone(),vec![original.clone()],json!({"mode":"move","conflict":"skip"})).unwrap();assert_eq!(skipped["items"][0]["status"],"skipped");assert!(original.exists());
 let moved=transfer(guard,destination.clone(),vec![original.clone()],json!({"mode":"move","conflict":"keep"})).unwrap();assert_eq!(moved["items"][0]["status"],"completed");assert!(!original.exists());assert_eq!(fs::read(destination.join("中文目录 (1)/实验.txt")).unwrap(),b"original scientific fixture");assert!(destination.join("中文目录/实验.txt").is_file());fs::remove_dir_all(root).unwrap();
}
#[cfg(windows)]
#[test]fn clipboard_paste_duplicates_in_place_without_changing_drag_or_overwriting(){
 let (root,guard)=fixture();
 let source=root.join("source/中文目录");let parent=source.parent().unwrap();
 let file=source.join("实验.txt");
 let options=json!({"mode":"copy","conflict":"keep","duplicateInPlace":true});
 for expected in ["实验 (1).txt","实验 (2).txt"]{
  let report=transfer(guard.clone(),source.clone(),vec![file.clone()],options.clone()).unwrap();
  assert_eq!(report["items"][0]["status"],"completed");
  assert_eq!(fs::read(source.join(expected)).unwrap(),b"original scientific fixture");
 }
 let report=transfer(guard.clone(),parent.to_owned(),vec![source.clone()],options.clone()).unwrap();
 assert_eq!(report["items"][0]["status"],"completed");
 assert_eq!(fs::read(parent.join("中文目录 (1)/实验.txt")).unwrap(),fs::read(&file).unwrap());
 assert!(transfer(guard.clone(),source.clone(),vec![source.clone()],options).is_err());
 let drop=transfer(guard,source,vec![file],json!({"mode":"copy","conflict":"keep"})).unwrap();
 assert_eq!(drop["items"][0]["status"],"skipped");
 fs::remove_dir_all(root).unwrap();
}
#[cfg(windows)]
#[test]fn native_drag_object_contains_actual_unicode_file_paths_and_cancel_contract(){
 use windows::{Win32::{Foundation::{DRAGDROP_S_CANCEL,DRAGDROP_S_DROP},System::{Com::{IDataObject,FORMATETC,DVASPECT_CONTENT,TYMED_HGLOBAL},Ole::{IDropSource,OleInitialize,OleUninitialize,ReleaseStgMedium},SystemServices::MODIFIERKEYS_FLAGS},UI::Shell::{HDROP,DragQueryFileW}}};
 let (root,_)=fixture();unsafe{
  OleInitialize(None).unwrap();let files=vec![root.join("source/中文目录"),root.join("source/中文目录/实验.txt")];let data:IDataObject=native::data_object(&files).unwrap();
  let format=FORMATETC{cfFormat:15,dwAspect:DVASPECT_CONTENT.0,lindex:-1,tymed:TYMED_HGLOBAL.0 as u32,..Default::default()};
  assert_eq!(native::paths_from_object(&data).unwrap(),files);
  let mut medium=data.GetData(&format).unwrap();let handle=HDROP(medium.u.hGlobal.0);assert_eq!(DragQueryFileW(handle,u32::MAX,None),2);let mut buf=[0u16;1024];let len=DragQueryFileW(handle,1,Some(&mut buf)) as usize;assert_eq!(key(Path::new(&String::from_utf16_lossy(&buf[..len]))),key(&files[1]));ReleaseStgMedium(&mut medium);
  let source:IDropSource=native::Source.into();assert_eq!(source.QueryContinueDrag(true,MODIFIERKEYS_FLAGS(1)),DRAGDROP_S_CANCEL);assert_eq!(source.QueryContinueDrag(false,MODIFIERKEYS_FLAGS(0)),DRAGDROP_S_DROP);
  drop(data);drop(source);OleUninitialize();
 }fs::remove_dir_all(root).unwrap();
}
