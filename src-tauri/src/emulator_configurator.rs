use crate::portable_paths::PortablePaths;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureResult {
    pub emulator_id: String,
    pub portable_file_path: String,
    pub user_directory: String,
    pub config_directory: String,
    pub gc_saves_directory: String,
    pub wii_saves_directory: String,
}

pub fn configure_emulator(paths: &PortablePaths, emulator_id: &str) -> Result<ConfigureResult, String> {
    match emulator_id {
        "dolphin" => configure_dolphin(paths),
        _ => Err(format!("Configuration non implémentée pour {}", emulator_id)),
    }
}

fn configure_dolphin(paths: &PortablePaths) -> Result<ConfigureResult, String> {
    let install_root = PathBuf::from(&paths.emu).join("Dolphin");
    let executable_dir = locate_dolphin_executable_dir(&install_root)?;
    let portable_file = executable_dir.join("portable.txt");
    let user_dir = executable_dir.join("User");
    let config_dir = user_dir.join("Config");
    let gc_saves_dir = user_dir.join("GC");
    let wii_saves_dir = user_dir.join("Wii");

    fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Impossible de créer User/Config: {}", error))?;
    fs::create_dir_all(&gc_saves_dir)
        .map_err(|error| format!("Impossible de créer User/GC: {}", error))?;
    fs::create_dir_all(&wii_saves_dir)
        .map_err(|error| format!("Impossible de créer User/Wii: {}", error))?;

    fs::write(&portable_file, "")
        .map_err(|error| format!("Impossible de créer portable.txt: {}", error))?;

    write_dolphin_ini(&config_dir.join("Dolphin.ini"), paths)?;

    Ok(ConfigureResult {
        emulator_id: "dolphin".to_string(),
        portable_file_path: portable_file.to_string_lossy().to_string(),
        user_directory: user_dir.to_string_lossy().to_string(),
        config_directory: config_dir.to_string_lossy().to_string(),
        gc_saves_directory: gc_saves_dir.to_string_lossy().to_string(),
        wii_saves_directory: wii_saves_dir.to_string_lossy().to_string(),
    })
}

fn locate_dolphin_executable_dir(install_root: &Path) -> Result<PathBuf, String> {
    let direct_exe = install_root.join("Dolphin.exe");
    if direct_exe.exists() {
        return Ok(install_root.to_path_buf());
    }

    let nested = install_root.join("Dolphin-x64");
    if nested.join("Dolphin.exe").exists() {
        return Ok(nested);
    }

    Err(format!(
        "Impossible de localiser Dolphin.exe dans {}",
        install_root.to_string_lossy()
    ))
}

fn write_dolphin_ini(path: &Path, portable_paths: &PortablePaths) -> Result<(), String> {
    let content = format!(
        concat!(
            "[General]\n",
            "ShowLag=False\n",
            "ConfirmStop=False\n",
            "AutoUpdateTrack=\n",
            "\n",
            "[Interface]\n",
            "UseBuiltinTitleDatabase=True\n",
            "\n",
            "[Display]\n",
            "RenderToMain=False\n",
            "\n",
            "[Analytics]\n",
            "Enabled=False\n",
            "\n",
            "; EmuManager roots\n",
            "; Roms={}\n",
            "; Saves={}\n",
            "; Firmware={}\n"
        ),
        portable_paths.roms,
        portable_paths.saves,
        portable_paths.firmware
    );

    fs::write(path, content).map_err(|error| format!("Impossible d'écrire Dolphin.ini: {}", error))
}