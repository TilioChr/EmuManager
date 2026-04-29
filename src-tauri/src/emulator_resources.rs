use crate::emulator_installer::resolve_emulator_executable;
use crate::emulator_registry::built_in_emulators;
use crate::portable_paths::PortablePaths;
use futures_util::StreamExt;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use zip::ZipArchive;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RommResourceSession {
    pub base_url: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorResourceRequirement {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub required: bool,
    pub install_hint: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorResourceStatus {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub required: bool,
    pub state: String,
    pub installed_path: Option<String>,
    pub message: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorResourceSummary {
    pub emulator_id: String,
    pub emulator_name: String,
    pub requirements: Vec<EmulatorResourceRequirement>,
    pub statuses: Vec<EmulatorResourceStatus>,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledResource {
    pub resource_id: String,
    pub resource_label: String,
    pub source_file_name: String,
    pub destination_path: String,
    pub verified_by_romm: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInstallResult {
    pub emulator_id: String,
    pub installed: Vec<InstalledResource>,
    pub summary: EmulatorResourceSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceInstallProgressPayload {
    emulator_id: String,
    resource_id: String,
    resource_label: String,
    stage: String,
    message: String,
    percent: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RommPlatform {
    id: Value,
    name: Option<String>,
    slug: Option<String>,
    #[serde(alias = "fs_slug")]
    fs_slug: Option<String>,
    #[serde(alias = "custom_name")]
    custom_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RommFirmware {
    id: Value,
    #[serde(alias = "file_name")]
    file_name: String,
    #[serde(default)]
    #[serde(alias = "file_path")]
    file_path: Option<String>,
    #[serde(default)]
    #[serde(alias = "file_size_bytes")]
    file_size_bytes: Option<u64>,
    #[serde(default)]
    #[serde(alias = "is_verified")]
    is_verified: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RommListPayload<T> {
    Direct(Vec<T>),
    Wrapped {
        items: Option<Vec<T>>,
        results: Option<Vec<T>>,
    },
}

struct FirmwareSearchResult {
    entries: Vec<RommFirmware>,
    platform_scoped: bool,
}

pub fn list_emulator_resource_summaries(paths: &PortablePaths) -> Vec<EmulatorResourceSummary> {
    built_in_emulators()
        .into_iter()
        .map(|emulator| emulator_resource_summary(paths, emulator.id))
        .collect()
}

pub fn emulator_resource_summary(
    paths: &PortablePaths,
    emulator_id: &str,
) -> EmulatorResourceSummary {
    let emulator_name = built_in_emulators()
        .into_iter()
        .find(|entry| entry.id == emulator_id)
        .map(|entry| entry.name.to_string())
        .unwrap_or_else(|| emulator_id.to_string());
    let requirements = resource_requirements(emulator_id);
    let statuses = requirements
        .iter()
        .map(|requirement| detect_resource(paths, emulator_id, requirement))
        .collect::<Vec<_>>();
    let ready = statuses
        .iter()
        .all(|status| !status.required || status.state == "valid");

    EmulatorResourceSummary {
        emulator_id: emulator_id.to_string(),
        emulator_name,
        requirements,
        statuses,
        ready,
    }
}

pub fn validate_required_resources(paths: &PortablePaths, emulator_id: &str) -> Result<(), String> {
    let summary = emulator_resource_summary(paths, emulator_id);

    if summary.ready {
        return Ok(());
    }

    Err(format_resource_error(&summary))
}

pub async fn install_required_resources(
    app: &AppHandle,
    paths: &PortablePaths,
    emulator_id: &str,
    session: &RommResourceSession,
) -> Result<ResourceInstallResult, String> {
    let initial_summary = emulator_resource_summary(paths, emulator_id);
    if initial_summary.requirements.is_empty() || initial_summary.ready {
        return Ok(ResourceInstallResult {
            emulator_id: emulator_id.to_string(),
            installed: Vec::new(),
            summary: initial_summary,
        });
    }

    let mut installed = Vec::new();

    for status in initial_summary
        .statuses
        .iter()
        .filter(|status| status.required && status.state != "valid")
    {
        match (emulator_id, status.id.as_str()) {
            ("pcsx2", "bios") => {
                installed.extend(install_pcsx2_bios(app, paths, session).await?);
            }
            ("eden", "keys") => {
                installed.extend(install_eden_keys(app, paths, session).await?);
            }
            ("eden", "firmware") => {
                installed.extend(install_eden_firmware(app, paths, session).await?);
            }
            _ => {}
        }
    }

    let summary = emulator_resource_summary(paths, emulator_id);
    if !summary.ready {
        return Err(format_resource_error(&summary));
    }

    Ok(ResourceInstallResult {
        emulator_id: emulator_id.to_string(),
        installed,
        summary,
    })
}

pub fn format_resource_error(summary: &EmulatorResourceSummary) -> String {
    let missing = summary
        .statuses
        .iter()
        .filter(|status| status.required && status.state != "valid")
        .map(|status| format!("{} ({})", status.label, status.message))
        .collect::<Vec<_>>()
        .join(", ");

    if missing.is_empty() {
        format!("{} is ready.", summary.emulator_name)
    } else {
        format!(
            "{} requires valid resources before launch: {}.",
            summary.emulator_name, missing
        )
    }
}

pub fn ensure_local_resource_configuration(
    paths: &PortablePaths,
    emulator_id: &str,
) -> Result<(), String> {
    match emulator_id {
        "pcsx2" => ensure_pcsx2_bios_configuration(paths),
        _ => Ok(()),
    }
}

fn resource_requirements(emulator_id: &str) -> Vec<EmulatorResourceRequirement> {
    match emulator_id {
        "pcsx2" => vec![requirement(
            "bios",
            "BIOS",
            "bios",
            "Place a PS2 BIOS dump in PCSX2/bios or store it in RomM firmware for PlayStation 2.",
        )],
        "eden" => vec![
            requirement(
                "firmware",
                "Firmware",
                "firmware",
                "Install Nintendo Switch firmware files into Eden's user NAND.",
            ),
            requirement(
                "keys",
                "Key",
                "keys",
                "Install prod.keys into Eden's user/keys directory.",
            ),
        ],
        _ => Vec::new(),
    }
}

fn requirement(
    id: &str,
    label: &str,
    kind: &str,
    install_hint: &str,
) -> EmulatorResourceRequirement {
    EmulatorResourceRequirement {
        id: id.to_string(),
        label: label.to_string(),
        kind: kind.to_string(),
        required: true,
        install_hint: install_hint.to_string(),
    }
}

fn detect_resource(
    paths: &PortablePaths,
    emulator_id: &str,
    requirement: &EmulatorResourceRequirement,
) -> EmulatorResourceStatus {
    match (emulator_id, requirement.id.as_str()) {
        ("pcsx2", "bios") => detect_pcsx2_bios(paths, requirement),
        ("eden", "keys") => detect_eden_keys(paths, requirement),
        ("eden", "firmware") => detect_eden_firmware(paths, requirement),
        _ => EmulatorResourceStatus {
            id: requirement.id.clone(),
            label: requirement.label.clone(),
            kind: requirement.kind.clone(),
            required: requirement.required,
            state: "valid".to_string(),
            installed_path: None,
            message: "No required resource.".to_string(),
            file_count: 0,
        },
    }
}

fn detect_pcsx2_bios(
    paths: &PortablePaths,
    requirement: &EmulatorResourceRequirement,
) -> EmulatorResourceStatus {
    let bios_dir = pcsx2_working_dir(paths).join("bios");
    let files = collect_matching_files(&bios_dir, is_pcsx2_bios_file_name);
    let invalid_count = files.iter().filter(|path| !is_non_empty_file(path)).count();
    let state = if !files.is_empty() && invalid_count == 0 {
        "valid"
    } else if !files.is_empty() {
        "invalid"
    } else {
        "missing"
    };
    let message = match state {
        "valid" => format!("{} BIOS file(s) found.", files.len()),
        "invalid" => "BIOS file is empty or unreadable.".to_string(),
        _ => "No PS2 BIOS file found in PCSX2/bios.".to_string(),
    };

    status_from_detection(requirement, state, &bios_dir, message, files.len())
}

fn detect_eden_keys(
    paths: &PortablePaths,
    requirement: &EmulatorResourceRequirement,
) -> EmulatorResourceStatus {
    let keys_file = eden_base_user_dir(paths).join("keys").join("prod.keys");
    let state = if is_non_empty_file(&keys_file) {
        "valid"
    } else if keys_file.exists() {
        "invalid"
    } else {
        "missing"
    };
    let message = match state {
        "valid" => "prod.keys found.".to_string(),
        "invalid" => "prod.keys is empty or unreadable.".to_string(),
        _ => "prod.keys is missing from Eden user/keys.".to_string(),
    };

    status_from_detection(requirement, state, &keys_file, message, usize::from(state == "valid"))
}

fn detect_eden_firmware(
    paths: &PortablePaths,
    requirement: &EmulatorResourceRequirement,
) -> EmulatorResourceStatus {
    let registered_dir = eden_registered_dir(&eden_base_user_dir(paths));
    let files = collect_matching_files(&registered_dir, is_eden_firmware_file_name);
    let invalid_count = files.iter().filter(|path| !is_non_empty_file(path)).count();
    let state = if !files.is_empty() && invalid_count == 0 {
        "valid"
    } else if !files.is_empty() {
        "invalid"
    } else {
        "missing"
    };
    let message = match state {
        "valid" => format!("{} firmware file(s) found.", files.len()),
        "invalid" => "Firmware file is empty or unreadable.".to_string(),
        _ => "No Switch firmware content found in Eden user/nand.".to_string(),
    };

    status_from_detection(requirement, state, &registered_dir, message, files.len())
}

fn status_from_detection(
    requirement: &EmulatorResourceRequirement,
    state: &str,
    path: &Path,
    message: String,
    file_count: usize,
) -> EmulatorResourceStatus {
    EmulatorResourceStatus {
        id: requirement.id.clone(),
        label: requirement.label.clone(),
        kind: requirement.kind.clone(),
        required: requirement.required,
        state: state.to_string(),
        installed_path: Some(path.to_string_lossy().to_string()),
        message,
        file_count,
    }
}

async fn install_pcsx2_bios(
    app: &AppHandle,
    paths: &PortablePaths,
    session: &RommResourceSession,
) -> Result<Vec<InstalledResource>, String> {
    emit_resource_progress(
        app,
        "pcsx2",
        "bios",
        "BIOS",
        "search",
        "Searching RomM for PlayStation 2 BIOS files",
        None,
    )?;

    let firmware = fetch_platform_firmware(session, &["ps2", "playstation 2"]).await?;
    let candidates = firmware
        .entries
        .into_iter()
        .filter(|entry| is_pcsx2_bios_candidate(entry, firmware.platform_scoped))
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        return Err(
            "No PS2 BIOS file was found in RomM firmware. Add a BIOS dump to the PlayStation 2 platform in RomM."
                .to_string(),
        );
    }

    let client = Client::new();
    let target_dir = pcsx2_working_dir(paths).join("bios");
    fs::create_dir_all(&target_dir)
        .map_err(|error| format!("Cannot create PCSX2 bios directory: {}", error))?;

    let mut installed = Vec::new();
    for firmware in candidates {
        let source = download_firmware_file(app, paths, &client, session, "pcsx2", "bios", "BIOS", &firmware).await?;
        let destinations = install_file_or_zip_flat(
            &source,
            &target_dir,
            is_pcsx2_bios_file_name,
            None,
        )?;

        for destination in destinations {
            installed.push(installed_resource(
                "bios",
                "BIOS",
                &firmware,
                &destination,
            ));
        }
    }

    ensure_pcsx2_bios_configuration(paths)?;

    emit_resource_progress(
        app,
        "pcsx2",
        "bios",
        "BIOS",
        "complete",
        "PCSX2 BIOS installed",
        Some(100.0),
    )?;

    Ok(installed)
}

async fn install_eden_keys(
    app: &AppHandle,
    paths: &PortablePaths,
    session: &RommResourceSession,
) -> Result<Vec<InstalledResource>, String> {
    emit_resource_progress(
        app,
        "eden",
        "keys",
        "Key",
        "search",
        "Searching RomM for Switch prod.keys",
        None,
    )?;

    let firmware = fetch_platform_firmware(session, &["switch", "nintendo switch", "nsw"]).await?;
    let candidates = firmware
        .entries
        .into_iter()
        .filter(|entry| is_eden_key_candidate(entry))
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        return Err(
            "No prod.keys file was found in RomM firmware. Add prod.keys to the Nintendo Switch platform in RomM."
                .to_string(),
        );
    }

    let client = Client::new();
    let roots = eden_user_roots(paths);
    let mut installed = Vec::new();

    for firmware in candidates {
        let source = download_firmware_file(app, paths, &client, session, "eden", "keys", "Key", &firmware).await?;

        for root in &roots {
            let target_dir = root.join("keys");
            fs::create_dir_all(&target_dir)
                .map_err(|error| format!("Cannot create Eden keys directory: {}", error))?;
            let destinations = install_file_or_zip_flat(
                &source,
                &target_dir,
                is_eden_key_file_name,
                Some("prod.keys"),
            )?;

            for destination in destinations {
                installed.push(installed_resource(
                    "keys",
                    "Key",
                    &firmware,
                    &destination,
                ));
            }
        }
    }

    emit_resource_progress(
        app,
        "eden",
        "keys",
        "Key",
        "complete",
        "Eden key installed",
        Some(100.0),
    )?;

    Ok(installed)
}

async fn install_eden_firmware(
    app: &AppHandle,
    paths: &PortablePaths,
    session: &RommResourceSession,
) -> Result<Vec<InstalledResource>, String> {
    emit_resource_progress(
        app,
        "eden",
        "firmware",
        "Firmware",
        "search",
        "Searching RomM for Switch firmware",
        None,
    )?;

    let firmware = fetch_platform_firmware(session, &["switch", "nintendo switch", "nsw"]).await?;
    let candidates = firmware
        .entries
        .into_iter()
        .filter(|entry| is_eden_firmware_candidate(entry, firmware.platform_scoped))
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        return Err(
            "No Switch firmware file was found in RomM firmware. Add a firmware zip or .nca files to the Nintendo Switch platform in RomM."
                .to_string(),
        );
    }

    let client = Client::new();
    let roots = eden_user_roots(paths);
    let mut installed = Vec::new();

    for firmware in candidates {
        let source =
            download_firmware_file(app, paths, &client, session, "eden", "firmware", "Firmware", &firmware).await?;

        for root in &roots {
            let target_dir = eden_registered_dir(root);
            fs::create_dir_all(&target_dir)
                .map_err(|error| format!("Cannot create Eden firmware directory: {}", error))?;
            let destinations = install_file_or_zip_flat(
                &source,
                &target_dir,
                is_eden_firmware_file_name,
                None,
            )?;

            for destination in destinations {
                installed.push(installed_resource(
                    "firmware",
                    "Firmware",
                    &firmware,
                    &destination,
                ));
            }
        }
    }

    emit_resource_progress(
        app,
        "eden",
        "firmware",
        "Firmware",
        "complete",
        "Eden firmware installed",
        Some(100.0),
    )?;

    Ok(installed)
}

async fn fetch_platform_firmware(
    session: &RommResourceSession,
    platform_tokens: &[&str],
) -> Result<FirmwareSearchResult, String> {
    let client = Client::new();
    let platforms = fetch_romm_platforms(&client, session).await.unwrap_or_default();
    let platform_ids = platforms
        .iter()
        .filter(|platform| platform_matches(platform, platform_tokens))
        .filter_map(|platform| value_to_string(&platform.id))
        .collect::<Vec<_>>();

    let mut entries = Vec::new();
    for platform_id in &platform_ids {
        entries.extend(fetch_romm_firmware(&client, session, Some(platform_id)).await?);
    }

    if !entries.is_empty() {
        return Ok(FirmwareSearchResult {
            entries,
            platform_scoped: true,
        });
    }

    let all_entries = fetch_romm_firmware(&client, session, None).await?;
    Ok(FirmwareSearchResult {
        entries: all_entries,
        platform_scoped: false,
    })
}

async fn fetch_romm_platforms(
    client: &Client,
    session: &RommResourceSession,
) -> Result<Vec<RommPlatform>, String> {
    let url = format!("{}/api/platforms", session.base_url.trim_end_matches('/'));
    let response = client
        .get(url)
        .bearer_auth(&session.token)
        .send()
        .await
        .map_err(|error| format!("RomM platform lookup failed: {}", error))?;

    if !response.status().is_success() {
        return Err(format!(
            "RomM platform lookup failed with status {}",
            response.status()
        ));
    }

    let raw = response
        .text()
        .await
        .map_err(|error| format!("RomM platform response is unreadable: {}", error))?;
    let payload = serde_json::from_str::<RommListPayload<RommPlatform>>(&raw)
        .map_err(|error| format!("RomM platform response is invalid: {}", error))?;

    Ok(list_payload_to_vec(payload))
}

async fn fetch_romm_firmware(
    client: &Client,
    session: &RommResourceSession,
    platform_id: Option<&str>,
) -> Result<Vec<RommFirmware>, String> {
    let mut url = Url::parse(&format!(
        "{}/api/firmware",
        session.base_url.trim_end_matches('/')
    ))
    .map_err(|error| format!("Invalid RomM URL: {}", error))?;

    if let Some(platform_id) = platform_id {
        url.query_pairs_mut().append_pair("platform_id", platform_id);
    }

    let response = client
        .get(url)
        .bearer_auth(&session.token)
        .send()
        .await
        .map_err(|error| format!("RomM firmware lookup failed: {}", error))?;

    if !response.status().is_success() {
        return Err(format!(
            "RomM firmware lookup failed with status {}",
            response.status()
        ));
    }

    let raw = response
        .text()
        .await
        .map_err(|error| format!("RomM firmware response is unreadable: {}", error))?;
    let payload = serde_json::from_str::<RommListPayload<RommFirmware>>(&raw)
        .map_err(|error| format!("RomM firmware response is invalid: {}", error))?;

    Ok(list_payload_to_vec(payload))
}

fn list_payload_to_vec<T>(payload: RommListPayload<T>) -> Vec<T> {
    match payload {
        RommListPayload::Direct(entries) => entries,
        RommListPayload::Wrapped { items, results } => items.or(results).unwrap_or_default(),
    }
}

async fn download_firmware_file(
    app: &AppHandle,
    paths: &PortablePaths,
    client: &Client,
    session: &RommResourceSession,
    emulator_id: &str,
    resource_id: &str,
    resource_label: &str,
    firmware: &RommFirmware,
) -> Result<PathBuf, String> {
    let temp_dir = Path::new(&paths.data)
        .join("downloads")
        .join("resources")
        .join(emulator_id);
    fs::create_dir_all(&temp_dir)
        .map_err(|error| format!("Cannot create resource download directory: {}", error))?;

    let safe_file_name = sanitize_file_name(&firmware.file_name);
    let destination = temp_dir.join(&safe_file_name);
    let partial_destination = temp_dir.join(format!("{}.part", safe_file_name));

    if partial_destination.exists() {
        tokio::fs::remove_file(&partial_destination)
            .await
            .map_err(|error| format!("Cannot remove previous partial download: {}", error))?;
    }

    let url = firmware_content_url(session, firmware)?;
    emit_resource_progress(
        app,
        emulator_id,
        resource_id,
        resource_label,
        "download",
        &format!("Downloading {}", firmware.file_name),
        Some(0.0),
    )?;

    let response = client
        .get(url)
        .bearer_auth(&session.token)
        .send()
        .await
        .map_err(|error| format!("Resource download failed: {}", error))?;

    if !response.status().is_success() {
        return Err(format!(
            "Resource download failed with status {}",
            response.status()
        ));
    }

    let total_bytes = response.content_length().or(firmware.file_size_bytes);
    let mut file = tokio::fs::File::create(&partial_destination)
        .await
        .map_err(|error| format!("Cannot create resource file: {}", error))?;
    let mut downloaded_bytes = 0_u64;
    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|error| format!("Cannot read resource download: {}", error))?;
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("Cannot write resource file: {}", error))?;
        downloaded_bytes += chunk.len() as u64;

        let percent = total_bytes
            .filter(|total| *total > 0)
            .map(|total| ((downloaded_bytes as f64 / total as f64) * 100.0).clamp(0.0, 100.0));
        emit_resource_progress(
            app,
            emulator_id,
            resource_id,
            resource_label,
            "download",
            &format!("Downloading {}", firmware.file_name),
            percent,
        )?;
    }

    file.flush()
        .await
        .map_err(|error| format!("Cannot finish resource file: {}", error))?;

    if destination.exists() {
        tokio::fs::remove_file(&destination)
            .await
            .map_err(|error| format!("Cannot replace previous resource download: {}", error))?;
    }

    tokio::fs::rename(&partial_destination, &destination)
        .await
        .map_err(|error| format!("Cannot finalize resource download: {}", error))?;

    emit_resource_progress(
        app,
        emulator_id,
        resource_id,
        resource_label,
        "download",
        &format!("Downloaded {}", firmware.file_name),
        Some(100.0),
    )?;

    Ok(destination)
}

fn firmware_content_url(
    session: &RommResourceSession,
    firmware: &RommFirmware,
) -> Result<Url, String> {
    let firmware_id = value_to_string(&firmware.id)
        .ok_or_else(|| format!("Invalid RomM firmware id for {}", firmware.file_name))?;
    let mut url = Url::parse(&format!(
        "{}/api/firmware",
        session.base_url.trim_end_matches('/')
    ))
    .map_err(|error| format!("Invalid RomM URL: {}", error))?;

    url.path_segments_mut()
        .map_err(|_| "Invalid RomM firmware URL".to_string())?
        .push(&firmware_id)
        .push("content")
        .push(&firmware.file_name);

    Ok(url)
}

fn install_file_or_zip_flat(
    source: &Path,
    target_dir: &Path,
    accepts_file_name: fn(&str) -> bool,
    forced_output_name: Option<&str>,
) -> Result<Vec<PathBuf>, String> {
    if is_zip_file(source) {
        return extract_zip_flat(source, target_dir, accepts_file_name, forced_output_name);
    }

    let file_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Invalid resource file name.".to_string())?;
    if !accepts_file_name(file_name) {
        return Ok(Vec::new());
    }

    fs::create_dir_all(target_dir)
        .map_err(|error| format!("Cannot create resource target directory: {}", error))?;
    let output_name = forced_output_name.unwrap_or(file_name);
    let destination = target_dir.join(sanitize_file_name(output_name));
    fs::copy(source, &destination)
        .map_err(|error| format!("Cannot install resource file: {}", error))?;
    Ok(vec![destination])
}

fn extract_zip_flat(
    source: &Path,
    target_dir: &Path,
    accepts_file_name: fn(&str) -> bool,
    forced_output_name: Option<&str>,
) -> Result<Vec<PathBuf>, String> {
    let file = fs::File::open(source)
        .map_err(|error| format!("Cannot open resource archive: {}", error))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("Resource archive is not a valid zip: {}", error))?;
    let mut destinations = Vec::new();
    let mut seen = HashSet::new();

    fs::create_dir_all(target_dir)
        .map_err(|error| format!("Cannot create resource target directory: {}", error))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Cannot read resource archive entry: {}", error))?;

        if entry.is_dir() {
            continue;
        }

        let Some(enclosed) = entry.enclosed_name().map(|path| path.to_path_buf()) else {
            continue;
        };
        let Some(entry_name) = enclosed.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        if !accepts_file_name(entry_name) {
            continue;
        }

        let output_name = forced_output_name.unwrap_or(entry_name);
        let destination = target_dir.join(sanitize_file_name(output_name));
        if !seen.insert(destination.clone()) {
            continue;
        }

        let mut out_file = fs::File::create(&destination)
            .map_err(|error| format!("Cannot create extracted resource: {}", error))?;
        io::copy(&mut entry, &mut out_file)
            .map_err(|error| format!("Cannot extract resource file: {}", error))?;
        destinations.push(destination);
    }

    Ok(destinations)
}

fn installed_resource(
    resource_id: &str,
    resource_label: &str,
    firmware: &RommFirmware,
    destination: &Path,
) -> InstalledResource {
    InstalledResource {
        resource_id: resource_id.to_string(),
        resource_label: resource_label.to_string(),
        source_file_name: firmware.file_name.clone(),
        destination_path: destination.to_string_lossy().to_string(),
        verified_by_romm: firmware.is_verified.unwrap_or(false),
    }
}

fn emit_resource_progress(
    app: &AppHandle,
    emulator_id: &str,
    resource_id: &str,
    resource_label: &str,
    stage: &str,
    message: &str,
    percent: Option<f64>,
) -> Result<(), String> {
    app.emit(
        "emulator-resource-progress",
        ResourceInstallProgressPayload {
            emulator_id: emulator_id.to_string(),
            resource_id: resource_id.to_string(),
            resource_label: resource_label.to_string(),
            stage: stage.to_string(),
            message: message.to_string(),
            percent,
        },
    )
    .map_err(|error| format!("Cannot emit resource progress: {}", error))
}

fn platform_matches(platform: &RommPlatform, tokens: &[&str]) -> bool {
    let haystack = [
        platform.name.as_deref(),
        platform.slug.as_deref(),
        platform.fs_slug.as_deref(),
        platform.custom_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(normalize_text)
    .collect::<Vec<_>>()
    .join(" ");

    tokens.iter().any(|token| haystack.contains(&normalize_text(token)))
}

fn is_pcsx2_bios_candidate(firmware: &RommFirmware, platform_scoped: bool) -> bool {
    let name = firmware.file_name.to_ascii_lowercase();
    let looks_like_bios_name = name.contains("bios")
        || name.contains("scph")
        || name.contains("ps2")
        || name.contains("pcsx2")
        || name.contains("rom0")
        || name.contains("rom1")
        || name.contains("rom2")
        || name.contains("erom")
        || name.contains("nvm");

    if is_zip_file_name(&name) {
        return platform_scoped
            || looks_like_bios_name;
    }

    if platform_scoped && is_pcsx2_bios_file_name(&name) {
        return true;
    }

    is_pcsx2_bios_file_name(&name)
        && (looks_like_bios_name
            || firmware
                .file_path
                .as_deref()
                .map(|path| normalize_text(path).contains("playstation 2"))
                .unwrap_or(false))
}

fn is_eden_key_candidate(firmware: &RommFirmware) -> bool {
    let name = firmware.file_name.to_ascii_lowercase();
    is_eden_key_file_name(&name) || (is_zip_file_name(&name) && name.contains("key"))
}

fn is_eden_firmware_candidate(firmware: &RommFirmware, platform_scoped: bool) -> bool {
    let name = firmware.file_name.to_ascii_lowercase();
    if is_eden_key_file_name(&name) || name == "title.keys" {
        return false;
    }

    if platform_scoped {
        return is_zip_file_name(&name) || is_eden_firmware_file_name(&name);
    }

    (is_zip_file_name(&name) || is_eden_firmware_file_name(&name))
        && (name.contains("firmware")
            || firmware
                .file_path
                .as_deref()
                .map(|path| normalize_text(path).contains("switch"))
                .unwrap_or(false))
}

fn is_pcsx2_bios_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".rom0")
        || lower.ends_with(".rom1")
        || lower.ends_with(".rom2")
        || lower.ends_with(".erom")
        || lower.ends_with(".nvm")
    {
        return true;
    }

    matches!(
        extension(&lower).as_deref(),
        Some("bin") | Some("rom") | Some("mec")
    )
}

fn is_eden_key_file_name(file_name: &str) -> bool {
    file_name.eq_ignore_ascii_case("prod.keys")
}

fn is_eden_firmware_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    matches!(
        extension(&lower).as_deref(),
        Some("nca") | Some("cnmt") | Some("tik") | Some("cert")
    )
}

fn collect_matching_files(dir: &Path, accepts_file_name: fn(&str) -> bool) -> Vec<PathBuf> {
    let mut results = Vec::new();
    collect_matching_files_inner(dir, accepts_file_name, &mut results);
    results
}

fn collect_matching_files_inner(
    dir: &Path,
    accepts_file_name: fn(&str) -> bool,
    results: &mut Vec<PathBuf>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_matching_files_inner(&path, accepts_file_name, results);
            continue;
        }

        if path
            .file_name()
            .and_then(|value| value.to_str())
            .map(accepts_file_name)
            .unwrap_or(false)
        {
            results.push(path);
        }
    }
}

fn pcsx2_working_dir(paths: &PortablePaths) -> PathBuf {
    resolve_emulator_executable(paths, "pcsx2")
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.to_path_buf()))
        .unwrap_or_else(|| Path::new(&paths.emu).join("PCSX2"))
}

fn ensure_pcsx2_bios_configuration(paths: &PortablePaths) -> Result<(), String> {
    let working_dir = pcsx2_working_dir(paths);
    let bios_dir = working_dir.join("bios");
    let mut bios_files = collect_matching_files(&bios_dir, is_pcsx2_bios_file_name)
        .into_iter()
        .filter(|path| is_non_empty_file(path))
        .collect::<Vec<_>>();

    if bios_files.is_empty() {
        return Ok(());
    }

    bios_files.sort();
    let config_dir = working_dir.join("inis");
    let config_path = config_dir.join("PCSX2.ini");
    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Cannot create PCSX2 ini directory: {}", error))?;

    let mut content = fs::read_to_string(&config_path).unwrap_or_else(|_| String::new());
    let selected_bios = select_pcsx2_bios_file_name(&content, &bios_files)
        .ok_or_else(|| "No usable PCSX2 BIOS file name found.".to_string())?;

    set_ini_value(&mut content, "Folders", "Bios", "bios");
    set_ini_value(&mut content, "Filenames", "BIOS", &selected_bios);

    fs::write(&config_path, content)
        .map_err(|error| format!("Cannot update PCSX2 BIOS configuration: {}", error))
}

fn select_pcsx2_bios_file_name(content: &str, bios_files: &[PathBuf]) -> Option<String> {
    let configured = get_ini_value(content, "Filenames", "BIOS");
    if let Some(configured) = configured.as_deref().filter(|value| !value.trim().is_empty()) {
        if bios_files.iter().any(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .map(|name| name.eq_ignore_ascii_case(configured))
                .unwrap_or(false)
        }) {
            return Some(configured.to_string());
        }
    }

    bios_files
        .iter()
        .filter_map(|path| path.file_name().and_then(|value| value.to_str()))
        .find(|name| name.to_ascii_lowercase().contains("scph"))
        .or_else(|| {
            bios_files
                .iter()
                .filter_map(|path| path.file_name().and_then(|value| value.to_str()))
                .next()
        })
        .map(str::to_string)
}

fn get_ini_value(content: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    let section_header = format!("[{}]", section);

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed.eq_ignore_ascii_case(&section_header);
            continue;
        }

        if !in_section {
            continue;
        }

        let Some((candidate, value)) = trimmed.split_once('=') else {
            continue;
        };
        if candidate.trim().eq_ignore_ascii_case(key) {
            return Some(value.trim().to_string());
        }
    }

    None
}

fn set_ini_value(content: &mut String, section: &str, key: &str, value: &str) {
    let had_final_newline = content.ends_with('\n') || content.ends_with("\r\n");
    let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
    let section_header = format!("[{}]", section);

    let section_index = lines
        .iter()
        .position(|line| line.trim().eq_ignore_ascii_case(&section_header))
        .unwrap_or_else(|| {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(section_header);
            lines.len() - 1
        });

    let next_section_index = lines
        .iter()
        .enumerate()
        .skip(section_index + 1)
        .find(|(_, line)| {
            let trimmed = line.trim();
            trimmed.starts_with('[') && trimmed.ends_with(']')
        })
        .map(|(index, _)| index)
        .unwrap_or(lines.len());

    if let Some(line) = lines
        .iter_mut()
        .take(next_section_index)
        .skip(section_index + 1)
        .find(|line| {
            line.split_once('=')
                .is_some_and(|(candidate, _)| candidate.trim().eq_ignore_ascii_case(key))
        })
    {
        *line = format!("{} = {}", key, value);
    } else {
        lines.insert(next_section_index, format!("{} = {}", key, value));
    }

    *content = lines.join("\n");
    if had_final_newline || !content.is_empty() {
        content.push('\n');
    }
}

fn eden_base_user_dir(paths: &PortablePaths) -> PathBuf {
    resolve_emulator_executable(paths, "eden")
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("user")))
        .unwrap_or_else(|| Path::new(&paths.emu).join("Eden").join("user"))
}

fn eden_user_roots(paths: &PortablePaths) -> Vec<PathBuf> {
    let mut roots = vec![eden_base_user_dir(paths)];
    let profiles_root = Path::new(&paths.data).join("EdenProfiles");

    if let Ok(entries) = fs::read_dir(profiles_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                roots.push(path);
            }
        }
    }

    roots
}

fn eden_registered_dir(user_root: &Path) -> PathBuf {
    user_root
        .join("nand")
        .join("system")
        .join("Contents")
        .join("registered")
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn normalize_text(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

fn is_non_empty_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

fn is_zip_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(is_zip_file_name)
        .unwrap_or(false)
}

fn is_zip_file_name(file_name: &str) -> bool {
    file_name.to_ascii_lowercase().ends_with(".zip")
}

fn extension(file_name: &str) -> Option<String> {
    Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
}
