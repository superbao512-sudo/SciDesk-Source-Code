//! Installed data lives beside scidesk.exe; development remains isolated.
use std::path::PathBuf;
pub fn data_root() -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    {
        let project=PathBuf::from(env!("SCIDESK_PROJECT_ROOT"));
        if let Some(test)=std::env::var_os("SCIDESK_DESKTOP_TEST_ROOT") {
            let test=std::fs::canonicalize(test).map_err(|e|e.to_string())?;
            let boundary=std::fs::canonicalize(project.join("development/verification")).map_err(|e|e.to_string())?;
            if test==boundary||!test.starts_with(&boundary){return Err("桌面测试目录必须是 development/verification 内的独立文件夹".into())}
            return Ok(test.join("runtime"));
        }
        Ok(project.join("runtime/desktop"))
    }
    #[cfg(not(debug_assertions))]
    {
        Ok(crate::project_service::application_root()?.join("SciDeskData"))
    }
}
pub fn legacy_data_root()->Option<PathBuf>{std::env::var_os("LOCALAPPDATA").map(PathBuf::from).filter(|p|p.is_absolute()).map(|p|p.join("com.scidesk.v2.desktop"))}
pub fn export_directory(kind:&str)->Result<PathBuf,String>{
    if !matches!(kind,"Screenshots"|"PlotDigitizer"){return Err("未知导出类型".into())}
    let path=data_root()?.join("Exports").join(kind);crate::personal_seed::prepare_project_directory(&path)?;Ok(path)
}
