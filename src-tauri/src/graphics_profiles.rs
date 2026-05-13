use crate::portable_paths::PortablePaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const GRAPHICS_FILE_NAME: &str = "graphics_profiles.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsProfile {
    pub id: String,
    pub emulator_id: String,
    pub platform_label: String,
    pub mode: String,
    pub preset: String,
    pub resolution_scale: u32,
    pub graphics_api: String,
    pub fullscreen: bool,
    pub vsync: bool,
    pub aspect_ratio: String,
    pub anti_aliasing: String,
    pub anisotropic_filtering: String,
    pub texture_filtering: String,
    pub shader_cache: bool,
    pub widescreen_hack: bool,
    pub integer_scaling: bool,
}

pub fn load_graphics_profiles(paths: &PortablePaths) -> Result<Vec<GraphicsProfile>, String> {
    let file_path = profiles_file_path(paths);

    if !file_path.exists() {
        return Ok(vec![]);
    }

    let raw = fs::read_to_string(&file_path)
        .map_err(|error| format!("Impossible de lire les profils graphiques: {}", error))?;

    serde_json::from_str::<Vec<GraphicsProfile>>(&raw)
        .map_err(|error| format!("Profils graphiques invalides: {}", error))
}

pub fn save_graphics_profiles(
    paths: &PortablePaths,
    profiles: &[GraphicsProfile],
) -> Result<(), String> {
    let config_dir = Path::new(&paths.config);
    fs::create_dir_all(config_dir)
        .map_err(|error| format!("Impossible de creer le dossier config: {}", error))?;

    let raw = serde_json::to_string_pretty(profiles)
        .map_err(|error| format!("Impossible de serialiser les profils graphiques: {}", error))?;

    fs::write(profiles_file_path(paths), raw)
        .map_err(|error| format!("Impossible d'ecrire les profils graphiques: {}", error))
}

fn profiles_file_path(paths: &PortablePaths) -> std::path::PathBuf {
    Path::new(&paths.config).join(GRAPHICS_FILE_NAME)
}
