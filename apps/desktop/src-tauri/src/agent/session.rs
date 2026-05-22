use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc, Mutex};

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
use crate::agent::snapshot::AgentSessionSnapshot;
use crate::agent::time::now_ms;
use crate::agent::tool_results::{
    push_assistant_result_with_synthetic_tool_results, repair_tool_use_adjacency,
    resolve_tool_result_for_model,
};
use crate::agent::turn_outcome::{
    final_answer_instruction, final_turn_status_for_run, final_turn_transition_reason_for_run,
    verification_has_failed,
};
use crate::agent::turn_state::{
    completed_tool_trace, running_tool_trace, AgentCompactTrace, AgentFailureTrace,
    AgentTurnMetadata, AgentTurnState, AgentTurnStatus, AgentVerificationStatus,
    AgentVerificationTrace,
};
use crate::agent::verification;
use crate::harness::Harness;
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;

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
    pub(crate) adapter: Arc<Box<dyn AiAdapter>>,
    pub(crate) messages: Arc<Mutex<Vec<ChatMessage>>>,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) harness: Arc<Harness>,
    pub(crate) system_prompt: Mutex<String>,
    pub(crate) summary: Mutex<Option<String>>,
    pub(crate) latest_turn: Mutex<Option<AgentTurnState>>,
    pub(crate) auto_compact_guard: Mutex<AutoCompactGuard>,
    pub(crate) context_window_tokens: Option<u32>,
    pub(crate) cancel: Mutex<Option<Arc<Notify>>>,
}

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
        adapter: Arc<Box<dyn AiAdapter>>,
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
            harness,
            system_prompt: Mutex::new(system_prompt),
            summary: Mutex::new(None),
            latest_turn: Mutex::new(None),
            auto_compact_guard: Mutex::new(AutoCompactGuard::default()),
            context_window_tokens,
            cancel: Mutex::new(None),
        };

        *session.status.lock().unwrap() = SessionStatus::Running;
        session
    }

    pub fn set_system_prompt(&self, prompt: String) {
        *self.system_prompt.lock().unwrap() = prompt;
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
        *self.messages.lock().unwrap() = repair_tool_use_adjacency(messages);
        *self.summary.lock().unwrap() = summary;
        *self.latest_turn.lock().unwrap() = latest_turn.map(|mut turn| {
            turn.normalize_for_session_resume();
            turn
        });
    }

    pub fn snapshot(&self) -> AgentSessionSnapshot {
        let snapshot = AgentSessionSnapshot::new(
            self.id.clone(),
            self.agent_type.clone(),
            self.model_id.clone(),
            self.harness.working_dir.to_string_lossy().to_string(),
            self.messages.lock().unwrap().clone(),
            self.summary.lock().unwrap().clone(),
            self.context_window_tokens,
        );
        if let Some(latest_turn) = self.latest_turn.lock().unwrap().clone() {
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
        if !self.running.load(Ordering::SeqCst) {
            return Err("Session is not running".to_string());
        }

        let previous_turn = self.latest_turn.lock().unwrap().clone();
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
            self.messages.lock().unwrap().len()
        );
        let turn_system_prompt = self
            .harness
            .build_system_prompt_for_request(
                &self.agent_type,
                &self.harness.working_dir,
                Some(activation_text.unwrap_or(text)),
            )
            .await;
        *self.system_prompt.lock().unwrap() = turn_system_prompt;
        self.adapter
            .set_external_tools(self.harness.external_mcp_tool_definitions().await);

        // Add user message to history
        self.messages.lock().unwrap().push(ChatMessage::user(text));
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
        *self.cancel.lock().unwrap() = Some(cancel.clone());

        let mut overflow_retry_used = false;

        // Agent loop: up to 10 tool-call round-trips with final text summary fallback
        for _round in 0..10 {
            if !self.running.load(Ordering::SeqCst) {
                break;
            }

            let all_messages = self.messages.lock().unwrap().clone();
            let existing_summary = self.summary.lock().unwrap().clone();
            let compacted = if self
                .auto_compact_guard
                .lock()
                .unwrap()
                .should_skip_proactive_compaction()
            {
                self.auto_compact_guard
                    .lock()
                    .unwrap()
                    .record_proactive_skip();
                CompactResult::unchanged(all_messages, existing_summary)
            } else {
                compact_messages_if_needed(
                    all_messages,
                    existing_summary,
                    self.context_window_tokens,
                )
            };
            self.auto_compact_guard
                .lock()
                .unwrap()
                .record_result(&compacted);

            if let Some(stats) = compacted.stats.as_ref() {
                self.apply_compaction(&compacted, stats, "auto_compact", app_handle);
            }

            let sp = self.system_prompt.lock().unwrap().clone();
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
                            let all_messages = self.messages.lock().unwrap().clone();
                            let existing_summary = self.summary.lock().unwrap().clone();
                            let compacted =
                                compact_messages_for_overflow_retry(all_messages, existing_summary);
                            self.auto_compact_guard
                                .lock()
                                .unwrap()
                                .record_result(&compacted);

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

                        if retries < 2
                            && (msg.contains("500")
                                || msg.contains("503")
                                || msg.contains("429")
                                || msg.contains("timed out"))
                        {
                            retries += 1;
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            continue;
                        }
                        let err_msg = format!("API error: {}", msg);
                        // Don't stop the session — let user retry
                        crate::transcript::emit_stream_event(
                            app_handle,
                            StreamEvent::Error {
                                session_id: self.id.clone(),
                                block_id: BlockId::new().to_string(),
                                message: err_msg.clone(),
                                code: "api_error".to_string(),
                            },
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
                self.messages.lock().unwrap().push(ChatMessage::assistant(
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
                    self.record_latest_tool(
                        running_tool_trace(tc.id.clone(), tc.name.clone(), &tc.input, now_ms()),
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
                    let cancel = self
                        .cancel
                        .lock()
                        .unwrap()
                        .clone()
                        .unwrap_or_else(|| Arc::new(Notify::new()));
                    let idx = result
                        .tool_calls
                        .iter()
                        .position(|t| t.id == tc.id)
                        .unwrap_or(0);
                    let wd = self.harness.working_dir.clone();
                    handles.push(tokio::spawn(async move {
                        let r = crate::agent::sub::SubAgent::run(
                            &task, adapter, harness, &app, cancel, &wd,
                        )
                        .await;
                        (idx, r)
                    }));
                }
                for handle in handles {
                    if let Ok((idx, r)) = handle.await {
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
                        if let Some(tc) = result.tool_calls.get(idx) {
                            crate::transcript::emit_stream_event(
                                app_handle,
                                StreamEvent::ToolCallResult {
                                    session_id: self.id.clone(),
                                    block_id: tc.id.clone(),
                                    result: r.clone(),
                                    is_error: false,
                                    duration_ms: 0,
                                },
                            );
                            self.record_latest_tool(
                                completed_tool_trace(
                                    tc.id.clone(),
                                    tc.name.clone(),
                                    &tc.input,
                                    &r,
                                    now_ms(),
                                    now_ms(),
                                ),
                                app_handle,
                            );
                        }
                        // Feed only the result text to the main agent
                        sub_results.push((idx, api_text));
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
            let mut tool_results: Vec<serde_json::Value> = Vec::new();
            for tc in &result.tool_calls {
                let resolution = resolve_tool_result_for_model(&result_map, tc);
                if resolution.missing {
                    self.record_latest_tool(
                        completed_tool_trace(
                            tc.id.clone(),
                            tc.name.clone(),
                            &tc.input,
                            &resolution.content,
                            now_ms(),
                            now_ms(),
                        ),
                        app_handle,
                    );
                }
                let exec_result = resolution.content;
                crate::app_log!(
                    "INFO",
                    "Agent tool '{}' result ({} chars)",
                    tc.name,
                    exec_result.len()
                );
                tool_results.push(serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": &tc.id,
                    "content": exec_result,
                }));
            }
            self.messages.lock().unwrap().push(ChatMessage {
                role: "user".to_string(),
                content: serde_json::Value::Array(tool_results),
            });

            // Yield briefly so frontend receives & renders events before next API call
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        let verification_trace = if self.running.load(Ordering::SeqCst) {
            self.verify_latest_turn(app_handle).await
        } else {
            None
        };

        // Ensure final text response: append instruction, call API one more time if needed
        if self.running.load(Ordering::SeqCst) {
            let messages = self.messages.lock().unwrap().clone();
            let summary = self.summary.lock().unwrap().clone();
            let sp = self.system_prompt.lock().unwrap().clone();
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
                        let mut messages = self.messages.lock().unwrap();
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
        let final_reason = final_turn_transition_reason_for_run(
            self.running.load(Ordering::SeqCst),
            verification_trace.as_ref(),
        );
        self.mark_latest_turn_status_with_reason(
            final_turn_status_for_run(
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
        *self.status.lock().unwrap() = SessionStatus::Stopped;
        self.mark_latest_turn_status_with_reason(
            AgentTurnStatus::Cancelled,
            "user_cancelled",
            Some("session killed"),
            app_handle,
        );
        // Cancel in-flight HTTP stream
        if let Some(cancel) = self.cancel.lock().unwrap().take() {
            cancel.notify_waiters();
        }
        crate::transcript::emit_stream_event(
            app_handle,
            StreamEvent::SessionStopped {
                session_id: self.id.clone(),
                reason: "killed".to_string(),
            },
        );
    }

    pub fn resume(&self, app_handle: &tauri::AppHandle) {
        self.running.store(true, Ordering::SeqCst);
        *self.status.lock().unwrap() = SessionStatus::Running;
        if let Some(turn) = self.latest_turn.lock().unwrap().as_mut() {
            turn.normalize_for_session_resume();
        }
        self.emit_latest_turn_projection(app_handle);
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
        *self.latest_turn.lock().unwrap() = Some(turn);
        self.emit_latest_turn_projection(app_handle);
    }

    fn repair_message_history(&self, reason: &str) {
        let mut messages = self.messages.lock().unwrap();
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
        if let Some(turn) = self.latest_turn.lock().unwrap().as_mut() {
            turn.mark_status_with_reason(status, reason, detail);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    fn record_latest_turn_failure(&self, trace: AgentFailureTrace, app_handle: &tauri::AppHandle) {
        if let Some(turn) = self.latest_turn.lock().unwrap().as_mut() {
            turn.record_failure(trace);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    fn record_latest_tool(
        &self,
        trace: crate::agent::turn_state::AgentToolTrace,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(turn) = self.latest_turn.lock().unwrap().as_mut() {
            turn.record_tool(trace);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    fn record_latest_compact(&self, trace: AgentCompactTrace, app_handle: &tauri::AppHandle) {
        if let Some(turn) = self.latest_turn.lock().unwrap().as_mut() {
            turn.record_compact(trace);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    fn record_latest_verification(
        &self,
        trace: AgentVerificationTrace,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(turn) = self.latest_turn.lock().unwrap().as_mut() {
            turn.set_verification(trace);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    pub fn record_latest_delivery_summary(
        &self,
        summary: &crate::protocol::events::DeliverySummary,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(turn) = self.latest_turn.lock().unwrap().as_mut() {
            turn.record_delivery_summary(summary);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    pub fn record_latest_preview_status(
        &self,
        update: AgentPreviewStatusUpdate<'_>,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(turn) = self.latest_turn.lock().unwrap().as_mut() {
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
        if let Some(turn) = self.latest_turn.lock().unwrap().as_mut() {
            turn.record_checkpoint_status(is_git_repo, dirty, has_checkpoint, label);
        }
        self.emit_latest_turn_projection(app_handle);
    }

    async fn verify_latest_turn(
        &self,
        app_handle: &tauri::AppHandle,
    ) -> Option<AgentVerificationTrace> {
        let turn = self.latest_turn.lock().unwrap().clone()?;

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
        let trace = verification::run_verification(plan).await;
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
        *self.summary.lock().unwrap() = compacted.summary.clone();
        *self.messages.lock().unwrap() = compacted.messages.clone();
        crate::transcript::emit_stream_event(
            app_handle,
            StreamEvent::ContextCompacted {
                session_id: self.id.clone(),
                block_id: BlockId::new().to_string(),
                summary: stats.summary.clone(),
                retained_messages: stats.retained_messages,
                compacted_messages: stats.compacted_messages,
                estimated_tokens_before: stats.estimated_tokens_before,
                estimated_tokens_after: stats.estimated_tokens_after,
            },
        );
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
        if let Some(turn) = self.latest_turn.lock().unwrap().as_mut() {
            turn.set_context(bundle.to_turn_context_snapshot());
        }
        self.emit_latest_turn_projection(app_handle);
    }

    pub fn emit_latest_turn_projection(&self, app_handle: &tauri::AppHandle) {
        let projection = self
            .latest_turn
            .lock()
            .unwrap()
            .as_ref()
            .map(AgentTurnState::to_projection);

        if let Some(state) = projection {
            crate::transcript::emit_stream_event(
                app_handle,
                StreamEvent::AgentTurnUpdated {
                    session_id: self.id.clone(),
                    state,
                },
            );
        }
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

fn is_read_only_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "read"
            | "list_directory"
            | "ls"
            | "list"
            | "search_files"
            | "glob"
            | "search_content"
            | "grep"
            | "web_search"
            | "web_fetch"
            | "git_diff"
    )
}
