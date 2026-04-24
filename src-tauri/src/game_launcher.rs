use crate::dolphin_controller_writer::apply_saved_controller_profile;
use crate::emulator_installer::resolve_emulator_executable;
use crate::platform_router::resolve_emulator_id_for_rom_path;
use crate::portable_paths::PortablePaths;
use crate::romm_sync::{
    launch_azahar, launch_dolphin, launch_eden, launch_melonds, launch_pcsx2, RommLaunchSession,
};
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
    romm_session: Option<&RommLaunchSession>,
) -> Result<GameLaunchResult, String> {
    let executable_path = resolve_emulator_executable(paths, emulator_id)?;
    let _ = apply_saved_controller_profile(paths, emulator_id);

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
            return launch_dolphin(paths, &executable_path, &rom, romm_session);
        }
        "melonds" => {
            return launch_melonds(paths, &executable_path, &rom, romm_session);
        }
        "eden" => {
            return launch_eden(paths, &executable_path, &rom, romm_session);
        }
        "pcsx2" => {
            return launch_pcsx2(paths, &executable_path, &rom, romm_session);
        }
        "azahar" => {
            return launch_azahar(paths, &executable_path, &rom, romm_session);
        }
        "ppsspp" => {
            command.arg(&rom);
        }
        "duckstation" => {
            command.arg("-batch").arg(&rom);
        }
        _ => {
            return Err(format!(
                "Lancement de jeu non implémenté pour {}",
                emulator_id
            ));
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
    launch_game(paths, &emulator_id, rom_path, None)
}

pub fn launch_game_auto_with_session(
    paths: &PortablePaths,
    rom_path: &str,
    romm_session: Option<&RommLaunchSession>,
) -> Result<GameLaunchResult, String> {
    let emulator_id = resolve_emulator_id_for_rom_path(paths, rom_path)?;
    launch_game(paths, &emulator_id, rom_path, romm_session)
}
