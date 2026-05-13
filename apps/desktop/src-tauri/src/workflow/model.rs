use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRoute {
    Direct,
    Light,
    Workflow,
    StrictWorkflow,
    Recovery,
    Verification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPhase {
    Idle,
    Classifying,
    Clarifying,
    Designing,
    Spec,
    Planning,
    Executing,
    Debugging,
    Verifying,
    Done,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGate {
    None,
    Soft,
    ApprovalRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowOverrideAction {
    Direct,
    PlanFirst,
    Debug,
    Verify,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowState {
    pub session_id: String,
    pub route: WorkflowRoute,
    pub phase: WorkflowPhase,
    pub beginner_label: String,
    pub developer_label: String,
    pub matched_signals: Vec<String>,
    pub reason: String,
    pub gate: WorkflowGate,
    pub override_actions: Vec<WorkflowOverrideAction>,
    pub spec_path: Option<String>,
    pub plan_path: Option<String>,
    pub checkpoint_id: Option<String>,
    pub updated_at: u64,
}
