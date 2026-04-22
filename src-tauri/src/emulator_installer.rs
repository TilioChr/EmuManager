use crate::emulator_registry::{built_in_emulators, EmulatorDefinition};
use crate::portable_paths::PortablePaths;
use reqwest::Client;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::io::AsyncWriteExt;
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub emulator_id: String,
    pub install_path: String,
    pub executable_path: String,
    pub archive_path: String,
}

pub fn is_emulator_installed(paths: &PortablePaths, emulator_id: &str) -> bool {
    resolve_emulator_executable(paths, emulator_id).is_ok()
}

pub fn get_installed_emulator_version(
    paths: &PortablePaths,
    emulator_id: &str,
) -> Result<Option<String>, String> {
    let executable = match resolve_emulator_executable(paths, emulator_id) {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };

    read_windows_exe_version(&executable).map(Some)
}

pub async fn install_emulator(paths: &PortablePaths, emulator_id: &str) -> Result<InstallResult, String> {
    let definition = get_emulator_definition(emulator_id)
        .ok_or_else(|| format!("Émulateur non supporté: {}", emulator_id))?;

    let download_url = definition
        .download_url
        .ok_or_else(|| format!("Installateur non implémenté pour {}", emulator_id))?;

    let archive_format = definition
        .archive_format
        .ok_or_else(|| format!("Format d'archive inconnu pour {}", emulator_id))?;

    let emu_root = PathBuf::from(&paths.emu);
    let install_dir = emu_root.join(definition.install_dir_name);
    let temp_dir = PathBuf::from(&paths.data).join("downloads");
    let archive_ext = if archive_format.eq_ignore_ascii_case("zip") { "zip" } else { "7z" };
    let archive_path = temp_dir.join(format!("{}-latest.{}", definition.id, archive_ext));

    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("Impossible de créer le dossier d'installation: {}", error))?;
    fs::create_dir_all(&temp_dir)
        .map_err(|error| format!("Impossible de créer le dossier temporaire: {}", error))?;

    download_file(download_url, &archive_path).await?;

    if install_dir.exists() {
        fs::remove_dir_all(&install_dir)
            .map_err(|error| format!("Impossible de nettoyer l'ancienne installation: {}", error))?;
    }

    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("Impossible de recréer le dossier d'installation: {}", error))?;

    match archive_format.to_ascii_lowercase().as_str() {
        "zip" => extract_zip(&archive_path, &install_dir)?,
        "7z" => sevenz_rust::decompress_file(&archive_path, &install_dir)
            .map_err(|error| format!("Extraction impossible: {}", error))?,
        other => return Err(format!("Format d'archive non supporté: {}", other)),
    }

    let executable = resolve_executable_in_install_dir(&install_dir, &definition)
        .ok_or_else(|| {
            format!(
                "Installation terminée mais exécutable introuvable pour {}",
                definition.name
            )
        })?;

    Ok(InstallResult {
        emulator_id: definition.id.to_string(),
        install_path: install_dir.to_string_lossy().to_string(),
        executable_path: executable.to_string_lossy().to_string(),
        archive_path: archive_path.to_string_lossy().to_string(),
    })
}

pub fn resolve_emulator_executable(paths: &PortablePaths, emulator_id: &str) -> Result<PathBuf, String> {
    let definition = get_emulator_definition(emulator_id)
        .ok_or_else(|| format!("Émulateur non supporté: {}", emulator_id))?;

    let install_dir = PathBuf::from(&paths.emu).join(definition.install_dir_name);

    if !install_dir.exists() {
        return Err(format!(
            "Dossier d'installation introuvable: {}",
            install_dir.to_string_lossy()
        ));
    }

    resolve_executable_in_install_dir(&install_dir, &definition).ok_or_else(|| {
        format!(
            "Exécutable introuvable pour {} dans {}",
            definition.name,
            install_dir.to_string_lossy()
        )
    })
}

fn get_emulator_definition(emulator_id: &str) -> Option<EmulatorDefinition> {
    built_in_emulators()
        .into_iter()
        .find(|entry| entry.id == emulator_id)
}

fn read_windows_exe_version(executable: &Path) -> Result<String, String> {
    let path = executable.to_string_lossy().replace('\'', "''");
    let script = format!("(Get-Item '{}').VersionInfo.ProductVersion", path);

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .map_err(|error| format!("Impossible de lire la version de l'exécutable: {}", error))?;

    if !output.status.success() {
        return Err("Impossible de lire la version Windows de l'exécutable.".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err("Version exécutable vide.".to_string());
    }

    Ok(stdout)
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

fn extract_zip(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path)
        .map_err(|error| format!("Impossible d'ouvrir l'archive zip: {}", error))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Zip invalide: {}", error))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Impossible de lire une entrée zip: {}", error))?;

        let enclosed = entry
            .enclosed_name()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "Entrée zip invalide".to_string())?;

        let out_path = destination.join(enclosed);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|error| format!("Impossible de créer un dossier extrait: {}", error))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Impossible de créer le dossier parent extrait: {}", error))?;
        }

        let mut outfile = fs::File::create(&out_path)
            .map_err(|error| format!("Impossible de créer un fichier extrait: {}", error))?;

        io::copy(&mut entry, &mut outfile)
            .map_err(|error| format!("Impossible d'extraire un fichier zip: {}", error))?;
    }

    Ok(())
}

fn resolve_executable_in_install_dir(install_dir: &Path, definition: &EmulatorDefinition) -> Option<PathBuf> {
    let direct = install_dir.join(definition.executable_rel_path);
    if direct.exists() {
        return Some(direct);
    }

    let wanted: Vec<String> = definition
        .executable_name_candidates
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect();

    find_first_matching_exe(install_dir, &wanted)
}

fn find_first_matching_exe(dir: &Path, wanted_names: &[String]) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(found) = find_first_matching_exe(&path, wanted_names) {
                return Some(found);
            }
            continue;
        }

        let file_name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
        if wanted_names.iter().any(|wanted| wanted == &file_name) {
            return Some(path);
        }
    }

    None
}