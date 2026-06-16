use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentA2AProjection {
    pub running_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub interrupted_count: usize,
    pub tasks: Vec<AgentA2ATaskProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentA2ATaskProjection {
    pub task_id: String,
    pub agent_id: String,
    pub role: String,
    pub execution_mode: String,
    pub status: String,
    pub title: String,
    pub messages: Vec<AgentA2AMessageProjection>,
    pub latest_message: Option<String>,
    pub failure_message: Option<String>,
    pub updated_at_ms: u64,
    pub artifact_count: usize,
    pub latest_artifact_kind: Option<String>,
    pub latest_artifact_title: Option<String>,
    // WorktreeWorker-specific metadata (populated when the latest artifact is worktree metadata).
    pub needs_human_review: Option<bool>,
    pub reason_codes: Vec<String>,
    pub tests_passed: Option<bool>,
    pub diff_truncated: Option<bool>,
    pub worktree_path: Option<String>,
    pub cleaned_up: Option<bool>,
    pub suggested_action: Option<String>,
    // Phase 4-A enriched fields — derived from AgentTaskRecord / artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    #[serde(default)]
    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_progress: Option<String>,
    // Phase 4-C — durable WorktreeWorker lease / retry state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_acquired_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_expires_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at_ms: Option<u64>,
    #[serde(default)]
    pub attempt_count: u32,
    #[serde(default)]
    pub max_attempts: u32,
    // Phase 4-B — diff-derived file visibility (safe: parsed from existing artifacts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed_file_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_report_excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentA2AMessageProjection {
    pub message_id: String,
    pub kind: String,
    pub content: String,
    pub created_at_ms: u64,
}
