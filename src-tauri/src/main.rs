#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod azahar_controller_writer;
mod config_store;
mod controller_profile_writer;
mod controller_profiles;
mod debug_log;
mod dolphin_controller_writer;
mod eden_controller_writer;
mod emulator_configurator;
mod emulator_installer;
mod emulator_registry;
mod emulator_resources;
mod game_launcher;
mod graphics_profile_writer;
mod graphics_profiles;
mod local_library;
mod manual_import;
mod melonds_controller_writer;
mod pcsx2_controller_writer;
mod platform_router;
mod portable_paths;
mod process_launcher;
mod rom_downloader;
mod romm_library_cache;
mod romm_media_cache;
mod romm_sync;
mod self_update;

use config_store::{load_config, save_config, AppConfig};
use controller_profile_writer::{apply_controller_profile, ControllerWriteResult};
use controller_profiles::{load_controller_profiles, save_controller_profiles, ControllerProfile};
use debug_log::emit_debug_log;
use emulator_configurator::ConfigureResult;
use emulator_installer::{
    get_installed_emulator_version, install_emulator, is_emulator_installed, uninstall_emulator,
    InstallResult, UninstallResult,
};
use emulator_registry::{built_in_emulators, EmulatorDefinition};
use emulator_resources::{
    ensure_local_resource_configuration, import_local_resource, install_required_resources,
    list_emulator_resource_summaries, pick_resource_source_paths, validate_required_resources,
    EmulatorResourceSummary, ResourceImportRequest, ResourceInstallResult, RommResourceSession,
};
use game_launcher::{launch_game, GameLaunchResult};
use graphics_profile_writer::{apply_graphics_profile, GraphicsWriteResult};
use graphics_profiles::{load_graphics_profiles, save_graphics_profiles, GraphicsProfile};
use local_library::{
    delete_local_rom, list_local_roms, list_local_saves, DeleteLocalRomResult, LocalRomEntry,
    LocalSaveEntry,
};
use manual_import::{
    import_local_rom, manual_import_platforms, ManualImportPlatform, ManualImportRequest,
    ManualImportResult,
};
use portable_paths::{default_root, ensure_portable_tree, PortablePaths};
use process_launcher::{launch_emulator, LaunchResult};
use rom_downloader::{download_rom_to_library, DownloadResult, DownloadRomRequest};
use romm_library_cache::{cache_romm_game_metadata, load_romm_game_metadata};
use romm_media_cache::{
    cache_romm_media, read_romm_cached_media, RommCachedMediaRequest, RommMediaCacheRequest,
    RommMediaCacheResult,
};
use romm_sync::{
    get_save_conflict_status, get_save_sync_statuses, register_rom_mapping, RommLaunchSession,
    SaveConflictStatus, SaveSyncStatus,
};
use self_update::{
    apply_update, check_for_update, current_version, download_update, AppUpdateDownloadRequest,
    AppUpdateDownloadResult, AppUpdateStatus,
};
use serde::Serialize;
use serde_json::Value;
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphicsProfileSaveResult {
    profiles: Vec<GraphicsProfile>,
    write_result: Option<GraphicsWriteResult>,
    warning: Option<String>,
}

#[tauri::command]
fn get_builtin_emulators() -> Vec<EmulatorDefinition> {
    built_in_emulators()
}

#[tauri::command]
fn get_app_version_command() -> String {
    current_version().to_string()
}

#[tauri::command]
async fn cache_romm_media_command(
    root: Option<String>,
    request: RommMediaCacheRequest,
) -> Result<RommMediaCacheResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    cache_romm_media(&paths, &request).await
}

#[tauri::command]
fn read_romm_cached_media_command(
    root: Option<String>,
    request: RommCachedMediaRequest,
) -> Result<Option<RommMediaCacheResult>, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    read_romm_cached_media(&paths, &request)
}

#[tauri::command]
fn cache_romm_game_metadata_command(root: Option<String>, game: Value) -> Result<(), String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    cache_romm_game_metadata(&paths, &game)
}

#[tauri::command]
fn load_romm_game_metadata_command(
    root: Option<String>,
    romm_ids: Vec<String>,
) -> Result<Vec<Value>, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    load_romm_game_metadata(&paths, &romm_ids)
}

#[tauri::command]
async fn check_app_update_command(app: tauri::AppHandle) -> Result<AppUpdateStatus, String> {
    check_for_update(&app).await
}

#[tauri::command]
async fn download_app_update_command(
    app: tauri::AppHandle,
    root: Option<String>,
    request: AppUpdateDownloadRequest,
) -> Result<AppUpdateDownloadResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    download_update(&app, &paths, &request).await
}

#[tauri::command]
fn apply_app_update_command(app: tauri::AppHandle, file_path: String) -> Result<(), String> {
    apply_update(&app, &file_path)
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
        profiles.remove(index);
    }
    profiles.push(profile.clone());

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
fn load_graphics_profiles_command(root: Option<String>) -> Result<Vec<GraphicsProfile>, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    load_graphics_profiles(&paths)
}

#[tauri::command]
fn save_graphics_profile_command(
    root: Option<String>,
    profile: GraphicsProfile,
) -> Result<GraphicsProfileSaveResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    let mut profiles = load_graphics_profiles(&paths)?;

    if let Some(index) = profiles
        .iter()
        .position(|entry| is_same_graphics_profile(entry, &profile))
    {
        profiles.remove(index);
    }
    profiles.push(profile.clone());

    save_graphics_profiles(&paths, &profiles)?;

    match apply_graphics_profile(&paths, &profile) {
        Ok(write_result) => Ok(GraphicsProfileSaveResult {
            profiles,
            write_result: Some(write_result),
            warning: None,
        }),
        Err(error) => Ok(GraphicsProfileSaveResult {
            profiles,
            write_result: None,
            warning: Some(format!(
                "Profil graphique sauvegarde, mais non applique: {}",
                error
            )),
        }),
    }
}

fn is_same_graphics_profile(existing: &GraphicsProfile, incoming: &GraphicsProfile) -> bool {
    existing.id == incoming.id || existing.emulator_id == incoming.emulator_id
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
fn delete_local_rom_command(
    root: Option<String>,
    rom_path: String,
) -> Result<DeleteLocalRomResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    delete_local_rom(&paths, &rom_path)
}

#[tauri::command]
fn get_manual_import_platforms_command() -> Vec<ManualImportPlatform> {
    manual_import_platforms()
}

#[tauri::command]
fn import_local_rom_command(
    root: Option<String>,
    request: ManualImportRequest,
) -> Result<ManualImportResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    import_local_rom(&paths, &request)
}

#[tauri::command]
fn check_emulator_installed(root: Option<String>, emulator_id: String) -> Result<bool, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    Ok(is_emulator_installed(&paths, &emulator_id))
}

#[tauri::command]
fn get_emulator_resource_statuses_command(
    root: Option<String>,
) -> Result<Vec<EmulatorResourceSummary>, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    Ok(list_emulator_resource_summaries(&paths))
}

#[tauri::command]
fn pick_emulator_resource_files_command(
    emulator_id: String,
    resource_id: String,
) -> Result<Option<Vec<String>>, String> {
    pick_resource_source_paths(&emulator_id, &resource_id)
}

#[tauri::command]
fn import_emulator_resource_command(
    root: Option<String>,
    request: ResourceImportRequest,
) -> Result<ResourceInstallResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    if !is_emulator_installed(&paths, &request.emulator_id) {
        let emulator_name = built_in_emulators()
            .into_iter()
            .find(|entry| entry.id == request.emulator_id)
            .map(|entry| entry.name.to_string())
            .unwrap_or_else(|| request.emulator_id.clone());
        return Err(format!(
            "{} must be installed before importing system files.",
            emulator_name
        ));
    }

    import_local_resource(&paths, &request)
}

#[tauri::command]
async fn install_emulator_resources_command(
    app: tauri::AppHandle,
    root: Option<String>,
    emulator_id: String,
    romm_session: RommResourceSession,
) -> Result<ResourceInstallResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    install_required_resources(&app, &paths, &emulator_id, &romm_session).await
}

#[tauri::command]
fn resolve_emulator_id_for_rom_command(
    root: Option<String>,
    rom_path: String,
) -> Result<String, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    platform_router::resolve_emulator_id_for_rom_path(&paths, &rom_path)
}

#[tauri::command]
async fn install_emulator_command(
    app: tauri::AppHandle,
    root: Option<String>,
    emulator_id: String,
) -> Result<InstallResult, String> {
    emit_debug_log(
        &app,
        "info",
        "emulator-install",
        &format!("Backend install command started for {}", emulator_id),
        None,
    );
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    let result = install_emulator(&app, &paths, &emulator_id).await;
    match &result {
        Ok(install) => emit_debug_log(
            &app,
            "success",
            "emulator-install",
            &format!("Backend install command completed for {}", emulator_id),
            Some(format!(
                "install_path={}\nexecutable_path={}\narchive_path={}",
                install.install_path, install.executable_path, install.archive_path
            )),
        ),
        Err(error) => emit_debug_log(
            &app,
            "error",
            "emulator-install",
            &format!("Backend install command failed for {}", emulator_id),
            Some(error.clone()),
        ),
    }
    result
}

#[tauri::command]
fn uninstall_emulator_command(
    app: tauri::AppHandle,
    root: Option<String>,
    emulator_id: String,
) -> Result<UninstallResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    let result = uninstall_emulator(&paths, &emulator_id);

    match &result {
        Ok(uninstall) => emit_debug_log(
            &app,
            "success",
            "emulator-uninstall",
            &format!("Backend uninstalled emulator {}", emulator_id),
            Some(format!(
                "install_path={}\nremoved={}",
                uninstall.install_path, uninstall.removed
            )),
        ),
        Err(error) => emit_debug_log(
            &app,
            "error",
            "emulator-uninstall",
            &format!("Backend uninstall failed for {}", emulator_id),
            Some(error.clone()),
        ),
    }

    result
}

#[tauri::command]
fn configure_emulator_command(
    app: tauri::AppHandle,
    root: Option<String>,
    emulator_id: String,
) -> Result<ConfigureResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    let result = emulator_configurator::configure_emulator(&paths, &emulator_id);
    match &result {
        Ok(configure) => emit_debug_log(
            &app,
            "success",
            "emulator-config",
            &format!("Backend configuration completed for {}", emulator_id),
            Some(format!(
                "user_directory={}\nconfig_directory={}",
                configure.user_directory, configure.config_directory
            )),
        ),
        Err(error) => emit_debug_log(
            &app,
            "error",
            "emulator-config",
            &format!("Backend configuration failed for {}", emulator_id),
            Some(error.clone()),
        ),
    }
    result
}

#[tauri::command]
async fn launch_emulator_command(
    app: tauri::AppHandle,
    root: Option<String>,
    emulator_id: String,
) -> Result<LaunchResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    let _ = ensure_local_resource_configuration(&paths, &emulator_id);
    let emulator_id_for_launch = emulator_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        launch_emulator(&paths, &emulator_id_for_launch)
    })
    .await
    .map_err(|error| format!("Lancement de l'emulateur interrompu: {}", error))?;

    match &result {
        Ok(launch) => emit_debug_log(
            &app,
            "success",
            "emulator-launch",
            &format!("Backend launched emulator {}", emulator_id),
            Some(format!(
                "executable_path={}\nworking_directory={}",
                launch.executable_path, launch.working_directory
            )),
        ),
        Err(error) => emit_debug_log(
            &app,
            "error",
            "emulator-launch",
            &format!("Backend emulator launch failed for {}", emulator_id),
            Some(error.clone()),
        ),
    }
    result
}

#[tauri::command]
async fn download_rom_command(
    app: tauri::AppHandle,
    root: Option<String>,
    request: DownloadRomRequest,
) -> Result<DownloadResult, String> {
    emit_debug_log(
        &app,
        "info",
        "rom-download",
        &format!("Backend ROM download started for {}", request.file_name),
        Some(format!(
            "download_id={}\nrelative_subdir={:?}\nexpected_total_bytes={:?}",
            request.download_id, request.relative_subdir, request.expected_total_bytes
        )),
    );
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    let result = download_rom_to_library(&app, &paths, &request).await;
    match &result {
        Ok(download) => emit_debug_log(
            &app,
            "success",
            "rom-download",
            &format!("Backend ROM download completed for {}", request.file_name),
            Some(format!(
                "file_path={}\nbytes_written={}",
                download.file_path, download.bytes_written
            )),
        ),
        Err(error) => emit_debug_log(
            &app,
            "error",
            "rom-download",
            &format!("Backend ROM download failed for {}", request.file_name),
            Some(error.clone()),
        ),
    }
    result
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
fn get_save_conflict_status_command(
    root: Option<String>,
    rom_path: String,
    romm_session: RommLaunchSession,
) -> Result<Option<SaveConflictStatus>, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    get_save_conflict_status(&paths, &romm_session, &rom_path)
}

#[tauri::command]
async fn launch_game_command(
    app: tauri::AppHandle,
    root: Option<String>,
    emulator_id: String,
    rom_path: String,
    romm_session: Option<RommLaunchSession>,
) -> Result<GameLaunchResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    ensure_local_resource_configuration(&paths, &emulator_id)?;

    if let Some(session) = romm_session.as_ref() {
        let resource_session = RommResourceSession {
            base_url: session.base_url.clone(),
            token: session.token.clone(),
        };
        install_required_resources(&app, &paths, &emulator_id, &resource_session).await?;
    } else {
        validate_required_resources(&paths, &emulator_id)?;
    }

    let emulator_id_for_launch = emulator_id.clone();
    let rom_path_for_launch = rom_path.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        launch_game(
            &paths,
            &emulator_id_for_launch,
            &rom_path_for_launch,
            romm_session.as_ref(),
        )
    })
    .await
    .map_err(|error| format!("Lancement du jeu interrompu: {}", error))?;

    log_game_launch_result(&app, &result, &emulator_id, &rom_path);
    result
}

#[tauri::command]
async fn launch_game_auto_command(
    app: tauri::AppHandle,
    root: Option<String>,
    rom_path: String,
    romm_session: Option<RommLaunchSession>,
) -> Result<GameLaunchResult, String> {
    let root_path = root.map(PathBuf::from).unwrap_or_else(default_root);
    let paths = ensure_portable_tree(&root_path)?;
    let emulator_id_for_launch =
        platform_router::resolve_emulator_id_for_rom_path(&paths, &rom_path)?;
    ensure_local_resource_configuration(&paths, &emulator_id_for_launch)?;

    if let Some(session) = romm_session.as_ref() {
        let resource_session = RommResourceSession {
            base_url: session.base_url.clone(),
            token: session.token.clone(),
        };
        install_required_resources(&app, &paths, &emulator_id_for_launch, &resource_session)
            .await?;
    } else {
        validate_required_resources(&paths, &emulator_id_for_launch)?;
    }

    let rom_path_for_launch = rom_path.clone();
    let emulator_id_for_result = emulator_id_for_launch.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        launch_game(
            &paths,
            &emulator_id_for_launch,
            &rom_path_for_launch,
            romm_session.as_ref(),
        )
    })
    .await
    .map_err(|error| format!("Lancement du jeu interrompu: {}", error))?;

    let emulator_id = result
        .as_ref()
        .map(|launch| launch.emulator_id.as_str())
        .unwrap_or(emulator_id_for_result.as_str());
    log_game_launch_result(&app, &result, emulator_id, &rom_path);
    result
}

fn log_game_launch_result(
    app: &tauri::AppHandle,
    result: &Result<GameLaunchResult, String>,
    emulator_id: &str,
    rom_path: &str,
) {
    match result {
        Ok(launch) => emit_debug_log(
            app,
            "success",
            "game-launch",
            &format!("Backend launched ROM with {}", launch.emulator_id),
            Some(format!(
                "rom_path={}\nexecutable_path={}",
                launch.rom_path, launch.executable_path
            )),
        ),
        Err(error) => emit_debug_log(
            app,
            "error",
            "game-launch",
            &format!("Backend ROM launch failed with {}", emulator_id),
            Some(format!("rom_path={}\nerror={}", rom_path, error)),
        ),
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_builtin_emulators,
            get_app_version_command,
            cache_romm_media_command,
            read_romm_cached_media_command,
            cache_romm_game_metadata_command,
            load_romm_game_metadata_command,
            check_app_update_command,
            download_app_update_command,
            apply_app_update_command,
            get_installed_emulator_versions,
            init_portable_layout,
            load_app_config,
            save_app_config,
            load_controller_profiles_command,
            save_controller_profile_command,
            load_graphics_profiles_command,
            save_graphics_profile_command,
            list_local_roms_command,
            list_local_saves_command,
            delete_local_rom_command,
            check_emulator_installed,
            get_emulator_resource_statuses_command,
            pick_emulator_resource_files_command,
            import_emulator_resource_command,
            install_emulator_resources_command,
            resolve_emulator_id_for_rom_command,
            install_emulator_command,
            uninstall_emulator_command,
            configure_emulator_command,
            launch_emulator_command,
            download_rom_command,
            register_romm_rom_command,
            get_save_sync_statuses_command,
            get_save_conflict_status_command,
            get_manual_import_platforms_command,
            import_local_rom_command,
            launch_game_command,
            launch_game_auto_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running EmuManager");
}
