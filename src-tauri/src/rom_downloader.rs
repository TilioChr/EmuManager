use crate::portable_paths::PortablePaths;
use reqwest::Client;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResult {
    pub file_path: String,
    pub file_name: String,
    pub bytes_written: u64,
}

pub async fn download_rom_to_library(
    paths: &PortablePaths,
    url: &str,
    file_name: &str,
) -> Result<DownloadResult, String> {
    let roms_dir = PathBuf::from(&paths.roms);
    fs::create_dir_all(&roms_dir)
        .map_err(|error| format!("Impossible de créer le dossier Roms: {}", error))?;

    let safe_file_name = sanitize_file_name(file_name);
    let destination = roms_dir.join(&safe_file_name);

    download_file(url, &destination).await?;

    let metadata = fs::metadata(&destination)
        .map_err(|error| format!("Impossible de lire le fichier téléchargé: {}", error))?;

    Ok(DownloadResult {
        file_path: destination.to_string_lossy().to_string(),
        file_name: safe_file_name,
        bytes_written: metadata.len(),
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
        .map_err(|error| format!("Création du fichier destination impossible: {}", error))?;

    file.write_all(&bytes)
        .await
        .map_err(|error| format!("Écriture du fichier destination impossible: {}", error))?;

    file.flush()
        .await
        .map_err(|error| format!("Flush du fichier destination impossible: {}", error))?;

    Ok(())
}

fn sanitize_file_name(input: &str) -> String {
    input
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => character,
        })
        .collect()
}