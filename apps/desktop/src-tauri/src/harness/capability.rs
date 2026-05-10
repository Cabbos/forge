use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Skill,
    Hook,
    McpServer,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub source: String, // "builtin" | "local" | "github:repo"
    pub kind: CapabilityKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventType {
    SessionStart,
    SessionStop,
    PreTool,
    PostTool,
    CapabilityChanged,
}

#[derive(Debug, Clone)]
pub enum Event {
    SessionStart { session_id: String, working_dir: String },
    SessionStop { session_id: String },
    PreTool { session_id: String, tool_name: String, input: serde_json::Value },
    PostTool { session_id: String, tool_name: String, result: String },
    CapabilityChanged { capability_id: String, action: String },
}

#[async_trait]
pub trait Capability: Send + Sync {
    fn id(&self) -> &str;
    fn metadata(&self) -> &CapabilityMetadata;
    fn enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn subscribed_events(&self) -> Vec<EventType> { vec![] }
    async fn on_event(&self, _event: &Event) -> Result<(), String> { Ok(()) }
}
