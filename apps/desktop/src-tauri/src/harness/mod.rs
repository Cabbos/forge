//! HarnessCore — unified agent orchestration combining Claude Code's
//! hooks/skills/permissions model with Hermes' agent-centric streaming architecture.

pub mod hooks;
pub mod skills;
pub mod permissions;
pub mod capability;
pub mod event_bus;

use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::AppHandle;

use hooks::HookEngine;
use skills::SkillLoader;
use permissions::PermissionGate;
use event_bus::EventBus;
use crate::adapters::base::AiAdapter;
use crate::executor::ToolExecutor;

/// Central harness that wires together all agent subsystems.
pub struct Harness {
    pub hook_engine: Arc<HookEngine>,
    pub skill_loader: Arc<SkillLoader>,
    pub permission_gate: Arc<PermissionGate>,
    pub event_bus: EventBus,
    pub tool_executor: Arc<ToolExecutor>,
    /// Pending confirmations (block_id → oneshot sender)
    pub pending_confirms: Arc<RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

impl Harness {
    pub fn new(working_dir: std::path::PathBuf) -> Self {
        let pending_confirms = Arc::new(RwLock::new(std::collections::HashMap::new()));
        let permission_gate = Arc::new(PermissionGate::new());
        let hook_engine = Arc::new(HookEngine::new());
        let skill_loader = Arc::new(SkillLoader::new());
        let event_bus = EventBus::new();
        let tool_executor = Arc::new(ToolExecutor::new(
            working_dir,
            pending_confirms.clone(),
        ));

        // Load built-in hooks
        let he = hook_engine.clone();
        tokio::spawn(async move {
            he.register(hooks::LoggingHook);
            he.register(hooks::FileSystemAuditHook);
        });

        Harness {
            hook_engine,
            skill_loader,
            permission_gate,
            event_bus,
            tool_executor,
            pending_confirms,
        }
    }

    /// Full agent lifecycle: load skills, run hooks, build system prompt.
    pub async fn build_system_prompt(&self, provider: &str) -> String {
        let skills = self.skill_loader.enabled_skills().await;
        let skill_prompts: Vec<String> = skills.iter().map(|s| s.instruction.clone()).collect();

        let base = format!(
            "You are a powerful AI coding agent. Provider: {}. \
            You have direct filesystem and shell access.\n\n\
            Core rules:\n\
            - Read files before editing\n\
            - Make targeted edits\n\
            - Verify with build/test commands\n\
            - Keep responses concise\n",
            provider
        );

        if skill_prompts.is_empty() {
            return base;
        }

        format!("{}\n\n## Active Skills\n\n{}", base, skill_prompts.join("\n\n---\n\n"))
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
                // 2. Permission check
                if !self.permission_gate.is_allowed(session_id, tool_name, &input).await {
                    return "Permission denied".to_string();
                }

                // 3. Execute via tool executor (emit events via event_bus)
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
