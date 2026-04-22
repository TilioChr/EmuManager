use crate::portable_paths::PortablePaths;
use std::path::{Path, PathBuf};

pub fn resolve_emulator_id_for_rom_path(
    paths: &PortablePaths,
    rom_path: &str,
) -> Result<String, String> {
    let roms_root = PathBuf::from(&paths.roms);
    let rom = PathBuf::from(rom_path);

    let relative = rom
        .strip_prefix(&roms_root)
        .map_err(|_| {
            format!(
                "La ROM n'est pas située dans le dossier Roms attendu: {}",
                rom.to_string_lossy()
            )
        })?;

    let first_component = relative
        .iter()
        .next()
        .ok_or_else(|| "Impossible de déterminer la plateforme depuis le chemin ROM.".to_string())?
        .to_string_lossy()
        .to_ascii_lowercase();

    if let Some(emulator_id) = map_folder_to_emulator(&first_component) {
        return Ok(emulator_id.to_string());
    }

    let fallback = fallback_emulator_from_extension(&rom);
    if let Some(emulator_id) = fallback {
        return Ok(emulator_id.to_string());
    }

    Err(format!(
        "Aucun émulateur associé au dossier \"{}\" pour la ROM {}",
        first_component,
        rom.to_string_lossy()
    ))
}

fn map_folder_to_emulator(folder: &str) -> Option<&'static str> {
    match folder {
        "wii" => Some("dolphin"),
        "gamecube" | "gc" | "gamecube-wii" | "wii-gamecube" => Some("dolphin"),
        "nds" | "ds" => Some("melonds"),
        "3ds" => Some("azahar"),
        "switch" | "nsw" => Some("eden"),
        "ps2" => Some("pcsx2"),
        "psp" => Some("ppsspp"),
        "ps1" | "psx" | "playstation" => Some("duckstation"),
        _ => None,
    }
}

fn fallback_emulator_from_extension(path: &Path) -> Option<&'static str> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "nds" => Some("melonds"),
        "3ds" | "cci" | "cia" | "3dsx" => Some("azahar"),
        "xci" | "nsp" | "nro" => Some("eden"),
        "pbp" | "cso" => Some("ppsspp"),
        "cue" | "bin" | "img" | "chd" => Some("duckstation"),
        _ => None,
    }
}