use serde::{Deserialize, Serialize};

/// Tool type for creating sessions (PTY-based CLI tools).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    Claude,
    Codex,
    Hermes,
    Bash,
}

/// Agent type for API-based AI sessions.
/// Mirrors ToolType but semantically distinguishes API-driven agents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Claude,
    Codex,
    Hermes,
}

impl std::str::FromStr for ToolType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "claude" => Ok(ToolType::Claude),
            "codex" => Ok(ToolType::Codex),
            "hermes" => Ok(ToolType::Hermes),
            "bash" => Ok(ToolType::Bash),
            _ => Err(format!("Unknown tool type: {}", s)),
        }
    }
}

/// Response for create_session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreated {
    pub session_id: String,
}

/// Session summary for list_sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub tool_type: String,
    pub status: String,
    pub created_at: String,
}

/// Signal type for send_signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    Interrupt,
    Terminate,
}
