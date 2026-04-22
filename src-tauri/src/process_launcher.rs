use crate::emulator_installer::resolve_emulator_executable;
use crate::portable_paths::PortablePaths;
use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub emulator_id: String,
    pub executable_path: String,
    pub working_directory: String,
    pub launched: bool,
}

pub fn launch_emulator(paths: &PortablePaths, emulator_id: &str) -> Result<LaunchResult, String> {
    let executable_path = resolve_emulator_executable(paths, emulator_id)?;

    let working_directory = executable_path
        .parent()
        .ok_or_else(|| "Impossible de déterminer le dossier de travail".to_string())?
        .to_path_buf();

    Command::new(&executable_path)
        .current_dir(&working_directory)
        .spawn()
        .map_err(|error| format!("Lancement impossible: {}", error))?;

    Ok(LaunchResult {
        emulator_id: emulator_id.to_string(),
        executable_path: executable_path.to_string_lossy().to_string(),
        working_directory: working_directory.to_string_lossy().to_string(),
        launched: true,
    })
}