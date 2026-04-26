use crate::emulator_registry::{built_in_emulators, EmulatorDefinition};
use crate::debug_log::emit_debug_log;
use crate::portable_paths::PortablePaths;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Emitter};
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorInstallProgressPayload {
    pub emulator_id: String,
    pub file_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: f64,
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

pub async fn install_emulator(
    app: &AppHandle,
    paths: &PortablePaths,
    emulator_id: &str,
) -> Result<InstallResult, String> {
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
    let archive_ext = if archive_format.eq_ignore_ascii_case("zip") {
        "zip"
    } else {
        "7z"
    };
    let archive_path = temp_dir.join(format!("{}-latest.{}", definition.id, archive_ext));
    let archive_file_name = archive_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("{}-latest.{}", definition.id, archive_ext));

    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("Impossible de créer le dossier d'installation: {}", error))?;
    fs::create_dir_all(&temp_dir)
        .map_err(|error| format!("Impossible de créer le dossier temporaire: {}", error))?;

    download_file(
        app,
        download_url,
        &archive_path,
        definition.id,
        &archive_file_name,
    )
    .await?;

    if install_dir.exists() {
        fs::remove_dir_all(&install_dir).map_err(|error| {
            format!("Impossible de nettoyer l'ancienne installation: {}", error)
        })?;
    }

    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("Impossible de recréer le dossier d'installation: {}", error))?;

    emit_debug_log(
        app,
        "info",
        "emulator-install",
        &format!("Extracting {} archive", definition.name),
        Some(format!(
            "archive_path={}\ninstall_dir={}",
            archive_path.to_string_lossy(),
            install_dir.to_string_lossy()
        )),
    );

    match archive_format.to_ascii_lowercase().as_str() {
        "zip" => extract_zip(&archive_path, &install_dir)?,
        "7z" => sevenz_rust::decompress_file(&archive_path, &install_dir)
            .map_err(|error| format!("Extraction impossible: {}", error))?,
        other => return Err(format!("Format d'archive non supporté: {}", other)),
    }

    emit_debug_log(
        app,
        "success",
        "emulator-install",
        &format!("Extracted {} archive", definition.name),
        Some(format!("install_dir={}", install_dir.to_string_lossy())),
    );

    let executable =
        resolve_executable_in_install_dir(&install_dir, &definition).ok_or_else(|| {
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

pub fn resolve_emulator_executable(
    paths: &PortablePaths,
    emulator_id: &str,
) -> Result<PathBuf, String> {
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

async fn download_file(
    app: &AppHandle,
    url: &str,
    destination: &Path,
    emulator_id: &str,
    file_name: &str,
) -> Result<(), String> {
    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Téléchargement impossible: {}", error))?;

    if !response.status().is_success() {
        return Err(format!(
            "Téléchargement échoué avec le statut {}",
            response.status()
        ));
    }

    let total_bytes = response.content_length();
    let mut stream = response.bytes_stream();

    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|error| format!("Création du fichier temporaire impossible: {}", error))?;

    let mut downloaded_bytes: u64 = 0;

    emit_install_progress(
        app,
        emulator_id,
        file_name,
        downloaded_bytes,
        total_bytes,
        0.0,
    )?;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result
            .map_err(|error| format!("Lecture du téléchargement impossible: {}", error))?;

        file.write_all(&chunk)
            .await
            .map_err(|error| format!("Écriture du fichier temporaire impossible: {}", error))?;

        downloaded_bytes += chunk.len() as u64;

        let percent = if let Some(total) = total_bytes {
            if total > 0 {
                ((downloaded_bytes as f64 / total as f64) * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        emit_install_progress(
            app,
            emulator_id,
            file_name,
            downloaded_bytes,
            total_bytes,
            percent,
        )?;
    }

    file.flush()
        .await
        .map_err(|error| format!("Flush du fichier temporaire impossible: {}", error))?;

    emit_install_progress(
        app,
        emulator_id,
        file_name,
        downloaded_bytes,
        total_bytes,
        100.0,
    )?;
    app.emit(
        "emulator-install-complete",
        EmulatorInstallProgressPayload {
            emulator_id: emulator_id.to_string(),
            file_name: file_name.to_string(),
            downloaded_bytes,
            total_bytes,
            percent: 100.0,
        },
    )
    .map_err(|error| format!("Impossible d'emettre la fin d'installation: {}", error))?;

    Ok(())
}

fn emit_install_progress(
    app: &AppHandle,
    emulator_id: &str,
    file_name: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    percent: f64,
) -> Result<(), String> {
    app.emit(
        "emulator-install-progress",
        EmulatorInstallProgressPayload {
            emulator_id: emulator_id.to_string(),
            file_name: file_name.to_string(),
            downloaded_bytes,
            total_bytes,
            percent,
        },
    )
    .map_err(|error| {
        format!(
            "Impossible d'emettre la progression d'installation: {}",
            error
        )
    })
}

fn extract_zip(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path)
        .map_err(|error| format!("Impossible d'ouvrir l'archive zip: {}", error))?;
    let mut archive = ZipArchive::new(file).map_err(|error| format!("Zip invalide: {}", error))?;

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
            fs::create_dir_all(parent).map_err(|error| {
                format!("Impossible de créer le dossier parent extrait: {}", error)
            })?;
        }

        let mut outfile = fs::File::create(&out_path)
            .map_err(|error| format!("Impossible de créer un fichier extrait: {}", error))?;

        io::copy(&mut entry, &mut outfile)
            .map_err(|error| format!("Impossible d'extraire un fichier zip: {}", error))?;
    }

    Ok(())
}

fn resolve_executable_in_install_dir(
    install_dir: &Path,
    definition: &EmulatorDefinition,
) -> Option<PathBuf> {
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
