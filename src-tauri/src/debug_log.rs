use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

static LOG_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugLogEntry {
    pub id: String,
    pub timestamp: u64,
    pub level: String,
    pub source: String,
    pub scope: String,
    pub message: String,
    pub details: Option<String>,
}

pub fn emit_debug_log(
    app: &AppHandle,
    level: &str,
    scope: &str,
    message: &str,
    details: Option<String>,
) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);

    let entry = DebugLogEntry {
        id: format!(
            "backend-{}-{}-{}",
            timestamp,
            LOG_COUNTER.fetch_add(1, Ordering::Relaxed),
            scope.replace(' ', "-")
        ),
        timestamp,
        level: level.to_string(),
        source: "backend".to_string(),
        scope: scope.to_string(),
        message: message.to_string(),
        details,
    };

    let _ = app.emit("debug-log-entry", entry);
}
