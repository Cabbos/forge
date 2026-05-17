use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::adapters::base::ChatMessage;
use crate::agent::turn_state::AgentTurnState;
use crate::protocol::events::DeliverySummary;
use crate::workflow::WorkflowState;

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
            latest_turn: None,
            latest_workflow: None,
            latest_delivery: None,
            updated_at_ms: now_ms(),
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
        assert!(restored.latest_turn.is_none());
        assert!(restored.latest_workflow.is_none());
        assert!(restored.latest_delivery.is_none());
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
}
