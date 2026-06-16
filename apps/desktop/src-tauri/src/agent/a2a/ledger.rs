use std::fs;
use std::path::{Path, PathBuf};

use crate::agent::a2a::bus::AgentA2ABus;
use crate::agent::a2a::projection::AgentA2AProjection;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AgentA2ASessionProjection {
    pub session_id: String,
    pub projection: AgentA2AProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AgentA2ALedgerLoadError {
    pub session_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AgentA2AProjectionList {
    pub states: Vec<AgentA2ASessionProjection>,
    pub load_errors: Vec<AgentA2ALedgerLoadError>,
}

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

pub(crate) fn save_session_ledger(session_id: &str, bus: &AgentA2ABus) -> Result<(), String> {
    save_session_ledger_at(&app_data_dir(), session_id, bus)
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

pub(crate) fn load_session_ledger(session_id: &str) -> Result<Option<AgentA2ABus>, String> {
    load_session_ledger_at(&app_data_dir(), session_id)
}

pub(crate) fn load_session_projection(
    session_id: &str,
) -> Result<Option<AgentA2AProjection>, String> {
    load_session_projection_at(&app_data_dir(), session_id)
}

pub(crate) fn load_session_projection_at(
    root: &Path,
    session_id: &str,
) -> Result<Option<AgentA2AProjection>, String> {
    Ok(load_session_ledger_at(root, session_id)?.map(|bus| bus.projection()))
}

pub(crate) fn list_session_projections() -> Result<AgentA2AProjectionList, String> {
    list_session_projections_at(&app_data_dir())
}

pub(crate) fn list_session_projections_at(root: &Path) -> Result<AgentA2AProjectionList, String> {
    let ledger_dir = root.join("a2a");
    if !ledger_dir.exists() {
        return Ok(AgentA2AProjectionList::default());
    }

    let entries = fs::read_dir(&ledger_dir).map_err(|e| format!("read A2A ledger dir: {e}"))?;
    let mut list = AgentA2AProjectionList::default();
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(session_id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !is_safe_session_id(session_id) {
            list.load_errors.push(AgentA2ALedgerLoadError {
                session_id: session_id.to_string(),
                message: "Invalid session id".to_string(),
            });
            continue;
        }
        match load_session_projection_at(root, session_id) {
            Ok(Some(projection)) => list.states.push(AgentA2ASessionProjection {
                session_id: session_id.to_string(),
                projection,
            }),
            Ok(None) => {}
            Err(message) => list.load_errors.push(AgentA2ALedgerLoadError {
                session_id: session_id.to_string(),
                message,
            }),
        }
    }
    list.states
        .sort_by(|left, right| left.session_id.cmp(&right.session_id));
    list.load_errors
        .sort_by(|left, right| left.session_id.cmp(&right.session_id));
    Ok(list)
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

fn app_data_dir() -> PathBuf {
    home_dir().join(".forge")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
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

    #[test]
    fn load_session_projection_returns_projected_ledger_state() {
        let root = temp_root("projection");
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement worker query",
            "Expose A2A ledger state",
            10,
        );
        bus.start_task(&task_id, 20);
        save_session_ledger_at(&root, "session-1", &bus).expect("save ledger");

        let projection = load_session_projection_at(&root, "session-1")
            .expect("load projection")
            .expect("projection exists");

        assert_eq!(projection.running_count, 1);
        assert_eq!(projection.tasks.len(), 1);
        assert_eq!(projection.tasks[0].execution_mode, "worktree_worker");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_session_projection_returns_none_when_ledger_is_missing() {
        let root = temp_root("projection-missing");

        let projection =
            load_session_projection_at(&root, "session-missing").expect("missing is ok");

        assert!(projection.is_none());
    }

    #[test]
    fn list_session_projections_reports_corrupt_ledgers_without_dropping_valid_ones() {
        let root = temp_root("projection-list");
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Inspect query",
            "Read A2A state",
            10,
        );
        bus.complete_task(&task_id, "done", 20);
        save_session_ledger_at(&root, "session-good", &bus).expect("save ledger");
        fs::write(root.join("a2a").join("session-bad.json"), "{").expect("write bad ledger");

        let list = list_session_projections_at(&root).expect("list projections");

        assert_eq!(list.states.len(), 1);
        assert_eq!(list.states[0].session_id, "session-good");
        assert_eq!(list.states[0].projection.completed_count, 1);
        assert_eq!(list.load_errors.len(), 1);
        assert_eq!(list.load_errors[0].session_id, "session-bad");

        let _ = fs::remove_dir_all(&root);
    }
}
