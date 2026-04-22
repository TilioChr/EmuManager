#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config_store;
mod controller_profiles;
mod dolphin_controller_writer;
mod emulator_configurator;
mod emulator_installer;
mod emulator_registry;
mod game_launcher;
mod portable_paths;
mod process_launcher;
mod rom_downloader;

use config_store::{load_config, save_config, AppConfig};
use controller_profiles::{
    load_controller_profiles, save_controller_profiles, ControllerProfile,
};
use dolphin_controller_writer::{apply_controller_profile, ControllerWriteResult};
use emulator_configurator::{configure_emulator, ConfigureResult};
use emulator_installer::{install_emulator, is_emulator_installed, InstallResult};
use emulator_registry::{built_in_emulators, EmulatorDefinition};
use game_launcher::{launch_game, GameLaunchResult};
use portable_paths::{default_root, ensure_portable_tree, PortablePaths};
use process_launcher::{launch_emulator, LaunchResult};
use rom_downloader::{download_rom_to_library, DownloadResult};
use std::path::PathBuf;

#[tauri::command]
fn get_builtin_emulators() -> Vec<EmulatorDefinition> {
    built_in_emulators()
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
fn load_controller_profiles_command(root: Option<String>) -> Result<Vec<ControllerProfile>, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    load_controller_profiles(&paths)
}

#[tauri::command]
fn save_controller_profiles_command(
    root: Option<String>,
    profiles: Vec<ControllerProfile>,
) -> Result<(), String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    save_controller_profiles(&paths, &profiles)
}

#[tauri::command]
fn apply_controller_profile_command(
    root: Option<String>,
    profile: ControllerProfile,
) -> Result<ControllerWriteResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    apply_controller_profile(&paths, &profile)
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
    configure_emulator(&paths, &emulator_id)
}

#[tauri::command]
fn launch_emulator_command(root: Option<String>, emulator_id: String) -> Result<LaunchResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    launch_emulator(&paths, &emulator_id)
}

#[tauri::command]
async fn download_rom_command(
    root: Option<String>,
    url: String,
    file_name: String,
) -> Result<DownloadResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    download_rom_to_library(&paths, &url, &file_name).await
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_builtin_emulators,
            init_portable_layout,
            load_app_config,
            save_app_config,
            load_controller_profiles_command,
            save_controller_profiles_command,
            apply_controller_profile_command,
            check_emulator_installed,
            install_emulator_command,
            configure_emulator_command,
            launch_emulator_command,
            download_rom_command,
            launch_game_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running EmuManager");
}