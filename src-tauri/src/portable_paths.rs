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

impl PortablePaths {
    pub fn from_root(root: PathBuf) -> Self {
        let emu = root.join("Emu");
        let roms = root.join("Roms");
        let saves = root.join("Saves");
        let firmware = root.join("Firmware");
        let config = root.join("Config");
        let data = root.join("Data");

        Self {
            root: display_path(&root),
            emu: display_path(&emu),
            roms: display_path(&roms),
            saves: display_path(&saves),
            firmware: display_path(&firmware),
            config: display_path(&config),
            data: display_path(&data),
        }
    }
}

pub fn default_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("EmuManager")
}

pub fn ensure_portable_tree(root: &Path) -> Result<PortablePaths, String> {
    let paths = PortablePaths::from_root(root.to_path_buf());

    for dir in [
        root.to_path_buf(),
        root.join("Emu"),
        root.join("Roms"),
        root.join("Saves"),
        root.join("Firmware"),
        root.join("Config"),
        root.join("Data"),
    ] {
        fs::create_dir_all(&dir)
            .map_err(|error| format!("Impossible de créer {}: {}", display_path(&dir), error))?;
    }

    Ok(paths)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}