use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    Preference,
    ProjectFact,
    Decision,
    TaskState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Session,
    UserProfile,
    Project,
    Document,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    Candidate,
    Accepted,
    Pinned,
    Forgotten,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WikiMemory {
    pub id: String,
    pub category: MemoryCategory,
    pub scope: MemoryScope,
    pub status: MemoryStatus,
    pub title: String,
    pub body: String,
    pub project_path: Option<String>,
    pub source_session_id: Option<String>,
    pub source_message_ids: Vec<String>,
    pub confidence: f32,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: Option<String>,
    pub use_count: u32,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectedContextMemory {
    pub memory_id: String,
    pub title: String,
    pub body: String,
    pub category: MemoryCategory,
    pub scope: MemoryScope,
    pub score: f32,
    pub reason: String,
    pub injected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryPatch {
    pub title: Option<String>,
    pub body: Option<String>,
    pub status: Option<MemoryStatus>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryListFilter {
    pub scope: Option<MemoryScope>,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    /// Memory is Forgotten.
    Forgotten,
    /// Memory is Archived.
    Archived,
    /// Memory has a project_path that doesn't match the active project.
    ProjectMismatch,
    /// UserProfile memory contains task-like instruction content (e.g. "我想做一个番茄钟").
    TaskLikeGlobalPreference,
    /// The user message was too low-signal to trigger memory injection (e.g. "继续", "好的").
    LowSignalQuery,
    /// No relevance signals matched between message and memory.
    NoRelevanceSignal,
    /// Final computed score was zero or negative.
    ScoreBelowThreshold,
    /// Project-scoped TaskState/Decision memory has no project_path — orphan.
    OrphanProjectMemory,
    /// Candidate memory with confidence below threshold.
    LowConfidenceCandidate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RejectedMemory {
    pub memory_id: String,
    pub title: String,
    pub scope: MemoryScope,
    pub category: MemoryCategory,
    pub project_path: Option<String>,
    pub reason: RejectReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemorySelectionAudit {
    pub selected: Vec<SelectedContextMemory>,
    pub rejected: Vec<RejectedMemory>,
}
