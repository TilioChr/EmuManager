use crate::portable_paths::PortablePaths;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalRomEntry {
    pub file_name: String,
    pub file_path: String,
    pub file_size_bytes: u64,
    pub platform_guess: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSaveEntry {
    pub file_name: String,
    pub file_path: String,
    pub file_size_bytes: u64,
    pub platform_guess: String,
}

pub fn list_local_roms(paths: &PortablePaths) -> Result<Vec<LocalRomEntry>, String> {
    let roms_dir = PathBuf::from(&paths.roms);
    if !roms_dir.exists() {
        return Ok(vec![]);
    }

    let mut results = Vec::new();
    collect_roms(&roms_dir, &roms_dir, &mut results)?;

    results.sort_by(|left, right| {
        left.file_name
            .to_lowercase()
            .cmp(&right.file_name.to_lowercase())
    });

    Ok(results)
}

pub fn list_local_saves(paths: &PortablePaths) -> Result<Vec<LocalSaveEntry>, String> {
    let roms_dir = PathBuf::from(&paths.roms);
    let saves_dir = PathBuf::from(&paths.saves);

    let mut results = Vec::new();

    if roms_dir.exists() {
        collect_saves(&roms_dir, &roms_dir, &mut results)?;
    }

    if saves_dir.exists() {
        collect_saves(&saves_dir, &saves_dir, &mut results)?;
    }

    results.sort_by(|left, right| {
        left.file_name
            .to_lowercase()
            .cmp(&right.file_name.to_lowercase())
    });

    Ok(results)
}

fn collect_roms(root: &Path, dir: &Path, output: &mut Vec<LocalRomEntry>) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|error| format!("Impossible de lire le dossier Roms: {}", error))?;

    for entry_result in entries {
        let entry = entry_result
            .map_err(|error| format!("Impossible de lire une entrée Roms: {}", error))?;
        let path = entry.path();

        if path.is_dir() {
            collect_roms(root, &path, output)?;
            continue;
        }

        if !is_rom_file(&path) {
            continue;
        }

        let metadata = fs::metadata(&path)
            .map_err(|error| format!("Impossible de lire les métadonnées ROM: {}", error))?;

        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        output.push(LocalRomEntry {
            file_name: file_name.clone(),
            file_path: path.to_string_lossy().to_string(),
            file_size_bytes: metadata.len(),
            platform_guess: guess_rom_platform(root, &path, &file_name),
        });
    }

    Ok(())
}

fn collect_saves(root: &Path, dir: &Path, output: &mut Vec<LocalSaveEntry>) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|error| format!("Impossible de lire le dossier de saves: {}", error))?;

    for entry_result in entries {
        let entry = entry_result
            .map_err(|error| format!("Impossible de lire une entrée de save: {}", error))?;
        let path = entry.path();

        if path.is_dir() {
            collect_saves(root, &path, output)?;
            continue;
        }

        if !is_save_file(&path) {
            continue;
        }

        let metadata = fs::metadata(&path)
            .map_err(|error| format!("Impossible de lire les métadonnées save: {}", error))?;

        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        output.push(LocalSaveEntry {
            file_name: file_name.clone(),
            file_path: path.to_string_lossy().to_string(),
            file_size_bytes: metadata.len(),
            platform_guess: guess_save_platform(root, &path, &file_name),
        });
    }

    Ok(())
}

fn is_rom_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("iso")
            | Some("rvz")
            | Some("wbfs")
            | Some("gcz")
            | Some("ciso")
            | Some("nds")
            | Some("3ds")
            | Some("cci")
            | Some("cia")
            | Some("3dsx")
            | Some("xci")
            | Some("nsp")
            | Some("nro")
            | Some("gba")
            | Some("gb")
            | Some("gbc")
            | Some("nes")
            | Some("sfc")
            | Some("smc")
            | Some("z64")
            | Some("n64")
            | Some("v64")
            | Some("cue")
            | Some("bin")
            | Some("img")
            | Some("chd")
            | Some("pbp")
            | Some("cso")
    )
}

fn is_save_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("sav")
            | Some("dsv")
            | Some("srm")
            | Some("eep")
            | Some("fla")
            | Some("dat")
            | Some("state")
            | Some("ss0")
            | Some("ss1")
            | Some("ss2")
            | Some("ss3")
            | Some("ss4")
            | Some("ss5")
            | Some("ss6")
            | Some("ss7")
            | Some("ss8")
            | Some("ss9")
    )
}

fn guess_rom_platform(root: &Path, file_path: &Path, file_name: &str) -> String {
    if let Some(folder_platform) = guess_platform_from_parent_folder(root, file_path) {
        return folder_platform;
    }

    guess_rom_platform_from_extension(file_name)
}

fn guess_save_platform(root: &Path, file_path: &Path, file_name: &str) -> String {
    if let Some(folder_platform) = guess_platform_from_parent_folder(root, file_path) {
        return folder_platform;
    }

    guess_save_platform_from_extension(file_name)
}

fn guess_platform_from_parent_folder(root: &Path, file_path: &Path) -> Option<String> {
    let parent = file_path.parent()?;
    let relative = parent.strip_prefix(root).ok()?;
    let first = relative.iter().next()?;
    let raw = first.to_string_lossy().to_ascii_lowercase();

    Some(match raw.as_str() {
        "wii" => "Wii".to_string(),
        "gamecube" | "gc" => "GameCube".to_string(),
        "gamecube-wii" | "wii-gamecube" => "GameCube / Wii".to_string(),
        "nds" | "ds" => "Nintendo DS".to_string(),
        "3ds" => "Nintendo 3DS".to_string(),
        "switch" | "nsw" => "Nintendo Switch".to_string(),
        "ps2" => "PS2".to_string(),
        "psp" => "PSP".to_string(),
        "ps1" | "psx" | "playstation" => "PlayStation".to_string(),
        "gba" => "Game Boy Advance".to_string(),
        "gb" => "Game Boy".to_string(),
        "gbc" => "Game Boy Color".to_string(),
        "nes" => "NES".to_string(),
        "snes" | "sfc" => "SNES".to_string(),
        "n64" => "Nintendo 64".to_string(),
        other => prettify_folder_name(other),
    })
}

fn prettify_folder_name(value: &str) -> String {
    value.replace('-', " ")
}

fn guess_rom_platform_from_extension(file_name: &str) -> String {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "iso" | "rvz" | "wbfs" | "gcz" => "Wii / GameCube".to_string(),
        "nds" => "Nintendo DS".to_string(),
        "3ds" | "cci" | "cia" | "3dsx" => "Nintendo 3DS".to_string(),
        "xci" | "nsp" | "nro" => "Nintendo Switch".to_string(),
        "gba" => "Game Boy Advance".to_string(),
        "gb" => "Game Boy".to_string(),
        "gbc" => "Game Boy Color".to_string(),
        "nes" => "NES".to_string(),
        "sfc" | "smc" => "SNES".to_string(),
        "z64" | "n64" | "v64" => "Nintendo 64".to_string(),
        "cue" | "bin" | "img" | "chd" => "PlayStation".to_string(),
        "pbp" | "cso" => "PSP".to_string(),
        _ => "Autre".to_string(),
    }
}

fn guess_save_platform_from_extension(file_name: &str) -> String {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "sav" | "dsv" => "Nintendo DS / portable".to_string(),
        "srm" => "SNES / rétro".to_string(),
        "eep" | "fla" => "Nintendo 64 / GBA".to_string(),
        "state" | "ss0" | "ss1" | "ss2" | "ss3" | "ss4" | "ss5" | "ss6" | "ss7" | "ss8" | "ss9" => {
            "Save state".to_string()
        }
        _ => "Autre".to_string(),
    }
}