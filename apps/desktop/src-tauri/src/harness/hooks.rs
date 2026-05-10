use std::sync::{Arc, RwLock};

/// Decision from a hook: proceed (possibly with modified data) or block.
#[derive(Debug, Clone)]
pub enum HookDecision {
    Proceed(serde_json::Value),
    Block(String),
}

/// A hook that intercepts tool execution.
#[async_trait::async_trait]
pub trait Hook: Send + Sync {
    fn name(&self) -> &str;
    /// Which trigger points this hook subscribes to.
    fn triggers(&self) -> Vec<HookTrigger>;
    /// Optional: only fire for these tools (empty = all tools).
    fn filter_tools(&self) -> Vec<String> { vec![] }

    /// Called before a tool executes. Return modified input or block.
    async fn on_pre_tool(&self, _session_id: &str, _tool: &str, input: serde_json::Value) -> HookDecision {
        HookDecision::Proceed(input)
    }
    /// Called after a tool executes. Return modified result.
    async fn on_post_tool(&self, _session_id: &str, _tool: &str, result: String) -> String {
        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookTrigger {
    PreTool,
    PostTool,
    SessionStart,
    SessionStop,
}

pub struct HookEngine {
    hooks: RwLock<Vec<Arc<dyn Hook>>>,
}

impl HookEngine {
    pub fn new() -> Self {
        Self { hooks: RwLock::new(Vec::new()) }
    }

    pub fn register(&self, hook: impl Hook + 'static) {
        self.hooks.write().unwrap().push(Arc::new(hook));
    }

    fn matches(hook: &dyn Hook, tool: &str, trigger: &HookTrigger) -> bool {
        let triggers = hook.triggers();
        if !triggers.contains(trigger) { return false; }
        let filter = hook.filter_tools();
        if filter.is_empty() { return true; }
        filter.iter().any(|t| t == tool)
    }

    pub async fn run_pre_tool(&self, session_id: &str, tool: &str, input: &serde_json::Value) -> HookDecision {
        // Collect matching hooks first, drop read guard before awaiting
        let matching: Vec<Arc<dyn Hook>> = {
            let hooks = self.hooks.read().unwrap();
            hooks.iter()
                .filter(|h| Self::matches(h.as_ref(), tool, &HookTrigger::PreTool))
                .cloned()
                .collect()
        };
        let mut current = input.clone();
        for h in matching {
            match h.on_pre_tool(session_id, tool, current).await {
                HookDecision::Block(reason) => return HookDecision::Block(reason),
                HookDecision::Proceed(modified) => current = modified,
            }
        }
        HookDecision::Proceed(current)
    }

    pub async fn run_post_tool(&self, session_id: &str, tool: &str, result: &str) -> String {
        let matching: Vec<Arc<dyn Hook>> = {
            let hooks = self.hooks.read().unwrap();
            hooks.iter()
                .filter(|h| Self::matches(h.as_ref(), tool, &HookTrigger::PostTool))
                .cloned()
                .collect()
        };
        let mut current = result.to_string();
        for h in matching {
            current = h.on_post_tool(session_id, tool, current).await;
        }
        current
    }
}

// ── Built-in hooks ──

pub struct LoggingHook;

#[async_trait::async_trait]
impl Hook for LoggingHook {
    fn name(&self) -> &str { "logging" }
    fn triggers(&self) -> Vec<HookTrigger> {
        vec![HookTrigger::PreTool, HookTrigger::PostTool]
    }

    async fn on_pre_tool(&self, session_id: &str, tool: &str, input: serde_json::Value) -> HookDecision {
        log::info!("[{session_id}] pre-tool: {tool}");
        HookDecision::Proceed(input)
    }

    async fn on_post_tool(&self, session_id: &str, tool: &str, result: String) -> String {
        log::info!("[{session_id}] post-tool: {tool} ({} chars)", result.len());
        result
    }
}

pub struct FileSystemAuditHook;

#[async_trait::async_trait]
impl Hook for FileSystemAuditHook {
    fn name(&self) -> &str { "fs-audit" }
    fn triggers(&self) -> Vec<HookTrigger> { vec![HookTrigger::PostTool] }
    fn filter_tools(&self) -> Vec<String> {
        vec!["write_to_file".into(), "edit_file".into(), "run_shell".into()]
    }

    async fn on_post_tool(&self, session_id: &str, tool: &str, result: String) -> String {
        log::info!("[AUDIT][{session_id}] {tool} completed");
        result
    }
}
