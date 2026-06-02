use std::sync::Arc;

use crate::agent::time::now_ms;
use crate::agent::turn_state::AgentTurnState;
use crate::continuity::{
    build_send_input_reflection_event, continuity_events_from_turn,
    continuity_lessons_from_memory_candidates, continuity_lessons_from_turn, dedupe_lessons,
    ContinuityEvent, ReflectionOutcome,
};
use crate::memory::extract_candidates_from_user_message;
use crate::protocol::events::StreamEvent;
use crate::state::AppState;

fn record_continuity_event_safely(
    state: &Arc<AppState>,
    project_path: &str,
    event: ContinuityEvent,
) {
    if let Err(error) = state.continuity.record_event(project_path, &event) {
        crate::app_log!("WARN", "[continuity] event record failed: {}", error);
    }
}

fn form_continuity_experiences_safely(
    state: &Arc<AppState>,
    project_path: &str,
    session_id: &str,
    now_ms: u64,
) {
    if let Err(error) =
        state
            .continuity
            .form_experiences_for_session(project_path, session_id, now_ms)
    {
        crate::app_log!(
            "WARN",
            "[continuity] experience formation failed: {}",
            error
        );
    }
}

fn record_turn_continuity_events_safely(
    state: &Arc<AppState>,
    project_path: &str,
    turn: Option<&AgentTurnState>,
) {
    let Some(turn) = turn else {
        return;
    };
    for event in continuity_events_from_turn(turn) {
        record_continuity_event_safely(state, project_path, event);
    }
}

pub(crate) fn record_send_input_user_message_continuity(
    state: &Arc<AppState>,
    project_path: &str,
    session_id: &str,
    text: &str,
) {
    record_continuity_event_safely(
        state,
        project_path,
        ContinuityEvent::UserMessage {
            session_id: session_id.to_string(),
            content: text.to_string(),
            timestamp_ms: now_ms(),
        },
    );
}

pub(crate) async fn record_successful_send_input_continuity(
    state: &Arc<AppState>,
    app_handle: &tauri::AppHandle,
    session_id: &str,
    text: &str,
    project_path: &str,
    latest_turn: Option<&AgentTurnState>,
) {
    let memory_candidates =
        extract_candidates_from_user_message(session_id, Some(project_path), text);
    let mut continuity_lessons = continuity_lessons_from_memory_candidates(&memory_candidates);
    if let Some(turn) = latest_turn {
        continuity_lessons.extend(continuity_lessons_from_turn(turn));
        continuity_lessons = dedupe_lessons(continuity_lessons);
    }
    for candidate in memory_candidates {
        match state.wiki_memory.upsert_candidate(candidate).await {
            Ok(Some(memory)) => {
                crate::transcript::emit_stream_event(
                    app_handle,
                    StreamEvent::MemoryCandidate {
                        session_id: session_id.to_string(),
                        memory,
                    },
                );
            }
            Ok(None) => {}
            Err(error) => {
                crate::app_log!("WARN", "[wiki_memory] candidate upsert failed: {}", error);
            }
        }
    }
    record_continuity_event_safely(
        state,
        project_path,
        build_send_input_reflection_event(
            session_id,
            text,
            ReflectionOutcome::Completed,
            continuity_lessons,
            now_ms(),
        ),
    );
    record_turn_continuity_events_safely(state, project_path, latest_turn);
    form_continuity_experiences_safely(state, project_path, session_id, now_ms());
}

pub(crate) fn record_failed_send_input_continuity(
    state: &Arc<AppState>,
    session_id: &str,
    text: &str,
    project_path: &str,
    latest_turn: Option<&AgentTurnState>,
) {
    let continuity_lessons = latest_turn
        .map(continuity_lessons_from_turn)
        .unwrap_or_default();
    record_continuity_event_safely(
        state,
        project_path,
        build_send_input_reflection_event(
            session_id,
            text,
            ReflectionOutcome::Failed,
            continuity_lessons,
            now_ms(),
        ),
    );
    record_turn_continuity_events_safely(state, project_path, latest_turn);
    form_continuity_experiences_safely(state, project_path, session_id, now_ms());
}
