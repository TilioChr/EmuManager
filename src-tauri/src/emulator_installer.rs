use crate::debug_log::emit_debug_log;
use crate::emulator_registry::{
    built_in_emulators, EmulatorDefinition, EmulatorDownloadSource, ReleaseAssetFilter,
    ReleaseAssetPlatform,
};
use crate::portable_paths::PortablePaths;
use futures_util::StreamExt;
use reqwest::header::{ACCEPT, USER_AGENT};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use zip::ZipArchive;

const RELEASE_CACHE_FILE_NAME: &str = "release-download-cache.json";
const RELEASE_CACHE_TTL_MS: u64 = 24 * 60 * 60 * 1000;
const RELEASE_API_USER_AGENT: &str = "EmuManager";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub emulator_id: String,
    pub install_path: String,
    pub executable_path: String,
    pub archive_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallResult {
    pub emulator_id: String,
    pub install_path: String,
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorInstallProgressPayload {
    pub emulator_id: String,
    pub file_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: f64,
}

#[derive(Debug, Clone)]
struct ResolvedDownloadAsset {
    download_url: String,
    file_name: String,
    tag_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseCacheFile {
    entries: HashMap<String, ReleaseCacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseCacheEntry {
    resolved_at_ms: u64,
    download_url: String,
    file_name: String,
    tag_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct LatestReleaseResponse {
    #[serde(default)]
    tag_name: Option<String>,
    #[serde(default)]
    assets: Vec<LatestReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct LatestReleaseAsset {
    name: String,
    #[serde(default)]
    browser_download_url: Option<String>,
}

impl Default for ReleaseCacheFile {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

pub fn is_emulator_installed(paths: &PortablePaths, emulator_id: &str) -> bool {
    resolve_emulator_executable(paths, emulator_id).is_ok()
}

pub fn get_installed_emulator_version(
    paths: &PortablePaths,
    emulator_id: &str,
) -> Result<Option<String>, String> {
    let executable = match resolve_emulator_executable(paths, emulator_id) {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };

    read_windows_exe_version(&executable).map(Some)
}

pub async fn install_emulator(
    app: &AppHandle,
    paths: &PortablePaths,
    emulator_id: &str,
) -> Result<InstallResult, String> {
    let definition = get_emulator_definition(emulator_id)
        .ok_or_else(|| format!("Émulateur non supporté: {}", emulator_id))?;

    let download_asset = resolve_download_asset(app, paths, &definition).await?;

    let emu_root = PathBuf::from(&paths.emu);
    let install_dir = emu_root.join(definition.install_dir_name);
    let temp_dir = PathBuf::from(&paths.data).join("downloads");

    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("Impossible de créer le dossier d'installation: {}", error))?;
    fs::create_dir_all(&temp_dir)
        .map_err(|error| format!("Impossible de créer le dossier temporaire: {}", error))?;

    let archive_format = archive_format_from_file_name(&download_asset.file_name)?;
    let archive_file_name = sanitize_file_name(&download_asset.file_name);
    let archive_path = temp_dir.join(format!("{}-{}", definition.id, archive_file_name));

    emit_debug_log(
        app,
        "info",
        "emulator-install",
        &format!("Resolved {} download asset", definition.name),
        Some(format!(
            "file_name={}\ntag_name={}\nurl={}",
            download_asset.file_name,
            download_asset.tag_name.as_deref().unwrap_or("unknown"),
            download_asset.download_url
        )),
    );

    download_file(
        app,
        &download_asset.download_url,
        &archive_path,
        definition.id,
        &archive_file_name,
    )
    .await?;

    if install_dir.exists() {
        fs::remove_dir_all(&install_dir).map_err(|error| {
            format!("Impossible de nettoyer l'ancienne installation: {}", error)
        })?;
    }

    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("Impossible de recréer le dossier d'installation: {}", error))?;

    emit_debug_log(
        app,
        "info",
        "emulator-install",
        &format!("Extracting {} archive", definition.name),
        Some(format!(
            "archive_path={}\ninstall_dir={}",
            archive_path.to_string_lossy(),
            install_dir.to_string_lossy()
        )),
    );

    match archive_format.as_str() {
        "zip" => extract_zip(&archive_path, &install_dir)?,
        "7z" => sevenz_rust::decompress_file(&archive_path, &install_dir)
            .map_err(|error| format!("Extraction impossible: {}", error))?,
        other => return Err(format!("Format d'archive non supporté: {}", other)),
    }

    emit_debug_log(
        app,
        "success",
        "emulator-install",
        &format!("Extracted {} archive", definition.name),
        Some(format!("install_dir={}", install_dir.to_string_lossy())),
    );

    let executable =
        resolve_executable_in_install_dir(&install_dir, &definition).ok_or_else(|| {
            format!(
                "Installation terminée mais exécutable introuvable pour {}",
                definition.name
            )
        })?;

    Ok(InstallResult {
        emulator_id: definition.id.to_string(),
        install_path: install_dir.to_string_lossy().to_string(),
        executable_path: executable.to_string_lossy().to_string(),
        archive_path: archive_path.to_string_lossy().to_string(),
    })
}

pub fn resolve_emulator_executable(
    paths: &PortablePaths,
    emulator_id: &str,
) -> Result<PathBuf, String> {
    let definition = get_emulator_definition(emulator_id)
        .ok_or_else(|| format!("Émulateur non supporté: {}", emulator_id))?;

    let install_dir = PathBuf::from(&paths.emu).join(definition.install_dir_name);

    if !install_dir.exists() {
        return Err(format!(
            "Dossier d'installation introuvable: {}",
            install_dir.to_string_lossy()
        ));
    }

    resolve_executable_in_install_dir(&install_dir, &definition).ok_or_else(|| {
        format!(
            "Exécutable introuvable pour {} dans {}",
            definition.name,
            install_dir.to_string_lossy()
        )
    })
}

pub fn uninstall_emulator(
    paths: &PortablePaths,
    emulator_id: &str,
) -> Result<UninstallResult, String> {
    let definition = get_emulator_definition(emulator_id)
        .ok_or_else(|| format!("Emulateur non supporte: {}", emulator_id))?;
    let emu_root = PathBuf::from(&paths.emu);
    let install_dir = emu_root.join(definition.install_dir_name);
    let install_path = install_dir.to_string_lossy().to_string();

    if !install_dir.exists() {
        return Ok(UninstallResult {
            emulator_id: definition.id.to_string(),
            install_path,
            removed: false,
        });
    }

    if !install_dir.is_dir() {
        return Err(format!(
            "Le chemin d'installation n'est pas un dossier: {}",
            install_path
        ));
    }

    let canonical_emu_root = fs::canonicalize(&emu_root)
        .map_err(|error| format!("Impossible de verifier le dossier Emu: {}", error))?;
    let canonical_install_dir = fs::canonicalize(&install_dir)
        .map_err(|error| format!("Impossible de verifier l'installation: {}", error))?;

    if !canonical_install_dir.starts_with(&canonical_emu_root) {
        return Err("Desinstallation refusee: dossier hors de Emu.".to_string());
    }

    fs::remove_dir_all(&canonical_install_dir)
        .map_err(|error| format!("Impossible de supprimer l'emulateur: {}", error))?;

    Ok(UninstallResult {
        emulator_id: definition.id.to_string(),
        install_path,
        removed: true,
    })
}

fn get_emulator_definition(emulator_id: &str) -> Option<EmulatorDefinition> {
    built_in_emulators()
        .into_iter()
        .find(|entry| entry.id == emulator_id)
}

async fn resolve_download_asset(
    app: &AppHandle,
    paths: &PortablePaths,
    definition: &EmulatorDefinition,
) -> Result<ResolvedDownloadAsset, String> {
    let source = definition
        .download_source
        .as_ref()
        .ok_or_else(|| format!("Installateur non implémenté pour {}", definition.id))?;

    match source {
        EmulatorDownloadSource::GitHubLatest(source) => {
            let api_url = format!(
                "https://api.github.com/repos/{}/{}/releases/latest",
                source.owner, source.repo
            );
            let source_key = format!("github:{}/{}", source.owner, source.repo);
            resolve_latest_release_asset(
                app,
                paths,
                definition,
                &source_key,
                &api_url,
                &source.asset_filters,
            )
            .await
        }
        EmulatorDownloadSource::GenericLatestReleaseApi(source) => {
            resolve_latest_release_asset(
                app,
                paths,
                definition,
                source.cache_key,
                source.api_url,
                &source.asset_filters,
            )
            .await
        }
        EmulatorDownloadSource::Direct(source) => {
            let file_name = file_name_from_url(source.url).ok_or_else(|| {
                format!(
                    "Impossible de résoudre le nom du fichier depuis l'URL de {}",
                    definition.name
                )
            })?;

            Ok(ResolvedDownloadAsset {
                download_url: source.url.to_string(),
                file_name,
                tag_name: None,
            })
        }
    }
}

async fn resolve_latest_release_asset(
    app: &AppHandle,
    paths: &PortablePaths,
    definition: &EmulatorDefinition,
    source_key: &str,
    api_url: &str,
    asset_filters: &[ReleaseAssetFilter],
) -> Result<ResolvedDownloadAsset, String> {
    let platform = current_release_platform().ok_or_else(|| {
        "Aucun filtre de téléchargement n'est configuré pour cette plateforme.".to_string()
    })?;
    let cache_key = format!(
        "{}:{}:{}",
        definition.id,
        source_key,
        release_platform_cache_key(platform)
    );
    let now_ms = current_time_ms();
    let mut cache = load_release_cache(paths);

    if let Some(entry) = cache.entries.get(&cache_key) {
        if now_ms.saturating_sub(entry.resolved_at_ms) <= RELEASE_CACHE_TTL_MS {
            emit_debug_log(
                app,
                "debug",
                "emulator-install",
                &format!("Using cached {} release asset", definition.name),
                Some(format!(
                    "file_name={}\ntag_name={}\ncache_key={}",
                    entry.file_name,
                    entry.tag_name.as_deref().unwrap_or("unknown"),
                    cache_key
                )),
            );

            return Ok(ResolvedDownloadAsset {
                download_url: entry.download_url.clone(),
                file_name: entry.file_name.clone(),
                tag_name: entry.tag_name.clone(),
            });
        }
    }

    if !asset_filters
        .iter()
        .any(|filter| filter.platform == platform)
    {
        return Err(format!(
            "Aucun filtre d'asset {} n'est configuré pour {}.",
            release_platform_cache_key(platform),
            definition.name
        ));
    }

    emit_debug_log(
        app,
        "debug",
        "emulator-install",
        &format!("Resolving latest {} release asset", definition.name),
        Some(format!("api_url={}", api_url)),
    );

    let client = Client::new();
    let response = client
        .get(api_url)
        .header(USER_AGENT, RELEASE_API_USER_AGENT)
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("Impossible de résoudre la dernière release: {}", error))?;

    if !response.status().is_success() {
        return Err(format!(
            "Résolution de la dernière release échouée avec le statut {}",
            response.status()
        ));
    }

    let raw = response
        .text()
        .await
        .map_err(|error| format!("Impossible de lire la réponse release: {}", error))?;
    let release = serde_json::from_str::<LatestReleaseResponse>(&raw)
        .map_err(|error| format!("Réponse release invalide: {}", error))?;

    let asset =
        select_release_asset(&release.assets, asset_filters, platform).ok_or_else(|| {
            let available_assets = release
                .assets
                .iter()
                .map(|asset| asset.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Aucun asset compatible trouvé pour {}. Assets disponibles: {}",
                definition.name, available_assets
            )
        })?;
    let download_url = asset.browser_download_url.clone().ok_or_else(|| {
        format!(
            "Asset compatible trouvé pour {}, mais sans URL de téléchargement.",
            definition.name
        )
    })?;

    let resolved = ResolvedDownloadAsset {
        download_url,
        file_name: asset.name.clone(),
        tag_name: release.tag_name.clone(),
    };

    cache.entries.insert(
        cache_key.clone(),
        ReleaseCacheEntry {
            resolved_at_ms: now_ms,
            download_url: resolved.download_url.clone(),
            file_name: resolved.file_name.clone(),
            tag_name: resolved.tag_name.clone(),
        },
    );

    if let Err(error) = save_release_cache(paths, &cache) {
        emit_debug_log(
            app,
            "warning",
            "emulator-install",
            "Release cache write failed",
            Some(format!("cache_key={}\nerror={}", cache_key, error)),
        );
    }

    Ok(resolved)
}

fn select_release_asset(
    assets: &[LatestReleaseAsset],
    filters: &[ReleaseAssetFilter],
    platform: ReleaseAssetPlatform,
) -> Option<LatestReleaseAsset> {
    for filter in filters.iter().filter(|filter| filter.platform == platform) {
        if let Some(asset) = assets.iter().find(|asset| {
            asset.browser_download_url.is_some() && asset_matches_filter(asset, filter)
        }) {
            return Some(asset.clone());
        }
    }

    None
}

fn asset_matches_filter(asset: &LatestReleaseAsset, filter: &ReleaseAssetFilter) -> bool {
    let name = asset.name.to_ascii_lowercase();

    let matches_required = filter
        .required_substrings
        .iter()
        .all(|value| name.contains(&value.to_ascii_lowercase()));
    let matches_exclusions = filter
        .excluded_substrings
        .iter()
        .all(|value| !name.contains(&value.to_ascii_lowercase()));
    let matches_extension = filter
        .extensions
        .iter()
        .any(|extension| name.ends_with(&extension.to_ascii_lowercase()));

    matches_required && matches_exclusions && matches_extension
}

fn archive_format_from_file_name(file_name: &str) -> Result<String, String> {
    let normalized = file_name.to_ascii_lowercase();

    if normalized.ends_with(".zip") {
        return Ok("zip".to_string());
    }

    if normalized.ends_with(".7z") {
        return Ok("7z".to_string());
    }

    Err(format!(
        "Format d'archive non supporté pour l'asset {}",
        file_name
    ))
}

fn load_release_cache(paths: &PortablePaths) -> ReleaseCacheFile {
    let cache_path = release_cache_path(paths);

    fs::read_to_string(cache_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<ReleaseCacheFile>(&raw).ok())
        .unwrap_or_default()
}

fn save_release_cache(paths: &PortablePaths, cache: &ReleaseCacheFile) -> Result<(), String> {
    let cache_path = release_cache_path(paths);

    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Impossible de créer le dossier cache: {}", error))?;
    }

    let raw = serde_json::to_string_pretty(cache)
        .map_err(|error| format!("Impossible de sérialiser le cache release: {}", error))?;
    fs::write(cache_path, raw)
        .map_err(|error| format!("Impossible d'écrire le cache release: {}", error))
}

fn release_cache_path(paths: &PortablePaths) -> PathBuf {
    Path::new(&paths.data).join(RELEASE_CACHE_FILE_NAME)
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn current_release_platform() -> Option<ReleaseAssetPlatform> {
    if cfg!(target_os = "windows") {
        Some(ReleaseAssetPlatform::Windows)
    } else if cfg!(target_os = "macos") {
        Some(ReleaseAssetPlatform::Macos)
    } else if cfg!(target_os = "linux") {
        Some(ReleaseAssetPlatform::Linux)
    } else {
        None
    }
}

fn release_platform_cache_key(platform: ReleaseAssetPlatform) -> &'static str {
    match platform {
        ReleaseAssetPlatform::Windows => "windows",
        ReleaseAssetPlatform::Macos => "macos",
        ReleaseAssetPlatform::Linux => "linux",
    }
}

fn file_name_from_url(url: &str) -> Option<String> {
    let trimmed = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .trim_end_matches('/');

    trimmed
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn release_asset(name: &str) -> LatestReleaseAsset {
        LatestReleaseAsset {
            name: name.to_string(),
            browser_download_url: Some(format!("https://example.test/{}", name)),
        }
    }

    #[test]
    fn selects_matching_portable_windows_asset() {
        let assets = vec![
            release_asset("PCSX2-v2.6.3-windows-x64-installer.exe"),
            release_asset("pcsx2-v2.6.3-windows-x64-Qt-symbols.7z"),
            release_asset("pcsx2-v2.6.3-windows-x64-Qt.7z"),
        ];
        let filters = vec![ReleaseAssetFilter {
            platform: ReleaseAssetPlatform::Windows,
            required_substrings: vec!["windows", "x64", "qt"],
            excluded_substrings: vec!["installer", "symbols"],
            extensions: vec![".7z"],
        }];

        let selected = select_release_asset(&assets, &filters, ReleaseAssetPlatform::Windows)
            .expect("expected matching asset");

        assert_eq!(selected.name, "pcsx2-v2.6.3-windows-x64-Qt.7z");
    }

    #[test]
    fn infers_supported_archive_formats() {
        assert_eq!(archive_format_from_file_name("emu.zip").unwrap(), "zip");
        assert_eq!(archive_format_from_file_name("emu.7z").unwrap(), "7z");
        assert!(archive_format_from_file_name("installer.exe").is_err());
    }

    #[test]
    fn extracts_file_name_from_download_url() {
        assert_eq!(
            file_name_from_url("https://example.test/releases/emu.zip?download=1"),
            Some("emu.zip".to_string())
        );
    }
}

fn read_windows_exe_version(executable: &Path) -> Result<String, String> {
    let path = executable.to_string_lossy().replace('\'', "''");
    let script = format!("(Get-Item '{}').VersionInfo.ProductVersion", path);

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .map_err(|error| format!("Impossible de lire la version de l'exécutable: {}", error))?;

    if !output.status.success() {
        return Err("Impossible de lire la version Windows de l'exécutable.".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err("Version exécutable vide.".to_string());
    }

    Ok(stdout)
}

async fn download_file(
    app: &AppHandle,
    url: &str,
    destination: &Path,
    emulator_id: &str,
    file_name: &str,
) -> Result<(), String> {
    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Téléchargement impossible: {}", error))?;

    if !response.status().is_success() {
        return Err(format!(
            "Téléchargement échoué avec le statut {}",
            response.status()
        ));
    }

    let total_bytes = response.content_length();
    let mut stream = response.bytes_stream();

    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|error| format!("Création du fichier temporaire impossible: {}", error))?;

    let mut downloaded_bytes: u64 = 0;

    emit_install_progress(
        app,
        emulator_id,
        file_name,
        downloaded_bytes,
        total_bytes,
        0.0,
    )?;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result
            .map_err(|error| format!("Lecture du téléchargement impossible: {}", error))?;

        file.write_all(&chunk)
            .await
            .map_err(|error| format!("Écriture du fichier temporaire impossible: {}", error))?;

        downloaded_bytes += chunk.len() as u64;

        let percent = if let Some(total) = total_bytes {
            if total > 0 {
                ((downloaded_bytes as f64 / total as f64) * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        emit_install_progress(
            app,
            emulator_id,
            file_name,
            downloaded_bytes,
            total_bytes,
            percent,
        )?;
    }

    file.flush()
        .await
        .map_err(|error| format!("Flush du fichier temporaire impossible: {}", error))?;

    emit_install_progress(
        app,
        emulator_id,
        file_name,
        downloaded_bytes,
        total_bytes,
        100.0,
    )?;
    app.emit(
        "emulator-install-complete",
        EmulatorInstallProgressPayload {
            emulator_id: emulator_id.to_string(),
            file_name: file_name.to_string(),
            downloaded_bytes,
            total_bytes,
            percent: 100.0,
        },
    )
    .map_err(|error| format!("Impossible d'emettre la fin d'installation: {}", error))?;

    Ok(())
}

fn emit_install_progress(
    app: &AppHandle,
    emulator_id: &str,
    file_name: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    percent: f64,
) -> Result<(), String> {
    app.emit(
        "emulator-install-progress",
        EmulatorInstallProgressPayload {
            emulator_id: emulator_id.to_string(),
            file_name: file_name.to_string(),
            downloaded_bytes,
            total_bytes,
            percent,
        },
    )
    .map_err(|error| {
        format!(
            "Impossible d'emettre la progression d'installation: {}",
            error
        )
    })
}

fn extract_zip(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path)
        .map_err(|error| format!("Impossible d'ouvrir l'archive zip: {}", error))?;
    let mut archive = ZipArchive::new(file).map_err(|error| format!("Zip invalide: {}", error))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Impossible de lire une entrée zip: {}", error))?;

        let enclosed = entry
            .enclosed_name()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "Entrée zip invalide".to_string())?;

        let out_path = destination.join(enclosed);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|error| format!("Impossible de créer un dossier extrait: {}", error))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("Impossible de créer le dossier parent extrait: {}", error)
            })?;
        }

        let mut outfile = fs::File::create(&out_path)
            .map_err(|error| format!("Impossible de créer un fichier extrait: {}", error))?;

        io::copy(&mut entry, &mut outfile)
            .map_err(|error| format!("Impossible d'extraire un fichier zip: {}", error))?;
    }

    Ok(())
}

fn resolve_executable_in_install_dir(
    install_dir: &Path,
    definition: &EmulatorDefinition,
) -> Option<PathBuf> {
    let direct = install_dir.join(definition.executable_rel_path);
    if direct.exists() {
        return Some(direct);
    }

    let wanted: Vec<String> = definition
        .executable_name_candidates
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect();

    find_first_matching_exe(install_dir, &wanted)
}

fn find_first_matching_exe(dir: &Path, wanted_names: &[String]) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(found) = find_first_matching_exe(&path, wanted_names) {
                return Some(found);
            }
            continue;
        }

        let file_name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
        if wanted_names.iter().any(|wanted| wanted == &file_name) {
            return Some(path);
        }
    }

    None
}
