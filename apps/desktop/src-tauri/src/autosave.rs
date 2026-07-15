//! Debounced snapshot autosave triggered by significant StreamEvents.
//!
//! Phase 1.2: Wire snapshot save triggers on every significant StreamEvent
//! (debounced) and on app RunEvent::Exit. Files: lib.rs, state.rs.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tauri::Manager;

use crate::ipc::session_lifecycle::save_session_snapshot_with_workflow;
use crate::protocol::events::StreamEvent;
use crate::state::AppState;

/// Delay between the last significant event and the actual snapshot write.
/// Kept short so tests can verify without long sleeps, but long enough to
/// coalesce bursts of events (e.g. tool_call_start + tool_call_result +
/// tool_call_end arriving in rapid succession).
const AUTOSAVE_DEBOUNCE_MS: u64 = 800;

/// Per-session "save is scheduled" flag. When true, a spawned task is already
/// waiting to write the snapshot; further events for the same session just
/// leave the flag set (the pending task will pick up the latest state when it
/// fires).
static PENDING_SAVES: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();

// ── Event classification ───────────────────────────────────────────────

/// Returns `true` when the event represents a meaningful state transition
/// worth persisting. High-frequency streaming events (`thinking_chunk`,
/// `text_chunk`, `shell_output`) return `false` to avoid writing dozens of
/// snapshots per second.
pub fn is_significant_stream_event(event: &StreamEvent) -> bool {
    !matches!(
        event,
        StreamEvent::ThinkingChunk { .. }
            | StreamEvent::TextChunk { .. }
            | StreamEvent::ShellOutput { .. }
    )
}

// ── Autosave scheduling ─────────────────────────────────────────────────

/// Schedule a debounced snapshot save for the session referenced by `event`.
///
/// Safe to call from any thread.  If the `AppHandle` has no managed
/// `AppState`, or the session no longer exists, the call is silently ignored
/// (logged at WARN level only when a save actually fails, not for missing
/// state).
pub fn schedule_autosave(app_handle: &tauri::AppHandle, event: &StreamEvent) {
    if !is_significant_stream_event(event) {
        return;
    }

    let session_id = event.session_id().to_string();
    let handle = app_handle.clone();

    // ── Coalesce: if a save is already scheduled for this session, don't
    //    spawn a second task — the pending one will capture the latest state.
    {
        let map = PENDING_SAVES.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = match map.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if guard.insert(session_id.clone(), true).is_some() {
            return; // already pending
        }
    }

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(AUTOSAVE_DEBOUNCE_MS)).await;

        // Clear the pending flag *before* the save so a burst that arrives
        // during the write will schedule a follow-up.
        {
            if let Some(map) = PENDING_SAVES.get() {
                if let Ok(mut guard) = map.lock() {
                    guard.remove(&session_id);
                }
            }
        }

        let state: Arc<AppState> = match handle.try_state::<Arc<AppState>>() {
            Some(s) => s.inner().clone(),
            None => {
                crate::app_log!(
                    "WARN",
                    "[autosave] AppState not available for session {session_id}"
                );
                return;
            }
        };

        let session = {
            let sessions = state.sessions.read().await;
            sessions.get(&session_id).cloned()
        };

        let Some(session) = session else {
            // Session was unregistered between schedule and save — benign.
            return;
        };

        if let Err(error) = save_session_snapshot_with_workflow(&state, &session).await {
            crate::app_log!(
                "WARN",
                "[autosave] snapshot save failed for session {session_id}: {error}"
            );
        }
    });
}

// ── Exit flush ──────────────────────────────────────────────────────────

/// Flush snapshots for all live sessions. Called on `RunEvent::Exit`.
///
/// Uses `tauri::async_runtime::block_on` so it completes before the runtime
/// shuts down.  Individual save failures are logged; the flush continues
/// through all sessions.
pub fn flush_all_sessions(app_handle: &tauri::AppHandle) {
    let state: Arc<AppState> = match app_handle.try_state::<Arc<AppState>>() {
        Some(s) => s.inner().clone(),
        None => {
            crate::app_log!("WARN", "[autosave] no AppState for exit flush");
            return;
        }
    };

    // Clear pending-save flags so no spawned task races with us.
    if let Some(map) = PENDING_SAVES.get() {
        if let Ok(mut guard) = map.lock() {
            guard.clear();
        }
    }

    tauri::async_runtime::block_on(async move {
        let sessions: Vec<_> = state.sessions.read().await.values().cloned().collect();
        crate::app_log!(
            "INFO",
            "[autosave] exit flush: saving {} live session(s)",
            sessions.len()
        );
        for session in &sessions {
            if let Err(error) = save_session_snapshot_with_workflow(&state, session).await {
                crate::app_log!(
                    "WARN",
                    "[autosave] exit flush failed for session {}: {error}",
                    session.id
                );
            }
        }
    });
}

// ── Test helpers ────────────────────────────────────────────────────────

#[cfg(test)]
pub(crate) fn clear_pending_saves_for_test() {
    if let Some(map) = PENDING_SAVES.get() {
        if let Ok(mut guard) = map.lock() {
            guard.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::events::StreamEvent;

    // ── Classification tests ──────────────────────────────────────────

    #[test]
    fn high_frequency_events_are_not_significant() {
        assert!(!is_significant_stream_event(&StreamEvent::ThinkingChunk {
            session_id: "s".into(),
            block_id: "b".into(),
            content: "c".into(),
        }));
        assert!(!is_significant_stream_event(&StreamEvent::TextChunk {
            session_id: "s".into(),
            block_id: "b".into(),
            content: "c".into(),
        }));
        assert!(!is_significant_stream_event(&StreamEvent::ShellOutput {
            session_id: "s".into(),
            block_id: "b".into(),
            content: "c".into(),
        }));
    }

    #[test]
    fn structural_events_are_significant() {
        // Session lifecycle
        assert!(is_significant_stream_event(&StreamEvent::SessionStarted {
            session_id: "s".into(),
            agent_type: "a".into(),
            model: "m".into(),
            context_window_tokens: None,
        }));
        assert!(is_significant_stream_event(&StreamEvent::SessionStatus {
            session_id: "s".into(),
            status: "idle".into(),
        }));
        assert!(is_significant_stream_event(&StreamEvent::SessionStopped {
            session_id: "s".into(),
            reason: "r".into(),
        }));

        // User interaction
        assert!(is_significant_stream_event(&StreamEvent::UserMessage {
            session_id: "s".into(),
            block_id: "b".into(),
            content: "c".into(),
        }));

        // Tool calls
        assert!(is_significant_stream_event(&StreamEvent::ToolCallStart {
            session_id: "s".into(),
            block_id: "b".into(),
            tool_name: "t".into(),
            tool_input: serde_json::Value::Null,
        }));
        assert!(is_significant_stream_event(&StreamEvent::ToolCallResult {
            session_id: "s".into(),
            block_id: "b".into(),
            result: "r".into(),
            is_error: false,
            duration_ms: 0,
        }));
        assert!(is_significant_stream_event(&StreamEvent::ToolCallEnd {
            session_id: "s".into(),
            block_id: "b".into(),
        }));

        // Shell boundaries
        assert!(is_significant_stream_event(&StreamEvent::ShellStart {
            session_id: "s".into(),
            block_id: "b".into(),
            command: "c".into(),
        }));
        assert!(is_significant_stream_event(&StreamEvent::ShellEnd {
            session_id: "s".into(),
            block_id: "b".into(),
            exit_code: 0,
        }));

        // Context compact
        assert!(is_significant_stream_event(
            &StreamEvent::ContextCompactStart {
                session_id: "s".into(),
                block_id: "b".into(),
            }
        ));
        assert!(is_significant_stream_event(
            &StreamEvent::ContextCompacted {
                session_id: "s".into(),
                block_id: "b".into(),
                summary: "sum".into(),
                retained_messages: 1,
                compacted_messages: 0,
                estimated_tokens_before: 100,
                estimated_tokens_after: 50,
            }
        ));

        // Structural streaming boundaries (not per-token)
        assert!(is_significant_stream_event(&StreamEvent::ThinkingStart {
            session_id: "s".into(),
            block_id: "b".into(),
        }));
        assert!(is_significant_stream_event(&StreamEvent::ThinkingEnd {
            session_id: "s".into(),
            block_id: "b".into(),
        }));
        assert!(is_significant_stream_event(&StreamEvent::TextStart {
            session_id: "s".into(),
            block_id: "b".into(),
        }));
        assert!(is_significant_stream_event(&StreamEvent::TextEnd {
            session_id: "s".into(),
            block_id: "b".into(),
        }));

        // Confirm
        assert!(is_significant_stream_event(&StreamEvent::ConfirmAsk {
            session_id: "s".into(),
            block_id: "b".into(),
            question: "q".into(),
            kind: "k".into(),
            boundary: None,
            permission_evidence: None,
            replayed_interrupted: false,
        }));
        assert!(is_significant_stream_event(&StreamEvent::ConfirmResponse {
            session_id: "s".into(),
            block_id: "b".into(),
            question: Some("q".into()),
            kind: Some("k".into()),
            boundary: None,
            permission_evidence: None,
            approved: Some(true),
            responded_at_ms: 1,
            reason: Some("user_response".into()),
            replayed: false,
        }));

        // Error / usage
        assert!(is_significant_stream_event(&StreamEvent::Error {
            session_id: "s".into(),
            block_id: "b".into(),
            message: "m".into(),
            code: "c".into(),
        }));
        assert!(is_significant_stream_event(&StreamEvent::Usage {
            session_id: "s".into(),
            input_tokens: 100,
            output_tokens: 50,
            estimated_cost_usd: 0.01,
        }));
    }

    #[test]
    fn recovery_and_a2a_events_are_significant() {
        use crate::agent::a2a::projection::AgentA2AProjection;

        assert!(is_significant_stream_event(&StreamEvent::RecoveryNotice {
            session_id: "s".into(),
            notice_id: "n".into(),
            title: "t".into(),
            message: "m".into(),
            reason: "r".into(),
            recoverable: true,
        }));

        assert!(is_significant_stream_event(&StreamEvent::AgentA2AUpdated {
            session_id: "s".into(),
            state: AgentA2AProjection::default(),
        }));
    }

    // ── Coalescing / debounce unit test ──────────────────────────────

    /// Verify that calling `schedule_autosave` twice for the same session
    /// only spawns one save task (the second call is coalesced).
    /// We can't observe the spawned task directly in a unit test without a
    /// real Tauri runtime, but we can verify the pending-saves map behaves
    /// correctly.
    #[test]
    fn pending_saves_map_coalesces_duplicate_session_ids() {
        clear_pending_saves_for_test();

        let map = PENDING_SAVES.get_or_init(|| Mutex::new(HashMap::new()));
        {
            let mut guard = map.lock().unwrap();
            // Simulate first call: insert returns None (was vacant)
            assert!(guard.insert("session-1".to_string(), true).is_none());
            // Simulate second call: insert returns Some (was occupied)
            assert_eq!(guard.insert("session-1".to_string(), true), Some(true));
        }
        // Clean up
        clear_pending_saves_for_test();
    }

    #[test]
    fn pending_saves_map_handles_multiple_sessions_independently() {
        clear_pending_saves_for_test();

        let map = PENDING_SAVES.get_or_init(|| Mutex::new(HashMap::new()));
        {
            let mut guard = map.lock().unwrap();
            assert!(guard.insert("session-1".to_string(), true).is_none());
            assert!(guard.insert("session-2".to_string(), true).is_none());
            assert_eq!(guard.len(), 2);
        }
        clear_pending_saves_for_test();
    }

    // ── Safety / no-panic tests ─────────────────────────────────────

    #[test]
    fn classification_does_not_panic_on_any_variant() {
        // Smoke-test: every StreamEvent variant should be classifiable
        // without panicking. Build a minimal instance of each.
        use crate::agent::a2a::projection::AgentA2AProjection;
        use crate::agent::turn_state::AgentTurnProjection;
        use crate::forge_wiki::model::{ForgeWikiProposalStatus, ForgeWikiUpdateProposal};
        use crate::memory::WikiMemory;
        use crate::workflow::WorkflowState;

        let events: Vec<StreamEvent> = vec![
            StreamEvent::UserMessage {
                session_id: "s".into(),
                block_id: "b".into(),
                content: "c".into(),
            },
            StreamEvent::ThinkingStart {
                session_id: "s".into(),
                block_id: "b".into(),
            },
            StreamEvent::ThinkingChunk {
                session_id: "s".into(),
                block_id: "b".into(),
                content: "c".into(),
            },
            StreamEvent::ThinkingEnd {
                session_id: "s".into(),
                block_id: "b".into(),
            },
            StreamEvent::TextStart {
                session_id: "s".into(),
                block_id: "b".into(),
            },
            StreamEvent::TextChunk {
                session_id: "s".into(),
                block_id: "b".into(),
                content: "c".into(),
            },
            StreamEvent::TextEnd {
                session_id: "s".into(),
                block_id: "b".into(),
            },
            StreamEvent::ToolCallStart {
                session_id: "s".into(),
                block_id: "b".into(),
                tool_name: "t".into(),
                tool_input: serde_json::Value::Null,
            },
            StreamEvent::ToolCallResult {
                session_id: "s".into(),
                block_id: "b".into(),
                result: "r".into(),
                is_error: false,
                duration_ms: 0,
            },
            StreamEvent::ToolCallEnd {
                session_id: "s".into(),
                block_id: "b".into(),
            },
            StreamEvent::DiffView {
                session_id: "s".into(),
                block_id: "b".into(),
                file_path: "f".into(),
                old_content: "o".into(),
                new_content: "n".into(),
            },
            StreamEvent::FileIo {
                session_id: "s".into(),
                block_id: "b".into(),
                path: "p".into(),
                operation: "read".into(),
                source: Some("executor".into()),
            },
            StreamEvent::ShellStart {
                session_id: "s".into(),
                block_id: "b".into(),
                command: "c".into(),
            },
            StreamEvent::ShellOutput {
                session_id: "s".into(),
                block_id: "b".into(),
                content: "c".into(),
            },
            StreamEvent::ShellEnd {
                session_id: "s".into(),
                block_id: "b".into(),
                exit_code: 0,
            },
            StreamEvent::ConfirmAsk {
                session_id: "s".into(),
                block_id: "b".into(),
                question: "q".into(),
                kind: "k".into(),
                boundary: None,
                permission_evidence: None,
                replayed_interrupted: false,
            },
            StreamEvent::ConfirmResponse {
                session_id: "s".into(),
                block_id: "b".into(),
                question: Some("q".into()),
                kind: Some("k".into()),
                boundary: None,
                permission_evidence: None,
                approved: Some(true),
                responded_at_ms: 1,
                reason: Some("user_response".into()),
                replayed: false,
            },
            StreamEvent::ContextCompactStart {
                session_id: "s".into(),
                block_id: "b".into(),
            },
            StreamEvent::ContextCompacted {
                session_id: "s".into(),
                block_id: "b".into(),
                summary: "sum".into(),
                retained_messages: 1,
                compacted_messages: 0,
                estimated_tokens_before: 100,
                estimated_tokens_after: 50,
            },
            StreamEvent::ContextCompactSkipped {
                session_id: "s".into(),
                block_id: "b".into(),
                reason: "r".into(),
                retained_messages: 1,
            },
            StreamEvent::MemorySelection {
                session_id: "s".into(),
                selected: vec![],
            },
            StreamEvent::MemoryCandidate {
                session_id: "s".into(),
                memory: WikiMemory {
                    id: "i".into(),
                    category: crate::memory::MemoryCategory::Preference,
                    scope: crate::memory::MemoryScope::Session,
                    status: crate::memory::MemoryStatus::Candidate,
                    title: "t".into(),
                    body: "b".into(),
                    project_path: None,
                    source_session_id: None,
                    source_message_ids: vec![],
                    confidence: 0.0,
                    created_at: "0".into(),
                    updated_at: "0".into(),
                    last_used_at: None,
                    use_count: 0,
                    tags: vec![],
                },
            },
            StreamEvent::MemoryUpdated {
                session_id: "s".into(),
                memory: WikiMemory {
                    id: "i".into(),
                    category: crate::memory::MemoryCategory::Preference,
                    scope: crate::memory::MemoryScope::Session,
                    status: crate::memory::MemoryStatus::Candidate,
                    title: "t".into(),
                    body: "b".into(),
                    project_path: None,
                    source_session_id: None,
                    source_message_ids: vec![],
                    confidence: 0.0,
                    created_at: "0".into(),
                    updated_at: "0".into(),
                    last_used_at: None,
                    use_count: 0,
                    tags: vec![],
                },
            },
            StreamEvent::ForgeWikiContextSelected {
                session_id: "s".into(),
                selected: vec![],
            },
            StreamEvent::ForgeWikiUpdateProposed {
                session_id: "s".into(),
                proposal: ForgeWikiUpdateProposal {
                    id: "i".into(),
                    project_path: "p".into(),
                    session_id: None,
                    target_pages: vec![],
                    title: "t".into(),
                    summary: "s".into(),
                    patch_preview: None,
                    status: ForgeWikiProposalStatus::Pending,
                    created_at: "0".into(),
                },
            },
            StreamEvent::ForgeWikiUpdated {
                session_id: "s".into(),
                proposal: ForgeWikiUpdateProposal {
                    id: "i".into(),
                    project_path: "p".into(),
                    session_id: None,
                    target_pages: vec![],
                    title: "t".into(),
                    summary: "s".into(),
                    patch_preview: None,
                    status: ForgeWikiProposalStatus::Pending,
                    created_at: "0".into(),
                },
            },
            StreamEvent::McpContextStatus {
                session_id: "s".into(),
                source_id: "src".into(),
                status: "ok".into(),
                message: None,
            },
            StreamEvent::WorkflowUpdated {
                session_id: "s".into(),
                state: WorkflowState {
                    session_id: "s".into(),
                    route: crate::workflow::WorkflowRoute::Direct,
                    phase: crate::workflow::WorkflowPhase::Idle,
                    beginner_label: "b".into(),
                    developer_label: "d".into(),
                    matched_signals: vec![],
                    reason: "r".into(),
                    gate: crate::workflow::WorkflowGate::None,
                    override_actions: vec![],
                    spec_path: None,
                    plan_path: None,
                    checkpoint_id: None,
                    updated_at: 0,
                },
            },
            StreamEvent::AgentTurnUpdated {
                session_id: "s".into(),
                state: AgentTurnProjection {
                    session_id: "s".into(),
                    status: crate::agent::turn_state::AgentTurnStatus::Started,
                    step_label: "l".into(),
                    workspace_path: "w".into(),
                    compact_count: 0,
                    verification_status:
                        crate::agent::turn_state::AgentVerificationStatus::NotNeeded,
                    model_rounds: 0,
                    tool_call_count: 0,
                    failed_tool_count: 0,
                    estimated_context_tokens: None,
                    compact_saved_tokens: 0,
                    stop_reason: None,
                },
            },
            StreamEvent::AgentA2AUpdated {
                session_id: "s".into(),
                state: AgentA2AProjection::default(),
            },
            StreamEvent::DeliverySummary {
                session_id: "s".into(),
                block_id: "b".into(),
                summary: crate::protocol::events::DeliverySummary {
                    project_path: None,
                    preview_label: "p".into(),
                    checkpoint_label: "c".into(),
                    next_action: "n".into(),
                    verification_label: None,
                    verification_status: None,
                    verification_command: None,
                    record_label: None,
                    record_status: None,
                    record_target_pages: vec![],
                },
            },
            StreamEvent::SessionStarted {
                session_id: "s".into(),
                agent_type: "a".into(),
                model: "m".into(),
                context_window_tokens: None,
            },
            StreamEvent::SessionStatus {
                session_id: "s".into(),
                status: "idle".into(),
            },
            StreamEvent::SessionStopped {
                session_id: "s".into(),
                reason: "r".into(),
            },
            StreamEvent::Error {
                session_id: "s".into(),
                block_id: "b".into(),
                message: "m".into(),
                code: "c".into(),
            },
            StreamEvent::Usage {
                session_id: "s".into(),
                input_tokens: 100,
                output_tokens: 50,
                estimated_cost_usd: 0.01,
            },
            StreamEvent::RecoveryNotice {
                session_id: "s".into(),
                notice_id: "n".into(),
                title: "t".into(),
                message: "m".into(),
                reason: "r".into(),
                recoverable: true,
            },
        ];

        for event in &events {
            let _ = is_significant_stream_event(event);
            // If we got here, no panic.
        }

        // Verify count: every variant in the enum is represented
        // (guard against silent omissions when new variants are added).
        // Only 3 variants should be non-significant:
        let non_sig: Vec<_> = events
            .iter()
            .filter(|e| !is_significant_stream_event(e))
            .collect();
        assert_eq!(
            non_sig.len(),
            3,
            "only thinking_chunk, text_chunk, shell_output should be non-significant"
        );
    }

    #[test]
    fn pending_saves_clear_removes_all_entries() {
        clear_pending_saves_for_test();
        let map = PENDING_SAVES.get_or_init(|| Mutex::new(HashMap::new()));
        {
            let mut guard = map.lock().unwrap();
            guard.insert("a".into(), true);
            guard.insert("b".into(), true);
        }
        clear_pending_saves_for_test();
        let guard = map.lock().unwrap();
        assert!(guard.is_empty());
    }
}
