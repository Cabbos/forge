use crate::agent::auto_compact::CompactStats;
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

pub(crate) fn session_stopped_event(session_id: &str, reason: &str) -> StreamEvent {
    StreamEvent::SessionStopped {
        session_id: session_id.to_string(),
        reason: reason.to_string(),
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

#[cfg(test)]
mod tests {
    use crate::agent::auto_compact::CompactStats;
    use crate::agent::session_events::{
        agent_turn_updated_event, api_error_event, context_compacted_event, session_stopped_event,
        tool_call_result_event,
    };
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
}
