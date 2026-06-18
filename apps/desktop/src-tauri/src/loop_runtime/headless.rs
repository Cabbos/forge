use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeadlessResumeMode {
    #[default]
    Disabled,
    RequireHumanApproval,
    ApprovedForTask,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeadlessResumeApproval {
    pub task_id: String,
    pub approved_by: String,
    pub approved_at_ms: u64,
    pub scope: String,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeadlessAgentLease {
    pub task_id: String,
    pub session_id: String,
    pub lease_id: String,
    pub owner_pid: u32,
    pub expires_at_ms: u64,
}
