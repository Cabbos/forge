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
pub(crate) enum AgentArtifactKind {
    Evidence,
    PatchProposal,
    TestReport,
    DiffSummary,
    Commit,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentTaskRecord {
    pub task_id: AgentTaskId,
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
}
