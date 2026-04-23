use crate::emulator_configurator::configure_emulator;
use crate::portable_paths::PortablePaths;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fmt::Write as _;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::async_runtime::block_on;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const ROMM_ROM_INDEX_FILE: &str = "romm-rom-index.json";
const DOLPHIN_SLOT_NAME: &str = "EmuManager Dolphin";
const MELONDS_SLOT_NAME: &str = "EmuManager melonDS";
const AZAHAR_SLOT_NAME: &str = "EmuManager Azahar";
const EDEN_SLOT_NAME: &str = "EmuManager Eden";
const PCSX2_SLOT_NAME: &str = "EmuManager PCSX2";
const MAX_REMOTE_SAVES: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RommLaunchSession {
    pub base_url: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RommRomMapping {
    rom_path: String,
    romm_id: String,
    platform_name: Option<String>,
    file_name: Option<String>,
    last_synced_local_save_at_ms: Option<u64>,
    last_remote_save_at: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RommRomIndex {
    entries: Vec<RommRomMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSyncStatus {
    pub rom_path: String,
    pub romm_id: Option<String>,
    pub emulator_id: String,
    pub has_local_save: bool,
    pub local_save_updated_at_ms: Option<u64>,
    pub last_known_remote_save_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RommSaveEntry {
    id: RommId,
    file_name: Option<String>,
    slot: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RommGameEntry {
    id: RommId,
    name: Option<String>,
    file_name: Option<String>,
    filename: Option<String>,
    fs_name: Option<String>,
    files: Option<Vec<RommGameFileEntry>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RommGameFileEntry {
    file_name: Option<String>,
    filename: Option<String>,
    fs_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RommGamesResponse {
    Direct(Vec<RommGameEntry>),
    Wrapped {
        items: Option<Vec<RommGameEntry>>,
        results: Option<Vec<RommGameEntry>>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RommId {
    String(String),
    Number(i64),
}

impl RommId {
    fn as_string(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Number(value) => value.to_string(),
        }
    }
}

pub fn register_rom_mapping(
    paths: &PortablePaths,
    rom_path: &str,
    romm_id: &str,
    platform_name: Option<&str>,
    file_name: Option<&str>,
) -> Result<(), String> {
    log_sync(
        paths,
        &format!(
            "register_rom_mapping rom_path={} romm_id={} platform={:?} file_name={:?}",
            rom_path, romm_id, platform_name, file_name
        ),
    );

    let mut index = load_rom_index(paths)?;

    if let Some(existing) = index.entries.iter_mut().find(|entry| entry.rom_path == rom_path) {
        existing.romm_id = romm_id.to_string();
        existing.platform_name = platform_name.map(str::to_string);
        existing.file_name = file_name.map(str::to_string);
    } else {
        index.entries.push(RommRomMapping {
            rom_path: rom_path.to_string(),
            romm_id: romm_id.to_string(),
            platform_name: platform_name.map(str::to_string),
            file_name: file_name.map(str::to_string),
            last_synced_local_save_at_ms: None,
            last_remote_save_at: None,
        });
    }

    save_rom_index(paths, &index)
}

pub fn get_save_sync_statuses(
    paths: &PortablePaths,
    rom_paths: &[String],
) -> Result<Vec<SaveSyncStatus>, String> {
    let index = load_rom_index(paths)?;

    Ok(rom_paths
        .iter()
        .map(|rom_path| {
            let mapping = index.entries.iter().find(|entry| entry.rom_path == *rom_path);
            let emulator_id = crate::platform_router::resolve_emulator_id_for_rom_path(paths, rom_path)
                .unwrap_or_else(|_| "unknown".to_string());
            let local_save_updated_at_ms = match emulator_id.as_str() {
                "dolphin" => latest_dolphin_save_timestamp_ms(paths, Path::new(rom_path)).ok().flatten(),
                "melonds" => latest_melonds_save_timestamp_ms(Path::new(rom_path)).ok().flatten(),
                "azahar" => latest_azahar_save_timestamp_ms(paths, Path::new(rom_path)).ok().flatten(),
                "eden" => latest_eden_save_timestamp_ms(paths, Path::new(rom_path)).ok().flatten(),
                "pcsx2" => latest_pcsx2_save_timestamp_ms(paths, Path::new(rom_path)).ok().flatten(),
                _ => None,
            };

            SaveSyncStatus {
                rom_path: rom_path.clone(),
                romm_id: mapping.map(|entry| entry.romm_id.clone()),
                emulator_id,
                has_local_save: local_save_updated_at_ms.is_some(),
                local_save_updated_at_ms,
                last_known_remote_save_at: mapping.and_then(|entry| entry.last_remote_save_at.clone()),
            }
        })
        .collect())
}

pub fn launch_dolphin(
    paths: &PortablePaths,
    executable_path: &Path,
    rom_path: &Path,
    session: Option<&RommLaunchSession>,
) -> Result<crate::game_launcher::GameLaunchResult, String> {
    log_sync(
        paths,
        &format!(
            "launch_dolphin rom_path={} mode={}",
            rom_path.to_string_lossy(),
            if session.is_some() { "online" } else { "offline" }
        ),
    );

    let mut mapping = load_rom_index(paths)?
        .entries
        .into_iter()
        .find(|entry| Path::new(&entry.rom_path) == rom_path);

    if mapping.is_none() {
        if let Some(active_session) = session {
            log_sync(
                paths,
                &format!(
                    "no direct romm mapping found for {}, trying filename lookup",
                    rom_path.to_string_lossy()
                ),
            );

            mapping = resolve_mapping_from_remote(paths, active_session, rom_path)?;
        }
    }

    let profile_root = dolphin_profile_root(paths, rom_path)?;
    let profile_user_dir = profile_root.join("User");
    let has_existing_local_save = latest_dolphin_save_timestamp_ms(paths, rom_path)?.is_some();

    if mapping.is_none() && !has_existing_local_save {
        log_sync(
            paths,
            &format!(
                "no mapping and no local dolphin save for {}, fallback to plain dolphin launch",
                rom_path.to_string_lossy()
            ),
        );
        return launch_plain_dolphin(executable_path, rom_path);
    }

    configure_emulator(paths, "dolphin")?;
    seed_profile_from_base_user(paths, &profile_user_dir)?;
    cleanup_dolphin_transient_files(paths, &profile_user_dir)?;

    if let (Some(active_session), Some(current_mapping)) = (session, mapping.as_ref()) {
        maybe_restore_latest_remote_save(paths, active_session, current_mapping, rom_path, &profile_root)?;
        cleanup_dolphin_transient_files(paths, &profile_user_dir)?;
    } else {
        log_sync(paths, "launching Dolphin with local profile only");
    }

    let working_directory = executable_path
        .parent()
        .ok_or_else(|| "Impossible de déterminer le dossier de travail Dolphin".to_string())?
        .to_path_buf();

    let status = Command::new(executable_path)
        .current_dir(&working_directory)
        .arg("-u")
        .arg(&profile_user_dir)
        .arg("--exec")
        .arg(rom_path)
        .status()
        .map_err(|error| format!("Lancement du jeu impossible: {}", error))?;

    log_sync(
        paths,
        &format!("dolphin exited success={} code={:?}", status.success(), status.code()),
    );

    if !status.success() {
        return Err(format!("Dolphin s'est fermé avec le code {:?}", status.code()));
    }

    if let (Some(active_session), Some(current_mapping)) = (session, mapping.as_ref()) {
        upload_save_bundle(paths, active_session, current_mapping, rom_path, &profile_root)?;
    } else {
        log_sync(paths, "skip remote upload because no active RomM session or no mapping");
    }

    Ok(crate::game_launcher::GameLaunchResult {
        emulator_id: "dolphin".to_string(),
        executable_path: executable_path.to_string_lossy().to_string(),
        rom_path: rom_path.to_string_lossy().to_string(),
        launched: true,
    })
}

fn launch_plain_dolphin(
    executable_path: &Path,
    rom_path: &Path,
) -> Result<crate::game_launcher::GameLaunchResult, String> {
    let working_directory = executable_path
        .parent()
        .ok_or_else(|| "Impossible de dÃ©terminer le dossier de travail Dolphin".to_string())?
        .to_path_buf();

    let status = Command::new(executable_path)
        .current_dir(&working_directory)
        .arg("--exec")
        .arg(rom_path)
        .status()
        .map_err(|error| format!("Lancement du jeu impossible: {}", error))?;

    if !status.success() {
        return Err(format!("Dolphin s'est fermÃ© avec le code {:?}", status.code()));
    }

    Ok(crate::game_launcher::GameLaunchResult {
        emulator_id: "dolphin".to_string(),
        executable_path: executable_path.to_string_lossy().to_string(),
        rom_path: rom_path.to_string_lossy().to_string(),
        launched: true,
    })
}

pub fn launch_melonds(
    paths: &PortablePaths,
    executable_path: &Path,
    rom_path: &Path,
    session: Option<&RommLaunchSession>,
) -> Result<crate::game_launcher::GameLaunchResult, String> {
    log_sync(
        paths,
        &format!(
            "launch_melonds rom_path={} mode={}",
            rom_path.to_string_lossy(),
            if session.is_some() { "online" } else { "offline" }
        ),
    );

    let mut mapping = load_rom_index(paths)?
        .entries
        .into_iter()
        .find(|entry| Path::new(&entry.rom_path) == rom_path);

    if mapping.is_none() {
        if let Some(active_session) = session {
            log_sync(
                paths,
                &format!(
                    "no direct romm mapping found for {}, trying filename lookup",
                    rom_path.to_string_lossy()
                ),
            );

            mapping = resolve_mapping_from_remote(paths, active_session, rom_path)?;
        }
    }

    let has_existing_local_save = latest_melonds_save_timestamp_ms(rom_path)?.is_some();

    if let (Some(active_session), Some(current_mapping)) = (session, mapping.as_ref()) {
        maybe_restore_latest_melonds_save(paths, active_session, current_mapping, rom_path)?;
    } else if has_existing_local_save {
        log_sync(paths, "launching melonDS with local save files only");
    } else {
        log_sync(paths, "launching melonDS without mapped cloud save");
    }

    let working_directory = executable_path
        .parent()
        .ok_or_else(|| "Impossible de dÃ©terminer le dossier de travail melonDS".to_string())?
        .to_path_buf();

    let status = Command::new(executable_path)
        .current_dir(&working_directory)
        .arg(rom_path)
        .status()
        .map_err(|error| format!("Lancement du jeu impossible: {}", error))?;

    log_sync(
        paths,
        &format!("melonds exited success={} code={:?}", status.success(), status.code()),
    );

    if !status.success() {
        return Err(format!("melonDS s'est fermÃ© avec le code {:?}", status.code()));
    }

    if let (Some(active_session), Some(current_mapping)) = (session, mapping.as_ref()) {
        upload_melonds_save_bundle(paths, active_session, current_mapping, rom_path)?;
    } else {
        log_sync(paths, "skip melonDS remote upload because no active RomM session or no mapping");
    }

    Ok(crate::game_launcher::GameLaunchResult {
        emulator_id: "melonds".to_string(),
        executable_path: executable_path.to_string_lossy().to_string(),
        rom_path: rom_path.to_string_lossy().to_string(),
        launched: true,
    })
}

pub fn launch_azahar(
    paths: &PortablePaths,
    executable_path: &Path,
    rom_path: &Path,
    session: Option<&RommLaunchSession>,
) -> Result<crate::game_launcher::GameLaunchResult, String> {
    log_sync(
        paths,
        &format!(
            "launch_azahar rom_path={} mode={}",
            rom_path.to_string_lossy(),
            if session.is_some() { "online" } else { "offline" }
        ),
    );

    let mut mapping = load_rom_index(paths)?
        .entries
        .into_iter()
        .find(|entry| Path::new(&entry.rom_path) == rom_path);

    if mapping.is_none() {
        if let Some(active_session) = session {
            log_sync(
                paths,
                &format!(
                    "no direct romm mapping found for {}, trying filename lookup",
                    rom_path.to_string_lossy()
                ),
            );

            mapping = resolve_mapping_from_remote(paths, active_session, rom_path)?;
        }
    }

    let profile_root = azahar_profile_root(paths, rom_path)?;
    let has_existing_local_save = latest_azahar_save_timestamp_ms(paths, rom_path)?.is_some();

    if mapping.is_none() && !has_existing_local_save {
        log_sync(
            paths,
            &format!(
                "no mapping and no local azahar save for {}, fallback to plain azahar launch",
                rom_path.to_string_lossy()
            ),
        );
        let working_directory = executable_path
            .parent()
            .ok_or_else(|| "Impossible de dÃ©terminer le dossier de travail Azahar".to_string())?
            .to_path_buf();

        let status = Command::new(executable_path)
            .current_dir(&working_directory)
            .arg(rom_path)
            .status()
            .map_err(|error| format!("Lancement du jeu impossible: {}", error))?;

        if !status.success() {
            return Err(format!("Azahar s'est fermÃ© avec le code {:?}", status.code()));
        }

        return Ok(crate::game_launcher::GameLaunchResult {
            emulator_id: "azahar".to_string(),
            executable_path: executable_path.to_string_lossy().to_string(),
            rom_path: rom_path.to_string_lossy().to_string(),
            launched: true,
        });
    }

    seed_azahar_profile_from_base_user(paths, &profile_root)?;

    if let (Some(active_session), Some(current_mapping)) = (session, mapping.as_ref()) {
        maybe_restore_latest_azahar_save(paths, active_session, current_mapping, rom_path, &profile_root)?;
    } else {
        log_sync(paths, "launching Azahar with local profile only");
    }

    let working_directory = executable_path
        .parent()
        .ok_or_else(|| "Impossible de dÃ©terminer le dossier de travail Azahar".to_string())?
        .to_path_buf();
    let portable_user_dir = working_directory.join("user");

    materialize_azahar_portable_user(paths, &profile_root, &portable_user_dir)?;

    let status = Command::new(executable_path)
        .current_dir(&working_directory)
        .arg(rom_path)
        .status()
        .map_err(|error| format!("Lancement du jeu impossible: {}", error))?;

    log_sync(
        paths,
        &format!("azahar exited success={} code={:?}", status.success(), status.code()),
    );

    sync_azahar_portable_user_back(paths, &portable_user_dir, &profile_root)?;

    if !status.success() {
        return Err(format!("Azahar s'est fermÃ© avec le code {:?}", status.code()));
    }

    if let (Some(active_session), Some(current_mapping)) = (session, mapping.as_ref()) {
        upload_azahar_save_bundle(paths, active_session, current_mapping, rom_path, &profile_root)?;
    } else {
        log_sync(paths, "skip Azahar remote upload because no active RomM session or no mapping");
    }

    Ok(crate::game_launcher::GameLaunchResult {
        emulator_id: "azahar".to_string(),
        executable_path: executable_path.to_string_lossy().to_string(),
        rom_path: rom_path.to_string_lossy().to_string(),
        launched: true,
    })
}

pub fn launch_eden(
    paths: &PortablePaths,
    executable_path: &Path,
    rom_path: &Path,
    session: Option<&RommLaunchSession>,
) -> Result<crate::game_launcher::GameLaunchResult, String> {
    log_sync(
        paths,
        &format!(
            "launch_eden rom_path={} mode={}",
            rom_path.to_string_lossy(),
            if session.is_some() { "online" } else { "offline" }
        ),
    );

    let mut mapping = load_rom_index(paths)?
        .entries
        .into_iter()
        .find(|entry| Path::new(&entry.rom_path) == rom_path);

    if mapping.is_none() {
        if let Some(active_session) = session {
            log_sync(
                paths,
                &format!(
                    "no direct romm mapping found for {}, trying filename lookup",
                    rom_path.to_string_lossy()
                ),
            );

            mapping = resolve_mapping_from_remote(paths, active_session, rom_path)?;
        }
    }

    let profile_root = eden_profile_root(paths, rom_path)?;
    let has_existing_local_save = latest_eden_save_timestamp_ms(paths, rom_path)?.is_some();

    if mapping.is_none() && !has_existing_local_save {
        log_sync(
            paths,
            &format!(
                "no mapping and no local eden save for {}, fallback to plain eden launch",
                rom_path.to_string_lossy()
            ),
        );

        let working_directory = executable_path
            .parent()
            .ok_or_else(|| "Impossible de dÃ©terminer le dossier de travail Eden".to_string())?
            .to_path_buf();

        let status = Command::new(executable_path)
            .current_dir(&working_directory)
            .arg(rom_path)
            .status()
            .map_err(|error| format!("Lancement du jeu impossible: {}", error))?;

        if !status.success() {
            return Err(format!("Eden s'est fermÃ© avec le code {:?}", status.code()));
        }

        return Ok(crate::game_launcher::GameLaunchResult {
            emulator_id: "eden".to_string(),
            executable_path: executable_path.to_string_lossy().to_string(),
            rom_path: rom_path.to_string_lossy().to_string(),
            launched: true,
        });
    }

    seed_eden_profile_from_base_user(paths, &profile_root)?;

    if let (Some(active_session), Some(current_mapping)) = (session, mapping.as_ref()) {
        maybe_restore_latest_eden_save(paths, active_session, current_mapping, rom_path, &profile_root)?;
    } else {
        log_sync(paths, "launching Eden with local profile only");
    }

    let working_directory = executable_path
        .parent()
        .ok_or_else(|| "Impossible de dÃ©terminer le dossier de travail Eden".to_string())?
        .to_path_buf();
    let portable_user_dir = working_directory.join("user");

    materialize_eden_portable_user(paths, &profile_root, &portable_user_dir)?;

    let status = Command::new(executable_path)
        .current_dir(&working_directory)
        .arg(rom_path)
        .status()
        .map_err(|error| format!("Lancement du jeu impossible: {}", error))?;

    log_sync(
        paths,
        &format!("eden exited success={} code={:?}", status.success(), status.code()),
    );

    sync_eden_portable_user_back(paths, &portable_user_dir, &profile_root)?;

    if !status.success() {
        return Err(format!("Eden s'est fermÃ© avec le code {:?}", status.code()));
    }

    if let (Some(active_session), Some(current_mapping)) = (session, mapping.as_ref()) {
        upload_eden_save_bundle(paths, active_session, current_mapping, rom_path, &profile_root)?;
    } else {
        log_sync(paths, "skip Eden remote upload because no active RomM session or no mapping");
    }

    Ok(crate::game_launcher::GameLaunchResult {
        emulator_id: "eden".to_string(),
        executable_path: executable_path.to_string_lossy().to_string(),
        rom_path: rom_path.to_string_lossy().to_string(),
        launched: true,
    })
}

pub fn launch_pcsx2(
    paths: &PortablePaths,
    executable_path: &Path,
    rom_path: &Path,
    session: Option<&RommLaunchSession>,
) -> Result<crate::game_launcher::GameLaunchResult, String> {
    log_sync(
        paths,
        &format!(
            "launch_pcsx2 rom_path={} mode={}",
            rom_path.to_string_lossy(),
            if session.is_some() { "online" } else { "offline" }
        ),
    );

    let mut mapping = load_rom_index(paths)?
        .entries
        .into_iter()
        .find(|entry| Path::new(&entry.rom_path) == rom_path);

    if mapping.is_none() {
        if let Some(active_session) = session {
            log_sync(
                paths,
                &format!(
                    "no direct romm mapping found for {}, trying filename lookup",
                    rom_path.to_string_lossy()
                ),
            );

            mapping = resolve_mapping_from_remote(paths, active_session, rom_path)?;
        }
    }

    let profile_root = pcsx2_profile_root(paths, rom_path)?;
    let has_existing_local_save = latest_pcsx2_save_timestamp_ms(paths, rom_path)?.is_some();

    if mapping.is_none() && !has_existing_local_save {
        log_sync(
            paths,
            &format!(
                "no mapping and no local pcsx2 save for {}, fallback to plain pcsx2 launch",
                rom_path.to_string_lossy()
            ),
        );

        let working_directory = executable_path
            .parent()
            .ok_or_else(|| "Impossible de dÃ©terminer le dossier de travail PCSX2".to_string())?
            .to_path_buf();

        let status = Command::new(executable_path)
            .current_dir(&working_directory)
            .arg("-batch")
            .arg("--")
            .arg(rom_path)
            .status()
            .map_err(|error| format!("Lancement du jeu impossible: {}", error))?;

        if !status.success() {
            return Err(format!("PCSX2 s'est fermÃ© avec le code {:?}", status.code()));
        }

        return Ok(crate::game_launcher::GameLaunchResult {
            emulator_id: "pcsx2".to_string(),
            executable_path: executable_path.to_string_lossy().to_string(),
            rom_path: rom_path.to_string_lossy().to_string(),
            launched: true,
        });
    }

    seed_pcsx2_profile_from_base(paths, &profile_root)?;

    if let (Some(active_session), Some(current_mapping)) = (session, mapping.as_ref()) {
        maybe_restore_latest_pcsx2_save(paths, active_session, current_mapping, rom_path, &profile_root)?;
    } else {
        log_sync(paths, "launching PCSX2 with local profile only");
    }

    let working_directory = executable_path
        .parent()
        .ok_or_else(|| "Impossible de dÃ©terminer le dossier de travail PCSX2".to_string())?
        .to_path_buf();
    let portable_ini = working_directory.join("portable.ini");

    ensure_pcsx2_portable_mode(&portable_ini)?;
    materialize_pcsx2_portable_profile(paths, &profile_root, &working_directory)?;

    let status = Command::new(executable_path)
        .current_dir(&working_directory)
        .arg("-batch")
        .arg("--")
        .arg(rom_path)
        .status()
        .map_err(|error| format!("Lancement du jeu impossible: {}", error))?;

    log_sync(
        paths,
        &format!("pcsx2 exited success={} code={:?}", status.success(), status.code()),
    );

    sync_pcsx2_portable_profile_back(paths, &working_directory, &profile_root)?;

    if !status.success() {
        return Err(format!("PCSX2 s'est fermÃ© avec le code {:?}", status.code()));
    }

    if let (Some(active_session), Some(current_mapping)) = (session, mapping.as_ref()) {
        upload_pcsx2_save_bundle(paths, active_session, current_mapping, rom_path, &profile_root)?;
    } else {
        log_sync(paths, "skip PCSX2 remote upload because no active RomM session or no mapping");
    }

    Ok(crate::game_launcher::GameLaunchResult {
        emulator_id: "pcsx2".to_string(),
        executable_path: executable_path.to_string_lossy().to_string(),
        rom_path: rom_path.to_string_lossy().to_string(),
        launched: true,
    })
}

fn maybe_restore_latest_remote_save(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    mapping: &RommRomMapping,
    rom_path: &Path,
    profile_root: &Path,
) -> Result<(), String> {
    log_sync(paths, &format!("restore_latest_save_bundle romm_id={}", mapping.romm_id));

    let local_timestamp = latest_dolphin_save_timestamp_ms(paths, rom_path)?;

    if let (Some(current_local), Some(last_synced_local)) =
        (local_timestamp, mapping.last_synced_local_save_at_ms)
    {
        if current_local > last_synced_local {
            log_sync(
                paths,
                &format!(
                    "skipping remote restore because local save is newer than last synced local copy current={} last_synced={}",
                    current_local, last_synced_local
                ),
            );
            return Ok(());
        }
    }

    let latest_save = fetch_emumanager_saves(
        paths,
        session,
        &mapping.romm_id,
        "dolphin",
        DOLPHIN_SLOT_NAME,
    )?
        .into_iter()
        .max_by(|left, right| left.updated_at.cmp(&right.updated_at));

    let Some(save) = latest_save else {
        log_sync(paths, "no remote save bundle found");
        return Ok(());
    };

    if mapping.last_remote_save_at.as_ref() == save.updated_at.as_ref() && local_timestamp.is_some() {
        log_sync(paths, "remote save timestamp matches last known sync, keeping local profile");
        return Ok(());
    }

    log_sync(
        paths,
        &format!(
            "selected remote save save_id={} file_name={:?} slot={:?} updated_at={:?}",
            save.id.as_string(),
            save.file_name,
            save.slot,
            save.updated_at
        ),
    );

    let archive_name = save
        .file_name
        .clone()
        .unwrap_or_else(|| "dolphin-sync.zip".to_string());

    let download_url = format!(
        "{}/api/saves/{}/content",
        session.base_url.trim_end_matches('/'),
        save.id.as_string()
    );

    let archive_path = profile_root.join(&archive_name);
    download_file(paths, session, &download_url, &archive_path)?;

    let profile_user_dir = profile_root.join("User");
    if profile_user_dir.exists() {
        fs::remove_dir_all(&profile_user_dir)
            .map_err(|error| format!("Impossible de réinitialiser le profil Dolphin: {}", error))?;
    }

    extract_zip_archive(&archive_path, profile_root)?;
    let _ = fs::remove_file(&archive_path);

    let latest_local_after_restore = latest_dolphin_save_timestamp_ms(paths, rom_path)?;
    update_mapping_sync_metadata(
        paths,
        &mapping.rom_path,
        latest_local_after_restore,
        save.updated_at.clone(),
    )?;

    log_sync(
        paths,
        &format!("remote save extracted into {}", profile_root.to_string_lossy()),
    );

    Ok(())
}

fn upload_save_bundle(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    mapping: &RommRomMapping,
    rom_path: &Path,
    profile_root: &Path,
) -> Result<(), String> {
    let user_dir = profile_root.join("User");
    if !user_dir.exists() {
        log_sync(paths, "upload skipped because profile User directory does not exist");
        return Ok(());
    }

    let archive_path = profile_root.join("emumanager-dolphin-sync.zip");
    create_zip_archive(&user_dir, &archive_path)?;

    let mut url = Url::parse(&format!("{}/api/saves", session.base_url.trim_end_matches('/')))
        .map_err(|error| format!("URL RomM invalide: {}", error))?;
    url.query_pairs_mut()
        .append_pair("rom_id", &mapping.romm_id)
        .append_pair("emulator", "dolphin")
        .append_pair("slot", DOLPHIN_SLOT_NAME)
        .append_pair("overwrite", "true");

    let bytes = fs::read(&archive_path)
        .map_err(|error| format!("Impossible de lire l'archive de save Dolphin: {}", error))?;

    let file_name = format!("emumanager-dolphin-{}.zip", sanitize_file_stem(rom_path));
    log_sync(
        paths,
        &format!(
            "uploading save bundle romm_id={} file_name={} archive={} bytes={}",
            mapping.romm_id,
            file_name,
            archive_path.to_string_lossy(),
            bytes.len()
        ),
    );

    let boundary = format!("emumanager-{:016x}", compute_hash(&file_name));
    let body = build_multipart_body(&boundary, "saveFile", &file_name, "application/zip", &bytes);

    let response = block_on(async {
        let client = build_http_client()?;
        client
            .post(url)
            .bearer_auth(&session.token)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(body)
            .send()
            .await
            .map_err(|error| format!("Upload RomM impossible: {}", error))
    })?;

    let status = response.status();
    let response_body = block_on(async {
        response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".to_string())
    });

    log_sync(
        paths,
        &format!("upload response status={} body={}", status, response_body),
    );

    if !status.is_success() {
        return Err(format!("Upload RomM échoué avec le statut {}: {}", status, response_body));
    }

    let uploaded_save = serde_json::from_str::<RommSaveEntry>(&response_body).ok();
    let latest_local_timestamp = latest_dolphin_save_timestamp_ms(paths, rom_path)?;

    update_mapping_sync_metadata(
        paths,
        &mapping.rom_path,
        latest_local_timestamp,
        uploaded_save.and_then(|save| save.updated_at),
    )?;

    cleanup_old_remote_saves(paths, session, &mapping.romm_id, "dolphin", DOLPHIN_SLOT_NAME)?;
    Ok(())
}

fn maybe_restore_latest_melonds_save(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    mapping: &RommRomMapping,
    rom_path: &Path,
) -> Result<(), String> {
    log_sync(paths, &format!("restore_latest_melonds_save romm_id={}", mapping.romm_id));

    let local_timestamp = latest_melonds_save_timestamp_ms(rom_path)?;

    if let (Some(current_local), Some(last_synced_local)) =
        (local_timestamp, mapping.last_synced_local_save_at_ms)
    {
        if current_local > last_synced_local {
            log_sync(
                paths,
                &format!(
                    "skipping melonDS remote restore because local save is newer than last synced local copy current={} last_synced={}",
                    current_local, last_synced_local
                ),
            );
            return Ok(());
        }
    }

    let latest_save = fetch_emumanager_saves(
        paths,
        session,
        &mapping.romm_id,
        "melonds",
        MELONDS_SLOT_NAME,
    )?
    .into_iter()
    .max_by(|left, right| left.updated_at.cmp(&right.updated_at));

    let Some(save) = latest_save else {
        log_sync(paths, "no remote melonDS save bundle found");
        return Ok(());
    };

    if mapping.last_remote_save_at.as_ref() == save.updated_at.as_ref() && local_timestamp.is_some() {
        log_sync(paths, "melonDS remote save timestamp matches last known sync, keeping local files");
        return Ok(());
    }

    log_sync(
        paths,
        &format!(
            "selected remote melonDS save save_id={} file_name={:?} slot={:?} updated_at={:?}",
            save.id.as_string(),
            save.file_name,
            save.slot,
            save.updated_at
        ),
    );

    let archive_name = save
        .file_name
        .clone()
        .unwrap_or_else(|| "melonds-sync.zip".to_string());
    let download_url = format!(
        "{}/api/saves/{}/content",
        session.base_url.trim_end_matches('/'),
        save.id.as_string()
    );

    let temp_dir = melonds_cache_root(paths, rom_path)?;
    fs::create_dir_all(&temp_dir)
        .map_err(|error| format!("Impossible de prÃ©parer le cache melonDS: {}", error))?;

    let archive_path = temp_dir.join(&archive_name);
    download_file(paths, session, &download_url, &archive_path)?;
    remove_melonds_local_save_files(rom_path)?;
    extract_zip_archive(&archive_path, rom_path.parent().unwrap_or_else(|| Path::new(".")))?;
    let _ = fs::remove_file(&archive_path);

    let latest_local_after_restore = latest_melonds_save_timestamp_ms(rom_path)?;
    update_mapping_sync_metadata(
        paths,
        &mapping.rom_path,
        latest_local_after_restore,
        save.updated_at.clone(),
    )?;

    log_sync(
        paths,
        &format!("melonDS remote save extracted next to {}", rom_path.to_string_lossy()),
    );

    Ok(())
}

fn upload_melonds_save_bundle(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    mapping: &RommRomMapping,
    rom_path: &Path,
) -> Result<(), String> {
    let save_files = collect_melonds_save_files(rom_path)?;
    if save_files.is_empty() {
        log_sync(paths, "melonDS upload skipped because no local save files were found");
        return Ok(());
    }

    let cache_root = melonds_cache_root(paths, rom_path)?;
    fs::create_dir_all(&cache_root)
        .map_err(|error| format!("Impossible de prÃ©parer le cache melonDS: {}", error))?;

    let archive_path = cache_root.join("emumanager-melonds-sync.zip");
    create_zip_archive_from_files(&save_files, &archive_path)?;

    let mut url = Url::parse(&format!("{}/api/saves", session.base_url.trim_end_matches('/')))
        .map_err(|error| format!("URL RomM invalide: {}", error))?;
    url.query_pairs_mut()
        .append_pair("rom_id", &mapping.romm_id)
        .append_pair("emulator", "melonds")
        .append_pair("slot", MELONDS_SLOT_NAME)
        .append_pair("overwrite", "true");

    let bytes = fs::read(&archive_path)
        .map_err(|error| format!("Impossible de lire l'archive de save melonDS: {}", error))?;

    let file_name = format!("emumanager-melonds-{}.zip", sanitize_file_stem(rom_path));
    log_sync(
        paths,
        &format!(
            "uploading melonDS save bundle romm_id={} file_name={} archive={} bytes={}",
            mapping.romm_id,
            file_name,
            archive_path.to_string_lossy(),
            bytes.len()
        ),
    );

    let boundary = format!("emumanager-{:016x}", compute_hash(&file_name));
    let body = build_multipart_body(&boundary, "saveFile", &file_name, "application/zip", &bytes);

    let response = block_on(async {
        let client = build_http_client()?;
        client
            .post(url)
            .bearer_auth(&session.token)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(body)
            .send()
            .await
            .map_err(|error| format!("Upload RomM impossible: {}", error))
    })?;

    let status = response.status();
    let response_body = block_on(async {
        response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".to_string())
    });

    log_sync(
        paths,
        &format!("melonDS upload response status={} body={}", status, response_body),
    );

    if !status.is_success() {
        return Err(format!("Upload RomM melonDS Ã©chouÃ© avec le statut {}: {}", status, response_body));
    }

    let uploaded_save = serde_json::from_str::<RommSaveEntry>(&response_body).ok();
    let latest_local_timestamp = latest_melonds_save_timestamp_ms(rom_path)?;

    update_mapping_sync_metadata(
        paths,
        &mapping.rom_path,
        latest_local_timestamp,
        uploaded_save.and_then(|save| save.updated_at),
    )?;

    cleanup_old_remote_saves(paths, session, &mapping.romm_id, "melonds", MELONDS_SLOT_NAME)?;
    Ok(())
}

fn maybe_restore_latest_azahar_save(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    mapping: &RommRomMapping,
    rom_path: &Path,
    profile_root: &Path,
) -> Result<(), String> {
    log_sync(paths, &format!("restore_latest_azahar_save romm_id={}", mapping.romm_id));

    let local_timestamp = latest_azahar_save_timestamp_ms(paths, rom_path)?;

    if let (Some(current_local), Some(last_synced_local)) =
        (local_timestamp, mapping.last_synced_local_save_at_ms)
    {
        if current_local > last_synced_local {
            log_sync(
                paths,
                &format!(
                    "skipping Azahar remote restore because local save is newer than last synced local copy current={} last_synced={}",
                    current_local, last_synced_local
                ),
            );
            return Ok(());
        }
    }

    let latest_save = fetch_emumanager_saves(
        paths,
        session,
        &mapping.romm_id,
        "azahar",
        AZAHAR_SLOT_NAME,
    )?
    .into_iter()
    .max_by(|left, right| left.updated_at.cmp(&right.updated_at));

    let Some(save) = latest_save else {
        log_sync(paths, "no remote Azahar save bundle found");
        return Ok(());
    };

    if mapping.last_remote_save_at.as_ref() == save.updated_at.as_ref() && local_timestamp.is_some() {
        log_sync(paths, "Azahar remote save timestamp matches last known sync, keeping local profile");
        return Ok(());
    }

    log_sync(
        paths,
        &format!(
            "selected remote Azahar save save_id={} file_name={:?} slot={:?} updated_at={:?}",
            save.id.as_string(),
            save.file_name,
            save.slot,
            save.updated_at
        ),
    );

    let archive_name = save
        .file_name
        .clone()
        .unwrap_or_else(|| "azahar-sync.zip".to_string());
    let download_url = format!(
        "{}/api/saves/{}/content",
        session.base_url.trim_end_matches('/'),
        save.id.as_string()
    );
    let archive_path = profile_root.join(&archive_name);

    download_file(paths, session, &download_url, &archive_path)?;

    if profile_root.exists() {
        fs::remove_dir_all(profile_root)
            .map_err(|error| format!("Impossible de rÃ©initialiser le profil Azahar: {}", error))?;
    }
    fs::create_dir_all(profile_root)
        .map_err(|error| format!("Impossible de recrÃ©er le profil Azahar: {}", error))?;

    extract_zip_archive(&archive_path, profile_root)?;
    let _ = fs::remove_file(&archive_path);

    let latest_local_after_restore = latest_azahar_save_timestamp_ms(paths, rom_path)?;
    update_mapping_sync_metadata(
        paths,
        &mapping.rom_path,
        latest_local_after_restore,
        save.updated_at.clone(),
    )?;

    log_sync(
        paths,
        &format!("Azahar remote save extracted into {}", profile_root.to_string_lossy()),
    );

    Ok(())
}

fn upload_azahar_save_bundle(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    mapping: &RommRomMapping,
    rom_path: &Path,
    profile_root: &Path,
) -> Result<(), String> {
    if !profile_root.exists() {
        log_sync(paths, "Azahar upload skipped because profile directory does not exist");
        return Ok(());
    }

    let archive_path = profile_root.join("emumanager-azahar-sync.zip");
    create_zip_archive(profile_root, &archive_path)?;

    let mut url = Url::parse(&format!("{}/api/saves", session.base_url.trim_end_matches('/')))
        .map_err(|error| format!("URL RomM invalide: {}", error))?;
    url.query_pairs_mut()
        .append_pair("rom_id", &mapping.romm_id)
        .append_pair("emulator", "azahar")
        .append_pair("slot", AZAHAR_SLOT_NAME)
        .append_pair("overwrite", "true");

    let bytes = fs::read(&archive_path)
        .map_err(|error| format!("Impossible de lire l'archive de save Azahar: {}", error))?;

    let file_name = format!("emumanager-azahar-{}.zip", sanitize_file_stem(rom_path));
    log_sync(
        paths,
        &format!(
            "uploading Azahar save bundle romm_id={} file_name={} archive={} bytes={}",
            mapping.romm_id,
            file_name,
            archive_path.to_string_lossy(),
            bytes.len()
        ),
    );

    let boundary = format!("emumanager-{:016x}", compute_hash(&file_name));
    let body = build_multipart_body(&boundary, "saveFile", &file_name, "application/zip", &bytes);

    let response = block_on(async {
        let client = build_http_client()?;
        client
            .post(url)
            .bearer_auth(&session.token)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(body)
            .send()
            .await
            .map_err(|error| format!("Upload RomM impossible: {}", error))
    })?;

    let status = response.status();
    let response_body = block_on(async {
        response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".to_string())
    });

    log_sync(
        paths,
        &format!("Azahar upload response status={} body={}", status, response_body),
    );

    if !status.is_success() {
        return Err(format!("Upload RomM Azahar Ã©chouÃ© avec le statut {}: {}", status, response_body));
    }

    let uploaded_save = serde_json::from_str::<RommSaveEntry>(&response_body).ok();
    let latest_local_timestamp = latest_azahar_save_timestamp_ms(paths, rom_path)?;

    update_mapping_sync_metadata(
        paths,
        &mapping.rom_path,
        latest_local_timestamp,
        uploaded_save.and_then(|save| save.updated_at),
    )?;

    cleanup_old_remote_saves(paths, session, &mapping.romm_id, "azahar", AZAHAR_SLOT_NAME)?;
    Ok(())
}

fn maybe_restore_latest_eden_save(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    mapping: &RommRomMapping,
    rom_path: &Path,
    profile_root: &Path,
) -> Result<(), String> {
    log_sync(paths, &format!("restore_latest_eden_save romm_id={}", mapping.romm_id));

    let local_timestamp = latest_eden_save_timestamp_ms(paths, rom_path)?;

    if let (Some(current_local), Some(last_synced_local)) =
        (local_timestamp, mapping.last_synced_local_save_at_ms)
    {
        if current_local > last_synced_local {
            log_sync(
                paths,
                &format!(
                    "skipping Eden remote restore because local save is newer than last synced local copy current={} last_synced={}",
                    current_local, last_synced_local
                ),
            );
            return Ok(());
        }
    }

    let latest_save = fetch_emumanager_saves(
        paths,
        session,
        &mapping.romm_id,
        "eden",
        EDEN_SLOT_NAME,
    )?
    .into_iter()
    .max_by(|left, right| left.updated_at.cmp(&right.updated_at));

    let Some(save) = latest_save else {
        log_sync(paths, "no remote Eden save bundle found");
        return Ok(());
    };

    if mapping.last_remote_save_at.as_ref() == save.updated_at.as_ref() && local_timestamp.is_some() {
        log_sync(paths, "Eden remote save timestamp matches last known sync, keeping local profile");
        return Ok(());
    }

    log_sync(
        paths,
        &format!(
            "selected remote Eden save save_id={} file_name={:?} slot={:?} updated_at={:?}",
            save.id.as_string(),
            save.file_name,
            save.slot,
            save.updated_at
        ),
    );

    let archive_name = save
        .file_name
        .clone()
        .unwrap_or_else(|| "eden-sync.zip".to_string());
    let download_url = format!(
        "{}/api/saves/{}/content",
        session.base_url.trim_end_matches('/'),
        save.id.as_string()
    );
    let archive_path = profile_root.join(&archive_name);

    download_file(paths, session, &download_url, &archive_path)?;

    fs::create_dir_all(profile_root)
        .map_err(|error| format!("Impossible de prÃ©parer le profil Eden: {}", error))?;
    for path in eden_sync_paths(profile_root) {
        remove_path_if_exists(&path)?;
    }

    extract_zip_archive(&archive_path, profile_root)?;
    let _ = fs::remove_file(&archive_path);

    let latest_local_after_restore = latest_eden_save_timestamp_ms(paths, rom_path)?;
    update_mapping_sync_metadata(
        paths,
        &mapping.rom_path,
        latest_local_after_restore,
        save.updated_at.clone(),
    )?;

    log_sync(
        paths,
        &format!("Eden remote save extracted into {}", profile_root.to_string_lossy()),
    );

    Ok(())
}

fn upload_eden_save_bundle(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    mapping: &RommRomMapping,
    rom_path: &Path,
    profile_root: &Path,
) -> Result<(), String> {
    if !profile_root.exists() {
        log_sync(paths, "Eden upload skipped because profile directory does not exist");
        return Ok(());
    }

    let sync_paths = eden_sync_paths(profile_root);
    if !sync_paths.iter().any(|path| path.exists()) {
        log_sync(paths, "Eden upload skipped because no selected save paths were found");
        return Ok(());
    }

    let archive_path = profile_root.join("emumanager-eden-sync.zip");
    create_zip_archive_from_paths(profile_root, &sync_paths, &archive_path)?;

    let mut url = Url::parse(&format!("{}/api/saves", session.base_url.trim_end_matches('/')))
        .map_err(|error| format!("URL RomM invalide: {}", error))?;
    url.query_pairs_mut()
        .append_pair("rom_id", &mapping.romm_id)
        .append_pair("emulator", "eden")
        .append_pair("slot", EDEN_SLOT_NAME)
        .append_pair("overwrite", "true");

    let bytes = fs::read(&archive_path)
        .map_err(|error| format!("Impossible de lire l'archive de save Eden: {}", error))?;

    let file_name = format!("emumanager-eden-{}.zip", sanitize_file_stem(rom_path));
    log_sync(
        paths,
        &format!(
            "uploading Eden save bundle romm_id={} file_name={} archive={} bytes={}",
            mapping.romm_id,
            file_name,
            archive_path.to_string_lossy(),
            bytes.len()
        ),
    );

    let boundary = format!("emumanager-{:016x}", compute_hash(&file_name));
    let body = build_multipart_body(&boundary, "saveFile", &file_name, "application/zip", &bytes);

    let response = block_on(async {
        let client = build_http_client()?;
        client
            .post(url)
            .bearer_auth(&session.token)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(body)
            .send()
            .await
            .map_err(|error| format!("Upload RomM impossible: {}", error))
    })?;

    let status = response.status();
    let response_body = block_on(async {
        response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".to_string())
    });

    log_sync(
        paths,
        &format!("Eden upload response status={} body={}", status, response_body),
    );

    if !status.is_success() {
        return Err(format!("Upload RomM Eden Ã©chouÃ© avec le statut {}: {}", status, response_body));
    }

    let uploaded_save = serde_json::from_str::<RommSaveEntry>(&response_body).ok();
    let latest_local_timestamp = latest_eden_save_timestamp_ms(paths, rom_path)?;

    update_mapping_sync_metadata(
        paths,
        &mapping.rom_path,
        latest_local_timestamp,
        uploaded_save.and_then(|save| save.updated_at),
    )?;

    cleanup_old_remote_saves(paths, session, &mapping.romm_id, "eden", EDEN_SLOT_NAME)?;
    Ok(())
}

fn maybe_restore_latest_pcsx2_save(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    mapping: &RommRomMapping,
    rom_path: &Path,
    profile_root: &Path,
) -> Result<(), String> {
    log_sync(paths, &format!("restore_latest_pcsx2_save romm_id={}", mapping.romm_id));

    let local_timestamp = latest_pcsx2_save_timestamp_ms(paths, rom_path)?;

    if let (Some(current_local), Some(last_synced_local)) =
        (local_timestamp, mapping.last_synced_local_save_at_ms)
    {
        if current_local > last_synced_local {
            log_sync(
                paths,
                &format!(
                    "skipping PCSX2 remote restore because local save is newer than last synced local copy current={} last_synced={}",
                    current_local, last_synced_local
                ),
            );
            return Ok(());
        }
    }

    let latest_save = fetch_emumanager_saves(
        paths,
        session,
        &mapping.romm_id,
        "pcsx2",
        PCSX2_SLOT_NAME,
    )?
    .into_iter()
    .max_by(|left, right| left.updated_at.cmp(&right.updated_at));

    let Some(save) = latest_save else {
        log_sync(paths, "no remote PCSX2 save bundle found");
        return Ok(());
    };

    if mapping.last_remote_save_at.as_ref() == save.updated_at.as_ref() && local_timestamp.is_some() {
        log_sync(paths, "PCSX2 remote save timestamp matches last known sync, keeping local profile");
        return Ok(());
    }

    log_sync(
        paths,
        &format!(
            "selected remote PCSX2 save save_id={} file_name={:?} slot={:?} updated_at={:?}",
            save.id.as_string(),
            save.file_name,
            save.slot,
            save.updated_at
        ),
    );

    let archive_name = save
        .file_name
        .clone()
        .unwrap_or_else(|| "pcsx2-sync.zip".to_string());
    let download_url = format!(
        "{}/api/saves/{}/content",
        session.base_url.trim_end_matches('/'),
        save.id.as_string()
    );
    let archive_path = profile_root.join(&archive_name);

    download_file(paths, session, &download_url, &archive_path)?;

    fs::create_dir_all(profile_root)
        .map_err(|error| format!("Impossible de prÃ©parer le profil PCSX2: {}", error))?;
    for path in pcsx2_sync_paths(profile_root) {
        remove_path_if_exists(&path)?;
    }

    extract_zip_archive(&archive_path, profile_root)?;
    let _ = fs::remove_file(&archive_path);

    let latest_local_after_restore = latest_pcsx2_save_timestamp_ms(paths, rom_path)?;
    update_mapping_sync_metadata(
        paths,
        &mapping.rom_path,
        latest_local_after_restore,
        save.updated_at.clone(),
    )?;

    log_sync(
        paths,
        &format!("PCSX2 remote save extracted into {}", profile_root.to_string_lossy()),
    );

    Ok(())
}

fn upload_pcsx2_save_bundle(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    mapping: &RommRomMapping,
    rom_path: &Path,
    profile_root: &Path,
) -> Result<(), String> {
    if !profile_root.exists() {
        log_sync(paths, "PCSX2 upload skipped because profile directory does not exist");
        return Ok(());
    }

    let sync_paths = pcsx2_sync_paths(profile_root);
    if !sync_paths.iter().any(|path| path.exists()) {
        log_sync(paths, "PCSX2 upload skipped because no selected save paths were found");
        return Ok(());
    }

    let archive_path = profile_root.join("emumanager-pcsx2-sync.zip");
    create_zip_archive_from_paths(profile_root, &sync_paths, &archive_path)?;

    let mut url = Url::parse(&format!("{}/api/saves", session.base_url.trim_end_matches('/')))
        .map_err(|error| format!("URL RomM invalide: {}", error))?;
    url.query_pairs_mut()
        .append_pair("rom_id", &mapping.romm_id)
        .append_pair("emulator", "pcsx2")
        .append_pair("slot", PCSX2_SLOT_NAME)
        .append_pair("overwrite", "true");

    let bytes = fs::read(&archive_path)
        .map_err(|error| format!("Impossible de lire l'archive de save PCSX2: {}", error))?;

    let file_name = format!("emumanager-pcsx2-{}.zip", sanitize_file_stem(rom_path));
    log_sync(
        paths,
        &format!(
            "uploading PCSX2 save bundle romm_id={} file_name={} archive={} bytes={}",
            mapping.romm_id,
            file_name,
            archive_path.to_string_lossy(),
            bytes.len()
        ),
    );

    let boundary = format!("emumanager-{:016x}", compute_hash(&file_name));
    let body = build_multipart_body(&boundary, "saveFile", &file_name, "application/zip", &bytes);

    let response = block_on(async {
        let client = build_http_client()?;
        client
            .post(url)
            .bearer_auth(&session.token)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(body)
            .send()
            .await
            .map_err(|error| format!("Upload RomM impossible: {}", error))
    })?;

    let status = response.status();
    let response_body = block_on(async {
        response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".to_string())
    });

    log_sync(
        paths,
        &format!("PCSX2 upload response status={} body={}", status, response_body),
    );

    if !status.is_success() {
        return Err(format!("Upload RomM PCSX2 Ã©chouÃ© avec le statut {}: {}", status, response_body));
    }

    let uploaded_save = serde_json::from_str::<RommSaveEntry>(&response_body).ok();
    let latest_local_timestamp = latest_pcsx2_save_timestamp_ms(paths, rom_path)?;

    update_mapping_sync_metadata(
        paths,
        &mapping.rom_path,
        latest_local_timestamp,
        uploaded_save.and_then(|save| save.updated_at),
    )?;

    cleanup_old_remote_saves(paths, session, &mapping.romm_id, "pcsx2", PCSX2_SLOT_NAME)?;
    Ok(())
}

fn cleanup_old_remote_saves(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    romm_id: &str,
    emulator_id: &str,
    slot_name: &str,
) -> Result<(), String> {
    let mut saves = fetch_emumanager_saves(paths, session, romm_id, emulator_id, slot_name)?;
    saves.sort_by(|left, right| left.updated_at.cmp(&right.updated_at));

    if saves.len() <= MAX_REMOTE_SAVES {
        log_sync(
            paths,
            &format!("remote save cleanup skipped count={} limit={}", saves.len(), MAX_REMOTE_SAVES),
        );
        return Ok(());
    }

    let to_delete_count = saves.len().saturating_sub(MAX_REMOTE_SAVES);
    let ids_to_delete: Vec<String> = saves
        .into_iter()
        .take(to_delete_count)
        .map(|save| save.id.as_string())
        .collect();

    log_sync(
        paths,
        &format!("deleting oldest remote saves ids={:?}", ids_to_delete),
    );

    let payload = format!(
        "{{\"saves\":[{}]}}",
        ids_to_delete
            .iter()
            .map(|id| id.parse::<i64>().map(|numeric| numeric.to_string()).unwrap_or_else(|_| "0".to_string()))
            .collect::<Vec<_>>()
            .join(",")
    );

    let url = format!("{}/api/saves/delete", session.base_url.trim_end_matches('/'));
    let response = block_on(async {
        let client = build_http_client()?;
        client
            .post(url)
            .bearer_auth(&session.token)
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await
            .map_err(|error| format!("Suppression des anciennes saves RomM impossible: {}", error))
    })?;

    let status = response.status();
    let body = block_on(async {
        response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".to_string())
    });
    log_sync(paths, &format!("cleanup response status={} body={}", status, body));

    if !status.is_success() {
        return Err(format!(
            "Suppression des anciennes saves RomM échouée avec le statut {}: {}",
            status, body
        ));
    }

    Ok(())
}

fn fetch_emumanager_saves(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    romm_id: &str,
    emulator_id: &str,
    slot_name: &str,
) -> Result<Vec<RommSaveEntry>, String> {
    let mut url = Url::parse(&format!("{}/api/saves", session.base_url.trim_end_matches('/')))
        .map_err(|error| format!("URL RomM invalide: {}", error))?;
    url.query_pairs_mut()
        .append_pair("rom_id", romm_id)
        .append_pair("emulator", emulator_id)
        .append_pair("slot", slot_name);

    log_sync(paths, &format!("fetching remote saves from {}", url));

    let response = block_on(async {
        let client = build_http_client()?;
        client
            .get(url)
            .bearer_auth(&session.token)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|error| format!("Lecture des saves RomM impossible: {}", error))
    })?;

    let status = response.status();
    let raw = block_on(async {
        response
            .text()
            .await
            .map_err(|error| format!("Réponse RomM illisible pour les saves: {}", error))
    })?;

    log_sync(paths, &format!("fetch saves status={} body={}", status, raw));

    if !status.is_success() {
        return Err(format!(
            "Récupération des saves RomM échouée avec le statut {}: {}",
            status, raw
        ));
    }

    serde_json::from_str::<Vec<RommSaveEntry>>(&raw)
        .map_err(|error| format!("Réponse RomM invalide pour les saves: {}", error))
}

fn resolve_mapping_from_remote(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    rom_path: &Path,
) -> Result<Option<RommRomMapping>, String> {
    let file_name = rom_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Nom de fichier ROM invalide".to_string())?;

    let mut url = Url::parse(&format!("{}/api/roms", session.base_url.trim_end_matches('/')))
        .map_err(|error| format!("URL RomM invalide: {}", error))?;
    url.query_pairs_mut().append_pair("search_term", file_name);

    log_sync(
        paths,
        &format!("trying remote rom lookup for file_name={} url={}", file_name, url),
    );

    let response = block_on(async {
        let client = build_http_client()?;
        client
            .get(url)
            .bearer_auth(&session.token)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|error| format!("Recherche RomM impossible: {}", error))
    })?;

    let status = response.status();
    let raw = block_on(async {
        response
            .text()
            .await
            .map_err(|error| format!("Réponse RomM illisible pour les roms: {}", error))
    })?;

    log_sync(paths, &format!("rom lookup status={} body={}", status, raw));

    if !status.is_success() {
        return Err(format!("Recherche RomM échouée avec le statut {}: {}", status, raw));
    }

    let payload = serde_json::from_str::<RommGamesResponse>(&raw)
        .map_err(|error| format!("Réponse RomM invalide pour les roms: {}", error))?;

    let games = match payload {
        RommGamesResponse::Direct(entries) => entries,
        RommGamesResponse::Wrapped { items, results } => items.or(results).unwrap_or_default(),
    };

    let normalized_target = normalize_lookup_value(file_name);
    let target_stem = normalize_lookup_value(
        rom_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(file_name),
    );

    let mut candidates = Vec::new();
    for game in &games {
        candidates.push(format!(
            "candidate romm_id={} names={:?}",
            game.id.as_string(),
            resolve_remote_game_file_names(game)
        ));
    }
    log_sync(paths, &format!("rom lookup candidates={}", candidates.join(" | ")));

    let mut matched = games.into_iter().find(|game| {
        let names = resolve_remote_game_file_names(game);
        names.iter().any(|candidate| {
            let normalized_candidate = normalize_lookup_value(candidate);
            normalized_candidate == normalized_target
                || strip_extension(&normalized_candidate) == target_stem
        })
    });

    if matched.is_none() {
        let mut fallback_games = match payload_again_from_single_search(paths, session, file_name)? {
            Some(entries) => entries,
            None => Vec::new(),
        };

        if fallback_games.len() == 1 {
            let only = fallback_games.remove(0);
            log_sync(
                paths,
                &format!(
                    "accepting single remote rom lookup result romm_id={} without exact filename match",
                    only.id.as_string()
                ),
            );
            matched = Some(only);
        }
    }

    let Some(game) = matched else {
        return Ok(None);
    };

    let resolved_file_name = resolve_remote_game_file_name(&game);
    let mapping = RommRomMapping {
        rom_path: rom_path.to_string_lossy().to_string(),
        romm_id: game.id.as_string(),
        platform_name: game.name.clone(),
        file_name: resolved_file_name.clone(),
        last_synced_local_save_at_ms: None,
        last_remote_save_at: None,
    };

    log_sync(
        paths,
        &format!(
            "remote rom lookup matched romm_id={} file_name={:?}",
            mapping.romm_id, resolved_file_name
        ),
    );

    register_rom_mapping(
        paths,
        &mapping.rom_path,
        &mapping.romm_id,
        mapping.platform_name.as_deref(),
        mapping.file_name.as_deref(),
    )?;

    Ok(Some(mapping))
}

fn download_file(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    url: &str,
    destination: &Path,
) -> Result<(), String> {
    log_sync(
        paths,
        &format!(
            "downloading remote save content url={} destination={}",
            url,
            destination.to_string_lossy()
        ),
    );

    let response = block_on(async {
        let client = build_http_client()?;
        client
            .get(url)
            .bearer_auth(&session.token)
            .send()
            .await
            .map_err(|error| format!("Téléchargement de save RomM impossible: {}", error))
    })?;

    let status = response.status();
    if !status.is_success() {
        let body = block_on(async {
            response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".to_string())
        });
        return Err(format!(
            "Téléchargement de save RomM échoué avec le statut {}: {}",
            status, body
        ));
    }

    let bytes = block_on(async {
        response
            .bytes()
            .await
            .map_err(|error| format!("Lecture de save RomM impossible: {}", error))
    })?;

    let mut file = fs::File::create(destination)
        .map_err(|error| format!("Impossible de créer le fichier save local: {}", error))?;
    file.write_all(bytes.as_ref())
        .map_err(|error| format!("Impossible d'écrire le fichier save local: {}", error))?;

    log_sync(paths, &format!("downloaded {} bytes", bytes.len()));
    Ok(())
}

fn build_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .build()
        .map_err(|error| format!("Impossible d'initialiser le client HTTP: {}", error))
}

fn dolphin_profile_root(paths: &PortablePaths, rom_path: &Path) -> Result<PathBuf, String> {
    let roms_root = Path::new(&paths.roms);
    let relative = rom_path
        .strip_prefix(roms_root)
        .unwrap_or(rom_path)
        .to_string_lossy()
        .to_string();

    let mut hasher = DefaultHasher::new();
    relative.hash(&mut hasher);
    let hash = hasher.finish();

    Ok(Path::new(&paths.data).join("DolphinProfiles").join(format!(
        "{}-{:016x}",
        sanitize_path_fragment(&relative),
        hash
    )))
}

fn latest_dolphin_save_timestamp_ms(
    paths: &PortablePaths,
    rom_path: &Path,
) -> Result<Option<u64>, String> {
    let profile_root = dolphin_profile_root(paths, rom_path)?;
    let user_dir = profile_root.join("User");
    let gc_dir = user_dir.join("GC");
    let wii_dir = user_dir.join("Wii");

    let mut latest: Option<u64> = None;

    for directory in [&gc_dir, &wii_dir] {
        if directory.exists() {
            let candidate = latest_modified_in_dir(directory)?;
            latest = match (latest, candidate) {
                (Some(left), Some(right)) => Some(left.max(right)),
                (Some(left), None) => Some(left),
                (None, Some(right)) => Some(right),
                (None, None) => None,
            };
        }
    }

    Ok(latest)
}

fn latest_melonds_save_timestamp_ms(rom_path: &Path) -> Result<Option<u64>, String> {
    let save_files = collect_melonds_save_files(rom_path)?;
    let mut latest: Option<u64> = None;

    for save_file in save_files {
        let metadata = fs::metadata(&save_file).map_err(|error| {
            format!(
                "Impossible de lire les mÃ©tadonnÃ©es de {}: {}",
                save_file.to_string_lossy(),
                error
            )
        })?;
        let modified = metadata.modified().ok().and_then(system_time_to_epoch_ms);
        latest = match (latest, modified) {
            (Some(left), Some(right)) => Some(left.max(right)),
            (Some(left), None) => Some(left),
            (None, Some(right)) => Some(right),
            (None, None) => None,
        };
    }

    Ok(latest)
}

fn melonds_cache_root(paths: &PortablePaths, rom_path: &Path) -> Result<PathBuf, String> {
    let roms_root = Path::new(&paths.roms);
    let relative = rom_path
        .strip_prefix(roms_root)
        .unwrap_or(rom_path)
        .to_string_lossy()
        .to_string();

    let mut hasher = DefaultHasher::new();
    relative.hash(&mut hasher);
    let hash = hasher.finish();

    Ok(Path::new(&paths.data).join("melonDSSaves").join(format!(
        "{}-{:016x}",
        sanitize_path_fragment(&relative),
        hash
    )))
}

fn azahar_profile_root(paths: &PortablePaths, rom_path: &Path) -> Result<PathBuf, String> {
    let roms_root = Path::new(&paths.roms);
    let relative = rom_path
        .strip_prefix(roms_root)
        .unwrap_or(rom_path)
        .to_string_lossy()
        .to_string();

    let mut hasher = DefaultHasher::new();
    relative.hash(&mut hasher);
    let hash = hasher.finish();

    Ok(Path::new(&paths.data).join("AzaharProfiles").join(format!(
        "{}-{:016x}",
        sanitize_path_fragment(&relative),
        hash
    )))
}

fn eden_profile_root(paths: &PortablePaths, rom_path: &Path) -> Result<PathBuf, String> {
    let roms_root = Path::new(&paths.roms);
    let relative = rom_path
        .strip_prefix(roms_root)
        .unwrap_or(rom_path)
        .to_string_lossy()
        .to_string();

    let mut hasher = DefaultHasher::new();
    relative.hash(&mut hasher);
    let hash = hasher.finish();

    Ok(Path::new(&paths.data).join("EdenProfiles").join(format!(
        "{}-{:016x}",
        sanitize_path_fragment(&relative),
        hash
    )))
}

fn pcsx2_profile_root(paths: &PortablePaths, rom_path: &Path) -> Result<PathBuf, String> {
    let roms_root = Path::new(&paths.roms);
    let relative = rom_path
        .strip_prefix(roms_root)
        .unwrap_or(rom_path)
        .to_string_lossy()
        .to_string();

    let mut hasher = DefaultHasher::new();
    relative.hash(&mut hasher);
    let hash = hasher.finish();

    Ok(Path::new(&paths.data).join("PCSX2Profiles").join(format!(
        "{}-{:016x}",
        sanitize_path_fragment(&relative),
        hash
    )))
}

fn collect_melonds_save_files(rom_path: &Path) -> Result<Vec<PathBuf>, String> {
    let Some(parent) = rom_path.parent() else {
        return Ok(Vec::new());
    };
    let file_name = rom_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Nom de ROM melonDS invalide".to_string())?;
    let stem = rom_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Nom de ROM melonDS invalide".to_string())?;

    let mut results = Vec::new();
    for entry_result in fs::read_dir(parent)
        .map_err(|error| format!("Impossible de lire le dossier de saves melonDS: {}", error))?
    {
        let entry = entry_result
            .map_err(|error| format!("Impossible de lire une entrÃ©e de save melonDS: {}", error))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(candidate_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        if is_melonds_save_file_name(candidate_name, file_name, stem) {
            results.push(path);
        }
    }

    results.sort();
    Ok(results)
}

fn remove_melonds_local_save_files(rom_path: &Path) -> Result<(), String> {
    for save_file in collect_melonds_save_files(rom_path)? {
        fs::remove_file(&save_file).map_err(|error| {
            format!(
                "Impossible de supprimer la save locale melonDS {}: {}",
                save_file.to_string_lossy(),
                error
            )
        })?;
    }

    Ok(())
}

fn latest_azahar_save_timestamp_ms(
    paths: &PortablePaths,
    rom_path: &Path,
) -> Result<Option<u64>, String> {
    let profile_root = azahar_profile_root(paths, rom_path)?;
    let mut latest: Option<u64> = None;

    for directory in [
        profile_root.join("sdmc"),
        profile_root.join("nand"),
        profile_root.join("sysdata"),
        profile_root.join("config").join("custom"),
    ] {
        if directory.exists() {
            let candidate = latest_modified_in_dir(&directory)?;
            latest = match (latest, candidate) {
                (Some(left), Some(right)) => Some(left.max(right)),
                (Some(left), None) => Some(left),
                (None, Some(right)) => Some(right),
                (None, None) => None,
            };
        }
    }

    Ok(latest)
}

fn latest_eden_save_timestamp_ms(
    paths: &PortablePaths,
    rom_path: &Path,
) -> Result<Option<u64>, String> {
    let profile_root = eden_profile_root(paths, rom_path)?;
    let mut latest: Option<u64> = None;

    for path in eden_sync_paths(&profile_root) {
        if path.exists() {
            let candidate = if path.is_dir() {
                latest_modified_in_dir(&path)?
            } else {
                fs::metadata(&path)
                    .ok()
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(system_time_to_epoch_ms)
            };
            latest = match (latest, candidate) {
                (Some(left), Some(right)) => Some(left.max(right)),
                (Some(left), None) => Some(left),
                (None, Some(right)) => Some(right),
                (None, None) => None,
            };
        }
    }

    Ok(latest)
}

fn latest_pcsx2_save_timestamp_ms(
    paths: &PortablePaths,
    rom_path: &Path,
) -> Result<Option<u64>, String> {
    let profile_root = pcsx2_profile_root(paths, rom_path)?;
    let mut latest: Option<u64> = None;

    for path in pcsx2_sync_paths(&profile_root) {
        if path.exists() {
            let candidate = if path.is_dir() {
                latest_modified_in_dir(&path)?
            } else {
                fs::metadata(&path)
                    .ok()
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(system_time_to_epoch_ms)
            };
            latest = match (latest, candidate) {
                (Some(left), Some(right)) => Some(left.max(right)),
                (Some(left), None) => Some(left),
                (None, Some(right)) => Some(right),
                (None, None) => None,
            };
        }
    }

    Ok(latest)
}

fn pcsx2_sync_paths(profile_root: &Path) -> Vec<PathBuf> {
    vec![
        profile_root.join("memcards"),
        profile_root.join("sstates"),
    ]
}

fn eden_sync_paths(profile_root: &Path) -> Vec<PathBuf> {
    vec![
        profile_root.join("nand").join("user").join("save"),
        profile_root.join("sdmc").join("Nintendo"),
        profile_root.join("play_time").join("playtime.bin"),
        profile_root.join("config").join("custom"),
    ]
}

fn is_melonds_save_file_name(candidate_name: &str, rom_file_name: &str, rom_stem: &str) -> bool {
    let candidate_lower = candidate_name.to_ascii_lowercase();
    let rom_file_name_lower = rom_file_name.to_ascii_lowercase();
    let rom_stem_lower = rom_stem.to_ascii_lowercase();

    matches!(
        candidate_lower.as_str(),
        value if value == format!("{}.sav", rom_stem_lower)
            || value == format!("{}.dsv", rom_stem_lower)
            || value == format!("{}.sav", rom_file_name_lower)
            || value == format!("{}.dsv", rom_file_name_lower)
    ) || candidate_lower.starts_with(&format!("{}.ml", rom_stem_lower))
        || candidate_lower.starts_with(&format!("{}.ml", rom_file_name_lower))
}

fn latest_modified_in_dir(directory: &Path) -> Result<Option<u64>, String> {
    let mut latest: Option<u64> = None;

    for entry_result in fs::read_dir(directory)
        .map_err(|error| format!("Impossible de lire {}: {}", directory.to_string_lossy(), error))?
    {
        let entry = entry_result
            .map_err(|error| format!("Impossible de lire une entrée dans {}: {}", directory.to_string_lossy(), error))?;
        let path = entry.path();

        if path.is_dir() {
            let nested = latest_modified_in_dir(&path)?;
            latest = match (latest, nested) {
                (Some(left), Some(right)) => Some(left.max(right)),
                (Some(left), None) => Some(left),
                (None, Some(right)) => Some(right),
                (None, None) => None,
            };
            continue;
        }

        let metadata = fs::metadata(&path)
            .map_err(|error| format!("Impossible de lire les métadonnées de {}: {}", path.to_string_lossy(), error))?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(system_time_to_epoch_ms);

        latest = match (latest, modified) {
            (Some(left), Some(right)) => Some(left.max(right)),
            (Some(left), None) => Some(left),
            (None, Some(right)) => Some(right),
            (None, None) => None,
        };
    }

    Ok(latest)
}

fn seed_profile_from_base_user(paths: &PortablePaths, profile_user_dir: &Path) -> Result<(), String> {
    if profile_user_dir.join("Config").exists() {
        log_sync(paths, "profile already seeded, keeping existing User directory");
        return Ok(());
    }

    fs::create_dir_all(profile_user_dir)
        .map_err(|error| format!("Impossible de créer le profil Dolphin: {}", error))?;

    let base_install_dir = Path::new(&paths.emu).join("Dolphin");
    let base_user_dir = if base_install_dir.join("Dolphin-x64").join("User").exists() {
        base_install_dir.join("Dolphin-x64").join("User")
    } else {
        base_install_dir.join("User")
    };

    if base_user_dir.exists() {
        copy_directory_recursive(&base_user_dir, profile_user_dir)?;
        log_sync(
            paths,
            &format!(
                "seeded profile from base user dir {}",
                base_user_dir.to_string_lossy()
            ),
        );
    }

    if !profile_user_dir.join("Config").exists() {
        fs::create_dir_all(profile_user_dir.join("Config"))
            .map_err(|error| format!("Impossible de créer Config pour Dolphin: {}", error))?;
    }

    Ok(())
}

fn seed_azahar_profile_from_base_user(paths: &PortablePaths, profile_root: &Path) -> Result<(), String> {
    if profile_root.join("config").exists() || profile_root.join("sdmc").exists() {
        log_sync(paths, "Azahar profile already seeded, keeping existing user directory");
        return Ok(());
    }

    fs::create_dir_all(profile_root)
        .map_err(|error| format!("Impossible de crÃ©er le profil Azahar: {}", error))?;

    let base_user_dir = azahar_base_user_dir(paths)?;
    if base_user_dir.exists() {
        copy_directory_recursive(&base_user_dir, profile_root)?;
        log_sync(
            paths,
            &format!(
                "seeded Azahar profile from base user dir {}",
                base_user_dir.to_string_lossy()
            ),
        );
    }

    for directory in ["config", "sdmc", "nand", "sysdata"] {
        let path = profile_root.join(directory);
        if !path.exists() {
            fs::create_dir_all(&path)
                .map_err(|error| format!("Impossible de crÃ©er {} pour Azahar: {}", directory, error))?;
        }
    }

    Ok(())
}

fn azahar_base_user_dir(paths: &PortablePaths) -> Result<PathBuf, String> {
    let install_root = Path::new(&paths.emu).join("Azahar");
    let executable_dir = locate_azahar_executable_dir(&install_root)?;
    let portable_dir = executable_dir.join("user");
    if portable_dir.exists() {
        return Ok(portable_dir);
    }

    let app_data = std::env::var("APPDATA")
        .map_err(|error| format!("Impossible de lire APPDATA pour Azahar: {}", error))?;
    Ok(Path::new(&app_data).join("Azahar"))
}

fn locate_azahar_executable_dir(install_root: &Path) -> Result<PathBuf, String> {
    let direct_exe = install_root.join("azahar.exe");
    if direct_exe.exists() {
        return Ok(install_root.to_path_buf());
    }

    for entry_result in fs::read_dir(install_root)
        .map_err(|error| format!("Impossible de lire le dossier Azahar: {}", error))?
    {
        let entry = entry_result
            .map_err(|error| format!("Impossible de lire une entrÃ©e Azahar: {}", error))?;
        let path = entry.path();
        if path.is_dir() && path.join("azahar.exe").exists() {
            return Ok(path);
        }
    }

    Err(format!(
        "Impossible de localiser azahar.exe dans {}",
        install_root.to_string_lossy()
    ))
}

fn materialize_azahar_portable_user(
    paths: &PortablePaths,
    profile_root: &Path,
    portable_user_dir: &Path,
) -> Result<(), String> {
    if portable_user_dir.exists() {
        fs::remove_dir_all(portable_user_dir)
            .map_err(|error| format!("Impossible de rÃ©initialiser le dossier user Azahar: {}", error))?;
    }

    copy_directory_recursive(profile_root, portable_user_dir)?;
    log_sync(
        paths,
        &format!(
            "materialized Azahar portable user {} from {}",
            portable_user_dir.to_string_lossy(),
            profile_root.to_string_lossy()
        ),
    );
    Ok(())
}

fn sync_azahar_portable_user_back(
    paths: &PortablePaths,
    portable_user_dir: &Path,
    profile_root: &Path,
) -> Result<(), String> {
    if !portable_user_dir.exists() {
        log_sync(paths, "Azahar portable user directory missing after launch, skipping local sync back");
        return Ok(());
    }

    if profile_root.exists() {
        fs::remove_dir_all(profile_root)
            .map_err(|error| format!("Impossible de nettoyer le profil Azahar local: {}", error))?;
    }

    copy_directory_recursive(portable_user_dir, profile_root)?;
    log_sync(
        paths,
        &format!(
            "synced Azahar portable user back from {} to {}",
            portable_user_dir.to_string_lossy(),
            profile_root.to_string_lossy()
        ),
    );
    Ok(())
}

fn seed_eden_profile_from_base_user(paths: &PortablePaths, profile_root: &Path) -> Result<(), String> {
    if profile_root.join("config").exists() || profile_root.join("nand").exists() || profile_root.join("sdmc").exists()
    {
        log_sync(paths, "Eden profile already seeded, keeping existing user directory");
        return Ok(());
    }

    fs::create_dir_all(profile_root)
        .map_err(|error| format!("Impossible de crÃ©er le profil Eden: {}", error))?;

    let base_user_dir = eden_base_user_dir()?;
    if base_user_dir.exists() {
        copy_directory_recursive(&base_user_dir, profile_root)?;
        log_sync(
            paths,
            &format!(
                "seeded Eden profile from base user dir {}",
                base_user_dir.to_string_lossy()
            ),
        );
    }

    for directory in ["config", "nand", "sdmc", "keys", "load", "amiibo", "play_time"] {
        let path = profile_root.join(directory);
        if !path.exists() {
            fs::create_dir_all(&path)
                .map_err(|error| format!("Impossible de crÃ©er {} pour Eden: {}", directory, error))?;
        }
    }

    Ok(())
}

fn eden_base_user_dir() -> Result<PathBuf, String> {
    let app_data = std::env::var("APPDATA")
        .map_err(|error| format!("Impossible de lire APPDATA pour Eden: {}", error))?;
    Ok(Path::new(&app_data).join("eden"))
}

fn materialize_eden_portable_user(
    paths: &PortablePaths,
    profile_root: &Path,
    portable_user_dir: &Path,
) -> Result<(), String> {
    if portable_user_dir.exists() {
        fs::remove_dir_all(portable_user_dir)
            .map_err(|error| format!("Impossible de rÃ©initialiser le dossier user Eden: {}", error))?;
    }

    copy_directory_recursive(profile_root, portable_user_dir)?;
    log_sync(
        paths,
        &format!(
            "materialized Eden portable user {} from {}",
            portable_user_dir.to_string_lossy(),
            profile_root.to_string_lossy()
        ),
    );
    Ok(())
}

fn sync_eden_portable_user_back(
    paths: &PortablePaths,
    portable_user_dir: &Path,
    profile_root: &Path,
) -> Result<(), String> {
    if !portable_user_dir.exists() {
        log_sync(paths, "Eden portable user directory missing after launch, skipping local sync back");
        return Ok(());
    }

    if profile_root.exists() {
        fs::remove_dir_all(profile_root)
            .map_err(|error| format!("Impossible de nettoyer le profil Eden local: {}", error))?;
    }

    copy_directory_recursive(portable_user_dir, profile_root)?;
    log_sync(
        paths,
        &format!(
            "synced Eden portable user back from {} to {}",
            portable_user_dir.to_string_lossy(),
            profile_root.to_string_lossy()
        ),
    );
    Ok(())
}

fn seed_pcsx2_profile_from_base(paths: &PortablePaths, profile_root: &Path) -> Result<(), String> {
    if profile_root.join("memcards").exists() || profile_root.join("sstates").exists() {
        log_sync(paths, "PCSX2 profile already seeded, keeping existing save directories");
        return Ok(());
    }

    fs::create_dir_all(profile_root)
        .map_err(|error| format!("Impossible de crÃ©er le profil PCSX2: {}", error))?;

    let install_root = Path::new(&paths.emu).join("PCSX2");
    for name in ["memcards", "sstates"] {
        let source = install_root.join(name);
        let destination = profile_root.join(name);
        if source.exists() {
            copy_directory_recursive(&source, &destination)?;
        } else {
            fs::create_dir_all(&destination)
                .map_err(|error| format!("Impossible de crÃ©er {} pour PCSX2: {}", name, error))?;
        }
    }

    log_sync(
        paths,
        &format!("seeded PCSX2 profile from {}", install_root.to_string_lossy()),
    );
    Ok(())
}

fn ensure_pcsx2_portable_mode(portable_ini_path: &Path) -> Result<(), String> {
    fs::write(portable_ini_path, "")
        .map_err(|error| format!("Impossible de crÃ©er portable.ini pour PCSX2: {}", error))
}

fn materialize_pcsx2_portable_profile(
    paths: &PortablePaths,
    profile_root: &Path,
    working_directory: &Path,
) -> Result<(), String> {
    for path in pcsx2_sync_paths(profile_root) {
        let relative = path.strip_prefix(profile_root).map_err(|error| {
            format!("Chemin PCSX2 invalide dans le profil: {}", error)
        })?;
        let destination = working_directory.join(relative);
        remove_path_if_exists(&destination)?;
        if path.exists() {
            copy_directory_recursive(&path, &destination)?;
        } else {
            fs::create_dir_all(&destination)
                .map_err(|error| format!("Impossible de crÃ©er {} pour PCSX2: {}", destination.to_string_lossy(), error))?;
        }
    }

    log_sync(
        paths,
        &format!(
            "materialized PCSX2 portable profile from {} into {}",
            profile_root.to_string_lossy(),
            working_directory.to_string_lossy()
        ),
    );
    Ok(())
}

fn sync_pcsx2_portable_profile_back(
    paths: &PortablePaths,
    working_directory: &Path,
    profile_root: &Path,
) -> Result<(), String> {
    fs::create_dir_all(profile_root)
        .map_err(|error| format!("Impossible de prÃ©parer le profil PCSX2: {}", error))?;

    for relative in ["memcards", "sstates"] {
        let source = working_directory.join(relative);
        let destination = profile_root.join(relative);
        remove_path_if_exists(&destination)?;
        if source.exists() {
            copy_directory_recursive(&source, &destination)?;
        }
    }

    log_sync(
        paths,
        &format!(
            "synced PCSX2 portable profile back from {} to {}",
            working_directory.to_string_lossy(),
            profile_root.to_string_lossy()
        ),
    );
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    if path.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Impossible de supprimer {}: {}", path.to_string_lossy(), error))?;
    } else {
        fs::remove_file(path)
            .map_err(|error| format!("Impossible de supprimer {}: {}", path.to_string_lossy(), error))?;
    }

    Ok(())
}

fn create_zip_archive(source_dir: &Path, archive_path: &Path) -> Result<(), String> {
    let file = fs::File::create(archive_path)
        .map_err(|error| format!("Impossible de créer l'archive de save: {}", error))?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o755);

    add_directory_to_zip(
        &mut writer,
        source_dir,
        source_dir.parent().unwrap_or(source_dir),
        options,
    )?;

    writer
        .finish()
        .map_err(|error| format!("Impossible de finaliser l'archive de save: {}", error))?;

    Ok(())
}

fn create_zip_archive_from_files(files: &[PathBuf], archive_path: &Path) -> Result<(), String> {
    let file = fs::File::create(archive_path)
        .map_err(|error| format!("Impossible de crÃ©er l'archive de save: {}", error))?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o755);

    for path in files {
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        writer
            .start_file(file_name.replace('\\', "/"), options)
            .map_err(|error| format!("Impossible d'ajouter un fichier Ã  l'archive: {}", error))?;

        let mut file = fs::File::open(path)
            .map_err(|error| format!("Impossible de lire un fichier de save: {}", error))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|error| format!("Impossible de lire un fichier de save: {}", error))?;
        writer
            .write_all(&buffer)
            .map_err(|error| format!("Impossible d'Ã©crire un fichier dans l'archive: {}", error))?;
    }

    writer
        .finish()
        .map_err(|error| format!("Impossible de finaliser l'archive de save: {}", error))?;

    Ok(())
}

fn create_zip_archive_from_paths(
    source_root: &Path,
    paths: &[PathBuf],
    archive_path: &Path,
) -> Result<(), String> {
    let file = fs::File::create(archive_path)
        .map_err(|error| format!("Impossible de crÃ©er l'archive de save: {}", error))?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o755);

    for path in paths {
        if !path.exists() {
            continue;
        }

        add_path_to_zip(&mut writer, path, source_root, options)?;
    }

    writer
        .finish()
        .map_err(|error| format!("Impossible de finaliser l'archive de save: {}", error))?;

    Ok(())
}

fn add_path_to_zip(
    writer: &mut ZipWriter<fs::File>,
    path: &Path,
    base_dir: &Path,
    options: SimpleFileOptions,
) -> Result<(), String> {
    let name = path
        .strip_prefix(base_dir)
        .map_err(|error| format!("Chemin de save invalide: {}", error))?
        .to_string_lossy()
        .replace('\\', "/");

    if should_skip_dolphin_sync_entry(&name) {
        return Ok(());
    }

    if path.is_dir() {
        writer
            .add_directory(format!("{}/", name), options)
            .map_err(|error| format!("Impossible d'ajouter un dossier Ã  l'archive: {}", error))?;

        for entry_result in fs::read_dir(path)
            .map_err(|error| format!("Impossible de lire le dossier de save: {}", error))?
        {
            let entry = entry_result
                .map_err(|error| format!("Impossible de lire une entrÃ©e de save: {}", error))?;
            add_path_to_zip(writer, &entry.path(), base_dir, options)?;
        }

        return Ok(());
    }

    writer
        .start_file(name, options)
        .map_err(|error| format!("Impossible d'ajouter un fichier Ã  l'archive: {}", error))?;

    let mut file = fs::File::open(path)
        .map_err(|error| format!("Impossible de lire un fichier de save: {}", error))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|error| format!("Impossible de lire un fichier de save: {}", error))?;
    writer
        .write_all(&buffer)
        .map_err(|error| format!("Impossible d'Ã©crire un fichier dans l'archive: {}", error))?;

    Ok(())
}

fn add_directory_to_zip(
    writer: &mut ZipWriter<fs::File>,
    current_dir: &Path,
    base_dir: &Path,
    options: SimpleFileOptions,
) -> Result<(), String> {
    for entry_result in fs::read_dir(current_dir)
        .map_err(|error| format!("Impossible de lire le dossier de save: {}", error))?
    {
        let entry = entry_result
            .map_err(|error| format!("Impossible de lire une entrée de save: {}", error))?;
        let path = entry.path();
        let name = path
            .strip_prefix(base_dir)
            .map_err(|error| format!("Chemin de save invalide: {}", error))?
            .to_string_lossy()
            .replace('\\', "/");

        if should_skip_dolphin_sync_entry(&name) {
            continue;
        }

        if path.is_dir() {
            writer
                .add_directory(format!("{}/", name), options)
                .map_err(|error| format!("Impossible d'ajouter un dossier à l'archive: {}", error))?;
            add_directory_to_zip(writer, &path, base_dir, options)?;
        } else {
            writer
                .start_file(name, options)
                .map_err(|error| format!("Impossible d'ajouter un fichier à l'archive: {}", error))?;

            let mut file = fs::File::open(&path)
                .map_err(|error| format!("Impossible de lire un fichier de save: {}", error))?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)
                .map_err(|error| format!("Impossible de lire un fichier de save: {}", error))?;
            writer
                .write_all(&buffer)
                .map_err(|error| format!("Impossible d'écrire un fichier dans l'archive: {}", error))?;
        }
    }

    Ok(())
}

fn extract_zip_archive(archive_path: &Path, destination_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path)
        .map_err(|error| format!("Impossible d'ouvrir l'archive de save: {}", error))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("Archive de save invalide: {}", error))?;

    fs::create_dir_all(destination_dir)
        .map_err(|error| format!("Impossible de préparer le dossier de restauration: {}", error))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Impossible de lire une entrée d'archive: {}", error))?;

        let Some(safe_name) = sanitize_zip_path(entry.name()) else {
            continue;
        };

        let output_path = destination_dir.join(safe_name);

        if entry.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|error| format!("Impossible de créer un dossier restauré: {}", error))?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Impossible de préparer un dossier restauré: {}", error))?;
        }

        let mut output = fs::File::create(&output_path)
            .map_err(|error| format!("Impossible d'écrire un fichier restauré: {}", error))?;
        io::copy(&mut entry, &mut output)
            .map_err(|error| format!("Impossible d'extraire un fichier restauré: {}", error))?;
    }

    Ok(())
}

fn sanitize_zip_path(raw: &str) -> Option<PathBuf> {
    let mut result = PathBuf::new();

    for component in Path::new(raw).components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }

    if result.as_os_str().is_empty() {
        None
    } else {
        Some(result)
    }
}

fn copy_directory_recursive(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("Impossible de créer {}: {}", destination.to_string_lossy(), error))?;

    for entry_result in fs::read_dir(source)
        .map_err(|error| format!("Impossible de lire {}: {}", source.to_string_lossy(), error))?
    {
        let entry = entry_result
            .map_err(|error| format!("Impossible de lire une entrée de dossier: {}", error))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_directory_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(|error| {
                format!(
                    "Impossible de copier {} vers {}: {}",
                    source_path.to_string_lossy(),
                    destination_path.to_string_lossy(),
                    error
                )
            })?;
        }
    }

    Ok(())
}

fn cleanup_dolphin_transient_files(paths: &PortablePaths, profile_user_dir: &Path) -> Result<(), String> {
    let transient_paths = [
        profile_user_dir.join("Wii").join("fst.bin"),
        profile_user_dir.join("Wii").join("fst.bin.tmp"),
        profile_user_dir.join("Wii").join("fst.bin.bak"),
    ];

    for transient_path in transient_paths {
        if transient_path.exists() {
            fs::remove_file(&transient_path).map_err(|error| {
                format!(
                    "Impossible de supprimer le cache Dolphin {}: {}",
                    transient_path.to_string_lossy(),
                    error
                )
            })?;

            log_sync(
                paths,
                &format!(
                    "removed transient dolphin file {}",
                    transient_path.to_string_lossy()
                ),
            );
        }
    }

    Ok(())
}

fn should_skip_dolphin_sync_entry(entry_name: &str) -> bool {
    let normalized = entry_name.replace('\\', "/").to_ascii_lowercase();

    matches!(
        normalized.as_str(),
        "user/wii/fst.bin" | "user/wii/fst.bin.tmp" | "user/wii/fst.bin.bak"
    ) || normalized == "log"
        || normalized.starts_with("log/")
        || normalized == "screenshots"
        || normalized.starts_with("screenshots/")
        || normalized == "shaders"
        || normalized.starts_with("shaders/")
        || normalized == "shader"
        || normalized.starts_with("shader/")
        || normalized == "cache"
        || normalized.starts_with("cache/")
        || normalized == "crash_dumps"
        || normalized.starts_with("crash_dumps/")
        || normalized == "dump"
        || normalized.starts_with("dump/")
        || normalized == "config/qt-config.ini"
        || normalized == "config/window_state.ini"
}

fn load_rom_index(paths: &PortablePaths) -> Result<RommRomIndex, String> {
    let file_path = Path::new(&paths.data).join(ROMM_ROM_INDEX_FILE);

    if !file_path.exists() {
        return Ok(RommRomIndex::default());
    }

    let raw = fs::read_to_string(&file_path)
        .map_err(|error| format!("Impossible de lire l'index RomM: {}", error))?;

    serde_json::from_str(&raw).map_err(|error| format!("Index RomM invalide: {}", error))
}

fn save_rom_index(paths: &PortablePaths, index: &RommRomIndex) -> Result<(), String> {
    let data_dir = Path::new(&paths.data);
    fs::create_dir_all(data_dir)
        .map_err(|error| format!("Impossible de créer le dossier Data: {}", error))?;

    let raw = serde_json::to_string_pretty(index)
        .map_err(|error| format!("Impossible de sérialiser l'index RomM: {}", error))?;

    fs::write(data_dir.join(ROMM_ROM_INDEX_FILE), raw)
        .map_err(|error| format!("Impossible d'écrire l'index RomM: {}", error))
}

fn update_mapping_sync_metadata(
    paths: &PortablePaths,
    rom_path: &str,
    last_synced_local_save_at_ms: Option<u64>,
    last_remote_save_at: Option<String>,
) -> Result<(), String> {
    let mut index = load_rom_index(paths)?;

    if let Some(entry) = index.entries.iter_mut().find(|entry| entry.rom_path == rom_path) {
        if let Some(local_value) = last_synced_local_save_at_ms {
            entry.last_synced_local_save_at_ms = Some(local_value);
        }

        if let Some(remote_value) = last_remote_save_at {
            entry.last_remote_save_at = Some(remote_value);
        }
    }

    save_rom_index(paths, &index)
}

fn sanitize_path_fragment(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' => character.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>();

    normalized
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn sanitize_file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(sanitize_path_fragment)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "game".to_string())
}

fn resolve_remote_game_file_name(game: &RommGameEntry) -> Option<String> {
    resolve_remote_game_file_names(game).into_iter().next()
}

fn normalize_lookup_value(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn resolve_remote_game_file_names(game: &RommGameEntry) -> Vec<String> {
    let mut names = Vec::new();

    for candidate in [&game.file_name, &game.filename, &game.fs_name] {
        if let Some(value) = candidate.as_ref() {
            names.push(value.clone());
        }
    }

    if let Some(files) = &game.files {
        for file in files {
            for candidate in [&file.file_name, &file.filename, &file.fs_name] {
                if let Some(value) = candidate.as_ref() {
                    names.push(value.clone());
                }
            }
        }
    }

    names.sort();
    names.dedup();
    names
}

fn strip_extension(value: &str) -> String {
    match value.rsplit_once('.') {
        Some((stem, _)) => stem.to_string(),
        None => value.to_string(),
    }
}

fn payload_again_from_single_search(
    paths: &PortablePaths,
    session: &RommLaunchSession,
    file_name: &str,
) -> Result<Option<Vec<RommGameEntry>>, String> {
    let stem = strip_extension(file_name);
    let mut url = Url::parse(&format!("{}/api/roms", session.base_url.trim_end_matches('/')))
        .map_err(|error| format!("URL RomM invalide: {}", error))?;
    url.query_pairs_mut().append_pair("search_term", &stem);

    log_sync(paths, &format!("retry rom lookup with stem url={}", url));

    let response = block_on(async {
        let client = build_http_client()?;
        client
            .get(url)
            .bearer_auth(&session.token)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|error| format!("Recherche RomM impossible: {}", error))
    })?;

    let status = response.status();
    let raw = block_on(async {
        response
            .text()
            .await
            .map_err(|error| format!("Réponse RomM illisible pour les roms: {}", error))
    })?;

    log_sync(paths, &format!("retry rom lookup status={} body={}", status, raw));

    if !status.is_success() {
        return Ok(None);
    }

    let payload = serde_json::from_str::<RommGamesResponse>(&raw)
        .map_err(|error| format!("Réponse RomM invalide pour les roms: {}", error))?;

    let games = match payload {
        RommGamesResponse::Direct(entries) => entries,
        RommGamesResponse::Wrapped { items, results } => items.or(results).unwrap_or_default(),
    };

    Ok(Some(games))
}

fn system_time_to_epoch_ms(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

fn compute_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn build_multipart_body(
    boundary: &str,
    field_name: &str,
    file_name: &str,
    mime_type: &str,
    file_bytes: &[u8],
) -> Vec<u8> {
    let mut header = String::new();
    let _ = write!(
        header,
        "--{boundary}\r\nContent-Disposition: form-data; name=\"{field_name}\"; filename=\"{file_name}\"\r\nContent-Type: {mime_type}\r\n\r\n"
    );

    let mut body = Vec::with_capacity(header.len() + file_bytes.len() + boundary.len() + 16);
    body.extend_from_slice(header.as_bytes());
    body.extend_from_slice(file_bytes);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    body
}

fn log_sync(paths: &PortablePaths, message: &str) {
    let logs_dir = Path::new(&paths.data).join("Logs");
    if fs::create_dir_all(&logs_dir).is_err() {
        return;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    let line = format!("[{}] {}\n", timestamp, message);
    let log_path = logs_dir.join("romm-sync.log");

    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = file.write_all(line.as_bytes());
    }
}
