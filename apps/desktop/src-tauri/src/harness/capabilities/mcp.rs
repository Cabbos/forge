use crate::harness::capability::{Capability, CapabilityKind, CapabilityMetadata};
use crate::harness::mcp::McpServerDefinition;
use async_trait::async_trait;

pub struct McpServerCap {
    enabled: bool,
    meta: CapabilityMetadata,
}

impl McpServerCap {
    pub fn new(server: McpServerDefinition) -> Self {
        Self {
            enabled: server.enabled,
            meta: CapabilityMetadata {
                id: format!("mcp:{}", server.id),
                name: server.name,
                description: server.description,
                version: "1.0.0".into(),
                source: server.source,
                kind: CapabilityKind::McpServer,
            },
        }
    }
}

#[async_trait]
impl Capability for McpServerCap {
    fn id(&self) -> &str {
        &self.meta.id
    }

    fn metadata(&self) -> &CapabilityMetadata {
        &self.meta
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}
