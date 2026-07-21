use std::sync::atomic::Ordering;

use crate::agent::goal_state::GoalLedger;
use crate::agent::session::{AgentSession, SessionStatus};
use crate::agent::session_guards::lock_unpoisoned;
use crate::agent::time::now_ms;
use crate::agent::turn_state::{AgentTurnMetadata, AgentTurnStatus};

pub(crate) fn set_goal_ledger(session: &AgentSession, ledger: GoalLedger) {
    *lock_unpoisoned(&session.goal_ledger) = Some(ledger);
}

pub(crate) fn current_goal(session: &AgentSession) -> Option<crate::agent::goal_state::GoalState> {
    lock_unpoisoned(&session.goal_ledger)
        .as_ref()
        .and_then(|ledger| ledger.current_goal().cloned())
}

pub(crate) fn normalize_goal_ledger_for_resume(session: &AgentSession) {
    if let Some(ledger) = lock_unpoisoned(&session.goal_ledger).as_mut() {
        ledger.normalize_for_resume(now_ms());
    }
}

pub(crate) fn sync_goal_task_for_a2a(
    session: &AgentSession,
    target_status: crate::agent::goal_state::GoalTaskStatus,
) {
    let task_id = {
        let ledger_guard = lock_unpoisoned(&session.goal_ledger);
        let Some(ledger) = ledger_guard.as_ref() else {
            return;
        };
        let Some(goal) = ledger.active_goal() else {
            return;
        };
        match target_status {
            crate::agent::goal_state::GoalTaskStatus::InProgress => goal
                .tasks
                .iter()
                .find(|t| t.status == crate::agent::goal_state::GoalTaskStatus::Pending)
                .map(|t| t.id.clone()),
            crate::agent::goal_state::GoalTaskStatus::Completed => goal
                .tasks
                .iter()
                .find(|t| t.status == crate::agent::goal_state::GoalTaskStatus::InProgress)
                .map(|t| t.id.clone()),
            _ => None,
        }
    };
    if let Some(task_id) = task_id {
        if let Some(ledger) = lock_unpoisoned(&session.goal_ledger).as_mut() {
            ledger.update_task_status(&task_id, target_status, now_ms());
        }
    }
}

pub(crate) fn kill(session: &AgentSession, app_handle: &tauri::AppHandle) {
    session.running.store(false, Ordering::SeqCst);
    *lock_unpoisoned(&session.status) = SessionStatus::Stopped;
    let emitter = crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone());
    session.mark_latest_turn_status_with_reason_emitter(
        AgentTurnStatus::Cancelled,
        "user_cancelled",
        Some("session killed"),
        &emitter,
    );
    if let Some(cancel) = lock_unpoisoned(&session.cancel).take() {
        cancel.notify_waiters();
    }
    crate::transcript::emit_stream_event(app_handle, session.session_stopped_event("killed"));
}

pub(crate) fn resume(session: &AgentSession, app_handle: &tauri::AppHandle) {
    session.running.store(true, Ordering::SeqCst);
    *lock_unpoisoned(&session.status) = SessionStatus::Running;
    normalize_session_state_for_resume(session);
    session.emit_latest_turn_projection(app_handle);
}

/// Normalize committed runtime state after a resume: latest turn, goal ledger,
/// and the A2A bus (running tasks become Interrupted). Each covered mutation
/// is marked dirty so the next runtime-state flush journals it; without the
/// marks, post-resume state changes would never reach the journal.
pub(crate) fn normalize_session_state_for_resume(session: &AgentSession) {
    if let Some(turn) = lock_unpoisoned(&session.latest_turn).as_mut() {
        turn.normalize_for_session_resume();
    }
    session.mark_latest_turn_dirty();
    session.normalize_goal_ledger_for_resume();
    lock_unpoisoned(&session.a2a_bus).normalize_for_resume(now_ms());
    session.mark_a2a_state_dirty();
}

pub(crate) fn kill_with_emitter(
    session: &AgentSession,
    emitter: &dyn crate::agent::event_sink::EventEmitter,
) {
    session.running.store(false, Ordering::SeqCst);
    *lock_unpoisoned(&session.status) = SessionStatus::Stopped;
    if let Some(cancel) = lock_unpoisoned(&session.cancel).as_ref() {
        cancel.notify_one();
    }
    emitter.emit(session.session_stopped_event("killed"));
}

pub(crate) fn start_turn(
    session: &AgentSession,
    text: &str,
    metadata: Option<AgentTurnMetadata>,
    app_handle: &tauri::AppHandle,
) {
    session.start_turn_with_emitter(
        text,
        metadata,
        &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
    );
}

pub(crate) fn mark_latest_turn_status(
    session: &AgentSession,
    status: AgentTurnStatus,
    app_handle: &tauri::AppHandle,
) {
    session.mark_latest_turn_status_with_reason_emitter(
        status,
        "status_update",
        None,
        &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
    );
}

pub(crate) fn mark_latest_turn_status_with_reason(
    session: &AgentSession,
    status: AgentTurnStatus,
    reason: &str,
    detail: Option<&str>,
    app_handle: &tauri::AppHandle,
) {
    session.mark_latest_turn_status_with_reason_emitter(
        status,
        reason,
        detail,
        &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
    );
}

pub(crate) fn record_latest_turn_failure(
    session: &AgentSession,
    trace: crate::agent::turn_state::AgentFailureTrace,
    app_handle: &tauri::AppHandle,
) {
    session.record_latest_turn_failure_emitter(
        trace,
        &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
    );
}

pub(crate) fn record_latest_tool(
    session: &AgentSession,
    trace: crate::agent::turn_state::AgentToolTrace,
    app_handle: &tauri::AppHandle,
) {
    session.record_latest_tool_emitter(
        trace,
        &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
    );
}

pub(crate) fn record_latest_compact(
    session: &AgentSession,
    trace: crate::agent::turn_state::AgentCompactTrace,
    app_handle: &tauri::AppHandle,
) {
    session.record_latest_compact_emitter(
        trace,
        &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
    );
}

pub(crate) fn record_latest_verification(
    session: &AgentSession,
    trace: crate::agent::turn_state::AgentVerificationTrace,
    app_handle: &tauri::AppHandle,
) {
    session.record_latest_verification_emitter(
        trace,
        &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
    );
}

pub(crate) fn record_latest_delivery_summary(
    session: &AgentSession,
    summary: &crate::protocol::events::DeliverySummary,
    app_handle: &tauri::AppHandle,
) {
    if let Some(turn) = lock_unpoisoned(&session.latest_turn).as_mut() {
        turn.record_delivery_summary(summary);
    }
    session.emit_with_emitter(&crate::agent::event_sink::TauriEventEmitter::new(
        app_handle.clone(),
    ));
}

pub(crate) fn record_latest_preview_status(
    session: &AgentSession,
    update: crate::agent::session::AgentPreviewStatusUpdate<'_>,
    app_handle: &tauri::AppHandle,
) {
    if let Some(turn) = lock_unpoisoned(&session.latest_turn).as_mut() {
        turn.record_preview_status(
            update.project_path,
            update.running,
            update.can_start,
            update.can_open,
            update.label,
            update.url,
        );
    }
    session.emit_with_emitter(&crate::agent::event_sink::TauriEventEmitter::new(
        app_handle.clone(),
    ));
}

pub(crate) fn record_latest_checkpoint_status(
    session: &AgentSession,
    is_git_repo: bool,
    dirty: bool,
    has_checkpoint: bool,
    label: &str,
    app_handle: &tauri::AppHandle,
) {
    if let Some(turn) = lock_unpoisoned(&session.latest_turn).as_mut() {
        turn.record_checkpoint_status(is_git_repo, dirty, has_checkpoint, label);
    }
    session.emit_with_emitter(&crate::agent::event_sink::TauriEventEmitter::new(
        app_handle.clone(),
    ));
}

pub(crate) async fn verify_latest_turn(
    session: &AgentSession,
    app_handle: &tauri::AppHandle,
) -> Option<crate::agent::turn_state::AgentVerificationTrace> {
    session
        .verify_latest_turn_emitter(&crate::agent::event_sink::TauriEventEmitter::new(
            app_handle.clone(),
        ))
        .await
}

pub(crate) fn apply_compaction(
    session: &AgentSession,
    compacted: &crate::agent::auto_compact::CompactResult,
    stats: &crate::agent::auto_compact::CompactStats,
    reason: &str,
    app_handle: &tauri::AppHandle,
) {
    session.apply_compaction_emitter(
        compacted,
        stats,
        reason,
        &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
    );
}

pub(crate) fn record_latest_context(
    session: &AgentSession,
    bundle: &crate::agent::context_builder::ContextBundle,
    app_handle: &tauri::AppHandle,
) {
    session.record_latest_context_emitter(
        bundle,
        &crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone()),
    );
}

pub(crate) fn emit_latest_turn_projection(session: &AgentSession, app_handle: &tauri::AppHandle) {
    session.emit_with_emitter(&crate::agent::event_sink::TauriEventEmitter::new(
        app_handle.clone(),
    ));
}
