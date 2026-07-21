use std::sync::{atomic::Ordering, Arc};

use tokio::sync::Notify;

use crate::adapters::base::ChatMessage;
use crate::agent::auto_compact::{
    prepare_compaction_for_overflow_retry, prepare_compaction_if_needed, CompactResult,
};
use crate::agent::context_builder::{
    ContextBuilder, ContextBundle, ContextSourceKind, HiddenContextPart,
};
use crate::agent::event_sink::EventEmitter;
use crate::agent::loop_guard::LoopStopReason;
use crate::agent::provider_capabilities::is_context_overflow_error;
use crate::agent::recovery::{
    api_failure_trace, build_recovery_context, verification_failure_trace,
};
use crate::agent::retry_policy::should_retry_adapter_error;
use crate::agent::session::compact::compact_summary_was_cancelled;
use crate::agent::session::{
    AgentSession, AgentTurnRunRequest, RoundDecision, MAX_AUTO_CONTINUATIONS,
};
use crate::agent::session_guards::{lock_unpoisoned, ActiveCancelGuard};
use crate::agent::time::now_ms;
use crate::agent::tool_results::{
    push_assistant_result_with_synthetic_tool_results, repair_tool_use_adjacency,
};
use crate::agent::turn_metrics::TurnMetricsEventEmitter;
use crate::agent::turn_outcome::{
    final_answer_instruction, final_turn_status_for_current_turn,
    final_turn_transition_reason_for_current_turn, verification_has_failed,
};
use crate::agent::turn_state::{
    AgentRecoveryAdvice, AgentTurnMetadata, AgentTurnStatus, AgentVerificationStatus,
    AgentVerificationTrace,
};
use crate::agent::verification;
use crate::consts::{AGENT_LOOP_SETTLE_DELAY, AGENT_OVERFLOW_RETRY_DELAY};
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;

impl AgentSession {
    /// Continue a send after the IPC layer has reserved the turn.
    pub(crate) async fn send_message_with_reserved_turn(
        &self,
        text: &str,
        app_handle: &tauri::AppHandle,
        hidden_contexts: Vec<HiddenContextPart>,
        turn_metadata: Option<AgentTurnMetadata>,
        activation_text: Option<&str>,
        _turn_guard: crate::agent::session_guards::TurnInflightGuard,
    ) -> Result<(), String> {
        let emitter: Arc<dyn EventEmitter> = Arc::new(
            crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
        );
        self.run_agent_turn(AgentTurnRunRequest {
            text,
            hidden_contexts,
            turn_metadata,
            activation_text,
            _turn_guard,
            emitter: &*emitter,
            tool_emitter: Some(emitter.clone()),
            app_handle: Some(app_handle),
        })
        .await
    }

    /// Core agent turn loop — unified implementation used by both production
    /// Tauri events and test/headless emitters.
    /// Phase 1 — set up the turn: recovery context, system prompt, user message,
    /// cancel token, and initial status.
    pub(crate) async fn setup_turn(
        &self,
        text: &str,
        mut hidden_contexts: Vec<HiddenContextPart>,
        turn_metadata: Option<AgentTurnMetadata>,
        activation_text: Option<&str>,
        emitter: &dyn EventEmitter,
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

        let _ = self.append_conversation_message(
            ChatMessage::user(text),
            crate::agent::session_mutation::SessionMutationSource::UserInput,
        );
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
        emitter: &dyn EventEmitter,
        app_handle: Option<&tauri::AppHandle>,
        tool_emitter: Option<Arc<dyn EventEmitter>>,
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
        if compacted.attempted {
            lock_unpoisoned(&self.loop_guard).record_compact_attempt();
        }

        if let Some(stats) = compacted.stats.as_ref() {
            self.apply_compaction_emitter(&compacted, stats, "auto_compact", emitter);
        } else if compacted.attempted {
            if let Some(reason) = compacted.skipped_reason.as_deref() {
                emitter.emit(self.context_compact_skipped_event(reason, compacted.messages.len()));
            }
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
                        lock_unpoisoned(&self.loop_guard).record_overflow_retry();

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
                        } else if compacted.attempted {
                            if let Some(reason) = compacted.skipped_reason.as_deref() {
                                emitter.emit(self.context_compact_skipped_event(
                                    reason,
                                    compacted.messages.len(),
                                ));
                            }
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

        self.record_model_round_emitter(emitter);

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
            let _ = self.append_conversation_message(
                ChatMessage::assistant(serde_json::Value::Array(result.assistant_content.clone())),
                crate::agent::session_mutation::SessionMutationSource::AssistantResponse,
            );
        }

        if result.tool_calls.is_empty() {
            // Before stopping, check if the goal ledger still has pending tasks.
            // If yes and we haven't exhausted auto-continuations, inject a
            // continuation prompt so the agent keeps working instead of
            // prematurely stopping.
            let should_auto_continue = {
                let ledger = lock_unpoisoned(&self.goal_ledger);
                ledger.as_ref().is_some_and(|l| l.has_pending_tasks())
            };

            if should_auto_continue {
                let has_more = {
                    let guard = lock_unpoisoned(&self.loop_guard);
                    guard.auto_continuation_count() < MAX_AUTO_CONTINUATIONS
                };
                if has_more {
                    lock_unpoisoned(&self.loop_guard).record_auto_continuation();
                    crate::app_log!(
                        "INFO",
                        "Auto-continuation for session {}: model returned no tool calls but pending tasks remain",
                        self.id
                    );
                    let _ = self.append_conversation_message(
                        ChatMessage::user(
                            "Please continue working on the remaining tasks. You have pending items that need to be completed. Proceed with the next concrete step.",
                        ),
                        crate::agent::session_mutation::SessionMutationSource::AutoContinuation,
                    );
                    return Ok(RoundDecision::Continue);
                }
            }

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

    /// Phase 3 — verification, final summary, and status transition.
    pub(crate) async fn finalize_turn(
        &self,
        hidden_contexts: &[HiddenContextPart],
        emitter: &dyn EventEmitter,
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
                let latest_turn = lock_unpoisoned(&self.latest_turn).clone();
                msgs.push(ChatMessage::user(&final_answer_instruction(
                    verification_trace.as_ref(),
                    latest_turn.as_ref(),
                )));
                crate::app_log!("INFO", "Agent loop complete — requesting text-only summary");
                let adapter_result = self.adapter.call(&msgs, cancel.clone()).await;
                if let Ok(result) = adapter_result {
                    if !result.assistant_content.is_empty() {
                        self.emit_final_summary_text_emitter(&result.assistant_content, emitter);
                        let mut staged = Vec::new();
                        push_assistant_result_with_synthetic_tool_results(
                            &mut staged,
                            result.assistant_content,
                            &result.tool_calls,
                            "final_summary_tool_call_not_executed",
                        );
                        let _ = self.append_conversation_messages(
                            staged,
                            crate::agent::session_mutation::SessionMutationSource::FinalSummary,
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

        loop {
            if let Err(stop) = lock_unpoisoned(&self.loop_guard).check() {
                crate::app_log!(
                    "WARN",
                    "Agent loop stopped for session {}: {:?}",
                    self.id,
                    stop
                );
                self.record_loop_guard_stop_emitter(&stop, request.emitter);
                break;
            }

            let round_decision = self
                .execute_single_round(
                    &hidden_contexts,
                    cancel.clone(),
                    request.emitter,
                    request.app_handle,
                    request.tool_emitter.clone(),
                    &mut overflow_retry_used,
                )
                .await?;
            // Round completion is the runtime-state append-policy tick: the
            // latest-turn pointer advance (plus any goal/A2A marks from the
            // round) coalesces into one journaled RuntimeStateUpdated.
            self.mark_latest_turn_dirty();
            let _ = self.flush_session_runtime_state(
                crate::agent::session_mutation::SessionMutationSource::RoundCompletion,
            );
            match round_decision {
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
        self.mark_latest_turn_dirty();
        let _ = self.flush_session_runtime_state(
            crate::agent::session_mutation::SessionMutationSource::RoundCompletion,
        );
        Ok(())
    }

    /// Testable entry point — delegates to the unified `run_agent_turn`.
    pub(crate) async fn send_message_with_emitter(
        &self,
        text: &str,
        emitter: &dyn EventEmitter,
        hidden_contexts: Vec<HiddenContextPart>,
        turn_metadata: Option<AgentTurnMetadata>,
        activation_text: Option<&str>,
        _turn_guard: crate::agent::session_guards::TurnInflightGuard,
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
        emitter: Arc<dyn EventEmitter>,
        hidden_contexts: Vec<HiddenContextPart>,
        turn_metadata: Option<AgentTurnMetadata>,
        activation_text: Option<&str>,
        _turn_guard: crate::agent::session_guards::TurnInflightGuard,
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

    fn record_model_round_emitter(&self, emitter: &dyn EventEmitter) {
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.model_rounds += 1;
        }
        lock_unpoisoned(&self.turn_metrics).record_model_round();
        lock_unpoisoned(&self.loop_guard).record_model_round();
        self.emit_with_emitter(emitter);
    }

    fn emit_final_summary_text_emitter(
        &self,
        assistant_content: &[serde_json::Value],
        emitter: &dyn EventEmitter,
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

    pub(crate) fn start_turn_with_emitter(
        &self,
        text: &str,
        metadata: Option<AgentTurnMetadata>,
        emitter: &dyn EventEmitter,
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
        lock_unpoisoned(&self.loop_guard).reset();
        self.emit_with_emitter(emitter);
    }

    fn record_loop_guard_stop_emitter(&self, stop: &LoopStopReason, emitter: &dyn EventEmitter) {
        let detail = loop_guard_recovery_detail(stop);
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.set_stop_reason(stop.as_str());
            turn.mark_status_with_reason(turn.status.clone(), "loop_guard_stopped", Some(&detail));
        }
        self.emit_with_emitter(emitter);
    }

    pub(crate) async fn verify_latest_turn_emitter(
        &self,
        emitter: &dyn EventEmitter,
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
}

pub(crate) fn final_summary_text(assistant_content: &[serde_json::Value]) -> String {
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

fn loop_guard_recovery_advice(stop: &LoopStopReason) -> AgentRecoveryAdvice {
    match stop {
        LoopStopReason::ModelRoundLimit => AgentRecoveryAdvice {
            action: "narrow the task or accept partial results".to_string(),
            reason: "the model used too many consecutive rounds without reaching a final answer"
                .to_string(),
            instruction: "Summarize the current state and ask the user to narrow the scope or accept the阶段性成果 before continuing.".to_string(),
            safe_to_auto_retry: false,
            requires_user_action: true,
        },
        LoopStopReason::ToolCallLimit => AgentRecoveryAdvice {
            action: "reduce file scope or limit tool actions".to_string(),
            reason: "the turn used too many tool calls".to_string(),
            instruction: "Reduce the number of files touched in each batch, or constrain the tool actions to a smaller surface area.".to_string(),
            safe_to_auto_retry: false,
            requires_user_action: true,
        },
        LoopStopReason::RepeatedCategoryBatch => AgentRecoveryAdvice {
            action: "stop exploring and synthesize conclusions".to_string(),
            reason: "the same category of tool requests repeated with changing inputs".to_string(),
            instruction: "Stop further exploration. Synthesize conclusions from the information already read, or ask the user to clarify the next concrete step.".to_string(),
            safe_to_auto_retry: false,
            requires_user_action: true,
        },
        LoopStopReason::RepeatedNoProgress => AgentRecoveryAdvice {
            action: "switch strategy or ask for user clarification".to_string(),
            reason: "multiple tool batches completed without useful progress".to_string(),
            instruction: "Switch strategy: check why tools are failing, or ask the user to provide missing context or clarify the goal.".to_string(),
            safe_to_auto_retry: false,
            requires_user_action: true,
        },
        LoopStopReason::CompactUnavailable => AgentRecoveryAdvice {
            action: "compact manually, split the task, or reduce context".to_string(),
            reason: "context compaction could not make enough room to continue safely".to_string(),
            instruction: "Try a manual compact, break the remaining work into smaller sub-tasks, or reduce the amount of context injected into the session.".to_string(),
            safe_to_auto_retry: false,
            requires_user_action: true,
        },
        LoopStopReason::RepeatedOverflow => AgentRecoveryAdvice {
            action: "compact, split task, or reduce context".to_string(),
            reason: "context overflow repeated after retry and compaction attempts".to_string(),
            instruction: "Context keeps overflowing. Compact the session history, split the remaining work into smaller pieces, or ask the user to focus on a narrower scope.".to_string(),
            safe_to_auto_retry: false,
            requires_user_action: true,
        },
        LoopStopReason::ToolLoopDetected => AgentRecoveryAdvice {
            action: "use a different approach".to_string(),
            reason: "the same tool request repeated and looked like a loop".to_string(),
            instruction: "The agent is stuck in a tool loop. Suggest a different approach or ask the user to provide more specific guidance.".to_string(),
            safe_to_auto_retry: false,
            requires_user_action: true,
        },
    }
}

pub(crate) fn loop_guard_recovery_detail(stop: &LoopStopReason) -> String {
    let advice = loop_guard_recovery_advice(stop);
    format!(
        "stop_reason={}; {}; advice={}; try a smaller next action before continuing.",
        stop.as_str(),
        advice.reason,
        advice.action,
    )
}
