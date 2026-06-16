use crate::harness::capability::{
    Capability, CapabilityDispatchError, CapabilityDispatchReport, CapabilityKind,
    CapabilityMetadata, Event, EventType,
};
use crate::harness::db::Database;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct CapabilityEntry {
    pub metadata: CapabilityMetadata,
    pub enabled: bool,
}

struct RegisteredCapability {
    cap: Arc<dyn Capability>,
    enabled: bool,
}

pub struct CapabilityRegistry {
    capabilities: RwLock<Vec<RegisteredCapability>>,
    db: Arc<Database>,
}

impl CapabilityRegistry {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            capabilities: RwLock::new(Vec::new()),
            db,
        }
    }

    pub fn register(&self, mut cap: Box<dyn Capability>) {
        let meta = cap.metadata().clone();
        let kind_str = match meta.kind {
            CapabilityKind::Skill => "skill",
            CapabilityKind::Hook => "hook",
            CapabilityKind::McpServer => "mcp_server",
            CapabilityKind::Tool => "tool",
        };
        let enabled = self
            .db
            .get_capability_enabled(&meta.id)
            .unwrap_or(None)
            .unwrap_or(cap.enabled());
        let _ = self
            .db
            .upsert_capability(&meta.id, &meta.name, kind_str, &meta.source, enabled);
        cap.set_enabled(enabled);
        self.capabilities
            .write()
            .unwrap()
            .push(RegisteredCapability {
                cap: Arc::from(cap),
                enabled,
            });
    }

    pub fn all(&self) -> Vec<CapabilityMetadata> {
        self.capabilities
            .read()
            .unwrap()
            .iter()
            .map(|c| c.cap.metadata().clone())
            .collect()
    }

    pub fn all_entries(&self) -> Vec<CapabilityEntry> {
        self.capabilities
            .read()
            .unwrap()
            .iter()
            .map(|c| CapabilityEntry {
                metadata: c.cap.metadata().clone(),
                enabled: c.enabled,
            })
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<CapabilityMetadata> {
        self.capabilities
            .read()
            .unwrap()
            .iter()
            .find(|c| c.cap.metadata().id == id)
            .map(|c| c.cap.metadata().clone())
    }

    pub fn toggle(&self, id: &str, enabled: bool) -> Result<(), String> {
        let mut caps = self.capabilities.write().unwrap();
        let cap = caps
            .iter_mut()
            .find(|c| c.cap.metadata().id == id)
            .ok_or_else(|| format!("Capability not found: {id}"))?;
        cap.enabled = enabled;
        let _ = self.db.set_enabled(id, enabled);
        Ok(())
    }

    pub async fn toggle_with_event(
        &self,
        id: &str,
        enabled: bool,
    ) -> Result<CapabilityDispatchReport, String> {
        self.toggle(id, enabled)?;
        let action = if enabled { "enabled" } else { "disabled" };
        Ok(self
            .dispatch_event(&Event::CapabilityChanged {
                capability_id: id.to_string(),
                action: action.to_string(),
            })
            .await)
    }

    pub fn is_tool_enabled(&self, tool_name: &str) -> bool {
        let id = capability_id_for_tool(tool_name);
        self.capabilities
            .read()
            .unwrap()
            .iter()
            .find(|c| c.cap.metadata().id == id)
            .map(|c| c.enabled)
            .unwrap_or(true)
    }

    pub fn is_hook_enabled(&self, hook_name: &str) -> bool {
        let id = format!("hook:{hook_name}");
        self.capabilities
            .read()
            .unwrap()
            .iter()
            .find(|c| c.cap.metadata().id == id)
            .map(|c| c.enabled)
            .unwrap_or(true)
    }

    pub fn is_mcp_enabled(&self, server_id: &str) -> bool {
        let id = format!("mcp:{server_id}");
        self.capabilities
            .read()
            .unwrap()
            .iter()
            .find(|c| c.cap.metadata().id == id)
            .map(|c| c.enabled)
            .unwrap_or(false)
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut caps = self.capabilities.write().unwrap();
        caps.retain(|c| c.cap.metadata().id != id);
        let _ = self.db.delete_capability(id);
        Ok(())
    }

    pub async fn dispatch_event(&self, event: &Event) -> CapabilityDispatchReport {
        let matching: Vec<(String, Arc<dyn Capability>)> = {
            let caps = self.capabilities.read().unwrap();
            caps.iter()
                .filter(|entry| entry.enabled)
                .filter_map(|entry| {
                    let subscribed = entry.cap.subscribed_events();
                    subscribed
                        .iter()
                        .any(|e| matches_event(e, event))
                        .then(|| (entry.cap.metadata().id.clone(), Arc::clone(&entry.cap)))
                })
                .collect()
        };

        let mut report = CapabilityDispatchReport::new(event.event_type());
        for (capability_id, cap) in matching {
            match cap.on_event(event).await {
                Ok(()) => report.handled_by.push(capability_id),
                Err(message) => report.errors.push(CapabilityDispatchError {
                    capability_id,
                    message,
                }),
            }
        }
        report
    }
}

fn capability_id_for_tool(tool_name: &str) -> &str {
    match tool_name {
        "read" => "read_file",
        "write" | "write_file" => "write_to_file",
        "edit" => "edit_file",
        "ls" | "list" => "list_directory",
        "glob" => "search_files",
        "grep" => "search_content",
        "bash" | "execute_command" | "shell" | "shell_command" | "run_command"
        | "run_shell_command" => "run_shell",
        other => other,
    }
}

fn matches_event(et: &EventType, event: &Event) -> bool {
    matches!(
        (et, event),
        (EventType::SessionStart, Event::SessionStart { .. })
            | (EventType::SessionStop, Event::SessionStop { .. })
            | (EventType::PreTool, Event::PreTool { .. })
            | (EventType::PostTool, Event::PostTool { .. })
            | (
                EventType::CapabilityChanged,
                Event::CapabilityChanged { .. }
            )
    )
}
