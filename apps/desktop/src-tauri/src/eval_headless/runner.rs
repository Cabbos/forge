use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::agent::event_sink::{CollectingEventEmitter, EventEmitter};
use crate::agent::loop_guard::LoopGuard;
use crate::agent::session_guards::lock_unpoisoned;
use crate::agent::turn_state::{AgentTurnState, AgentTurnStatus};
use crate::continuity::{
    build_episode_from_turn, build_send_input_reflection_event, continuity_events_from_turn,
    continuity_lessons_from_turn, ContinuityEvent, ContinuityService, ReflectionOutcome,
};
use crate::protocol::events::StreamEvent;

use super::types::{
    PendingConfirms, HEADLESS_CONFIRM_RETRY_ATTEMPTS, HEADLESS_CONFIRM_RETRY_DELAY_MS,
};

pub(crate) struct HeadlessEventEmitter {
    collector: CollectingEventEmitter,
    pending_confirms: PendingConfirms,
    model_rounds: Arc<AtomicUsize>,
    was_calling_model: AtomicBool,
}

impl HeadlessEventEmitter {
    pub(crate) fn new(pending_confirms: PendingConfirms) -> Self {
        Self {
            collector: CollectingEventEmitter::new(),
            pending_confirms,
            model_rounds: Arc::new(AtomicUsize::new(0)),
            was_calling_model: AtomicBool::new(false),
        }
    }

    pub(crate) fn drain(&self) -> Vec<StreamEvent> {
        self.collector.drain()
    }

    pub(crate) fn model_rounds(&self) -> usize {
        self.model_rounds.load(Ordering::SeqCst)
    }
}

impl EventEmitter for HeadlessEventEmitter {
    fn emit(&self, event: StreamEvent) {
        if let StreamEvent::ConfirmAsk { block_id, kind, .. } = &event {
            let pending_confirms = self.pending_confirms.clone();
            let block_id = block_id.clone();
            let approve = kind != "ask_user";
            tokio::spawn(async move {
                for _ in 0..HEADLESS_CONFIRM_RETRY_ATTEMPTS {
                    if let Some(sender) = pending_confirms.write().await.remove(&block_id) {
                        let _ = sender.send(approve);
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(
                        HEADLESS_CONFIRM_RETRY_DELAY_MS,
                    ))
                    .await;
                }
            });
        }

        if let StreamEvent::AgentTurnUpdated { state, .. } = &event {
            let is_calling_model = state.status == AgentTurnStatus::CallingModel;
            let was_calling = self.was_calling_model.load(Ordering::SeqCst);
            if is_calling_model && !was_calling {
                self.model_rounds.fetch_add(1, Ordering::SeqCst);
            }
            self.was_calling_model
                .store(is_calling_model, Ordering::SeqCst);
        }

        self.collector.emit(event);
    }
}

pub(crate) fn spawn_timeout_watchdog(
    started: Instant,
    timeout_secs: u64,
    session: Arc<crate::agent::session::AgentSession>,
    emitter: Arc<HeadlessEventEmitter>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let elapsed = started.elapsed().as_secs();
        if elapsed >= timeout_secs {
            session.kill_with_emitter(&*emitter);
            return;
        }
        let remaining = timeout_secs - elapsed;
        tokio::time::sleep(Duration::from_secs(remaining)).await;
        if started.elapsed().as_secs() >= timeout_secs {
            session.kill_with_emitter(&*emitter);
        }
    })
}

pub(crate) async fn send_headless_turn(
    session: &crate::agent::session::AgentSession,
    prompt: &str,
    emitter: Arc<HeadlessEventEmitter>,
    model_rounds_used: usize,
    max_model_rounds: usize,
) -> Result<(), String> {
    configure_headless_model_round_budget(session, model_rounds_used, max_model_rounds);
    let turn_guard = session.reserve_turn()?;
    session
        .send_message_with_shared_emitter(prompt, emitter, Vec::new(), None, None, turn_guard)
        .await
}

pub(crate) fn configure_headless_model_round_budget(
    session: &crate::agent::session::AgentSession,
    model_rounds_used: usize,
    max_model_rounds: usize,
) {
    let remaining = max_model_rounds.saturating_sub(model_rounds_used).max(1);
    *lock_unpoisoned(&session.loop_guard) =
        LoopGuard::default_limits().with_max_model_rounds(remaining);
}

pub(crate) fn headless_reflection_outcome(
    agent_error: Option<&String>,
    latest_turn: Option<&AgentTurnState>,
) -> ReflectionOutcome {
    if agent_error.is_some() {
        return ReflectionOutcome::Failed;
    }

    let Some(turn) = latest_turn else {
        return ReflectionOutcome::Failed;
    };

    if matches!(turn.status, AgentTurnStatus::Cancelled) {
        return ReflectionOutcome::Cancelled;
    }
    if matches!(turn.status, AgentTurnStatus::Failed)
        || matches!(
            turn.verification.status,
            crate::agent::turn_state::AgentVerificationStatus::Failed
                | crate::agent::turn_state::AgentVerificationStatus::Error
        )
    {
        return ReflectionOutcome::Failed;
    }

    ReflectionOutcome::Completed
}

pub(crate) fn record_headless_continuity(
    service: &ContinuityService,
    project_path: &Path,
    session_id: &str,
    prompt: &str,
    latest_turn: Option<&AgentTurnState>,
    outcome: ReflectionOutcome,
    timestamp_ms: u64,
) -> Result<usize, String> {
    let project_path = project_path
        .to_str()
        .ok_or_else(|| "headless workspace path is not valid UTF-8".to_string())?;
    service.record_event(
        project_path,
        &ContinuityEvent::UserMessage {
            session_id: session_id.to_string(),
            content: prompt.to_string(),
            timestamp_ms,
        },
    )?;

    let continuity_lessons = latest_turn
        .map(continuity_lessons_from_turn)
        .unwrap_or_default();
    let episode = latest_turn.map(build_episode_from_turn);
    service.record_event(
        project_path,
        &build_send_input_reflection_event(
            session_id,
            prompt,
            outcome,
            continuity_lessons,
            episode,
            timestamp_ms,
        ),
    )?;

    if let Some(turn) = latest_turn {
        for event in continuity_events_from_turn(turn) {
            service.record_event(project_path, &event)?;
        }
    }

    let formed = service.form_experiences_for_session(project_path, session_id, timestamp_ms)?;
    Ok(formed.len())
}
