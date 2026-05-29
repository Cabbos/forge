use parking_lot::Mutex;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};

use tokio::sync::Notify;

use crate::adapters::base::{AiAdapter, ChatMessage};
use crate::agent::auto_compact::{
    compact_messages_for_overflow_retry, compact_messages_if_needed, AutoCompactGuard,
    CompactResult, CompactStats,
};
use crate::agent::context_builder::{
    ContextBuilder, ContextBundle, ContextSourceKind, HiddenContextPart,
};
use crate::agent::provider_capabilities::is_context_overflow_error;
use crate::agent::recovery::{
    api_failure_trace, build_recovery_context, verification_failure_trace,
};
use crate::agent::retry_policy::should_retry_adapter_error;
use crate::agent::session_events;
pub(crate) use crate::agent::session_guards::TurnInflightGuard;
use crate::agent::session_guards::{
    lock_unpoisoned, sub_agent_join_error_message, try_begin_turn, ActiveCancelGuard,
};
use crate::agent::snapshot::AgentSessionSnapshot;
use crate::agent::time::now_ms;
use crate::agent::tool_results::{
    build_tool_result_message_for_model, is_read_only_tool,
    push_assistant_result_with_synthetic_tool_results, repair_tool_use_adjacency,
};
use crate::agent::turn_outcome::{
    final_answer_instruction, final_turn_status_for_current_turn,
    final_turn_transition_reason_for_current_turn, verification_has_failed,
};
use crate::agent::turn_state::{
    completed_tool_trace, running_tool_trace, AgentCompactTrace, AgentFailureTrace,
    AgentTurnMetadata, AgentTurnState, AgentTurnStatus, AgentVerificationStatus,
    AgentVerificationTrace,
};
use crate::agent::verification;
use crate::consts::{AGENT_LOOP_SETTLE_DELAY, AGENT_OVERFLOW_RETRY_DELAY};
use crate::harness::Harness;
use crate::protocol::events::StreamEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Starting,
    Running,
    Stopped,
    Error(String),
}

impl SessionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            SessionStatus::Starting => "starting",
            SessionStatus::Running => "running",
            SessionStatus::Stopped => "stopped",
            SessionStatus::Error(_) => "error",
        }
    }
}

pub struct AgentSession {
    pub id: String,
    pub agent_type: String,
    pub model: String,
    pub model_id: String,
    pub status: Arc<Mutex<SessionStatus>>,
    pub(crate) adapter: Arc<dyn AiAdapter>,
    pub(crate) messages: Arc<Mutex<Vec<ChatMessage>>>,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) turn_inflight: Arc<AtomicBool>,
    pub(crate) harness: Arc<Harness>,
    pub(crate) system_prompt: Mutex<String>,
    pub(crate) summary: Mutex<Option<String>>,
    pub(crate) latest_turn: Mutex<Option<AgentTurnState>>,
    pub(crate) auto_compact_guard: Mutex<AutoCompactGuard>,
    pub(crate) context_window_tokens: Option<u32>,
    pub(crate) cancel: Mutex<Option<Arc<Notify>>>,
}

// Lock discipline for AgentSession:
// - Prefer taking one mutex per statement and cloning the small value needed.
// - If multiple locks are unavoidable, acquire them in this order:
//   status -> system_prompt -> messages -> summary -> latest_turn -> auto_compact_guard -> cancel.
// This keeps resume/snapshot/turn setup from growing accidental lock-order cycles.

pub(crate) struct AgentPreviewStatusUpdate<'a> {
    pub project_path: Option<&'a str>,
    pub running: bool,
    pub can_start: bool,
    pub can_open: bool,
    pub label: &'a str,
    pub url: Option<&'a str>,
}

impl AgentSession {
    pub fn new(
        id: String,
        agent_type: String,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        system_prompt: String,
        context_window_tokens: Option<u32>,
    ) -> Self {
        let model_id = adapter.model_id().to_string();
        let model = adapter.model_name().to_string();

        let session = AgentSession {
            id: id.clone(),
            agent_type: agent_type.clone(),
            model: model.clone(),
            model_id,
            status: Arc::new(Mutex::new(SessionStatus::Starting)),
            adapter,
            messages: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(AtomicBool::new(true)),
            turn_inflight: Arc::new(AtomicBool::new(false)),
            harness,
            system_prompt: Mutex::new(system_prompt),
            summary: Mutex::new(None),
            latest_turn: Mutex::new(None),
            auto_compact_guard: Mutex::new(AutoCompactGuard::default()),
            context_window_tokens,
            cancel: Mutex::new(None),
        };

        *lock_unpoisoned(&session.status) = SessionStatus::Running;
        session
    }

    pub fn set_system_prompt(&self, prompt: String) {
        *lock_unpoisoned(&self.system_prompt) = prompt;
    }

    pub fn is_waiting_for_api_key(&self) -> bool {
        self.adapter.is_missing_api_key_adapter()
    }

    pub fn restore_state(
        &self,
        messages: Vec<ChatMessage>,
        summary: Option<String>,
        latest_turn: Option<AgentTurnState>,
    ) {
        *lock_unpoisoned(&self.messages) = repair_tool_use_adjacency(messages);
        *lock_unpoisoned(&self.summary) = summary;
        *lock_unpoisoned(&self.latest_turn) = latest_turn.map(|mut turn| {
            turn.normalize_for_session_resume();
            turn
        });
    }

    pub fn snapshot(&self) -> AgentSessionSnapshot {
        let messages = lock_unpoisoned(&self.messages).clone();
        let summary = lock_unpoisoned(&self.summary).clone();
        let snapshot = AgentSessionSnapshot::new(
            self.id.clone(),
            self.agent_type.clone(),
            self.model_id.clone(),
            self.harness.working_dir.to_string_lossy().to_string(),
            messages,
            summary,
            self.context_window_tokens,
        );
        if let Some(latest_turn) = lock_unpoisoned(&self.latest_turn).clone() {
            snapshot.with_latest_turn(latest_turn)
        } else {
            snapshot
        }
    }

    /// Send a user message and run the agent loop through the harness.
    pub async fn send_message(
        &self,
        text: &str,
        app_handle: &tauri::AppHandle,
    ) -> Result<(), String> {
        self.send_message_with_context(text, app_handle, None, None)
            .await
    }

    /// Send a user message with optional hidden memory context for this turn.
    pub async fn send_message_with_context(
        &self,
        text: &str,
        app_handle: &tauri::AppHandle,
        memory_context: Option<String>,
        turn_metadata: Option<AgentTurnMetadata>,
    ) -> Result<(), String> {
        let hidden_contexts = memory_context
            .filter(|context| !context.trim().is_empty())
            .map(|context| {
                vec![HiddenContextPart::new(
                    ContextSourceKind::MemoryContext,
                    "已保存背景",
                    "本轮自动带入的用户和项目背景",
                    context,
                )]
            })
            .unwrap_or_default();

        self.send_message_with_context_parts(text, app_handle, hidden_contexts, turn_metadata)
            .await
    }

    /// Send a user message with structured hidden context parts for this turn.
    pub async fn send_message_with_context_parts(
        &self,
        text: &str,
        app_handle: &tauri::AppHandle,
        hidden_contexts: Vec<HiddenContextPart>,
        turn_metadata: Option<AgentTurnMetadata>,
    ) -> Result<(), String> {
        self.send_message_with_context_parts_and_activation_text(
            text,
            app_handle,
            hidden_contexts,
            turn_metadata,
            None,
        )
        .await
    }

    /// Send a user message while allowing hidden composer intent to influence skill activation.
    pub async fn send_message_with_context_parts_and_activation_text(
        &self,
        text: &str,
        app_handle: &tauri::AppHandle,
        hidden_contexts: Vec<HiddenContextPart>,
        turn_metadata: Option<AgentTurnMetadata>,
        activation_text: Option<&str>,
    ) -> Result<(), String> {
        let turn_guard = self.reserve_turn()?;
        self.send_message_with_reserved_turn(
            text,
            app_handle,
            hidden_contexts,
            turn_metadata,
            activation_text,
            turn_guard,
        )
        .await
    }

    pub(crate) fn reserve_turn(&self) -> Result<TurnInflightGuard, String> {
        if !self.running.load(Ordering::SeqCst) {
            return Err("Session is not running".to_string());
        }
        try_begin_turn(self.turn_inflight.clone())
    }

    /// Continue a send after the IPC layer has reserved the turn.
    pub(crate) async fn send_message_with_reserved_turn(
        &self,
        text: &str,
        app_handle: &tauri::AppHandle,
        hidden_contexts: Vec<HiddenContextPart>,
        turn_metadata: Option<AgentTurnMetadata>,
        activation_text: Option<&str>,
        _turn_guard: TurnInflightGuard,
    ) -> Result<(), String> {
        if !self.running.load(Ordering::SeqCst) {
            return Err("Session is not running".to_string());
        }
        let previous_turn = lock_unpoisoned(&self.latest_turn).clone();
        let mut hidden_contexts = hidden_contexts;
        if let Some(context) = build_recovery_context(previous_turn.as_ref(), text) {
            hidden_contexts.push(HiddenContextPart::new(
                ContextSourceKind::RecoveryTrace,
                "恢复线索",
                "上一轮失败后用于继续处理的内部线索",
                context,
            ));
        }
        self.start_turn(text, turn_metadata, app_handle);
        crate::app_log!(
            "INFO",
            "Agent received user message, history size: {}",
            lock_unpoisoned(&self.messages).len()
        );
        let turn_system_prompt = self
            .harness
            .build_system_prompt_for_request(
                &self.agent_type,
                &self.harness.working_dir,
                Some(activation_text.unwrap_or(text)),
            )
            .await;
        *lock_unpoisoned(&self.system_prompt) = turn_system_prompt;
        self.adapter
            .set_external_tools(self.harness.external_mcp_tool_definitions().await);

        // Add user message to history
        lock_unpoisoned(&self.messages).push(ChatMessage::user(text));
        self.repair_message_history("before_model_call");
        let hidden_contexts = hidden_contexts
            .into_iter()
            .filter(|context| !context.content.trim().is_empty())
            .collect::<Vec<_>>();
        self.mark_latest_turn_status_with_reason(
            AgentTurnStatus::GatheringContext,
            "gather_context",
            None,
            app_handle,
        );

        // Fresh cancel token for this request
        let cancel = Arc::new(Notify::new());
        *lock_unpoisoned(&self.cancel) = Some(cancel.clone());
        let _cancel_guard = ActiveCancelGuard::new(&self.cancel, cancel.clone());

        let mut overflow_retry_used = false;

        // Agent loop: up to 10 tool-call round-trips with final text summary fallback
        for _round in 0..10 {
            if !self.running.load(Ordering::SeqCst) {
                break;
            }

            let all_messages = lock_unpoisoned(&self.messages).clone();
            let existing_summary = lock_unpoisoned(&self.summary).clone();
            let skip_proactive_compaction = {
                let mut guard = lock_unpoisoned(&self.auto_compact_guard);
                if guard.should_skip_proactive_compaction() {
                    guard.record_proactive_skip();
                    true
                } else {
                    false
                }
            };
            let compacted = if skip_proactive_compaction {
                CompactResult::unchanged(all_messages, existing_summary)
            } else {
                compact_messages_if_needed(
                    all_messages,
                    existing_summary,
                    self.context_window_tokens,
                )
            };
            lock_unpoisoned(&self.auto_compact_guard).record_result(&compacted);

            if let Some(stats) = compacted.stats.as_ref() {
                self.apply_compaction(&compacted, stats, "auto_compact", app_handle);
            }

            let sp = lock_unpoisoned(&self.system_prompt).clone();
            crate::app_log!(
                "INFO",
                "[send_message] system_prompt length: {} chars, has 'Active Skills': {}",
                sp.len(),
                sp.contains("Active Skills")
            );
            let context_bundle = build_context_bundle(
                compacted.messages,
                compacted.summary,
                hidden_contexts.clone(),
                sp.clone(),
                self.context_window_tokens,
            );
            self.record_latest_context(&context_bundle, app_handle);
            let mut msgs_with_context = repair_tool_use_adjacency(context_bundle.messages);

            self.mark_latest_turn_status_with_reason(
                AgentTurnStatus::CallingModel,
                "call_model",
                None,
                app_handle,
            );
            let mut retries = 0;
            let result = loop {
                match self
                    .adapter
                    .stream_message(&self.id, &msgs_with_context, app_handle, cancel.clone())
                    .await
                {
                    Ok(r) => break r,
                    Err(e) => {
                        let msg = e.to_string();
                        if !overflow_retry_used && is_context_overflow_error(&self.agent_type, &msg)
                        {
                            let all_messages = lock_unpoisoned(&self.messages).clone();
                            let existing_summary = lock_unpoisoned(&self.summary).clone();
                            let compacted =
                                compact_messages_for_overflow_retry(all_messages, existing_summary);
                            lock_unpoisoned(&self.auto_compact_guard).record_result(&compacted);

                            if let Some(stats) = compacted.stats.as_ref() {
                                overflow_retry_used = true;
                                self.apply_compaction(
                                    &compacted,
                                    stats,
                                    "overflow_retry",
                                    app_handle,
                                );

                                let context_bundle = build_context_bundle(
                                    compacted.messages,
                                    compacted.summary,
                                    hidden_contexts.clone(),
                                    sp.clone(),
                                    self.context_window_tokens,
                                );
                                self.record_latest_context(&context_bundle, app_handle);
                                msgs_with_context =
                                    repair_tool_use_adjacency(context_bundle.messages);
                                continue;
                            }
                        }

                        if should_retry_adapter_error(&e, retries) {
                            retries += 1;
                            tokio::time::sleep(AGENT_OVERFLOW_RETRY_DELAY).await;
                            continue;
                        }
                        let err_msg = format!("API error: {}", msg);
                        // Don't stop the session — let user retry
                        crate::transcript::emit_stream_event(
                            app_handle,
                            self.api_error_event(err_msg.clone()),
                        );
                        if self.running.load(Ordering::SeqCst) {
                            self.record_latest_turn_failure(
                                api_failure_trace(&err_msg),
                                app_handle,
                            );
                        } else {
                            self.mark_latest_turn_status_with_reason(
                                AgentTurnStatus::Cancelled,
                                "user_cancelled",
                                Some("cancelled while handling api error"),
                                app_handle,
                            );
                        }
                        return Err(err_msg);
                    }
                }
            };

            if !self.running.load(Ordering::SeqCst) {
                self.mark_latest_turn_status_with_reason(
                    AgentTurnStatus::Cancelled,
                    "user_cancelled",
                    Some("cancelled after model call"),
                    app_handle,
                );
                break;
            }

            // Save assistant response (DeepSeek Anthropic API requires thinking blocks in history)
            if !result.assistant_content.is_empty() {
                lock_unpoisoned(&self.messages).push(ChatMessage::assistant(
                    serde_json::Value::Array(result.assistant_content.clone()),
                ));
            }

            // No tool calls = done
            if result.tool_calls.is_empty() {
                crate::app_log!("INFO", "Agent turn {}: no tool calls, done", _round);
                self.mark_latest_turn_status_with_reason(
                    AgentTurnStatus::Completed,
                    "final_answer",
                    Some("model returned no tool calls"),
                    app_handle,
                );
                break;
            }

            self.mark_latest_turn_status_with_reason(
                AgentTurnStatus::RunningTools,
                "tool_calls_requested",
                Some("model requested tool execution"),
                app_handle,
            );

            crate::app_log!(
                "INFO",
                "Agent turn {}: {} tool calls to execute: {:?}",
                _round,
                result.tool_calls.len(),
                result
                    .tool_calls
                    .iter()
                    .map(|tc| tc.name.clone())
                    .collect::<Vec<_>>()
            );

            // Separate delegate_task calls from regular tool calls
            let (delegated, regular): (Vec<_>, Vec<_>) = result
                .tool_calls
                .iter()
                .partition(|tc| tc.name == "delegate_task");

            // Run delegated tasks as sub-agents in parallel
            let mut sub_results: Vec<(usize, String)> = Vec::new();
            if !delegated.is_empty() {
                let mut handles = Vec::new();
                for tc in &delegated {
                    let started_at_ms = now_ms();
                    self.record_latest_tool(
                        running_tool_trace(
                            tc.id.clone(),
                            tc.name.clone(),
                            &tc.input,
                            started_at_ms,
                        ),
                        app_handle,
                    );
                    let task = tc
                        .input
                        .get("task")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Investigate and report findings")
                        .to_string();
                    let adapter = self.adapter.clone();
                    let harness = self.harness.clone();
                    let app = app_handle.clone();
                    let cancel = lock_unpoisoned(&self.cancel)
                        .clone()
                        .unwrap_or_else(|| Arc::new(Notify::new()));
                    let idx = result
                        .tool_calls
                        .iter()
                        .position(|t| t.id == tc.id)
                        .unwrap_or(0);
                    let wd = self.harness.working_dir.clone();
                    handles.push((
                        idx,
                        tc.id.clone(),
                        tc.name.clone(),
                        tc.input.clone(),
                        started_at_ms,
                        tokio::spawn(async move {
                            let r = crate::agent::sub::SubAgent::run(
                                &task, adapter, harness, &app, cancel, &wd,
                            )
                            .await;
                            (idx, r)
                        }),
                    ));
                }
                for (fallback_idx, id, name, input, started_at_ms, handle) in handles {
                    match handle.await {
                        Ok((idx, r)) => {
                            // r is JSON: {"result": "...", "steps": [...]}
                            // Extract just the result text for the main agent's context
                            let api_text: String = serde_json::from_str::<serde_json::Value>(&r)
                                .ok()
                                .and_then(|v| {
                                    v.get("result")
                                        .and_then(|r| r.as_str())
                                        .map(|s| s.to_string())
                                })
                                .unwrap_or_else(|| r.clone());
                            crate::app_log!(
                                "INFO",
                                "Agent sub-agent result ({} chars, api_text {} chars)",
                                r.len(),
                                api_text.len()
                            );
                            // Emit full JSON to frontend for SubAgentTrace rendering
                            crate::transcript::emit_stream_event(
                                app_handle,
                                self.tool_call_result_event(&id, &r, false, 0),
                            );
                            self.record_latest_tool(
                                completed_tool_trace(
                                    id.clone(),
                                    name.clone(),
                                    &input,
                                    &r,
                                    started_at_ms,
                                    now_ms(),
                                ),
                                app_handle,
                            );
                            // Feed only the result text to the main agent
                            sub_results.push((idx, api_text));
                        }
                        Err(err) => {
                            let message = sub_agent_join_error_message(&err);
                            crate::app_log!(
                                "ERROR",
                                "Agent sub-agent join failed block={} error={}",
                                id,
                                err
                            );
                            crate::transcript::emit_stream_event(
                                app_handle,
                                self.tool_call_result_event(&id, &message, true, 0),
                            );
                            self.record_latest_tool(
                                completed_tool_trace(
                                    id,
                                    name,
                                    &input,
                                    &message,
                                    started_at_ms,
                                    now_ms(),
                                ),
                                app_handle,
                            );
                            sub_results.push((fallback_idx, message));
                        }
                    }
                }
            }

            // Execute regular tools through the harness
            let (reads, writes): (
                Vec<&crate::adapters::base::ToolCall>,
                Vec<&crate::adapters::base::ToolCall>,
            ) = regular.iter().partition(|tc| is_read_only_tool(&tc.name));

            let mut read_results: Vec<(String, String)> = Vec::new();
            {
                let mut handles = Vec::new();
                for tc in &reads {
                    let h = self.harness.clone();
                    let sid = self.id.clone();
                    let name = tc.name.clone();
                    let input = tc.input.clone();
                    let app = app_handle.clone();
                    let id = tc.id.clone();
                    let started_at_ms = now_ms();
                    let cancel_for_tool = cancel.clone();
                    self.record_latest_tool(
                        running_tool_trace(id.clone(), name.clone(), &input, started_at_ms),
                        app_handle,
                    );
                    handles.push(tokio::spawn(async move {
                        let result = h
                            .execute_tool_with_block_id_and_cancel(
                                &sid,
                                &name,
                                &input,
                                &app,
                                Some(&id),
                                Some(cancel_for_tool),
                            )
                            .await;
                        (id, name, input, started_at_ms, now_ms(), result)
                    }));
                }
                for handle in handles {
                    if let Ok((id, name, input, started_at_ms, ended_at_ms, result)) = handle.await
                    {
                        self.record_latest_tool(
                            completed_tool_trace(
                                id.clone(),
                                name,
                                &input,
                                &result,
                                started_at_ms,
                                ended_at_ms,
                            ),
                            app_handle,
                        );
                        read_results.push((id, result));
                    }
                }
            }

            let mut write_results: Vec<(String, String)> = Vec::new();
            for tc in &writes {
                let started_at_ms = now_ms();
                self.record_latest_tool(
                    running_tool_trace(tc.id.clone(), tc.name.clone(), &tc.input, started_at_ms),
                    app_handle,
                );
                let result = self
                    .harness
                    .execute_tool_with_block_id_and_cancel(
                        &self.id,
                        &tc.name,
                        &tc.input,
                        app_handle,
                        Some(&tc.id),
                        Some(cancel.clone()),
                    )
                    .await;
                self.record_latest_tool(
                    completed_tool_trace(
                        tc.id.clone(),
                        tc.name.clone(),
                        &tc.input,
                        &result,
                        started_at_ms,
                        now_ms(),
                    ),
                    app_handle,
                );
                write_results.push((tc.id.clone(), result));
            }

            // Build results map: tool_call_id → result string
            let mut result_map: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for (id, r) in read_results {
                result_map.insert(id, r);
            }
            for (id, r) in write_results {
                result_map.insert(id, r);
            }
            for (idx, r) in sub_results {
                if let Some(tc) = result.tool_calls.get(idx) {
                    result_map.insert(tc.id.clone(), r);
                }
            }

            // Feed results back in original order, grouped into one user message
            let model_tool_results =
                build_tool_result_message_for_model(&result_map, &result.tool_calls);
            for resolved in &model_tool_results.results {
                if resolved.missing {
                    let Some(tc) = result
                        .tool_calls
                        .iter()
                        .find(|tc| tc.id == resolved.tool_call_id)
                    else {
                        continue;
                    };
                    self.record_latest_tool(
                        completed_tool_trace(
                            tc.id.clone(),
                            tc.name.clone(),
                            &tc.input,
                            &resolved.content,
                            now_ms(),
                            now_ms(),
                        ),
                        app_handle,
                    );
                }
                crate::app_log!(
                    "INFO",
                    "Agent tool '{}' result ({} chars)",
                    resolved.tool_name,
                    resolved.content.len()
                );
            }
            lock_unpoisoned(&self.messages).push(model_tool_results.message);

            // Yield briefly so frontend receives & renders events before next API call
            tokio::time::sleep(AGENT_LOOP_SETTLE_DELAY).await;
        }

        let verification_trace = if self.running.load(Ordering::SeqCst) {
            self.verify_latest_turn(app_handle).await
        } else {
            None
        };

        // Ensure final text response: append instruction, call API one more time if needed
        if self.running.load(Ordering::SeqCst) {
            let messages = lock_unpoisoned(&self.messages).clone();
            let summary = lock_unpoisoned(&self.summary).clone();
            let sp = lock_unpoisoned(&self.system_prompt).clone();
            let context_bundle = build_context_bundle(
                messages,
                summary,
                hidden_contexts.clone(),
                sp,
                self.context_window_tokens,
            );
            self.record_latest_context(&context_bundle, app_handle);
            let mut msgs = repair_tool_use_adjacency(context_bundle.messages);
            let last_role = msgs.last().map(|m| m.role.clone()).unwrap_or_default();
            if last_role == "tool" || last_role == "user" {
                msgs.push(ChatMessage::user(&final_answer_instruction(
                    verification_trace.as_ref(),
                )));
                crate::app_log!("INFO", "Agent loop complete — requesting text-only summary");
                if let Ok(result) = self
                    .adapter
                    .stream_message(&self.id, &msgs, app_handle, cancel.clone())
                    .await
                {
                    if !result.assistant_content.is_empty() {
                        let mut messages = lock_unpoisoned(&self.messages);
                        push_assistant_result_with_synthetic_tool_results(
                            &mut messages,
                            result.assistant_content,
                            &result.tool_calls,
                            "final_summary_tool_call_not_executed",
                        );
                    }
                }
            }
        }

        crate::app_log!("INFO", "Agent loop complete");
        let current_turn_status = lock_unpoisoned(&self.latest_turn)
            .as_ref()
            .map(|turn| turn.status.clone())
            .unwrap_or(AgentTurnStatus::Started);
        let final_reason = final_turn_transition_reason_for_current_turn(
            current_turn_status.clone(),
            self.running.load(Ordering::SeqCst),
            verification_trace.as_ref(),
        );
        self.mark_latest_turn_status_with_reason(
            final_turn_status_for_current_turn(
                current_turn_status,
                self.running.load(Ordering::SeqCst),
                verification_trace.as_ref(),
            ),
            final_reason,
            None,
            app_handle,
        );
        Ok(())
    }

    pub fn kill(&self, app_handle: &tauri::AppHandle) {
        self.running.store(false, Ordering::SeqCst);
        *lock_unpoisoned(&self.status) = SessionStatus::Stopped;
        self.mark_latest_turn_status_with_reason(
            AgentTurnStatus::Cancelled,
            "user_cancelled",
            Some("session killed"),
            app_handle,
        );
        // Cancel in-flight HTTP stream
        if let Some(cancel) = lock_unpoisoned(&self.cancel).take() {
            cancel.notify_waiters();
        }
        crate::transcript::emit_stream_event(app_handle, self.session_stopped_event("killed"));
    }

    pub fn resume(&self, app_handle: &tauri::AppHandle) {
        self.running.store(true, Ordering::SeqCst);
        *lock_unpoisoned(&self.status) = SessionStatus::Running;
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.normalize_for_session_resume();
        }
        self.emit_latest_turn_projection(app_handle);
    }

    /// Emitter-path counterpart to `kill` — identical behavior but uses an
    /// `EventEmitter` instead of `tauri::AppHandle`, making it testable
    /// without a running Tauri window.
    ///
    /// Does NOT lock `latest_turn` — the agent loop itself will mark the turn
    /// as Cancelled when it sees `running == false` after the adapter returns.
    /// This avoids deadlock on tokio's current_thread runtime where the spawned
    /// agent task and the caller share one OS thread and `parking_lot::Mutex`
    /// is non-reentrant.
    pub fn kill_with_emitter(&self, emitter: &dyn crate::agent::event_sink::EventEmitter) {
        self.running.store(false, Ordering::SeqCst);
        *lock_unpoisoned(&self.status) = SessionStatus::Stopped;
        // Fire cancel token — wakes the adapter if it's waiting on `cancel.notified()`.
        // Use notify_one() which stores a permit if no waiter yet.
        if let Some(cancel) = lock_unpoisoned(&self.cancel).as_ref() {
            cancel.notify_one();
        }
        emitter.emit(self.session_stopped_event("killed"));
    }

    fn start_turn(
        &self,
        text: &str,
        metadata: Option<AgentTurnMetadata>,
        app_handle: &tauri::AppHandle,
    ) {
        let metadata = metadata.unwrap_or_else(|| {
            AgentTurnMetadata::default_for_session(
                self.id.clone(),
                self.harness.working_dir.to_string_lossy().to_string(),
                self.agent_type.clone(),
                self.model_id.clone(),
                text.to_string(),
            )
        });
        let mut turn = metadata.into_turn_state(uuid::Uuid::now_v7().to_string());
        turn.set_execution_plan(
            "处理本轮请求".to_string(),
            vec![
                "理解请求与上下文".to_string(),
                "执行必要操作".to_string(),
                "验证并交付结果".to_string(),
            ],
        );
        *lock_unpoisoned(&self.latest_turn) = Some(turn);
        self.emit_latest_turn_projection(app_handle);
    }

    fn repair_message_history(&self, reason: &str) {
        let mut messages = lock_unpoisoned(&self.messages);
        let before_len = messages.len();
        let repaired = repair_tool_use_adjacency(std::mem::take(&mut *messages));
        let after_len = repaired.len();
        if after_len != before_len {
            crate::app_log!(
                "WARN",
                "[agent_history] repaired dangling tool_use history before model call: reason={}, before={}, after={}",
                reason,
                before_len,
                after_len
            );
        }
        *messages = repaired;
    }

    fn mark_latest_turn_status(&self, status: AgentTurnStatus, app_handle: &tauri::AppHandle) {
        self.mark_latest_turn_status_with_reason(status, "status_update", None, app_handle);
    }

    fn mark_latest_turn_status_with_reason(
        &self,
        status: AgentTurnStatus,
        reason: &str,
        detail: Option<&str>,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.mark_status_with_reason(status, reason, detail);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    fn record_latest_turn_failure(&self, trace: AgentFailureTrace, app_handle: &tauri::AppHandle) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.record_failure(trace);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    fn record_latest_tool(
        &self,
        trace: crate::agent::turn_state::AgentToolTrace,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.record_tool(trace);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    fn record_latest_compact(&self, trace: AgentCompactTrace, app_handle: &tauri::AppHandle) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.record_compact(trace);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    fn record_latest_verification(
        &self,
        trace: AgentVerificationTrace,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.set_verification(trace);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    pub fn record_latest_delivery_summary(
        &self,
        summary: &crate::protocol::events::DeliverySummary,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.record_delivery_summary(summary);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    pub fn record_latest_preview_status(
        &self,
        update: AgentPreviewStatusUpdate<'_>,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.record_preview_status(
                update.project_path,
                update.running,
                update.can_start,
                update.can_open,
                update.label,
                update.url,
            );
        }
        self.emit_latest_turn_projection(app_handle);
    }

    pub fn record_latest_checkpoint_status(
        &self,
        is_git_repo: bool,
        dirty: bool,
        has_checkpoint: bool,
        label: &str,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.record_checkpoint_status(is_git_repo, dirty, has_checkpoint, label);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    async fn verify_latest_turn(
        &self,
        app_handle: &tauri::AppHandle,
    ) -> Option<AgentVerificationTrace> {
        let turn = lock_unpoisoned(&self.latest_turn).clone()?;

        if !verification::needs_verification(&turn) {
            let trace = AgentVerificationTrace::default();
            self.record_latest_verification(trace.clone(), app_handle);
            return Some(trace);
        }

        if let Some(trace) = verification::already_verified_after_last_mutation(&turn) {
            self.record_latest_verification(trace.clone(), app_handle);
            return Some(trace);
        }

        let Some(plan) = verification::select_verification_plan(&self.harness.working_dir, &turn)
        else {
            let trace = AgentVerificationTrace {
                status: AgentVerificationStatus::Error,
                command: None,
                exit_code: None,
                stdout_preview: None,
                stderr_preview: Some("no safe verification command found".to_string()),
                duration_ms: Some(0),
                completed_at_ms: Some(now_ms()),
            };
            self.record_latest_verification(trace.clone(), app_handle);
            self.record_latest_turn_failure(verification_failure_trace(&trace), app_handle);
            return Some(trace);
        };

        self.mark_latest_turn_status_with_reason(
            AgentTurnStatus::Verifying,
            "verification_started",
            None,
            app_handle,
        );
        self.record_latest_verification(
            AgentVerificationTrace {
                status: AgentVerificationStatus::Running,
                command: Some(plan.display_command.clone()),
                exit_code: None,
                stdout_preview: None,
                stderr_preview: None,
                duration_ms: None,
                completed_at_ms: None,
            },
            app_handle,
        );
        let cancel = lock_unpoisoned(&self.cancel).clone();
        let trace = verification::run_verification_with_cancel(plan, cancel).await;
        self.record_latest_verification(trace.clone(), app_handle);
        if verification_has_failed(&trace) {
            self.record_latest_turn_failure(verification_failure_trace(&trace), app_handle);
        }
        Some(trace)
    }

    fn apply_compaction(
        &self,
        compacted: &CompactResult,
        stats: &CompactStats,
        reason: &str,
        app_handle: &tauri::AppHandle,
    ) {
        *lock_unpoisoned(&self.summary) = compacted.summary.clone();
        *lock_unpoisoned(&self.messages) = compacted.messages.clone();
        crate::transcript::emit_stream_event(app_handle, self.context_compacted_event(stats));
        self.record_latest_compact(
            AgentCompactTrace {
                reason: reason.to_string(),
                retained_messages: stats.retained_messages,
                compacted_messages: stats.compacted_messages,
                estimated_tokens_before: Some(stats.estimated_tokens_before),
                estimated_tokens_after: Some(stats.estimated_tokens_after),
                created_at_ms: now_ms(),
            },
            app_handle,
        );
    }

    fn record_latest_context(&self, bundle: &ContextBundle, app_handle: &tauri::AppHandle) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.set_context(bundle.to_turn_context_snapshot());
        }
        self.emit_latest_turn_projection(app_handle);
    }

    pub fn emit_latest_turn_projection(&self, app_handle: &tauri::AppHandle) {
        if let Some(event) = self.latest_turn_updated_event() {
            crate::transcript::emit_stream_event(app_handle, event);
        }
    }

    pub(crate) fn latest_turn_updated_event(&self) -> Option<StreamEvent> {
        let latest_turn = lock_unpoisoned(&self.latest_turn);
        session_events::agent_turn_updated_event(&self.id, latest_turn.as_ref())
    }

    pub(crate) fn api_error_event(&self, message: String) -> StreamEvent {
        session_events::api_error_event(&self.id, &message)
    }

    pub(crate) fn session_stopped_event(&self, reason: &str) -> StreamEvent {
        session_events::session_stopped_event(&self.id, reason)
    }

    pub(crate) fn context_compacted_event(&self, stats: &CompactStats) -> StreamEvent {
        session_events::context_compacted_event(&self.id, stats)
    }

    pub(crate) fn tool_call_result_event(
        &self,
        block_id: &str,
        result: &str,
        is_error: bool,
        duration_ms: u64,
    ) -> StreamEvent {
        session_events::tool_call_result_event(&self.id, block_id, result, is_error, duration_ms)
    }

    // ── Emitter-based helpers ──────────────────────────────────────

    fn emit_with_emitter(&self, emitter: &dyn crate::agent::event_sink::EventEmitter) {
        if let Some(event) = self.latest_turn_updated_event() {
            emitter.emit(event);
        }
    }

    fn start_turn_with_emitter(
        &self,
        text: &str,
        metadata: Option<AgentTurnMetadata>,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
    ) {
        let metadata = metadata.unwrap_or_else(|| {
            AgentTurnMetadata::default_for_session(
                self.id.clone(),
                self.harness.working_dir.to_string_lossy().to_string(),
                self.agent_type.clone(),
                self.model_id.clone(),
                text.to_string(),
            )
        });
        let mut turn = metadata.into_turn_state(uuid::Uuid::now_v7().to_string());
        turn.set_execution_plan(
            "处理本轮请求".to_string(),
            vec![
                "理解请求与上下文".to_string(),
                "执行必要操作".to_string(),
                "验证并交付结果".to_string(),
            ],
        );
        *lock_unpoisoned(&self.latest_turn) = Some(turn);
        self.emit_with_emitter(emitter);
    }

    fn mark_latest_turn_status_with_reason_emitter(
        &self,
        status: AgentTurnStatus,
        reason: &str,
        detail: Option<&str>,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.mark_status_with_reason(status, reason, detail);
        }
        self.emit_with_emitter(emitter);
    }

    fn record_latest_turn_failure_emitter(
        &self,
        trace: AgentFailureTrace,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.record_failure(trace);
        }
        self.emit_with_emitter(emitter);
    }

    fn record_latest_tool_emitter(
        &self,
        trace: crate::agent::turn_state::AgentToolTrace,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.record_tool(trace);
        }
        self.emit_with_emitter(emitter);
    }

    fn apply_compaction_emitter(
        &self,
        compacted: &CompactResult,
        stats: &CompactStats,
        reason: &str,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
    ) {
        *lock_unpoisoned(&self.summary) = compacted.summary.clone();
        *lock_unpoisoned(&self.messages) = compacted.messages.clone();
        emitter.emit(self.context_compacted_event(stats));
        self.record_latest_compact_emitter(
            AgentCompactTrace {
                reason: reason.to_string(),
                retained_messages: stats.retained_messages,
                compacted_messages: stats.compacted_messages,
                estimated_tokens_before: Some(stats.estimated_tokens_before),
                estimated_tokens_after: Some(stats.estimated_tokens_after),
                created_at_ms: now_ms(),
            },
            emitter,
        );
    }

    fn record_latest_compact_emitter(
        &self,
        trace: AgentCompactTrace,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.record_compact(trace);
        }
        self.emit_with_emitter(emitter);
    }

    fn record_latest_context_emitter(
        &self,
        bundle: &ContextBundle,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.set_context(bundle.to_turn_context_snapshot());
        }
        self.emit_with_emitter(emitter);
    }

    async fn verify_latest_turn_emitter(
        &self,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
    ) -> Option<AgentVerificationTrace> {
        let turn = lock_unpoisoned(&self.latest_turn).clone()?;

        if !verification::needs_verification(&turn) {
            let trace = AgentVerificationTrace::default();
            self.record_latest_verification_emitter(trace.clone(), emitter);
            return Some(trace);
        }

        if let Some(trace) = verification::already_verified_after_last_mutation(&turn) {
            self.record_latest_verification_emitter(trace.clone(), emitter);
            return Some(trace);
        }

        let Some(plan) = verification::select_verification_plan(&self.harness.working_dir, &turn)
        else {
            let trace = AgentVerificationTrace {
                status: AgentVerificationStatus::Error,
                command: None,
                exit_code: None,
                stdout_preview: None,
                stderr_preview: Some("no safe verification command found".to_string()),
                duration_ms: Some(0),
                completed_at_ms: Some(now_ms()),
            };
            self.record_latest_verification_emitter(trace.clone(), emitter);
            self.record_latest_turn_failure_emitter(verification_failure_trace(&trace), emitter);
            return Some(trace);
        };

        self.mark_latest_turn_status_with_reason_emitter(
            AgentTurnStatus::Verifying,
            "verification_started",
            None,
            emitter,
        );
        self.record_latest_verification_emitter(
            AgentVerificationTrace {
                status: AgentVerificationStatus::Running,
                command: Some(plan.display_command.clone()),
                exit_code: None,
                stdout_preview: None,
                stderr_preview: None,
                duration_ms: None,
                completed_at_ms: None,
            },
            emitter,
        );
        let cancel = lock_unpoisoned(&self.cancel).clone();
        let trace = verification::run_verification_with_cancel(plan, cancel).await;
        self.record_latest_verification_emitter(trace.clone(), emitter);
        if verification_has_failed(&trace) {
            self.record_latest_turn_failure_emitter(verification_failure_trace(&trace), emitter);
        }
        Some(trace)
    }

    fn record_latest_verification_emitter(
        &self,
        trace: AgentVerificationTrace,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.set_verification(trace);
        }
        self.emit_with_emitter(emitter);
    }

    /// Core agent turn loop using an abstract event emitter.
    /// This is the testable counterpart to `send_message_with_reserved_turn`.
    pub(crate) async fn send_message_with_emitter(
        &self,
        text: &str,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
        hidden_contexts: Vec<HiddenContextPart>,
        turn_metadata: Option<AgentTurnMetadata>,
        activation_text: Option<&str>,
        _turn_guard: TurnInflightGuard,
    ) -> Result<(), String> {
        if !self.running.load(Ordering::SeqCst) {
            return Err("Session is not running".to_string());
        }
        let previous_turn = lock_unpoisoned(&self.latest_turn).clone();
        let mut hidden_contexts = hidden_contexts;
        if let Some(context) = build_recovery_context(previous_turn.as_ref(), text) {
            hidden_contexts.push(HiddenContextPart::new(
                ContextSourceKind::RecoveryTrace,
                "恢复线索",
                "上一轮失败后用于继续处理的内部线索",
                context,
            ));
        }
        self.start_turn_with_emitter(text, turn_metadata, emitter);
        crate::app_log!(
            "INFO",
            "Agent received user message, history size: {}",
            lock_unpoisoned(&self.messages).len()
        );
        let turn_system_prompt = self
            .harness
            .build_system_prompt_for_request(
                &self.agent_type,
                &self.harness.working_dir,
                Some(activation_text.unwrap_or(text)),
            )
            .await;
        *lock_unpoisoned(&self.system_prompt) = turn_system_prompt;
        self.adapter
            .set_external_tools(self.harness.external_mcp_tool_definitions().await);

        lock_unpoisoned(&self.messages).push(ChatMessage::user(text));
        self.repair_message_history("before_model_call");
        let hidden_contexts = hidden_contexts
            .into_iter()
            .filter(|context| !context.content.trim().is_empty())
            .collect::<Vec<_>>();
        self.mark_latest_turn_status_with_reason_emitter(
            AgentTurnStatus::GatheringContext,
            "gather_context",
            None,
            emitter,
        );

        let cancel = Arc::new(Notify::new());
        *lock_unpoisoned(&self.cancel) = Some(cancel.clone());
        let _cancel_guard = ActiveCancelGuard::new(&self.cancel, cancel.clone());

        let mut overflow_retry_used = false;

        for _round in 0..10 {
            if !self.running.load(Ordering::SeqCst) {
                break;
            }

            let all_messages = lock_unpoisoned(&self.messages).clone();
            let existing_summary = lock_unpoisoned(&self.summary).clone();
            let skip_proactive_compaction = {
                let mut guard = lock_unpoisoned(&self.auto_compact_guard);
                if guard.should_skip_proactive_compaction() {
                    guard.record_proactive_skip();
                    true
                } else {
                    false
                }
            };
            let compacted = if skip_proactive_compaction {
                CompactResult::unchanged(all_messages, existing_summary)
            } else {
                compact_messages_if_needed(
                    all_messages,
                    existing_summary,
                    self.context_window_tokens,
                )
            };
            lock_unpoisoned(&self.auto_compact_guard).record_result(&compacted);

            if let Some(stats) = compacted.stats.as_ref() {
                self.apply_compaction_emitter(&compacted, stats, "auto_compact", emitter);
            }

            let sp = lock_unpoisoned(&self.system_prompt).clone();
            let context_bundle = build_context_bundle(
                compacted.messages,
                compacted.summary,
                hidden_contexts.clone(),
                sp.clone(),
                self.context_window_tokens,
            );
            self.record_latest_context_emitter(&context_bundle, emitter);
            let mut msgs_with_context = repair_tool_use_adjacency(context_bundle.messages);

            self.mark_latest_turn_status_with_reason_emitter(
                AgentTurnStatus::CallingModel,
                "call_model",
                None,
                emitter,
            );
            let mut retries = 0;
            let result = loop {
                match self
                    .adapter
                    .call_with_emitter(&self.id, &msgs_with_context, emitter, cancel.clone())
                    .await
                {
                    Ok(r) => break r,
                    Err(e) => {
                        let msg = e.to_string();
                        if !overflow_retry_used && is_context_overflow_error(&self.agent_type, &msg)
                        {
                            let all_messages = lock_unpoisoned(&self.messages).clone();
                            let existing_summary = lock_unpoisoned(&self.summary).clone();
                            let compacted =
                                compact_messages_for_overflow_retry(all_messages, existing_summary);
                            lock_unpoisoned(&self.auto_compact_guard).record_result(&compacted);

                            if let Some(stats) = compacted.stats.as_ref() {
                                overflow_retry_used = true;
                                self.apply_compaction_emitter(
                                    &compacted,
                                    stats,
                                    "overflow_retry",
                                    emitter,
                                );

                                let context_bundle = build_context_bundle(
                                    compacted.messages,
                                    compacted.summary,
                                    hidden_contexts.clone(),
                                    sp.clone(),
                                    self.context_window_tokens,
                                );
                                self.record_latest_context_emitter(&context_bundle, emitter);
                                msgs_with_context =
                                    repair_tool_use_adjacency(context_bundle.messages);
                                continue;
                            }
                        }

                        if should_retry_adapter_error(&e, retries) {
                            retries += 1;
                            tokio::time::sleep(AGENT_OVERFLOW_RETRY_DELAY).await;
                            continue;
                        }
                        let err_msg = format!("API error: {}", msg);
                        emitter.emit(self.api_error_event(err_msg.clone()));
                        if self.running.load(Ordering::SeqCst) {
                            self.record_latest_turn_failure_emitter(
                                api_failure_trace(&err_msg),
                                emitter,
                            );
                        } else {
                            self.mark_latest_turn_status_with_reason_emitter(
                                AgentTurnStatus::Cancelled,
                                "user_cancelled",
                                Some("cancelled while handling api error"),
                                emitter,
                            );
                        }
                        return Err(err_msg);
                    }
                }
            };

            if !self.running.load(Ordering::SeqCst) {
                self.mark_latest_turn_status_with_reason_emitter(
                    AgentTurnStatus::Cancelled,
                    "user_cancelled",
                    Some("cancelled after model call"),
                    emitter,
                );
                break;
            }

            if !result.assistant_content.is_empty() {
                lock_unpoisoned(&self.messages).push(ChatMessage::assistant(
                    serde_json::Value::Array(result.assistant_content.clone()),
                ));
            }

            if result.tool_calls.is_empty() {
                crate::app_log!("INFO", "Agent turn {}: no tool calls, done", _round);
                self.mark_latest_turn_status_with_reason_emitter(
                    AgentTurnStatus::Completed,
                    "final_answer",
                    Some("model returned no tool calls"),
                    emitter,
                );
                break;
            }

            self.mark_latest_turn_status_with_reason_emitter(
                AgentTurnStatus::RunningTools,
                "tool_calls_requested",
                Some("model requested tool execution"),
                emitter,
            );

            crate::app_log!(
                "INFO",
                "Agent turn {}: {} tool calls to execute: {:?}",
                _round,
                result.tool_calls.len(),
                result
                    .tool_calls
                    .iter()
                    .map(|tc| tc.name.clone())
                    .collect::<Vec<_>>()
            );

            let (delegated, regular): (Vec<_>, Vec<_>) = result
                .tool_calls
                .iter()
                .partition(|tc| tc.name == "delegate_task");

            let mut sub_results: Vec<(usize, String)> = Vec::new();
            if !delegated.is_empty() {
                let mut handles = Vec::new();
                for tc in &delegated {
                    let started_at_ms = now_ms();
                    self.record_latest_tool_emitter(
                        running_tool_trace(
                            tc.id.clone(),
                            tc.name.clone(),
                            &tc.input,
                            started_at_ms,
                        ),
                        emitter,
                    );
                    let task = tc
                        .input
                        .get("task")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Investigate and report findings")
                        .to_string();
                    let adapter = self.adapter.clone();
                    let harness = self.harness.clone();
                    let cancel = lock_unpoisoned(&self.cancel)
                        .clone()
                        .unwrap_or_else(|| Arc::new(Notify::new()));
                    let idx = result
                        .tool_calls
                        .iter()
                        .position(|t| t.id == tc.id)
                        .unwrap_or(0);
                    let wd = self.harness.working_dir.clone();
                    let emitter: Arc<dyn crate::agent::event_sink::EventEmitter> =
                        Arc::new(crate::agent::event_sink::NoopEventEmitter);
                    handles.push((
                        idx,
                        tc.id.clone(),
                        tc.name.clone(),
                        tc.input.clone(),
                        started_at_ms,
                        tokio::spawn(async move {
                            let r = crate::agent::sub::SubAgent::run_with_emitter(
                                &task, adapter, harness, &*emitter, cancel, &wd,
                            )
                            .await;
                            (idx, r)
                        }),
                    ));
                }
                for (fallback_idx, id, name, input, started_at_ms, handle) in handles {
                    match handle.await {
                        Ok((idx, r)) => {
                            let api_text: String = serde_json::from_str::<serde_json::Value>(&r)
                                .ok()
                                .and_then(|v| {
                                    v.get("result")
                                        .and_then(|r| r.as_str())
                                        .map(|s| s.to_string())
                                })
                                .unwrap_or_else(|| r.clone());
                            emitter.emit(self.tool_call_result_event(&id, &r, false, 0));
                            self.record_latest_tool_emitter(
                                completed_tool_trace(
                                    id.clone(),
                                    name.clone(),
                                    &input,
                                    &r,
                                    started_at_ms,
                                    now_ms(),
                                ),
                                emitter,
                            );
                            sub_results.push((idx, api_text));
                        }
                        Err(err) => {
                            let message = sub_agent_join_error_message(&err);
                            emitter.emit(self.tool_call_result_event(&id, &message, true, 0));
                            self.record_latest_tool_emitter(
                                completed_tool_trace(
                                    id,
                                    name,
                                    &input,
                                    &message,
                                    started_at_ms,
                                    now_ms(),
                                ),
                                emitter,
                            );
                            sub_results.push((fallback_idx, message));
                        }
                    }
                }
            }

            let (reads, writes): (
                Vec<&crate::adapters::base::ToolCall>,
                Vec<&crate::adapters::base::ToolCall>,
            ) = regular.iter().partition(|tc| is_read_only_tool(&tc.name));

            let mut read_results: Vec<(String, String)> = Vec::new();
            {
                let mut handles = Vec::new();
                for tc in &reads {
                    let h = self.harness.clone();
                    let sid = self.id.clone();
                    let name = tc.name.clone();
                    let input = tc.input.clone();
                    let tool_emitter: Arc<dyn crate::agent::event_sink::EventEmitter> =
                        Arc::new(crate::agent::event_sink::NoopEventEmitter);
                    let id = tc.id.clone();
                    let started_at_ms = now_ms();
                    let cancel_for_tool = cancel.clone();
                    self.record_latest_tool_emitter(
                        running_tool_trace(id.clone(), name.clone(), &input, started_at_ms),
                        emitter,
                    );
                    handles.push(tokio::spawn(async move {
                        let result = h
                            .execute_tool_with_emitter(
                                &sid,
                                &name,
                                &input,
                                tool_emitter,
                                Some(&id),
                                Some(cancel_for_tool),
                            )
                            .await;
                        (id, name, input, started_at_ms, now_ms(), result)
                    }));
                }
                for handle in handles {
                    if let Ok((id, name, input, started_at_ms, ended_at_ms, result)) = handle.await
                    {
                        self.record_latest_tool_emitter(
                            completed_tool_trace(
                                id.clone(),
                                name,
                                &input,
                                &result,
                                started_at_ms,
                                ended_at_ms,
                            ),
                            emitter,
                        );
                        read_results.push((id, result));
                    }
                }
            }

            let mut write_results: Vec<(String, String)> = Vec::new();
            for tc in &writes {
                let started_at_ms = now_ms();
                self.record_latest_tool_emitter(
                    running_tool_trace(tc.id.clone(), tc.name.clone(), &tc.input, started_at_ms),
                    emitter,
                );
                let result = self
                    .harness
                    .execute_tool_with_emitter(
                        &self.id,
                        &tc.name,
                        &tc.input,
                        Arc::new(crate::agent::event_sink::NoopEventEmitter),
                        Some(&tc.id),
                        Some(cancel.clone()),
                    )
                    .await;
                self.record_latest_tool_emitter(
                    completed_tool_trace(
                        tc.id.clone(),
                        tc.name.clone(),
                        &tc.input,
                        &result,
                        started_at_ms,
                        now_ms(),
                    ),
                    emitter,
                );
                write_results.push((tc.id.clone(), result));
            }

            let mut result_map: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for (id, r) in read_results {
                result_map.insert(id, r);
            }
            for (id, r) in write_results {
                result_map.insert(id, r);
            }
            for (idx, r) in sub_results {
                if let Some(tc) = result.tool_calls.get(idx) {
                    result_map.insert(tc.id.clone(), r);
                }
            }

            let model_tool_results =
                build_tool_result_message_for_model(&result_map, &result.tool_calls);
            for resolved in &model_tool_results.results {
                if resolved.missing {
                    let Some(tc) = result
                        .tool_calls
                        .iter()
                        .find(|tc| tc.id == resolved.tool_call_id)
                    else {
                        continue;
                    };
                    self.record_latest_tool_emitter(
                        completed_tool_trace(
                            tc.id.clone(),
                            tc.name.clone(),
                            &tc.input,
                            &resolved.content,
                            now_ms(),
                            now_ms(),
                        ),
                        emitter,
                    );
                }
                crate::app_log!(
                    "INFO",
                    "Agent tool '{}' result ({} chars)",
                    resolved.tool_name,
                    resolved.content.len()
                );
            }
            lock_unpoisoned(&self.messages).push(model_tool_results.message);

            tokio::time::sleep(AGENT_LOOP_SETTLE_DELAY).await;
        }

        let verification_trace = if self.running.load(Ordering::SeqCst) {
            self.verify_latest_turn_emitter(emitter).await
        } else {
            None
        };

        if self.running.load(Ordering::SeqCst) {
            let messages = lock_unpoisoned(&self.messages).clone();
            let summary = lock_unpoisoned(&self.summary).clone();
            let sp = lock_unpoisoned(&self.system_prompt).clone();
            let context_bundle = build_context_bundle(
                messages,
                summary,
                hidden_contexts.clone(),
                sp,
                self.context_window_tokens,
            );
            self.record_latest_context_emitter(&context_bundle, emitter);
            let mut msgs = repair_tool_use_adjacency(context_bundle.messages);
            let last_role = msgs.last().map(|m| m.role.clone()).unwrap_or_default();
            if last_role == "tool" || last_role == "user" {
                msgs.push(ChatMessage::user(&final_answer_instruction(
                    verification_trace.as_ref(),
                )));
                crate::app_log!("INFO", "Agent loop complete — requesting text-only summary");
                if let Ok(result) = self
                    .adapter
                    .call_with_emitter(&self.id, &msgs, emitter, cancel.clone())
                    .await
                {
                    if !result.assistant_content.is_empty() {
                        let mut messages = lock_unpoisoned(&self.messages);
                        push_assistant_result_with_synthetic_tool_results(
                            &mut messages,
                            result.assistant_content,
                            &result.tool_calls,
                            "final_summary_tool_call_not_executed",
                        );
                    }
                }
            }
        }

        crate::app_log!("INFO", "Agent loop complete");
        let current_turn_status = lock_unpoisoned(&self.latest_turn)
            .as_ref()
            .map(|turn| turn.status.clone())
            .unwrap_or(AgentTurnStatus::Started);
        let final_reason = final_turn_transition_reason_for_current_turn(
            current_turn_status.clone(),
            self.running.load(Ordering::SeqCst),
            verification_trace.as_ref(),
        );
        self.mark_latest_turn_status_with_reason_emitter(
            final_turn_status_for_current_turn(
                current_turn_status,
                self.running.load(Ordering::SeqCst),
                verification_trace.as_ref(),
            ),
            final_reason,
            None,
            emitter,
        );
        Ok(())
    }
}

fn build_context_bundle(
    messages: Vec<ChatMessage>,
    summary: Option<String>,
    hidden_contexts: Vec<HiddenContextPart>,
    system_prompt: String,
    context_window_tokens: Option<u32>,
) -> ContextBundle {
    ContextBuilder::new()
        .messages(messages)
        .summary(summary)
        .hidden_contexts(hidden_contexts)
        .system_prompt(system_prompt)
        .context_window_tokens(context_window_tokens)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::missing_key::MissingKeyAdapter;
    use crate::agent::turn_state::{AgentEvidenceKind, AgentToolStatus};

    #[test]
    fn restore_state_normalizes_interrupted_turn_and_repairs_tool_history() {
        let workspace =
            std::env::temp_dir().join(format!("forge-session-restore-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
        let session = AgentSession::new(
            "session-1".to_string(),
            "deepseek".to_string(),
            adapter,
            Arc::new(Harness::new(workspace.clone())),
            "system".to_string(),
            Some(128_000),
        );
        let messages = vec![
            ChatMessage::user("先安装依赖"),
            ChatMessage::assistant(serde_json::json!([{
                "type": "tool_use",
                "id": "call_1",
                "name": "bash",
                "input": {"command": "npm install"}
            }])),
            ChatMessage::user("继续"),
        ];
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            workspace.to_string_lossy().to_string(),
            "deepseek".to_string(),
            "deepseek-chat".to_string(),
            "workflow".to_string(),
            "implementation".to_string(),
            "安装依赖并继续生成工具".to_string(),
        );
        turn.mark_status_with_reason(
            AgentTurnStatus::RunningTools,
            "tool_calls_requested",
            Some("model requested tool execution"),
        );
        turn.record_tool(running_tool_trace(
            "call_1".to_string(),
            "bash".to_string(),
            &serde_json::json!({"command": "npm install"}),
            10,
        ));

        session.restore_state(messages, Some("old summary".to_string()), Some(turn));

        let snapshot = session.snapshot();
        assert_eq!(snapshot.messages.len(), 4);
        assert_eq!(snapshot.messages[2].role, "user");
        assert!(snapshot.messages[2]
            .content
            .to_string()
            .contains("previous tool call was interrupted"));

        let restored_turn = snapshot.latest_turn.expect("latest turn");
        assert_eq!(restored_turn.status, AgentTurnStatus::Cancelled);
        assert_eq!(restored_turn.tools[0].status, AgentToolStatus::Cancelled);
        assert!(restored_turn.tools[0].is_error);
        assert_eq!(
            restored_turn.tools[0].command.as_deref(),
            Some("npm install")
        );
        let evidence = restored_turn
            .evidence
            .iter()
            .find(|item| item.kind == AgentEvidenceKind::Tool && item.tool_call_id == "call_1")
            .expect("cancelled tool evidence");
        assert_eq!(evidence.status, AgentToolStatus::Cancelled);
        assert_eq!(evidence.failure_kind.as_deref(), Some("interrupted"));

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn restore_state_preserves_completed_turn_unchanged() {
        let workspace = std::env::temp_dir().join(format!(
            "forge-session-restore-completed-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
        let session = AgentSession::new(
            "session-restore-completed".to_string(),
            "deepseek".to_string(),
            adapter,
            Arc::new(Harness::new(workspace.clone())),
            "system".to_string(),
            Some(128_000),
        );

        let messages = vec![
            ChatMessage::user("hello"),
            ChatMessage::assistant(serde_json::json!([{"type": "text", "text": "hi"}])),
        ];
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-restore-completed".to_string(),
            workspace.to_string_lossy().to_string(),
            "deepseek".to_string(),
            "deepseek-chat".to_string(),
            "workflow".to_string(),
            "direct".to_string(),
            "hello".to_string(),
        );
        turn.mark_status_with_reason(AgentTurnStatus::Completed, "final_answer", None);

        session.restore_state(messages, None, Some(turn));

        let restored_turn = lock_unpoisoned(&session.latest_turn);
        assert_eq!(
            restored_turn.as_ref().unwrap().status,
            AgentTurnStatus::Completed,
            "completed turn should stay completed after restore"
        );
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn restore_state_preserves_cancelled_turn_unchanged() {
        let workspace = std::env::temp_dir().join(format!(
            "forge-session-restore-cancelled-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
        let session = AgentSession::new(
            "session-restore-cancelled".to_string(),
            "deepseek".to_string(),
            adapter,
            Arc::new(Harness::new(workspace.clone())),
            "system".to_string(),
            Some(128_000),
        );

        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-restore-cancelled".to_string(),
            workspace.to_string_lossy().to_string(),
            "deepseek".to_string(),
            "deepseek-chat".to_string(),
            "workflow".to_string(),
            "direct".to_string(),
            "test".to_string(),
        );
        turn.mark_status_with_reason(AgentTurnStatus::Cancelled, "user_cancelled", Some("killed"));

        session.restore_state(vec![ChatMessage::user("test")], None, Some(turn));

        let restored_turn = lock_unpoisoned(&session.latest_turn);
        assert_eq!(
            restored_turn.as_ref().unwrap().status,
            AgentTurnStatus::Cancelled,
            "cancelled turn should stay cancelled after restore"
        );
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn restore_state_with_no_latest_turn_preserves_none() {
        let workspace = std::env::temp_dir().join(format!(
            "forge-session-restore-none-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
        let session = AgentSession::new(
            "session-restore-none".to_string(),
            "deepseek".to_string(),
            adapter,
            Arc::new(Harness::new(workspace.clone())),
            "system".to_string(),
            Some(128_000),
        );

        session.restore_state(
            vec![ChatMessage::user("hello")],
            Some("summary".to_string()),
            None,
        );

        assert!(lock_unpoisoned(&session.latest_turn).is_none());
        assert_eq!(
            *lock_unpoisoned(&session.summary),
            Some("summary".to_string())
        );
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn restore_state_normalizes_only_active_turn_statuses() {
        let workspace = std::env::temp_dir().join(format!(
            "forge-session-restore-active-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));

        for (input_status, expected_status, label) in [
            (
                AgentTurnStatus::Started,
                AgentTurnStatus::Cancelled,
                "Started",
            ),
            (
                AgentTurnStatus::GatheringContext,
                AgentTurnStatus::Cancelled,
                "GatheringContext",
            ),
            (
                AgentTurnStatus::CallingModel,
                AgentTurnStatus::Cancelled,
                "CallingModel",
            ),
            (
                AgentTurnStatus::RunningTools,
                AgentTurnStatus::Cancelled,
                "RunningTools",
            ),
            (
                AgentTurnStatus::Verifying,
                AgentTurnStatus::Cancelled,
                "Verifying",
            ),
            (
                AgentTurnStatus::Completed,
                AgentTurnStatus::Completed,
                "Completed",
            ),
            (
                AgentTurnStatus::Cancelled,
                AgentTurnStatus::Cancelled,
                "Cancelled",
            ),
            (AgentTurnStatus::Failed, AgentTurnStatus::Failed, "Failed"),
        ] {
            let session = AgentSession::new(
                format!("session-restore-{label}"),
                "deepseek".to_string(),
                adapter.clone(),
                Arc::new(Harness::new(workspace.clone())),
                "system".to_string(),
                Some(128_000),
            );

            let mut turn = AgentTurnState::new(
                "turn-1".to_string(),
                format!("session-restore-{label}"),
                workspace.to_string_lossy().to_string(),
                "deepseek".to_string(),
                "deepseek-chat".to_string(),
                "workflow".to_string(),
                "direct".to_string(),
                "test".to_string(),
            );
            turn.mark_status_with_reason(input_status.clone(), "test_reason", None);

            session.restore_state(vec![ChatMessage::user("x")], None, Some(turn));

            let restored = lock_unpoisoned(&session.latest_turn);
            assert_eq!(
                restored.as_ref().unwrap().status,
                expected_status,
                "{label}: {input_status:?} should become {expected_status:?}"
            );
        }

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn latest_turn_updated_event_can_be_built_without_app_handle() {
        let workspace =
            std::env::temp_dir().join(format!("forge-session-turn-event-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
        let session = AgentSession::new(
            "session-1".to_string(),
            "deepseek".to_string(),
            adapter,
            Arc::new(Harness::new(workspace.clone())),
            "system".to_string(),
            Some(128_000),
        );
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            workspace.to_string_lossy().to_string(),
            "deepseek".to_string(),
            "deepseek-chat".to_string(),
            "workflow".to_string(),
            "implementation".to_string(),
            "生成一个本地小工具".to_string(),
        );
        turn.mark_status_with_reason(AgentTurnStatus::CallingModel, "call_model", None);
        *lock_unpoisoned(&session.latest_turn) = Some(turn);

        let event = session
            .latest_turn_updated_event()
            .expect("latest turn event");

        match event {
            StreamEvent::AgentTurnUpdated { session_id, state } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(state.status, AgentTurnStatus::CallingModel);
                assert_eq!(state.step_label, "请求模型");
                assert_eq!(
                    std::path::PathBuf::from(state.workspace_path)
                        .canonicalize()
                        .expect("projection workspace"),
                    workspace.canonicalize().expect("workspace")
                );
            }
            other => panic!("unexpected event: {other:?}"),
        }

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn lifecycle_events_can_be_built_without_app_handle() {
        let workspace = std::env::temp_dir().join(format!(
            "forge-session-lifecycle-events-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
        let session = AgentSession::new(
            "session-1".to_string(),
            "deepseek".to_string(),
            adapter,
            Arc::new(Harness::new(workspace.clone())),
            "system".to_string(),
            Some(128_000),
        );
        let compact_stats = CompactStats {
            summary: "保留最近上下文".to_string(),
            retained_messages: 16,
            compacted_messages: 48,
            estimated_tokens_before: 120_000,
            estimated_tokens_after: 42_000,
        };

        let error = session.api_error_event("API error: timeout".to_string());
        let stopped = session.session_stopped_event("killed");
        let compacted = session.context_compacted_event(&compact_stats);
        let tool_result = session.tool_call_result_event("tool-1", "ok", false, 25);

        match error {
            StreamEvent::Error {
                session_id,
                message,
                code,
                ..
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(message, "API error: timeout");
                assert_eq!(code, "api_error");
            }
            other => panic!("unexpected error event: {other:?}"),
        }
        match stopped {
            StreamEvent::SessionStopped { session_id, reason } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(reason, "killed");
            }
            other => panic!("unexpected stopped event: {other:?}"),
        }
        match compacted {
            StreamEvent::ContextCompacted {
                session_id,
                summary,
                retained_messages,
                compacted_messages,
                estimated_tokens_before,
                estimated_tokens_after,
                ..
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(summary, "保留最近上下文");
                assert_eq!(retained_messages, 16);
                assert_eq!(compacted_messages, 48);
                assert_eq!(estimated_tokens_before, 120_000);
                assert_eq!(estimated_tokens_after, 42_000);
            }
            other => panic!("unexpected compacted event: {other:?}"),
        }
        match tool_result {
            StreamEvent::ToolCallResult {
                session_id,
                block_id,
                result,
                is_error,
                duration_ms,
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(block_id, "tool-1");
                assert_eq!(result, "ok");
                assert!(!is_error);
                assert_eq!(duration_ms, 25);
            }
            other => panic!("unexpected tool result event: {other:?}"),
        }

        let _ = std::fs::remove_dir_all(workspace);
    }

    // ── FakeAdapter for full-turn testing ─────────────────────────

    use crate::adapters::base::{AdapterError, AiAdapter, StreamResult, ToolCall};
    use crate::agent::event_sink::EventEmitter;

    /// Scriptable adapter that returns a pre-defined sequence of `StreamResult`s.
    /// Thread-safe: uses `AtomicUsize` for call counting.
    struct FakeAdapter {
        results: Vec<Result<StreamResult, String>>,
        call_count: std::sync::atomic::AtomicUsize,
        model_id: String,
    }

    impl FakeAdapter {
        fn new(responses: Vec<StreamResult>) -> Self {
            Self {
                results: responses.into_iter().map(Ok).collect(),
                call_count: std::sync::atomic::AtomicUsize::new(0),
                model_id: "fake-model".to_string(),
            }
        }

        fn new_with_errors(errors: Vec<Result<StreamResult, String>>) -> Self {
            Self {
                results: errors,
                call_count: std::sync::atomic::AtomicUsize::new(0),
                model_id: "fake-model".to_string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl AiAdapter for FakeAdapter {
        async fn stream_message(
            &self,
            _session_id: &str,
            _messages: &[crate::adapters::base::ChatMessage],
            _app_handle: &tauri::AppHandle,
            _cancel: Arc<Notify>,
        ) -> Result<StreamResult, AdapterError> {
            panic!(
                "FakeAdapter::stream_message should not be called in tests — use call_with_emitter"
            );
        }

        async fn call(
            &self,
            _messages: &[crate::adapters::base::ChatMessage],
            _cancel: Arc<Notify>,
        ) -> Result<StreamResult, AdapterError> {
            let idx = self
                .call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match self.results.get(idx) {
                Some(Ok(r)) => Ok(r.clone()),
                Some(Err(msg)) => Err(AdapterError::Stream(msg.clone())),
                None => Err(AdapterError::Stream(format!(
                    "FakeAdapter: no response at index {idx}"
                ))),
            }
        }

        async fn call_with_emitter(
            &self,
            _session_id: &str,
            _messages: &[crate::adapters::base::ChatMessage],
            _emitter: &dyn EventEmitter,
            cancel: Arc<Notify>,
        ) -> Result<StreamResult, AdapterError> {
            self.call(_messages, cancel).await
        }

        fn model_id(&self) -> &str {
            &self.model_id
        }

        fn model_name(&self) -> &str {
            "Fake Model"
        }
    }

    /// Build a test workspace with a known file and return the path.
    fn setup_test_workspace(prefix: &str) -> std::path::PathBuf {
        let workspace = std::env::temp_dir().join(format!("{}-{}", prefix, uuid::Uuid::now_v7()));
        std::fs::create_dir_all(workspace.join("src")).expect("create workspace");
        std::fs::write(
            workspace.join("src").join("main.rs"),
            "fn main() { println!(\"hello world\"); }\n",
        )
        .expect("write test file");
        workspace
    }

    #[tokio::test]
    async fn full_agent_turn_fake_adapter_preserves_tool_result_order_and_history() {
        // This test proves the complete agent turn loop works without a real API:
        //   user input → fake adapter returns tool_call → harness executes read_file →
        //   tool_result assembled in original order → fake adapter returns final text →
        //   turn completes with correct message history.
        let workspace = setup_test_workspace("forge-full-turn");
        let harness = Arc::new(Harness::new(workspace.clone()));
        let session_id = "session-full-turn".to_string();

        // Response 1: model wants to read a file
        // assistant_content must include the tool_use block (matches real adapter behavior)
        let tool_call_id = "call-read-1".to_string();
        let response_1 = StreamResult {
            assistant_content: vec![
                serde_json::json!({
                    "type": "text",
                    "text": "让我先查看源码"
                }),
                serde_json::json!({
                    "type": "tool_use",
                    "id": tool_call_id.clone(),
                    "name": "read_file",
                    "input": {"path": "src/main.rs"}
                }),
            ],
            tool_calls: vec![ToolCall {
                id: tool_call_id.clone(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "src/main.rs"}),
            }],
            stop_reason: Some("tool_use".to_string()),
        };

        // Response 2: model returns final text after seeing tool result
        let response_2 = StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text",
                "text": "这是一个 hello world 程序"
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        };

        let adapter = Arc::new(FakeAdapter::new(vec![response_1, response_2]));
        let session = AgentSession::new(
            session_id.clone(),
            "deepseek".to_string(),
            adapter.clone(),
            harness.clone(),
            "你是一个编程助手".to_string(),
            Some(128_000),
        );

        let emitter = crate::agent::event_sink::NoopEventEmitter;
        let turn_guard = session.reserve_turn().expect("reserve turn");

        let result = session
            .send_message_with_emitter(
                "帮我看看 src/main.rs 的内容",
                &emitter,
                vec![],
                None,
                None,
                turn_guard,
            )
            .await;

        assert!(
            result.is_ok(),
            "agent turn should succeed: {:?}",
            result.err()
        );

        // Verify message history structure
        let messages = lock_unpoisoned(&session.messages);
        // Expected: user, assistant(tool_use), user(tool_result), assistant(final text)
        // The summary request may add one more assistant message
        assert!(
            messages.len() >= 4,
            "expected at least 4 messages, got {}",
            messages.len()
        );

        // 1. User message
        assert_eq!(messages[0].role, "user");
        assert!(messages[0]
            .content
            .as_str()
            .unwrap_or_default()
            .contains("src/main.rs"));

        // 2. Assistant with tool_use
        assert_eq!(messages[1].role, "assistant");
        let assistant_blocks = messages[1]
            .content
            .as_array()
            .expect("assistant content blocks");
        let tool_use_block = assistant_blocks
            .iter()
            .find(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
            .expect("assistant should have tool_use block");
        assert_eq!(
            tool_use_block.get("id").and_then(|v| v.as_str()),
            Some(tool_call_id.as_str())
        );
        assert_eq!(
            tool_use_block.get("name").and_then(|v| v.as_str()),
            Some("read_file")
        );

        // 3. User with tool_result — must follow immediately after assistant tool_use
        assert_eq!(messages[2].role, "user");
        let result_blocks = messages[2].content.as_array().expect("tool result blocks");
        assert_eq!(
            result_blocks.len(),
            1,
            "expected exactly 1 tool_result block"
        );
        assert_eq!(
            result_blocks[0].get("type").and_then(|v| v.as_str()),
            Some("tool_result")
        );
        assert_eq!(
            result_blocks[0].get("tool_use_id").and_then(|v| v.as_str()),
            Some(tool_call_id.as_str()),
            "tool_result must reference the original tool_use id"
        );
        // Tool result should contain the file content
        let result_content = result_blocks[0]
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            result_content.contains("hello world"),
            "tool result should contain file content, got: {}",
            result_content
        );

        // 4. Assistant final text
        let final_msg = messages.iter().rev().find(|m| m.role == "assistant");
        let final_text = final_msg
            .and_then(|m| m.content.as_array())
            .and_then(|blocks| {
                blocks.iter().find_map(|b| {
                    if b.get("type").and_then(|v| v.as_str()) == Some("text") {
                        b.get("text").and_then(|v| v.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
            })
            .expect("final assistant text");
        assert!(
            final_text.contains("hello world"),
            "final text should reference the file content"
        );

        // Verify adapter was called exactly 2 times (tool round + final summary)
        assert_eq!(
            adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "adapter should be called exactly 2 times"
        );

        // Verify turn state
        let turn = lock_unpoisoned(&session.latest_turn);
        let turn = turn.as_ref().expect("latest turn should exist");
        assert_eq!(
            turn.status,
            AgentTurnStatus::Completed,
            "turn should be completed"
        );

        // Verify workspace is preserved in turn metadata
        assert!(
            turn.session_id == session_id,
            "turn should reference the correct session"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn full_agent_turn_multiple_tool_calls_preserve_order() {
        // This test proves that when the model requests multiple tool calls,
        // the results are assembled in the ORIGINAL tool_call order,
        // not the execution completion order.
        let workspace = setup_test_workspace("forge-multi-tool");
        let harness = Arc::new(Harness::new(workspace.clone()));

        // Write two test files
        std::fs::write(workspace.join("a.txt"), "content-A\n").expect("write a.txt");
        std::fs::write(workspace.join("b.txt"), "content-B\n").expect("write b.txt");

        // Response 1: model requests two read_file calls
        let response_1 = StreamResult {
            assistant_content: vec![
                serde_json::json!({
                    "type": "text",
                    "text": "让我同时读取两个文件"
                }),
                serde_json::json!({
                    "type": "tool_use",
                    "id": "call-a",
                    "name": "read_file",
                    "input": {"path": "a.txt"}
                }),
                serde_json::json!({
                    "type": "tool_use",
                    "id": "call-b",
                    "name": "read_file",
                    "input": {"path": "b.txt"}
                }),
            ],
            tool_calls: vec![
                ToolCall {
                    id: "call-a".to_string(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({"path": "a.txt"}),
                },
                ToolCall {
                    id: "call-b".to_string(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({"path": "b.txt"}),
                },
            ],
            stop_reason: Some("tool_use".to_string()),
        };

        // Response 2: final text
        let response_2 = StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text",
                "text": "两个文件都读取完毕"
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        };

        let adapter = Arc::new(FakeAdapter::new(vec![response_1, response_2]));
        let session = AgentSession::new(
            "session-multi".to_string(),
            "deepseek".to_string(),
            adapter,
            harness.clone(),
            "你是一个编程助手".to_string(),
            Some(128_000),
        );

        let emitter = crate::agent::event_sink::NoopEventEmitter;
        let turn_guard = session.reserve_turn().expect("reserve turn");

        let result = session
            .send_message_with_emitter(
                "读取 a.txt 和 b.txt",
                &emitter,
                vec![],
                None,
                None,
                turn_guard,
            )
            .await;

        assert!(result.is_ok(), "turn should succeed: {:?}", result.err());

        let messages = lock_unpoisoned(&session.messages);

        // Find the tool_result message
        let tool_result_msg = messages
            .iter()
            .find(|m| {
                m.role == "user"
                    && m.content.as_array().is_some_and(|blocks| {
                        blocks
                            .iter()
                            .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_result"))
                    })
            })
            .expect("should have tool_result message");

        let blocks = tool_result_msg.content.as_array().unwrap();
        assert_eq!(blocks.len(), 2, "expected 2 tool_result blocks");

        // Verify ORDER: call-a first, call-b second
        assert_eq!(
            blocks[0].get("tool_use_id").and_then(|v| v.as_str()),
            Some("call-a"),
            "first result must be for call-a"
        );
        assert_eq!(
            blocks[1].get("tool_use_id").and_then(|v| v.as_str()),
            Some("call-b"),
            "second result must be for call-b"
        );

        // Verify content
        let content_a = blocks[0]
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let content_b = blocks[1]
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            content_a.contains("content-A"),
            "result A should contain content-A"
        );
        assert!(
            content_b.contains("content-B"),
            "result B should contain content-B"
        );

        // Verify no missing results
        assert!(
            blocks[0].get("is_error").is_none(),
            "call-a should not be marked as error"
        );
        assert!(
            blocks[1].get("is_error").is_none(),
            "call-b should not be marked as error"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn api_error_does_not_stop_session_and_recovery_succeeds() {
        // This test proves the recovery contract:
        //   API error → session stays running → user retries → turn succeeds.
        // The session must NOT be stopped on API error; the turn must be
        // recoverable via the standard send_message path.
        let workspace = setup_test_workspace("forge-api-error-recovery");
        let harness = Arc::new(Harness::new(workspace.clone()));

        // Response 1: API error
        let error_response = Err("API error: 500 — Internal server error".to_string());

        // Response 2: success after retry
        let success_response = Ok(StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text",
                "text": "恢复成功，文件内容已读取"
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        });

        let adapter = Arc::new(FakeAdapter::new_with_errors(vec![
            error_response,
            success_response,
        ]));
        let session = AgentSession::new(
            "session-api-recovery".to_string(),
            "deepseek".to_string(),
            adapter.clone(),
            harness,
            "你是一个编程助手".to_string(),
            Some(128_000),
        );

        let emitter = crate::agent::event_sink::CollectingEventEmitter::new();

        // First turn: API error
        let turn_guard = session.reserve_turn().expect("reserve turn 1");
        let result1 = session
            .send_message_with_emitter("读取文件", &emitter, vec![], None, None, turn_guard)
            .await;

        assert!(result1.is_err(), "first turn should fail with API error");
        assert!(
            result1.unwrap_err().contains("500"),
            "error should mention status code"
        );

        // Session must still be running
        assert!(
            session.running.load(Ordering::SeqCst),
            "session should still be running after API error"
        );

        // Turn guard was dropped, so turn_inflight should be false
        let turn_guard2 = session.reserve_turn();
        assert!(
            turn_guard2.is_ok(),
            "should be able to reserve a new turn after API error"
        );

        // Second turn: recovery
        let result2 = session
            .send_message_with_emitter("继续", &emitter, vec![], None, None, turn_guard2.unwrap())
            .await;

        assert!(
            result2.is_ok(),
            "recovery turn should succeed: {:?}",
            result2.err()
        );

        // Verify turn state recovered
        let turn = lock_unpoisoned(&session.latest_turn);
        let turn = turn.as_ref().expect("latest turn should exist");
        assert_eq!(
            turn.status,
            AgentTurnStatus::Completed,
            "recovered turn should be completed"
        );

        // Verify adapter was called: 1 (failed) + 1 (recovery) + 1 (final summary) = 3
        // The failed call counts because FakeAdapter processes it
        assert_eq!(
            adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "adapter should be called 2 times (1 failed + 1 success with final summary)"
        );

        // Verify error events were emitted
        let events = emitter.drain();
        let has_error = events
            .iter()
            .any(|e| matches!(e, StreamEvent::Error { .. }));
        assert!(has_error, "should have emitted an error event");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn tool_failure_result_is_error_and_model_sees_it() {
        // This test proves that when a tool execution fails (e.g., file not found),
        // the error result is properly marked and the model can see it and recover.
        let workspace = setup_test_workspace("forge-tool-failure");
        let harness = Arc::new(Harness::new(workspace.clone()));

        // Response 1: model requests a non-existent file
        let response_1 = StreamResult {
            assistant_content: vec![
                serde_json::json!({
                    "type": "text",
                    "text": "让我读取这个文件"
                }),
                serde_json::json!({
                    "type": "tool_use",
                    "id": "call-missing",
                    "name": "read_file",
                    "input": {"path": "nonexistent.txt"}
                }),
            ],
            tool_calls: vec![ToolCall {
                id: "call-missing".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "nonexistent.txt"}),
            }],
            stop_reason: Some("tool_use".to_string()),
        };

        // Response 2: model sees the error and responds accordingly
        let response_2 = StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text",
                "text": "文件不存在，让我检查一下目录"
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        };

        let adapter = Arc::new(FakeAdapter::new(vec![response_1, response_2]));
        let session = AgentSession::new(
            "session-tool-failure".to_string(),
            "deepseek".to_string(),
            adapter,
            harness,
            "你是一个编程助手".to_string(),
            Some(128_000),
        );

        let emitter = crate::agent::event_sink::NoopEventEmitter;
        let turn_guard = session.reserve_turn().expect("reserve turn");

        let result = session
            .send_message_with_emitter(
                "读取 nonexistent.txt",
                &emitter,
                vec![],
                None,
                None,
                turn_guard,
            )
            .await;

        assert!(
            result.is_ok(),
            "turn should succeed even with tool failure: {:?}",
            result.err()
        );

        let messages = lock_unpoisoned(&session.messages);

        // Find the tool_result message
        let tool_result_msg = messages
            .iter()
            .find(|m| {
                m.role == "user"
                    && m.content.as_array().is_some_and(|blocks| {
                        blocks
                            .iter()
                            .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_result"))
                    })
            })
            .expect("should have tool_result message");

        let blocks = tool_result_msg.content.as_array().unwrap();
        assert_eq!(blocks.len(), 1);

        // Tool result should contain the error
        let content = blocks[0]
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            content.contains("Error")
                || content.contains("不存在")
                || content.contains("not found"),
            "tool result should indicate file not found, got: {}",
            content
        );

        // Turn should be completed
        let turn = lock_unpoisoned(&session.latest_turn);
        let turn = turn.as_ref().expect("latest turn");
        assert_eq!(turn.status, AgentTurnStatus::Completed);

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn adapter_called_exactly_once_per_tool_round_plus_final_summary() {
        // This test proves the adapter call budget is correct:
        //   1 call for tool round + 1 call for final summary = 2 total.
        // No phantom retries, no extra calls.
        let workspace = setup_test_workspace("forge-call-count");
        let harness = Arc::new(Harness::new(workspace.clone()));

        std::fs::write(workspace.join("data.txt"), "test-data\n").expect("write data.txt");

        let response_1 = StreamResult {
            assistant_content: vec![
                serde_json::json!({"type": "text", "text": "读取中"}),
                serde_json::json!({
                    "type": "tool_use", "id": "c1", "name": "read_file",
                    "input": {"path": "data.txt"}
                }),
            ],
            tool_calls: vec![ToolCall {
                id: "c1".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "data.txt"}),
            }],
            stop_reason: Some("tool_use".to_string()),
        };

        let response_2 = StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text", "text": "数据已读取: test-data"
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        };

        let adapter = Arc::new(FakeAdapter::new(vec![response_1, response_2]));
        let session = AgentSession::new(
            "session-call-count".to_string(),
            "deepseek".to_string(),
            adapter.clone(),
            harness,
            "你是一个编程助手".to_string(),
            Some(128_000),
        );

        let emitter = crate::agent::event_sink::NoopEventEmitter;
        let turn_guard = session.reserve_turn().expect("reserve turn");

        let _ = session
            .send_message_with_emitter("读取 data.txt", &emitter, vec![], None, None, turn_guard)
            .await;

        assert_eq!(
            adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "should call adapter exactly 2 times: 1 tool round + 1 final summary"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    // ── CancellableFakeAdapter for cancel mid-turn testing ─────────

    /// Adapter that returns tool_use on call 1, signals "ready" then waits for
    /// the passed-in cancel token on call 2, returns Ok text on call 3+.
    /// The cancel token comes from the session — `kill_with_emitter` fires it
    /// via `notify_one()`, matching real HTTP adapter behavior.
    struct CancellableFakeAdapter {
        call_count: std::sync::atomic::AtomicUsize,
        /// Set to true when the adapter reaches its blocking call (call 2).
        ready: std::sync::atomic::AtomicBool,
    }

    impl CancellableFakeAdapter {
        fn new() -> Self {
            Self {
                call_count: std::sync::atomic::AtomicUsize::new(0),
                ready: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }

    #[async_trait::async_trait]
    impl AiAdapter for CancellableFakeAdapter {
        async fn stream_message(
            &self,
            _session_id: &str,
            _messages: &[crate::adapters::base::ChatMessage],
            _app_handle: &tauri::AppHandle,
            _cancel: Arc<Notify>,
        ) -> Result<StreamResult, AdapterError> {
            panic!("use call_with_emitter");
        }

        async fn call(
            &self,
            _messages: &[crate::adapters::base::ChatMessage],
            cancel: Arc<Notify>,
        ) -> Result<StreamResult, AdapterError> {
            let idx = self
                .call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match idx {
                0 => Ok(StreamResult {
                    assistant_content: vec![
                        serde_json::json!({"type": "text", "text": "读取中"}),
                        serde_json::json!({
                            "type": "tool_use", "id": "tc-1", "name": "read_file",
                            "input": {"path": "data.txt"}
                        }),
                    ],
                    tool_calls: vec![ToolCall {
                        id: "tc-1".to_string(),
                        name: "read_file".to_string(),
                        input: serde_json::json!({"path": "data.txt"}),
                    }],
                    stop_reason: Some("tool_use".to_string()),
                }),
                1 => {
                    // Signal that we've reached the blocking point, then wait
                    // on the session's cancel token — the same token that
                    // kill_with_emitter fires via notify_one().
                    self.ready.store(true, std::sync::atomic::Ordering::SeqCst);
                    cancel.notified().await;
                    Err(AdapterError::Stream("cancelled".to_string()))
                }
                _ => Ok(StreamResult {
                    assistant_content: vec![serde_json::json!({
                        "type": "text",
                        "text": "已恢复"
                    })],
                    tool_calls: vec![],
                    stop_reason: Some("end_turn".to_string()),
                }),
            }
        }

        async fn call_with_emitter(
            &self,
            _session_id: &str,
            _messages: &[crate::adapters::base::ChatMessage],
            _emitter: &dyn EventEmitter,
            cancel: Arc<Notify>,
        ) -> Result<StreamResult, AdapterError> {
            self.call(_messages, cancel).await
        }

        fn model_id(&self) -> &str {
            "fake-cancel-model"
        }
        fn model_name(&self) -> &str {
            "Fake Cancel Model"
        }
    }

    #[tokio::test]
    async fn cancel_mid_turn_sets_cancelled_state_and_preserves_history() {
        // This test proves the cancel contract:
        //   1. Adapter returns tool_use, tool executes, loop enters round 2.
        //   2. During round 2 adapter call, session.running is set to false.
        //   3. Adapter returns cancelled, turn state is Cancelled.
        //   4. Message history preserves tool_use/tool_result pairing.
        //   5. Recovery after cancel succeeds.
        let workspace = setup_test_workspace("forge-cancel-mid-turn");
        let harness = Arc::new(Harness::new(workspace.clone()));
        std::fs::write(workspace.join("data.txt"), "cancel-test-data\n").expect("write data.txt");

        let adapter = Arc::new(CancellableFakeAdapter::new());
        let session = AgentSession::new(
            "session-cancel".to_string(),
            "deepseek".to_string(),
            adapter.clone(),
            harness,
            "你是一个编程助手".to_string(),
            Some(128_000),
        );

        let session = Arc::new(session);
        let emitter = Arc::new(crate::agent::event_sink::CollectingEventEmitter::new());

        // Spawn the turn in a background task
        let turn_guard = session.reserve_turn().expect("reserve turn");
        let session2 = session.clone();
        let emitter2 = emitter.clone();
        let handle = tokio::spawn(async move {
            session2
                .send_message_with_emitter(
                    "读取 data.txt",
                    &*emitter2,
                    vec![],
                    None,
                    None,
                    turn_guard,
                )
                .await
        });

        // Wait for the adapter to reach its second call before cancelling.
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !adapter.ready.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("adapter should reach the second model call");

        // Cancel: set running to false and fire the session's cancel token
        // (this is what the cancel IPC does — same token the adapter waits on)
        session.running.store(false, Ordering::SeqCst);
        lock_unpoisoned(&session.cancel)
            .as_ref()
            .unwrap()
            .notify_one();

        // Wait for the turn to finish
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("cancelled turn should finish")
            .expect("task should not panic");
        assert!(result.is_err(), "cancelled turn should return error");

        // Session running should be false (we set it directly)
        assert!(
            !session.running.load(Ordering::SeqCst),
            "session.running should be false after cancel"
        );

        {
            // Turn state should be cancelled
            let turn = lock_unpoisoned(&session.latest_turn);
            let turn = turn.as_ref().expect("latest turn should exist");
            assert_eq!(
                turn.status,
                AgentTurnStatus::Cancelled,
                "cancelled turn should be marked cancelled"
            );

            // Message history should have user + assistant(tool_use) + user(tool_result)
            let messages = lock_unpoisoned(&session.messages);
            assert!(
                messages.len() >= 3,
                "history should have user, assistant(tool_use), tool_result at minimum, got {}",
                messages.len()
            );

            // Verify tool_use/tool_result pairing is intact
            let assistant_msg = messages
                .iter()
                .find(|m| {
                    m.role == "assistant"
                        && m.content.as_array().is_some_and(|blocks| {
                            blocks
                                .iter()
                                .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
                        })
                })
                .expect("should have assistant with tool_use");
            let tool_use_id = assistant_msg
                .content
                .as_array()
                .unwrap()
                .iter()
                .find(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
                .and_then(|b| b.get("id"))
                .and_then(|v| v.as_str())
                .expect("tool_use id");

            let tool_result_msg = messages
                .iter()
                .find(|m| {
                    m.role == "user"
                        && m.content.as_array().is_some_and(|blocks| {
                            blocks.iter().any(|b| {
                                b.get("tool_use_id").and_then(|v| v.as_str()) == Some(tool_use_id)
                            })
                        })
                })
                .expect("should have matching tool_result");
            let result_content = tool_result_msg
                .content
                .as_array()
                .unwrap()
                .iter()
                .find(|b| b.get("tool_use_id").and_then(|v| v.as_str()) == Some(tool_use_id))
                .and_then(|b| b.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            assert!(
                result_content.contains("cancel-test-data"),
                "tool result should contain file content, got: {}",
                result_content
            );
        }

        // ── Recovery after cancel ──
        // Re-enable the session and try again
        session.running.store(true, Ordering::SeqCst);

        let emitter2 = crate::agent::event_sink::NoopEventEmitter;
        let turn_guard2 = session.reserve_turn().expect("reserve turn after cancel");
        let recovery_result = session
            .send_message_with_emitter("继续", &emitter2, vec![], None, None, turn_guard2)
            .await;

        assert!(
            recovery_result.is_ok(),
            "recovery after cancel should succeed: {:?}",
            recovery_result.err()
        );

        // Final turn should be completed
        let turn = lock_unpoisoned(&session.latest_turn);
        let turn = turn.as_ref().expect("latest turn");
        assert_eq!(
            turn.status,
            AgentTurnStatus::Completed,
            "recovery turn should be completed"
        );

        // Total adapter calls: 2 (first turn: tool_use + error) + 2 (recovery: text + final summary) = 4
        // But call 2 returns error which causes early return, so the final summary call doesn't happen.
        // So: call 0 (tool_use) + call 1 (cancel_error) + call 2 (recovery text) = 3
        // Actually, after recovery text (no tool_calls), the loop breaks, then the final summary
        // section checks if last_role is "tool" or "user" — the recovery response has text only,
        // so the last message is assistant. No final summary call needed.
        // Total: 3 calls
        assert_eq!(
            adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
            3,
            "adapter should be called 3 times: tool_use + cancel_error + recovery"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn overflow_retry_compacts_and_turn_completes() {
        // This test proves that when the adapter returns a context overflow error,
        // the agent loop triggers compaction, retries, and completes successfully.
        //
        // Scenario:
        //   1. Session has 24 pre-seeded messages (above MIN_COMPACT_MESSAGES threshold)
        //   2. FakeAdapter call 0 → context_length_exceeded error
        //   3. Overflow retry: compact_messages_for_overflow_retry reduces messages + creates summary
        //   4. FakeAdapter call 1 → final text answer (no tool calls)
        //   5. Turn completes with correct state
        //
        // Verified:
        //   - Turn status: Completed
        //   - Adapter called exactly 2 times (overflow + retry)
        //   - Messages compacted (fewer than initial)
        //   - Summary is set
        //   - ContextCompacted event emitted
        //   - overflow_retry_used prevents infinite retry

        let workspace = setup_test_workspace("forge-overflow-compact");
        let harness = Arc::new(Harness::new(workspace.clone()));
        let session_id = "session-overflow-compact".to_string();

        // Response after compaction retry: simple text, no tool calls
        let retry_response = StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text",
                "text": "已完成分析"
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        };

        let adapter = Arc::new(FakeAdapter::new_with_errors(vec![
            // Call 0: context overflow error — triggers compaction
            Err(
                "context_length_exceeded: This model's maximum context length is 128000 tokens."
                    .to_string(),
            ),
            // Call 1: success after compaction retry
            Ok(retry_response),
        ]));

        let session = AgentSession::new(
            session_id.clone(),
            "openai".to_string(), // provider type for overflow detection
            adapter.clone(),
            harness.clone(),
            "你是一个编程助手".to_string(),
            Some(128_000),
        );

        // Pre-seed 24 messages to ensure compaction has enough material.
        // compact_messages_for_overflow_retry uses OVERFLOW_RETRY_RETAIN_RECENT_MESSAGES = 16,
        // and MIN_COMPACT_MESSAGES = 8. With 24 pre-seeded + 1 user message = 25 total,
        // split_at = 25 - 16 = 9 >= 8, so compaction will proceed:
        // first 9 messages compacted into summary, last 16 retained.
        {
            let mut msgs = lock_unpoisoned(&session.messages);
            for i in 0..12 {
                msgs.push(ChatMessage::user(&format!(
                    "用户消息 {}: 请帮我分析代码结构",
                    i
                )));
                msgs.push(ChatMessage::assistant(serde_json::json!([
                    {
                        "type": "text",
                        "text": format!("助手回复 {}: 代码结构分析如下...", i)
                    }
                ])));
            }
            // After send_message_with_emitter adds the user message, total = 25
        }

        let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
        let turn_guard = session.reserve_turn().expect("reserve turn");

        let result = session
            .send_message_with_emitter(
                "请继续分析剩余文件",
                &emitter,
                vec![],
                None,
                None,
                turn_guard,
            )
            .await;

        assert!(
            result.is_ok(),
            "overflow retry turn should succeed: {:?}",
            result.err()
        );

        // 1. Turn should be completed
        {
            let turn = lock_unpoisoned(&session.latest_turn);
            let turn = turn.as_ref().expect("latest turn should exist");
            assert_eq!(
                turn.status,
                AgentTurnStatus::Completed,
                "turn should be completed after overflow retry"
            );
        }

        // 2. Messages should be compacted — fewer than the 25 we started with
        let msg_count = lock_unpoisoned(&session.messages).len();
        assert!(
            msg_count < 25,
            "messages should be compacted: expected < 25, got {}",
            msg_count
        );

        // 3. Summary should be set after compaction
        let summary = lock_unpoisoned(&session.summary).clone();
        assert!(
            summary.is_some(),
            "summary should be set after overflow compaction"
        );

        // 4. ContextCompacted event should be emitted
        let events = emitter.drain();
        let has_compacted_event = events.iter().any(|e| matches!(
            e,
            StreamEvent::ContextCompacted { session_id, .. } if session_id == "session-overflow-compact"
        ));
        assert!(
            has_compacted_event,
            "ContextCompacted event should be emitted for overflow retry, got: {:?}",
            events
                .iter()
                .map(std::mem::discriminant)
                .collect::<Vec<_>>()
        );

        // 5. Adapter should be called exactly 2 times: overflow error + retry success
        assert_eq!(
            adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "adapter should be called exactly 2 times: overflow error + retry"
        );

        // 6. Final assistant message should contain the retry response text
        let messages = lock_unpoisoned(&session.messages);
        let final_assistant = messages.iter().rev().find(|m| m.role == "assistant");
        assert!(
            final_assistant.is_some(),
            "should have a final assistant message"
        );
        let final_blocks = final_assistant
            .unwrap()
            .content
            .as_array()
            .expect("assistant content blocks");
        let final_text = final_blocks
            .iter()
            .find_map(|b| {
                if b.get("type").and_then(|v| v.as_str()) == Some("text") {
                    b.get("text").and_then(|v| v.as_str())
                } else {
                    None
                }
            })
            .expect("final text block");
        assert!(
            final_text.contains("已完成分析"),
            "final text should be from retry response, got: {}",
            final_text
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn tool_use_followed_by_overflow_retry_preserves_pairing_and_completes() {
        // This test proves the more realistic scenario where overflow happens AFTER
        // a tool round — the model first makes a tool call, the harness executes it,
        // then the second model call hits context overflow, compaction fires, and the
        // retry succeeds.
        //
        // Scenario:
        //   0. Pre-seed 24 messages (to ensure compaction threshold)
        //   1. FakeAdapter call 0 → tool_use (read_file)
        //   2. Harness executes read_file → tool_result added to history
        //   3. FakeAdapter call 1 → context_length_exceeded error
        //   4. Overflow compaction: 27 messages → compacted, tool_use/tool_result preserved
        //   5. FakeAdapter call 2 → final text answer
        //   6. Turn Completed
        //
        // Verified:
        //   - tool_use/tool_result pairing survives compaction
        //   - tool_result content is correct (contains file content)
        //   - Turn status: Completed
        //   - Adapter called exactly 3 times
        //   - ContextCompacted event emitted
        //   - Messages compacted

        let workspace = setup_test_workspace("forge-overflow-tool-combo");
        let harness = Arc::new(Harness::new(workspace.clone()));
        let session_id = "session-overflow-tool-combo".to_string();

        let tool_call_id = "call-read-overflow".to_string();

        // Response 0: model wants to read a file
        let response_tool_use = StreamResult {
            assistant_content: vec![
                serde_json::json!({
                    "type": "text",
                    "text": "让我先看源码"
                }),
                serde_json::json!({
                    "type": "tool_use",
                    "id": tool_call_id.clone(),
                    "name": "read_file",
                    "input": {"path": "src/main.rs"}
                }),
            ],
            tool_calls: vec![ToolCall {
                id: tool_call_id.clone(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "src/main.rs"}),
            }],
            stop_reason: Some("tool_use".to_string()),
        };

        // Response 1: overflow error on second model call
        let overflow_error =
            "context_length_exceeded: This model's maximum context length is 128000 tokens.";

        // Response 2: success after compaction retry
        let response_final = StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text",
                "text": "源码分析完成，这是一个 hello world 程序"
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        };

        let adapter = Arc::new(FakeAdapter::new_with_errors(vec![
            Ok(response_tool_use),
            Err(overflow_error.to_string()),
            Ok(response_final),
        ]));

        let session = AgentSession::new(
            session_id.clone(),
            "openai".to_string(),
            adapter.clone(),
            harness.clone(),
            "你是一个编程助手".to_string(),
            Some(128_000),
        );

        // Pre-seed 24 messages. After send_message adds 1 user msg, and the tool round
        // adds assistant(tool_use) + user(tool_result), total = 27 before overflow.
        // split_at = 27 - 16 = 11 >= MIN_COMPACT_MESSAGES(8) → compaction proceeds.
        // The tool_use/tool_result pair is in the last 16 messages → preserved.
        {
            let mut msgs = lock_unpoisoned(&session.messages);
            for i in 0..12 {
                msgs.push(ChatMessage::user(&format!("用户消息 {}", i)));
                msgs.push(ChatMessage::assistant(serde_json::json!([
                    { "type": "text", "text": format!("助手回复 {}", i) }
                ])));
            }
        }

        let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
        let turn_guard = session.reserve_turn().expect("reserve turn");

        let result = session
            .send_message_with_emitter(
                "帮我看看 src/main.rs 的内容",
                &emitter,
                vec![],
                None,
                None,
                turn_guard,
            )
            .await;

        assert!(
            result.is_ok(),
            "tool_use + overflow retry turn should succeed: {:?}",
            result.err()
        );

        // 1. Turn should be completed
        {
            let turn = lock_unpoisoned(&session.latest_turn);
            let turn = turn.as_ref().expect("latest turn");
            assert_eq!(
                turn.status,
                AgentTurnStatus::Completed,
                "turn should be completed after tool_use + overflow retry"
            );
        }

        // 2. Messages should be compacted
        let messages = lock_unpoisoned(&session.messages);
        let msg_count = messages.len();
        assert!(
            msg_count < 27,
            "messages should be compacted: expected < 27, got {}",
            msg_count
        );

        // 3. tool_use/tool_result pairing should survive compaction:
        //    find the assistant message with tool_use, then the next user message
        //    should have a tool_result referencing the same id.
        let tool_use_idx = messages
            .iter()
            .position(|m| {
                m.role == "assistant"
                    && m.content.as_array().is_some_and(|blocks| {
                        blocks
                            .iter()
                            .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
                    })
            })
            .expect("should have assistant message with tool_use after compaction");

        let tool_result_msg = messages
            .get(tool_use_idx + 1)
            .expect("tool_result message should follow tool_use");
        assert_eq!(
            tool_result_msg.role, "user",
            "tool_result should be in a user message"
        );
        let result_blocks = tool_result_msg
            .content
            .as_array()
            .expect("tool_result content blocks");
        assert!(
            result_blocks.iter().any(|b| {
                b.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                    && b.get("tool_use_id").and_then(|v| v.as_str()) == Some(tool_call_id.as_str())
            }),
            "tool_result should reference tool_use id '{}', got: {:?}",
            tool_call_id,
            result_blocks
        );

        // 4. Tool result should contain the file content
        let result_text = result_blocks
            .iter()
            .find_map(|b| {
                if b.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                    b.get("content").and_then(|v| v.as_str()).map(String::from)
                } else {
                    None
                }
            })
            .expect("tool_result content");
        assert!(
            result_text.contains("hello world"),
            "tool result should contain file content, got: {}",
            result_text
        );

        // 5. Summary should be set
        let summary = lock_unpoisoned(&session.summary).clone();
        assert!(summary.is_some(), "summary should be set after compaction");

        // 6. ContextCompacted event should be emitted
        let events = emitter.drain();
        let has_compacted = events.iter().any(|e| {
            matches!(
                e,
                StreamEvent::ContextCompacted { session_id, .. }
                    if session_id == "session-overflow-tool-combo"
            )
        });
        assert!(has_compacted, "ContextCompacted event should be emitted");

        // 7. Adapter should be called exactly 3 times: tool_use + overflow + retry
        assert_eq!(
            adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
            3,
            "adapter should be called 3 times: tool_use + overflow error + retry"
        );

        // 8. Final assistant message from retry
        let final_assistant = messages.iter().rev().find(|m| m.role == "assistant");
        let final_blocks = final_assistant
            .and_then(|m| m.content.as_array())
            .expect("final assistant blocks");
        let final_text = final_blocks
            .iter()
            .find_map(|b| {
                if b.get("type").and_then(|v| v.as_str()) == Some("text") {
                    b.get("text").and_then(|v| v.as_str())
                } else {
                    None
                }
            })
            .expect("final text");
        assert!(
            final_text.contains("源码分析完成"),
            "final text should be from retry response, got: {}",
            final_text
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }
    #[tokio::test]
    async fn multiple_tool_calls_followed_by_overflow_preserves_all_pairings() {
        // This test proves that when the model returns multiple tool_calls in one
        // response, all results are properly paired, and overflow compaction after
        // the tool round preserves all pairings.
        //
        // Scenario:
        //   0. Pre-seed 24 messages
        //   1. FakeAdapter call 0 → 2 tool_calls (read_file × 2)
        //   2. Harness executes both → 2 tool_results added
        //   3. FakeAdapter call 1 → context_length_exceeded
        //   4. Overflow compaction → preserves recent messages including both pairs
        //   5. FakeAdapter call 2 → final text
        //   6. Turn Completed

        let workspace = setup_test_workspace("forge-multi-tool-overflow");
        let harness = Arc::new(Harness::new(workspace.clone()));
        let session_id = "session-multi-tool-overflow".to_string();

        let tool_id_read = "call-read-multi".to_string();
        let tool_id_read2 = "call-read-multi-2".to_string();

        // Response 0: 2 tool_calls
        let response_multi_tool = StreamResult {
            assistant_content: vec![
                serde_json::json!({ "type": "text", "text": "让我同时看源码和检查编译" }),
                serde_json::json!({
                    "type": "tool_use",
                    "id": tool_id_read.clone(),
                    "name": "read_file",
                    "input": {"path": "src/main.rs"}
                }),
                serde_json::json!({
                    "type": "tool_use",
                    "id": tool_id_read2.clone(),
                    "name": "read_file",
                    "input": {"path": "src/main.rs"}
                }),
            ],
            tool_calls: vec![
                ToolCall {
                    id: tool_id_read.clone(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({"path": "src/main.rs"}),
                },
                ToolCall {
                    id: tool_id_read2.clone(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({"path": "src/main.rs"}),
                },
            ],
            stop_reason: Some("tool_use".to_string()),
        };

        let response_final = StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text",
                "text": "分析完成，编译通过"
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        };

        let adapter = Arc::new(FakeAdapter::new_with_errors(vec![
            Ok(response_multi_tool),
            Err("context_length_exceeded: too many tokens".to_string()),
            Ok(response_final),
        ]));

        let session = AgentSession::new(
            session_id.clone(),
            "openai".to_string(),
            adapter.clone(),
            harness.clone(),
            "你是一个编程助手".to_string(),
            Some(128_000),
        );

        // Pre-seed 24 messages. After send_message adds 1 user + 2 tool rounds
        // (assistant with 2 tool_uses + user with 2 tool_results) = 28 total.
        // split_at = 28 - 16 = 12 >= 8 → compaction proceeds.
        {
            let mut msgs = lock_unpoisoned(&session.messages);
            for i in 0..12 {
                msgs.push(ChatMessage::user(&format!("消息 {}", i)));
                msgs.push(ChatMessage::assistant(serde_json::json!([
                    { "type": "text", "text": format!("回复 {}", i) }
                ])));
            }
        }

        let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
        let turn_guard = session.reserve_turn().expect("reserve turn");

        let result = session
            .send_message_with_emitter(
                "读取源码并检查编译",
                &emitter,
                vec![],
                None,
                None,
                turn_guard,
            )
            .await;

        assert!(
            result.is_ok(),
            "multi-tool + overflow turn should succeed: {:?}",
            result.err()
        );

        // 1. Turn completed
        {
            let turn = lock_unpoisoned(&session.latest_turn);
            assert_eq!(turn.as_ref().unwrap().status, AgentTurnStatus::Completed);
        }

        // 2. Messages compacted
        let messages = lock_unpoisoned(&session.messages);
        assert!(
            messages.len() < 28,
            "messages should be compacted: expected < 28, got {}",
            messages.len()
        );

        // 3. Both tool_use/tool_result pairs survive compaction
        //    Find all tool_use blocks and verify each has a matching tool_result
        let tool_use_blocks: Vec<_> = messages
            .iter()
            .flat_map(|m| {
                m.content.as_array().into_iter().flat_map(|blocks| {
                    blocks.iter().filter_map(|b| {
                        if b.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                            Some((
                                m.role.clone(),
                                b.get("id").and_then(|v| v.as_str()).map(String::from),
                                b.get("name").and_then(|v| v.as_str()).map(String::from),
                            ))
                        } else {
                            None
                        }
                    })
                })
            })
            .collect();

        assert_eq!(
            tool_use_blocks.len(),
            2,
            "should have 2 tool_use blocks, got {}",
            tool_use_blocks.len()
        );

        // Verify each tool_use has a matching tool_result immediately after
        for (role, id, name) in &tool_use_blocks {
            assert_eq!(role, "assistant");
            let id = id.as_ref().expect("tool_use id");
            let name = name.as_ref().expect("tool_use name");

            // Find the tool_result for this id
            let has_matching_result = messages.iter().any(|m| {
                m.role == "user"
                    && m.content.as_array().is_some_and(|blocks| {
                        blocks.iter().any(|b| {
                            b.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                                && b.get("tool_use_id").and_then(|v| v.as_str())
                                    == Some(id.as_str())
                        })
                    })
            });
            assert!(
                has_matching_result,
                "tool_use '{}' ({}) should have matching tool_result",
                name, id
            );
        }

        // 4. Tool results contain correct content
        let tool_result_contents: Vec<String> = messages
            .iter()
            .flat_map(|m| {
                m.content.as_array().into_iter().flat_map(|blocks| {
                    blocks.iter().filter_map(|b| {
                        if b.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                            b.get("content").and_then(|v| v.as_str()).map(String::from)
                        } else {
                            None
                        }
                    })
                })
            })
            .collect();

        assert!(
            tool_result_contents
                .iter()
                .any(|c| c.contains("hello world")),
            "read_file result should contain file content"
        );

        // 5. Summary set
        assert!(lock_unpoisoned(&session.summary).is_some());

        // 6. Adapter called exactly 3 times: multi-tool + overflow + retry
        assert_eq!(
            adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
            3
        );

        // 7. Final text from retry
        let final_text = messages
            .iter()
            .rev()
            .find(|m| m.role == "assistant")
            .and_then(|m| m.content.as_array())
            .and_then(|blocks| {
                blocks.iter().find_map(|b| {
                    if b.get("type").and_then(|v| v.as_str()) == Some("text") {
                        b.get("text").and_then(|v| v.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
            })
            .expect("final text");
        assert!(final_text.contains("分析完成"));

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn snapshot_and_turn_state_bind_to_workspace_after_complete_turn() {
        // This test proves that after a complete FakeAdapter turn, the session's
        // turn state, message history, and snapshot all reflect the correct workspace.
        //
        // Verified:
        //   - Turn state is Completed
        //   - Latest turn has evidence (tool traces)
        //   - Message history has correct structure
        //   - Session snapshot serializes with correct session_id and workspace

        let workspace = setup_test_workspace("forge-snapshot-workspace");
        let harness = Arc::new(Harness::new(workspace.clone()));
        let session_id = "session-snapshot-ws".to_string();

        let tool_call_id = "call-read-snap".to_string();

        let response_tool = StreamResult {
            assistant_content: vec![
                serde_json::json!({ "type": "text", "text": "查看文件" }),
                serde_json::json!({
                    "type": "tool_use",
                    "id": tool_call_id.clone(),
                    "name": "read_file",
                    "input": {"path": "src/main.rs"}
                }),
            ],
            tool_calls: vec![ToolCall {
                id: tool_call_id.clone(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "src/main.rs"}),
            }],
            stop_reason: Some("tool_use".to_string()),
        };

        let response_final = StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text",
                "text": "文件内容已确认"
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        };

        let adapter = Arc::new(FakeAdapter::new(vec![response_tool, response_final]));
        let session = AgentSession::new(
            session_id.clone(),
            "deepseek".to_string(),
            adapter.clone(),
            harness.clone(),
            "你是一个编程助手".to_string(),
            Some(128_000),
        );

        let emitter = crate::agent::event_sink::NoopEventEmitter;
        let turn_guard = session.reserve_turn().expect("reserve turn");

        let result = session
            .send_message_with_emitter("查看 src/main.rs", &emitter, vec![], None, None, turn_guard)
            .await;

        assert!(result.is_ok(), "turn should succeed: {:?}", result.err());

        // 1. Turn state is Completed
        {
            let turn = lock_unpoisoned(&session.latest_turn);
            let turn = turn.as_ref().expect("latest turn");
            assert_eq!(turn.status, AgentTurnStatus::Completed);
        }

        // 2. Message history: user, assistant(tool_use), user(tool_result), assistant(text)
        let messages = lock_unpoisoned(&session.messages);
        assert!(
            messages.len() >= 4,
            "expected >= 4 messages, got {}",
            messages.len()
        );
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[2].role, "user");
        // Last assistant message
        let last = messages.last().unwrap();
        assert_eq!(last.role, "assistant");

        // 3. tool_result references correct tool_use id
        let result_blocks = messages[2].content.as_array().expect("tool result blocks");
        assert!(result_blocks.iter().any(|b| {
            b.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                && b.get("tool_use_id").and_then(|v| v.as_str()) == Some(tool_call_id.as_str())
        }));

        // 4. Session id is correct
        assert_eq!(session.id, "session-snapshot-ws");

        // 5. Working dir is the test workspace
        assert_eq!(session.harness.working_dir, workspace);

        // 6. Snapshot can be constructed (tests snapshot module separately, but
        //    verify the basic data needed for snapshot is available)
        let turn = lock_unpoisoned(&session.latest_turn);
        let turn = turn.as_ref().unwrap();
        // Turn should have tool evidence
        assert!(!turn.tools.is_empty(), "turn should have tool traces");

        // 7. System prompt was set
        let sp = lock_unpoisoned(&session.system_prompt);
        assert!(!sp.is_empty(), "system prompt should be set");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn kill_with_emitter_cancels_inflight_turn_and_recovery_succeeds() {
        // This test proves the production cancel/stop path end-to-end:
        //   1. Spawn send_message_with_emitter — adapter blocks on session's cancel token.
        //   2. Call kill_with_emitter from test thread — fires cancel token.
        //   3. Adapter wakes → returns error → agent loop sees running=false →
        //      marks turn Cancelled (agent loop does this, not kill_with_emitter).
        //   4. Verify: turn=Cancelled, status=Stopped, SessionStopped event emitted.
        //   5. Verify: message history and tool_use/tool_result pairing preserved.
        //   6. After resume, recovery turn succeeds.
        //
        // This mirrors IPC kill_session: the IPC handler calls session.kill() which
        // sets running=false and fires the cancel token. The agent loop (running in
        // a separate tokio task) sees the token, the adapter returns, and the loop
        // marks the turn as Cancelled.

        let workspace = setup_test_workspace("forge-kill-concurrent");
        let harness = Arc::new(Harness::new(workspace.clone()));
        std::fs::write(workspace.join("data.txt"), "kill-concurrent-data\n")
            .expect("write data.txt");

        let adapter = Arc::new(CancellableFakeAdapter::new());
        let session = Arc::new(AgentSession::new(
            "session-kill-concurrent".to_string(),
            "deepseek".to_string(),
            adapter.clone(),
            harness,
            "你是一个编程助手".to_string(),
            Some(128_000),
        ));

        let emitter = Arc::new(crate::agent::event_sink::CollectingEventEmitter::new());

        // 1. Spawn the turn — adapter will block on cancel token in call 2
        let turn_guard = session.reserve_turn().expect("reserve turn");
        let s2 = session.clone();
        let e2 = emitter.clone();
        let handle = tokio::spawn(async move {
            s2.send_message_with_emitter("读取 data.txt", &*e2, vec![], None, None, turn_guard)
                .await
        });

        // 2. Wait for adapter to reach its blocking point (call 2, waiting on cancel token)
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !adapter.ready.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("adapter should reach blocking call");

        // 3. Kill via emitter — fires cancel token, no latest_turn lock, no deadlock
        let kill_emitter = crate::agent::event_sink::CollectingEventEmitter::new();
        session.kill_with_emitter(&kill_emitter);

        // 4. Wait for the spawned turn to finish
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("killed turn should finish")
            .expect("task should not panic");
        assert!(result.is_err(), "killed turn should return error");

        // 5. Verify kill state
        assert!(!session.running.load(Ordering::SeqCst));
        assert_eq!(
            *lock_unpoisoned(&session.status),
            crate::agent::session::SessionStatus::Stopped
        );
        {
            let turn = lock_unpoisoned(&session.latest_turn);
            let turn = turn.as_ref().expect("latest turn");
            assert_eq!(
                turn.status,
                AgentTurnStatus::Cancelled,
                "agent loop should mark turn as Cancelled after kill"
            );
        }

        // 6. Verify SessionStopped event
        let kill_events = kill_emitter.drain();
        assert!(
            kill_events.iter().any(|e| matches!(
                e,
                crate::protocol::events::StreamEvent::SessionStopped { reason, .. }
                    if reason == "killed"
            )),
            "kill should emit SessionStopped, got: {:?}",
            kill_events
                .iter()
                .map(std::mem::discriminant)
                .collect::<Vec<_>>()
        );

        // 7. Verify message history preserves tool_use/tool_result pairing
        {
            let messages = lock_unpoisoned(&session.messages);
            assert!(
                messages.len() >= 3,
                "should have user + assistant(tool_use) + user(tool_result), got {}",
                messages.len()
            );
            assert_eq!(messages[0].role, "user");
            assert_eq!(messages[1].role, "assistant");
            assert_eq!(messages[2].role, "user");

            let tool_use_id = messages[1]
                .content
                .as_array()
                .and_then(|blocks| {
                    blocks.iter().find_map(|b| {
                        if b.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                            b.get("id").and_then(|v| v.as_str()).map(String::from)
                        } else {
                            None
                        }
                    })
                })
                .expect("assistant should have tool_use");
            let result_blocks = messages[2].content.as_array().expect("tool result blocks");
            assert!(
                result_blocks.iter().any(|b| {
                    b.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                        && b.get("tool_use_id").and_then(|v| v.as_str())
                            == Some(tool_use_id.as_str())
                }),
                "tool_result should reference tool_use id"
            );
        }

        // 8. Recovery — resume and send new turn
        let recovery_emitter = crate::agent::event_sink::CollectingEventEmitter::new();
        session.running.store(true, Ordering::SeqCst);
        *lock_unpoisoned(&session.status) = crate::agent::session::SessionStatus::Running;
        *lock_unpoisoned(&session.cancel) = Some(Arc::new(tokio::sync::Notify::new()));

        let turn_guard2 = session.reserve_turn().expect("reserve recovery turn");
        let recovery_result = session
            .send_message_with_emitter("继续", &recovery_emitter, vec![], None, None, turn_guard2)
            .await;
        assert!(
            recovery_result.is_ok(),
            "recovery should succeed: {:?}",
            recovery_result.err()
        );
        {
            let turn = lock_unpoisoned(&session.latest_turn);
            assert_eq!(
                turn.as_ref().unwrap().status,
                AgentTurnStatus::Completed,
                "recovery turn should be Completed"
            );
        }

        let _ = std::fs::remove_dir_all(&workspace);
    }
}
