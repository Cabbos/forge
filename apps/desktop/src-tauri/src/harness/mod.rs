//! HarnessCore — unified agent orchestration combining Claude Code's
//! hooks/skills/permissions model with Hermes' agent-centric streaming architecture.

pub mod hooks;
pub mod skills;
pub mod permissions;
pub mod capability;
pub mod capabilities;
pub mod db;
pub mod registry;
pub mod event_bus;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{AppHandle, Emitter};

use hooks::{HookEngine, LoggingHook, FileSystemAuditHook};
use skills::SkillLoader;
use permissions::PermissionGate;
use event_bus::EventBus;
use crate::executor::ToolExecutor;
use crate::harness::capabilities::skills::SkillLoaderCap;
use crate::harness::capabilities::tools;
use crate::harness::db::Database;
use crate::harness::registry::CapabilityRegistry;

/// Central harness that wires together all agent subsystems.
pub struct Harness {
    pub hook_engine: Arc<HookEngine>,
    pub skill_loader: Arc<SkillLoader>,
    pub permission_gate: Arc<PermissionGate>,
    pub event_bus: EventBus,
    pub capability_registry: Arc<CapabilityRegistry>,
    pub database: Arc<Database>,
    /// Pending confirmations (block_id → oneshot sender)
    pub pending_confirms: Arc<RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
    /// Internal tool executor — used by the execute_tool pipeline.
    tool_executor: Arc<ToolExecutor>,
    /// Working directory for this session — used to discover project files (CLAUDE.md etc.)
    working_dir: PathBuf,
}

impl Harness {
    pub fn new(working_dir: PathBuf) -> Self {
        let pending_confirms = Arc::new(RwLock::new(std::collections::HashMap::new()));
        let hook_engine = Arc::new(HookEngine::new());
        let skill_loader = Arc::new(SkillLoader::new());
        let event_bus = EventBus::new();
        let tool_executor = Arc::new(ToolExecutor::new(
            working_dir.clone(),
            pending_confirms.clone(),
        ));

        // Open SQLite database at <working_dir>/.ai-studio/registry.db
        let db_path = working_dir.join(".ai-studio").join("registry.db");
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let database = Arc::new(
            Database::open(&db_path)
                .expect("Failed to open registry database")
        );

        let permission_gate = Arc::new(PermissionGate::new(database.clone()));

        // Create CapabilityRegistry backed by the database
        let capability_registry = Arc::new(CapabilityRegistry::new(database.clone()));

        // Register all builtin capabilities synchronously (before tokio runtime starts)
        capability_registry.register(Box::new(tools::FileToolCap::new()));
        capability_registry.register(Box::new(tools::WriteFileToolCap::new()));
        capability_registry.register(Box::new(tools::ShellToolCap::new()));
        capability_registry.register(Box::new(tools::SearchToolCap::new()));
        capability_registry.register(Box::new(SkillLoaderCap::new(skill_loader.clone())));

        // Register built-in hooks
        hook_engine.register(LoggingHook);
        hook_engine.register(FileSystemAuditHook);

        Harness {
            hook_engine,
            skill_loader,
            permission_gate,
            event_bus,
            capability_registry,
            database,
            tool_executor,
            pending_confirms,
            working_dir,
        }
    }

    /// Full agent lifecycle: load skills, run hooks, build system prompt.
    pub async fn build_system_prompt(&self, provider: &str, working_dir: &std::path::Path) -> String {
        // Ensure skills are scanned before reading
        self.skill_loader.scan_all().await;
        let skills = self.skill_loader.enabled_skills().await;
        let skill_prompts: Vec<String> = skills.iter().map(|s| s.instruction.clone()).collect();

        // Read project context from working directory (CLAUDE.md, AGENTS.md, etc.)
        let project_ctx = read_project_context(working_dir);

        let mut parts: Vec<String> = Vec::new();

        // Always include a minimal role prompt
        parts.push(format!(
            "You are a coding agent running in a desktop app with filesystem and shell access. Provider: {}.\n\
            You have tools for reading/writing files, running shell commands, searching code, and web access.\n\
            Default to reading files before editing, making targeted edits, and verifying with build/test commands.",
            provider
        ));

        // Project context (CLAUDE.md etc.)
        if let Some(ctx) = &project_ctx {
            parts.push(format!("## Project Context\n\n{}", ctx));
            crate::app_log!("INFO", "[harness] Loaded project context: {} chars", ctx.len());
        } else {
            crate::app_log!("INFO", "[harness] No project context file found in {}", self.working_dir.display());
        }

        // Active skills
        if !skill_prompts.is_empty() {
            let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
            crate::app_log!("INFO", "[harness] Active skills: {:?}", names);
            parts.push(format!("## Active Skills\n\n{}", skill_prompts.join("\n\n---\n\n")));
        } else {
            crate::app_log!("INFO", "[harness] No active skills");
        }

        let result = parts.join("\n\n");
        crate::app_log!("INFO", "[harness] System prompt built: {} chars total", result.len());
        result
    }

    /// Dispatch a tool execution through the full hook + permission pipeline.
    /// Returns the tool result.
    pub async fn execute_tool(
        &self,
        session_id: &str,
        tool_name: &str,
        tool_input: &serde_json::Value,
        app_handle: &AppHandle,
    ) -> String {
        // 1. Pre-tool hooks (can modify input or block)
        let modified_input = self.hook_engine
            .run_pre_tool(session_id, tool_name, tool_input)
            .await;

        match modified_input {
            hooks::HookDecision::Block(reason) => {
                return format!("Tool execution blocked by hook: {reason}");
            }
            hooks::HookDecision::Proceed(input) => {
                // 2. Permission check — ask user if not pre-approved
                if !self.permission_gate.is_allowed(session_id, tool_name, &input).await {
                    // Emit ConfirmAsk and wait for user response
                    let question = format!("Allow {}?\n{:?}", tool_name, tool_input);
                    let kind = match tool_name {
                        "run_shell" | "bash" => "dangerous_cmd",
                        "write_to_file" | "write_file" | "edit_file" => "file_write",
                        _ => "confirm",
                    };
                    let block_id = uuid::Uuid::now_v7().to_string();
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    {
                        self.pending_confirms.write().await.insert(block_id.clone(), tx);
                    }
                    let _ = app_handle.emit("session-output",
                        crate::protocol::events::StreamEvent::ConfirmAsk {
                            session_id: session_id.to_string(),
                            block_id: block_id.clone(),
                            question,
                            kind: kind.to_string(),
                        });
                    // Wait 120s for user response
                    let approved = match tokio::time::timeout(
                        std::time::Duration::from_secs(120), rx).await
                    {
                        Ok(Ok(true)) => {
                            self.permission_gate.approve_in_session(session_id, tool_name).await;
                            true
                        }
                        _ => false,
                    };
                    self.pending_confirms.write().await.remove(&block_id);
                    if !approved {
                        return "Permission denied by user".to_string();
                    }
                }

                // 3. Execute via tool executor
                let result = self.tool_executor.execute(
                    session_id, tool_name, &input, app_handle,
                ).await;

                // 4. Post-tool hooks (can modify result)
                let modified_result = self.hook_engine
                    .run_post_tool(session_id, tool_name, &result)
                    .await;

                modified_result
            }
        }
    }
}

/// Read project context from working directory.
/// Tries CLAUDE.md first, then AGENTS.md, GEMINI.md.
fn read_project_context(working_dir: &std::path::Path) -> Option<String> {
    let candidates = ["CLAUDE.md", "AGENTS.md", "GEMINI.md"];
    for name in &candidates {
        let path = working_dir.join(name);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if !content.trim().is_empty() {
                return Some(content.trim().to_string());
            }
        }
    }
    None
}
