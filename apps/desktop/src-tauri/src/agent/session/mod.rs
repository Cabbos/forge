use parking_lot::Mutex;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};

use tokio::sync::Notify;

use crate::adapters::base::{AiAdapter, ChatMessage};
use crate::agent::auto_compact::AutoCompactGuard;
use crate::agent::context_builder::{ContextBundle, ContextSourceKind, HiddenContextPart};
use crate::agent::event_sink::EventEmitter;
use crate::agent::goal_state::GoalLedger;
use crate::agent::loop_guard::LoopGuard;
use crate::agent::session_events;
pub(crate) use crate::agent::session_guards::TurnInflightGuard;
use crate::agent::session_guards::{lock_unpoisoned, try_begin_turn};
use crate::agent::snapshot::AgentSessionSnapshot;
use crate::agent::tool_results::repair_tool_use_adjacency;
use crate::agent::turn_metrics::{TurnMetrics, TurnUsageSnapshot};
use crate::agent::turn_state::{
    AgentCompactTrace, AgentFailureTrace, AgentTurnMetadata, AgentTurnState, AgentTurnStatus,
    AgentVerificationTrace,
};
// Re-exports for tests
#[cfg(test)]
pub(crate) use crate::agent::auto_compact::CompactStats;
#[cfg(test)]
pub(crate) use crate::agent::session::r#loop::{final_summary_text, loop_guard_recovery_detail};
#[cfg(test)]
pub(crate) use crate::agent::session::tools::{tool_batch_signature, tool_category_signature};
#[cfg(test)]
pub(crate) use crate::agent::turn_state::running_tool_trace;
use crate::harness::Harness;
use crate::protocol::events::StreamEvent;

pub(crate) mod a2a;
pub(crate) mod compact;
pub(crate) mod lifecycle;
pub(crate) mod r#loop;
pub(crate) mod tools;

const MAX_AGENT_TOOL_ROUNDS: usize = 80;
/// Max times the agent may auto-continue when the model returns no tool calls
/// but pending goal tasks remain.
const MAX_AUTO_CONTINUATIONS: usize = 3;
pub use crate::agent::manual_compact::ManualCompactResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Starting,
    Resuming,
    Running,
    Stopped,
    Error(String),
}

impl SessionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            SessionStatus::Starting => "starting",
            SessionStatus::Resuming => "resuming",
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
    pub(crate) loop_guard: Mutex<LoopGuard>,
    pub(crate) context_window_tokens: Option<u32>,
    pub(crate) cancel: Mutex<Option<Arc<Notify>>>,
    pub(crate) goal_ledger: Mutex<Option<GoalLedger>>,
    pub(crate) a2a_bus: Mutex<crate::agent::a2a::bus::AgentA2ABus>,
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
            loop_guard: Mutex::new(LoopGuard::default_limits()),
            context_window_tokens,
            cancel: Mutex::new(None),
            goal_ledger: Mutex::new(None),
            a2a_bus: Mutex::new(crate::agent::a2a::bus::AgentA2ABus::default()),
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
        goal_ledger: Option<GoalLedger>,
        a2a_state: Option<crate::agent::a2a::bus::AgentA2ABus>,
    ) {
        *lock_unpoisoned(&self.messages) = repair_tool_use_adjacency(messages);
        *lock_unpoisoned(&self.summary) = summary;
        *lock_unpoisoned(&self.latest_turn) = latest_turn.map(|mut turn| {
            turn.normalize_for_session_resume();
            turn
        });
        *lock_unpoisoned(&self.goal_ledger) = goal_ledger;
        *lock_unpoisoned(&self.a2a_bus) = a2a_state.unwrap_or_default();
    }

    pub fn snapshot(&self) -> AgentSessionSnapshot {
        let messages = lock_unpoisoned(&self.messages).clone();
        let summary = lock_unpoisoned(&self.summary).clone();
        let mut snapshot = AgentSessionSnapshot::new(
            self.id.clone(),
            self.agent_type.clone(),
            self.model_id.clone(),
            self.harness.working_dir.to_string_lossy().to_string(),
            messages,
            summary,
            self.context_window_tokens,
        );
        if let Some(latest_turn) = lock_unpoisoned(&self.latest_turn).clone() {
            snapshot = snapshot.with_latest_turn(latest_turn);
        }
        if let Some(goal_ledger) = lock_unpoisoned(&self.goal_ledger).clone() {
            snapshot = snapshot.with_goal_ledger(goal_ledger);
        }
        let a2a_state = lock_unpoisoned(&self.a2a_bus).clone();
        if !a2a_state.tasks.is_empty() || !a2a_state.messages.is_empty() {
            snapshot = snapshot.with_a2a_state(a2a_state);
        }
        snapshot
    }

    pub fn set_goal_ledger(&self, ledger: GoalLedger) {
        lifecycle::set_goal_ledger(self, ledger);
    }

    pub fn current_goal(&self) -> Option<crate::agent::goal_state::GoalState> {
        lifecycle::current_goal(self)
    }

    pub(crate) fn normalize_goal_ledger_for_resume(&self) {
        lifecycle::normalize_goal_ledger_for_resume(self);
    }

    pub(crate) fn sync_goal_task_for_a2a(
        &self,
        target_status: crate::agent::goal_state::GoalTaskStatus,
    ) {
        lifecycle::sync_goal_task_for_a2a(self, target_status);
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

    pub fn kill(&self, app_handle: &tauri::AppHandle) {
        lifecycle::kill(self, app_handle);
    }

    pub fn resume(&self, app_handle: &tauri::AppHandle) {
        lifecycle::resume(self, app_handle);
    }

    pub fn kill_with_emitter(&self, emitter: &dyn crate::agent::event_sink::EventEmitter) {
        lifecycle::kill_with_emitter(self, emitter);
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

    pub fn record_latest_delivery_summary(
        &self,
        summary: &crate::protocol::events::DeliverySummary,
        app_handle: &tauri::AppHandle,
    ) {
        lifecycle::record_latest_delivery_summary(self, summary, app_handle);
    }

    pub fn record_latest_preview_status(
        &self,
        update: AgentPreviewStatusUpdate<'_>,
        app_handle: &tauri::AppHandle,
    ) {
        lifecycle::record_latest_preview_status(self, update, app_handle);
    }

    pub fn record_latest_checkpoint_status(
        &self,
        is_git_repo: bool,
        dirty: bool,
        has_checkpoint: bool,
        label: &str,
        app_handle: &tauri::AppHandle,
    ) {
        lifecycle::record_latest_checkpoint_status(
            self,
            is_git_repo,
            dirty,
            has_checkpoint,
            label,
            app_handle,
        );
    }

    pub fn emit_latest_turn_projection(&self, app_handle: &tauri::AppHandle) {
        lifecycle::emit_latest_turn_projection(self, app_handle);
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

    pub(crate) fn context_compact_start_event(&self) -> StreamEvent {
        session_events::context_compact_start_event(&self.id)
    }

    pub(crate) fn context_compacted_event(
        &self,
        stats: &crate::agent::auto_compact::CompactStats,
    ) -> StreamEvent {
        session_events::context_compacted_event(&self.id, stats)
    }

    pub(crate) fn context_compact_skipped_event(
        &self,
        reason: &str,
        retained_messages: usize,
    ) -> StreamEvent {
        session_events::context_compact_skipped_event(&self.id, reason, retained_messages)
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
}

#[cfg(test)]
#[path = "../session_tests.rs"]
mod tests;
