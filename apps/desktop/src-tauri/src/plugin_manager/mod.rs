pub mod installer;
pub mod presets;
pub mod registry;
pub mod scanner;

use serde::{Deserialize, Serialize};

/// Standardized description of a plugin/skill/extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub plugin_type: PluginType,
    pub agent: AgentTarget,
    pub category: String,
    pub status: PluginStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginType {
    McpServer,
    Hook,
    Skill,
    Extension,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentTarget {
    Claude,
    Codex,
    Hermes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginStatus {
    NotInstalled,
    Installed { enabled: bool },
    Installing,
    Error { message: String },
}
