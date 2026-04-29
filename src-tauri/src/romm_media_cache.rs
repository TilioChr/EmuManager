use crate::portable_paths::PortablePaths;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_MEDIA_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RommMediaCacheRequest {
    pub media_id: String,
    pub media_kind: String,
    pub url: String,
    pub bearer_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RommCachedMediaRequest {
    pub media_id: String,
    pub media_kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RommMediaCacheResult {
    pub media_id: String,
    pub media_kind: String,
    pub file_path: String,
    pub mime_type: String,
    pub data_url: String,
}

pub async fn cache_romm_media(
    paths: &PortablePaths,
    request: &RommMediaCacheRequest,
) -> Result<RommMediaCacheResult, String> {
    let media_kind = sanitize_path_part(&request.media_kind);
    let media_id = sanitize_path_part(&request.media_id);
    let cache_dir = Path::new(&paths.data)
        .join("media-cache")
        .join("romm")
        .join(&media_kind);

    fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("Cannot create media cache directory: {}", error))?;

    if let Some(cached_path) = find_cached_media(&cache_dir, &media_id) {
        return read_cached_media(&media_id, &media_kind, cached_path);
    }

    let client = Client::new();
    let token = request
        .bearer_token
        .as_deref()
        .filter(|token| !token.trim().is_empty());
    let response = download_media(&client, &request.url, token).await?;
    let response = if response.status() == StatusCode::UNAUTHORIZED && token.is_some() {
        download_media(&client, &request.url, None).await?
    } else {
        response
    };

    if !response.status().is_success() {
        return Err(format!(
            "RomM media download failed with status {}",
            response.status()
        ));
    }

    let mime_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| value.starts_with("image/"))
        .unwrap_or("image/jpeg")
        .to_string();
    let extension = media_extension_from_url(&request.url)
        .or_else(|| media_extension_from_mime(&mime_type))
        .unwrap_or("jpg");

    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Cannot read RomM media: {}", error))?;

    if bytes.len() > MAX_MEDIA_BYTES {
        return Err("RomM media is too large to cache.".to_string());
    }

    let file_path = cache_dir.join(format!("{}.{}", media_id, extension));
    fs::write(&file_path, &bytes)
        .map_err(|error| format!("Cannot write RomM media cache: {}", error))?;

    Ok(RommMediaCacheResult {
        media_id: request.media_id.clone(),
        media_kind: request.media_kind.clone(),
        file_path: file_path.to_string_lossy().to_string(),
        mime_type: mime_type.clone(),
        data_url: format!("data:{};base64,{}", mime_type, STANDARD.encode(bytes)),
    })
}

pub fn read_romm_cached_media(
    paths: &PortablePaths,
    request: &RommCachedMediaRequest,
) -> Result<Option<RommMediaCacheResult>, String> {
    let media_kind = sanitize_path_part(&request.media_kind);
    let media_id = sanitize_path_part(&request.media_id);
    let cache_dir = Path::new(&paths.data)
        .join("media-cache")
        .join("romm")
        .join(&media_kind);

    Ok(find_cached_media(&cache_dir, &media_id)
        .map(|cached_path| read_cached_media(&media_id, &media_kind, cached_path))
        .transpose()?)
}

async fn download_media(
    client: &Client,
    url: &str,
    bearer_token: Option<&str>,
) -> Result<Response, String> {
    let mut request = client.get(url);

    if let Some(token) = bearer_token {
        request = request.bearer_auth(token);
    }

    request
        .send()
        .await
        .map_err(|error| format!("Cannot download RomM media: {}", error))
}

fn find_cached_media(cache_dir: &Path, media_id: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(cache_dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        let stem = path.file_stem()?.to_string_lossy();

        if path.is_file() && stem == media_id {
            return Some(path);
        }
    }

    None
}

fn read_cached_media(
    media_id: &str,
    media_kind: &str,
    file_path: PathBuf,
) -> Result<RommMediaCacheResult, String> {
    let bytes = fs::read(&file_path)
        .map_err(|error| format!("Cannot read cached RomM media: {}", error))?;
    let mime_type = media_extension_from_path(&file_path)
        .and_then(media_mime_from_extension)
        .unwrap_or("image/jpeg")
        .to_string();

    Ok(RommMediaCacheResult {
        media_id: media_id.to_string(),
        media_kind: media_kind.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
        mime_type: mime_type.clone(),
        data_url: format!("data:{};base64,{}", mime_type, STANDARD.encode(bytes)),
    })
}

fn media_extension_from_url(url: &str) -> Option<&'static str> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    media_extension_from_path(Path::new(path))
}

fn media_extension_from_path(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => Some("jpg"),
        Some("png") => Some("png"),
        Some("webp") => Some("webp"),
        Some("gif") => Some("gif"),
        _ => None,
    }
}

fn media_extension_from_mime(mime_type: &str) -> Option<&'static str> {
    match mime_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        _ => None,
    }
}

fn media_mime_from_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

fn sanitize_path_part(input: &str) -> String {
    let sanitized: String = input
        .trim()
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => character,
            _ => '-',
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');

    if trimmed.is_empty() {
        "media".to_string()
    } else {
        trimmed.to_string()
    }
}
