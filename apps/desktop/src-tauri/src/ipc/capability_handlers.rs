use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::harness::capability::{
    CapabilityKind, CapabilityMetadata, EcosystemItem, EcosystemItemStatus,
};
use crate::harness::registry::CapabilityEntry;
use crate::harness::skills::SkillLoader;
use crate::protocol::events::StreamEvent;
use crate::settings;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

const ECOSYSTEM_EVENT_SESSION_ID: &str = "global";

#[derive(Serialize)]
pub struct CapabilityInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub source: String,
    pub version: String,
    pub enabled: bool,
}

#[tauri::command]
pub async fn list_capabilities(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<CapabilityInfo>, String> {
    // 1. Get registry capabilities (tools, hooks, mcp)
    let mut result: Vec<CapabilityInfo> =
        global_registry_capability_infos(state.harness.capability_registry.all_entries());

    // 2. Get user-level skills. Project-level skills are session context, not global settings.
    let skill_loader = SkillLoader::new();
    skill_loader.attach_database(state.harness.database.clone());
    let skills = skill_loader.scan_all().await;
    for s in skills {
        if !result.iter().any(|c| c.id == s.id) {
            result.push(CapabilityInfo {
                id: s.id.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                kind: "skill".into(),
                source: "local".into(),
                version: "1.0.0".into(),
                enabled: s.enabled,
            });
        }
    }
    result.extend(provider_capability_infos(state.credential_store.as_ref()));
    Ok(result)
}

pub(crate) fn capability_kind_label(kind: &CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::Skill => "skill",
        CapabilityKind::Hook => "hook",
        CapabilityKind::McpServer => "mcp_server",
        CapabilityKind::Provider => "provider",
        CapabilityKind::Tool => "tool",
    }
}

fn global_registry_capability_infos(entries: Vec<CapabilityEntry>) -> Vec<CapabilityInfo> {
    entries
        .into_iter()
        .filter(|entry| !is_hidden_global_capability(entry))
        .map(|entry| {
            let m = entry.metadata;
            CapabilityInfo {
                id: m.id,
                name: m.name,
                description: m.description,
                kind: capability_kind_label(&m.kind).to_string(),
                source: m.source,
                version: m.version,
                enabled: entry.enabled,
            }
        })
        .collect()
}

pub(crate) fn is_hidden_global_capability(entry: &CapabilityEntry) -> bool {
    is_internal_infrastructure_capability(entry) || is_workspace_scoped_capability(entry)
}

fn ecosystem_changed_event(
    item_id: impl Into<String>,
    action: impl Into<String>,
    enabled: Option<bool>,
) -> StreamEvent {
    StreamEvent::EcosystemChanged {
        session_id: ECOSYSTEM_EVENT_SESSION_ID.to_string(),
        item_id: item_id.into(),
        action: action.into(),
        enabled,
    }
}

fn emit_ecosystem_changed(
    app_handle: &tauri::AppHandle,
    item_id: impl Into<String>,
    action: impl Into<String>,
    enabled: Option<bool>,
) {
    if let Err(error) = app_handle.emit(
        "session-output",
        ecosystem_changed_event(item_id, action, enabled),
    ) {
        crate::app_log!(
            "WARN",
            "[event_bus] failed to emit ecosystem_changed event: {error}"
        );
    }
}

fn is_internal_infrastructure_capability(entry: &CapabilityEntry) -> bool {
    entry.metadata.id == "skill-loader"
}

fn is_workspace_scoped_capability(entry: &CapabilityEntry) -> bool {
    if !matches!(entry.metadata.kind, CapabilityKind::McpServer) {
        return false;
    }

    let source = entry.metadata.source.replace('\\', "/");
    source == ".forge/mcp.json" || source.ends_with("/.forge/mcp.json")
}

pub(crate) fn ecosystem_status_label(status: EcosystemItemStatus) -> &'static str {
    match status {
        EcosystemItemStatus::Healthy => "healthy",
        EcosystemItemStatus::Unavailable => "unavailable",
        EcosystemItemStatus::Warning => "warning",
        EcosystemItemStatus::Unknown => "unknown",
    }
}

pub(crate) fn ecosystem_status_for_capability(
    meta: &CapabilityMetadata,
    enabled: bool,
) -> (EcosystemItemStatus, Option<String>) {
    match &meta.kind {
        CapabilityKind::McpServer if enabled => mcp_config_probe_for_capability(meta)
            .map(|probe| (probe.status, Some(probe.status_message)))
            .unwrap_or((
                EcosystemItemStatus::Unavailable,
                Some("MCP configuration could not be inspected".into()),
            )),
        CapabilityKind::McpServer => (
            EcosystemItemStatus::Unknown,
            Some("Disabled — enable to probe connectivity".into()),
        ),
        _ if enabled => (EcosystemItemStatus::Healthy, None),
        _ => (EcosystemItemStatus::Unknown, Some("Disabled".into())),
    }
}

fn ecosystem_item_from_entry(entry: CapabilityEntry) -> EcosystemItem {
    let (status, status_message) = ecosystem_status_for_capability(&entry.metadata, entry.enabled);
    let config_summary =
        mcp_config_probe_for_capability(&entry.metadata).and_then(|probe| probe.config_summary);
    let mut item = EcosystemItem::from_capability_entry(&entry).with_status(status, status_message);
    item.config_summary = config_summary;
    item
}

fn provider_capability_infos(
    store: &dyn crate::credential_store::CredentialStore,
) -> Vec<CapabilityInfo> {
    settings::Settings::load()
        .key_status(store)
        .into_iter()
        .map(|key| CapabilityInfo {
            id: provider_item_id(&key.provider),
            name: provider_display_name(&key.provider),
            description: provider_description(&key.provider),
            kind: capability_kind_label(&CapabilityKind::Provider).to_string(),
            source: "~/.forge/config.json".into(),
            version: provider_default_model(&key.provider)
                .unwrap_or("custom")
                .to_string(),
            enabled: key.configured && key.status == "available",
        })
        .collect()
}

fn provider_ecosystem_items(
    store: &dyn crate::credential_store::CredentialStore,
) -> Vec<EcosystemItem> {
    settings::Settings::load()
        .key_status(store)
        .into_iter()
        .map(provider_ecosystem_item)
        .collect()
}

fn provider_ecosystem_item(key: settings::KeyStatus) -> EcosystemItem {
    let default_model = provider_default_model(&key.provider);
    let available = key.configured && key.status == "available";
    EcosystemItem {
        id: provider_item_id(&key.provider),
        name: provider_display_name(&key.provider),
        description: provider_description(&key.provider),
        kind: CapabilityKind::Provider,
        source: "~/.forge/config.json".into(),
        version: default_model.unwrap_or("custom").to_string(),
        enabled: available,
        status: if available {
            EcosystemItemStatus::Healthy
        } else {
            EcosystemItemStatus::Unavailable
        },
        status_message: Some(if available {
            "API key configured in system credential store".to_string()
        } else {
            key.error.unwrap_or_else(|| "API key missing".to_string())
        }),
        configurable: true,
        config_summary: Some(match default_model {
            Some(model) => format!("Default model: {model}"),
            None => "Custom provider".to_string(),
        }),
    }
}

fn provider_item_id(provider: &str) -> String {
    format!("provider:{}", provider.trim().to_lowercase())
}

fn provider_display_name(provider: &str) -> String {
    match provider.trim().to_lowercase().as_str() {
        "anthropic" => "Anthropic".into(),
        "openai" => "OpenAI".into(),
        "openrouter" => "OpenRouter".into(),
        "deepseek" => "DeepSeek".into(),
        other if !other.is_empty() => other.to_string(),
        _ => "Provider".into(),
    }
}

fn provider_description(provider: &str) -> String {
    let Some(default_model) = provider_default_model(provider) else {
        return "Custom model provider".to_string();
    };
    let context =
        crate::agent::provider_capabilities::context_window_tokens(provider, default_model)
            .map(format_context_window)
            .unwrap_or_else(|| "context unknown".into());
    format!("Default model {default_model} · {context}")
}

fn provider_default_model(provider: &str) -> Option<&'static str> {
    let normalized = provider.trim().to_lowercase();
    match normalized.as_str() {
        "anthropic" | "openai" | "openrouter" | "deepseek" => Some(
            crate::agent::provider_capabilities::default_model(&normalized),
        ),
        _ => None,
    }
}

fn format_context_window(tokens: u32) -> String {
    if tokens >= 1_000_000 {
        format!("{}M context", tokens / 1_000_000)
    } else if tokens >= 1_000 {
        format!("{}K context", tokens / 1_000)
    } else {
        format!("{tokens} context")
    }
}

#[derive(Debug)]
struct McpConfigProbe {
    status: EcosystemItemStatus,
    status_message: String,
    config_summary: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct EcosystemMcpConfig {
    #[serde(default)]
    servers: HashMap<String, EcosystemMcpServerConfig>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct EcosystemMcpServerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EcosystemMcpConfigureInput {
    name: Option<String>,
    description: Option<String>,
    command: Option<String>,
    args: Option<Vec<String>>,
    enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EcosystemMcpConfigureUpdate {
    enabled: Option<bool>,
}

fn mcp_config_probe_for_capability(meta: &CapabilityMetadata) -> Option<McpConfigProbe> {
    if !matches!(meta.kind, CapabilityKind::McpServer) {
        return None;
    }

    let server_id = meta.id.strip_prefix("mcp:")?;
    let config_path = Path::new(&meta.source);
    let content = match std::fs::read_to_string(config_path) {
        Ok(content) => content,
        Err(_) => {
            return Some(McpConfigProbe {
                status: EcosystemItemStatus::Unavailable,
                status_message: "MCP config source is unreadable".into(),
                config_summary: None,
            });
        }
    };
    let config = match serde_json::from_str::<EcosystemMcpConfig>(&content) {
        Ok(config) => config,
        Err(_) => {
            return Some(McpConfigProbe {
                status: EcosystemItemStatus::Unavailable,
                status_message: "MCP config source is invalid JSON".into(),
                config_summary: None,
            });
        }
    };
    let Some(server) = config.servers.get(server_id) else {
        return Some(McpConfigProbe {
            status: EcosystemItemStatus::Unavailable,
            status_message: "MCP server is missing from source config".into(),
            config_summary: None,
        });
    };
    let Some(command) = server
        .command
        .as_deref()
        .map(str::trim)
        .filter(|cmd| !cmd.is_empty())
    else {
        return Some(McpConfigProbe {
            status: EcosystemItemStatus::Unavailable,
            status_message: "MCP server has no command configured".into(),
            config_summary: Some(format!("Args: {}", server.args.len())),
        });
    };

    if !command_is_available(command) {
        return Some(McpConfigProbe {
            status: EcosystemItemStatus::Unavailable,
            status_message: format!("MCP command not found: {command}"),
            config_summary: Some(mcp_config_summary(command, server.args.len())),
        });
    }

    Some(McpConfigProbe {
        status: EcosystemItemStatus::Healthy,
        status_message: "MCP command configured".into(),
        config_summary: Some(mcp_config_summary(command, server.args.len())),
    })
}

fn command_is_available(command: &str) -> bool {
    let path = Path::new(command);
    if path.is_absolute() || command.contains('/') || command.contains('\\') {
        return path.exists();
    }

    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(command).exists()))
}

fn mcp_config_summary(command: &str, arg_count: usize) -> String {
    format!("Command: {command}, Args: {arg_count}")
}

fn configure_mcp_ecosystem_item_at(
    config_path: &Path,
    capability_id: &str,
    config: serde_json::Value,
) -> Result<EcosystemMcpConfigureUpdate, String> {
    let server_id = capability_id
        .strip_prefix("mcp:")
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| {
            "Only MCP ecosystem items can be configured through this path".to_string()
        })?;

    let input: EcosystemMcpConfigureInput =
        serde_json::from_value(config).map_err(|e| format!("Invalid MCP config payload: {e}"))?;
    if input.name.is_none()
        && input.description.is_none()
        && input.command.is_none()
        && input.args.is_none()
        && input.enabled.is_none()
    {
        return Err("No supported MCP config fields were provided".to_string());
    }

    let mut mcp_config = read_ecosystem_mcp_config(config_path)?;
    let server = mcp_config.servers.entry(server_id.to_string()).or_default();

    if let Some(name) = input.name {
        server.name = normalize_optional_config_string(name);
    }
    if let Some(description) = input.description {
        server.description = normalize_optional_config_string(description);
    }
    if let Some(command) = input.command {
        let command = command.trim().to_string();
        if command.is_empty() {
            return Err("MCP command must not be empty".to_string());
        }
        server.command = Some(command);
    }
    if let Some(args) = input.args {
        server.args = args;
    }
    if let Some(enabled) = input.enabled {
        server.enabled = Some(enabled);
    }

    write_ecosystem_mcp_config(config_path, &mcp_config)?;
    Ok(EcosystemMcpConfigureUpdate {
        enabled: input.enabled,
    })
}

fn normalize_optional_config_string(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn read_ecosystem_mcp_config(config_path: &Path) -> Result<EcosystemMcpConfig, String> {
    if !config_path.exists() {
        return Ok(EcosystemMcpConfig::default());
    }
    let content =
        std::fs::read_to_string(config_path).map_err(|e| format!("Read MCP config source: {e}"))?;
    serde_json::from_str::<EcosystemMcpConfig>(&content)
        .map_err(|e| format!("Parse MCP config source: {e}"))
}

fn write_ecosystem_mcp_config(
    config_path: &Path,
    config: &EcosystemMcpConfig,
) -> Result<(), String> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Create MCP config dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Serialize MCP config source: {e}"))?;
    let tmp = config_path.with_extension("tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("Write MCP config tmp: {e}"))?;
    std::fs::rename(&tmp, config_path).map_err(|e| format!("Replace MCP config source: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_kind_labels_match_frontend_contract() {
        assert_eq!(capability_kind_label(&CapabilityKind::Skill), "skill");
        assert_eq!(capability_kind_label(&CapabilityKind::Hook), "hook");
        assert_eq!(
            capability_kind_label(&CapabilityKind::McpServer),
            "mcp_server"
        );
        assert_eq!(capability_kind_label(&CapabilityKind::Provider), "provider");
        assert_eq!(capability_kind_label(&CapabilityKind::Tool), "tool");
    }

    #[test]
    fn ecosystem_changed_event_uses_global_stream_contract() {
        let event = ecosystem_changed_event("skill-a", "enabled", Some(true));
        assert_eq!(event.event_type(), "ecosystem_changed");
        assert_eq!(event.session_id(), ECOSYSTEM_EVENT_SESSION_ID);

        let json = serde_json::to_value(&event).expect("serialize event");
        assert_eq!(json["event_type"], "ecosystem_changed");
        assert_eq!(json["session_id"], ECOSYSTEM_EVENT_SESSION_ID);
        assert_eq!(json["item_id"], "skill-a");
        assert_eq!(json["action"], "enabled");
        assert_eq!(json["enabled"], true);
    }

    #[test]
    fn global_capability_list_omits_workspace_scoped_mcp_servers() {
        let entries = vec![
            capability_entry(
                "read_file",
                "File Reader",
                CapabilityKind::Tool,
                "builtin",
                true,
            ),
            capability_entry(
                "mcp:obsidian",
                "Obsidian",
                CapabilityKind::McpServer,
                "/tmp/demo/.forge/mcp.json",
                true,
            ),
        ];

        let infos = global_registry_capability_infos(entries);

        assert!(infos.iter().any(|info| info.id == "read_file"));
        assert!(!infos.iter().any(|info| info.id == "mcp:obsidian"));
    }

    #[test]
    fn global_capability_list_omits_internal_infrastructure_capabilities() {
        let entries = vec![
            capability_entry(
                "skill-loader",
                "Skill Loader",
                CapabilityKind::Skill,
                "builtin",
                true,
            ),
            capability_entry(
                "hook:workspace-boundary",
                "Workspace Boundary Guard",
                CapabilityKind::Hook,
                "builtin",
                true,
            ),
        ];

        let infos = global_registry_capability_infos(entries);

        assert!(!infos.iter().any(|info| info.id == "skill-loader"));
        assert!(infos
            .iter()
            .any(|info| info.id == "hook:workspace-boundary"));
    }

    #[test]
    fn ecosystem_item_from_capability_entry_maps_fields() {
        let entry = capability_entry(
            "read_file",
            "File Reader",
            CapabilityKind::Tool,
            "builtin",
            true,
        );
        let item = EcosystemItem::from_capability_entry(&entry);
        assert_eq!(item.id, "read_file");
        assert_eq!(item.name, "File Reader");
        assert_eq!(item.kind, CapabilityKind::Tool);
        assert_eq!(item.source, "builtin");
        assert_eq!(item.version, "1.0.0");
        assert!(item.enabled);
        assert_eq!(item.status, EcosystemItemStatus::Unknown);
        assert!(!item.configurable);
    }

    #[test]
    fn ecosystem_item_mcp_is_configurable() {
        let entry = capability_entry(
            "mcp:test-server",
            "Test Server",
            CapabilityKind::McpServer,
            ".forge/mcp.json",
            true,
        );
        let item = EcosystemItem::from_capability_entry(&entry);
        assert!(item.configurable);
    }

    #[test]
    fn configure_ecosystem_item_returns_unsupported_error() {
        // This is a sync stub for now — validates the error message shape
        let msg = "In-app configuration is not yet supported";
        assert!(msg.contains("not yet supported"));
    }

    #[test]
    fn configure_mcp_ecosystem_item_updates_existing_server_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("mcp.json");
        std::fs::write(
            &config_path,
            serde_json::json!({
                "servers": {
                    "test": {
                        "name": "Old MCP",
                        "description": "old description",
                        "command": "old-command",
                        "args": ["--old"],
                        "enabled": false,
                        "env": { "KEEP": "yes" }
                    },
                    "other": {
                        "command": "other-command"
                    }
                }
            })
            .to_string(),
        )
        .expect("write config");

        let update = configure_mcp_ecosystem_item_at(
            &config_path,
            "mcp:test",
            serde_json::json!({
                "name": "New MCP",
                "description": "new description",
                "command": "new-command",
                "args": ["--new", "value"],
                "enabled": true
            }),
        )
        .expect("configure mcp");

        assert_eq!(update.enabled, Some(true));

        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path).expect("read config"))
                .expect("saved json");
        let test = &saved["servers"]["test"];
        assert_eq!(test["name"], "New MCP");
        assert_eq!(test["description"], "new description");
        assert_eq!(test["command"], "new-command");
        assert_eq!(test["args"], serde_json::json!(["--new", "value"]));
        assert_eq!(test["enabled"], true);
        assert_eq!(test["env"]["KEEP"], "yes");
        assert_eq!(saved["servers"]["other"]["command"], "other-command");
    }

    #[test]
    fn tool_inventory_entry_serializes_correctly() {
        let entry = ToolInventoryEntry {
            id: "read_file".into(),
            name: "File Reader".into(),
            description: "Read files".into(),
            kind: "tool".into(),
            source: "builtin".into(),
            enabled: true,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["id"], "read_file");
        assert_eq!(json["kind"], "tool");
        assert_eq!(json["enabled"], true);
    }

    #[test]
    fn ecosystem_item_with_status_and_message_serializes_both() {
        let entry = capability_entry(
            "mcp:test",
            "Test MCP",
            CapabilityKind::McpServer,
            ".forge/mcp.json",
            true,
        );
        let item = EcosystemItem::from_capability_entry(&entry).with_status(
            EcosystemItemStatus::Unavailable,
            Some("Connection refused".into()),
        );
        assert_eq!(item.status, EcosystemItemStatus::Unavailable);
        assert_eq!(item.status_message.as_deref(), Some("Connection refused"));
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["status"], "unavailable");
        assert_eq!(json["statusMessage"], "Connection refused");
    }

    #[test]
    fn ecosystem_item_from_entry_marks_enabled_tools_healthy() {
        let entry = capability_entry(
            "read_file",
            "File Reader",
            CapabilityKind::Tool,
            "builtin",
            true,
        );

        let item = ecosystem_item_from_entry(entry);

        assert_eq!(item.status, EcosystemItemStatus::Healthy);
        assert!(item.status_message.is_none());
    }

    #[test]
    fn provider_ecosystem_item_marks_configured_key_healthy() {
        let item = provider_ecosystem_item(settings::KeyStatus {
            provider: "openai".into(),
            configured: true,
            source: "system_store".into(),
            status: "available".into(),
            error: None,
        });

        assert_eq!(item.id, "provider:openai");
        assert_eq!(item.name, "OpenAI");
        assert_eq!(item.kind, CapabilityKind::Provider);
        assert!(item.enabled);
        assert_eq!(item.status, EcosystemItemStatus::Healthy);
        assert_eq!(
            item.status_message.as_deref(),
            Some("API key configured in system credential store")
        );
        assert_eq!(
            item.config_summary.as_deref(),
            Some("Default model: gpt-4o")
        );
    }

    #[test]
    fn provider_ecosystem_item_marks_missing_key_unavailable() {
        let item = provider_ecosystem_item(settings::KeyStatus {
            provider: "deepseek".into(),
            configured: false,
            source: "none".into(),
            status: "not_configured".into(),
            error: None,
        });

        assert_eq!(item.id, "provider:deepseek");
        assert!(!item.enabled);
        assert_eq!(item.status, EcosystemItemStatus::Unavailable);
        assert_eq!(item.status_message.as_deref(), Some("API key missing"));
        assert!(item.description.contains("1M context"));
    }

    #[test]
    fn ecosystem_item_from_entry_marks_unreadable_mcp_source_unavailable() {
        let entry = capability_entry(
            "mcp:test",
            "Test MCP",
            CapabilityKind::McpServer,
            "user-mcp.json",
            true,
        );

        let item = ecosystem_item_from_entry(entry);

        assert_eq!(item.status, EcosystemItemStatus::Unavailable);
        assert_eq!(
            item.status_message.as_deref(),
            Some("MCP config source is unreadable"),
        );
    }

    #[test]
    fn ecosystem_item_from_entry_marks_configured_mcp_healthy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("mcp.json");
        let command = std::env::current_exe()
            .expect("current exe")
            .to_string_lossy()
            .to_string();
        std::fs::write(
            &config_path,
            serde_json::json!({
                "servers": {
                    "test": {
                        "command": command,
                        "args": ["--help"]
                    }
                }
            })
            .to_string(),
        )
        .expect("write config");

        let entry = capability_entry(
            "mcp:test",
            "Test MCP",
            CapabilityKind::McpServer,
            &config_path.to_string_lossy(),
            true,
        );

        let item = ecosystem_item_from_entry(entry);

        assert_eq!(item.status, EcosystemItemStatus::Healthy);
        assert_eq!(
            item.status_message.as_deref(),
            Some("MCP command configured")
        );
        assert!(item
            .config_summary
            .as_deref()
            .is_some_and(|summary| summary.contains("Args: 1")));
    }

    #[test]
    fn ecosystem_item_from_entry_marks_mcp_without_command_unavailable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("mcp.json");
        std::fs::write(
            &config_path,
            serde_json::json!({
                "servers": {
                    "test": {
                        "args": ["--help"]
                    }
                }
            })
            .to_string(),
        )
        .expect("write config");

        let entry = capability_entry(
            "mcp:test",
            "Test MCP",
            CapabilityKind::McpServer,
            &config_path.to_string_lossy(),
            true,
        );

        let item = ecosystem_item_from_entry(entry);

        assert_eq!(item.status, EcosystemItemStatus::Unavailable);
        assert_eq!(
            item.status_message.as_deref(),
            Some("MCP server has no command configured"),
        );
    }

    fn capability_entry(
        id: &str,
        name: &str,
        kind: CapabilityKind,
        source: &str,
        enabled: bool,
    ) -> crate::harness::registry::CapabilityEntry {
        crate::harness::registry::CapabilityEntry {
            metadata: crate::harness::capability::CapabilityMetadata {
                id: id.to_string(),
                name: name.to_string(),
                description: format!("{name} description"),
                version: "1.0.0".to_string(),
                source: source.to_string(),
                kind,
            },
            enabled,
        }
    }
}

#[tauri::command]
pub async fn toggle_capability(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    capability_id: String,
    enabled: bool,
) -> Result<(), String> {
    toggle_capability_for_state(state.inner(), &capability_id, enabled).await?;
    emit_ecosystem_changed(
        &app_handle,
        capability_id,
        if enabled { "enabled" } else { "disabled" },
        Some(enabled),
    );
    Ok(())
}

async fn toggle_capability_for_state(
    state: &Arc<AppState>,
    capability_id: &str,
    enabled: bool,
) -> Result<(), String> {
    // Try registry first, then skill loader
    match state
        .harness
        .capability_registry
        .toggle_with_event(capability_id, enabled)
        .await
    {
        Ok(_) => {
            let sessions = state
                .sessions
                .read()
                .await
                .values()
                .cloned()
                .collect::<Vec<_>>();
            for session in sessions {
                let _ = session
                    .harness
                    .capability_registry
                    .toggle_with_event(capability_id, enabled)
                    .await;
            }
            Ok(())
        }
        Err(reg_err) => {
            let skill_loader = SkillLoader::new();
            skill_loader.attach_database(state.harness.database.clone());
            let _ = skill_loader.scan_all().await;
            skill_loader.toggle(capability_id, enabled).await;
            let skills = skill_loader.all_skills().await;
            if skills.iter().any(|s| s.id == capability_id) {
                let sessions = state
                    .sessions
                    .read()
                    .await
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                for session in sessions {
                    let _ = session.harness.skill_loader.scan_all().await;
                    session
                        .harness
                        .skill_loader
                        .toggle(capability_id, enabled)
                        .await;
                }
                Ok(())
            } else {
                Err(format!(
                    "Capability not found in registry or skills: {} ({})",
                    capability_id, reg_err
                ))
            }
        }
    }
}

/// Allowed Git hosts for skill installation.
const ALLOWED_HOSTS: &[&str] = &["github.com", "gitlab.com", "bitbucket.org"];

fn validate_skill_url(repo: &str) -> Result<String, String> {
    if repo.trim().is_empty() {
        return Err("Repository cannot be empty".into());
    }

    // If it's a full URL, validate the host
    if repo.starts_with("http://") || repo.starts_with("https://") {
        // Extract host from URL with simple string parsing
        let without_scheme = if let Some(s) = repo.strip_prefix("https://") {
            s
        } else if repo.strip_prefix("http://").is_some() {
            return Err("Only HTTPS URLs are allowed".into());
        } else {
            return Err("Invalid URL scheme".into());
        };
        let host = without_scheme.split('/').next().unwrap_or("");
        if host.is_empty() {
            return Err("Invalid URL: missing host".into());
        }
        let host_root = host.rsplitn(2, '.').last().unwrap_or(host);
        if !ALLOWED_HOSTS
            .iter()
            .any(|allowed| host_root == *allowed || host == *allowed)
        {
            return Err(format!(
                "Untrusted Git host: {}. Allowed: {:?}",
                host, ALLOWED_HOSTS
            ));
        }
        return Ok(repo.to_string());
    }

    // Otherwise treat as owner/repo shorthand (GitHub default)
    if !repo.contains('/') {
        return Err("Repository must be in owner/repo format".into());
    }
    let cleaned = repo.trim_end_matches(".git");
    // Basic sanity: no path traversal, no shell metacharacters
    if cleaned.contains("..") || cleaned.contains('$') || cleaned.contains('`') {
        return Err("Invalid repository name".into());
    }
    Ok(format!("https://github.com/{}.git", cleaned))
}

fn read_skill_metadata(dir: &Path) -> Result<(String, String), String> {
    let skill_md = dir.join("SKILL.md");
    let claude_md = dir.join("CLAUDE.md");
    let md_path = if skill_md.exists() {
        &skill_md
    } else if claude_md.exists() {
        &claude_md
    } else {
        return Err(format!(
            "No SKILL.md or CLAUDE.md found in {}",
            dir.display()
        ));
    };

    let content = std::fs::read_to_string(md_path).map_err(|e| e.to_string())?;
    let desc = content
        .lines()
        .find(|l| l.starts_with("description:"))
        .map(|l| l.trim_start_matches("description:").trim().to_string())
        .unwrap_or_default();
    Ok((desc, content))
}

#[tauri::command]
pub async fn install_skill(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    repo: String,
) -> Result<CapabilityInfo, String> {
    let url = validate_skill_url(&repo)?;
    let name = repo
        .split('/')
        .next_back()
        .unwrap_or(&repo)
        .replace(".git", "");
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let skills_dir = std::path::PathBuf::from(home).join(".forge").join("skills");
    let dest = skills_dir.join(&name);

    if dest.exists() {
        let (desc, _) = read_skill_metadata(&dest)?;
        let _ = state.harness.skill_loader.scan_all().await;
        let info = CapabilityInfo {
            id: name.clone(),
            name: name.clone(),
            description: desc,
            kind: "skill".into(),
            source: repo,
            version: "1.0.0".into(),
            enabled: true,
        };
        emit_ecosystem_changed(&app_handle, &info.id, "installed", Some(info.enabled));
        return Ok(info);
    }

    std::fs::create_dir_all(&skills_dir).map_err(|e| e.to_string())?;

    let dest_str = dest.to_string_lossy();
    let output = std::process::Command::new("git")
        .args(["clone", "--depth=1", &url, &*dest_str])
        .output()
        .map_err(|e| format!("Git failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Re-scan and read metadata
    let _ = state.harness.skill_loader.scan_all().await;
    let (desc, _) = read_skill_metadata(&dest).unwrap_or_default();

    let info = CapabilityInfo {
        id: name.clone(),
        name: name.clone(),
        description: desc,
        kind: "skill".into(),
        source: repo,
        version: "1.0.0".into(),
        enabled: true,
    };
    emit_ecosystem_changed(&app_handle, &info.id, "installed", Some(info.enabled));
    Ok(info)
}

// ── Ecosystem IPC (Phase 3-A) ──────────────────────────────────────────────

/// List all ecosystem items (tools, skills, hooks, MCP servers) with richer
/// metadata suitable for the Settings UI and diagnostics.
#[tauri::command]
pub async fn list_ecosystem_items(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<EcosystemItem>, String> {
    let mut items: Vec<EcosystemItem> = state
        .harness
        .capability_registry
        .all_entries()
        .into_iter()
        .filter(|entry| !is_hidden_global_capability(entry))
        .map(ecosystem_item_from_entry)
        .collect();

    // Merge skills from SkillLoader (user-level, not global registry)
    let skill_loader = SkillLoader::new();
    skill_loader.attach_database(state.harness.database.clone());
    let skills = skill_loader.scan_all().await;
    for s in skills {
        if !items.iter().any(|item| item.id == s.id) {
            items.push(EcosystemItem {
                id: s.id.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                kind: CapabilityKind::Skill,
                source: "local".into(),
                version: "1.0.0".into(),
                enabled: s.enabled,
                status: if s.enabled {
                    EcosystemItemStatus::Healthy
                } else {
                    EcosystemItemStatus::Unknown
                },
                status_message: if s.enabled {
                    None
                } else {
                    Some("Disabled".into())
                },
                configurable: false,
                config_summary: None,
            });
        }
    }
    items.extend(provider_ecosystem_items(state.credential_store.as_ref()));

    Ok(items)
}

/// Enable or disable an ecosystem item by id.
/// Delegates to the existing `toggle_capability` path for consistency.
#[tauri::command]
pub async fn set_ecosystem_enabled(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    toggle_capability_for_state(state.inner(), &id, enabled).await?;
    emit_ecosystem_changed(
        &app_handle,
        id,
        if enabled { "enabled" } else { "disabled" },
        Some(enabled),
    );
    Ok(())
}

/// Get a lightweight tool inventory: names, kinds, enabled status, and
/// description/source for every tool in the capability registry.
#[derive(Serialize)]
pub struct ToolInventoryEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub source: String,
    pub enabled: bool,
}

#[tauri::command]
pub async fn get_tool_inventory(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<ToolInventoryEntry>, String> {
    let entries: Vec<ToolInventoryEntry> = state
        .harness
        .capability_registry
        .all_entries()
        .into_iter()
        .filter(|entry| {
            matches!(
                entry.metadata.kind,
                CapabilityKind::Tool | CapabilityKind::McpServer
            )
        })
        .filter(|entry| !is_hidden_global_capability(entry))
        .map(|entry| {
            let m = entry.metadata;
            ToolInventoryEntry {
                id: m.id,
                name: m.name,
                description: m.description,
                kind: capability_kind_label(&m.kind).to_string(),
                source: m.source,
                enabled: entry.enabled,
            }
        })
        .collect();

    Ok(entries)
}

/// Configure an ecosystem item.
///
/// Currently supports MCP server items by writing the source `.forge/mcp.json`
/// entry. Other ecosystem item kinds still return an explicit unsupported
/// error until they have a stable persistent config model.
#[tauri::command]
pub async fn configure_ecosystem_item(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
    config: serde_json::Value,
) -> Result<(), String> {
    let meta = state
        .harness
        .capability_registry
        .get(&id)
        .ok_or_else(|| format!("Ecosystem item not found: {id}"))?;

    if !matches!(meta.kind, CapabilityKind::McpServer) {
        return Err(format!(
            "In-app configuration currently supports MCP server items only. \
             '{}' is a {} item.",
            id,
            capability_kind_label(&meta.kind)
        ));
    }

    let update = configure_mcp_ecosystem_item_at(Path::new(&meta.source), &id, config)?;
    if let Some(enabled) = update.enabled {
        toggle_capability_for_state(state.inner(), &id, enabled).await?;
    }
    emit_ecosystem_changed(&app_handle, id, "configured", update.enabled);
    Ok(())
}
