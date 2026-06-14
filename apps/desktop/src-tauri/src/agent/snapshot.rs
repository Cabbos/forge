use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::adapters::base::ChatMessage;
use crate::agent::a2a::bus::AgentA2ABus;
use crate::agent::goal_state::GoalLedger;
use crate::agent::turn_state::AgentTurnState;
use crate::harness::write_boundary::WriteBoundary;
use crate::protocol::events::DeliverySummary;
use crate::workflow::WorkflowState;

const MAX_LISTED_SESSION_SNAPSHOTS: usize = 200;
const CURRENT_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
const LEGACY_SNAPSHOT_SCHEMA_VERSION: u32 = 0;

fn current_schema_version() -> u32 {
    CURRENT_SNAPSHOT_SCHEMA_VERSION
}

fn legacy_schema_version() -> u32 {
    LEGACY_SNAPSHOT_SCHEMA_VERSION
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentSessionSnapshot {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub working_dir: String,
    pub messages: Vec<ChatMessage>,
    pub summary: Option<String>,
    pub context_window_tokens: Option<u32>,
    #[serde(default)]
    pub latest_turn: Option<AgentTurnState>,
    #[serde(default)]
    pub latest_workflow: Option<WorkflowState>,
    #[serde(default)]
    pub latest_delivery: Option<DeliverySummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_ledger: Option<GoalLedger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a2a_state: Option<AgentA2ABus>,
    #[serde(default = "now_ms")]
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default = "legacy_schema_version")]
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_confirms: Vec<PendingConfirmDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_tool_calls: Vec<ActiveToolCallDescriptor>,
}

/// Serializable descriptor for a pending confirmation that was interrupted
/// before the user responded. Stored in the snapshot so a resumed session can
/// replay the confirm prompt in the UI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PendingConfirmDescriptor {
    pub block_id: String,
    pub question: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary: Option<WriteBoundary>,
    pub created_at_ms: u64,
}

impl PendingConfirmDescriptor {
    pub fn new(block_id: String, question: String, kind: String, created_at_ms: u64) -> Self {
        Self {
            block_id,
            question,
            kind,
            boundary: None,
            created_at_ms,
        }
    }

    pub fn with_boundary(mut self, boundary: WriteBoundary) -> Self {
        self.boundary = Some(boundary);
        self
    }
}

/// Minimal status for an active tool call captured in a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveToolCallStatus {
    Started,
    AwaitingResult,
    TimedOut,
    Cancelled,
}

/// Serializable descriptor for a tool call that was in flight when the snapshot
/// was taken. Stored so a resumed session can decide whether to re-associate a
/// late result, time out, or cancel the call.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActiveToolCallDescriptor {
    pub block_id: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub started_at_ms: u64,
    pub status: ActiveToolCallStatus,
}

impl ActiveToolCallDescriptor {
    pub fn new(
        block_id: String,
        tool_name: String,
        tool_input: serde_json::Value,
        started_at_ms: u64,
    ) -> Self {
        Self {
            block_id,
            tool_name,
            tool_input,
            started_at_ms,
            status: ActiveToolCallStatus::Started,
        }
    }

    pub fn with_status(mut self, status: ActiveToolCallStatus) -> Self {
        self.status = status;
        self
    }
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
        let timestamp = now_ms();
        Self {
            session_id,
            provider,
            model,
            working_dir,
            messages,
            summary,
            context_window_tokens,
            latest_turn: None,
            latest_workflow: None,
            latest_delivery: None,
            goal_ledger: None,
            a2a_state: None,
            created_at_ms: timestamp,
            updated_at_ms: timestamp,
            schema_version: CURRENT_SNAPSHOT_SCHEMA_VERSION,
            pending_confirms: Vec::new(),
            active_tool_calls: Vec::new(),
        }
    }

    pub fn with_latest_turn(mut self, latest_turn: AgentTurnState) -> Self {
        self.latest_turn = Some(latest_turn);
        self.updated_at_ms = now_ms();
        self
    }

    pub fn with_latest_workflow(mut self, latest_workflow: WorkflowState) -> Self {
        self.latest_workflow = Some(latest_workflow);
        self.updated_at_ms = now_ms();
        self
    }

    pub fn with_latest_delivery(mut self, latest_delivery: DeliverySummary) -> Self {
        self.latest_delivery = Some(latest_delivery);
        self.updated_at_ms = now_ms();
        self
    }

    pub fn with_goal_ledger(mut self, goal_ledger: GoalLedger) -> Self {
        self.goal_ledger = Some(goal_ledger);
        self.updated_at_ms = now_ms();
        self
    }

    pub fn with_a2a_state(mut self, a2a_state: AgentA2ABus) -> Self {
        self.a2a_state = Some(a2a_state);
        self.updated_at_ms = now_ms();
        self
    }

    pub fn with_pending_confirms(
        mut self,
        pending_confirms: Vec<PendingConfirmDescriptor>,
    ) -> Self {
        self.pending_confirms = pending_confirms;
        self.updated_at_ms = now_ms();
        self
    }

    pub fn with_active_tool_calls(
        mut self,
        active_tool_calls: Vec<ActiveToolCallDescriptor>,
    ) -> Self {
        self.active_tool_calls = active_tool_calls;
        self.updated_at_ms = now_ms();
        self
    }
}

pub fn save_session_snapshot(snapshot: &AgentSessionSnapshot) -> Result<(), String> {
    save_session_snapshot_at(&app_data_dir(), snapshot)
}

fn save_session_snapshot_at(
    root: &std::path::Path,
    snapshot: &AgentSessionSnapshot,
) -> Result<(), String> {
    let path = snapshot_path_at(root, &snapshot.session_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create session snapshot dir: {e}"))?;
    }
    let mut snapshot = snapshot.clone();
    if let Ok(existing_json) = fs::read_to_string(&path) {
        if let Ok(existing) = serde_json::from_str::<AgentSessionSnapshot>(&existing_json) {
            if existing.session_id == snapshot.session_id {
                snapshot.created_at_ms = existing.created_at_ms;
            }
        }
    }
    let json = serde_json::to_string_pretty(&snapshot)
        .map_err(|e| format!("Failed to serialize session snapshot: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write session snapshot: {e}"))?;
    sync_a2a_ledger_at(root, &snapshot)?;
    Ok(())
}

pub fn load_session_snapshot(session_id: &str) -> Result<AgentSessionSnapshot, String> {
    load_session_snapshot_at(&app_data_dir(), session_id)
}

fn load_session_snapshot_at(
    root: &std::path::Path,
    session_id: &str,
) -> Result<AgentSessionSnapshot, String> {
    let path = snapshot_path_at(root, session_id)?;
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read saved session '{}': {e}", session_id))?;
    let mut snapshot: AgentSessionSnapshot = serde_json::from_str(&json)
        .map_err(|e| format!("Saved session '{}' is corrupted: {e}", session_id))?;
    if snapshot.session_id != session_id {
        return Err(format!(
            "Saved session '{}' has mismatched session id",
            session_id
        ));
    }
    if snapshot.a2a_state.is_none() {
        match crate::agent::a2a::ledger::load_session_ledger_at(root, session_id) {
            Ok(Some(bus)) => snapshot.a2a_state = Some(bus),
            Ok(None) => {}
            Err(error) => crate::app_log!("WARN", "[a2a_ledger] {error}"),
        }
    }
    Ok(snapshot)
}

pub fn delete_session_snapshot(session_id: &str) -> Result<(), String> {
    delete_session_snapshot_at(&app_data_dir(), session_id)
}

fn delete_session_snapshot_at(root: &Path, session_id: &str) -> Result<(), String> {
    let path = snapshot_path_at(root, session_id)?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete saved session '{}': {e}", session_id))?;
    }
    crate::agent::a2a::ledger::delete_session_ledger_at(root, session_id)?;
    Ok(())
}

fn sync_a2a_ledger_at(root: &Path, snapshot: &AgentSessionSnapshot) -> Result<(), String> {
    if let Some(bus) = &snapshot.a2a_state {
        crate::agent::a2a::ledger::save_session_ledger_at(root, &snapshot.session_id, bus)
    } else {
        crate::agent::a2a::ledger::delete_session_ledger_at(root, &snapshot.session_id)
    }
}

pub fn list_session_snapshots() -> Result<Vec<AgentSessionSnapshot>, String> {
    list_session_snapshots_from_dir(&app_data_dir())
}

fn list_session_snapshots_from_dir(
    root: &std::path::Path,
) -> Result<Vec<AgentSessionSnapshot>, String> {
    let sessions_dir = root.join("sessions");
    if !sessions_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&sessions_dir)
        .map_err(|e| format!("Failed to read session snapshot dir: {e}"))?;
    let mut snapshots = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        match load_listable_session_snapshot(&path) {
            Ok(snapshot) => snapshots.push(snapshot),
            Err(reason) => crate::app_log!(
                "WARN",
                "[session_snapshot] skipped {}: {}",
                path.display(),
                reason.as_str()
            ),
        }
    }
    snapshots.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.updated_at_ms));
    snapshots.truncate(MAX_LISTED_SESSION_SNAPSHOTS);
    Ok(snapshots)
}

#[derive(Debug, PartialEq, Eq)]
enum SnapshotListSkipReason {
    Unreadable,
    Corrupted,
    UnsafeOrMismatchedSessionId,
}

impl SnapshotListSkipReason {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Unreadable => "snapshot file is not readable",
            Self::Corrupted => "snapshot JSON is corrupted",
            Self::UnsafeOrMismatchedSessionId => {
                "snapshot file name and session id do not match safely"
            }
        }
    }
}

fn load_listable_session_snapshot(
    path: &Path,
) -> Result<AgentSessionSnapshot, SnapshotListSkipReason> {
    let json = fs::read_to_string(path).map_err(|_| SnapshotListSkipReason::Unreadable)?;
    let snapshot = serde_json::from_str::<AgentSessionSnapshot>(&json)
        .map_err(|_| SnapshotListSkipReason::Corrupted)?;
    if !snapshot_file_matches_session_id(path, &snapshot.session_id) {
        return Err(SnapshotListSkipReason::UnsafeOrMismatchedSessionId);
    }
    Ok(snapshot)
}

fn snapshot_file_matches_session_id(path: &Path, session_id: &str) -> bool {
    is_safe_session_id(session_id)
        && path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem == session_id)
}

fn snapshot_path(session_id: &str) -> Result<PathBuf, String> {
    snapshot_path_at(&app_data_dir(), session_id)
}

fn snapshot_path_at(root: &std::path::Path, session_id: &str) -> Result<PathBuf, String> {
    if !is_safe_session_id(session_id) {
        return Err("Invalid session id".to_string());
    }
    Ok(root.join("sessions").join(format!("{session_id}.json")))
}

fn is_safe_session_id(session_id: &str) -> bool {
    let id = safe_session_id(session_id);
    !id.is_empty() && id == session_id
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::turn_state::{AgentTurnState, AgentTurnStatus};
    use crate::workflow::{classify_workflow, WorkflowRoute};

    fn snapshot() -> AgentSessionSnapshot {
        AgentSessionSnapshot::new(
            "session-1".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "/workspace".to_string(),
            vec![ChatMessage::user("hello")],
            Some("summary".to_string()),
            Some(128_000),
        )
    }

    fn turn_state() -> AgentTurnState {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/workspace".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "agent-core".to_string(),
            "phase-1".to_string(),
            "Build turn state".to_string(),
        );
        turn.mark_status(AgentTurnStatus::Completed);
        turn
    }

    #[test]
    fn old_snapshot_json_without_latest_turn_deserializes() {
        let json = r#"{
            "session_id": "session-1",
            "provider": "openai",
            "model": "gpt-5",
            "working_dir": "/workspace",
            "messages": [],
            "summary": null,
            "context_window_tokens": null,
            "updated_at_ms": 123
        }"#;

        let restored: AgentSessionSnapshot =
            serde_json::from_str(json).expect("old snapshot should deserialize");

        assert_eq!(restored.session_id, "session-1");
        assert!(restored.created_at_ms > 0);
        assert!(restored.latest_turn.is_none());
        assert!(restored.latest_workflow.is_none());
        assert!(restored.latest_delivery.is_none());
    }

    #[test]
    fn lists_session_snapshots_from_directory_in_updated_order() {
        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-list-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let sessions_dir = root.join("sessions");
        fs::create_dir_all(&sessions_dir).expect("sessions dir");

        let mut older = snapshot();
        older.session_id = "older".to_string();
        older.updated_at_ms = 10;
        fs::write(
            sessions_dir.join("older.json"),
            serde_json::to_string(&older).expect("older json"),
        )
        .expect("write older");

        let mut newer = snapshot();
        newer.session_id = "newer".to_string();
        newer.updated_at_ms = 20;
        fs::write(
            sessions_dir.join("newer.json"),
            serde_json::to_string(&newer).expect("newer json"),
        )
        .expect("write newer");
        fs::write(sessions_dir.join("broken.json"), "{").expect("write broken");

        let listed = list_session_snapshots_from_dir(&root).expect("list snapshots");

        assert_eq!(
            listed
                .iter()
                .map(|snapshot| snapshot.session_id.as_str())
                .collect::<Vec<_>>(),
            vec!["newer", "older"]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_session_snapshots_caps_to_most_recent_entries() {
        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-list-cap-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let sessions_dir = root.join("sessions");
        fs::create_dir_all(&sessions_dir).expect("sessions dir");

        for index in 0..205 {
            let mut snapshot = snapshot();
            snapshot.session_id = format!("session-{index:03}");
            snapshot.updated_at_ms = index;
            fs::write(
                sessions_dir.join(format!("session-{index:03}.json")),
                serde_json::to_string(&snapshot).expect("snapshot json"),
            )
            .expect("write snapshot");
        }

        let listed = list_session_snapshots_from_dir(&root).expect("list snapshots");

        assert_eq!(listed.len(), 200);
        assert_eq!(
            listed.first().map(|snapshot| snapshot.updated_at_ms),
            Some(204)
        );
        assert_eq!(
            listed.last().map(|snapshot| snapshot.updated_at_ms),
            Some(5)
        );
        assert!(!listed
            .iter()
            .any(|snapshot| snapshot.session_id == "session-004"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_session_snapshots_skips_unsafe_or_mismatched_session_ids() {
        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-list-safety-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let sessions_dir = root.join("sessions");
        fs::create_dir_all(&sessions_dir).expect("sessions dir");

        let mut valid = snapshot();
        valid.session_id = "session-1".to_string();
        fs::write(
            sessions_dir.join("session-1.json"),
            serde_json::to_string(&valid).expect("valid json"),
        )
        .expect("write valid");

        let mut unsafe_id = snapshot();
        unsafe_id.session_id = "../session-1".to_string();
        fs::write(
            sessions_dir.join("unsafe.json"),
            serde_json::to_string(&unsafe_id).expect("unsafe json"),
        )
        .expect("write unsafe");

        let mut mismatched = snapshot();
        mismatched.session_id = "other-session".to_string();
        fs::write(
            sessions_dir.join("mismatched.json"),
            serde_json::to_string(&mismatched).expect("mismatched json"),
        )
        .expect("write mismatched");

        let listed = list_session_snapshots_from_dir(&root).expect("list snapshots");

        assert_eq!(
            listed
                .iter()
                .map(|snapshot| snapshot.session_id.as_str())
                .collect::<Vec<_>>(),
            vec!["session-1"]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn listable_snapshot_reports_skip_reasons_for_bad_files() {
        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-list-reasons-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let sessions_dir = root.join("sessions");
        fs::create_dir_all(&sessions_dir).expect("sessions dir");

        let corrupted_path = sessions_dir.join("corrupted.json");
        fs::write(&corrupted_path, "{").expect("write corrupted");
        assert_eq!(
            load_listable_session_snapshot(&corrupted_path).expect_err("corrupted snapshot"),
            SnapshotListSkipReason::Corrupted
        );

        let mut unsafe_id = snapshot();
        unsafe_id.session_id = "../session-1".to_string();
        let unsafe_path = sessions_dir.join("unsafe.json");
        fs::write(
            &unsafe_path,
            serde_json::to_string(&unsafe_id).expect("unsafe json"),
        )
        .expect("write unsafe");
        assert_eq!(
            load_listable_session_snapshot(&unsafe_path).expect_err("unsafe snapshot"),
            SnapshotListSkipReason::UnsafeOrMismatchedSessionId
        );

        let mut mismatched = snapshot();
        mismatched.session_id = "other-session".to_string();
        let mismatched_path = sessions_dir.join("mismatched.json");
        fs::write(
            &mismatched_path,
            serde_json::to_string(&mismatched).expect("mismatched json"),
        )
        .expect("write mismatched");
        assert_eq!(
            load_listable_session_snapshot(&mismatched_path).expect_err("mismatched snapshot"),
            SnapshotListSkipReason::UnsafeOrMismatchedSessionId
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn snapshot_path_rejects_session_ids_that_would_be_sanitized() {
        let root = std::env::temp_dir();

        assert!(snapshot_path_at(&root, "session-1").is_ok());
        assert!(snapshot_path_at(&root, "../session-1").is_err());
        assert!(snapshot_path_at(&root, "session/1").is_err());
        assert!(snapshot_path_at(&root, "session 1").is_err());
        assert!(snapshot_path_at(&root, "").is_err());
    }

    #[test]
    fn load_session_snapshot_rejects_mismatched_snapshot_session_id() {
        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-load-safety-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let sessions_dir = root.join("sessions");
        fs::create_dir_all(&sessions_dir).expect("sessions dir");

        let mut mismatched = snapshot();
        mismatched.session_id = "other-session".to_string();
        fs::write(
            sessions_dir.join("session-1.json"),
            serde_json::to_string(&mismatched).expect("mismatched json"),
        )
        .expect("write mismatched");

        let error = load_session_snapshot_at(&root, "session-1")
            .expect_err("mismatched snapshot should be rejected");

        assert!(error.contains("session id"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_session_snapshot_preserves_original_created_timestamp() {
        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-created-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));

        let mut first = snapshot();
        first.created_at_ms = 11;
        first.updated_at_ms = 12;
        save_session_snapshot_at(&root, &first).expect("save first");

        let mut second = snapshot();
        second.created_at_ms = 99;
        second.updated_at_ms = 100;
        save_session_snapshot_at(&root, &second).expect("save second");

        let restored = serde_json::from_str::<AgentSessionSnapshot>(
            &fs::read_to_string(root.join("sessions").join("session-1.json"))
                .expect("read saved snapshot"),
        )
        .expect("deserialize saved snapshot");

        assert_eq!(restored.created_at_ms, 11);
        assert_eq!(restored.updated_at_ms, 100);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_session_snapshot_ignores_mismatched_existing_created_timestamp() {
        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-created-safety-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let sessions_dir = root.join("sessions");
        fs::create_dir_all(&sessions_dir).expect("sessions dir");

        let mut existing = snapshot();
        existing.session_id = "other-session".to_string();
        existing.created_at_ms = 11;
        existing.updated_at_ms = 12;
        fs::write(
            sessions_dir.join("session-1.json"),
            serde_json::to_string(&existing).expect("existing json"),
        )
        .expect("write existing");

        let mut replacement = snapshot();
        replacement.session_id = "session-1".to_string();
        replacement.created_at_ms = 99;
        replacement.updated_at_ms = 100;
        save_session_snapshot_at(&root, &replacement).expect("save replacement");

        let restored = load_session_snapshot_at(&root, "session-1").expect("load replacement");

        assert_eq!(restored.created_at_ms, 99);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn snapshot_with_latest_turn_serializes_roundtrip() {
        let snapshot = snapshot().with_latest_turn(turn_state());

        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let restored: AgentSessionSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");

        let latest_turn = restored.latest_turn.expect("latest turn");
        assert_eq!(latest_turn.turn_id, "turn-1");
        assert_eq!(latest_turn.status, AgentTurnStatus::Completed);
    }

    #[test]
    fn snapshot_with_latest_workflow_serializes_roundtrip() {
        let workflow = classify_workflow("session-1", "新增一个功能", 42);
        let snapshot = snapshot().with_latest_workflow(workflow);

        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let restored: AgentSessionSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");

        let latest_workflow = restored.latest_workflow.expect("latest workflow");
        assert_eq!(latest_workflow.session_id, "session-1");
        assert_eq!(latest_workflow.route, WorkflowRoute::Workflow);
    }

    #[test]
    fn snapshot_with_latest_delivery_serializes_roundtrip() {
        let snapshot = snapshot().with_latest_delivery(crate::protocol::events::DeliverySummary {
            project_path: Some("/workspace".to_string()),
            preview_label: "预览运行中".to_string(),
            checkpoint_label: "检查点已就绪".to_string(),
            next_action: "下一步：交付状态可以继续验收。".to_string(),
            verification_label: Some("检查已通过".to_string()),
            verification_status: Some("passed".to_string()),
            verification_command: Some("npm run build".to_string()),
            record_label: Some("建议更新项目记录".to_string()),
            record_status: Some("pending".to_string()),
            record_target_pages: vec!["tasks.md".to_string(), "log.md".to_string()],
        });

        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let restored: AgentSessionSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");

        let latest_delivery = restored.latest_delivery.unwrap();
        assert_eq!(latest_delivery.preview_label, "预览运行中");
        assert_eq!(
            latest_delivery.record_label.as_deref(),
            Some("建议更新项目记录")
        );
        assert_eq!(latest_delivery.record_status.as_deref(), Some("pending"));
        assert_eq!(
            latest_delivery.record_target_pages,
            vec!["tasks.md", "log.md"]
        );
    }

    #[test]
    fn old_delivery_summary_json_without_record_fields_deserializes() {
        let json = r#"{
          "session_id":"session-1",
          "provider":"deepseek",
          "model":"deepseek-v4-flash",
          "working_dir":"/workspace",
          "messages":[],
          "summary":null,
          "context_window_tokens":null,
          "latest_turn":null,
          "latest_workflow":null,
          "latest_delivery":{
            "project_path":"/workspace",
            "preview_label":"预览运行中",
            "checkpoint_label":"检查点已就绪",
            "next_action":"下一步：交付状态可以继续验收。",
            "verification_label":"检查已通过",
            "verification_status":"passed",
            "verification_command":"npm run build"
          },
          "updated_at_ms":42
        }"#;

        let restored: AgentSessionSnapshot =
            serde_json::from_str(json).expect("deserialize old delivery snapshot");

        let latest_delivery = restored.latest_delivery.expect("latest delivery");
        assert_eq!(latest_delivery.record_label, None);
        assert_eq!(latest_delivery.record_status, None);
        assert!(latest_delivery.record_target_pages.is_empty());
    }

    #[test]
    fn old_snapshot_json_without_goal_ledger_deserializes() {
        let json = r#"{
            "session_id": "session-1",
            "provider": "openai",
            "model": "gpt-5",
            "working_dir": "/workspace",
            "messages": [],
            "summary": null,
            "context_window_tokens": null,
            "updated_at_ms": 123
        }"#;

        let restored: AgentSessionSnapshot = serde_json::from_str(json)
            .expect("old snapshot should deserialize without goal_ledger");

        assert_eq!(restored.session_id, "session-1");
        assert!(restored.goal_ledger.is_none());
    }

    #[test]
    fn snapshot_with_goal_ledger_roundtrips() {
        use crate::agent::goal_state::{GoalLedger, GoalStatus, GoalTaskStatus};

        let ledger = GoalLedger::new_active(
            "goal-1",
            "Ship feature",
            vec!["Task A".to_string(), "Task B".to_string()],
            10,
        );
        let snapshot = snapshot().with_goal_ledger(ledger);

        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let restored: AgentSessionSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");

        let goal = restored
            .goal_ledger
            .as_ref()
            .unwrap()
            .current_goal()
            .unwrap();
        assert_eq!(goal.id, "goal-1");
        assert_eq!(goal.objective, "Ship feature");
        assert_eq!(goal.status, GoalStatus::Active);
        assert_eq!(goal.tasks.len(), 2);
        assert_eq!(goal.tasks[0].status, GoalTaskStatus::Pending);
    }

    #[test]
    fn snapshot_goal_ledger_preserves_completed_and_blocked_goals() {
        use crate::agent::goal_state::{GoalLedger, GoalStatus, GoalTaskStatus};

        let mut completed_ledger =
            GoalLedger::new_active("goal-done", "Done goal", vec!["Step 1".to_string()], 10);
        completed_ledger.complete_active(20);

        let snapshot = snapshot().with_goal_ledger(completed_ledger);
        let json = serde_json::to_string(&snapshot).expect("serialize");
        let restored: AgentSessionSnapshot = serde_json::from_str(&json).expect("deserialize");

        let goal = restored
            .goal_ledger
            .as_ref()
            .unwrap()
            .current_goal()
            .unwrap();
        assert_eq!(goal.status, GoalStatus::Completed);
        assert_eq!(goal.closed_at_ms, Some(20));
        assert_eq!(goal.tasks[0].status, GoalTaskStatus::Completed);
    }

    #[test]
    fn old_snapshot_json_without_a2a_state_deserializes() {
        let json = r#"{
          "session_id":"s1",
          "provider":"anthropic",
          "model":"claude",
          "working_dir":"/tmp/project",
          "messages":[],
          "summary":null,
          "context_window_tokens":200000,
          "updated_at_ms":10
        }"#;

        let restored: AgentSessionSnapshot =
            serde_json::from_str(json).expect("old snapshot should deserialize without a2a_state");

        assert!(restored.a2a_state.is_none());
    }

    #[test]
    fn snapshot_with_a2a_state_roundtrips() {
        use crate::agent::a2a::bus::AgentA2ABus;
        use crate::agent::a2a::types::{AgentExecutionMode, AgentRole};

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Inspect A2A",
            "Read A2A files",
            10,
        );
        bus.start_task(&task_id, 20);
        bus.complete_task(&task_id, "done", 30);

        let snapshot = snapshot().with_a2a_state(bus);
        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let restored: AgentSessionSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");

        assert_eq!(restored.a2a_state.expect("a2a state").tasks.len(), 1);
    }

    #[test]
    fn save_session_snapshot_writes_a2a_ledger_sidecar() {
        use crate::agent::a2a::bus::AgentA2ABus;
        use crate::agent::a2a::types::{AgentExecutionMode, AgentRole};

        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-a2a-ledger-save-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("sessions")).expect("sessions dir");

        let mut bus = AgentA2ABus::default();
        bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement diagnostics",
            "Wire doctor repair action",
            10,
        );
        let snapshot = snapshot().with_a2a_state(bus);

        save_session_snapshot_at(&root, &snapshot).expect("save snapshot");

        let ledger_path = root.join("a2a").join("session-1.json");
        assert!(ledger_path.exists(), "a2a ledger sidecar should be saved");
        let ledger: AgentA2ABus =
            serde_json::from_str(&fs::read_to_string(&ledger_path).expect("read ledger"))
                .expect("parse ledger");
        assert_eq!(ledger.tasks.len(), 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_session_snapshot_recovers_a2a_state_from_ledger_sidecar() {
        use crate::agent::a2a::bus::AgentA2ABus;
        use crate::agent::a2a::types::{AgentExecutionMode, AgentRole};

        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-a2a-ledger-load-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("sessions")).expect("sessions dir");
        fs::create_dir_all(root.join("a2a")).expect("a2a dir");

        let snapshot = snapshot();
        fs::write(
            root.join("sessions").join("session-1.json"),
            serde_json::to_string_pretty(&snapshot).expect("serialize snapshot"),
        )
        .expect("write snapshot");

        let mut bus = AgentA2ABus::default();
        bus.assign_task(
            AgentRole::Reviewer,
            AgentExecutionMode::ReadOnly,
            "Review A2A ledger",
            "Check restore behavior",
            10,
        );
        fs::write(
            root.join("a2a").join("session-1.json"),
            serde_json::to_string_pretty(&bus).expect("serialize ledger"),
        )
        .expect("write ledger");

        let restored = load_session_snapshot_at(&root, "session-1").expect("load snapshot");

        let restored_bus = restored.a2a_state.expect("a2a state from ledger");
        assert_eq!(restored_bus.tasks.len(), 1);
        assert_eq!(restored_bus.messages.len(), 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn delete_session_snapshot_removes_a2a_ledger_sidecar() {
        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-a2a-ledger-delete-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("sessions")).expect("sessions dir");
        fs::create_dir_all(root.join("a2a")).expect("a2a dir");
        fs::write(root.join("sessions").join("session-1.json"), "{}").expect("write snapshot");
        fs::write(root.join("a2a").join("session-1.json"), "{}").expect("write ledger");

        delete_session_snapshot_at(&root, "session-1").expect("delete snapshot");

        assert!(!root.join("sessions").join("session-1.json").exists());
        assert!(!root.join("a2a").join("session-1.json").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn new_snapshot_serializes_current_schema_version() {
        let snapshot = snapshot();
        assert_eq!(snapshot.schema_version, CURRENT_SNAPSHOT_SCHEMA_VERSION);

        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let value: serde_json::Value = serde_json::from_str(&json).expect("parse snapshot json");
        assert_eq!(
            value
                .get("schema_version")
                .and_then(serde_json::Value::as_u64),
            Some(u64::from(CURRENT_SNAPSHOT_SCHEMA_VERSION))
        );
    }

    #[test]
    fn old_snapshot_json_without_schema_version_deserializes_to_legacy_version() {
        let json = r#"{
            "session_id": "session-1",
            "provider": "openai",
            "model": "gpt-5",
            "working_dir": "/workspace",
            "messages": [],
            "summary": null,
            "context_window_tokens": null,
            "updated_at_ms": 123
        }"#;

        let restored: AgentSessionSnapshot =
            serde_json::from_str(json).expect("old snapshot should deserialize");

        assert_eq!(restored.schema_version, LEGACY_SNAPSHOT_SCHEMA_VERSION);
    }

    #[test]
    fn pending_confirm_descriptor_roundtrips_without_boundary() {
        let descriptor = PendingConfirmDescriptor::new(
            "confirm-1".to_string(),
            "Allow write?".to_string(),
            "file_write".to_string(),
            42,
        );
        let snapshot = snapshot().with_pending_confirms(vec![descriptor]);

        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let restored: AgentSessionSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");

        assert_eq!(restored.pending_confirms.len(), 1);
        let restored_descriptor = &restored.pending_confirms[0];
        assert_eq!(restored_descriptor.block_id, "confirm-1");
        assert_eq!(restored_descriptor.question, "Allow write?");
        assert_eq!(restored_descriptor.kind, "file_write");
        assert_eq!(restored_descriptor.created_at_ms, 42);
        assert!(restored_descriptor.boundary.is_none());
    }

    #[test]
    fn pending_confirm_descriptor_roundtrips_with_boundary() {
        let boundary = WriteBoundary {
            title: "准备修改项目".to_string(),
            target_label: Some("target".to_string()),
            workspace_name: "workspace".to_string(),
            workspace_path: "/workspace".to_string(),
            operation: "写入文件".to_string(),
            affected_files: vec!["file.txt".to_string()],
            command: Some("cmd".to_string()),
            impact: "将修改 1 个文件".to_string(),
            risk: crate::harness::write_boundary::WriteBoundaryRisk::Caution,
            recovery: "可恢复".to_string(),
            checkpoint_status: Some("ready".to_string()),
            warning: Some("注意".to_string()),
        };
        let descriptor = PendingConfirmDescriptor::new(
            "confirm-2".to_string(),
            "Allow dangerous command?".to_string(),
            "dangerous_cmd".to_string(),
            100,
        )
        .with_boundary(boundary.clone());
        let snapshot = snapshot().with_pending_confirms(vec![descriptor]);

        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let restored: AgentSessionSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");

        let restored_descriptor = restored.pending_confirms.first().expect("pending confirm");
        assert_eq!(restored_descriptor.block_id, "confirm-2");
        assert_eq!(
            restored_descriptor
                .boundary
                .as_ref()
                .unwrap()
                .workspace_name,
            "workspace"
        );
        assert_eq!(
            restored_descriptor.boundary.as_ref().unwrap().risk,
            crate::harness::write_boundary::WriteBoundaryRisk::Caution
        );
    }

    #[test]
    fn active_tool_call_descriptor_roundtrips() {
        let tool_input = serde_json::json!({"path": "file.txt", "content": "hello"});
        let descriptor = ActiveToolCallDescriptor::new(
            "tool-1".to_string(),
            "write_to_file".to_string(),
            tool_input.clone(),
            200,
        )
        .with_status(ActiveToolCallStatus::AwaitingResult);
        let snapshot = snapshot().with_active_tool_calls(vec![descriptor]);

        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let restored: AgentSessionSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");

        assert_eq!(restored.active_tool_calls.len(), 1);
        let restored_descriptor = &restored.active_tool_calls[0];
        assert_eq!(restored_descriptor.block_id, "tool-1");
        assert_eq!(restored_descriptor.tool_name, "write_to_file");
        assert_eq!(restored_descriptor.tool_input, tool_input);
        assert_eq!(restored_descriptor.started_at_ms, 200);
        assert_eq!(
            restored_descriptor.status,
            ActiveToolCallStatus::AwaitingResult
        );
    }

    #[test]
    fn old_snapshot_json_without_descriptor_fields_defaults_empty() {
        let json = r#"{
            "session_id": "session-1",
            "provider": "openai",
            "model": "gpt-5",
            "working_dir": "/workspace",
            "messages": [],
            "summary": null,
            "context_window_tokens": null,
            "updated_at_ms": 123
        }"#;

        let restored: AgentSessionSnapshot =
            serde_json::from_str(json).expect("old snapshot should deserialize");

        assert!(restored.pending_confirms.is_empty());
        assert!(restored.active_tool_calls.is_empty());
    }

    // ── Phase 1.8: Agent snapshot shape roundtrip ──────────────────────

    /// Helper: build a realistic snapshot that exercises every field including
    /// multi-role ChatMessage history (user, assistant with tool_use blocks,
    /// system, tool), workflow, delivery, goal ledger, and A2A state.
    fn realistic_agent_snapshot() -> AgentSessionSnapshot {
        let messages = vec![
            ChatMessage::system("你是一个编程助手"),
            ChatMessage::user("帮我读取文件"),
            ChatMessage::assistant(serde_json::json!([
                {"type": "text", "text": "让我读取文件内容"},
                {"type": "tool_use", "id": "call-1", "name": "read_file", "input": {"path": "src/main.rs"}}
            ])),
            ChatMessage::tool("call-1", "fn main() { println!(\"hello\"); }"),
            ChatMessage::user("再写一个测试"),
            ChatMessage::assistant(serde_json::json!([
                {"type": "text", "text": "好的，我来写测试"},
                {"type": "tool_use", "id": "call-2", "name": "write_to_file", "input": {"path": "test.rs", "content": "#[test]\nfn it_works() {}"}}
            ])),
            ChatMessage::tool("call-2", "File written: test.rs"),
        ];

        let mut turn = turn_state();
        turn.mark_status(AgentTurnStatus::Completed);

        let workflow = classify_workflow("session-agent-roundtrip", "实现新功能", 42);

        let delivery = crate::protocol::events::DeliverySummary {
            project_path: Some("/workspace".to_string()),
            preview_label: "预览运行中".to_string(),
            checkpoint_label: "检查点已就绪".to_string(),
            next_action: "下一步：交付状态可以继续验收。".to_string(),
            verification_label: Some("检查已通过".to_string()),
            verification_status: Some("passed".to_string()),
            verification_command: Some("npm run build".to_string()),
            record_label: Some("建议更新项目记录".to_string()),
            record_status: Some("pending".to_string()),
            record_target_pages: vec!["tasks.md".to_string()],
        };

        use crate::agent::goal_state::GoalLedger;
        let ledger = GoalLedger::new_active(
            "goal-1",
            "实现新功能",
            vec!["任务 A".to_string(), "任务 B".to_string()],
            10,
        );

        use crate::agent::a2a::bus::AgentA2ABus;
        use crate::agent::a2a::types::{AgentExecutionMode, AgentRole};
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "调研 A2A",
            "阅读 A2A 文件",
            10,
        );
        bus.start_task(&task_id, 20);

        AgentSessionSnapshot::new(
            "session-agent-roundtrip".to_string(),
            "deepseek".to_string(),
            "deepseek-v4-flash[1m]".to_string(),
            "/workspace".to_string(),
            messages,
            Some("摘要：已完成文件读取和测试编写".to_string()),
            Some(1_000_000),
        )
        .with_latest_turn(turn)
        .with_latest_workflow(workflow)
        .with_latest_delivery(delivery)
        .with_goal_ledger(ledger)
        .with_a2a_state(bus)
    }

    #[test]
    fn realistic_agent_snapshot_survives_save_load_roundtrip() {
        let snapshot = realistic_agent_snapshot();
        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-agent-roundtrip-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let sessions_dir = root.join("sessions");
        fs::create_dir_all(&sessions_dir).expect("sessions dir");

        save_session_snapshot_at(&root, &snapshot).expect("save snapshot");
        let restored =
            load_session_snapshot_at(&root, "session-agent-roundtrip").expect("load snapshot");

        // ── Identity fields ──
        assert_eq!(restored.session_id, "session-agent-roundtrip");
        assert_eq!(restored.provider, "deepseek");
        assert_eq!(restored.model, "deepseek-v4-flash[1m]");
        assert_eq!(restored.working_dir, "/workspace");
        assert_eq!(restored.schema_version, CURRENT_SNAPSHOT_SCHEMA_VERSION);

        // ── Messages preserve roles and content shape ──
        assert_eq!(restored.messages.len(), 7);
        assert_eq!(restored.messages[0].role, "system");
        assert_eq!(restored.messages[1].role, "user");
        assert_eq!(restored.messages[2].role, "assistant");
        // assistant message should have structured content blocks
        let blocks = restored.messages[2]
            .content
            .as_array()
            .expect("structured blocks");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[1]["type"], "tool_use");
        assert_eq!(blocks[1]["id"], "call-1");
        assert_eq!(blocks[1]["name"], "read_file");

        assert_eq!(restored.messages[3].role, "tool");
        let tool_content = &restored.messages[3].content;
        assert_eq!(tool_content["tool_call_id"].as_str(), Some("call-1"));
        assert!(tool_content["content"]
            .as_str()
            .unwrap_or("")
            .contains("hello"));

        assert_eq!(restored.messages[4].role, "user");
        assert_eq!(restored.messages[5].role, "assistant");
        assert_eq!(restored.messages[6].role, "tool");

        // ── Summary and context ──
        assert_eq!(
            restored.summary.as_deref(),
            Some("摘要：已完成文件读取和测试编写")
        );
        assert_eq!(restored.context_window_tokens, Some(1_000_000));

        // ── Turn state ──
        let latest_turn = restored.latest_turn.expect("latest turn");
        assert_eq!(latest_turn.turn_id, "turn-1");
        assert_eq!(latest_turn.status, AgentTurnStatus::Completed);

        // ── Workflow ──
        let latest_workflow = restored.latest_workflow.expect("latest workflow");
        assert_eq!(latest_workflow.session_id, "session-agent-roundtrip");
        assert!(
            matches!(
                latest_workflow.route,
                WorkflowRoute::Workflow | WorkflowRoute::Direct | WorkflowRoute::Recovery
            ),
            "workflow route should be preserved: {:?}",
            latest_workflow.route
        );

        // ── Delivery ──
        let latest_delivery = restored.latest_delivery.expect("latest delivery");
        assert_eq!(latest_delivery.preview_label, "预览运行中");
        assert_eq!(latest_delivery.record_target_pages, vec!["tasks.md"]);

        // ── Goal ledger ──
        let goal = restored
            .goal_ledger
            .as_ref()
            .expect("goal ledger")
            .current_goal()
            .expect("current goal");
        assert_eq!(goal.id, "goal-1");
        assert_eq!(goal.objective, "实现新功能");

        // ── A2A state ──
        assert_eq!(restored.a2a_state.expect("a2a state").tasks.len(), 1);

        let _ = fs::remove_dir_all(root);
    }

    // ── Phase 1.8: Provider alias snapshot compatibility ───────────────

    #[test]
    fn snapshot_preserves_raw_provider_without_normalizing_aliases() {
        // Snapshots store the provider as-written. Normalisation (e.g.
        // "claude" → "anthropic") is applied during session creation/restore
        // in session_lifecycle and handlers, NOT by the snapshot layer.
        let raw_providers = vec!["claude", "anthropic", "gpt", "openai", "deepseek", "custom"];

        for raw_provider in &raw_providers {
            let mut snapshot = snapshot();
            snapshot.session_id = format!("provider-{}", sanitize_provider_key(raw_provider));
            snapshot.provider = raw_provider.to_string();

            let json = serde_json::to_string(&snapshot).expect("serialize");
            let restored: AgentSessionSnapshot = serde_json::from_str(&json).expect("deserialize");

            assert_eq!(
                restored.provider, *raw_provider,
                "snapshot must preserve raw provider '{raw_provider}' without normalisation"
            );
        }
    }

    fn sanitize_provider_key(provider: &str) -> String {
        provider
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    // ── Phase 1.8: Multi-descriptor pending confirm list ────────────────

    #[test]
    fn pending_confirm_descriptor_list_preserves_ordering_and_mixed_boundaries() {
        let boundary = || WriteBoundary {
            title: "确认写入".to_string(),
            target_label: Some("target.txt".to_string()),
            workspace_name: "workspace".to_string(),
            workspace_path: "/workspace".to_string(),
            operation: "写入文件".to_string(),
            affected_files: vec!["target.txt".to_string()],
            command: None,
            impact: "将会写入 1 个文件".to_string(),
            risk: crate::harness::write_boundary::WriteBoundaryRisk::Caution,
            recovery: "可通过 git 回滚".to_string(),
            checkpoint_status: Some("ready".to_string()),
            warning: None,
        };

        let descriptors = vec![
            PendingConfirmDescriptor::new(
                "confirm-1".to_string(),
                "First question?".to_string(),
                "file_write".to_string(),
                10,
            )
            .with_boundary(boundary()),
            PendingConfirmDescriptor::new(
                "confirm-2".to_string(),
                "Second question?".to_string(),
                "dangerous_cmd".to_string(),
                20,
            ), // no boundary
            PendingConfirmDescriptor::new(
                "confirm-3".to_string(),
                "Third question?".to_string(),
                "file_write".to_string(),
                30,
            )
            .with_boundary(boundary()),
            PendingConfirmDescriptor::new(
                "confirm-4".to_string(),
                "Fourth question?".to_string(),
                "ask_user".to_string(),
                40,
            ), // no boundary
        ];

        let snapshot = snapshot().with_pending_confirms(descriptors);

        let json = serde_json::to_string(&snapshot).expect("serialize");
        let restored: AgentSessionSnapshot = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(
            restored.pending_confirms.len(),
            4,
            "all four confirm descriptors should survive roundtrip"
        );

        // Verify ordering is preserved
        let ids: Vec<&str> = restored
            .pending_confirms
            .iter()
            .map(|d| d.block_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["confirm-1", "confirm-2", "confirm-3", "confirm-4"]
        );

        // Verify boundary presence / absence
        assert!(restored.pending_confirms[0].boundary.is_some());
        assert!(restored.pending_confirms[1].boundary.is_none());
        assert!(restored.pending_confirms[2].boundary.is_some());
        assert!(restored.pending_confirms[3].boundary.is_none());

        // Verify boundary content for a bounded descriptor
        let b0 = restored.pending_confirms[0].boundary.as_ref().unwrap();
        assert_eq!(b0.workspace_name, "workspace");
        assert_eq!(
            b0.risk,
            crate::harness::write_boundary::WriteBoundaryRisk::Caution
        );

        // Verify the unbounded descriptor still has correct fields
        let d1 = &restored.pending_confirms[1];
        assert_eq!(d1.question, "Second question?");
        assert_eq!(d1.kind, "dangerous_cmd");
        assert_eq!(d1.created_at_ms, 20);
    }

    // ── Phase 1.8: Corruption rejection ─────────────────────────────────

    #[test]
    fn load_session_snapshot_at_rejects_corrupted_json() {
        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-corrupted-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let sessions_dir = root.join("sessions");
        fs::create_dir_all(&sessions_dir).expect("sessions dir");

        // Truncated JSON
        fs::write(
            sessions_dir.join("broken.json"),
            "{ \"session_id\": \"broken\", \"prov",
        )
        .expect("write truncated json");

        let err =
            load_session_snapshot_at(&root, "broken").expect_err("corrupted JSON should fail");
        assert!(
            err.contains("corrupted"),
            "error should mention corruption: {err}"
        );

        // Malformed JSON with extra trailing garbage
        fs::write(
            sessions_dir.join("garbage.json"),
            r#"{"session_id":"garbage","provider":"x"}}}"#,
        )
        .expect("write garbage json");

        let err2 =
            load_session_snapshot_at(&root, "garbage").expect_err("garbage JSON should fail");
        assert!(
            err2.contains("corrupted"),
            "error should mention corruption: {err2}"
        );

        let _ = fs::remove_dir_all(root);
    }

    // ── Phase 1.8: Unsafe session ID rejection pre-filesystem ───────────

    #[test]
    fn load_session_snapshot_at_rejects_unsafe_session_id_before_filesystem_access() {
        let root = std::env::temp_dir().join(format!(
            "forge-snapshot-unsafe-id-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        // Don't create sessions dir — rejection should happen before any
        // filesystem traversal.
        let err = load_session_snapshot_at(&root, "../etc-passwd")
            .expect_err("unsafe session id should be rejected");
        assert!(
            err.contains("session id") || err.contains("Invalid"),
            "error should mention invalid session id: {err}"
        );

        let err2 =
            load_session_snapshot_at(&root, "").expect_err("empty session id should be rejected");
        assert!(
            err2.contains("session id") || err2.contains("Invalid"),
            "error should mention invalid session id: {err2}"
        );

        let _ = fs::remove_dir_all(root);
    }

    // ── Phase 1.8: Future/unknown schema version behavior ───────────────

    #[test]
    fn snapshot_with_future_schema_version_is_accepted_and_preserves_version() {
        // Current implementation accepts future schema versions without
        // rejection — it is the caller's responsibility (startup restore,
        // list) to decide whether to skip or fall back. This test documents
        // that behaviour so future maintainers know the contract.
        let json = r#"{
            "session_id": "session-future",
            "provider": "deepseek",
            "model": "deepseek-future",
            "working_dir": "/workspace",
            "messages": [],
            "summary": null,
            "context_window_tokens": null,
            "schema_version": 999,
            "updated_at_ms": 123
        }"#;

        let restored: AgentSessionSnapshot =
            serde_json::from_str(json).expect("future schema should deserialize");

        assert_eq!(restored.schema_version, 999);
        assert_eq!(restored.session_id, "session-future");
    }

    // ── Phase 1.8: Legacy schema without all optional fields ────────────

    #[test]
    fn legacy_schema_without_any_optional_fields_deserializes_with_defaults() {
        let json = r#"{
            "session_id": "legacy-1",
            "provider": "openai",
            "model": "gpt-4",
            "working_dir": "/tmp",
            "messages": [],
            "summary": null,
            "context_window_tokens": null,
            "updated_at_ms": 100
        }"#;

        let restored: AgentSessionSnapshot =
            serde_json::from_str(json).expect("legacy snapshot should deserialize");

        assert_eq!(restored.schema_version, LEGACY_SNAPSHOT_SCHEMA_VERSION);
        assert!(restored.latest_turn.is_none());
        assert!(restored.latest_workflow.is_none());
        assert!(restored.latest_delivery.is_none());
        assert!(restored.goal_ledger.is_none());
        assert!(restored.a2a_state.is_none());
        assert!(restored.pending_confirms.is_empty());
        assert!(restored.active_tool_calls.is_empty());
        assert!(restored.created_at_ms > 0); // defaulted to now
    }
}
