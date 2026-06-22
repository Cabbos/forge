use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Notify;

use crate::adapters::base::{AiAdapter, ChatMessage};
use crate::agent::event_sink::EventEmitter;
use crate::harness::Harness;
use crate::loop_runtime::{LoopUsageLedger, UsageEvent};
use crate::protocol::events::{ProviderUsageReason, StreamEvent, SubagentRuntimePayload};

const MAX_ROUNDS: usize = 20;
const MAX_RESULT_CHARS: usize = 8000;

/// Structured trace of one sub-agent round for the frontend viewer.
#[derive(Debug, Clone, Serialize)]
struct RoundTrace {
    round: usize,
    thinking: String,
    text: String,
    tool_calls: Vec<ToolCallTrace>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolCallTrace {
    name: String,
    input: String,
    result: String,
}

/// Final JSON payload returned to the frontend.
#[derive(Debug, Clone, Serialize)]
struct SubAgentResult {
    result: String,
    steps: Vec<RoundTrace>,
    usage: LoopUsageLedger,
}

struct SubAgentUsageTracker {
    model: Option<String>,
    started_at: Instant,
    events: Vec<UsageEvent>,
    turn_count: u32,
    tool_call_count: u32,
}

impl SubAgentUsageTracker {
    fn new(model: &str) -> Self {
        Self {
            model: (!model.trim().is_empty()).then(|| model.to_string()),
            started_at: Instant::now(),
            events: Vec::new(),
            turn_count: 0,
            tool_call_count: 0,
        }
    }

    fn record_model_round(
        &mut self,
        tool_call_count: usize,
        usage_events: Vec<UsageEvent>,
    ) -> Vec<UsageEvent> {
        self.turn_count = self.turn_count.saturating_add(1);
        self.tool_call_count = self
            .tool_call_count
            .saturating_add(tool_call_count.try_into().unwrap_or(u32::MAX));
        let recorded = if usage_events.is_empty() {
            vec![UsageEvent {
                provider_id: None,
                model: self.model.clone(),
                source: None,
                reason: ProviderUsageReason::ProviderOmitted,
                input_tokens: None,
                output_tokens: None,
                cache_read_tokens: None,
                cache_creation_tokens: None,
                reasoning_tokens: None,
                estimated_cost_micros: None,
                pricing_source: None,
            }]
        } else {
            usage_events
                .into_iter()
                .map(|mut event| {
                    if event.model.is_none() {
                        event.model = self.model.clone();
                    }
                    event
                })
                .collect::<Vec<_>>()
        };
        self.events.extend(recorded.clone());
        recorded
    }

    fn ledger(&self) -> LoopUsageLedger {
        LoopUsageLedger::from_events(self.events.clone()).with_runtime_counts(
            self.turn_count,
            self.tool_call_count,
            self.started_at
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        )
    }
}

struct UsageCaptureEmitter {
    model: Option<String>,
    events: parking_lot::Mutex<Vec<UsageEvent>>,
    saw_provider_usage: parking_lot::Mutex<bool>,
}

impl UsageCaptureEmitter {
    fn new(model: &str) -> Self {
        Self {
            model: (!model.trim().is_empty()).then(|| model.to_string()),
            events: parking_lot::Mutex::new(Vec::new()),
            saw_provider_usage: parking_lot::Mutex::new(false),
        }
    }

    fn drain(&self) -> Vec<UsageEvent> {
        *self.saw_provider_usage.lock() = false;
        std::mem::take(&mut *self.events.lock())
    }
}

impl EventEmitter for UsageCaptureEmitter {
    fn emit(&self, event: StreamEvent) {
        match event {
            StreamEvent::ProviderUsage {
                provider_id,
                model,
                source,
                reason,
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_creation_tokens,
                reasoning_tokens,
                estimated_cost_micros,
                pricing_source,
                ..
            } => {
                *self.saw_provider_usage.lock() = true;
                self.events.lock().push(UsageEvent {
                    provider_id,
                    model: model.or_else(|| self.model.clone()),
                    source,
                    reason,
                    input_tokens,
                    output_tokens,
                    cache_read_tokens,
                    cache_creation_tokens,
                    reasoning_tokens,
                    estimated_cost_micros,
                    pricing_source,
                });
            }
            StreamEvent::Usage {
                input_tokens,
                output_tokens,
                estimated_cost_usd,
                ..
            } => {
                if *self.saw_provider_usage.lock() {
                    return;
                }
                self.events.lock().push(UsageEvent {
                    provider_id: None,
                    model: self.model.clone(),
                    source: None,
                    reason: ProviderUsageReason::ProviderReported,
                    input_tokens: Some(input_tokens.into()),
                    output_tokens: Some(output_tokens.into()),
                    cache_read_tokens: None,
                    cache_creation_tokens: None,
                    reasoning_tokens: None,
                    estimated_cost_micros: estimated_cost_micros(estimated_cost_usd),
                    pricing_source: None,
                });
            }
            _ => {}
        }
    }
}

fn estimated_cost_micros(estimated_cost_usd: f64) -> Option<u64> {
    if estimated_cost_usd.is_finite() && estimated_cost_usd >= 0.0 {
        Some((estimated_cost_usd * 1_000_000.0).round() as u64)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubAgentMode {
    Research,
    PatchProposal,
    WorktreeWorker,
}

impl SubAgentMode {
    fn runtime_role(self) -> &'static str {
        match self {
            SubAgentMode::Research => "research",
            SubAgentMode::PatchProposal => "patch_proposal",
            SubAgentMode::WorktreeWorker => "worktree_worker",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SubagentRuntimeContext {
    pub session_id: String,
    pub task_id: String,
    pub loop_task_id: Option<String>,
}

/// A lightweight ephemeral agent for read-only subtasks.
/// Runs in parallel with other sub-agents via tokio::spawn.
/// Returns JSON with full conversation trace for frontend rendering.
pub struct SubAgent;

impl SubAgent {
    pub async fn run(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        app_handle: &tauri::AppHandle,
        cancel: Arc<Notify>,
        working_dir: &std::path::Path,
    ) -> String {
        // Build context: project CLAUDE.md if available, plus working directory
        let project_ctx = crate::harness::read_project_context(working_dir).unwrap_or_default();
        let context_section = if project_ctx.is_empty() {
            format!("Working directory: {}\n", working_dir.display())
        } else {
            format!(
                "Working directory: {}\n\nProject context:\n{}\n",
                working_dir.display(),
                project_ctx
            )
        };

        let system = ChatMessage {
            role: "system".to_string(),
            content: serde_json::Value::String(format!(
                "You are a focused research sub-agent. Your task is to investigate a specific \
                question and return a concise answer.\n\
                You have read-only tools: read_file, search_content, search_files, list_directory, \
                web_search, web_fetch, git_diff.\n\
                Do NOT use write_to_file, edit_file, run_shell, or delegate_task.\n\
                Be thorough but concise. Return your findings as plain text.\n\n\
                {context_section}"
            )),
        };

        let task_msg = ChatMessage::user(task);
        let mut messages: Vec<ChatMessage> = vec![system, task_msg];
        let mut traces: Vec<RoundTrace> = Vec::new();
        let mut usage = SubAgentUsageTracker::new(adapter.model_id());
        let usage_capture = UsageCaptureEmitter::new(adapter.model_id());

        for round in 0..MAX_ROUNDS {
            let stream_result = match adapter
                .call_with_emitter("subagent", &messages, &usage_capture, cancel.clone())
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    crate::app_log!("INFO", "[subagent] API error: {}", e);
                    return build_error_json(&format!("API error: {e}"), &traces, usage.ledger());
                }
            };
            usage.record_model_round(stream_result.tool_calls.len(), usage_capture.drain());

            let thinking = extract_by_type(&stream_result.assistant_content, "thinking");
            let text = extract_by_type(&stream_result.assistant_content, "text");

            if !stream_result.assistant_content.is_empty() {
                messages.push(ChatMessage::assistant(serde_json::Value::Array(
                    stream_result.assistant_content.clone(),
                )));
            }

            if stream_result.tool_calls.is_empty() {
                crate::app_log!("INFO", "[subagent] round {}: done, no tool calls", round);
                traces.push(RoundTrace {
                    round,
                    thinking: truncate_each(thinking, 1000),
                    text: truncate_each(text.clone(), 2000),
                    tool_calls: vec![],
                });
                let result_text = text.clone();
                return build_result_json(&result_text, &traces, usage.ledger());
            }

            crate::app_log!(
                "INFO",
                "[subagent] round {}: {} tool calls — {:?}",
                round,
                stream_result.tool_calls.len(),
                stream_result
                    .tool_calls
                    .iter()
                    .map(|tc| tc.name.clone())
                    .collect::<Vec<_>>()
            );

            // Execute tools in parallel (block dangerous tools)
            // Track by index so each result maps to the correct tool_use_id
            let tool_count = stream_result.tool_calls.len();
            let mut handles = Vec::new();
            for (i, tc) in stream_result.tool_calls.iter().enumerate() {
                let h = harness.clone();
                let name = tc.name.clone();
                let input = tc.input.clone();
                let input_str = serde_json::to_string(&tc.input).unwrap_or_default();
                let tool_id = tc.id.clone();
                let is_blocked = matches!(
                    name.as_str(),
                    "run_shell"
                        | "shell_command"
                        | "run_command"
                        | "run_shell_command"
                        | "write_to_file"
                        | "edit_file"
                        | "bash"
                        | "delegate_task"
                );
                let app = app_handle.clone();
                let cancel_for_tool = cancel.clone();
                handles.push(tokio::spawn(async move {
                    let result = if is_blocked {
                        format!(
                            "Tool '{}' is blocked for sub-agents (read-only access only)",
                            name
                        )
                    } else {
                        h.execute_tool_with_block_id_and_cancel(
                            "sub",
                            &name,
                            &input,
                            &app,
                            Some(&tool_id),
                            Some(cancel_for_tool),
                        )
                        .await
                    };
                    (i, name, input_str, result, tool_id)
                }));
            }

            let mut tool_traces: Vec<ToolCallTrace> = Vec::new();
            let mut tool_results: Vec<serde_json::Value> =
                vec![serde_json::Value::Null; tool_count];

            for handle in handles {
                match handle.await {
                    Ok((i, name, input_str, result, tool_id)) => {
                        crate::app_log!(
                            "INFO",
                            "[subagent] tool '{}' result ({} chars)",
                            name,
                            result.len()
                        );
                        let truncated_input = truncate_each(input_str, 200);
                        let truncated_result = truncate_each(result.clone(), 1500);
                        tool_traces.push(ToolCallTrace {
                            name,
                            input: truncated_input,
                            result: truncated_result,
                        });
                        tool_results[i] = serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_id,
                            "content": result,
                        });
                    }
                    Err(e) => {
                        crate::app_log!("INFO", "[subagent] tool panicked: {}", e);
                    }
                }
            }
            // Remove any Null entries from tool_results (panicked tools)
            tool_results.retain(|v| !v.is_null());

            traces.push(RoundTrace {
                round,
                thinking: truncate_each(thinking.clone(), 1000),
                text: truncate_each(text.clone(), 2000),
                tool_calls: tool_traces,
            });

            messages.push(ChatMessage {
                role: "user".to_string(),
                content: serde_json::Value::Array(tool_results),
            });
        }

        // Max rounds — request final summary
        messages.push(ChatMessage::user(
            "Summarize your findings concisely. Do not use tools.",
        ));
        match adapter
            .call_with_emitter("subagent", &messages, &usage_capture, cancel.clone())
            .await
        {
            Ok(r) => {
                usage.record_model_round(r.tool_calls.len(), usage_capture.drain());
                let text = extract_by_type(&r.assistant_content, "text");
                crate::app_log!("INFO", "[subagent] complete: {} chars", text.len());
                build_result_json(&text, &traces, usage.ledger())
            }
            Err(e) => build_error_json(&format!("Final error: {e}"), &traces, usage.ledger()),
        }
    }

    /// Run a sub-agent using an abstract event emitter.
    pub async fn run_with_emitter(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        emitter: Arc<dyn EventEmitter>,
        cancel: Arc<Notify>,
        working_dir: &std::path::Path,
        runtime_context: Option<SubagentRuntimeContext>,
    ) -> String {
        Self::run_with_mode(
            task,
            adapter,
            harness,
            emitter,
            cancel,
            working_dir,
            SubAgentMode::Research,
            runtime_context,
        )
        .await
    }

    pub async fn run_patch_proposal(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        emitter: Arc<dyn EventEmitter>,
        cancel: Arc<Notify>,
        working_dir: &std::path::Path,
        runtime_context: Option<SubagentRuntimeContext>,
    ) -> String {
        Self::run_with_mode(
            task,
            adapter,
            harness,
            emitter,
            cancel,
            working_dir,
            SubAgentMode::PatchProposal,
            runtime_context,
        )
        .await
    }

    pub async fn run_worktree_worker(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        emitter: Arc<dyn EventEmitter>,
        cancel: Arc<Notify>,
        working_dir: &std::path::Path,
        runtime_context: Option<SubagentRuntimeContext>,
    ) -> String {
        Self::run_with_mode(
            task,
            adapter,
            harness,
            emitter,
            cancel,
            working_dir,
            SubAgentMode::WorktreeWorker,
            runtime_context,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_with_mode(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        emitter: Arc<dyn EventEmitter>,
        cancel: Arc<Notify>,
        working_dir: &std::path::Path,
        mode: SubAgentMode,
        runtime_context: Option<SubagentRuntimeContext>,
    ) -> String {
        if let Some(context) = runtime_context.as_ref() {
            emit_subagent_runtime_event(
                emitter.as_ref(),
                context,
                SubagentRuntimePayload::Started {
                    role: mode.runtime_role().to_string(),
                },
            );
        }

        let project_ctx = crate::harness::read_project_context(working_dir).unwrap_or_default();
        let context_section = if project_ctx.is_empty() {
            format!("Working directory: {}\n", working_dir.display())
        } else {
            format!(
                "Working directory: {}\n\nProject context:\n{}\n",
                working_dir.display(),
                project_ctx
            )
        };

        let system = ChatMessage::system(&build_system_prompt(mode, &context_section));

        let task_msg = ChatMessage::user(task);
        let mut messages: Vec<ChatMessage> = vec![system, task_msg];
        let mut usage = SubAgentUsageTracker::new(adapter.model_id());
        let usage_capture = UsageCaptureEmitter::new(adapter.model_id());

        for _round in 0..MAX_ROUNDS {
            let stream_result = match adapter
                .call_with_emitter("subagent", &messages, &usage_capture, cancel.clone())
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    if let Some(context) = runtime_context.as_ref() {
                        emit_subagent_runtime_event(
                            emitter.as_ref(),
                            context,
                            SubagentRuntimePayload::Failed {
                                reason: format!("API error: {e}"),
                            },
                        );
                    }
                    return build_error_json(&format!("API error: {e}"), &[], usage.ledger());
                }
            };
            let recorded_usage =
                usage.record_model_round(stream_result.tool_calls.len(), usage_capture.drain());
            if let Some(context) = runtime_context.as_ref() {
                emit_subagent_usage_events(emitter.as_ref(), context, &recorded_usage);
            }

            if !stream_result.assistant_content.is_empty() {
                messages.push(ChatMessage::assistant(serde_json::Value::Array(
                    stream_result.assistant_content.clone(),
                )));
            }

            if stream_result.tool_calls.is_empty() {
                let text = extract_by_type(&stream_result.assistant_content, "text");
                if let Some(context) = runtime_context.as_ref() {
                    emit_subagent_runtime_event(
                        emitter.as_ref(),
                        context,
                        SubagentRuntimePayload::Ended {
                            status: "completed".to_string(),
                        },
                    );
                }
                return build_result_json(&text, &[], usage.ledger());
            }

            let mut result_map = std::collections::HashMap::new();
            for tc in &stream_result.tool_calls {
                let is_blocked = match mode {
                    SubAgentMode::Research | SubAgentMode::PatchProposal => matches!(
                        tc.name.as_str(),
                        "run_shell"
                            | "shell_command"
                            | "run_command"
                            | "run_shell_command"
                            | "write_to_file"
                            | "edit_file"
                            | "bash"
                            | "delegate_task"
                    ),
                    SubAgentMode::WorktreeWorker => {
                        matches!(tc.name.as_str(), "delegate_task")
                    }
                };
                let result = if is_blocked {
                    format!("Tool '{}' is blocked for sub-agents", tc.name)
                } else {
                    harness
                        .execute_tool_with_emitter(
                            "sub",
                            &tc.name,
                            &tc.input,
                            emitter.clone(),
                            Some(&tc.id),
                            Some(cancel.clone()),
                        )
                        .await
                };
                if !is_blocked && is_successful_runtime_file_io_result(&tc.name, &result) {
                    if let (Some(context), Some((path, operation))) = (
                        runtime_context.as_ref(),
                        runtime_file_io_fact(&tc.name, &tc.input),
                    ) {
                        emit_subagent_runtime_event(
                            emitter.as_ref(),
                            context,
                            SubagentRuntimePayload::FileIo { path, operation },
                        );
                    }
                }
                result_map.insert(tc.id.clone(), result);
            }

            let model_tool_results =
                crate::agent::tool_results::build_tool_result_message_for_model(
                    &result_map,
                    &stream_result.tool_calls,
                );
            messages.push(model_tool_results.message);
        }

        if let Some(context) = runtime_context.as_ref() {
            emit_subagent_runtime_event(
                emitter.as_ref(),
                context,
                SubagentRuntimePayload::Failed {
                    reason: "Max rounds reached".to_string(),
                },
            );
        }
        build_error_json("Max rounds reached", &[], usage.ledger())
    }
}

fn emit_subagent_runtime_event(
    emitter: &dyn EventEmitter,
    context: &SubagentRuntimeContext,
    event: SubagentRuntimePayload,
) {
    emitter.emit(StreamEvent::SubagentRuntimeEvent {
        session_id: context.session_id.clone(),
        loop_task_id: context.loop_task_id.clone(),
        task_id: context.task_id.clone(),
        event,
    });
}

fn emit_subagent_usage_events(
    emitter: &dyn EventEmitter,
    context: &SubagentRuntimeContext,
    events: &[UsageEvent],
) {
    for event in events {
        emit_subagent_runtime_event(
            emitter,
            context,
            SubagentRuntimePayload::UsageRecorded {
                provider_id: event.provider_id.clone(),
                model: event.model.clone(),
                source: event.source.clone(),
                reason: event.reason.clone(),
                input_tokens: event.input_tokens,
                output_tokens: event.output_tokens,
                cache_read_tokens: event.cache_read_tokens,
                cache_creation_tokens: event.cache_creation_tokens,
                reasoning_tokens: event.reasoning_tokens,
                estimated_cost_micros: event.estimated_cost_micros,
                pricing_source: event.pricing_source.clone(),
            },
        );
    }
}

fn runtime_file_io_fact(tool_name: &str, input: &serde_json::Value) -> Option<(String, String)> {
    let explicit_path = || {
        input
            .get("path")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    match tool_name {
        "read_file" | "read" => explicit_path().map(|path| (path, "read".to_string())),
        "write_file" | "write_to_file" | "write" => {
            explicit_path().map(|path| (path, "write".to_string()))
        }
        "edit_file" | "edit" => explicit_path().map(|path| (path, "edit".to_string())),
        "git_diff" => Some((
            explicit_path().unwrap_or_else(|| "all files".to_string()),
            "diff".to_string(),
        )),
        "list_directory" | "ls" | "list" => Some((
            explicit_path().unwrap_or_else(|| ".".to_string()),
            "list".to_string(),
        )),
        "search_files" | "glob" | "search_content" | "grep" => Some((
            explicit_path().unwrap_or_else(|| ".".to_string()),
            "search".to_string(),
        )),
        _ => None,
    }
}

fn is_successful_runtime_file_io_result(tool_name: &str, result: &str) -> bool {
    let is_file_io_tool = matches!(
        tool_name,
        "read_file"
            | "read"
            | "write_file"
            | "write_to_file"
            | "write"
            | "edit_file"
            | "edit"
            | "git_diff"
            | "list_directory"
            | "ls"
            | "list"
            | "search_files"
            | "glob"
            | "search_content"
            | "grep"
    );
    if !is_file_io_tool || crate::agent::turn_state::is_errorish_tool_result(result) {
        return false;
    }

    let normalized = result.trim_start().to_ascii_lowercase();
    ![
        "git diff failed",
        "search path is not available",
        "search path is not a directory",
    ]
    .iter()
    .any(|prefix| normalized.starts_with(prefix))
}

fn build_system_prompt(mode: SubAgentMode, context_section: &str) -> String {
    match mode {
        SubAgentMode::Research => format!(
            "You are a focused research sub-agent. Your task is to investigate a specific \
            question and return a concise answer.\n\
            You have read-only tools: read_file, search_content, search_files, list_directory, \
            web_search, web_fetch, git_diff.\n\
            Do NOT use write_to_file, edit_file, run_shell, or delegate_task.\n\
            Be thorough but concise. Return your findings as plain text.\n\n\
            {context_section}"
        ),
        SubAgentMode::PatchProposal => format!(
            "You are a code analysis sub-agent. Your task is to analyze code and produce a \
            structured patch proposal WITHOUT modifying any files.\n\
            You have read-only tools: read_file, search_content, search_files, list_directory, \
            web_search, web_fetch, git_diff.\n\
            Do NOT use write_to_file, edit_file, run_shell, or delegate_task.\n\
            You are NOT writing files — you are producing a proposal that will be reviewed.\n\n\
            Workflow:\n\
            1. Read relevant files to understand the codebase and the issue.\n\
            2. Analyze the problem and design a minimal, safe fix.\n\
            3. At the end of your response, output a JSON block (inside ```json) with:\n\n\
            ```json\n\
            {{\n\
              \"result\": \"Textual analysis of the problem and your reasoning\",\n\
              \"patch_proposal\": {{\n\
                \"file_path\": \"path/to/file.ext\",\n\
                \"intent\": \"Clear description of what this change does and why\",\n\
                \"diff_summary\": \"One-line summary of the change\",\n\
                \"original_snippet\": \"The original code that will be changed\",\n\
                \"proposed_snippet\": \"Your proposed replacement code\",\n\
                \"risk_level\": \"low|medium|high\",\n\
                \"test_suggestion\": \"How to test this change\",\n\
                \"confidence\": 0.85\n\
              }}\n\
            }}\n\
            ```\n\n\
            {context_section}"
        ),
        SubAgentMode::WorktreeWorker => format!(
            "You are an implementation sub-agent running in an isolated git worktree.\n\
            You have FULL tool access: read_file, write_to_file, edit_file, run_shell, \
            search_content, search_files, list_directory, web_search, web_fetch, git_diff.\n\
            Do NOT use delegate_task — you are the worker, do not spawn further sub-agents.\n\n\
            Your workspace is ISOLATED from the main branch. You can safely write files and \
            run commands. Your changes will be reviewed as a diff before any merge decision.\n\n\
            Workflow:\n\
            1. Read relevant files to understand the task.\n\
            2. Make the necessary changes (write files, run tests, etc.).\n\
            3. Run tests or verification commands to confirm your changes work.\n\
            4. Report what you changed and the test results concisely.\n\n\
            Return your findings as plain text with a clear summary of:\n\
            - What files you modified\n\
            - What tests you ran and their outcomes\n\
            - Any issues or follow-up needed\n\n\
            {context_section}"
        ),
    }
}

fn extract_by_type(content: &[serde_json::Value], target_type: &str) -> String {
    let result = content
        .iter()
        .filter_map(|block| {
            let t = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if t == target_type {
                block
                    .get(target_type)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    // Fallback: if no text blocks found, try thinking blocks (AI may return thinking-only)
    if !result.is_empty() || target_type != "text" {
        return result;
    }
    extract_by_type(content, "thinking")
}

fn build_result_json(text: &str, traces: &[RoundTrace], usage: LoopUsageLedger) -> String {
    let payload = SubAgentResult {
        result: truncate_any(text.to_string(), MAX_RESULT_CHARS),
        steps: traces.to_vec(),
        usage,
    };
    serde_json::to_string(&payload).unwrap_or_else(|_| text.to_string())
}

fn build_error_json(error: &str, traces: &[RoundTrace], usage: LoopUsageLedger) -> String {
    let payload = SubAgentResult {
        result: error.to_string(),
        steps: traces.to_vec(),
        usage,
    };
    serde_json::to_string(&payload).unwrap_or_else(|_| error.to_string())
}

fn truncate_each(s: String, max: usize) -> String {
    if s.len() <= max {
        s
    } else {
        let mut t = s.chars().take(max).collect::<String>();
        t.push_str("...");
        t
    }
}

fn truncate_any(s: String, max: usize) -> String {
    if s.len() <= max {
        s
    } else {
        let mut t = s.chars().take(max).collect::<String>();
        t.push_str("\n\n... (truncated)");
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_capture_prefers_provider_usage_over_legacy_usage() {
        let emitter = UsageCaptureEmitter::new("claude-sonnet");

        emitter.emit(StreamEvent::ProviderUsage {
            session_id: "subagent".to_string(),
            block_id: uuid::Uuid::now_v7().to_string(),
            provider_id: Some("anthropic".to_string()),
            model: Some("claude-sonnet".to_string()),
            input_tokens: Some(100),
            output_tokens: Some(50),
            cache_read_tokens: None,
            cache_creation_tokens: None,
            reasoning_tokens: None,
            estimated_cost_micros: Some(1050),
            pricing_source: Some(crate::adapters::anthropic::STATIC_PRICING_SOURCE.to_string()),
            source: Some("anthropic".to_string()),
            reason: ProviderUsageReason::ProviderReported,
        });
        emitter.emit(StreamEvent::Usage {
            session_id: "subagent".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            estimated_cost_usd: 0.00105,
        });

        let events = emitter.drain();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].model.as_deref(), Some("claude-sonnet"));
        assert_eq!(events[0].provider_id.as_deref(), Some("anthropic"));
        assert_eq!(events[0].source.as_deref(), Some("anthropic"));
        assert_eq!(events[0].reason, ProviderUsageReason::ProviderReported);
        assert_eq!(events[0].input_tokens, Some(100));
        assert_eq!(events[0].output_tokens, Some(50));
        assert_eq!(events[0].estimated_cost_micros, Some(1050));
    }
}
