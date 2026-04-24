use crate::portable_paths::PortablePaths;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResult {
    pub file_path: String,
    pub file_name: String,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressPayload {
    pub download_id: String,
    pub file_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRomRequest {
    pub url: String,
    pub file_name: String,
    pub bearer_token: Option<String>,
    pub download_id: String,
    pub expected_total_bytes: Option<u64>,
    pub relative_subdir: Option<String>,
}

pub async fn download_rom_to_library(
    app: &AppHandle,
    paths: &PortablePaths,
    request: &DownloadRomRequest,
) -> Result<DownloadResult, String> {
    let roms_root = PathBuf::from(&paths.roms);
    let target_dir = resolve_target_rom_dir(&roms_root, request.relative_subdir.as_deref())?;

    fs::create_dir_all(&target_dir)
        .map_err(|error| format!("Impossible de créer le dossier cible Roms: {}", error))?;

    let safe_file_name = sanitize_file_name(&request.file_name);
    let destination = target_dir.join(&safe_file_name);

    let bytes_written = download_file(
        app,
        &request.url,
        &destination,
        request.bearer_token.as_deref(),
        &request.download_id,
        &safe_file_name,
        request.expected_total_bytes,
    )
    .await?;

    Ok(DownloadResult {
        file_path: destination.to_string_lossy().to_string(),
        file_name: safe_file_name,
        bytes_written,
    })
}

fn resolve_target_rom_dir(root: &Path, relative_subdir: Option<&str>) -> Result<PathBuf, String> {
    let mut target = root.to_path_buf();

    if let Some(subdir) = relative_subdir {
        let candidate = Path::new(subdir);

        for component in candidate.components() {
            match component {
                Component::Normal(part) => target.push(part),
                Component::CurDir => {}
                _ => return Err("Sous-dossier ROM invalide.".to_string()),
            }
        }
    }

    Ok(target)
}

async fn download_file(
    app: &AppHandle,
    url: &str,
    destination: &Path,
    bearer_token: Option<&str>,
    download_id: &str,
    file_name: &str,
    expected_total_bytes: Option<u64>,
) -> Result<u64, String> {
    let client = Client::new();

    let mut request = client.get(url);

    if let Some(token) = bearer_token {
        if !token.trim().is_empty() {
            request = request.bearer_auth(token);
        }
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("Téléchargement impossible: {}", error))?;

    if !response.status().is_success() {
        return Err(format!(
            "Téléchargement échoué avec le statut {}",
            response.status()
        ));
    }

    let total_bytes = response.content_length().or(expected_total_bytes);
    let mut stream = response.bytes_stream();

    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|error| format!("Création du fichier destination impossible: {}", error))?;

    let mut downloaded_bytes: u64 = 0;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result
            .map_err(|error| format!("Lecture du téléchargement impossible: {}", error))?;

        file.write_all(&chunk)
            .await
            .map_err(|error| format!("Écriture du fichier destination impossible: {}", error))?;

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

        let payload = DownloadProgressPayload {
            download_id: download_id.to_string(),
            file_name: file_name.to_string(),
            downloaded_bytes,
            total_bytes,
            percent,
        };

        app.emit("rom-download-progress", payload)
            .map_err(|error| format!("Impossible d'émettre la progression: {}", error))?;
    }

    file.flush()
        .await
        .map_err(|error| format!("Flush du fichier destination impossible: {}", error))?;

    let payload = DownloadProgressPayload {
        download_id: download_id.to_string(),
        file_name: file_name.to_string(),
        downloaded_bytes,
        total_bytes,
        percent: 100.0,
    };

    app.emit("rom-download-complete", payload)
        .map_err(|error| format!("Impossible d'émettre la fin du téléchargement: {}", error))?;

    Ok(downloaded_bytes)
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
