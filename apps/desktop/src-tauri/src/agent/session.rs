use parking_lot::Mutex;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};

use tokio::sync::Notify;

use crate::adapters::base::{AiAdapter, ChatMessage};
use crate::agent::auto_compact::{
    finalize_compaction_plan, finalize_compaction_plan_with_heuristic_summary,
    prepare_compaction_for_overflow_retry, prepare_compaction_if_needed, prepare_compaction_now,
    AutoCompactGuard, CompactPlan, CompactResult, CompactStats,
};
use crate::agent::compact_summary::{
    compact_summary_prompt_messages, extract_compact_summary_text,
};
use crate::agent::context_builder::{
    ContextBuilder, ContextBundle, ContextSourceKind, HiddenContextPart,
};
use crate::agent::event_sink::EventEmitter;
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
use crate::agent::turn_metrics::{TurnMetrics, TurnMetricsEventEmitter, TurnUsageSnapshot};
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
use crate::protocol::BlockId;

const MAX_AGENT_TOOL_ROUNDS: usize = 80;
pub use crate::agent::manual_compact::ManualCompactResult;

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
    pub(crate) turn_metrics: Arc<Mutex<TurnMetrics>>,
    pub(crate) auto_compact_guard: Mutex<AutoCompactGuard>,
    pub(crate) context_window_tokens: Option<u32>,
    pub(crate) cancel: Mutex<Option<Arc<Notify>>>,
}

// Lock discipline for AgentSession:
// - Prefer taking one mutex per statement and cloning the small value needed.
// - If multiple locks are unavoidable, acquire them in this order:
//   status -> system_prompt -> messages -> summary -> latest_turn -> turn_metrics
//   -> auto_compact_guard -> cancel.
// This keeps resume/snapshot/turn setup from growing accidental lock-order cycles.

pub(crate) struct AgentPreviewStatusUpdate<'a> {
    pub project_path: Option<&'a str>,
    pub running: bool,
    pub can_start: bool,
    pub can_open: bool,
    pub label: &'a str,
    pub url: Option<&'a str>,
}

/// Outcome of a single tool-round iteration inside the agent loop.
pub(crate) enum RoundDecision {
    /// Stop the outer loop (turn completed or cancelled).
    Break,
    /// Continue to the next round.
    Continue,
}

pub(crate) struct AgentTurnRunRequest<'a> {
    pub text: &'a str,
    pub hidden_contexts: Vec<HiddenContextPart>,
    pub turn_metadata: Option<AgentTurnMetadata>,
    pub activation_text: Option<&'a str>,
    pub _turn_guard: TurnInflightGuard,
    pub emitter: &'a dyn EventEmitter,
    pub tool_emitter: Option<Arc<dyn EventEmitter>>,
    pub app_handle: Option<&'a tauri::AppHandle>,
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
            turn_metrics: Arc::new(Mutex::new(TurnMetrics::default())),
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
        let emitter = crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone());
        self.run_agent_turn(AgentTurnRunRequest {
            text,
            hidden_contexts,
            turn_metadata,
            activation_text,
            _turn_guard,
            emitter: &emitter,
            tool_emitter: None,
            app_handle: Some(app_handle),
        })
        .await
    }

    /// Core agent turn loop — unified implementation used by both production
    /// Tauri events and test/headless emitters.
    /// Phase 1 — set up the turn: recovery context, system prompt, user message,
    /// cancel token, and initial status.
    async fn setup_turn(
        &self,
        text: &str,
        mut hidden_contexts: Vec<HiddenContextPart>,
        turn_metadata: Option<AgentTurnMetadata>,
        activation_text: Option<&str>,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
    ) -> Vec<HiddenContextPart> {
        let previous_turn = lock_unpoisoned(&self.latest_turn).clone();
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
        hidden_contexts
    }

    /// Phase 2 — one iteration of compaction → model call → tool execution.
    /// Returns `Break` when the outer loop should stop (no tool calls or cancelled).
    async fn execute_single_round(
        &self,
        hidden_contexts: &[HiddenContextPart],
        cancel: Arc<Notify>,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
        app_handle: Option<&tauri::AppHandle>,
        tool_emitter: Option<Arc<dyn crate::agent::event_sink::EventEmitter>>,
        overflow_retry_used: &mut bool,
    ) -> Result<RoundDecision, String> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(RoundDecision::Break);
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
            match prepare_compaction_if_needed(
                all_messages,
                existing_summary,
                self.context_window_tokens,
            ) {
                Ok(plan) => match self
                    .compact_plan_with_summary(&plan, cancel.clone(), false)
                    .await
                {
                    Ok(compacted) => compacted,
                    Err(err)
                        if compact_summary_was_cancelled(&err)
                            || !self.running.load(Ordering::SeqCst) =>
                    {
                        self.mark_latest_turn_status_with_reason_emitter(
                            AgentTurnStatus::Cancelled,
                            "user_cancelled",
                            Some("cancelled during auto compact summary"),
                            emitter,
                        );
                        return Err("Cancelled".to_string());
                    }
                    Err(err) => {
                        crate::app_log!(
                            "WARN",
                            "Auto compact summary failed for session {}: {}",
                            self.id,
                            err
                        );
                        CompactResult::skipped(
                            plan.original_messages.clone(),
                            plan.existing_summary.clone(),
                            format!("model_summary_failed: {err}"),
                        )
                    }
                },
                Err(result) => *result,
            }
        };
        lock_unpoisoned(&self.auto_compact_guard).record_result(&compacted);

        if let Some(stats) = compacted.stats.as_ref() {
            self.apply_compaction_emitter(&compacted, stats, "auto_compact", emitter);
        }

        let sp = lock_unpoisoned(&self.system_prompt).clone();
        crate::app_log!(
            "INFO",
            "[send_message] system_prompt length: {} chars, has 'Active Skills': {}",
            sp.len(),
            sp.contains("Active Skills")
        );
        let context_bundle = Self::build_context_bundle(
            compacted.messages,
            compacted.summary,
            hidden_contexts.to_vec(),
            sp.clone(),
            self.context_window_tokens,
        );
        self.record_latest_context_emitter(&context_bundle, emitter);
        self.record_context_metrics(&context_bundle);
        let mut msgs_with_context = repair_tool_use_adjacency(context_bundle.messages);

        self.mark_latest_turn_status_with_reason_emitter(
            AgentTurnStatus::CallingModel,
            "call_model",
            None,
            emitter,
        );
        let mut retries = 0;
        let metrics_emitter = TurnMetricsEventEmitter::new(emitter, self.turn_metrics.clone());
        let result = loop {
            let adapter_result = self
                .adapter
                .stream_message_with_emitter(
                    &self.id,
                    &msgs_with_context,
                    &metrics_emitter,
                    cancel.clone(),
                )
                .await;
            match adapter_result {
                Ok(r) => break r,
                Err(e) => {
                    let msg = e.to_string();
                    if !*overflow_retry_used && is_context_overflow_error(&self.agent_type, &msg) {
                        let all_messages = lock_unpoisoned(&self.messages).clone();
                        let existing_summary = lock_unpoisoned(&self.summary).clone();
                        let compacted = match prepare_compaction_for_overflow_retry(
                            all_messages,
                            existing_summary,
                        ) {
                            Ok(plan) => match self
                                .compact_plan_with_summary(&plan, cancel.clone(), true)
                                .await
                            {
                                Ok(compacted) => compacted,
                                Err(err)
                                    if compact_summary_was_cancelled(&err)
                                        || !self.running.load(Ordering::SeqCst) =>
                                {
                                    self.mark_latest_turn_status_with_reason_emitter(
                                        AgentTurnStatus::Cancelled,
                                        "user_cancelled",
                                        Some("cancelled during overflow compact summary"),
                                        emitter,
                                    );
                                    return Err("Cancelled".to_string());
                                }
                                Err(err) => {
                                    crate::app_log!(
                                        "WARN",
                                        "Overflow compact summary failed for session {}: {}",
                                        self.id,
                                        err
                                    );
                                    CompactResult::skipped(
                                        plan.original_messages.clone(),
                                        plan.existing_summary.clone(),
                                        format!("model_summary_failed: {err}"),
                                    )
                                }
                            },
                            Err(result) => *result,
                        };
                        lock_unpoisoned(&self.auto_compact_guard).record_result(&compacted);

                        if let Some(stats) = compacted.stats.as_ref() {
                            *overflow_retry_used = true;
                            self.apply_compaction_emitter(
                                &compacted,
                                stats,
                                "overflow_retry",
                                emitter,
                            );

                            let context_bundle = Self::build_context_bundle(
                                compacted.messages,
                                compacted.summary,
                                hidden_contexts.to_vec(),
                                sp.clone(),
                                self.context_window_tokens,
                            );
                            self.record_latest_context_emitter(&context_bundle, emitter);
                            self.record_context_metrics(&context_bundle);
                            msgs_with_context = repair_tool_use_adjacency(context_bundle.messages);
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
            return Ok(RoundDecision::Break);
        }

        if !result.assistant_content.is_empty() {
            lock_unpoisoned(&self.messages).push(ChatMessage::assistant(serde_json::Value::Array(
                result.assistant_content.clone(),
            )));
        }

        if result.tool_calls.is_empty() {
            self.mark_latest_turn_status_with_reason_emitter(
                AgentTurnStatus::Completed,
                "final_answer",
                Some("model returned no tool calls"),
                emitter,
            );
            return Ok(RoundDecision::Break);
        }

        self.mark_latest_turn_status_with_reason_emitter(
            AgentTurnStatus::RunningTools,
            "tool_calls_requested",
            Some("model requested tool execution"),
            emitter,
        );

        crate::app_log!(
            "INFO",
            "Agent turn: {} tool calls to execute: {:?}",
            result.tool_calls.len(),
            result
                .tool_calls
                .iter()
                .map(|tc| tc.name.clone())
                .collect::<Vec<_>>()
        );

        self.execute_tools(
            &result.tool_calls,
            emitter,
            app_handle,
            tool_emitter,
            cancel,
        )
        .await;

        tokio::time::sleep(AGENT_LOOP_SETTLE_DELAY).await;
        Ok(RoundDecision::Continue)
    }

    /// Execute a batch of tool calls: sub-agents, read tools in parallel, write tools sequentially.
    async fn execute_tools(
        &self,
        tool_calls: &[crate::adapters::base::ToolCall],
        emitter: &dyn crate::agent::event_sink::EventEmitter,
        app_handle: Option<&tauri::AppHandle>,
        tool_emitter_override: Option<Arc<dyn crate::agent::event_sink::EventEmitter>>,
        cancel: Arc<Notify>,
    ) {
        let (delegated, regular): (Vec<_>, Vec<_>) =
            tool_calls.iter().partition(|tc| tc.name == "delegate_task");

        let mut sub_results: Vec<(usize, String)> = Vec::new();
        if !delegated.is_empty() {
            let mut handles = Vec::new();
            for tc in &delegated {
                let started_at_ms = now_ms();
                self.record_latest_tool_emitter(
                    running_tool_trace(tc.id.clone(), tc.name.clone(), &tc.input, started_at_ms),
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
                let idx = tool_calls.iter().position(|t| t.id == tc.id).unwrap_or(0);
                let wd = self.harness.working_dir.clone();
                let sub_emitter: Arc<dyn crate::agent::event_sink::EventEmitter> =
                    Arc::new(crate::agent::event_sink::NoopEventEmitter);
                handles.push((
                    idx,
                    tc.id.clone(),
                    tc.name.clone(),
                    tc.input.clone(),
                    started_at_ms,
                    tokio::spawn(async move {
                        let r = crate::agent::sub::SubAgent::run_with_emitter(
                            &task,
                            adapter,
                            harness,
                            &*sub_emitter,
                            cancel,
                            &wd,
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
                    if let Some(app) = app_handle {
                        Arc::new(crate::agent::event_sink::TauriEventEmitter::new(
                            app.clone(),
                        ))
                    } else if let Some(shared) = tool_emitter_override.clone() {
                        shared
                    } else {
                        Arc::new(crate::agent::event_sink::NoopEventEmitter)
                    };
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
                if let Ok((id, name, input, started_at_ms, ended_at_ms, result)) = handle.await {
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
            let tool_emitter: Arc<dyn crate::agent::event_sink::EventEmitter> =
                if let Some(app) = app_handle {
                    Arc::new(crate::agent::event_sink::TauriEventEmitter::new(
                        app.clone(),
                    ))
                } else if let Some(shared) = tool_emitter_override.clone() {
                    shared
                } else {
                    Arc::new(crate::agent::event_sink::NoopEventEmitter)
                };
            let result = self
                .harness
                .execute_tool_with_emitter(
                    &self.id,
                    &tc.name,
                    &tc.input,
                    tool_emitter,
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
            if let Some(tc) = tool_calls.get(idx) {
                result_map.insert(tc.id.clone(), r);
            }
        }

        let model_tool_results = build_tool_result_message_for_model(&result_map, tool_calls);
        for resolved in &model_tool_results.results {
            if resolved.missing {
                let Some(tc) = tool_calls.iter().find(|tc| tc.id == resolved.tool_call_id) else {
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
    }

    /// Phase 3 — verification, final summary, and status transition.
    async fn finalize_turn(
        &self,
        hidden_contexts: &[HiddenContextPart],
        emitter: &dyn crate::agent::event_sink::EventEmitter,
        _app_handle: Option<&tauri::AppHandle>,
        cancel: Arc<Notify>,
    ) {
        let verification_trace = if self.running.load(Ordering::SeqCst) {
            self.verify_latest_turn_emitter(emitter).await
        } else {
            None
        };

        if self.running.load(Ordering::SeqCst) {
            let messages = lock_unpoisoned(&self.messages).clone();
            let summary = lock_unpoisoned(&self.summary).clone();
            let sp = lock_unpoisoned(&self.system_prompt).clone();
            let context_bundle = Self::build_context_bundle(
                messages,
                summary,
                hidden_contexts.to_vec(),
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
                let adapter_result = self.adapter.call(&msgs, cancel.clone()).await;
                if let Ok(result) = adapter_result {
                    if !result.assistant_content.is_empty() {
                        self.emit_final_summary_text_emitter(&result.assistant_content, emitter);
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
    }

    pub(crate) async fn run_agent_turn(
        &self,
        request: AgentTurnRunRequest<'_>,
    ) -> Result<(), String> {
        if !self.running.load(Ordering::SeqCst) {
            return Err("Session is not running".to_string());
        }

        let hidden_contexts = self
            .setup_turn(
                request.text,
                request.hidden_contexts,
                request.turn_metadata,
                request.activation_text,
                request.emitter,
            )
            .await;

        let cancel = Arc::new(Notify::new());
        *lock_unpoisoned(&self.cancel) = Some(cancel.clone());
        let _cancel_guard = ActiveCancelGuard::new(&self.cancel, cancel.clone());

        let mut overflow_retry_used = false;

        for _round in 0..MAX_AGENT_TOOL_ROUNDS {
            match self
                .execute_single_round(
                    &hidden_contexts,
                    cancel.clone(),
                    request.emitter,
                    request.app_handle,
                    request.tool_emitter.clone(),
                    &mut overflow_retry_used,
                )
                .await?
            {
                RoundDecision::Break => break,
                RoundDecision::Continue => continue,
            }
        }

        self.finalize_turn(
            &hidden_contexts,
            request.emitter,
            request.app_handle,
            cancel,
        )
        .await;
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
        self.start_turn_with_emitter(
            text,
            metadata,
            &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
        );
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
        self.mark_latest_turn_status_with_reason_emitter(
            status,
            reason,
            detail,
            &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
        );
    }

    fn record_latest_turn_failure(&self, trace: AgentFailureTrace, app_handle: &tauri::AppHandle) {
        self.record_latest_turn_failure_emitter(
            trace,
            &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
        );
    }

    fn record_latest_tool(
        &self,
        trace: crate::agent::turn_state::AgentToolTrace,
        app_handle: &tauri::AppHandle,
    ) {
        self.record_latest_tool_emitter(
            trace,
            &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
        );
    }

    fn record_latest_compact(&self, trace: AgentCompactTrace, app_handle: &tauri::AppHandle) {
        self.record_latest_compact_emitter(
            trace,
            &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
        );
    }

    fn record_latest_verification(
        &self,
        trace: AgentVerificationTrace,
        app_handle: &tauri::AppHandle,
    ) {
        self.record_latest_verification_emitter(
            trace,
            &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
        );
    }

    pub fn record_latest_delivery_summary(
        &self,
        summary: &crate::protocol::events::DeliverySummary,
        app_handle: &tauri::AppHandle,
    ) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.record_delivery_summary(summary);
        }
        self.emit_with_emitter(&crate::agent::event_sink::TauriEventEmitter::new(
            app_handle.clone(),
        ));
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
        self.emit_with_emitter(&crate::agent::event_sink::TauriEventEmitter::new(
            app_handle.clone(),
        ));
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
        self.emit_with_emitter(&crate::agent::event_sink::TauriEventEmitter::new(
            app_handle.clone(),
        ));
    }

    async fn verify_latest_turn(
        &self,
        app_handle: &tauri::AppHandle,
    ) -> Option<AgentVerificationTrace> {
        self.verify_latest_turn_emitter(&crate::agent::event_sink::TauriEventEmitter::new(
            app_handle.clone(),
        ))
        .await
    }

    fn apply_compaction(
        &self,
        compacted: &CompactResult,
        stats: &CompactStats,
        reason: &str,
        app_handle: &tauri::AppHandle,
    ) {
        self.apply_compaction_emitter(
            compacted,
            stats,
            reason,
            &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
        );
    }

    fn record_latest_context(&self, bundle: &ContextBundle, app_handle: &tauri::AppHandle) {
        self.record_latest_context_emitter(
            bundle,
            &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
        );
    }

    pub fn emit_latest_turn_projection(&self, app_handle: &tauri::AppHandle) {
        self.emit_with_emitter(&crate::agent::event_sink::TauriEventEmitter::new(
            app_handle.clone(),
        ));
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

    pub(crate) fn context_compact_skipped_event(
        &self,
        reason: &str,
        retained_messages: usize,
    ) -> StreamEvent {
        session_events::context_compact_skipped_event(&self.id, reason, retained_messages)
    }

    async fn compact_plan_with_summary(
        &self,
        plan: &CompactPlan,
        cancel: Arc<Notify>,
        fallback_on_model_error: bool,
    ) -> Result<CompactResult, String> {
        if self.adapter.is_missing_api_key_adapter() {
            return Ok(finalize_compaction_plan_with_heuristic_summary(
                plan.clone(),
            ));
        }

        match self.generate_model_compact_summary(plan, cancel).await {
            Ok(summary) => Ok(finalize_compaction_plan(plan.clone(), summary)),
            Err(err) if compact_summary_was_cancelled(&err) => Err(err),
            Err(err) if fallback_on_model_error => {
                crate::app_log!(
                    "WARN",
                    "Falling back to heuristic compact summary for session {}: {}",
                    self.id,
                    err
                );
                Ok(finalize_compaction_plan_with_heuristic_summary(
                    plan.clone(),
                ))
            }
            Err(err) => Err(err),
        }
    }

    async fn generate_model_compact_summary(
        &self,
        plan: &CompactPlan,
        cancel: Arc<Notify>,
    ) -> Result<String, String> {
        let messages = compact_summary_prompt_messages(plan, self.context_window_tokens);
        let result = self
            .adapter
            .compact_summary(&messages, cancel)
            .await
            .map_err(|err| err.to_string())?;
        extract_compact_summary_text(&result)
    }

    pub(crate) async fn compact_now_with_emitter(
        &self,
        emitter: &dyn EventEmitter,
    ) -> Result<ManualCompactResult, String> {
        let all_messages = lock_unpoisoned(&self.messages).clone();
        let existing_summary = lock_unpoisoned(&self.summary).clone();
        let compacted = match prepare_compaction_now(all_messages, existing_summary) {
            Ok(plan) => {
                self.compact_plan_with_summary(&plan, Arc::new(Notify::new()), false)
                    .await?
            }
            Err(result) => *result,
        };

        if let Some(stats) = compacted.stats.as_ref() {
            lock_unpoisoned(&self.auto_compact_guard).record_result(&compacted);
            self.apply_compaction_emitter(&compacted, stats, "manual_compact", emitter);
            return Ok(ManualCompactResult {
                compacted: true,
                skipped_reason: None,
                retained_messages: stats.retained_messages,
                compacted_messages: stats.compacted_messages,
                estimated_tokens_before: stats.estimated_tokens_before,
                estimated_tokens_after: stats.estimated_tokens_after,
            });
        }

        let retained_messages = compacted.messages.len();
        let skipped_reason = compacted.skipped_reason.clone();
        if let Some(reason) = skipped_reason.as_deref() {
            emitter.emit(self.context_compact_skipped_event(reason, retained_messages));
        }

        Ok(ManualCompactResult {
            compacted: false,
            skipped_reason,
            retained_messages,
            compacted_messages: 0,
            estimated_tokens_before: 0,
            estimated_tokens_after: 0,
        })
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

    fn emit_final_summary_text_emitter(
        &self,
        assistant_content: &[serde_json::Value],
        emitter: &dyn crate::agent::event_sink::EventEmitter,
    ) {
        let text = final_summary_text(assistant_content);
        if text.trim().is_empty() {
            return;
        }

        let block_id = BlockId::new().to_string();
        emitter.emit(StreamEvent::TextStart {
            session_id: self.id.clone(),
            block_id: block_id.clone(),
        });
        emitter.emit(StreamEvent::TextChunk {
            session_id: self.id.clone(),
            block_id: block_id.clone(),
            content: text,
        });
        emitter.emit(StreamEvent::TextEnd {
            session_id: self.id.clone(),
            block_id,
        });
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
        lock_unpoisoned(&self.turn_metrics).begin_turn();
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
        lock_unpoisoned(&self.turn_metrics)
            .record_compaction(stats.estimated_tokens_before, stats.estimated_tokens_after);
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

    fn record_context_metrics(&self, bundle: &ContextBundle) {
        lock_unpoisoned(&self.turn_metrics)
            .record_context_before_model_call(bundle.estimated_tokens);
    }

    pub(crate) fn latest_turn_usage_snapshot(&self) -> TurnUsageSnapshot {
        lock_unpoisoned(&self.turn_metrics).snapshot()
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

    /// Testable entry point — delegates to the unified `run_agent_turn`.
    pub(crate) async fn send_message_with_emitter(
        &self,
        text: &str,
        emitter: &dyn crate::agent::event_sink::EventEmitter,
        hidden_contexts: Vec<HiddenContextPart>,
        turn_metadata: Option<AgentTurnMetadata>,
        activation_text: Option<&str>,
        _turn_guard: TurnInflightGuard,
    ) -> Result<(), String> {
        self.run_agent_turn(AgentTurnRunRequest {
            text,
            hidden_contexts,
            turn_metadata,
            activation_text,
            _turn_guard,
            emitter,
            tool_emitter: None,
            app_handle: None,
        })
        .await
    }

    pub(crate) async fn send_message_with_shared_emitter(
        &self,
        text: &str,
        emitter: Arc<dyn crate::agent::event_sink::EventEmitter>,
        hidden_contexts: Vec<HiddenContextPart>,
        turn_metadata: Option<AgentTurnMetadata>,
        activation_text: Option<&str>,
        _turn_guard: TurnInflightGuard,
    ) -> Result<(), String> {
        let tool_emitter = emitter.clone();
        self.run_agent_turn(AgentTurnRunRequest {
            text,
            hidden_contexts,
            turn_metadata,
            activation_text,
            _turn_guard,
            emitter: &*emitter,
            tool_emitter: Some(tool_emitter),
            app_handle: None,
        })
        .await
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
}

fn final_summary_text(assistant_content: &[serde_json::Value]) -> String {
    assistant_content
        .iter()
        .filter_map(|block| {
            if let Some(text) = block.as_str() {
                return Some(text);
            }
            (block.get("type").and_then(|value| value.as_str()) == Some("text"))
                .then(|| block.get("text").and_then(|value| value.as_str()))
                .flatten()
        })
        .collect::<Vec<_>>()
        .join("")
}

fn compact_summary_was_cancelled(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("cancelled") || lower.contains("canceled")
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
