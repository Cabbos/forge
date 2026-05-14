//! HarnessCore — unified agent orchestration combining Claude Code's
//! hooks/skills/permissions model with Hermes' agent-centric streaming architecture.

pub mod capabilities;
pub mod capability;
pub mod db;
pub mod event_bus;
pub mod hooks;
pub mod permissions;
pub mod registry;
pub mod skills;
pub mod write_boundary;

use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use crate::executor::ToolExecutor;
use crate::harness::capabilities::skills::SkillLoaderCap;
use crate::harness::capabilities::tools;
use crate::harness::db::Database;
use crate::harness::registry::CapabilityRegistry;
use crate::harness::write_boundary::build_write_boundary;
use event_bus::EventBus;
use hooks::{FileSystemAuditHook, HookEngine, LoggingHook};
use permissions::{PermissionDecision, PermissionGate};
use skills::SkillLoader;

/// Central harness that wires together all agent subsystems.
pub struct Harness {
    pub hook_engine: Arc<HookEngine>,
    pub skill_loader: Arc<SkillLoader>,
    pub permission_gate: Arc<PermissionGate>,
    pub event_bus: EventBus,
    pub capability_registry: Arc<CapabilityRegistry>,
    pub database: Arc<Database>,
    /// Pending confirmations (block_id → oneshot sender)
    pub pending_confirms:
        Arc<RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
    /// Internal tool executor — used by the execute_tool pipeline.
    tool_executor: Arc<ToolExecutor>,
    /// Working directory for this session — used to discover project files (CLAUDE.md etc.)
    pub working_dir: PathBuf,
}

impl Harness {
    pub fn new(working_dir: PathBuf) -> Self {
        Self::new_with_pending(
            working_dir,
            Arc::new(RwLock::new(std::collections::HashMap::new())),
        )
    }

    pub fn new_with_pending(
        working_dir: PathBuf,
        pending_confirms: Arc<
            RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
        >,
    ) -> Self {
        let hook_engine = Arc::new(HookEngine::new());
        let skill_loader = Arc::new(SkillLoader::new_for_workspace(&working_dir));
        let event_bus = EventBus::new();
        let tool_executor = Arc::new(ToolExecutor::new(
            working_dir.clone(),
            pending_confirms.clone(),
        ));

        // Open SQLite database at <working_dir>/.forge/registry.db
        let db_path = working_dir.join(".forge").join("registry.db");
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let database =
            Arc::new(Database::open(&db_path).expect("Failed to open registry database"));
        skill_loader.attach_database(database.clone());

        let permission_gate = Arc::new(PermissionGate::new(database.clone()));

        // Create CapabilityRegistry backed by the database
        let capability_registry = Arc::new(CapabilityRegistry::new(database.clone()));

        // Register all builtin capabilities synchronously (before tokio runtime starts)
        capability_registry.register(Box::new(tools::FileToolCap::new()));
        capability_registry.register(Box::new(tools::WriteFileToolCap::new()));
        capability_registry.register(Box::new(tools::BuiltinToolCap::new(
            "edit_file",
            "File Editor",
            "Edit existing files with targeted replacements",
        )));
        capability_registry.register(Box::new(tools::BuiltinToolCap::new(
            "list_directory",
            "Directory Lister",
            "List workspace files and folders",
        )));
        capability_registry.register(Box::new(tools::BuiltinToolCap::new(
            "search_content",
            "Content Searcher",
            "Search text inside workspace files",
        )));
        capability_registry.register(Box::new(tools::ShellToolCap::new()));
        capability_registry.register(Box::new(tools::SearchToolCap::new()));
        capability_registry.register(Box::new(tools::BuiltinToolCap::new(
            "web_search",
            "Web Search",
            "Search the web for current documentation or references",
        )));
        capability_registry.register(Box::new(tools::BuiltinToolCap::new(
            "web_fetch",
            "Web Fetch",
            "Fetch and summarize a web page",
        )));
        capability_registry.register(Box::new(tools::BuiltinToolCap::new(
            "git_diff",
            "Git Diff",
            "Inspect uncommitted git changes",
        )));
        capability_registry.register(Box::new(tools::BuiltinToolCap::new(
            "ask_user",
            "Ask User",
            "Ask the user for a decision or clarification",
        )));
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
    pub async fn build_system_prompt(
        &self,
        provider: &str,
        working_dir: &std::path::Path,
    ) -> String {
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
            Default to reading files before editing, making targeted edits, and verifying with build/test commands.\n\
            For small-tool creation requests, prefer a previewable first version: visible, clickable, and continueable.\n\
            Keep the first version scoped; explain what is included, what is not included yet, and the next step.",
            provider
        ));

        // Project context (CLAUDE.md etc.)
        if let Some(ctx) = &project_ctx {
            parts.push(format!("## Project Context\n\n{}", ctx));
            crate::app_log!(
                "INFO",
                "[harness] Loaded project context: {} chars",
                ctx.len()
            );
        } else {
            crate::app_log!(
                "INFO",
                "[harness] No project context file found in {}",
                self.working_dir.display()
            );
        }

        // Active skills
        if !skill_prompts.is_empty() {
            let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
            crate::app_log!("INFO", "[harness] Active skills: {:?}", names);
            parts.push(format!(
                "## Active Skills\n\n{}",
                skill_prompts.join("\n\n---\n\n")
            ));
        } else {
            crate::app_log!("INFO", "[harness] No active skills");
        }

        let result = parts.join("\n\n");
        crate::app_log!(
            "INFO",
            "[harness] System prompt built: {} chars total",
            result.len()
        );
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
        self.execute_tool_with_block_id(session_id, tool_name, tool_input, app_handle, None)
            .await
    }

    pub async fn execute_tool_with_block_id(
        &self,
        session_id: &str,
        tool_name: &str,
        tool_input: &serde_json::Value,
        app_handle: &AppHandle,
        tool_block_id: Option<&str>,
    ) -> String {
        if !self.capability_registry.is_tool_enabled(tool_name) {
            let result = format!("Tool disabled by capability settings: {}", tool_name);
            emit_blocked_tool_result(session_id, tool_block_id, &result, app_handle);
            return result;
        }

        // 1. Pre-tool hooks (can modify input or block)
        let modified_input = self
            .hook_engine
            .run_pre_tool(session_id, tool_name, tool_input)
            .await;

        match modified_input {
            hooks::HookDecision::Block(reason) => {
                let result = format!("Tool execution blocked by hook: {reason}");
                emit_blocked_tool_result(session_id, tool_block_id, &result, app_handle);
                return result;
            }
            hooks::HookDecision::Proceed(input) => {
                // 2. Permission check — ask user if not pre-approved
                match self
                    .permission_gate
                    .check(session_id, tool_name, &input, &self.working_dir)
                    .await
                {
                    PermissionDecision::Allow => {}
                    PermissionDecision::Deny { reason } => {
                        emit_blocked_tool_result(session_id, tool_block_id, &reason, app_handle);
                        return reason;
                    }
                    PermissionDecision::Ask {
                        question,
                        kind,
                        remember_key,
                    } => {
                        let boundary =
                            build_write_boundary(tool_name, &input, &self.working_dir, &kind);
                        let block_id = uuid::Uuid::now_v7().to_string();
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        {
                            self.pending_confirms
                                .write()
                                .await
                                .insert(block_id.clone(), tx);
                        }
                        let _ = app_handle.emit(
                            "session-output",
                            crate::protocol::events::StreamEvent::ConfirmAsk {
                                session_id: session_id.to_string(),
                                block_id: block_id.clone(),
                                question,
                                kind,
                                boundary: Some(boundary),
                            },
                        );
                        // Wait 120s for user response
                        let approved =
                            match tokio::time::timeout(std::time::Duration::from_secs(120), rx)
                                .await
                            {
                                Ok(Ok(true)) => {
                                    if let Some(key) = remember_key {
                                        self.permission_gate
                                            .approve_in_session(session_id, &key)
                                            .await;
                                    }
                                    true
                                }
                                _ => false,
                            };
                        self.pending_confirms.write().await.remove(&block_id);
                        if !approved {
                            let result = "Permission denied by user".to_string();
                            emit_blocked_tool_result(
                                session_id,
                                tool_block_id,
                                &result,
                                app_handle,
                            );
                            return result;
                        }
                    }
                }

                // 3. Execute via tool executor
                let result = self
                    .tool_executor
                    .execute(session_id, tool_name, &input, app_handle, tool_block_id)
                    .await;

                // 4. Post-tool hooks (can modify result)
                let modified_result = self
                    .hook_engine
                    .run_post_tool(session_id, tool_name, &result)
                    .await;

                modified_result
            }
        }
    }
}

fn emit_blocked_tool_result(
    session_id: &str,
    tool_block_id: Option<&str>,
    result: &str,
    app_handle: &AppHandle,
) {
    let block_id = tool_block_id
        .map(str::to_string)
        .unwrap_or_else(|| crate::protocol::BlockId::new().to_string());
    let _ = app_handle.emit(
        "session-output",
        crate::protocol::events::StreamEvent::ToolCallResult {
            session_id: session_id.to_string(),
            block_id,
            result: result.to_string(),
            is_error: true,
            duration_ms: 0,
        },
    );
}

/// Read project context from working directory.
/// Tries CLAUDE.md first, then AGENTS.md, GEMINI.md.
pub fn read_project_context(working_dir: &std::path::Path) -> Option<String> {
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
