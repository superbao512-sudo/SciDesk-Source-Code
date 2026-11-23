use std::{fs::{self,OpenOptions},io::Write};
pub fn record(stage:&str,detail:impl AsRef<str>){
 let Ok(root)=crate::runtime_paths::data_root()else{return};let path=root.join("logs/boot.log");if let Some(parent)=path.parent(){let _=fs::create_dir_all(parent);}if let Ok(mut file)=OpenOptions::new().create(true).append(true).open(path){let _=writeln!(file,"{stage} {}",detail.as_ref());}
}
#[tauri::command]pub fn boot_log(stage:String,detail:String){record(&stage,detail)}
