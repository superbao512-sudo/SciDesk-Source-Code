// Native launcher integration fixture; writes only to its assigned verification data directory.
fn main(){
 let data=std::path::PathBuf::from(std::env::var_os("SCIDESK_TOOL_DATA_DIR").expect("assigned data directory"));
 let output=format!("api={}\ncwd={}\nargs={:?}",std::env::var("SCIDESK_TOOL_API_VERSION").unwrap(),std::env::current_dir().unwrap().display(),std::env::args().skip(1).collect::<Vec<_>>());
 std::fs::write(data.join("probe-result.txt"),output).unwrap();
}
