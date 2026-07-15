use serde::{Deserialize, Serialize};

use crate::loop_runtime::LoopUsageLedger;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentA2AChildEventKind {
    Assigned,
    LeaseClaimed,
    Started,
    Progress,
    FileFact,
    PatchProposed,
    WaitingReview,
    Completed,
    Failed,
    Abandoned,
    Recovered,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentA2AChildRuntimeEvent {
    pub kind: AgentA2AChildEventKind,
    pub label: String,
    pub detail: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentA2AChildCapsule {
    pub capsule_id: String,
    pub parent_task_id: String,
    pub child_task_id: String,
    pub child_goal: String,
    pub status: String,
    pub artifact_titles: Vec<String>,
    pub changed_files: Vec<String>,
    pub review_decision: Option<String>,
    pub failure_reason: Option<String>,
    pub next_action: String,
    pub estimated_tokens: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentA2AReviewGateKind {
    Approved,
    ChangesRequested,
    Rejected,
    StaleReview,
    WrongParent,
    MissingEvidence,
    WaitingReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentA2AReviewGateProjection {
    pub kind: AgentA2AReviewGateKind,
    pub label: String,
    pub reason: String,
    pub completion_impact: String,
    pub parent_task_id: Option<String>,
    pub child_task_id: String,
    pub reviewed_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentA2ARecoveryActionKind {
    Retry,
    Abandon,
    Reassign,
    InspectWorktree,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentA2ARecoveryActionSuggestion {
    pub action: AgentA2ARecoveryActionKind,
    pub label: String,
    pub reason: String,
    pub requires_human_approval: bool,
    pub retryable: bool,
    pub next_attempt: Option<u32>,
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_decision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_at_ms: Option<u64>,
    // Phase 4-A enriched fields — derived from AgentTaskRecord / artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub child_task_ids: Vec<String>,
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
    // A2A runtime contract — compact, replayable child event facts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_events: Vec<AgentA2AChildRuntimeEvent>,
    // Parent-consumable summaries of direct child tasks. These intentionally avoid
    // embedding full child transcripts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub child_capsules: Vec<AgentA2AChildCapsule>,
    // A2A Review Gate V2 — typed task-local review facts. Parent approval is not implied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_gate: Option<AgentA2AReviewGateProjection>,
    // A2A Failure Recovery — suggestions only; callers must explicitly execute commands.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recovery_actions: Vec<AgentA2ARecoveryActionSuggestion>,
    // Phase 4-B — diff-derived file visibility (safe: parsed from existing artifacts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed_file_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_report_excerpt: Option<String>,
    // Task 5 — boundary-level telemetry, derived from worktree metadata artifacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_io_events: Vec<AgentFileIoEventProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_ledger: Option<LoopUsageLedger>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentA2AMessageProjection {
    pub message_id: String,
    pub kind: String,
    pub content: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentFileIoEventProjection {
    pub path: String,
    pub operation: String,
}
