use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::adapters::base::ChatMessage;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentSessionSnapshot {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub working_dir: String,
    pub messages: Vec<ChatMessage>,
    pub summary: Option<String>,
    pub context_window_tokens: Option<u32>,
    pub updated_at_ms: u64,
}

impl AgentSessionSnapshot {
    pub fn new(
        session_id: String,
        provider: String,
        model: String,
        working_dir: String,
        messages: Vec<ChatMessage>,
        summary: Option<String>,
        context_window_tokens: Option<u32>,
    ) -> Self {
        Self {
            session_id,
            provider,
            model,
            working_dir,
            messages,
            summary,
            context_window_tokens,
            updated_at_ms: now_ms(),
        }
    }
}

pub fn save_session_snapshot(snapshot: &AgentSessionSnapshot) -> Result<(), String> {
    let path = snapshot_path(&snapshot.session_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create session snapshot dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(snapshot)
        .map_err(|e| format!("Failed to serialize session snapshot: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write session snapshot: {e}"))?;
    Ok(())
}

pub fn load_session_snapshot(session_id: &str) -> Result<AgentSessionSnapshot, String> {
    let path = snapshot_path(session_id)?;
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read saved session '{}': {e}", session_id))?;
    serde_json::from_str(&json)
        .map_err(|e| format!("Saved session '{}' is corrupted: {e}", session_id))
}

pub fn delete_session_snapshot(session_id: &str) -> Result<(), String> {
    let path = snapshot_path(session_id)?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete saved session '{}': {e}", session_id))?;
    }
    Ok(())
}

fn snapshot_path(session_id: &str) -> Result<PathBuf, String> {
    let id = safe_session_id(session_id);
    if id.is_empty() {
        return Err("Invalid session id".to_string());
    }
    Ok(app_data_dir().join("sessions").join(format!("{id}.json")))
}

fn safe_session_id(session_id: &str) -> String {
    session_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

fn app_data_dir() -> PathBuf {
    home_dir().join(".forge")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}
