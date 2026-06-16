use crate::agent::auto_compact::CompactStats;
use crate::agent::snapshot::{ActiveToolCallDescriptor, PendingConfirmDescriptor};
use crate::agent::time::now_ms;
use crate::agent::turn_state::AgentTurnState;
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;

pub(crate) fn agent_turn_updated_event(
    session_id: &str,
    latest_turn: Option<&AgentTurnState>,
) -> Option<StreamEvent> {
    latest_turn.map(|turn| StreamEvent::AgentTurnUpdated {
        session_id: session_id.to_string(),
        state: turn.to_projection(),
    })
}

pub(crate) fn api_error_event(session_id: &str, message: &str) -> StreamEvent {
    StreamEvent::Error {
        session_id: session_id.to_string(),
        block_id: BlockId::new().to_string(),
        message: message.to_string(),
        code: "api_error".to_string(),
    }
}

pub(crate) fn session_status_event(session_id: &str, status: &str) -> StreamEvent {
    StreamEvent::SessionStatus {
        session_id: session_id.to_string(),
        status: status.to_string(),
    }
}

pub(crate) fn session_stopped_event(session_id: &str, reason: &str) -> StreamEvent {
    StreamEvent::SessionStopped {
        session_id: session_id.to_string(),
        reason: reason.to_string(),
    }
}

pub(crate) fn context_compact_start_event(session_id: &str) -> StreamEvent {
    StreamEvent::ContextCompactStart {
        session_id: session_id.to_string(),
        block_id: BlockId::new().to_string(),
    }
}

pub(crate) fn context_compacted_event(session_id: &str, stats: &CompactStats) -> StreamEvent {
    StreamEvent::ContextCompacted {
        session_id: session_id.to_string(),
        block_id: BlockId::new().to_string(),
        summary: stats.summary.clone(),
        retained_messages: stats.retained_messages,
        compacted_messages: stats.compacted_messages,
        estimated_tokens_before: stats.estimated_tokens_before,
        estimated_tokens_after: stats.estimated_tokens_after,
    }
}

pub(crate) fn context_compact_skipped_event(
    session_id: &str,
    reason: &str,
    retained_messages: usize,
) -> StreamEvent {
    StreamEvent::ContextCompactSkipped {
        session_id: session_id.to_string(),
        block_id: BlockId::new().to_string(),
        reason: reason.to_string(),
        retained_messages,
    }
}

pub(crate) fn tool_call_result_event(
    session_id: &str,
    block_id: &str,
    result: &str,
    is_error: bool,
    duration_ms: u64,
) -> StreamEvent {
    StreamEvent::ToolCallResult {
        session_id: session_id.to_string(),
        block_id: block_id.to_string(),
        result: result.to_string(),
        is_error,
        duration_ms,
    }
}

/// Build a replayed/interrupted ConfirmAsk event from a saved descriptor.
/// The frontend will render this as non-interactive (same visual path as
/// `closeInterruptedConfirmBlocks` with reason "session_restored").
pub(crate) fn pending_confirm_replay_event(
    session_id: &str,
    descriptor: &PendingConfirmDescriptor,
) -> StreamEvent {
    StreamEvent::ConfirmAsk {
        session_id: session_id.to_string(),
        block_id: descriptor.block_id.clone(),
        question: descriptor.question.clone(),
        kind: descriptor.kind.clone(),
        boundary: descriptor.boundary.clone(),
        replayed_interrupted: true,
    }
}

/// Build replay events for an active tool call that was interrupted by session
/// restore. Returns a ToolCallStart followed by a ToolCallResult with
/// is_error=true so the frontend renders the tool as completed/interrupted rather
/// than in-progress. The duration_ms is derived from the elapsed wall clock
/// between started_at_ms and now (saturating to 0 when clock is uncertain).
pub(crate) fn active_tool_call_replay_events(
    session_id: &str,
    descriptor: &ActiveToolCallDescriptor,
) -> Vec<StreamEvent> {
    let now = now_ms();
    let duration_ms = now.saturating_sub(descriptor.started_at_ms);
    vec![
        StreamEvent::ToolCallStart {
            session_id: session_id.to_string(),
            block_id: descriptor.block_id.clone(),
            tool_name: descriptor.tool_name.clone(),
            tool_input: descriptor.tool_input.clone(),
        },
        StreamEvent::ToolCallResult {
            session_id: session_id.to_string(),
            block_id: descriptor.block_id.clone(),
            result: "Tool call interrupted by session restore before it returned.".to_string(),
            is_error: true,
            duration_ms,
        },
    ]
}

/// Build a recovery notice event to surface snapshot corruption or restore
/// failures in the UI. The notice is session-scoped when session_id is
/// non-empty, or global when empty, and includes a unique notice_id so the
/// frontend can dismiss individual notices.
pub(crate) fn recovery_notice_event(
    session_id: &str,
    notice_id: &str,
    title: &str,
    message: &str,
    reason: &str,
    recoverable: bool,
) -> StreamEvent {
    StreamEvent::RecoveryNotice {
        session_id: session_id.to_string(),
        notice_id: notice_id.to_string(),
        title: title.to_string(),
        message: message.to_string(),
        reason: reason.to_string(),
        recoverable,
    }
}

#[cfg(test)]
mod tests {
    use crate::agent::auto_compact::CompactStats;
    use crate::agent::session_events::{
        active_tool_call_replay_events, agent_turn_updated_event, api_error_event,
        context_compact_skipped_event, context_compacted_event, pending_confirm_replay_event,
        recovery_notice_event, session_status_event, session_stopped_event, tool_call_result_event,
    };
    use crate::agent::snapshot::{ActiveToolCallDescriptor, PendingConfirmDescriptor};
    use crate::agent::time::now_ms;
    use crate::agent::turn_state::{AgentTurnState, AgentTurnStatus};
    use crate::protocol::events::StreamEvent;

    #[test]
    fn session_event_helpers_build_without_agent_session_or_app_handle() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/tmp/forge-demo".to_string(),
            "deepseek".to_string(),
            "deepseek-chat".to_string(),
            "workflow".to_string(),
            "implementation".to_string(),
            "生成本地小工具".to_string(),
        );
        turn.mark_status_with_reason(AgentTurnStatus::RunningTools, "tool_calls_requested", None);
        let compact_stats = CompactStats {
            summary: "已压缩上下文".to_string(),
            retained_messages: 12,
            compacted_messages: 36,
            estimated_tokens_before: 100_000,
            estimated_tokens_after: 30_000,
        };

        let turn_event = agent_turn_updated_event("session-1", Some(&turn)).expect("turn event");
        let error_event = api_error_event("session-1", "API error: timeout");
        let stopped_event = session_stopped_event("session-1", "killed");
        let compacted_event = context_compacted_event("session-1", &compact_stats);
        let compact_skipped_event =
            context_compact_skipped_event("session-1", "history_too_short", 12);
        let tool_result_event = tool_call_result_event("session-1", "tool-1", "ok", false, 25);

        match turn_event {
            StreamEvent::AgentTurnUpdated { session_id, state } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(state.status, AgentTurnStatus::RunningTools);
                assert_eq!(state.step_label, "处理项目");
            }
            other => panic!("unexpected turn event: {other:?}"),
        }
        match error_event {
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
        match stopped_event {
            StreamEvent::SessionStopped { session_id, reason } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(reason, "killed");
            }
            other => panic!("unexpected stopped event: {other:?}"),
        }
        match compacted_event {
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
                assert_eq!(summary, "已压缩上下文");
                assert_eq!(retained_messages, 12);
                assert_eq!(compacted_messages, 36);
                assert_eq!(estimated_tokens_before, 100_000);
                assert_eq!(estimated_tokens_after, 30_000);
            }
            other => panic!("unexpected compacted event: {other:?}"),
        }
        match compact_skipped_event {
            StreamEvent::ContextCompactSkipped {
                session_id,
                reason,
                retained_messages,
                ..
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(reason, "history_too_short");
                assert_eq!(retained_messages, 12);
            }
            other => panic!("unexpected compact skipped event: {other:?}"),
        }
        match tool_result_event {
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

        assert!(agent_turn_updated_event("session-1", None).is_none());
    }

    #[test]
    fn session_status_event_resuming_and_running() {
        let resuming = session_status_event("session-1", "resuming");
        let running = session_status_event("session-1", "running");
        match resuming {
            StreamEvent::SessionStatus { session_id, status } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(status, "resuming");
            }
            other => panic!("unexpected event: {other:?}"),
        }
        match running {
            StreamEvent::SessionStatus { session_id, status } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(status, "running");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn pending_confirm_replay_event_sets_replayed_interrupted_true() {
        let descriptor = PendingConfirmDescriptor::new(
            "confirm-1".to_string(),
            "Allow write?".to_string(),
            "file_write".to_string(),
            42,
        );
        let event = pending_confirm_replay_event("session-1", &descriptor);
        match event {
            StreamEvent::ConfirmAsk {
                session_id,
                block_id,
                question,
                kind,
                replayed_interrupted,
                ..
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(block_id, "confirm-1");
                assert_eq!(question, "Allow write?");
                assert_eq!(kind, "file_write");
                assert!(replayed_interrupted);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn pending_confirm_replay_event_serializes_without_replayed_interrupted_when_false() {
        // Normal ConfirmAsk (replayed_interrupted: false) should omit the field.
        let normal = StreamEvent::ConfirmAsk {
            session_id: "session-1".to_string(),
            block_id: "confirm-1".to_string(),
            question: "Allow?".to_string(),
            kind: "file_write".to_string(),
            boundary: None,
            replayed_interrupted: false,
        };
        let json = serde_json::to_value(&normal).unwrap();
        assert!(
            json.get("replayed_interrupted").is_none(),
            "normal ConfirmAsk should omit replayed_interrupted"
        );

        // Replayed ConfirmAsk should include the field as true.
        let replay = StreamEvent::ConfirmAsk {
            session_id: "session-1".to_string(),
            block_id: "confirm-1".to_string(),
            question: "Allow?".to_string(),
            kind: "file_write".to_string(),
            boundary: None,
            replayed_interrupted: true,
        };
        let json = serde_json::to_value(&replay).unwrap();
        assert_eq!(
            json.get("replayed_interrupted").and_then(|v| v.as_bool()),
            Some(true),
            "replayed ConfirmAsk should serialize replayed_interrupted: true"
        );
    }

    #[test]
    fn active_tool_call_replay_events_emits_start_and_error_result() {
        let descriptor = ActiveToolCallDescriptor::new(
            "tool-1".to_string(),
            "write_to_file".to_string(),
            serde_json::json!({"path": "file.txt"}),
            100,
        );
        let events = active_tool_call_replay_events("session-1", &descriptor);
        assert_eq!(
            events.len(),
            2,
            "should emit ToolCallStart + ToolCallResult"
        );

        match &events[0] {
            StreamEvent::ToolCallStart {
                session_id,
                block_id,
                tool_name,
                tool_input,
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(block_id, "tool-1");
                assert_eq!(tool_name, "write_to_file");
                assert_eq!(tool_input, &serde_json::json!({"path": "file.txt"}));
            }
            other => panic!("expected ToolCallStart, got {other:?}"),
        }

        match &events[1] {
            StreamEvent::ToolCallResult {
                session_id,
                block_id,
                result,
                is_error,
                duration_ms,
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(block_id, "tool-1");
                assert!(result.contains("interrupted"));
                assert!(result.contains("session restore"));
                assert!(is_error);
                // Duration should be non-zero since started_at_ms is far in the past.
                assert!(*duration_ms > 0, "duration_ms should be positive");
            }
            other => panic!("expected ToolCallResult, got {other:?}"),
        }
    }

    #[test]
    fn recovery_notice_event_has_stable_shape() {
        let notice = recovery_notice_event(
            "session-1",
            "notice-corrupt-1",
            "Session data was unreadable",
            "Forge could not restore your last session because its saved data was corrupted.",
            "snapshot_corrupted",
            true,
        );
        let notice_type = notice.event_type();
        let notice_sid = notice.session_id().to_string();
        match notice {
            StreamEvent::RecoveryNotice {
                session_id,
                notice_id,
                title,
                message,
                reason,
                recoverable,
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(notice_id, "notice-corrupt-1");
                assert_eq!(title, "Session data was unreadable");
                assert!(
                    message.contains("could not restore"),
                    "message should explain the failure"
                );
                assert_eq!(reason, "snapshot_corrupted");
                assert!(recoverable, "should be recoverable when fallback succeeded");
            }
            other => panic!("expected RecoveryNotice, got {other:?}"),
        }
        assert_eq!(notice_type, "recovery_notice");
        assert_eq!(notice_sid, "session-1");
    }

    #[test]
    fn recovery_notice_event_with_empty_session_id_still_valid() {
        let notice = recovery_notice_event(
            "",
            "notice-global",
            "Restore failed",
            "Details.",
            "restore_failed",
            false,
        );
        match notice {
            StreamEvent::RecoveryNotice {
                session_id,
                notice_id,
                recoverable,
                ..
            } => {
                assert_eq!(session_id, "");
                assert_eq!(notice_id, "notice-global");
                assert!(!recoverable);
            }
            other => panic!("expected RecoveryNotice, got {other:?}"),
        }
    }

    #[test]
    fn active_tool_call_replay_events_duration_is_saturating() {
        // When started_at_ms is in the future (clock skew), duration should be 0.
        let future_start = now_ms().saturating_add(60_000);
        let descriptor = ActiveToolCallDescriptor::new(
            "tool-future".to_string(),
            "run_shell".to_string(),
            serde_json::json!({"cmd": "ls"}),
            future_start,
        );
        let events = active_tool_call_replay_events("session-1", &descriptor);
        match &events[1] {
            StreamEvent::ToolCallResult { duration_ms, .. } => {
                assert_eq!(
                    *duration_ms, 0,
                    "duration should saturate to 0 for future start"
                );
            }
            other => panic!("expected ToolCallResult, got {other:?}"),
        }
    }
}
