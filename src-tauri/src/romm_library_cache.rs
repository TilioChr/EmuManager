use crate::portable_paths::PortablePaths;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RommGameCacheRecord {
    cached_at_ms: u64,
    game: Value,
}

pub fn cache_romm_game_metadata(paths: &PortablePaths, game: &Value) -> Result<(), String> {
    let romm_id =
        game_id(game).ok_or_else(|| "RomM game metadata is missing an id.".to_string())?;
    let cache_dir = game_cache_dir(paths);
    fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("Cannot create RomM game metadata cache: {}", error))?;

    let record = RommGameCacheRecord {
        cached_at_ms: now_ms(),
        game: game.clone(),
    };
    let raw = serde_json::to_string_pretty(&record)
        .map_err(|error| format!("Cannot serialize RomM game metadata: {}", error))?;

    fs::write(
        cache_dir.join(format!("{}.json", sanitize_path_part(&romm_id))),
        raw,
    )
    .map_err(|error| format!("Cannot write RomM game metadata cache: {}", error))
}

pub fn load_romm_game_metadata(
    paths: &PortablePaths,
    romm_ids: &[String],
) -> Result<Vec<Value>, String> {
    let cache_dir = game_cache_dir(paths);
    let mut games = Vec::new();

    for romm_id in romm_ids {
        let file_path = cache_dir.join(format!("{}.json", sanitize_path_part(romm_id)));
        if !file_path.exists() {
            continue;
        }

        let raw = fs::read_to_string(&file_path)
            .map_err(|error| format!("Cannot read RomM game metadata cache: {}", error))?;
        let record = serde_json::from_str::<RommGameCacheRecord>(&raw)
            .map_err(|error| format!("Cannot parse RomM game metadata cache: {}", error))?;

        games.push(record.game);
    }

    Ok(games)
}

fn game_cache_dir(paths: &PortablePaths) -> std::path::PathBuf {
    Path::new(&paths.data)
        .join("library-cache")
        .join("romm")
        .join("games")
}

fn game_id(game: &Value) -> Option<String> {
    match game.get("id")? {
        Value::String(value) if !value.trim().is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
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
        "game".to_string()
    } else {
        trimmed.to_string()
    }
}
