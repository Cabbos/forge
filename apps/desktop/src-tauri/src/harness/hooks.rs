use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::harness::mcp;

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
    fn filter_tools(&self) -> Vec<String> {
        vec![]
    }

    /// Called before a tool executes. Return modified input or block.
    async fn on_pre_tool(
        &self,
        _session_id: &str,
        _tool: &str,
        input: serde_json::Value,
    ) -> HookDecision {
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
        Self {
            hooks: RwLock::new(Vec::new()),
        }
    }

    pub fn register(&self, hook: impl Hook + 'static) {
        self.hooks.write().unwrap().push(Arc::new(hook));
    }

    fn matches(hook: &dyn Hook, tool: &str, trigger: &HookTrigger) -> bool {
        let triggers = hook.triggers();
        if !triggers.contains(trigger) {
            return false;
        }
        let filter = hook.filter_tools();
        if filter.is_empty() {
            return true;
        }
        filter.iter().any(|t| t == tool)
    }

    pub async fn run_pre_tool(
        &self,
        session_id: &str,
        tool: &str,
        input: &serde_json::Value,
    ) -> HookDecision {
        self.run_pre_tool_with_enabled(session_id, tool, input, |_| true)
            .await
    }

    pub async fn run_pre_tool_with_enabled<F>(
        &self,
        session_id: &str,
        tool: &str,
        input: &serde_json::Value,
        is_enabled: F,
    ) -> HookDecision
    where
        F: Fn(&str) -> bool,
    {
        // Collect matching hooks first, drop read guard before awaiting
        let matching: Vec<Arc<dyn Hook>> = {
            let hooks = self.hooks.read().unwrap();
            hooks
                .iter()
                .filter(|h| Self::matches(h.as_ref(), tool, &HookTrigger::PreTool))
                .filter(|h| is_enabled(h.name()))
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
        self.run_post_tool_with_enabled(session_id, tool, result, |_| true)
            .await
    }

    pub async fn run_post_tool_with_enabled<F>(
        &self,
        session_id: &str,
        tool: &str,
        result: &str,
        is_enabled: F,
    ) -> String
    where
        F: Fn(&str) -> bool,
    {
        let matching: Vec<Arc<dyn Hook>> = {
            let hooks = self.hooks.read().unwrap();
            hooks
                .iter()
                .filter(|h| Self::matches(h.as_ref(), tool, &HookTrigger::PostTool))
                .filter(|h| is_enabled(h.name()))
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

impl Default for HookEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Built-in hooks ──

pub struct LoggingHook;

#[async_trait::async_trait]
impl Hook for LoggingHook {
    fn name(&self) -> &str {
        "logging"
    }
    fn triggers(&self) -> Vec<HookTrigger> {
        vec![HookTrigger::PreTool, HookTrigger::PostTool]
    }

    async fn on_pre_tool(
        &self,
        session_id: &str,
        tool: &str,
        input: serde_json::Value,
    ) -> HookDecision {
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
    fn name(&self) -> &str {
        "fs-audit"
    }
    fn triggers(&self) -> Vec<HookTrigger> {
        vec![HookTrigger::PostTool]
    }
    fn filter_tools(&self) -> Vec<String> {
        vec![
            "write_to_file".into(),
            "edit_file".into(),
            "run_shell".into(),
        ]
    }

    async fn on_post_tool(&self, session_id: &str, tool: &str, result: String) -> String {
        log::info!("[AUDIT][{session_id}] {tool} completed");
        result
    }
}

pub struct SensitiveContentHook;

#[async_trait::async_trait]
impl Hook for SensitiveContentHook {
    fn name(&self) -> &str {
        "sensitive-content"
    }

    fn triggers(&self) -> Vec<HookTrigger> {
        vec![HookTrigger::PreTool]
    }

    fn filter_tools(&self) -> Vec<String> {
        Vec::new()
    }

    async fn on_pre_tool(
        &self,
        _session_id: &str,
        tool: &str,
        input: serde_json::Value,
    ) -> HookDecision {
        if sensitive_tool_text(tool, &input)
            .iter()
            .any(|text| looks_like_secret(text))
        {
            return HookDecision::Block(
                "已阻止：工具输入中疑似包含敏感信息，请移除密钥、令牌或私钥后再继续。".to_string(),
            );
        }

        HookDecision::Proceed(input)
    }
}

pub struct WorkspaceBoundaryHook {
    working_dir: PathBuf,
}

impl WorkspaceBoundaryHook {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

#[async_trait::async_trait]
impl Hook for WorkspaceBoundaryHook {
    fn name(&self) -> &str {
        "workspace-boundary"
    }

    fn triggers(&self) -> Vec<HookTrigger> {
        vec![HookTrigger::PreTool]
    }

    fn filter_tools(&self) -> Vec<String> {
        vec![
            "read_file".into(),
            "write_to_file".into(),
            "edit_file".into(),
            "list_directory".into(),
            "search_files".into(),
            "search_content".into(),
        ]
    }

    async fn on_pre_tool(
        &self,
        _session_id: &str,
        _tool: &str,
        input: serde_json::Value,
    ) -> HookDecision {
        let Some(path) = input.get("path").and_then(|value| value.as_str()) else {
            return HookDecision::Proceed(input);
        };
        if path.trim().is_empty() {
            return HookDecision::Proceed(input);
        }

        match ensure_path_in_workspace(&self.working_dir, path) {
            Ok(()) => HookDecision::Proceed(input),
            Err(reason) => HookDecision::Block(reason),
        }
    }
}

pub(crate) fn sensitive_tool_text(tool: &str, input: &serde_json::Value) -> Vec<String> {
    if mcp::is_public_tool_name(tool) {
        return collect_json_strings(input);
    }

    match tool {
        "write_to_file" => input
            .get("content")
            .and_then(|value| value.as_str())
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        "edit_file" => ["old_string", "new_string"]
            .iter()
            .filter_map(|key| input.get(*key).and_then(|value| value.as_str()))
            .map(str::to_string)
            .collect(),
        "run_shell" => input
            .get("command")
            .and_then(|value| value.as_str())
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

pub(crate) fn collect_json_strings(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::String(text) => vec![text.clone()],
        serde_json::Value::Array(values) => values.iter().flat_map(collect_json_strings).collect(),
        serde_json::Value::Object(values) => {
            values.values().flat_map(collect_json_strings).collect()
        }
        _ => Vec::new(),
    }
}

pub(crate) fn looks_like_secret(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let patterns = [
        r"sk-[A-Za-z0-9_\-]{16,}",
        r"ghp_[A-Za-z0-9_]{16,}",
        r"gho_[A-Za-z0-9_]{16,}",
        r"ghu_[A-Za-z0-9_]{16,}",
        r"ghs_[A-Za-z0-9_]{16,}",
        r"ghr_[A-Za-z0-9_]{16,}",
        r"github_pat_[A-Za-z0-9_]{20,}",
        r"AIza[0-9A-Za-z_\-]{20,}",
        r"AKIA[0-9A-Z]{16}",
        r"-----BEGIN [A-Z ]+PRIVATE KEY-----",
        r"(?i)\btoken\s*[:=]\s*[A-Za-z0-9._~+/=-]{8,}",
        r"(?i)\bmy\s+token\s+(?:is|=|:)\s*[A-Za-z0-9._~+/=-]{8,}",
        r"(?i)\b(?:auth|access)\s+token(?:\s*(?:is|=|:))?\s+[A-Za-z0-9._~+/=-]{8,}",
        r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{8,}",
    ];

    patterns.iter().any(|pattern| {
        regex::Regex::new(pattern)
            .map(|regex| regex.is_match(trimmed))
            .unwrap_or(false)
    })
}

pub(crate) fn ensure_path_in_workspace(
    working_dir: &std::path::Path,
    path: &str,
) -> Result<(), String> {
    let requested = std::path::Path::new(path);
    let resolved = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        working_dir.join(requested)
    };
    let canonical = resolved.canonicalize().or_else(|_| {
        resolved
            .parent()
            .and_then(|parent| {
                let parent = parent.canonicalize().ok()?;
                let file_name = resolved.file_name()?;
                Some(parent.join(file_name))
            })
            .ok_or_else(|| format!("无法确认路径是否安全：{}", resolved.display()))
    })?;
    let workspace = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());

    if canonical.starts_with(&workspace) {
        Ok(())
    } else {
        Err(format!(
            "已阻止：这个操作会访问项目目录之外的文件。\n目标：{}\n项目：{}",
            canonical.display(),
            workspace.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Hook, HookDecision, SensitiveContentHook};

    #[tokio::test]
    async fn sensitive_content_hook_blocks_secret_mcp_tool_input() {
        let hook = SensitiveContentHook;
        let decision = hook
            .on_pre_tool(
                "session-1",
                "mcp__notes__save_note",
                serde_json::json!({
                    "title": "deploy notes",
                    "body": "API key: sk-1234567890abcdefghijkl"
                }),
            )
            .await;

        match decision {
            HookDecision::Block(reason) => {
                assert!(reason.contains("敏感信息"));
            }
            HookDecision::Proceed(_) => panic!("secret-like MCP input should be blocked"),
        }
    }

    #[tokio::test]
    async fn hook_engine_runs_sensitive_content_hook_for_dynamic_mcp_tools() {
        let engine = super::HookEngine::new();
        engine.register(SensitiveContentHook);

        let decision = engine
            .run_pre_tool(
                "session-1",
                "mcp__notes__save_note",
                &serde_json::json!({
                    "body": "bearer sk-1234567890abcdefghijkl"
                }),
            )
            .await;

        assert!(matches!(decision, HookDecision::Block(_)));
    }
}
