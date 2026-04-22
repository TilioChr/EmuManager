use crate::emulator_registry::{built_in_emulators, EmulatorDefinition};
use crate::portable_paths::PortablePaths;
use reqwest::Client;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

const DOLPHIN_DOWNLOAD_URL: &str = "https://dl.dolphin-emu.org/releases/2603a/dolphin-2603a-x64.7z";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub emulator_id: String,
    pub install_path: String,
    pub executable_path: String,
    pub archive_path: String,
}

pub fn is_emulator_installed(paths: &PortablePaths, emulator_id: &str) -> bool {
    match get_emulator_definition(emulator_id) {
        Some(definition) => emulator_executable_path(paths, &definition).exists(),
        None => false,
    }
}

pub async fn install_emulator(paths: &PortablePaths, emulator_id: &str) -> Result<InstallResult, String> {
    let definition = get_emulator_definition(emulator_id)
        .ok_or_else(|| format!("Émulateur non supporté: {}", emulator_id))?;

    match emulator_id {
        "dolphin" => install_dolphin(paths, &definition).await,
        _ => Err(format!("Installateur non implémenté pour {}", emulator_id)),
    }
}

fn get_emulator_definition(emulator_id: &str) -> Option<EmulatorDefinition> {
    built_in_emulators()
        .into_iter()
        .find(|entry| entry.id == emulator_id)
}

async fn install_dolphin(
    paths: &PortablePaths,
    definition: &EmulatorDefinition,
) -> Result<InstallResult, String> {
    let emu_root = PathBuf::from(&paths.emu);
    let install_dir = emu_root.join(definition.install_dir_name);
    let temp_dir = PathBuf::from(&paths.data).join("downloads");
    let archive_path = temp_dir.join("dolphin-latest.7z");

    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("Impossible de créer le dossier d'installation: {}", error))?;
    fs::create_dir_all(&temp_dir)
        .map_err(|error| format!("Impossible de créer le dossier temporaire: {}", error))?;

    download_file(DOLPHIN_DOWNLOAD_URL, &archive_path).await?;

    if install_dir.exists() {
        fs::remove_dir_all(&install_dir)
            .map_err(|error| format!("Impossible de nettoyer l'ancienne installation: {}", error))?;
        fs::create_dir_all(&install_dir)
            .map_err(|error| format!("Impossible de recréer le dossier d'installation: {}", error))?;
    }

    sevenz_rust::decompress_file(&archive_path, &install_dir)
        .map_err(|error| format!("Extraction Dolphin impossible: {}", error))?;

    let executable = emulator_executable_path(paths, definition);
    if !executable.exists() {
        return Err(format!(
            "Installation terminée mais exécutable introuvable: {}",
            executable.to_string_lossy()
        ));
    }

    Ok(InstallResult {
        emulator_id: definition.id.to_string(),
        install_path: install_dir.to_string_lossy().to_string(),
        executable_path: executable.to_string_lossy().to_string(),
        archive_path: archive_path.to_string_lossy().to_string(),
    })
}

async fn download_file(url: &str, destination: &Path) -> Result<(), String> {
    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Téléchargement impossible: {}", error))?;

    if !response.status().is_success() {
        return Err(format!("Téléchargement échoué avec le statut {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Lecture du téléchargement impossible: {}", error))?;

    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|error| format!("Création du fichier temporaire impossible: {}", error))?;

    file.write_all(&bytes)
        .await
        .map_err(|error| format!("Écriture du fichier temporaire impossible: {}", error))?;

    file.flush()
        .await
        .map_err(|error| format!("Flush du fichier temporaire impossible: {}", error))?;

    Ok(())
}

fn emulator_executable_path(paths: &PortablePaths, definition: &EmulatorDefinition) -> PathBuf {
    PathBuf::from(&paths.emu)
        .join(definition.install_dir_name)
        .join(definition.executable_rel_path)
}