use async_trait::async_trait;
use crate::harness::capability::{Capability, CapabilityKind, CapabilityMetadata, Event, EventType};

pub struct BuiltinToolCap {
    enabled: bool,
    meta: CapabilityMetadata,
}

impl BuiltinToolCap {
    pub fn new(id: &str, name: &str, description: &str) -> Self {
        Self {
            enabled: true,
            meta: CapabilityMetadata {
                id: id.into(),
                name: name.into(),
                description: description.into(),
                version: "1.0.0".into(),
                source: "builtin".into(),
                kind: CapabilityKind::Tool,
            },
        }
    }
}

#[async_trait]
impl Capability for BuiltinToolCap {
    fn id(&self) -> &str {
        &self.meta.id
    }
    fn metadata(&self) -> &CapabilityMetadata {
        &self.meta
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, e: bool) {
        self.enabled = e;
    }
    fn subscribed_events(&self) -> Vec<EventType> {
        vec![EventType::PreTool]
    }
    async fn on_event(&self, _event: &Event) -> Result<(), String> {
        Ok(())
    }
}

// FileTool
pub struct FileToolCap {
    enabled: bool,
    meta: CapabilityMetadata,
}

impl FileToolCap {
    pub fn new() -> Self {
        Self {
            enabled: true,
            meta: CapabilityMetadata {
                id: "read_file".into(),
                name: "File Reader".into(),
                description: "Read file contents".into(),
                version: "1.0.0".into(),
                source: "builtin".into(),
                kind: CapabilityKind::Tool,
            },
        }
    }
}

#[async_trait]
impl Capability for FileToolCap {
    fn id(&self) -> &str {
        "read_file"
    }
    fn metadata(&self) -> &CapabilityMetadata {
        &self.meta
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, e: bool) {
        self.enabled = e;
    }
    fn subscribed_events(&self) -> Vec<EventType> {
        vec![EventType::PreTool]
    }
    async fn on_event(&self, _event: &Event) -> Result<(), String> {
        Ok(())
    }
}

// WriteFileTool
pub struct WriteFileToolCap {
    enabled: bool,
    meta: CapabilityMetadata,
}

impl WriteFileToolCap {
    pub fn new() -> Self {
        Self {
            enabled: true,
            meta: CapabilityMetadata {
                id: "write_to_file".into(),
                name: "File Writer".into(),
                description: "Create or overwrite files".into(),
                version: "1.0.0".into(),
                source: "builtin".into(),
                kind: CapabilityKind::Tool,
            },
        }
    }
}

#[async_trait]
impl Capability for WriteFileToolCap {
    fn id(&self) -> &str {
        "write_to_file"
    }
    fn metadata(&self) -> &CapabilityMetadata {
        &self.meta
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, e: bool) {
        self.enabled = e;
    }
    fn subscribed_events(&self) -> Vec<EventType> {
        vec![EventType::PreTool]
    }
    async fn on_event(&self, _event: &Event) -> Result<(), String> {
        Ok(())
    }
}

// ShellTool
pub struct ShellToolCap {
    enabled: bool,
    meta: CapabilityMetadata,
}

impl ShellToolCap {
    pub fn new() -> Self {
        Self {
            enabled: true,
            meta: CapabilityMetadata {
                id: "run_shell".into(),
                name: "Shell Executor".into(),
                description: "Execute shell commands".into(),
                version: "1.0.0".into(),
                source: "builtin".into(),
                kind: CapabilityKind::Tool,
            },
        }
    }
}

#[async_trait]
impl Capability for ShellToolCap {
    fn id(&self) -> &str {
        "run_shell"
    }
    fn metadata(&self) -> &CapabilityMetadata {
        &self.meta
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, e: bool) {
        self.enabled = e;
    }
    fn subscribed_events(&self) -> Vec<EventType> {
        vec![EventType::PreTool]
    }
    async fn on_event(&self, _event: &Event) -> Result<(), String> {
        Ok(())
    }
}

// SearchTool
pub struct SearchToolCap {
    enabled: bool,
    meta: CapabilityMetadata,
}

impl SearchToolCap {
    pub fn new() -> Self {
        Self {
            enabled: true,
            meta: CapabilityMetadata {
                id: "search_files".into(),
                name: "File Searcher".into(),
                description: "Search files by glob/grep".into(),
                version: "1.0.0".into(),
                source: "builtin".into(),
                kind: CapabilityKind::Tool,
            },
        }
    }
}

#[async_trait]
impl Capability for SearchToolCap {
    fn id(&self) -> &str {
        "search_files"
    }
    fn metadata(&self) -> &CapabilityMetadata {
        &self.meta
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, e: bool) {
        self.enabled = e;
    }
    fn subscribed_events(&self) -> Vec<EventType> {
        vec![EventType::PreTool]
    }
    async fn on_event(&self, _event: &Event) -> Result<(), String> {
        Ok(())
    }
}
