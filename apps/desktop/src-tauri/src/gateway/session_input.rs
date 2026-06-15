//! Durable inbox for input addressed to existing gateway sessions.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionInputRecord {
    pub id: String,
    pub session_id: String,
    pub message: String,
    pub received_at_ms: u64,
}

#[derive(Debug, Default)]
pub struct SessionInputStore {
    records: Mutex<Vec<SessionInputRecord>>,
    path: Option<PathBuf>,
}

impl SessionInputStore {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            path: None,
        }
    }

    pub fn persistent_default() -> Self {
        Self::persistent_at(default_session_input_store_path())
    }

    pub fn persistent_at(path: PathBuf) -> Self {
        Self {
            records: Mutex::new(load_session_inputs(&path)),
            path: Some(path),
        }
    }

    pub fn push(&self, record: SessionInputRecord) {
        if let Ok(mut records) = self.records.lock() {
            self.refresh_locked(&mut records);
            records.retain(|existing| existing.id != record.id);
            records.push(record);
            self.save_locked(&records);
        }
    }

    pub fn list(&self) -> Vec<SessionInputRecord> {
        self.records
            .lock()
            .map(|mut records| {
                self.refresh_locked(&mut records);
                records.clone()
            })
            .unwrap_or_default()
    }

    fn refresh_locked(&self, records: &mut Vec<SessionInputRecord>) {
        let Some(path) = &self.path else {
            return;
        };
        merge_session_inputs(records, load_session_inputs(path));
    }

    fn save_locked(&self, records: &[SessionInputRecord]) {
        let Some(path) = &self.path else {
            return;
        };
        if let Err(error) = save_session_inputs(path, records) {
            log::warn!("failed to persist gateway session inputs: {error}");
        }
    }
}

pub fn new_session_input_record(
    id: String,
    session_id: String,
    message: String,
) -> SessionInputRecord {
    SessionInputRecord {
        id,
        session_id,
        message,
        received_at_ms: now_millis(),
    }
}

fn default_session_input_store_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".forge")
        .join("session-inputs.json")
}

fn load_session_inputs(path: &Path) -> Vec<SessionInputRecord> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<SessionInputRecord>>(&raw) {
        Ok(records) => records,
        Err(error) => {
            log::warn!("failed to load gateway session inputs from disk: {error}");
            Vec::new()
        }
    }
}

fn save_session_inputs(path: &Path, records: &[SessionInputRecord]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("create input dir: {error}"))?;
    }
    let json = serde_json::to_string_pretty(records)
        .map_err(|error| format!("serialize inputs: {error}"))?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json.as_bytes()).map_err(|error| format!("write input tmp: {error}"))?;
    std::fs::rename(&tmp, path).map_err(|error| format!("replace input store: {error}"))?;
    Ok(())
}

fn merge_session_inputs(target: &mut Vec<SessionInputRecord>, incoming: Vec<SessionInputRecord>) {
    for record in incoming {
        if let Some(existing) = target.iter_mut().find(|existing| existing.id == record.id) {
            *existing = record;
        } else {
            target.push(record);
        }
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_input_store_persists_and_reloads_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("session-inputs.json");
        let store = SessionInputStore::persistent_at(path.clone());

        store.push(SessionInputRecord {
            id: "input-1".into(),
            session_id: "session-1".into(),
            message: "continue".into(),
            received_at_ms: 10,
        });

        let restored = SessionInputStore::persistent_at(path);
        let records = restored.list();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "input-1");
        assert_eq!(records[0].session_id, "session-1");
        assert_eq!(records[0].message, "continue");
    }
}
