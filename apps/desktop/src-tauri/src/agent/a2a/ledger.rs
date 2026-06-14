use std::fs;
use std::path::{Path, PathBuf};

use crate::agent::a2a::bus::AgentA2ABus;

pub(crate) fn save_session_ledger_at(
    root: &Path,
    session_id: &str,
    bus: &AgentA2ABus,
) -> Result<(), String> {
    let path = ledger_path_at(root, session_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create A2A ledger dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(bus)
        .map_err(|e| format!("serialize A2A ledger for '{session_id}': {e}"))?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, json.as_bytes())
        .map_err(|e| format!("write A2A ledger tmp for '{session_id}': {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("replace A2A ledger for '{session_id}': {e}"))?;
    Ok(())
}

pub(crate) fn load_session_ledger_at(
    root: &Path,
    session_id: &str,
) -> Result<Option<AgentA2ABus>, String> {
    let path = ledger_path_at(root, session_id)?;
    if !path.exists() {
        return Ok(None);
    }
    let json =
        fs::read_to_string(&path).map_err(|e| format!("read A2A ledger '{session_id}': {e}"))?;
    let bus = serde_json::from_str::<AgentA2ABus>(&json)
        .map_err(|e| format!("parse A2A ledger '{session_id}': {e}"))?;
    Ok(Some(bus))
}

pub(crate) fn delete_session_ledger_at(root: &Path, session_id: &str) -> Result<(), String> {
    let path = ledger_path_at(root, session_id)?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("delete A2A ledger '{session_id}': {e}"))?;
    }
    Ok(())
}

fn ledger_path_at(root: &Path, session_id: &str) -> Result<PathBuf, String> {
    if !is_safe_session_id(session_id) {
        return Err("Invalid session id".to_string());
    }
    Ok(root.join("a2a").join(format!("{session_id}.json")))
}

fn is_safe_session_id(session_id: &str) -> bool {
    let sanitized: String = session_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    !sanitized.is_empty() && sanitized == session_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::a2a::types::{AgentExecutionMode, AgentRole};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "forge-a2a-ledger-{prefix}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    #[test]
    fn ledger_roundtrips_bus_state() {
        let root = temp_root("roundtrip");
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Inspect runtime",
            "Read worker files",
            10,
        );
        bus.start_task(&task_id, 20);

        save_session_ledger_at(&root, "session-1", &bus).expect("save ledger");
        let restored = load_session_ledger_at(&root, "session-1")
            .expect("load ledger")
            .expect("ledger exists");

        assert_eq!(restored.tasks.len(), 1);
        assert_eq!(restored.messages.len(), 2);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ledger_path_rejects_unsafe_session_ids() {
        let root = temp_root("safety");

        assert!(ledger_path_at(&root, "session-1").is_ok());
        assert!(ledger_path_at(&root, "../session-1").is_err());
        assert!(ledger_path_at(&root, "session/1").is_err());
        assert!(ledger_path_at(&root, "session 1").is_err());
        assert!(ledger_path_at(&root, "").is_err());
    }
}
