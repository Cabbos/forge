use std::path::PathBuf;

use super::{AgentTarget, PluginEntry, PluginType};

/// Handles installing, uninstalling, and toggling plugins by modifying
/// the appropriate agent config files.
pub struct PluginInstaller;

impl PluginInstaller {
    /// Install a plugin by writing its config to the agent's config file.
    pub fn install(plugin: &PluginEntry) -> Result<(), String> {
        match plugin.agent {
            AgentTarget::Claude => Self::install_claude(plugin),
            AgentTarget::Codex => Self::install_codex(plugin),
            AgentTarget::Hermes => Self::install_hermes(plugin),
        }
    }

    /// Uninstall a plugin by removing its config.
    pub fn uninstall(plugin: &PluginEntry) -> Result<(), String> {
        match plugin.agent {
            AgentTarget::Claude => Self::uninstall_claude(plugin),
            AgentTarget::Codex => Self::uninstall_codex(plugin),
            AgentTarget::Hermes => Self::uninstall_hermes(plugin),
        }
    }

    /// Toggle a plugin enabled/disabled.
    pub fn toggle(plugin_id: &str, agent: &AgentTarget, enabled: bool) -> Result<(), String> {
        match agent {
            AgentTarget::Claude => Self::toggle_claude(plugin_id, enabled),
            AgentTarget::Codex => Self::toggle_codex(plugin_id, enabled),
            AgentTarget::Hermes => Self::toggle_hermes(plugin_id, enabled),
        }
    }

    // ── Claude ──

    fn claude_settings_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".claude").join("settings.json")
    }

    fn read_claude_settings() -> Result<(PathBuf, serde_json::Value), String> {
        let path = Self::claude_settings_path();
        if path.exists() {
            let content =
                std::fs::read_to_string(&path).map_err(|e| format!("Read error: {}", e))?;
            let json: serde_json::Value =
                serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
            Ok((path, json))
        } else {
            // Create parent dir and empty settings
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create .claude dir: {}", e))?;
            }
            Ok((path, serde_json::json!({})))
        }
    }

    fn write_claude_settings(json: &serde_json::Value) -> Result<(), String> {
        let path = Self::claude_settings_path();
        let content = serde_json::to_string_pretty(json).map_err(|e| format!("Serialize: {}", e))?;
        std::fs::write(&path, content).map_err(|e| format!("Write error: {}", e))
    }

    fn install_claude(plugin: &PluginEntry) -> Result<(), String> {
        let (_, mut settings) = Self::read_claude_settings()?;

        match plugin.plugin_type {
            PluginType::McpServer => {
                let mcp = settings
                    .as_object_mut()
                    .ok_or("Invalid settings.json")?
                    .entry("mcpServers")
                    .or_insert_with(|| serde_json::json!({}));

                let name = plugin.name.clone();
                let config = plugin.current_config.clone().unwrap_or(serde_json::json!({
                    "command": "npx",
                    "args": ["-y", &format!("@anthropic/mcp-server-{}", plugin.name.to_lowercase().replace(' ', "-"))],
                }));
                mcp[name] = config;
            }
            PluginType::Hook => {
                let hooks = settings
                    .as_object_mut()
                    .ok_or("Invalid settings.json")?
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}));
                hooks[&plugin.name] = plugin
                    .current_config
                    .clone()
                    .unwrap_or(serde_json::json!({"enabled": true}));
            }
            PluginType::Skill => {
                // Skills are typically just directories — ensure the skills dir exists
                let home = std::env::var("HOME").unwrap_or_default();
                let skills_dir = PathBuf::from(home)
                    .join(".claude")
                    .join("skills")
                    .join(&plugin.name);
                std::fs::create_dir_all(&skills_dir)
                    .map_err(|e| format!("Failed to create skill dir: {}", e))?;
            }
            PluginType::Extension => {
                return Err("Extension type not supported for Claude".to_string());
            }
        }

        Self::write_claude_settings(&settings)
    }

    fn uninstall_claude(plugin: &PluginEntry) -> Result<(), String> {
        let (_, mut settings) = Self::read_claude_settings()?;

        match plugin.plugin_type {
            PluginType::McpServer => {
                if let Some(mcp) = settings.get_mut("mcpServers") {
                    if let Some(obj) = mcp.as_object_mut() {
                        obj.remove(&plugin.name);
                    }
                }
            }
            PluginType::Hook => {
                if let Some(hooks) = settings.get_mut("hooks") {
                    if let Some(obj) = hooks.as_object_mut() {
                        obj.remove(&plugin.name);
                    }
                }
            }
            _ => {}
        }

        Self::write_claude_settings(&settings)
    }

    fn toggle_claude(plugin_id: &str, enabled: bool) -> Result<(), String> {
        let (_, mut settings) = Self::read_claude_settings()?;

        // Try MCP servers
        if let Some(mcp) = settings.get_mut("mcpServers") {
            if let Some(obj) = mcp.as_object_mut() {
                for (name, config) in obj.iter_mut() {
                    let id = format!("mcp__{}", name);
                    if id == plugin_id {
                        config["enabled"] = serde_json::Value::Bool(enabled);
                        return Self::write_claude_settings(&settings);
                    }
                }
            }
        }

        // Try hooks
        if let Some(hooks) = settings.get_mut("hooks") {
            if let Some(obj) = hooks.as_object_mut() {
                for (name, config) in obj.iter_mut() {
                    let id = format!("hook__{}", name);
                    if id == plugin_id {
                        config["enabled"] = serde_json::Value::Bool(enabled);
                        return Self::write_claude_settings(&settings);
                    }
                }
            }
        }

        Err(format!("Plugin not found: {}", plugin_id))
    }

    // ── Codex (placeholder) ──
    fn install_codex(plugin: &PluginEntry) -> Result<(), String> {
        let _ = plugin;
        Err("Codex plugin installation not yet implemented".to_string())
    }

    fn uninstall_codex(plugin: &PluginEntry) -> Result<(), String> {
        let _ = plugin;
        Err("Codex plugin uninstallation not yet implemented".to_string())
    }

    fn toggle_codex(plugin_id: &str, enabled: bool) -> Result<(), String> {
        let _ = (plugin_id, enabled);
        Err("Codex plugin toggle not yet implemented".to_string())
    }

    // ── Hermes (placeholder) ──
    fn install_hermes(plugin: &PluginEntry) -> Result<(), String> {
        let _ = plugin;
        Err("Hermes plugin installation not yet implemented".to_string())
    }

    fn uninstall_hermes(plugin: &PluginEntry) -> Result<(), String> {
        let _ = plugin;
        Err("Hermes plugin uninstallation not yet implemented".to_string())
    }

    fn toggle_hermes(plugin_id: &str, enabled: bool) -> Result<(), String> {
        let _ = (plugin_id, enabled);
        Err("Hermes plugin toggle not yet implemented".to_string())
    }
}
