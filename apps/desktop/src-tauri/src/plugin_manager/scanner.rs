use std::path::PathBuf;

use super::{AgentTarget, PluginEntry, PluginStatus, PluginType};

/// Scan local filesystem for installed MCP servers / hooks / skills / plugins.
pub struct PluginScanner;

impl PluginScanner {
    /// Scan for plugins installed for a specific agent.
    pub fn scan(agent: &AgentTarget) -> Vec<PluginEntry> {
        let home = dirs::home_dir().unwrap_or_default();
        let mut entries = Vec::new();

        match agent {
            AgentTarget::Claude => {
                // Scan ~/.claude/ directory
                let claude_dir = home.join(".claude");
                if claude_dir.exists() {
                    Self::scan_claude_mcp(&claude_dir, &mut entries);
                    Self::scan_claude_hooks(&claude_dir, &mut entries);
                    Self::scan_claude_skills(&claude_dir, &mut entries);
                }
            }
            AgentTarget::Codex => {
                let codex_dir = home.join(".codex");
                if codex_dir.exists() {
                    Self::scan_codex_plugins(&codex_dir, &mut entries);
                }
            }
            AgentTarget::Hermes => {
                // Hermes — to be determined
            }
        }

        entries
    }

    fn scan_claude_mcp(claude_dir: &PathBuf, entries: &mut Vec<PluginEntry>) {
        // Check settings.json for MCP servers
        let settings = claude_dir.join("settings.json");
        if let Ok(content) = std::fs::read_to_string(&settings) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(mcp_servers) = json.get("mcpServers") {
                    if let Some(obj) = mcp_servers.as_object() {
                        for (name, config) in obj {
                            entries.push(PluginEntry {
                                id: format!("mcp__{}", name),
                                name: name.clone(),
                                description: config["description"]
                                    .as_str()
                                    .unwrap_or("MCP Server")
                                    .to_string(),
                                plugin_type: PluginType::McpServer,
                                agent: AgentTarget::Claude,
                                category: "tools".to_string(),
                                status: PluginStatus::Installed { enabled: true },
                                config_schema: None,
                                current_config: Some(config.clone()),
                                homepage: None,
                                author: None,
                            });
                        }
                    }
                }
            }
        }

        // Also check standalone mcp.json
        let mcp_json = claude_dir.join("mcp.json");
        if let Ok(content) = std::fs::read_to_string(&mcp_json) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(mcp_servers) = json.get("mcpServers").or_else(|| json.as_object().map(|_| &json)) {
                    if let Some(obj) = mcp_servers.as_object() {
                        for (name, config) in obj {
                            let id = format!("mcp__{}", name);
                            if !entries.iter().any(|e| e.id == id) {
                                entries.push(PluginEntry {
                                    id,
                                    name: name.clone(),
                                    description: config["description"]
                                        .as_str()
                                        .unwrap_or("MCP Server")
                                        .to_string(),
                                    plugin_type: PluginType::McpServer,
                                    agent: AgentTarget::Claude,
                                    category: "tools".to_string(),
                                    status: PluginStatus::Installed { enabled: true },
                                    config_schema: None,
                                    current_config: Some(config.clone()),
                                    homepage: None,
                                    author: None,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    fn scan_claude_hooks(claude_dir: &PathBuf, entries: &mut Vec<PluginEntry>) {
        let settings = claude_dir.join("settings.json");
        if let Ok(content) = std::fs::read_to_string(&settings) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(hooks) = json.get("hooks") {
                    if let Some(obj) = hooks.as_object() {
                        for (name, config) in obj {
                            entries.push(PluginEntry {
                                id: format!("hook__{}", name),
                                name: name.clone(),
                                description: config["description"]
                                    .as_str()
                                    .unwrap_or("Hook")
                                    .to_string(),
                                plugin_type: PluginType::Hook,
                                agent: AgentTarget::Claude,
                                category: "automation".to_string(),
                                status: PluginStatus::Installed { enabled: true },
                                config_schema: None,
                                current_config: Some(config.clone()),
                                homepage: None,
                                author: None,
                            });
                        }
                    }
                }
            }
        }
    }

    fn scan_claude_skills(claude_dir: &PathBuf, entries: &mut Vec<PluginEntry>) {
        let skills_dir = claude_dir.join("skills");
        if !skills_dir.exists() {
            return;
        }
        if let Ok(read_dir) = std::fs::read_dir(&skills_dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    entries.push(PluginEntry {
                        id: format!("skill__{}", name),
                        name: name.to_string(),
                        description: format!("Skill: {}", name),
                        plugin_type: PluginType::Skill,
                        agent: AgentTarget::Claude,
                        category: "skill".to_string(),
                        status: PluginStatus::Installed { enabled: true },
                        config_schema: None,
                        current_config: None,
                        homepage: None,
                        author: None,
                    });
                }
            }
        }
    }

    fn scan_codex_plugins(codex_dir: &PathBuf, entries: &mut Vec<PluginEntry>) {
        let plugins_dir = codex_dir.join("plugins");
        if !plugins_dir.exists() {
            return;
        }
        if let Ok(read_dir) = std::fs::read_dir(&plugins_dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let is_dir = path.is_dir();
                entries.push(PluginEntry {
                    id: format!("codex__{}", name),
                    name: name.to_string(),
                    description: if is_dir { "Codex Plugin" } else { "Codex Extension" }
                        .to_string(),
                    plugin_type: if is_dir {
                        PluginType::Extension
                    } else {
                        PluginType::Extension
                    },
                    agent: AgentTarget::Codex,
                    category: "tools".to_string(),
                    status: PluginStatus::Installed { enabled: true },
                    config_schema: None,
                    current_config: None,
                    homepage: None,
                    author: None,
                });
            }
        }
    }
}

/// Minimal home directory lookup without adding a dep.
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        #[cfg(target_os = "linux")]
        {
            std::env::var("HOME").ok().map(PathBuf::from)
        }
        #[cfg(target_os = "macos")]
        {
            std::env::var("HOME").ok().map(PathBuf::from)
        }
        #[cfg(target_os = "windows")]
        {
            std::env::var("USERPROFILE").ok().map(PathBuf::from)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            None
        }
    }
}
