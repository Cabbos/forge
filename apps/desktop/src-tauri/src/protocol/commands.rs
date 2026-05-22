use serde::{Deserialize, Serialize};

use crate::protocol::events::DeliverySummary;
use crate::workflow::WorkflowState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreated {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub missing_api_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub created_at: String,
    pub working_dir: Option<String>,
    pub created_at_ms: Option<u64>,
    pub updated_at_ms: Option<u64>,
    pub context_window_tokens: Option<u32>,
    pub latest_workflow: Option<WorkflowState>,
    pub latest_delivery: Option<DeliverySummary>,
}
