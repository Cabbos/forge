//! Durable inbox for input addressed to existing gateway sessions.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_SESSION_INPUT_COMPLETIONS: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionInputRecord {
    pub id: String,
    pub session_id: String,
    pub message: String,
    pub received_at_ms: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionInputCompletionAction {
    #[default]
    Accepted,
    ClearedStale,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionInputCompletionRecord {
    pub input_id: String,
    pub session_id: String,
    pub message_preview: String,
    pub received_at_ms: u64,
    pub completed_at_ms: u64,
    #[serde(default)]
    pub action: SessionInputCompletionAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Default)]
pub struct SessionInputStore {
    records: Mutex<Vec<SessionInputRecord>>,
    path: Option<PathBuf>,
    completion_records: Mutex<Vec<SessionInputCompletionRecord>>,
    completion_path: Option<PathBuf>,
}

impl SessionInputStore {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            path: None,
            completion_records: Mutex::new(Vec::new()),
            completion_path: None,
        }
    }

    pub fn persistent_default() -> Self {
        Self::persistent_at(default_session_input_store_path())
    }

    pub fn persistent_at(path: PathBuf) -> Self {
        let completion_path = completion_store_path_for_input_path(&path);
        Self {
            records: Mutex::new(load_session_inputs(&path)),
            path: Some(path),
            completion_records: Mutex::new(load_session_input_completions(&completion_path)),
            completion_path: Some(completion_path),
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
                sorted_session_inputs(records.iter().cloned())
            })
            .unwrap_or_default()
    }

    pub fn list_for_sessions(
        &self,
        session_ids: &[String],
        limit: usize,
    ) -> Vec<SessionInputRecord> {
        let session_ids = session_ids
            .iter()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
            .collect::<HashSet<_>>();
        if session_ids.is_empty() || limit == 0 {
            return Vec::new();
        }

        self.records
            .lock()
            .map(|mut records| {
                self.refresh_locked(&mut records);
                sorted_session_inputs(
                    records
                        .iter()
                        .filter(|record| session_ids.contains(record.session_id.as_str()))
                        .cloned(),
                )
                .into_iter()
                .take(limit)
                .collect()
            })
            .unwrap_or_default()
    }

    pub fn complete(&self, input_id: &str) -> bool {
        self.complete_with_record(input_id).is_some()
    }

    pub fn complete_with_record(&self, input_id: &str) -> Option<SessionInputCompletionRecord> {
        let input_id = input_id.trim();
        if input_id.is_empty() {
            return None;
        }
        let record = self.records.lock().ok().and_then(|mut records| {
            self.refresh_locked(&mut records);
            let position = records.iter().position(|record| record.id == input_id)?;
            let record = records.remove(position);
            self.save_locked(&records);
            Some(record)
        })?;
        let completion = SessionInputCompletionRecord {
            input_id: record.id,
            session_id: record.session_id,
            message_preview: message_preview(&record.message),
            received_at_ms: record.received_at_ms,
            completed_at_ms: now_millis(),
            action: SessionInputCompletionAction::Accepted,
            reason: None,
        };
        self.push_completion(completion.clone());
        Some(completion)
    }

    pub fn clear_stale_with_record(
        &self,
        input_id: &str,
        reason: &str,
    ) -> Option<SessionInputCompletionRecord> {
        let input_id = input_id.trim();
        if input_id.is_empty() {
            return None;
        }
        let record = self.records.lock().ok().and_then(|mut records| {
            self.refresh_locked(&mut records);
            let position = records.iter().position(|record| record.id == input_id)?;
            let record = records.remove(position);
            self.save_locked(&records);
            Some(record)
        })?;
        let reason = reason.trim();
        let completion = SessionInputCompletionRecord {
            input_id: record.id,
            session_id: record.session_id,
            message_preview: message_preview(&record.message),
            received_at_ms: record.received_at_ms,
            completed_at_ms: now_millis(),
            action: SessionInputCompletionAction::ClearedStale,
            reason: Some(if reason.is_empty() {
                "stale gateway session input cleared by operator".to_string()
            } else {
                reason.to_string()
            }),
        };
        self.push_completion(completion.clone());
        Some(completion)
    }

    pub fn recent_completions(&self, limit: usize) -> Vec<SessionInputCompletionRecord> {
        if limit == 0 {
            return Vec::new();
        }
        self.completion_records
            .lock()
            .map(|mut records| {
                self.refresh_completions_locked(&mut records);
                sorted_session_input_completions(records.iter().cloned())
                    .into_iter()
                    .take(limit)
                    .collect()
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

    fn push_completion(&self, completion: SessionInputCompletionRecord) {
        if let Ok(mut records) = self.completion_records.lock() {
            self.refresh_completions_locked(&mut records);
            records.retain(|existing| existing.input_id != completion.input_id);
            records.push(completion);
            *records = sorted_session_input_completions(records.iter().cloned())
                .into_iter()
                .take(MAX_SESSION_INPUT_COMPLETIONS)
                .collect();
            self.save_completions_locked(&records);
        }
    }

    fn refresh_completions_locked(&self, records: &mut Vec<SessionInputCompletionRecord>) {
        let Some(path) = &self.completion_path else {
            return;
        };
        merge_session_input_completions(records, load_session_input_completions(path));
    }

    fn save_completions_locked(&self, records: &[SessionInputCompletionRecord]) {
        let Some(path) = &self.completion_path else {
            return;
        };
        if let Err(error) = save_session_input_completions(path, records) {
            log::warn!("failed to persist gateway session input completions: {error}");
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

fn completion_store_path_for_input_path(path: &Path) -> PathBuf {
    path.with_file_name("session-input-completions.json")
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

fn load_session_input_completions(path: &Path) -> Vec<SessionInputCompletionRecord> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<SessionInputCompletionRecord>>(&raw) {
        Ok(records) => records,
        Err(error) => {
            log::warn!("failed to load gateway session input completions from disk: {error}");
            Vec::new()
        }
    }
}

fn save_session_input_completions(
    path: &Path,
    records: &[SessionInputCompletionRecord],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create input completion dir: {error}"))?;
    }
    let json = serde_json::to_string_pretty(records)
        .map_err(|error| format!("serialize input completions: {error}"))?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json.as_bytes())
        .map_err(|error| format!("write input completion tmp: {error}"))?;
    std::fs::rename(&tmp, path)
        .map_err(|error| format!("replace input completion store: {error}"))?;
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

fn merge_session_input_completions(
    target: &mut Vec<SessionInputCompletionRecord>,
    incoming: Vec<SessionInputCompletionRecord>,
) {
    for record in incoming {
        if let Some(existing) = target
            .iter_mut()
            .find(|existing| existing.input_id == record.input_id)
        {
            *existing = record;
        } else {
            target.push(record);
        }
    }
    *target = sorted_session_input_completions(target.iter().cloned())
        .into_iter()
        .take(MAX_SESSION_INPUT_COMPLETIONS)
        .collect();
}

fn sorted_session_inputs(
    records: impl IntoIterator<Item = SessionInputRecord>,
) -> Vec<SessionInputRecord> {
    let mut records = records.into_iter().collect::<Vec<_>>();
    records.sort_by(|left, right| {
        left.received_at_ms
            .cmp(&right.received_at_ms)
            .then_with(|| left.id.cmp(&right.id))
    });
    records
}

fn sorted_session_input_completions(
    records: impl IntoIterator<Item = SessionInputCompletionRecord>,
) -> Vec<SessionInputCompletionRecord> {
    let mut records = records.into_iter().collect::<Vec<_>>();
    records.sort_by(|left, right| {
        right
            .completed_at_ms
            .cmp(&left.completed_at_ms)
            .then_with(|| right.received_at_ms.cmp(&left.received_at_ms))
            .then_with(|| left.input_id.cmp(&right.input_id))
    });
    records
}

fn message_preview(message: &str) -> String {
    let trimmed = message.trim();
    let mut preview = trimmed.chars().take(160).collect::<String>();
    if trimmed.chars().count() > 160 {
        preview.push_str("...");
    }
    preview
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

    #[test]
    fn session_input_store_lists_matching_sessions_in_received_order() {
        let store = SessionInputStore::new();
        store.push(SessionInputRecord {
            id: "input-3".into(),
            session_id: "session-2".into(),
            message: "third".into(),
            received_at_ms: 30,
        });
        store.push(SessionInputRecord {
            id: "input-1".into(),
            session_id: "session-1".into(),
            message: "first".into(),
            received_at_ms: 10,
        });
        store.push(SessionInputRecord {
            id: "input-2".into(),
            session_id: "session-1".into(),
            message: "second".into(),
            received_at_ms: 20,
        });

        let records = store.list_for_sessions(&[" session-1 ".to_string()], 1);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "input-1");
        assert_eq!(records[0].message, "first");
    }

    #[test]
    fn session_input_store_completes_records_and_persists_removal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("session-inputs.json");
        let store = SessionInputStore::persistent_at(path.clone());
        store.push(SessionInputRecord {
            id: "input-1".into(),
            session_id: "session-1".into(),
            message: "continue".into(),
            received_at_ms: 10,
        });

        assert!(store.complete(" input-1 "));
        assert!(!store.complete("input-1"));

        let restored = SessionInputStore::persistent_at(path);
        assert!(restored.list().is_empty());
    }

    #[test]
    fn session_input_store_records_completion_history() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("session-inputs.json");
        let store = SessionInputStore::persistent_at(path.clone());
        store.push(SessionInputRecord {
            id: "input-1".into(),
            session_id: "session-1".into(),
            message: "continue with the queued task".into(),
            received_at_ms: 10,
        });

        let completion = store
            .complete_with_record(" input-1 ")
            .expect("completion record");

        assert_eq!(completion.input_id, "input-1");
        assert_eq!(completion.session_id, "session-1");
        assert_eq!(completion.message_preview, "continue with the queued task");
        assert_eq!(completion.received_at_ms, 10);
        assert!(completion.completed_at_ms >= completion.received_at_ms);
        assert!(store.list().is_empty());
        assert_eq!(store.recent_completions(10), vec![completion.clone()]);

        let restored = SessionInputStore::persistent_at(path);
        assert_eq!(restored.recent_completions(10), vec![completion]);
    }

    #[test]
    fn session_input_store_clears_stale_input_with_recovery_evidence() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("session-inputs.json");
        let store = SessionInputStore::persistent_at(path.clone());
        store.push(SessionInputRecord {
            id: "input-stale".into(),
            session_id: "session-1".into(),
            message: "continue but owner disappeared".into(),
            received_at_ms: 10,
        });

        let cleared = store
            .clear_stale_with_record(" input-stale ", "operator cleared stale queued input")
            .expect("clear stale input");

        assert_eq!(cleared.input_id, "input-stale");
        assert_eq!(cleared.session_id, "session-1");
        assert_eq!(cleared.action, SessionInputCompletionAction::ClearedStale);
        assert_eq!(
            cleared.reason.as_deref(),
            Some("operator cleared stale queued input")
        );
        assert!(store.list().is_empty());

        let restored = SessionInputStore::persistent_at(path);
        assert_eq!(restored.recent_completions(10), vec![cleared]);
    }
}
