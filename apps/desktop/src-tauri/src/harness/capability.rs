use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Skill,
    Hook,
    McpServer,
    Provider,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    SessionStart,
    SessionStop,
    PreTool,
    PostTool,
    CapabilityChanged,
}

#[derive(Debug, Clone)]
pub enum Event {
    SessionStart {
        session_id: String,
        working_dir: String,
    },
    SessionStop {
        session_id: String,
    },
    PreTool {
        session_id: String,
        tool_name: String,
        input: serde_json::Value,
    },
    PostTool {
        session_id: String,
        tool_name: String,
        result: String,
    },
    CapabilityChanged {
        capability_id: String,
        action: String,
    },
}

impl Event {
    pub fn event_type(&self) -> EventType {
        match self {
            Event::SessionStart { .. } => EventType::SessionStart,
            Event::SessionStop { .. } => EventType::SessionStop,
            Event::PreTool { .. } => EventType::PreTool,
            Event::PostTool { .. } => EventType::PostTool,
            Event::CapabilityChanged { .. } => EventType::CapabilityChanged,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDispatchError {
    pub capability_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDispatchReport {
    pub event_type: EventType,
    pub handled_by: Vec<String>,
    pub errors: Vec<CapabilityDispatchError>,
}

impl CapabilityDispatchReport {
    pub fn new(event_type: EventType) -> Self {
        Self {
            event_type,
            handled_by: Vec::new(),
            errors: Vec::new(),
        }
    }
}

#[async_trait]
pub trait Capability: Send + Sync {
    fn id(&self) -> &str;
    fn metadata(&self) -> &CapabilityMetadata;
    fn enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn subscribed_events(&self) -> Vec<EventType> {
        vec![]
    }
    async fn on_event(&self, _event: &Event) -> Result<(), String> {
        Ok(())
    }
}

// ── Ecosystem item model (Phase 3-A) ─────────────────────────────────────

/// Health/availability status for an ecosystem item surfaced in Settings UI
/// and diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EcosystemItemStatus {
    /// Item is healthy and available.
    Healthy,
    /// Item is present but unavailable (e.g. MCP server unreachable).
    Unavailable,
    /// Item has a non-fatal issue (stale, slow, partial).
    Warning,
    /// Status is unknown (not yet probed, or probe not supported).
    #[default]
    Unknown,
}

/// A richer serializable item for the ecosystem Settings UI and diagnostics.
/// Built from registry entries, skill scans, provider status, and (future)
/// extension inventory.  This does NOT replace `CapabilityMetadata` — it wraps
/// it with UI-facing fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EcosystemItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: CapabilityKind,
    pub source: String,
    pub version: String,
    pub enabled: bool,
    /// Health / availability status for this item.
    pub status: EcosystemItemStatus,
    /// Human-readable status message (e.g. "Connected to localhost:8080" or
    /// "Connection refused").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    /// Whether this item supports in-app configuration.  When false the UI
    /// should show a clear "not yet supported" message rather than a broken
    /// config form.
    pub configurable: bool,
    /// Short human-readable summary of the current configuration (e.g.
    /// "Port: 8080, Auth: token") or `None` when not applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_summary: Option<String>,
}

impl EcosystemItem {
    /// Build an `EcosystemItem` from a registry `CapabilityEntry`.
    /// Status defaults to `Unknown`; callers should enrich with probe results.
    pub fn from_capability_entry(entry: &crate::harness::registry::CapabilityEntry) -> Self {
        let m = &entry.metadata;
        Self {
            id: m.id.clone(),
            name: m.name.clone(),
            description: m.description.clone(),
            kind: m.kind.clone(),
            source: m.source.clone(),
            version: m.version.clone(),
            enabled: entry.enabled,
            status: EcosystemItemStatus::Unknown,
            status_message: None,
            configurable: matches!(m.kind, CapabilityKind::McpServer | CapabilityKind::Provider),
            config_summary: None,
        }
    }

    /// Mark the item with a health status and optional message.
    pub fn with_status(mut self, status: EcosystemItemStatus, message: Option<String>) -> Self {
        self.status = status;
        self.status_message = message;
        self
    }

    /// Mark the item as not configurable (the default for tools, hooks, and
    /// skills without a config path).
    pub fn with_configurable(mut self, configurable: bool) -> Self {
        self.configurable = configurable;
        self
    }
}

#[cfg(test)]
mod ecosystem_tests {
    use super::*;
    use crate::harness::registry::CapabilityEntry;

    fn make_entry(
        id: &str,
        name: &str,
        kind: CapabilityKind,
        source: &str,
        enabled: bool,
    ) -> CapabilityEntry {
        CapabilityEntry {
            metadata: CapabilityMetadata {
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

    #[test]
    fn ecosystem_item_status_default_is_unknown() {
        assert_eq!(EcosystemItemStatus::default(), EcosystemItemStatus::Unknown);
    }

    #[test]
    fn ecosystem_item_status_serializes_snake_case() {
        let cases = [
            (EcosystemItemStatus::Healthy, "healthy"),
            (EcosystemItemStatus::Unavailable, "unavailable"),
            (EcosystemItemStatus::Warning, "warning"),
            (EcosystemItemStatus::Unknown, "unknown"),
        ];
        for (status, expected) in &cases {
            let json = serde_json::to_value(status).unwrap();
            assert_eq!(json.as_str().unwrap(), *expected);
        }
    }

    #[test]
    fn ecosystem_item_from_registry_entry_has_unknown_status() {
        let entry = make_entry(
            "read_file",
            "File Reader",
            CapabilityKind::Tool,
            "builtin",
            true,
        );
        let item = EcosystemItem::from_capability_entry(&entry);
        assert_eq!(item.id, "read_file");
        assert_eq!(item.name, "File Reader");
        assert_eq!(item.status, EcosystemItemStatus::Unknown);
        assert!(!item.configurable, "tools should not be configurable");
        assert!(item.status_message.is_none());
    }

    #[test]
    fn ecosystem_item_mcp_server_is_configurable() {
        let entry = make_entry(
            "mcp:obsidian",
            "Obsidian",
            CapabilityKind::McpServer,
            ".forge/mcp.json",
            true,
        );
        let item = EcosystemItem::from_capability_entry(&entry);
        assert!(item.configurable, "MCP servers should be configurable");
    }

    #[test]
    fn ecosystem_item_with_status_updates_both_fields() {
        let entry = make_entry(
            "hook:logging",
            "Logging",
            CapabilityKind::Hook,
            "builtin",
            true,
        );
        let item = EcosystemItem::from_capability_entry(&entry)
            .with_status(EcosystemItemStatus::Warning, Some("stale".into()));
        assert_eq!(item.status, EcosystemItemStatus::Warning);
        assert_eq!(item.status_message.as_deref(), Some("stale"));
    }

    #[test]
    fn ecosystem_item_with_configurable_overrides_default() {
        let entry = make_entry(
            "read_file",
            "File Reader",
            CapabilityKind::Tool,
            "builtin",
            true,
        );
        let item = EcosystemItem::from_capability_entry(&entry).with_configurable(true);
        assert!(item.configurable);
    }

    #[test]
    fn ecosystem_item_serializes_camelcase_and_omits_optionals() {
        let entry = make_entry(
            "test-skill",
            "Test Skill",
            CapabilityKind::Skill,
            "local",
            true,
        );
        let item = EcosystemItem::from_capability_entry(&entry);
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["id"], "test-skill");
        assert_eq!(json["name"], "Test Skill");
        assert_eq!(json["enabled"], true);
        assert_eq!(json["status"], "unknown");
        assert!(json.get("statusMessage").is_none());
        assert!(json.get("configSummary").is_none());
        assert_eq!(json["configurable"], false);
    }
}
