use crate::portable_paths::PortablePaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const CONFIG_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RommConnectionConfig {
    pub base_url: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub romm: Option<RommConnectionConfig>,
    pub installed_emulators: Vec<String>,
}

pub fn load_config(paths: &PortablePaths) -> Result<AppConfig, String> {
    let config_path = config_file_path(paths);

    if !config_path.exists() {
        return Ok(AppConfig::default());
    }

    let raw = fs::read_to_string(&config_path)
        .map_err(|error| format!("Impossible de lire la configuration: {}", error))?;

    serde_json::from_str::<AppConfig>(&raw)
        .map_err(|error| format!("Configuration invalide: {}", error))
}

pub fn save_config(paths: &PortablePaths, config: &AppConfig) -> Result<(), String> {
    let config_dir = Path::new(&paths.config);
    fs::create_dir_all(config_dir)
        .map_err(|error| format!("Impossible de créer le dossier config: {}", error))?;

    let raw = serde_json::to_string_pretty(config)
        .map_err(|error| format!("Impossible de sérialiser la configuration: {}", error))?;

    fs::write(config_file_path(paths), raw)
        .map_err(|error| format!("Impossible d'écrire la configuration: {}", error))
}

fn config_file_path(paths: &PortablePaths) -> std::path::PathBuf {
    Path::new(&paths.config).join(CONFIG_FILE_NAME)
}