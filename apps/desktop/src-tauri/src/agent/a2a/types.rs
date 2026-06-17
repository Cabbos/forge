use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct AgentTaskId(String);

impl AgentTaskId {
    pub(crate) fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct AgentId(String);

impl AgentId {
    pub(crate) fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentRole {
    Researcher,
    Reviewer,
    TestPlanner,
    Implementer,
}

impl AgentRole {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Researcher => "researcher",
            Self::Reviewer => "reviewer",
            Self::TestPlanner => "test_planner",
            Self::Implementer => "implementer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentExecutionMode {
    ReadOnly,
    PatchProposal,
    WorktreeWorker,
}

impl AgentExecutionMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::PatchProposal => "patch_proposal",
            Self::WorktreeWorker => "worktree_worker",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentTaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl AgentTaskStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PatchRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PatchProposal {
    pub file_path: String,
    pub intent: String,
    pub diff_summary: String,
    pub original_snippet: String,
    pub proposed_snippet: String,
    pub risk_level: PatchRiskLevel,
    pub test_suggestion: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentArtifactKind {
    Evidence,
    PatchProposal,
    TestReport,
    DiffSummary,
    Commit,
}

impl AgentArtifactKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Evidence => "evidence",
            Self::PatchProposal => "patch_proposal",
            Self::TestReport => "test_report",
            Self::DiffSummary => "diff_summary",
            Self::Commit => "commit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentPermissionSet {
    pub execution_mode: AgentExecutionMode,
    pub allow_read_files: bool,
    pub allow_web: bool,
    pub allow_git_diff: bool,
    pub allow_workspace_write: bool,
    pub allow_shell: bool,
    pub allow_delegate: bool,
}

impl AgentPermissionSet {
    pub(crate) fn for_mode(execution_mode: AgentExecutionMode) -> Self {
        match execution_mode {
            AgentExecutionMode::ReadOnly => Self {
                execution_mode,
                allow_read_files: true,
                allow_web: true,
                allow_git_diff: true,
                allow_workspace_write: false,
                allow_shell: false,
                allow_delegate: false,
            },
            AgentExecutionMode::PatchProposal => Self {
                execution_mode,
                allow_read_files: true,
                allow_web: true,
                allow_git_diff: true,
                allow_workspace_write: false,
                allow_shell: false,
                allow_delegate: false,
            },
            AgentExecutionMode::WorktreeWorker => Self {
                execution_mode,
                allow_read_files: true,
                allow_web: true,
                allow_git_diff: true,
                allow_workspace_write: true,
                allow_shell: true,
                allow_delegate: false,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentArtifact {
    pub artifact_id: String,
    pub task_id: AgentTaskId,
    pub kind: AgentArtifactKind,
    pub title: String,
    pub content: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentTaskFailure {
    pub kind: String,
    pub message: String,
    pub retryable: bool,
    pub created_at_ms: u64,
}

pub(crate) fn default_max_task_attempts() -> u32 {
    3
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentTaskRecord {
    pub task_id: AgentTaskId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<AgentTaskId>,
    pub agent_id: AgentId,
    pub role: AgentRole,
    pub execution_mode: AgentExecutionMode,
    pub title: String,
    pub prompt: String,
    pub status: AgentTaskStatus,
    pub permissions: AgentPermissionSet,
    pub artifacts: Vec<AgentArtifact>,
    pub failure: Option<AgentTaskFailure>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub ended_at_ms: Option<u64>,
    pub resume_note: Option<String>,
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
    #[serde(default = "default_max_task_attempts")]
    pub max_attempts: u32,
}

impl AgentTaskRecord {
    pub(crate) fn new(
        task_id: AgentTaskId,
        agent_id: AgentId,
        role: AgentRole,
        execution_mode: AgentExecutionMode,
        title: impl Into<String>,
        prompt: impl Into<String>,
        timestamp_ms: u64,
    ) -> Self {
        let permissions = AgentPermissionSet::for_mode(execution_mode.clone());
        Self {
            task_id,
            parent_task_id: None,
            agent_id,
            role,
            execution_mode,
            title: title.into(),
            prompt: prompt.into(),
            status: AgentTaskStatus::Pending,
            permissions,
            artifacts: Vec::new(),
            failure: None,
            created_at_ms: timestamp_ms,
            updated_at_ms: timestamp_ms,
            started_at_ms: None,
            ended_at_ms: None,
            resume_note: None,
            lease_owner: None,
            lease_acquired_at_ms: None,
            lease_expires_at_ms: None,
            last_heartbeat_at_ms: None,
            attempt_count: 0,
            max_attempts: default_max_task_attempts(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentMessageKind {
    TaskAssigned,
    Started,
    Progress,
    Evidence,
    ArtifactCreated,
    FinalResult,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentMessage {
    pub message_id: String,
    pub task_id: AgentTaskId,
    pub agent_id: AgentId,
    pub kind: AgentMessageKind,
    pub content: String,
    pub created_at_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_mode_serializes_snake_case_and_reserves_worktree_worker() {
        let mode = AgentExecutionMode::WorktreeWorker;

        let json = serde_json::to_string(&mode).expect("serialize mode");

        assert_eq!(json, r#""worktree_worker""#);
    }

    #[test]
    fn task_record_defaults_to_read_only_permissions() {
        let record = AgentTaskRecord::new(
            AgentTaskId::new("task-1"),
            AgentId::new("agent-1"),
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Inspect compact flow",
            "Find where compact triggers",
            10,
        );

        assert_eq!(record.status, AgentTaskStatus::Pending);
        assert_eq!(
            record.permissions.execution_mode,
            AgentExecutionMode::ReadOnly
        );
        assert!(record.permissions.allow_read_files);
        assert!(!record.permissions.allow_workspace_write);
        assert!(!record.permissions.allow_shell);
        assert!(!record.permissions.allow_delegate);
    }

    #[test]
    fn task_record_deserializes_legacy_json_without_parent_task_id() {
        let record = AgentTaskRecord::new(
            AgentTaskId::new("task-legacy"),
            AgentId::new("agent-legacy"),
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Legacy task",
            "Inspect old ledger",
            10,
        );
        let mut value = serde_json::to_value(record).expect("serialize task record");
        value
            .as_object_mut()
            .expect("task record object")
            .remove("parent_task_id");

        let restored: AgentTaskRecord =
            serde_json::from_value(value).expect("deserialize legacy task record");

        assert_eq!(restored.parent_task_id, None);
    }

    #[test]
    fn patch_proposal_serializes_and_roundtrips() {
        let proposal = PatchProposal {
            file_path: "src/main.rs".to_string(),
            intent: "Add error handling".to_string(),
            diff_summary: "Wrap handle() with Result".to_string(),
            original_snippet: "fn handle() {}".to_string(),
            proposed_snippet: "fn handle() -> Result<()> {}".to_string(),
            risk_level: PatchRiskLevel::Medium,
            test_suggestion: "Test error propagation".to_string(),
            confidence: 0.85,
        };

        let json = serde_json::to_string(&proposal).expect("serialize");
        assert!(json.contains("\"risk_level\":\"medium\""));
        assert!(json.contains("\"confidence\":0.85"));

        let restored: PatchProposal = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.file_path, "src/main.rs");
        assert_eq!(restored.risk_level, PatchRiskLevel::Medium);
        assert!((restored.confidence - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn patch_risk_level_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&PatchRiskLevel::High).unwrap(),
            r#""high""#
        );
    }
}
