use serde::Deserialize;
use std::time::Duration;

const LATEST: &str = "https://api.github.com/repos/superbao512-sudo/SciDesk/releases/latest";
const RELEASES: &str = "https://github.com/superbao512-sudo/SciDesk/releases";

#[derive(Deserialize)]
struct Release { tag_name: String }

#[tauri::command]
pub async fn check_release(app: tauri::AppHandle) -> Result<Option<(String, bool)>, String> {
    let response = reqwest::Client::builder()
        .user_agent(concat!("SciDesk/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15)).build().map_err(|e| e.to_string())?
        .get(LATEST).send().await.map_err(|_| "无法连接 GitHub，请检查网络后重试。")?;
    if response.status() == reqwest::StatusCode::NOT_FOUND { return Ok(None); }
    let release: Release = response.error_for_status().map_err(|e| format!("GitHub 暂时无法提供版本信息：{e}"))?
        .json().await.map_err(|_| "无法读取 GitHub 版本信息。")?;
    let version: semver::Version = release.tag_name.trim_start_matches('v').parse()
        .map_err(|_| "发布标签无法识别，请查看发布页。")?;
    Ok(Some((release.tag_name, version > app.package_info().version)))
}

#[tauri::command]
pub fn open_release_page() -> Result<(), String> {
    crate::project_platform::open(std::path::Path::new(RELEASES))
}
