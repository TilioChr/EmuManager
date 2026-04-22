use crate::emulator_registry::{built_in_emulators, EmulatorDefinition};
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
    let definition = built_in_emulators()
        .into_iter()
        .find(|entry| entry.id == emulator_id)
        .ok_or_else(|| format!("Émulateur non supporté: {}", emulator_id))?;

    let executable_path = emulator_executable_path(paths, &definition);
    if !executable_path.exists() {
        return Err(format!(
            "Exécutable introuvable: {}",
            executable_path.to_string_lossy()
        ));
    }

    let rom = PathBuf::from(rom_path);
    if !rom.exists() {
        return Err(format!("ROM introuvable: {}", rom.to_string_lossy()));
    }

    let working_directory = executable_path
        .parent()
        .ok_or_else(|| "Impossible de déterminer le dossier de travail".to_string())?
        .to_path_buf();

    match emulator_id {
        "dolphin" => {
            Command::new(&executable_path)
                .current_dir(&working_directory)
                .arg("--exec")
                .arg(&rom)
                .spawn()
                .map_err(|error| format!("Lancement du jeu impossible: {}", error))?;
        }
        _ => {
            return Err(format!("Lancement de jeu non implémenté pour {}", emulator_id));
        }
    }

    Ok(GameLaunchResult {
        emulator_id: emulator_id.to_string(),
        executable_path: executable_path.to_string_lossy().to_string(),
        rom_path: rom.to_string_lossy().to_string(),
        launched: true,
    })
}

fn emulator_executable_path(paths: &PortablePaths, definition: &EmulatorDefinition) -> PathBuf {
    PathBuf::from(&paths.emu)
        .join(definition.install_dir_name)
        .join(definition.executable_rel_path)
}