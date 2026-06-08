use crate::harness::capability::{Capability, CapabilityKind, CapabilityMetadata};
use async_trait::async_trait;

pub struct BuiltinHookCap {
    hook_name: String,
    enabled: bool,
    meta: CapabilityMetadata,
}

impl BuiltinHookCap {
    pub fn new(
        hook_name: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        let hook_name = hook_name.into();
        Self {
            meta: CapabilityMetadata {
                id: format!("hook:{hook_name}"),
                name: display_name.into(),
                description: description.into(),
                version: "1.0.0".into(),
                source: "builtin".into(),
                kind: CapabilityKind::Hook,
            },
            hook_name,
            enabled: true,
        }
    }
}

#[async_trait]
impl Capability for BuiltinHookCap {
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
