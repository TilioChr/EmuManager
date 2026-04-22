use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortablePaths {
    pub root: String,
    pub emu: String,
    pub roms: String,
    pub saves: String,
    pub firmware: String,
    pub config: String,
    pub data: String,
}

pub fn default_root() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.to_path_buf()));

    exe_dir.unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    })
}

pub fn ensure_portable_tree(root: &Path) -> Result<PortablePaths, String> {
    let emu = root.join("Emu");
    let roms = root.join("Roms");
    let saves = root.join("Saves");
    let firmware = root.join("Firmware");
    let config = root.join("Config");
    let data = root.join("Data");

    for path in [&emu, &roms, &saves, &firmware, &config, &data] {
        fs::create_dir_all(path)
            .map_err(|error| format!("Impossible de créer {}: {}", path.to_string_lossy(), error))?;
    }

    Ok(PortablePaths {
        root: root.to_string_lossy().to_string(),
        emu: emu.to_string_lossy().to_string(),
        roms: roms.to_string_lossy().to_string(),
        saves: saves.to_string_lossy().to_string(),
        firmware: firmware.to_string_lossy().to_string(),
        config: config.to_string_lossy().to_string(),
        data: data.to_string_lossy().to_string(),
    })
}