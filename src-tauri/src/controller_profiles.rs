use crate::portable_paths::PortablePaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const CONTROLLER_FILE_NAME: &str = "controller_profiles.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerBinding {
    pub physical_input: String,
    pub emulated_input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerProfile {
    pub id: String,
    pub name: String,
    pub emulator_id: String,
    pub platform_label: String,
    #[serde(default)]
    pub physical_device_id: Option<String>,
    pub physical_device_label: String,
    #[serde(default)]
    pub emulated_controller_id: Option<String>,
    pub emulated_device_label: String,
    #[serde(default)]
    pub dolphin_settings: Option<ControllerDolphinSettings>,
    pub bindings: Vec<ControllerBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerDolphinSettings {
    pub ir_auto_hide: bool,
    pub ir_relative_input: bool,
}

pub fn load_controller_profiles(paths: &PortablePaths) -> Result<Vec<ControllerProfile>, String> {
    let file_path = profiles_file_path(paths);

    if !file_path.exists() {
        return Ok(vec![]);
    }

    let raw = fs::read_to_string(&file_path)
        .map_err(|error| format!("Impossible de lire les profils manette: {}", error))?;

    serde_json::from_str::<Vec<ControllerProfile>>(&raw)
        .map_err(|error| format!("Profils manette invalides: {}", error))
}

pub fn save_controller_profiles(
    paths: &PortablePaths,
    profiles: &[ControllerProfile],
) -> Result<(), String> {
    let config_dir = Path::new(&paths.config);
    fs::create_dir_all(config_dir)
        .map_err(|error| format!("Impossible de créer le dossier config: {}", error))?;

    let raw = serde_json::to_string_pretty(profiles)
        .map_err(|error| format!("Impossible de sérialiser les profils manette: {}", error))?;

    fs::write(profiles_file_path(paths), raw)
        .map_err(|error| format!("Impossible d'écrire les profils manette: {}", error))
}

fn profiles_file_path(paths: &PortablePaths) -> std::path::PathBuf {
    Path::new(&paths.config).join(CONTROLLER_FILE_NAME)
}
