use std::sync::{Arc, RwLock};
use crate::harness::capability::{Capability, CapabilityKind, CapabilityMetadata, Event, EventType};
use crate::harness::db::Database;

#[derive(Clone)]
pub struct CapabilityEntry {
    pub metadata: CapabilityMetadata,
    pub enabled: bool,
}

pub struct CapabilityRegistry {
    capabilities: RwLock<Vec<Box<dyn Capability>>>,
    db: Arc<Database>,
}

impl CapabilityRegistry {
    pub fn new(db: Arc<Database>) -> Self {
        Self { capabilities: RwLock::new(Vec::new()), db }
    }

    pub fn register(&self, cap: Box<dyn Capability>) {
        let meta = cap.metadata().clone();
        let kind_str = match meta.kind {
            CapabilityKind::Skill => "skill",
            CapabilityKind::Hook => "hook",
            CapabilityKind::McpServer => "mcp_server",
            CapabilityKind::Tool => "tool",
        };
        let enabled = self.db.get_capability_enabled(&meta.id).unwrap_or(None).unwrap_or(cap.enabled());
        let _ = self.db.upsert_capability(&meta.id, &meta.name, kind_str, &meta.source, enabled);
        let mut cap = cap;
        cap.set_enabled(enabled);
        self.capabilities.write().unwrap().push(cap);
    }

    pub fn all(&self) -> Vec<CapabilityMetadata> {
        self.capabilities.read().unwrap().iter().map(|c| c.metadata().clone()).collect()
    }

    pub fn all_entries(&self) -> Vec<CapabilityEntry> {
        self.capabilities
            .read()
            .unwrap()
            .iter()
            .map(|c| CapabilityEntry {
                metadata: c.metadata().clone(),
                enabled: c.enabled(),
            })
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<CapabilityMetadata> {
        self.capabilities.read().unwrap().iter()
            .find(|c| c.metadata().id == id)
            .map(|c| c.metadata().clone())
    }

    pub fn toggle(&self, id: &str, enabled: bool) -> Result<(), String> {
        let mut caps = self.capabilities.write().unwrap();
        let cap = caps.iter_mut().find(|c| c.metadata().id == id)
            .ok_or_else(|| format!("Capability not found: {id}"))?;
        cap.set_enabled(enabled);
        let _ = self.db.set_enabled(id, enabled);
        Ok(())
    }

    pub fn is_tool_enabled(&self, tool_name: &str) -> bool {
        let id = capability_id_for_tool(tool_name);
        self.capabilities
            .read()
            .unwrap()
            .iter()
            .find(|c| c.metadata().id == id)
            .map(|c| c.enabled())
            .unwrap_or(true)
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut caps = self.capabilities.write().unwrap();
        caps.retain(|c| c.metadata().id != id);
        let _ = self.db.delete_capability(id);
        Ok(())
    }

    pub async fn dispatch_event(&self, event: &Event) {
        let caps = self.capabilities.read().unwrap();
        for cap in caps.iter() {
            if cap.enabled() {
                let subscribed = cap.subscribed_events();
                if subscribed.iter().any(|e| matches_event(e, event)) {
                    let _ = cap.on_event(event).await;
                }
            }
        }
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
        "bash" | "execute_command" | "shell" => "run_shell",
        other => other,
    }
}

fn matches_event(et: &EventType, event: &Event) -> bool {
    match (et, event) {
        (EventType::SessionStart, Event::SessionStart { .. }) => true,
        (EventType::SessionStop, Event::SessionStop { .. }) => true,
        (EventType::PreTool, Event::PreTool { .. }) => true,
        (EventType::PostTool, Event::PostTool { .. }) => true,
        (EventType::CapabilityChanged, Event::CapabilityChanged { .. }) => true,
        _ => false,
    }
}
