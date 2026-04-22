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

use config_store::{load_config, save_config, AppConfig};
use dolphin_controller_writer::ControllerWriteResult;
use emulator_configurator::ConfigureResult;
use emulator_installer::{
    get_installed_emulator_version, install_emulator, is_emulator_installed, InstallResult,
};
use emulator_registry::{built_in_emulators, EmulatorDefinition};
use game_launcher::{launch_game, launch_game_auto, GameLaunchResult};
use local_library::{list_local_roms, list_local_saves, LocalRomEntry, LocalSaveEntry};
use portable_paths::{default_root, ensure_portable_tree, PortablePaths};
use process_launcher::{launch_emulator, LaunchResult};
use rom_downloader::{download_rom_to_library, DownloadResult};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstalledVersionMap {
    versions: HashMap<String, String>,
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
fn configure_emulator_command(root: Option<String>, emulator_id: String) -> Result<ConfigureResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    emulator_configurator::configure_emulator(&paths, &emulator_id)
}

#[tauri::command]
fn launch_emulator_command(root: Option<String>, emulator_id: String) -> Result<LaunchResult, String> {
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
fn launch_game_command(
    root: Option<String>,
    emulator_id: String,
    rom_path: String,
) -> Result<GameLaunchResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    launch_game(&paths, &emulator_id, &rom_path)
}

#[tauri::command]
fn launch_game_auto_command(
    root: Option<String>,
    rom_path: String,
) -> Result<GameLaunchResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    launch_game_auto(&paths, &rom_path)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_builtin_emulators,
            get_installed_emulator_versions,
            init_portable_layout,
            load_app_config,
            save_app_config,
            list_local_roms_command,
            list_local_saves_command,
            check_emulator_installed,
            install_emulator_command,
            configure_emulator_command,
            launch_emulator_command,
            download_rom_command,
            launch_game_command,
            launch_game_auto_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running EmuManager");
}