use crate::emulator_installer::resolve_emulator_executable;
use crate::platform_router::resolve_emulator_id_for_rom_path;
use crate::portable_paths::PortablePaths;
use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameLaunchResult {
    pub emulator_id: String,
    pub executable_path: String,
    pub rom_path: String,
    pub launched: bool,
}

pub fn launch_game(
    paths: &PortablePaths,
    emulator_id: &str,
    rom_path: &str,
) -> Result<GameLaunchResult, String> {
    let executable_path = resolve_emulator_executable(paths, emulator_id)?;

    let rom = PathBuf::from(rom_path);
    if !rom.exists() {
        return Err(format!("ROM introuvable: {}", rom.to_string_lossy()));
    }

    let working_directory = executable_path
        .parent()
        .ok_or_else(|| "Impossible de déterminer le dossier de travail".to_string())?
        .to_path_buf();

    let mut command = Command::new(&executable_path);
    command.current_dir(&working_directory);

    match emulator_id {
        "dolphin" => {
            command.arg("--exec").arg(&rom);
        }
        "melonds" => {
            command.arg(&rom);
        }
        "eden" => {
            command.arg(&rom);
        }
        "pcsx2" => {
            command.arg("-batch").arg("--").arg(&rom);
        }
        "azahar" => {
            command.arg(&rom);
        }
        "ppsspp" => {
            command.arg(&rom);
        }
        "duckstation" => {
            command.arg("-batch").arg(&rom);
        }
        _ => {
            return Err(format!("Lancement de jeu non implémenté pour {}", emulator_id));
        }
    }

    command
        .spawn()
        .map_err(|error| format!("Lancement du jeu impossible: {}", error))?;

    Ok(GameLaunchResult {
        emulator_id: emulator_id.to_string(),
        executable_path: executable_path.to_string_lossy().to_string(),
        rom_path: rom.to_string_lossy().to_string(),
        launched: true,
    })
}

pub fn launch_game_auto(paths: &PortablePaths, rom_path: &str) -> Result<GameLaunchResult, String> {
    let emulator_id = resolve_emulator_id_for_rom_path(paths, rom_path)?;
    launch_game(paths, &emulator_id, rom_path)
}