#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config_store;
mod controller_profiles;
mod dolphin_controller_writer;
mod emulator_configurator;
mod emulator_installer;
mod emulator_registry;
mod game_launcher;
mod local_library;
mod platform_router;
mod portable_paths;
mod process_launcher;
mod rom_downloader;
mod romm_sync;

use config_store::{load_config, save_config, AppConfig};
use controller_profiles::{load_controller_profiles, save_controller_profiles, ControllerProfile};
use dolphin_controller_writer::{apply_controller_profile, ControllerWriteResult};
use emulator_configurator::ConfigureResult;
use emulator_installer::{
    get_installed_emulator_version, install_emulator, is_emulator_installed, InstallResult,
};
use emulator_registry::{built_in_emulators, EmulatorDefinition};
use game_launcher::{launch_game, launch_game_auto_with_session, GameLaunchResult};
use local_library::{list_local_roms, list_local_saves, LocalRomEntry, LocalSaveEntry};
use portable_paths::{default_root, ensure_portable_tree, PortablePaths};
use process_launcher::{launch_emulator, LaunchResult};
use rom_downloader::{download_rom_to_library, DownloadResult};
use romm_sync::{get_save_sync_statuses, register_rom_mapping, RommLaunchSession, SaveSyncStatus};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstalledVersionMap {
    versions: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ControllerProfileSaveResult {
    profiles: Vec<ControllerProfile>,
    write_result: Option<ControllerWriteResult>,
    warning: Option<String>,
}

#[tauri::command]
fn get_builtin_emulators() -> Vec<EmulatorDefinition> {
    built_in_emulators()
}

#[tauri::command]
fn get_installed_emulator_versions(root: Option<String>) -> Result<InstalledVersionMap, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;

    let mut versions = HashMap::new();

    for emulator in built_in_emulators() {
        if let Ok(Some(version)) = get_installed_emulator_version(&paths, emulator.id) {
            versions.insert(emulator.id.to_string(), version);
        }
    }

    Ok(InstalledVersionMap { versions })
}

#[tauri::command]
fn init_portable_layout(root: Option<String>) -> Result<PortablePaths, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    ensure_portable_tree(&root_path)
}

#[tauri::command]
fn load_app_config(root: Option<String>) -> Result<AppConfig, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    load_config(&paths)
}

#[tauri::command]
fn save_app_config(root: Option<String>, config: AppConfig) -> Result<(), String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    save_config(&paths, &config)
}

#[tauri::command]
fn load_controller_profiles_command(
    root: Option<String>,
) -> Result<Vec<ControllerProfile>, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    load_controller_profiles(&paths)
}

#[tauri::command]
fn save_controller_profile_command(
    root: Option<String>,
    profile: ControllerProfile,
) -> Result<ControllerProfileSaveResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    let mut profiles = load_controller_profiles(&paths)?;

    if let Some(index) = profiles
        .iter()
        .position(|entry| is_same_controller_profile(entry, &profile))
    {
        profiles[index] = profile.clone();
    } else {
        profiles.push(profile.clone());
    }

    save_controller_profiles(&paths, &profiles)?;

    match apply_controller_profile(&paths, &profile) {
        Ok(write_result) => Ok(ControllerProfileSaveResult {
            profiles,
            write_result: Some(write_result),
            warning: None,
        }),
        Err(error) => Ok(ControllerProfileSaveResult {
            profiles,
            write_result: None,
            warning: Some(format!("Profil sauvegardé, mais non appliqué: {}", error)),
        }),
    }
}

fn is_same_controller_profile(existing: &ControllerProfile, incoming: &ControllerProfile) -> bool {
    if existing.id == incoming.id {
        return true;
    }

    existing.emulator_id == incoming.emulator_id
        && existing.emulated_controller_id == incoming.emulated_controller_id
        && existing.physical_device_id == incoming.physical_device_id
}

#[tauri::command]
fn list_local_roms_command(root: Option<String>) -> Result<Vec<LocalRomEntry>, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    list_local_roms(&paths)
}

#[tauri::command]
fn list_local_saves_command(root: Option<String>) -> Result<Vec<LocalSaveEntry>, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    list_local_saves(&paths)
}

#[tauri::command]
fn check_emulator_installed(root: Option<String>, emulator_id: String) -> Result<bool, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    Ok(is_emulator_installed(&paths, &emulator_id))
}

#[tauri::command]
async fn install_emulator_command(
    root: Option<String>,
    emulator_id: String,
) -> Result<InstallResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    install_emulator(&paths, &emulator_id).await
}

#[tauri::command]
fn configure_emulator_command(
    root: Option<String>,
    emulator_id: String,
) -> Result<ConfigureResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    emulator_configurator::configure_emulator(&paths, &emulator_id)
}

#[tauri::command]
fn launch_emulator_command(
    root: Option<String>,
    emulator_id: String,
) -> Result<LaunchResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    launch_emulator(&paths, &emulator_id)
}

#[tauri::command]
async fn download_rom_command(
    app: tauri::AppHandle,
    root: Option<String>,
    url: String,
    file_name: String,
    bearer_token: Option<String>,
    download_id: String,
    expected_total_bytes: Option<u64>,
    relative_subdir: Option<String>,
) -> Result<DownloadResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    download_rom_to_library(
        &app,
        &paths,
        &url,
        &file_name,
        bearer_token.as_deref(),
        &download_id,
        expected_total_bytes,
        relative_subdir.as_deref(),
    )
    .await
}

#[tauri::command]
fn register_romm_rom_command(
    root: Option<String>,
    rom_path: String,
    romm_id: String,
    platform_name: Option<String>,
    file_name: Option<String>,
) -> Result<(), String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    register_rom_mapping(
        &paths,
        &rom_path,
        &romm_id,
        platform_name.as_deref(),
        file_name.as_deref(),
    )
}

#[tauri::command]
fn get_save_sync_statuses_command(
    root: Option<String>,
    rom_paths: Vec<String>,
) -> Result<Vec<SaveSyncStatus>, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    get_save_sync_statuses(&paths, &rom_paths)
}

#[tauri::command]
fn launch_game_command(
    root: Option<String>,
    emulator_id: String,
    rom_path: String,
    romm_session: Option<RommLaunchSession>,
) -> Result<GameLaunchResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    launch_game(&paths, &emulator_id, &rom_path, romm_session.as_ref())
}

#[tauri::command]
fn launch_game_auto_command(
    root: Option<String>,
    rom_path: String,
    romm_session: Option<RommLaunchSession>,
) -> Result<GameLaunchResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    launch_game_auto_with_session(&paths, &rom_path, romm_session.as_ref())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_builtin_emulators,
            get_installed_emulator_versions,
            init_portable_layout,
            load_app_config,
            save_app_config,
            load_controller_profiles_command,
            save_controller_profile_command,
            list_local_roms_command,
            list_local_saves_command,
            check_emulator_installed,
            install_emulator_command,
            configure_emulator_command,
            launch_emulator_command,
            download_rom_command,
            register_romm_rom_command,
            get_save_sync_statuses_command,
            launch_game_command,
            launch_game_auto_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running EmuManager");
}
