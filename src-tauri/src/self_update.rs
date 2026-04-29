use crate::debug_log::emit_debug_log;
use crate::portable_paths::PortablePaths;
use futures_util::StreamExt;
use reqwest::header::{ACCEPT, USER_AGENT};
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

const GITHUB_OWNER: &str = "TilioChr";
const GITHUB_REPO: &str = "EmuManager";
const GITHUB_LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/TilioChr/EmuManager/releases/latest";
const UPDATE_USER_AGENT: &str = "EmuManager";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_name: Option<String>,
    pub release_url: Option<String>,
    pub published_at: Option<String>,
    pub asset_name: Option<String>,
    pub asset_size: Option<u64>,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateDownloadRequest {
    pub version: String,
    pub asset_name: String,
    pub download_url: String,
    pub asset_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateDownloadResult {
    pub version: String,
    pub asset_name: String,
    pub file_path: String,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateProgressPayload {
    pub version: String,
    pub file_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubReleaseResponse {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    browser_download_url: Option<String>,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub async fn check_for_update(app: &AppHandle) -> Result<AppUpdateStatus, String> {
    emit_debug_log(
        app,
        "debug",
        "app-update",
        "Checking EmuManager release feed",
        Some(format!("url={}", GITHUB_LATEST_RELEASE_URL)),
    );

    let client = Client::new();
    let response = client
        .get(GITHUB_LATEST_RELEASE_URL)
        .header(USER_AGENT, UPDATE_USER_AGENT)
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("Update check failed: {}", error))?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(AppUpdateStatus {
            current_version: current_version().to_string(),
            latest_version: None,
            update_available: false,
            release_name: None,
            release_url: None,
            published_at: None,
            asset_name: None,
            asset_size: None,
            download_url: None,
        });
    }

    if !response.status().is_success() {
        return Err(format!(
            "Update check failed with status {}",
            response.status()
        ));
    }

    let raw = response
        .text()
        .await
        .map_err(|error| format!("Cannot read update response: {}", error))?;
    let release = serde_json::from_str::<GithubReleaseResponse>(&raw)
        .map_err(|error| format!("Invalid update response: {}", error))?;

    let latest_version = normalize_release_version(&release.tag_name);
    let update_available = compare_versions(current_version(), &latest_version) == Ordering::Less;
    let selected_asset = select_update_asset(&release.assets);

    emit_debug_log(
        app,
        if update_available { "info" } else { "debug" },
        "app-update",
        if update_available {
            "EmuManager update available"
        } else {
            "EmuManager is up to date"
        },
        Some(format!(
            "current_version={}\nlatest_version={}\nasset_name={}",
            current_version(),
            latest_version,
            selected_asset
                .as_ref()
                .map(|asset| asset.name.as_str())
                .unwrap_or("none")
        )),
    );

    Ok(AppUpdateStatus {
        current_version: current_version().to_string(),
        latest_version: Some(latest_version),
        update_available,
        release_name: release.name,
        release_url: release.html_url,
        published_at: release.published_at,
        asset_name: selected_asset.as_ref().map(|asset| asset.name.clone()),
        asset_size: selected_asset.as_ref().and_then(|asset| asset.size),
        download_url: selected_asset.and_then(|asset| asset.browser_download_url),
    })
}

pub async fn download_update(
    app: &AppHandle,
    paths: &PortablePaths,
    request: &AppUpdateDownloadRequest,
) -> Result<AppUpdateDownloadResult, String> {
    validate_update_download_url(&request.download_url)?;

    let file_name = sanitize_file_name(&request.asset_name);
    if !is_supported_self_update_binary_name(&file_name) {
        return Err(format!(
            "Unsupported update asset for self-update: {}",
            request.asset_name
        ));
    }

    let update_dir = Path::new(&paths.data)
        .join("downloads")
        .join("updates")
        .join(sanitize_file_name(&request.version));
    fs::create_dir_all(&update_dir)
        .map_err(|error| format!("Cannot create update download directory: {}", error))?;

    let destination = update_dir.join(&file_name);
    let partial_destination = update_dir.join(format!("{}.part", file_name));

    if partial_destination.exists() {
        tokio::fs::remove_file(&partial_destination)
            .await
            .map_err(|error| format!("Cannot remove previous update download: {}", error))?;
    }

    let client = Client::new();
    let response = client
        .get(&request.download_url)
        .header(USER_AGENT, UPDATE_USER_AGENT)
        .send()
        .await
        .map_err(|error| format!("Update download failed: {}", error))?;

    if !response.status().is_success() {
        return Err(format!(
            "Update download failed with status {}",
            response.status()
        ));
    }

    let total_bytes = response.content_length().or(request.asset_size);
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(&partial_destination)
        .await
        .map_err(|error| format!("Cannot create update file: {}", error))?;
    let mut downloaded_bytes = 0_u64;

    emit_update_progress(app, request, &file_name, downloaded_bytes, total_bytes)?;

    while let Some(chunk_result) = stream.next().await {
        let chunk =
            chunk_result.map_err(|error| format!("Cannot read update download: {}", error))?;
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("Cannot write update file: {}", error))?;
        downloaded_bytes += chunk.len() as u64;
        emit_update_progress(app, request, &file_name, downloaded_bytes, total_bytes)?;
    }

    file.flush()
        .await
        .map_err(|error| format!("Cannot finalize update file: {}", error))?;

    if destination.exists() {
        tokio::fs::remove_file(&destination)
            .await
            .map_err(|error| format!("Cannot replace previous update download: {}", error))?;
    }

    tokio::fs::rename(&partial_destination, &destination)
        .await
        .map_err(|error| format!("Cannot move update download into place: {}", error))?;

    emit_update_progress(app, request, &file_name, downloaded_bytes, total_bytes)?;
    app.emit(
        "app-update-complete",
        AppUpdateProgressPayload {
            version: request.version.clone(),
            file_name: file_name.clone(),
            downloaded_bytes,
            total_bytes,
            percent: 100.0,
        },
    )
    .map_err(|error| format!("Cannot emit update completion: {}", error))?;

    Ok(AppUpdateDownloadResult {
        version: request.version.clone(),
        asset_name: file_name,
        file_path: destination.to_string_lossy().to_string(),
        bytes_written: downloaded_bytes,
    })
}

pub fn apply_update(app: &AppHandle, update_file_path: &str) -> Result<(), String> {
    let update_file = PathBuf::from(update_file_path);

    if !update_file.exists() {
        return Err(format!(
            "Downloaded update file not found: {}",
            update_file.to_string_lossy()
        ));
    }

    let update_file_name = update_file
        .file_name()
        .map(|name| name.to_string_lossy())
        .ok_or_else(|| "Downloaded update file has no file name.".to_string())?;

    if !is_supported_self_update_binary_name(&update_file_name) {
        return Err("Downloaded update is not a supported EmuManager executable.".to_string());
    }

    let current_exe = std::env::current_exe()
        .map_err(|error| format!("Cannot locate current executable: {}", error))?;

    if update_file == current_exe {
        return Err("Update file cannot be the running executable.".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        spawn_windows_swapper(&current_exe, &update_file)?;
        emit_debug_log(
            app,
            "info",
            "app-update",
            "Applying EmuManager update and relaunching",
            Some(format!(
                "current_exe={}\nupdate_file={}",
                current_exe.to_string_lossy(),
                update_file.to_string_lossy()
            )),
        );
        app.exit(0);
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("Self-update apply is currently implemented for Windows only.".to_string())
    }
}

fn emit_update_progress(
    app: &AppHandle,
    request: &AppUpdateDownloadRequest,
    file_name: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) -> Result<(), String> {
    let percent = total_bytes
        .filter(|total| *total > 0)
        .map(|total| ((downloaded_bytes as f64 / total as f64) * 100.0).clamp(0.0, 100.0))
        .unwrap_or(0.0);

    app.emit(
        "app-update-progress",
        AppUpdateProgressPayload {
            version: request.version.clone(),
            file_name: file_name.to_string(),
            downloaded_bytes,
            total_bytes,
            percent,
        },
    )
    .map_err(|error| format!("Cannot emit update progress: {}", error))
}

fn validate_update_download_url(download_url: &str) -> Result<(), String> {
    let url = Url::parse(download_url).map_err(|error| format!("Invalid update URL: {}", error))?;

    if url.scheme() != "https" {
        return Err("Update download URL must use HTTPS.".to_string());
    }

    if url.host_str() != Some("github.com") {
        return Err("Update download URL must come from GitHub releases.".to_string());
    }

    let expected_path_prefix = format!(
        "/{}/{}/releases/download/",
        GITHUB_OWNER.to_ascii_lowercase(),
        GITHUB_REPO.to_ascii_lowercase()
    );
    if !url
        .path()
        .to_ascii_lowercase()
        .starts_with(&expected_path_prefix)
    {
        return Err("Update download URL does not belong to EmuManager releases.".to_string());
    }

    Ok(())
}

fn select_update_asset(assets: &[GithubReleaseAsset]) -> Option<GithubReleaseAsset> {
    assets
        .iter()
        .filter(|asset| {
            asset.browser_download_url.is_some()
                && is_supported_self_update_binary_name(&asset.name)
        })
        .max_by_key(|asset| update_asset_score(&asset.name))
        .cloned()
}

fn is_supported_self_update_binary_name(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();

    if !normalized.contains("emumanager")
        || contains_any(&normalized, &["setup", "installer", "nsis", "msi"])
    {
        return false;
    }

    if cfg!(target_os = "windows") {
        normalized.ends_with(".exe")
    } else {
        false
    }
}

fn update_asset_score(name: &str) -> i32 {
    let normalized = name.to_ascii_lowercase();
    let mut score = 0;

    if normalized.contains("emumanager") {
        score += 100;
    }

    if contains_any(&normalized, &["windows", "win"]) {
        score += 40;
    }

    if contains_any(&normalized, &["x64", "x86_64", "amd64"]) && cfg!(target_arch = "x86_64") {
        score += 30;
    }

    if normalized.ends_with(".exe") {
        score += 20;
    }

    score
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn normalize_release_version(tag_name: &str) -> String {
    tag_name
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

fn compare_versions(left: &str, right: &str) -> Ordering {
    let left = parse_version(left);
    let right = parse_version(right);
    let max_len = left.core.len().max(right.core.len());

    for index in 0..max_len {
        let left_part = *left.core.get(index).unwrap_or(&0);
        let right_part = *right.core.get(index).unwrap_or(&0);

        match left_part.cmp(&right_part) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }

    match (left.pre_release, right.pre_release) {
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(left_pre), Some(right_pre)) => left_pre.cmp(&right_pre),
        (None, None) => Ordering::Equal,
    }
}

#[derive(Debug)]
struct ParsedVersion {
    core: Vec<u64>,
    pre_release: Option<String>,
}

fn parse_version(value: &str) -> ParsedVersion {
    let normalized = normalize_release_version(value);
    let without_build = normalized.split('+').next().unwrap_or(&normalized);
    let mut parts = without_build.splitn(2, '-');
    let core = parts.next().unwrap_or_default();
    let pre_release = parts.next().map(str::to_string);

    ParsedVersion {
        core: core
            .split('.')
            .map(|part| {
                part.chars()
                    .take_while(|character| character.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u64>()
                    .unwrap_or(0)
            })
            .collect(),
        pre_release,
    }
}

fn sanitize_file_name(input: &str) -> String {
    input
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => character,
        })
        .collect()
}

#[cfg(target_os = "windows")]
fn spawn_windows_swapper(current_exe: &Path, update_file: &Path) -> Result<(), String> {
    let script_path = update_file
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            "apply-emumanager-update-{}.ps1",
            std::process::id()
        ));
    let working_directory = current_exe.parent().unwrap_or_else(|| Path::new("."));
    let backup_path = current_exe.with_extension("old");
    let script = format!(
        "$ErrorActionPreference = 'Stop'\n\
         $ProcessIdToWait = {}\n\
         $Source = {}\n\
         $Target = {}\n\
         $Backup = {}\n\
         $WorkingDirectory = {}\n\
         try {{ Wait-Process -Id $ProcessIdToWait -Timeout 60 -ErrorAction SilentlyContinue }} catch {{ }}\n\
         Start-Sleep -Milliseconds 500\n\
         if (Test-Path -LiteralPath $Backup) {{ Remove-Item -LiteralPath $Backup -Force -ErrorAction SilentlyContinue }}\n\
         if (Test-Path -LiteralPath $Target) {{ Move-Item -LiteralPath $Target -Destination $Backup -Force }}\n\
         Move-Item -LiteralPath $Source -Destination $Target -Force\n\
         Start-Process -FilePath $Target -WorkingDirectory $WorkingDirectory\n\
         if (Test-Path -LiteralPath $Backup) {{ Remove-Item -LiteralPath $Backup -Force -ErrorAction SilentlyContinue }}\n\
         Remove-Item -LiteralPath $PSCommandPath -Force -ErrorAction SilentlyContinue\n",
        std::process::id(),
        powershell_quote(update_file),
        powershell_quote(current_exe),
        powershell_quote(&backup_path),
        powershell_quote(working_directory),
    );

    fs::write(&script_path, script)
        .map_err(|error| format!("Cannot write update swap script: {}", error))?;

    Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script_path.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Cannot launch update swap script: {}", error))?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semver_like_versions() {
        assert_eq!(compare_versions("0.1.0", "0.1.1"), Ordering::Less);
        assert_eq!(compare_versions("v1.2.0", "1.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.2.0-beta", "1.2.0"), Ordering::Less);
        assert_eq!(compare_versions("1.3.0", "1.2.9"), Ordering::Greater);
    }

    #[test]
    fn selects_portable_windows_executable_asset() {
        let assets = vec![
            GithubReleaseAsset {
                name: "EmuManager-setup.exe".to_string(),
                size: Some(100),
                browser_download_url: Some(
                    "https://github.com/TilioChr/EmuManager/releases/download/v1/setup.exe"
                        .to_string(),
                ),
            },
            GithubReleaseAsset {
                name: "EmuManager-windows-x64.exe".to_string(),
                size: Some(100),
                browser_download_url: Some(
                    "https://github.com/TilioChr/EmuManager/releases/download/v1/EmuManager.exe"
                        .to_string(),
                ),
            },
        ];

        let selected = select_update_asset(&assets).expect("expected update asset");

        assert_eq!(selected.name, "EmuManager-windows-x64.exe");
    }
}
