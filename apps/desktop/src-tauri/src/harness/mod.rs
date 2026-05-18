//! HarnessCore — unified agent orchestration combining Claude Code's
//! hooks/skills/permissions model with Hermes' agent-centric streaming architecture.

pub mod capabilities;
pub mod capability;
pub mod db;
pub mod event_bus;
pub mod hooks;
pub mod mcp;
pub mod permissions;
pub mod registry;
pub mod skills;
pub mod write_boundary;

use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use crate::adapters::anthropic::ToolDef;
use crate::executor::ToolExecutor;
use crate::harness::capabilities::hooks::BuiltinHookCap;
use crate::harness::capabilities::mcp::McpServerCap;
use crate::harness::capabilities::skills::SkillLoaderCap;
use crate::harness::capabilities::tools;
use crate::harness::db::Database;
use crate::harness::registry::CapabilityRegistry;
use crate::harness::write_boundary::build_write_boundary;
use event_bus::EventBus;
use hooks::{
    FileSystemAuditHook, HookEngine, LoggingHook, SensitiveContentHook, WorkspaceBoundaryHook,
};
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
    /// Workspace MCP server definitions discovered from `.forge/mcp.json`.
    mcp_servers: Vec<mcp::McpServerDefinition>,
    /// Cached MCP tool discovery for the current enabled connector set.
    mcp_tool_cache: Arc<RwLock<Option<McpToolCache>>>,
    /// Cached MCP resource discovery for the current enabled connector set.
    mcp_resource_cache: Arc<RwLock<Option<McpResourceCache>>>,
    /// Cached MCP prompt discovery for the current enabled connector set.
    mcp_prompt_cache: Arc<RwLock<Option<McpPromptCache>>>,
    /// Working directory for this session — used to discover project files (CLAUDE.md etc.)
    pub working_dir: PathBuf,
}

#[derive(Clone)]
struct McpToolCache {
    enabled_server_ids: Vec<String>,
    tools: Vec<ResolvedMcpTool>,
}

#[derive(Clone)]
struct ResolvedMcpTool {
    server: mcp::McpServerDefinition,
    tool: mcp::McpToolDefinition,
    public_name: String,
}

#[derive(Clone)]
struct McpResourceCache {
    enabled_server_ids: Vec<String>,
    resources: Vec<mcp::McpResourceDefinition>,
}

#[derive(Clone)]
struct McpPromptCache {
    enabled_server_ids: Vec<String>,
    prompts: Vec<mcp::McpPromptDefinition>,
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
        capability_registry.register(Box::new(BuiltinHookCap::new(
            "logging",
            "Logging Hook",
            "Records tool execution lifecycle for diagnostics",
        )));
        capability_registry.register(Box::new(BuiltinHookCap::new(
            "fs-audit",
            "File System Audit Hook",
            "Audits write and shell operations after execution",
        )));
        capability_registry.register(Box::new(BuiltinHookCap::new(
            "sensitive-content",
            "Sensitive Content Guard",
            "Blocks tool inputs that appear to contain secrets or tokens",
        )));
        capability_registry.register(Box::new(BuiltinHookCap::new(
            "workspace-boundary",
            "Workspace Boundary Guard",
            "Blocks file operations outside the active workspace",
        )));
        let mcp_servers = mcp::load_mcp_servers(&working_dir);
        for server in mcp_servers.clone() {
            capability_registry.register(Box::new(McpServerCap::new(server)));
        }

        // Register built-in hooks
        hook_engine.register(LoggingHook);
        hook_engine.register(FileSystemAuditHook);
        hook_engine.register(SensitiveContentHook);
        hook_engine.register(WorkspaceBoundaryHook::new(working_dir.clone()));

        Harness {
            hook_engine,
            skill_loader,
            permission_gate,
            event_bus,
            capability_registry,
            database,
            tool_executor,
            mcp_servers,
            mcp_tool_cache: Arc::new(RwLock::new(None)),
            mcp_resource_cache: Arc::new(RwLock::new(None)),
            mcp_prompt_cache: Arc::new(RwLock::new(None)),
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
        self.build_system_prompt_for_request(provider, working_dir, None)
            .await
    }

    pub async fn build_system_prompt_for_request(
        &self,
        provider: &str,
        working_dir: &std::path::Path,
        user_request: Option<&str>,
    ) -> String {
        // Ensure skills are scanned before reading
        self.skill_loader.scan_all().await;
        let skills = if let Some(request) = user_request {
            self.skill_loader.enabled_skills_for_request(request).await
        } else {
            self.skill_loader.enabled_skills().await
        };
        let skill_prompts: Vec<String> = skills.iter().map(|s| s.instruction.clone()).collect();

        // Read project context from working directory (CLAUDE.md, AGENTS.md, etc.)
        let project_ctx = read_project_context(working_dir);

        let mut parts: Vec<String> = Vec::new();

        // Always include a minimal role prompt
        parts.push(format!(
            "You are a coding agent running in a desktop app with filesystem and shell access. Provider: {}.\n\
            You have tools for reading/writing files, running shell commands, searching code, and web access.\n\
            Default to reading files before editing, making targeted edits, and verifying with build/test commands.\n\
            Answer in the user's language by default.\n\
            When asked what was discussed before, summarize only the retained visible conversation and clearly say when older context is unavailable; do not invent.\n\
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

    pub async fn external_mcp_tool_definitions(&self) -> Vec<ToolDef> {
        let mut definitions = Vec::new();
        for resolved in self.resolved_mcp_tools().await {
            let description = format_mcp_tool_description(&resolved.server, &resolved.tool);
            definitions.push(ToolDef {
                name: resolved.public_name,
                description,
                input_schema: resolved.tool.input_schema,
            });
        }
        definitions
    }

    pub async fn external_mcp_resource_definitions(&self) -> Vec<mcp::McpResourceDefinition> {
        let enabled_server_ids = self.enabled_mcp_server_ids();
        {
            let cache = self.mcp_resource_cache.read().await;
            if let Some(cache) = cache.as_ref() {
                if cache.enabled_server_ids == enabled_server_ids {
                    return cache.resources.clone();
                }
            }
        }

        let mut resources = Vec::new();
        for server in &self.mcp_servers {
            if !enabled_server_ids.contains(&server.id) {
                continue;
            }
            let Ok(discovered_resources) = mcp::discover_stdio_resources(server).await else {
                continue;
            };
            resources.extend(discovered_resources);
        }

        let mut cache = self.mcp_resource_cache.write().await;
        *cache = Some(McpResourceCache {
            enabled_server_ids,
            resources: resources.clone(),
        });

        resources
    }

    pub async fn external_mcp_prompt_definitions(&self) -> Vec<mcp::McpPromptDefinition> {
        let enabled_server_ids = self.enabled_mcp_server_ids();
        {
            let cache = self.mcp_prompt_cache.read().await;
            if let Some(cache) = cache.as_ref() {
                if cache.enabled_server_ids == enabled_server_ids {
                    return cache.prompts.clone();
                }
            }
        }

        let mut prompts = Vec::new();
        for server in &self.mcp_servers {
            if !enabled_server_ids.contains(&server.id) {
                continue;
            }
            let Ok(discovered_prompts) = mcp::discover_stdio_prompts(server).await else {
                continue;
            };
            prompts.extend(discovered_prompts);
        }

        let mut cache = self.mcp_prompt_cache.write().await;
        *cache = Some(McpPromptCache {
            enabled_server_ids,
            prompts: prompts.clone(),
        });

        prompts
    }

    pub async fn read_mcp_resource(
        &self,
        server_id: &str,
        uri: &str,
    ) -> Result<Vec<mcp::McpResourceContent>, String> {
        let server = self.enabled_mcp_server(server_id)?;
        mcp::read_stdio_resource(&server, uri).await
    }

    pub async fn get_mcp_prompt(
        &self,
        server_id: &str,
        prompt_name: &str,
        arguments: serde_json::Value,
    ) -> Result<Vec<mcp::McpPromptMessage>, String> {
        let server = self.enabled_mcp_server(server_id)?;
        mcp::get_stdio_prompt(&server, prompt_name, arguments).await
    }

    pub async fn call_public_mcp_tool(
        &self,
        public_tool_name: &str,
        input: serde_json::Value,
    ) -> Option<String> {
        if !mcp::is_public_tool_name(public_tool_name) {
            return None;
        }
        for resolved in self.resolved_mcp_tools().await {
            if resolved.public_name == public_tool_name {
                return Some(
                    mcp::call_stdio_tool(&resolved.server, &resolved.tool.name, input)
                        .await
                        .unwrap_or_else(|err| format!("Error: {err}")),
                );
            }
        }
        Some(format!("Unknown MCP tool: {public_tool_name}"))
    }

    async fn resolved_mcp_tools(&self) -> Vec<ResolvedMcpTool> {
        let enabled_server_ids = self.enabled_mcp_server_ids();
        {
            let cache = self.mcp_tool_cache.read().await;
            if let Some(cache) = cache.as_ref() {
                if cache.enabled_server_ids == enabled_server_ids {
                    return cache.tools.clone();
                }
            }
        }

        let mut tools = Vec::new();
        for server in &self.mcp_servers {
            if !enabled_server_ids.contains(&server.id) {
                continue;
            }
            let Ok(discovered_tools) = mcp::discover_stdio_tools(server).await else {
                continue;
            };
            for tool in discovered_tools {
                let public_name = mcp::public_tool_name(&server.id, &tool.name);
                tools.push(ResolvedMcpTool {
                    server: server.clone(),
                    tool,
                    public_name,
                });
            }
        }

        let mut cache = self.mcp_tool_cache.write().await;
        *cache = Some(McpToolCache {
            enabled_server_ids,
            tools: tools.clone(),
        });

        tools
    }

    fn enabled_mcp_server_ids(&self) -> Vec<String> {
        self.mcp_servers
            .iter()
            .filter(|server| self.capability_registry.is_mcp_enabled(&server.id))
            .map(|server| server.id.clone())
            .collect()
    }

    fn enabled_mcp_server(&self, server_id: &str) -> Result<mcp::McpServerDefinition, String> {
        let enabled_server_ids = self.enabled_mcp_server_ids();
        self.mcp_servers
            .iter()
            .find(|server| server.id == server_id && enabled_server_ids.contains(&server.id))
            .cloned()
            .ok_or_else(|| format!("MCP server '{server_id}' is not enabled"))
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
            .run_pre_tool_with_enabled(session_id, tool_name, tool_input, |hook| {
                self.capability_registry.is_hook_enabled(hook)
            })
            .await;

        match modified_input {
            hooks::HookDecision::Block(reason) => {
                let result = format!("Tool execution blocked by hook: {reason}");
                emit_blocked_tool_result(session_id, tool_block_id, &result, app_handle);
                result
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
                if mcp::is_public_tool_name(tool_name) {
                    let started = std::time::Instant::now();
                    let result = self
                        .call_public_mcp_tool(tool_name, input.clone())
                        .await
                        .unwrap_or_else(|| format!("Unknown MCP tool: {tool_name}"));
                    emit_tool_result(
                        session_id,
                        tool_block_id,
                        &result,
                        result.starts_with("Error:") || result.starts_with("Unknown MCP tool:"),
                        started.elapsed().as_millis() as u64,
                        app_handle,
                    );

                    return self
                        .hook_engine
                        .run_post_tool_with_enabled(session_id, tool_name, &result, |hook| {
                            self.capability_registry.is_hook_enabled(hook)
                        })
                        .await;
                }

                let result = self
                    .tool_executor
                    .execute(session_id, tool_name, &input, app_handle, tool_block_id)
                    .await;

                // 4. Post-tool hooks (can modify result)
                let modified_result = self
                    .hook_engine
                    .run_post_tool_with_enabled(session_id, tool_name, &result, |hook| {
                        self.capability_registry.is_hook_enabled(hook)
                    })
                    .await;

                modified_result
            }
        }
    }
}

fn format_mcp_tool_description(
    server: &mcp::McpServerDefinition,
    tool: &mcp::McpToolDefinition,
) -> String {
    if tool.description.trim().is_empty() {
        format!("MCP connector {} tool {}", server.name, tool.name)
    } else {
        format!("MCP connector {}: {}", server.name, tool.description)
    }
}

fn emit_blocked_tool_result(
    session_id: &str,
    tool_block_id: Option<&str>,
    result: &str,
    app_handle: &AppHandle,
) {
    emit_tool_result(session_id, tool_block_id, result, true, 0, app_handle);
}

fn emit_tool_result(
    session_id: &str,
    tool_block_id: Option<&str>,
    result: &str,
    is_error: bool,
    duration_ms: u64,
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
            is_error,
            duration_ms,
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
