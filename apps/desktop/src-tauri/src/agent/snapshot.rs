use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::adapters::base::ChatMessage;
use crate::agent::goal_state::GoalLedger;
use crate::agent::turn_state::AgentTurnState;
use crate::protocol::events::DeliverySummary;
use crate::workflow::WorkflowState;

const MAX_LISTED_SESSION_SNAPSHOTS: usize = 200;

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
    #[serde(default = "now_ms")]
    pub created_at_ms: u64,
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
            created_at_ms: timestamp,
            updated_at_ms: timestamp,
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
    let snapshot: AgentSessionSnapshot = serde_json::from_str(&json)
        .map_err(|e| format!("Saved session '{}' is corrupted: {e}", session_id))?;
    if snapshot.session_id != session_id {
        return Err(format!(
            "Saved session '{}' has mismatched session id",
            session_id
        ));
    }
    Ok(snapshot)
}

pub fn delete_session_snapshot(session_id: &str) -> Result<(), String> {
    let path = snapshot_path(session_id)?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete saved session '{}': {e}", session_id))?;
    }
    Ok(())
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
}
