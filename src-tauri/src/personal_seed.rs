//! Optional private installer snapshot. Never replace an existing module.
use std::{fs, io::Write, path::{Component, Path, PathBuf}};
use serde::Deserialize;
use sha2::{Digest, Sha256};
#[derive(Deserialize)]
struct Manifest { version: u32, groups: Vec<Group> }
#[derive(Deserialize)]
struct Group { target: String, directory: bool, files: Vec<Entry> }
#[derive(Deserialize)]
struct Entry { path: String, size: u64, sha256: String }
fn relative(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty() || value.contains(['\\', ':']) || path.components().any(|c| !matches!(c, Component::Normal(_))) { return Err("内置数据包含无效相对路径".into()); }
    Ok(path.to_owned())
}
pub(crate) fn prepare_project_directory(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) { prepare_project_directory(parent)?; }
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !crate::project_service::is_link(&meta) => Ok(()),
        Ok(_) => Err(format!("数据目录不是普通文件夹：{}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => fs::create_dir(path).map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    }
}
pub fn initialize(source: &Path, destination: &Path) -> Result<Vec<String>, String> {
    let manifest_path = source.join("manifest.json");
    if !manifest_path.exists() { return Ok(vec![]); }
    let manifest: Manifest = serde_json::from_slice(&fs::read(&manifest_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if manifest.version != 1 { return Err("内置个人数据版本不受支持".into()); }
    // Validate all payloads before publishing any module.
    for group in &manifest.groups {
        relative(&group.target)?;
        if group.files.is_empty() || (!group.directory && group.files.len() != 1) { return Err("内置数据清单不完整".into()); }
        for entry in &group.files {
            let path = source.join(relative(&entry.path)?);
            let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if !meta.is_file() || crate::project_service::is_link(&meta) || meta.len() != entry.size { return Err("内置数据文件不完整".into()); }
            let bytes = fs::read(path).map_err(|e| e.to_string())?;
            if format!("{:x}", Sha256::digest(&bytes)) != entry.sha256 { return Err("内置数据校验失败，请重新安装".into()); }
            if group.directory {
                relative(entry.path.strip_prefix(&format!("{}/", group.target)).ok_or("内置数据文件超出模块目录")?)?;
            } else if entry.path != group.target { return Err("内置数据文件目标不一致".into()); }
        }
    }
    prepare_project_directory(destination)?;
    let mut initialized = vec![];
    for group in manifest.groups {
        let target = destination.join(&group.target);
        if fs::symlink_metadata(&target).is_ok() { continue; }
        prepare_project_directory(target.parent().ok_or("内置数据目标无父目录")?)?;
        let staging = target.with_file_name(format!(".seed-{}", uuid::Uuid::new_v4()));
        if group.directory { fs::create_dir(&staging).map_err(|e| e.to_string())?; }
        for entry in group.files {
            let out = if group.directory {
                let prefix=format!("{}/", group.target);
                let child = entry.path.strip_prefix(&prefix).ok_or("无效模块文件")?;
                let out = staging.join(child); prepare_project_directory(out.parent().unwrap())?; out
            } else { staging.clone() };
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(out).map_err(|e| e.to_string())?;
            file.write_all(&fs::read(source.join(entry.path)).map_err(|e| e.to_string())?).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
        }
        // MoveFileW refuses both existing files and directories on Windows.
        if let Err(error) = crate::project_platform::rename_no_replace(&staging, &target) {
            if !target.exists() { return Err(error); }
            // Another startup won the race. Remove only our unpublished staging.
            if group.directory { let _ = fs::remove_dir_all(&staging); } else { let _ = fs::remove_file(&staging); }
        } else { initialized.push(group.target); }
    }
    Ok(initialized)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let root = PathBuf::from(env!("SCIDESK_PROJECT_ROOT")).join("development/verification/personal-seed-tests").join(uuid::Uuid::new_v4().to_string());
        let source = root.join("source"); let destination = root.join("fresh-user");
        fs::create_dir_all(source.join("journals")).unwrap(); fs::write(source.join("journals/journals.db"), b"snapshot").unwrap();
        fs::write(source.join("manifest.json"), serde_json::json!({"version":1,"groups":[{"target":"journals","directory":true,"files":[{"path":"journals/journals.db","size":8,"sha256":format!("{:x}",Sha256::digest(b"snapshot"))}]}]}).to_string()).unwrap();
        (root,source,destination)
    }
    #[test] fn fresh_seed_and_existing_edits_are_preserved() {
        let (root,source,destination)=fixture(); assert_eq!(initialize(&source,&destination).unwrap(),vec!["journals"]);
        fs::write(destination.join("journals/journals.db"),b"user edits").unwrap();
        assert!(initialize(&source,&destination).unwrap().is_empty()); assert_eq!(fs::read(destination.join("journals/journals.db")).unwrap(),b"user edits"); fs::remove_dir_all(root).unwrap();
    }
    #[test] fn corrupted_payload_is_rejected_before_writes() {
        let (root,source,destination)=fixture();fs::write(source.join("journals/journals.db"),b"modified").unwrap();
        assert!(initialize(&source,&destination).is_err());assert!(!destination.exists());fs::remove_dir_all(root).unwrap();
    }
    #[test] fn paths_cannot_escape_the_seed() {
        for path in ["../secret","/absolute","C:/data","a/../../data","a\\b",""] { assert!(relative(path).is_err(),"{path}"); }
        assert!(relative("database/learning/learning.db").is_ok());
    }
}
