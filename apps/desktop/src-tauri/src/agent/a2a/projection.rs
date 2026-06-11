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
    pub latest_message: Option<String>,
    pub failure_message: Option<String>,
    pub updated_at_ms: u64,
}
