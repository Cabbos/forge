use async_trait::async_trait;
use std::sync::Arc;
use crate::harness::capability::{Capability, CapabilityKind, CapabilityMetadata, Event, EventType};
use crate::harness::skills::SkillLoader;

pub struct SkillLoaderCap {
    pub loader: Arc<SkillLoader>,
    enabled: bool,
    meta: CapabilityMetadata,
}

impl SkillLoaderCap {
    pub fn new(loader: Arc<SkillLoader>) -> Self {
        Self {
            loader,
            enabled: true,
            meta: CapabilityMetadata {
                id: "skill-loader".into(),
                name: "Skill Loader".into(),
                description: "Loads and manages SKILL.md files from ~/.forge/skills/".into(),
                version: "1.0.0".into(),
                source: "builtin".into(),
                kind: CapabilityKind::Skill,
            },
        }
    }
}

#[async_trait]
impl Capability for SkillLoaderCap {
    fn id(&self) -> &str {
        "skill-loader"
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
        vec![EventType::SessionStart]
    }

    async fn on_event(&self, _event: &Event) -> Result<(), String> {
        self.loader.scan_all().await;
        Ok(())
    }
}
