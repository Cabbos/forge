use super::*;
use crate::adapters::missing_key::MissingKeyAdapter;
use crate::agent::turn_state::{AgentEvidenceKind, AgentToolStatus};

#[test]
fn restore_state_normalizes_interrupted_turn_and_repairs_tool_history() {
    let workspace =
        std::env::temp_dir().join(format!("forge-session-restore-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );
    let messages = vec![
        ChatMessage::user("先安装依赖"),
        ChatMessage::assistant(serde_json::json!([{
            "type": "tool_use",
            "id": "call_1",
            "name": "bash",
            "input": {"command": "npm install"}
        }])),
        ChatMessage::user("继续"),
    ];
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        workspace.to_string_lossy().to_string(),
        "deepseek".to_string(),
        "deepseek-chat".to_string(),
        "workflow".to_string(),
        "implementation".to_string(),
        "安装依赖并继续生成工具".to_string(),
    );
    turn.mark_status_with_reason(
        AgentTurnStatus::RunningTools,
        "tool_calls_requested",
        Some("model requested tool execution"),
    );
    turn.record_tool(running_tool_trace(
        "call_1".to_string(),
        "bash".to_string(),
        &serde_json::json!({"command": "npm install"}),
        10,
    ));

    session.restore_state(
        messages,
        Some("old summary".to_string()),
        Some(turn),
        None,
        None,
    );

    let snapshot = session.snapshot();
    assert_eq!(snapshot.messages.len(), 4);
    assert_eq!(snapshot.messages[2].role, "user");
    assert!(snapshot.messages[2]
        .content
        .to_string()
        .contains("previous tool call was interrupted"));

    let restored_turn = snapshot.latest_turn.expect("latest turn");
    assert_eq!(restored_turn.status, AgentTurnStatus::Cancelled);
    assert_eq!(restored_turn.tools[0].status, AgentToolStatus::Cancelled);
    assert!(restored_turn.tools[0].is_error);
    assert_eq!(
        restored_turn.tools[0].command.as_deref(),
        Some("npm install")
    );
    let evidence = restored_turn
        .evidence
        .iter()
        .find(|item| item.kind == AgentEvidenceKind::Tool && item.tool_call_id == "call_1")
        .expect("cancelled tool evidence");
    assert_eq!(evidence.status, AgentToolStatus::Cancelled);
    assert_eq!(evidence.failure_kind.as_deref(), Some("interrupted"));

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn restore_state_preserves_completed_turn_unchanged() {
    let workspace = std::env::temp_dir().join(format!(
        "forge-session-restore-completed-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-restore-completed".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );

    let messages = vec![
        ChatMessage::user("hello"),
        ChatMessage::assistant(serde_json::json!([{"type": "text", "text": "hi"}])),
    ];
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-restore-completed".to_string(),
        workspace.to_string_lossy().to_string(),
        "deepseek".to_string(),
        "deepseek-chat".to_string(),
        "workflow".to_string(),
        "direct".to_string(),
        "hello".to_string(),
    );
    turn.mark_status_with_reason(AgentTurnStatus::Completed, "final_answer", None);

    session.restore_state(messages, None, Some(turn), None, None);

    let restored_turn = lock_unpoisoned(&session.latest_turn);
    assert_eq!(
        restored_turn.as_ref().unwrap().status,
        AgentTurnStatus::Completed,
        "completed turn should stay completed after restore"
    );
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn restore_state_preserves_cancelled_turn_unchanged() {
    let workspace = std::env::temp_dir().join(format!(
        "forge-session-restore-cancelled-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-restore-cancelled".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );

    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-restore-cancelled".to_string(),
        workspace.to_string_lossy().to_string(),
        "deepseek".to_string(),
        "deepseek-chat".to_string(),
        "workflow".to_string(),
        "direct".to_string(),
        "test".to_string(),
    );
    turn.mark_status_with_reason(AgentTurnStatus::Cancelled, "user_cancelled", Some("killed"));

    session.restore_state(
        vec![ChatMessage::user("test")],
        None,
        Some(turn),
        None,
        None,
    );

    let restored_turn = lock_unpoisoned(&session.latest_turn);
    assert_eq!(
        restored_turn.as_ref().unwrap().status,
        AgentTurnStatus::Cancelled,
        "cancelled turn should stay cancelled after restore"
    );
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn restore_state_with_no_latest_turn_preserves_none() {
    let workspace = std::env::temp_dir().join(format!(
        "forge-session-restore-none-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-restore-none".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );

    session.restore_state(
        vec![ChatMessage::user("hello")],
        Some("summary".to_string()),
        None,
        None,
        None,
    );

    assert!(lock_unpoisoned(&session.latest_turn).is_none());
    assert_eq!(
        *lock_unpoisoned(&session.summary),
        Some("summary".to_string())
    );
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn restore_state_normalizes_only_active_turn_statuses() {
    let workspace = std::env::temp_dir().join(format!(
        "forge-session-restore-active-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));

    for (input_status, expected_status, label) in [
        (
            AgentTurnStatus::Started,
            AgentTurnStatus::Cancelled,
            "Started",
        ),
        (
            AgentTurnStatus::GatheringContext,
            AgentTurnStatus::Cancelled,
            "GatheringContext",
        ),
        (
            AgentTurnStatus::CallingModel,
            AgentTurnStatus::Cancelled,
            "CallingModel",
        ),
        (
            AgentTurnStatus::RunningTools,
            AgentTurnStatus::Cancelled,
            "RunningTools",
        ),
        (
            AgentTurnStatus::Verifying,
            AgentTurnStatus::Cancelled,
            "Verifying",
        ),
        (
            AgentTurnStatus::Completed,
            AgentTurnStatus::Completed,
            "Completed",
        ),
        (
            AgentTurnStatus::Cancelled,
            AgentTurnStatus::Cancelled,
            "Cancelled",
        ),
        (AgentTurnStatus::Failed, AgentTurnStatus::Failed, "Failed"),
    ] {
        let session = AgentSession::new(
            format!("session-restore-{label}"),
            "deepseek".to_string(),
            adapter.clone(),
            Arc::new(Harness::new(workspace.clone())),
            "system".to_string(),
            Some(128_000),
        );

        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            format!("session-restore-{label}"),
            workspace.to_string_lossy().to_string(),
            "deepseek".to_string(),
            "deepseek-chat".to_string(),
            "workflow".to_string(),
            "direct".to_string(),
            "test".to_string(),
        );
        turn.mark_status_with_reason(input_status.clone(), "test_reason", None);

        session.restore_state(vec![ChatMessage::user("x")], None, Some(turn), None, None);

        let restored = lock_unpoisoned(&session.latest_turn);
        assert_eq!(
            restored.as_ref().unwrap().status,
            expected_status,
            "{label}: {input_status:?} should become {expected_status:?}"
        );
    }

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn latest_turn_updated_event_can_be_built_without_app_handle() {
    let workspace =
        std::env::temp_dir().join(format!("forge-session-turn-event-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        workspace.to_string_lossy().to_string(),
        "deepseek".to_string(),
        "deepseek-chat".to_string(),
        "workflow".to_string(),
        "implementation".to_string(),
        "生成一个本地小工具".to_string(),
    );
    turn.mark_status_with_reason(AgentTurnStatus::CallingModel, "call_model", None);
    *lock_unpoisoned(&session.latest_turn) = Some(turn);

    let event = session
        .latest_turn_updated_event()
        .expect("latest turn event");

    match event {
        StreamEvent::AgentTurnUpdated { session_id, state } => {
            assert_eq!(session_id, "session-1");
            assert_eq!(state.status, AgentTurnStatus::CallingModel);
            assert_eq!(state.step_label, "请求模型");
            assert_eq!(
                std::path::PathBuf::from(state.workspace_path)
                    .canonicalize()
                    .expect("projection workspace"),
                workspace.canonicalize().expect("workspace")
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn lifecycle_events_can_be_built_without_app_handle() {
    let workspace = std::env::temp_dir().join(format!(
        "forge-session-lifecycle-events-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );
    let compact_stats = CompactStats {
        summary: "保留最近上下文".to_string(),
        retained_messages: 16,
        compacted_messages: 48,
        estimated_tokens_before: 120_000,
        estimated_tokens_after: 42_000,
    };

    let error = session.api_error_event("API error: timeout".to_string());
    let stopped = session.session_stopped_event("killed");
    let compacted = session.context_compacted_event(&compact_stats);
    let tool_result = session.tool_call_result_event("tool-1", "ok", false, 25);

    match error {
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
    match stopped {
        StreamEvent::SessionStopped { session_id, reason } => {
            assert_eq!(session_id, "session-1");
            assert_eq!(reason, "killed");
        }
        other => panic!("unexpected stopped event: {other:?}"),
    }
    match compacted {
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
            assert_eq!(summary, "保留最近上下文");
            assert_eq!(retained_messages, 16);
            assert_eq!(compacted_messages, 48);
            assert_eq!(estimated_tokens_before, 120_000);
            assert_eq!(estimated_tokens_after, 42_000);
        }
        other => panic!("unexpected compacted event: {other:?}"),
    }
    match tool_result {
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

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn manual_compact_updates_session_history_summary_and_emits_event() {
    let workspace = std::env::temp_dir().join(format!(
        "forge-session-manual-compact-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-manual-compact".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );
    {
        let mut messages = lock_unpoisoned(&session.messages);
        for index in 0..40 {
            if index % 2 == 0 {
                messages.push(ChatMessage::user(&format!("user message {index}")));
            } else {
                messages.push(ChatMessage::assistant(serde_json::Value::String(format!(
                    "assistant message {index}"
                ))));
            }
        }
    }
    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();

    let result = session
        .compact_now_with_emitter(&emitter)
        .await
        .expect("manual compact should be handled");

    assert!(result.compacted);
    assert_eq!(result.compacted_messages, 8);
    assert_eq!(lock_unpoisoned(&session.messages).len(), 32);
    assert!(lock_unpoisoned(&session.summary).is_some());
    let events = emitter.drain();
    assert!(events.iter().any(|event| matches!(
        event,
        StreamEvent::ContextCompacted {
            session_id,
            compacted_messages: 8,
            retained_messages: 32,
            ..
        } if session_id == "session-manual-compact"
    )));

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn manual_compact_uses_model_generated_summary_when_adapter_available() {
    let workspace = std::env::temp_dir().join(format!(
        "forge-session-manual-model-compact-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(FakeAdapter::new(vec![StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "<analysis>scratch space that must not be stored</analysis>\n<summary>\nMODEL GENERATED SUMMARY\n</summary>",
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    }]));
    let session = AgentSession::new(
        "session-manual-model-compact".to_string(),
        "deepseek".to_string(),
        adapter.clone(),
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );
    {
        let mut messages = lock_unpoisoned(&session.messages);
        for index in 0..40 {
            if index % 2 == 0 {
                messages.push(ChatMessage::user(&format!("user message {index}")));
            } else {
                messages.push(ChatMessage::assistant(serde_json::Value::String(format!(
                    "assistant message {index}"
                ))));
            }
        }
    }
    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();

    let result = session
        .compact_now_with_emitter(&emitter)
        .await
        .expect("manual compact should be handled");

    assert!(result.compacted);
    assert_eq!(
        adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "manual compact should call the adapter to generate a semantic summary"
    );
    let summary = lock_unpoisoned(&session.summary)
        .clone()
        .expect("summary should be persisted");
    assert!(summary.contains("MODEL GENERATED SUMMARY"));
    assert!(
        !summary.contains("<analysis>"),
        "scratch analysis tags should be stripped before persisting the summary"
    );
    assert_eq!(lock_unpoisoned(&session.messages).len(), 32);

    let events = emitter.drain();
    assert!(events.iter().any(|event| matches!(
        event,
        StreamEvent::ContextCompacted {
            session_id,
            summary,
            compacted_messages: 8,
            retained_messages: 32,
            ..
        } if session_id == "session-manual-model-compact"
            && summary.contains("MODEL GENERATED SUMMARY")
            && !summary.contains("<analysis>")
    )));

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn manual_compact_short_history_emits_skipped_event() {
    let workspace = std::env::temp_dir().join(format!(
        "forge-session-manual-compact-skipped-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-manual-compact-skipped".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );
    {
        let mut messages = lock_unpoisoned(&session.messages);
        for index in 0..12 {
            if index % 2 == 0 {
                messages.push(ChatMessage::user(&format!("user message {index}")));
            } else {
                messages.push(ChatMessage::assistant(serde_json::Value::String(format!(
                    "assistant message {index}"
                ))));
            }
        }
    }
    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();

    let result = session
        .compact_now_with_emitter(&emitter)
        .await
        .expect("manual compact should return skipped result");

    assert!(!result.compacted);
    assert_eq!(result.skipped_reason.as_deref(), Some("history_too_short"));
    let events = emitter.drain();
    assert!(events.iter().any(|event| matches!(
        event,
        StreamEvent::ContextCompactSkipped {
            session_id,
            reason,
            retained_messages: 12,
            ..
        } if session_id == "session-manual-compact-skipped" && reason == "history_too_short"
    )));

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn manual_compact_emits_start_event_before_processing() {
    let workspace = std::env::temp_dir().join(format!(
        "forge-session-manual-compact-start-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-manual-compact-start".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );
    {
        let mut messages = lock_unpoisoned(&session.messages);
        for index in 0..40 {
            if index % 2 == 0 {
                messages.push(ChatMessage::user(&format!("user message {index}")));
            } else {
                messages.push(ChatMessage::assistant(serde_json::Value::String(format!(
                    "assistant message {index}"
                ))));
            }
        }
    }
    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();

    let result = session
        .compact_now_with_emitter(&emitter)
        .await
        .expect("manual compact should be handled");

    assert!(result.compacted);
    let events = emitter.drain();

    // The compact_start event must be emitted BEFORE the compacted event
    let start_index = events.iter().position(|event| {
        matches!(
            event,
            StreamEvent::ContextCompactStart { session_id, .. }
                if session_id == "session-manual-compact-start"
        )
    });
    let compacted_index = events.iter().position(|event| {
        matches!(
            event,
            StreamEvent::ContextCompacted { session_id, .. }
                if session_id == "session-manual-compact-start"
        )
    });

    assert!(
        start_index.is_some(),
        "should emit context_compact_start event"
    );
    assert!(
        compacted_index.is_some(),
        "should emit context_compacted event"
    );
    assert!(
        start_index.unwrap() < compacted_index.unwrap(),
        "context_compact_start must come before context_compacted"
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn agent_turn_stops_with_model_round_limit_when_max_rounds_hit() {
    let workspace = setup_test_workspace("forge-model-round-limit");
    let harness = Arc::new(Harness::new(workspace.clone()));

    // Adapter always returns a tool_use, so the loop would continue forever
    // without the guard.  We set max_model_rounds = 2.
    let tool_response = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "tool_use",
            "id": "call-1",
            "name": "read_file",
            "input": {"path": "data.txt"}
        })],
        tool_calls: vec![ToolCall {
            id: "call-1".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "data.txt"}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    // Need 2 model responses + 1 final summary response.
    let adapter = Arc::new(FakeAdapter::new(vec![
        tool_response.clone(),
        tool_response.clone(),
        StreamResult {
            assistant_content: vec![serde_json::json!({"type": "text", "text": "done"})],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        },
    ]));

    let session = AgentSession::new(
        "session-round-limit".to_string(),
        "deepseek".to_string(),
        adapter.clone(),
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    // Override the loop guard to a very low limit for testing.
    *lock_unpoisoned(&session.loop_guard) =
        crate::agent::loop_guard::LoopGuard::default_limits().with_max_model_rounds(2);

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    let turn_guard = session.reserve_turn().expect("reserve turn");

    session
        .send_message_with_emitter(
            "keep calling tools",
            &emitter,
            vec![],
            None,
            None,
            turn_guard,
        )
        .await
        .expect("turn should finish gracefully when limit hit");

    // Verify the adapter was called 2 times in the loop + 1 final summary.
    assert_eq!(
        adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
        3,
        "should stop after 2 model rounds, then finalize_turn calls adapter once more"
    );

    // Verify the turn has the correct stop_reason.
    let turn = lock_unpoisoned(&session.latest_turn);
    let turn = turn.as_ref().expect("latest turn");
    assert_eq!(
        turn.stop_reason,
        Some("model_round_limit".to_string()),
        "stop_reason should be model_round_limit"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn agent_turn_stops_with_tool_loop_detected_when_same_tool_repeats() {
    let workspace = setup_test_workspace("forge-tool-loop-detected");
    let harness = Arc::new(Harness::new(workspace.clone()));

    let repeated_read = |id: &str| StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "tool_use",
            "id": id,
            "name": "read_file",
            "input": {"path": "src/main.rs"}
        })],
        tool_calls: vec![ToolCall {
            id: id.to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "src/main.rs"}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new(vec![
        repeated_read("call-1"),
        repeated_read("call-2"),
        repeated_read("call-3"),
        repeated_read("call-4"),
        StreamResult {
            assistant_content: vec![serde_json::json!({"type": "text", "text": "done"})],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        },
    ]));

    let session = AgentSession::new(
        "session-tool-loop-detected".to_string(),
        "deepseek".to_string(),
        adapter.clone(),
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    let turn_guard = session.reserve_turn().expect("reserve turn");

    session
        .send_message_with_emitter(
            "keep reading the same file",
            &emitter,
            vec![],
            None,
            None,
            turn_guard,
        )
        .await
        .expect("turn should finish gracefully when tool loop is detected");

    let turn = lock_unpoisoned(&session.latest_turn);
    let turn = turn.as_ref().expect("latest turn");
    assert_eq!(
        turn.stop_reason,
        Some("tool_loop_detected".to_string()),
        "same tool and same input should stop before another model round"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn agent_turn_stops_with_repeated_no_progress_when_tools_keep_failing() {
    let workspace = setup_test_workspace("forge-repeated-no-progress");
    let harness = Arc::new(Harness::new(workspace.clone()));

    let missing_read = |id: &str, path: &str| StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "tool_use",
            "id": id,
            "name": "read_file",
            "input": {"path": path}
        })],
        tool_calls: vec![ToolCall {
            id: id.to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": path}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new(vec![
        missing_read("call-1", "missing-1.txt"),
        missing_read("call-2", "missing-2.txt"),
        missing_read("call-3", "missing-3.txt"),
        missing_read("call-4", "missing-4.txt"),
        StreamResult {
            assistant_content: vec![serde_json::json!({"type": "text", "text": "done"})],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        },
    ]));

    let session = AgentSession::new(
        "session-repeated-no-progress".to_string(),
        "deepseek".to_string(),
        adapter.clone(),
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    let turn_guard = session.reserve_turn().expect("reserve turn");

    session
        .send_message_with_emitter(
            "keep reading missing files",
            &emitter,
            vec![],
            None,
            None,
            turn_guard,
        )
        .await
        .expect("turn should finish gracefully when no progress repeats");

    let turn = lock_unpoisoned(&session.latest_turn);
    let turn = turn.as_ref().expect("latest turn");
    assert_eq!(
        turn.stop_reason,
        Some("repeated_no_progress".to_string()),
        "consecutive failed tool batches should stop as no progress"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn loop_guard_stop_records_recovery_trace_for_repeated_category_batch() {
    let workspace = setup_test_workspace("forge-repeated-category-recovery");
    let harness = Arc::new(Harness::new(workspace.clone()));

    let varied_read = |id: &str, path: &str| StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "tool_use",
            "id": id,
            "name": "read_file",
            "input": {"path": path}
        })],
        tool_calls: vec![ToolCall {
            id: id.to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": path}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new(vec![
        varied_read("call-1", "first.txt"),
        varied_read("call-2", "second.txt"),
        varied_read("call-3", "third.txt"),
        StreamResult {
            assistant_content: vec![serde_json::json!({"type": "text", "text": "done"})],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        },
    ]));

    let session = AgentSession::new(
        "session-repeated-category-recovery".to_string(),
        "deepseek".to_string(),
        adapter,
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    *lock_unpoisoned(&session.loop_guard) = crate::agent::loop_guard::LoopGuard::default_limits()
        .with_max_repeated_category_batches(3)
        .with_max_repeated_tool_batches(100);

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    let turn_guard = session.reserve_turn().expect("reserve turn");

    session
        .send_message_with_emitter(
            "keep reading related files",
            &emitter,
            vec![],
            None,
            None,
            turn_guard,
        )
        .await
        .expect("turn should finish gracefully when category loop is detected");

    let turn = lock_unpoisoned(&session.latest_turn);
    let turn = turn.as_ref().expect("latest turn");
    assert_eq!(
        turn.stop_reason,
        Some("repeated_category_batch".to_string()),
        "turn should preserve the machine-readable guard stop reason"
    );

    let recovery_transition = turn
        .transition_log
        .iter()
        .find(|transition| transition.reason == "loop_guard_stopped")
        .expect("should record a loop guard recovery transition");
    let detail = recovery_transition
        .detail
        .as_deref()
        .expect("loop guard transition should explain recovery");
    assert!(
        detail.contains("repeated_category_batch"),
        "detail should include the stop reason: {detail}"
    );
    assert!(
        detail.contains("smaller next action"),
        "detail should suggest a smaller next action: {detail}"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn tool_batch_signature_ignores_call_id_and_object_key_order() {
    let first = vec![ToolCall {
        id: "call-1".to_string(),
        name: "run_shell".to_string(),
        input: serde_json::json!({"command": "npm test", "cwd": "."}),
    }];
    let second = vec![ToolCall {
        id: "call-2".to_string(),
        name: "run_shell".to_string(),
        input: serde_json::json!({"cwd": ".", "command": "npm test"}),
    }];

    assert_eq!(tool_batch_signature(&first), tool_batch_signature(&second));
}

#[test]
fn tool_category_signature_ignores_input_differences() {
    let read_a = vec![ToolCall {
        id: "call-1".to_string(),
        name: "read_file".to_string(),
        input: serde_json::json!({"path": "a.txt"}),
    }];
    let read_b = vec![ToolCall {
        id: "call-2".to_string(),
        name: "read_file".to_string(),
        input: serde_json::json!({"path": "b.txt"}),
    }];
    let mixed = vec![
        ToolCall {
            id: "call-3".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "c.txt"}),
        },
        ToolCall {
            id: "call-4".to_string(),
            name: "list_directory".to_string(),
            input: serde_json::json!({"path": "src"}),
        },
    ];

    assert_eq!(tool_category_signature(&read_a), "read_file");
    assert_eq!(
        tool_category_signature(&read_a),
        tool_category_signature(&read_b)
    );
    assert_eq!(tool_category_signature(&mixed), "list_directory,read_file");
}

#[tokio::test]
async fn loop_guard_resets_between_turns_so_budget_does_not_accumulate() {
    let workspace = setup_test_workspace("forge-loop-guard-reset");
    let harness = Arc::new(Harness::new(workspace.clone()));

    let tool_response = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "tool_use",
            "id": "call-1",
            "name": "read_file",
            "input": {"path": "data.txt"}
        })],
        tool_calls: vec![ToolCall {
            id: "call-1".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "data.txt"}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    // Sequence:
    //   Turn 1 round 1: tool_use
    //   Turn 1 round 2: tool_use (hits 2-round limit)
    //   Turn 1 final summary: text
    //   Turn 2 round 1: text (no tool calls — normal completion)
    let adapter = Arc::new(FakeAdapter::new(vec![
        tool_response.clone(),
        tool_response.clone(),
        StreamResult {
            assistant_content: vec![serde_json::json!({"type": "text", "text": "done"})],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        },
        StreamResult {
            assistant_content: vec![serde_json::json!({"type": "text", "text": "second done"})],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        },
    ]));

    let session = AgentSession::new(
        "session-loop-guard-reset".to_string(),
        "deepseek".to_string(),
        adapter.clone(),
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    // Override the loop guard to a very low limit for testing.
    *lock_unpoisoned(&session.loop_guard) =
        crate::agent::loop_guard::LoopGuard::default_limits().with_max_model_rounds(2);

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();

    // First turn — hits the 2-round limit.
    let turn_guard_1 = session.reserve_turn().expect("reserve turn 1");
    session
        .send_message_with_emitter(
            "keep calling tools",
            &emitter,
            vec![],
            None,
            None,
            turn_guard_1,
        )
        .await
        .expect("first turn should finish");

    {
        let turn_1 = lock_unpoisoned(&session.latest_turn);
        let turn_1 = turn_1.as_ref().expect("latest turn after first");
        assert_eq!(
            turn_1.stop_reason,
            Some("model_round_limit".to_string()),
            "first turn should hit model_round_limit"
        );
    }

    // Second turn — if loop_guard is NOT reset, this will immediately fail
    // with model_round_limit because the counter from turn 1 is still there.
    let turn_guard_2 = session.reserve_turn().expect("reserve turn 2");
    session
        .send_message_with_emitter("second message", &emitter, vec![], None, None, turn_guard_2)
        .await
        .expect("second turn should finish, not be blocked by accumulated budget");

    let turn_2 = lock_unpoisoned(&session.latest_turn);
    let turn_2 = turn_2.as_ref().expect("latest turn after second");
    assert_ne!(
        turn_2.stop_reason,
        Some("model_round_limit".to_string()),
        "second turn should NOT hit model_round_limit; loop_guard must reset between turns"
    );
    assert_eq!(
        turn_2.stop_reason, None,
        "second turn should complete normally without any stop reason"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

// ── FakeAdapter for full-turn testing ─────────────────────────

use crate::adapters::base::{AdapterError, AiAdapter, StreamResult, ToolCall};
use crate::agent::event_sink::EventEmitter;

/// Scriptable adapter that returns a pre-defined sequence of `StreamResult`s.
/// Thread-safe: uses `AtomicUsize` for call counting.
struct FakeAdapter {
    results: Vec<Result<StreamResult, String>>,
    call_count: std::sync::atomic::AtomicUsize,
    model_id: String,
}

impl FakeAdapter {
    fn new(responses: Vec<StreamResult>) -> Self {
        Self {
            results: responses.into_iter().map(Ok).collect(),
            call_count: std::sync::atomic::AtomicUsize::new(0),
            model_id: "fake-model".to_string(),
        }
    }

    fn new_with_errors(errors: Vec<Result<StreamResult, String>>) -> Self {
        Self {
            results: errors,
            call_count: std::sync::atomic::AtomicUsize::new(0),
            model_id: "fake-model".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl AiAdapter for FakeAdapter {
    async fn stream_message(
        &self,
        _session_id: &str,
        _messages: &[crate::adapters::base::ChatMessage],
        _app_handle: &tauri::AppHandle,
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        panic!("FakeAdapter::stream_message should not be called in tests — use call_with_emitter");
    }

    async fn call(
        &self,
        _messages: &[crate::adapters::base::ChatMessage],
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        let idx = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match self.results.get(idx) {
            Some(Ok(r)) => Ok(r.clone()),
            Some(Err(msg)) => Err(AdapterError::Stream(msg.clone())),
            None => Err(AdapterError::Stream(format!(
                "FakeAdapter: no response at index {idx}"
            ))),
        }
    }

    async fn call_with_emitter(
        &self,
        _session_id: &str,
        _messages: &[crate::adapters::base::ChatMessage],
        _emitter: &dyn EventEmitter,
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        self.call(_messages, cancel).await
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn model_name(&self) -> &str {
        "Fake Model"
    }
}

struct StreamingFakeAdapter {
    inner: FakeAdapter,
}

impl StreamingFakeAdapter {
    fn new(responses: Vec<StreamResult>) -> Self {
        Self {
            inner: FakeAdapter::new(responses),
        }
    }
}

#[async_trait::async_trait]
impl AiAdapter for StreamingFakeAdapter {
    async fn stream_message(
        &self,
        _session_id: &str,
        _messages: &[crate::adapters::base::ChatMessage],
        _app_handle: &tauri::AppHandle,
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        panic!("StreamingFakeAdapter::stream_message should not be called in tests");
    }

    async fn call(
        &self,
        messages: &[crate::adapters::base::ChatMessage],
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        self.inner.call(messages, cancel).await
    }

    async fn call_with_emitter(
        &self,
        session_id: &str,
        messages: &[crate::adapters::base::ChatMessage],
        emitter: &dyn EventEmitter,
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        let result = self.call(messages, cancel).await?;
        for tool_call in &result.tool_calls {
            emitter.emit(StreamEvent::ToolCallStart {
                session_id: session_id.to_string(),
                block_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                tool_input: tool_call.input.clone(),
            });
            emitter.emit(StreamEvent::ToolCallEnd {
                session_id: session_id.to_string(),
                block_id: tool_call.id.clone(),
            });
        }
        Ok(result)
    }

    fn model_id(&self) -> &str {
        self.inner.model_id()
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }
}

struct UsageEmittingFakeAdapter {
    inner: FakeAdapter,
    usage_by_call: Vec<Option<(u32, u32)>>,
}

impl UsageEmittingFakeAdapter {
    fn new(responses: Vec<StreamResult>, usage_by_call: Vec<Option<(u32, u32)>>) -> Self {
        Self {
            inner: FakeAdapter::new(responses),
            usage_by_call,
        }
    }
}

#[async_trait::async_trait]
impl AiAdapter for UsageEmittingFakeAdapter {
    async fn stream_message(
        &self,
        _session_id: &str,
        _messages: &[crate::adapters::base::ChatMessage],
        _app_handle: &tauri::AppHandle,
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        panic!("UsageEmittingFakeAdapter::stream_message should not be called in tests");
    }

    async fn call(
        &self,
        messages: &[crate::adapters::base::ChatMessage],
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        self.inner.call(messages, cancel).await
    }

    async fn call_with_emitter(
        &self,
        session_id: &str,
        messages: &[crate::adapters::base::ChatMessage],
        emitter: &dyn EventEmitter,
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        let result = self.inner.call(messages, cancel).await?;
        let call_index = self
            .inner
            .call_count
            .load(std::sync::atomic::Ordering::SeqCst)
            .saturating_sub(1);
        if let Some(Some((input_tokens, output_tokens))) = self.usage_by_call.get(call_index) {
            emitter.emit(StreamEvent::Usage {
                session_id: session_id.to_string(),
                input_tokens: *input_tokens,
                output_tokens: *output_tokens,
                estimated_cost_usd: 0.0,
            });
        }
        Ok(result)
    }

    fn model_id(&self) -> &str {
        self.inner.model_id()
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }
}

struct StreamingUsageOnlyFakeAdapter {
    response: StreamResult,
}

#[async_trait::async_trait]
impl AiAdapter for StreamingUsageOnlyFakeAdapter {
    async fn stream_message(
        &self,
        _session_id: &str,
        _messages: &[crate::adapters::base::ChatMessage],
        _app_handle: &tauri::AppHandle,
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        panic!("test should use stream_message_with_emitter, not AppHandle streaming");
    }

    async fn stream_message_with_emitter(
        &self,
        session_id: &str,
        _messages: &[crate::adapters::base::ChatMessage],
        emitter: &dyn EventEmitter,
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        emitter.emit(StreamEvent::Usage {
            session_id: session_id.to_string(),
            input_tokens: 345,
            output_tokens: 67,
            estimated_cost_usd: 0.0,
        });
        Ok(self.response.clone())
    }

    async fn call(
        &self,
        _messages: &[crate::adapters::base::ChatMessage],
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        panic!("test should use streaming path, not non-streaming call");
    }

    async fn call_with_emitter(
        &self,
        _session_id: &str,
        _messages: &[crate::adapters::base::ChatMessage],
        _emitter: &dyn EventEmitter,
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        panic!("test should use stream_message_with_emitter, not call_with_emitter");
    }

    fn model_id(&self) -> &str {
        "streaming-usage-only"
    }

    fn model_name(&self) -> &str {
        "Streaming Usage Only"
    }
}

/// Build a test workspace with a known file and return the path.
fn setup_test_workspace(prefix: &str) -> std::path::PathBuf {
    let workspace = std::env::temp_dir().join(format!("{}-{}", prefix, uuid::Uuid::now_v7()));
    std::fs::create_dir_all(workspace.join("src")).expect("create workspace");
    std::fs::write(
        workspace.join("src").join("main.rs"),
        "fn main() { println!(\"hello world\"); }\n",
    )
    .expect("write test file");
    workspace
}

fn setup_git_test_workspace(prefix: &str) -> std::path::PathBuf {
    let workspace = setup_test_workspace(prefix);
    let init = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&workspace)
        .output()
        .expect("git init");
    assert!(init.status.success(), "git init failed: {init:?}");

    let add = std::process::Command::new("git")
        .args(["add", "src/main.rs"])
        .current_dir(&workspace)
        .output()
        .expect("git add");
    assert!(add.status.success(), "git add failed: {add:?}");

    let commit = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=Forge Test",
            "-c",
            "user.email=forge-test@example.com",
            "commit",
            "-m",
            "init",
            "--no-gpg-sign",
        ])
        .current_dir(&workspace)
        .output()
        .expect("git commit");
    assert!(commit.status.success(), "git commit failed: {commit:?}");

    workspace
}

#[tokio::test]
async fn agent_turn_records_usage_from_streaming_event_sink() {
    let workspace = setup_test_workspace("forge-streaming-turn-metrics");
    let response = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "streamed"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };
    let session = AgentSession::new(
        "session-streaming-turn-metrics".to_string(),
        "deepseek".to_string(),
        Arc::new(StreamingUsageOnlyFakeAdapter { response }),
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    let turn_guard = session.reserve_turn().expect("reserve turn");
    session
        .send_message_with_emitter("stream usage", &emitter, vec![], None, None, turn_guard)
        .await
        .expect("turn should complete");

    let metrics = session.latest_turn_usage_snapshot();
    assert_eq!(metrics.provider_input_tokens, Some(345));
    assert_eq!(metrics.provider_output_tokens, Some(67));

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn agent_turn_records_context_usage_and_compaction_metrics() {
    let workspace = setup_test_workspace("forge-turn-metrics");
    let session_id = "session-turn-metrics".to_string();
    let compact_summary_response = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "<summary>Older task history was compacted.</summary>"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };
    let final_response = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "done"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };
    let adapter = Arc::new(UsageEmittingFakeAdapter::new(
        vec![compact_summary_response, final_response],
        vec![None, Some((1234, 56))],
    ));
    let session = AgentSession::new(
        session_id,
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );
    let mut history = Vec::new();
    for index in 0..90 {
        if index % 2 == 0 {
            history.push(ChatMessage::user(&format!("user history {index}")));
        } else {
            history.push(ChatMessage::assistant(serde_json::Value::String(format!(
                "assistant history {index}"
            ))));
        }
    }
    session.restore_state(history, None, None, None, None);

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    let turn_guard = session.reserve_turn().expect("reserve turn");
    session
        .send_message_with_emitter("continue", &emitter, vec![], None, None, turn_guard)
        .await
        .expect("turn should complete");

    let metrics = session.latest_turn_usage_snapshot();
    assert_eq!(metrics.provider_input_tokens, Some(1234));
    assert_eq!(metrics.provider_output_tokens, Some(56));
    assert_eq!(metrics.compact_count, 1);
    assert!(metrics.compact_saved_tokens > 0);
    assert!(metrics
        .estimated_context_tokens_before_model_call
        .is_some_and(|tokens| tokens > 0));

    // Verify compact_saved_tokens is projected to frontend events
    let projection = session
        .latest_turn_updated_event()
        .and_then(|e| match e {
            crate::protocol::events::StreamEvent::AgentTurnUpdated { state, .. } => Some(state),
            _ => None,
        })
        .expect("turn updated event with projection");
    assert_eq!(
        projection.compact_saved_tokens,
        metrics.compact_saved_tokens
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn full_agent_turn_fake_adapter_preserves_tool_result_order_and_history() {
    // This test proves the complete agent turn loop works without a real API:
    //   user input → fake adapter returns tool_call → harness executes read_file →
    //   tool_result assembled in original order → fake adapter returns final text →
    //   turn completes with correct message history.
    let workspace = setup_test_workspace("forge-full-turn");
    let harness = Arc::new(Harness::new(workspace.clone()));
    let session_id = "session-full-turn".to_string();

    // Response 1: model wants to read a file
    // assistant_content must include the tool_use block (matches real adapter behavior)
    let tool_call_id = "call-read-1".to_string();
    let response_1 = StreamResult {
        assistant_content: vec![
            serde_json::json!({
                "type": "text",
                "text": "让我先查看源码"
            }),
            serde_json::json!({
                "type": "tool_use",
                "id": tool_call_id.clone(),
                "name": "read_file",
                "input": {"path": "src/main.rs"}
            }),
        ],
        tool_calls: vec![ToolCall {
            id: tool_call_id.clone(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "src/main.rs"}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    // Response 2: model returns final text after seeing tool result
    let response_2 = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "这是一个 hello world 程序"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new(vec![response_1, response_2]));
    let session = AgentSession::new(
        session_id.clone(),
        "deepseek".to_string(),
        adapter.clone(),
        harness.clone(),
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    let emitter = crate::agent::event_sink::NoopEventEmitter;
    let turn_guard = session.reserve_turn().expect("reserve turn");

    let result = session
        .send_message_with_emitter(
            "帮我看看 src/main.rs 的内容",
            &emitter,
            vec![],
            None,
            None,
            turn_guard,
        )
        .await;

    assert!(
        result.is_ok(),
        "agent turn should succeed: {:?}",
        result.err()
    );

    // Verify message history structure
    let messages = lock_unpoisoned(&session.messages);
    // Expected: user, assistant(tool_use), user(tool_result), assistant(final text)
    // The summary request may add one more assistant message
    assert!(
        messages.len() >= 4,
        "expected at least 4 messages, got {}",
        messages.len()
    );

    // 1. User message
    assert_eq!(messages[0].role, "user");
    assert!(messages[0]
        .content
        .as_str()
        .unwrap_or_default()
        .contains("src/main.rs"));

    // 2. Assistant with tool_use
    assert_eq!(messages[1].role, "assistant");
    let assistant_blocks = messages[1]
        .content
        .as_array()
        .expect("assistant content blocks");
    let tool_use_block = assistant_blocks
        .iter()
        .find(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
        .expect("assistant should have tool_use block");
    assert_eq!(
        tool_use_block.get("id").and_then(|v| v.as_str()),
        Some(tool_call_id.as_str())
    );
    assert_eq!(
        tool_use_block.get("name").and_then(|v| v.as_str()),
        Some("read_file")
    );

    // 3. User with tool_result — must follow immediately after assistant tool_use
    assert_eq!(messages[2].role, "user");
    let result_blocks = messages[2].content.as_array().expect("tool result blocks");
    assert_eq!(
        result_blocks.len(),
        1,
        "expected exactly 1 tool_result block"
    );
    assert_eq!(
        result_blocks[0].get("type").and_then(|v| v.as_str()),
        Some("tool_result")
    );
    assert_eq!(
        result_blocks[0].get("tool_use_id").and_then(|v| v.as_str()),
        Some(tool_call_id.as_str()),
        "tool_result must reference the original tool_use id"
    );
    // Tool result should contain the file content
    let result_content = result_blocks[0]
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        result_content.contains("hello world"),
        "tool result should contain file content, got: {}",
        result_content
    );

    // 4. Assistant final text
    let final_msg = messages.iter().rev().find(|m| m.role == "assistant");
    let final_text = final_msg
        .and_then(|m| m.content.as_array())
        .and_then(|blocks| {
            blocks.iter().find_map(|b| {
                if b.get("type").and_then(|v| v.as_str()) == Some("text") {
                    b.get("text").and_then(|v| v.as_str()).map(String::from)
                } else {
                    None
                }
            })
        })
        .expect("final assistant text");
    assert!(
        final_text.contains("hello world"),
        "final text should reference the file content"
    );

    // Verify adapter was called exactly 2 times (tool round + final summary)
    assert_eq!(
        adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "adapter should be called exactly 2 times"
    );

    // Verify turn state
    let turn = lock_unpoisoned(&session.latest_turn);
    let turn = turn.as_ref().expect("latest turn should exist");
    assert_eq!(
        turn.status,
        AgentTurnStatus::Completed,
        "turn should be completed"
    );

    // Verify workspace is preserved in turn metadata
    assert!(
        turn.session_id == session_id,
        "turn should reference the correct session"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

struct AutoApprovePendingEmitter {
    pending_confirms: Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    >,
}

impl AutoApprovePendingEmitter {
    fn new(
        pending_confirms: Arc<
            tokio::sync::RwLock<
                std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>,
            >,
        >,
    ) -> Self {
        Self { pending_confirms }
    }
}

impl EventEmitter for AutoApprovePendingEmitter {
    fn emit(&self, event: crate::protocol::events::StreamEvent) {
        if let crate::protocol::events::StreamEvent::ConfirmAsk { block_id, kind, .. } = event {
            let pending_confirms = self.pending_confirms.clone();
            let approve = kind != "ask_user";
            tokio::spawn(async move {
                for _ in 0..100 {
                    if let Some(sender) = pending_confirms.write().await.remove(&block_id) {
                        let _ = sender.send(approve);
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            });
        }
    }
}

#[tokio::test]
async fn shared_emitter_resolves_headless_write_permission_during_agent_turn() {
    let workspace = std::env::temp_dir().join(format!(
        "forge-session-shared-emitter-write-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let pending_confirms = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
    let harness = Arc::new(Harness::new_with_pending(
        workspace.clone(),
        pending_confirms.clone(),
    ));
    let created_path = workspace.join("created.txt");
    let response_1 = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "tool_use",
            "id": "call-write-1",
            "name": "write_to_file",
            "input": {
                "path": created_path.to_string_lossy(),
                "content": "created through shared emitter"
            }
        })],
        tool_calls: vec![ToolCall {
            id: "call-write-1".to_string(),
            name: "write_to_file".to_string(),
            input: serde_json::json!({
                "path": created_path.to_string_lossy(),
                "content": "created through shared emitter"
            }),
        }],
        stop_reason: Some("tool_use".to_string()),
    };
    let response_2 = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "done"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new(vec![response_1, response_2]));
    let session = AgentSession::new(
        "session-shared-emitter".to_string(),
        "deepseek".to_string(),
        adapter,
        harness,
        "system".to_string(),
        Some(128_000),
    );
    let emitter: Arc<dyn EventEmitter> = Arc::new(AutoApprovePendingEmitter::new(pending_confirms));
    let turn_guard = session.reserve_turn().expect("reserve turn");

    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        session.send_message_with_shared_emitter(
            "create a file",
            emitter,
            vec![],
            None,
            None,
            turn_guard,
        ),
    )
    .await
    .expect("shared emitter should resolve permission without hanging")
    .expect("agent turn should succeed");

    assert_eq!(
        std::fs::read_to_string(&created_path).expect("created file should exist"),
        "created through shared emitter"
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn worktree_worker_delegate_uses_shared_emitter_for_child_tool_confirmations() {
    let workspace = setup_git_test_workspace("forge-session-worktree-worker-emitter");
    let pending_confirms = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
    let harness = Arc::new(Harness::new_with_pending(
        workspace.clone(),
        pending_confirms.clone(),
    ));

    let parent_delegates = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "tool_use",
            "id": "call-delegate-1",
            "name": "delegate_task",
            "input": {
                "mode": "worktree_worker",
                "task": "Create subagent_smoke.txt with the text ok, then report the result."
            }
        })],
        tool_calls: vec![ToolCall {
            id: "call-delegate-1".to_string(),
            name: "delegate_task".to_string(),
            input: serde_json::json!({
                "mode": "worktree_worker",
                "task": "Create subagent_smoke.txt with the text ok, then report the result."
            }),
        }],
        stop_reason: Some("tool_use".to_string()),
    };
    let child_writes = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "tool_use",
            "id": "call-child-shell-1",
            "name": "run_shell",
            "input": {
                "command": "printf 'ok\\n' > subagent_smoke.txt",
                "timeout": 5
            }
        })],
        tool_calls: vec![ToolCall {
            id: "call-child-shell-1".to_string(),
            name: "run_shell".to_string(),
            input: serde_json::json!({
                "command": "printf 'ok\\n' > subagent_smoke.txt",
                "timeout": 5
            }),
        }],
        stop_reason: Some("tool_use".to_string()),
    };
    let child_finishes = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "Created subagent_smoke.txt and verified the command completed."
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };
    let parent_finishes = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "Subagent task completed and is ready for review."
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new(vec![
        parent_delegates,
        child_writes,
        child_finishes,
        parent_finishes,
    ]));
    let session = AgentSession::new(
        "session-worktree-worker-emitter".to_string(),
        "deepseek".to_string(),
        adapter,
        harness,
        "system".to_string(),
        Some(128_000),
    );
    let emitter: Arc<dyn EventEmitter> = Arc::new(AutoApprovePendingEmitter::new(pending_confirms));
    let turn_guard = session.reserve_turn().expect("reserve turn");

    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        session.send_message_with_shared_emitter(
            "delegate an isolated worker",
            emitter,
            vec![],
            None,
            None,
            turn_guard,
        ),
    )
    .await
    .expect("worktree worker child confirmation should not hang")
    .expect("agent turn should succeed");

    let snapshot = session.snapshot();
    let a2a = snapshot.a2a_state.expect("a2a state should be persisted");
    assert_eq!(a2a.tasks.len(), 1);
    let task = &a2a.tasks[0];
    assert_eq!(
        task.status,
        crate::agent::a2a::types::AgentTaskStatus::Completed
    );
    assert_eq!(
        task.execution_mode,
        crate::agent::a2a::types::AgentExecutionMode::WorktreeWorker
    );
    assert!(
        task.artifacts.iter().any(|artifact| artifact.kind
            == crate::agent::a2a::types::AgentArtifactKind::DiffSummary
            && artifact.content.contains("subagent_smoke.txt")),
        "completed worktree task should expose a diff artifact, got: {:?}",
        task.artifacts
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn full_agent_turn_multiple_tool_calls_preserve_order() {
    // This test proves that when the model requests multiple tool calls,
    // the results are assembled in the ORIGINAL tool_call order,
    // not the execution completion order.
    let workspace = setup_test_workspace("forge-multi-tool");
    let harness = Arc::new(Harness::new(workspace.clone()));

    // Write two test files
    std::fs::write(workspace.join("a.txt"), "content-A\n").expect("write a.txt");
    std::fs::write(workspace.join("b.txt"), "content-B\n").expect("write b.txt");

    // Response 1: model requests two read_file calls
    let response_1 = StreamResult {
        assistant_content: vec![
            serde_json::json!({
                "type": "text",
                "text": "让我同时读取两个文件"
            }),
            serde_json::json!({
                "type": "tool_use",
                "id": "call-a",
                "name": "read_file",
                "input": {"path": "a.txt"}
            }),
            serde_json::json!({
                "type": "tool_use",
                "id": "call-b",
                "name": "read_file",
                "input": {"path": "b.txt"}
            }),
        ],
        tool_calls: vec![
            ToolCall {
                id: "call-a".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "a.txt"}),
            },
            ToolCall {
                id: "call-b".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "b.txt"}),
            },
        ],
        stop_reason: Some("tool_use".to_string()),
    };

    // Response 2: final text
    let response_2 = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "两个文件都读取完毕"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new(vec![response_1, response_2]));
    let session = AgentSession::new(
        "session-multi".to_string(),
        "deepseek".to_string(),
        adapter,
        harness.clone(),
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    let emitter = crate::agent::event_sink::NoopEventEmitter;
    let turn_guard = session.reserve_turn().expect("reserve turn");

    let result = session
        .send_message_with_emitter(
            "读取 a.txt 和 b.txt",
            &emitter,
            vec![],
            None,
            None,
            turn_guard,
        )
        .await;

    assert!(result.is_ok(), "turn should succeed: {:?}", result.err());

    let messages = lock_unpoisoned(&session.messages);

    // Find the tool_result message
    let tool_result_msg = messages
        .iter()
        .find(|m| {
            m.role == "user"
                && m.content.as_array().is_some_and(|blocks| {
                    blocks
                        .iter()
                        .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_result"))
                })
        })
        .expect("should have tool_result message");

    let blocks = tool_result_msg.content.as_array().unwrap();
    assert_eq!(blocks.len(), 2, "expected 2 tool_result blocks");

    // Verify ORDER: call-a first, call-b second
    assert_eq!(
        blocks[0].get("tool_use_id").and_then(|v| v.as_str()),
        Some("call-a"),
        "first result must be for call-a"
    );
    assert_eq!(
        blocks[1].get("tool_use_id").and_then(|v| v.as_str()),
        Some("call-b"),
        "second result must be for call-b"
    );

    // Verify content
    let content_a = blocks[0]
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let content_b = blocks[1]
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        content_a.contains("content-A"),
        "result A should contain content-A"
    );
    assert!(
        content_b.contains("content-B"),
        "result B should contain content-B"
    );

    // Verify no missing results
    assert!(
        blocks[0].get("is_error").is_none(),
        "call-a should not be marked as error"
    );
    assert!(
        blocks[1].get("is_error").is_none(),
        "call-b should not be marked as error"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn api_error_does_not_stop_session_and_recovery_succeeds() {
    // This test proves the recovery contract:
    //   API error → session stays running → user retries → turn succeeds.
    // The session must NOT be stopped on API error; the turn must be
    // recoverable via the standard send_message path.
    let workspace = setup_test_workspace("forge-api-error-recovery");
    let harness = Arc::new(Harness::new(workspace.clone()));

    // Response 1: API error
    let error_response = Err("API error: 500 — Internal server error".to_string());

    // Response 2: success after retry
    let success_response = Ok(StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "恢复成功，文件内容已读取"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    });

    let adapter = Arc::new(FakeAdapter::new_with_errors(vec![
        error_response,
        success_response,
    ]));
    let session = AgentSession::new(
        "session-api-recovery".to_string(),
        "deepseek".to_string(),
        adapter.clone(),
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();

    // First turn: API error
    let turn_guard = session.reserve_turn().expect("reserve turn 1");
    let result1 = session
        .send_message_with_emitter("读取文件", &emitter, vec![], None, None, turn_guard)
        .await;

    assert!(result1.is_err(), "first turn should fail with API error");
    assert!(
        result1.unwrap_err().contains("500"),
        "error should mention status code"
    );

    // Session must still be running
    assert!(
        session.running.load(Ordering::SeqCst),
        "session should still be running after API error"
    );

    // Turn guard was dropped, so turn_inflight should be false
    let turn_guard2 = session.reserve_turn();
    assert!(
        turn_guard2.is_ok(),
        "should be able to reserve a new turn after API error"
    );

    // Second turn: recovery
    let result2 = session
        .send_message_with_emitter("继续", &emitter, vec![], None, None, turn_guard2.unwrap())
        .await;

    assert!(
        result2.is_ok(),
        "recovery turn should succeed: {:?}",
        result2.err()
    );

    // Verify turn state recovered
    let turn = lock_unpoisoned(&session.latest_turn);
    let turn = turn.as_ref().expect("latest turn should exist");
    assert_eq!(
        turn.status,
        AgentTurnStatus::Completed,
        "recovered turn should be completed"
    );

    // Verify adapter was called: 1 (failed) + 1 (recovery) + 1 (final summary) = 3
    // The failed call counts because FakeAdapter processes it
    assert_eq!(
        adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "adapter should be called 2 times (1 failed + 1 success with final summary)"
    );

    // Verify error events were emitted
    let events = emitter.drain();
    let has_error = events
        .iter()
        .any(|e| matches!(e, StreamEvent::Error { .. }));
    assert!(has_error, "should have emitted an error event");

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn tool_failure_result_is_error_and_model_sees_it() {
    // This test proves that when a tool execution fails (e.g., file not found),
    // the error result is properly marked and the model can see it and recover.
    let workspace = setup_test_workspace("forge-tool-failure");
    let harness = Arc::new(Harness::new(workspace.clone()));

    // Response 1: model requests a non-existent file
    let response_1 = StreamResult {
        assistant_content: vec![
            serde_json::json!({
                "type": "text",
                "text": "让我读取这个文件"
            }),
            serde_json::json!({
                "type": "tool_use",
                "id": "call-missing",
                "name": "read_file",
                "input": {"path": "nonexistent.txt"}
            }),
        ],
        tool_calls: vec![ToolCall {
            id: "call-missing".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "nonexistent.txt"}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    // Response 2: model sees the error and responds accordingly
    let response_2 = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "文件不存在，让我检查一下目录"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new(vec![response_1, response_2]));
    let session = AgentSession::new(
        "session-tool-failure".to_string(),
        "deepseek".to_string(),
        adapter,
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    let emitter = crate::agent::event_sink::NoopEventEmitter;
    let turn_guard = session.reserve_turn().expect("reserve turn");

    let result = session
        .send_message_with_emitter(
            "读取 nonexistent.txt",
            &emitter,
            vec![],
            None,
            None,
            turn_guard,
        )
        .await;

    assert!(
        result.is_ok(),
        "turn should succeed even with tool failure: {:?}",
        result.err()
    );

    let messages = lock_unpoisoned(&session.messages);

    // Find the tool_result message
    let tool_result_msg = messages
        .iter()
        .find(|m| {
            m.role == "user"
                && m.content.as_array().is_some_and(|blocks| {
                    blocks
                        .iter()
                        .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_result"))
                })
        })
        .expect("should have tool_result message");

    let blocks = tool_result_msg.content.as_array().unwrap();
    assert_eq!(blocks.len(), 1);

    // Tool result should contain the error
    let content = blocks[0]
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        content.contains("Error") || content.contains("不存在") || content.contains("not found"),
        "tool result should indicate file not found, got: {}",
        content
    );

    // Turn should be completed
    let turn = lock_unpoisoned(&session.latest_turn);
    let turn = turn.as_ref().expect("latest turn");
    assert_eq!(turn.status, AgentTurnStatus::Completed);

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn adapter_called_exactly_once_per_tool_round_plus_final_summary() {
    // This test proves the adapter call budget is correct:
    //   1 call for tool round + 1 call for final summary = 2 total.
    // No phantom retries, no extra calls.
    let workspace = setup_test_workspace("forge-call-count");
    let harness = Arc::new(Harness::new(workspace.clone()));

    std::fs::write(workspace.join("data.txt"), "test-data\n").expect("write data.txt");

    let response_1 = StreamResult {
        assistant_content: vec![
            serde_json::json!({"type": "text", "text": "读取中"}),
            serde_json::json!({
                "type": "tool_use", "id": "c1", "name": "read_file",
                "input": {"path": "data.txt"}
            }),
        ],
        tool_calls: vec![ToolCall {
            id: "c1".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "data.txt"}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    let response_2 = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text", "text": "数据已读取: test-data"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new(vec![response_1, response_2]));
    let session = AgentSession::new(
        "session-call-count".to_string(),
        "deepseek".to_string(),
        adapter.clone(),
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    let emitter = crate::agent::event_sink::NoopEventEmitter;
    let turn_guard = session.reserve_turn().expect("reserve turn");

    let _ = session
        .send_message_with_emitter("读取 data.txt", &emitter, vec![], None, None, turn_guard)
        .await;

    assert_eq!(
        adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "should call adapter exactly 2 times: 1 tool round + 1 final summary"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn final_summary_tool_calls_are_kept_out_of_rendered_stream() {
    let workspace = setup_test_workspace("forge-final-summary-tool-stream");
    let harness = Arc::new(Harness::new(workspace.clone()));
    let response = StreamResult {
        assistant_content: vec![
            serde_json::json!({"type": "text", "text": "我需要验证一下"}),
            serde_json::json!({
                "type": "tool_use", "id": "summary-shell-1", "name": "run_shell",
                "input": {"command": "npm run build"}
            }),
        ],
        tool_calls: vec![ToolCall {
            id: "summary-shell-1".to_string(),
            name: "run_shell".to_string(),
            input: serde_json::json!({"command": "npm run build"}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    let adapter = Arc::new(StreamingFakeAdapter::new(vec![response]));
    let session = AgentSession::new(
        "session-final-summary-tool-stream".to_string(),
        "deepseek".to_string(),
        adapter,
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );
    lock_unpoisoned(&session.messages).push(ChatMessage::user("工具结果已经返回"));

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    session
        .finalize_turn(&[], &emitter, None, Arc::new(Notify::new()))
        .await;

    let events = emitter.drain();
    assert!(
        events.iter().any(|event| matches!(
            event,
            StreamEvent::TextChunk { content, .. } if content.contains("我需要验证一下")
        )),
        "final summary text should still render"
    );
    assert!(
        !events.iter().any(|event| matches!(
            event,
            StreamEvent::ToolCallStart { block_id, .. }
                | StreamEvent::ToolCallEnd { block_id, .. }
                | StreamEvent::ToolCallResult { block_id, .. } if block_id == "summary-shell-1"
        )),
        "final-summary tool calls are not executed and must not render as failed tools"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn agent_turn_allows_long_tool_loop_before_final_answer() {
    let workspace = setup_test_workspace("forge-many-tool-rounds");
    let harness = Arc::new(Harness::new(workspace.clone()));

    let mut responses = Vec::new();
    for idx in 0..30 {
        let id = format!("read-{idx}");
        let path = format!("data-{idx}.txt");
        std::fs::write(workspace.join(&path), format!("test-data-{idx}\n"))
            .expect("write data file");
        responses.push(StreamResult {
            assistant_content: vec![
                serde_json::json!({"type": "text", "text": format!("第 {idx} 轮读取")}),
                serde_json::json!({
                    "type": "tool_use", "id": id, "name": "read_file",
                    "input": {"path": path.clone()}
                }),
            ],
            tool_calls: vec![ToolCall {
                id: format!("read-{idx}"),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": path}),
            }],
            stop_reason: Some("tool_use".to_string()),
        });
    }
    responses.push(StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text", "text": "完成"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    });

    let adapter = Arc::new(FakeAdapter::new(responses));
    let session = AgentSession::new(
        "session-many-tool-rounds".to_string(),
        "deepseek".to_string(),
        adapter.clone(),
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );
    // Allow many repeated read_file category batches for this exploration test
    {
        use crate::agent::session_guards::lock_unpoisoned;
        let mut guard = lock_unpoisoned(&session.loop_guard);
        *guard = guard.clone().with_max_repeated_category_batches(100);
    }

    let emitter = crate::agent::event_sink::NoopEventEmitter;
    let turn_guard = session.reserve_turn().expect("reserve turn");
    session
        .send_message_with_emitter(
            "连续读取文件直到完成",
            &emitter,
            vec![],
            None,
            None,
            turn_guard,
        )
        .await
        .expect("turn succeeds");

    assert_eq!(
        adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
        31,
        "30 tool rounds plus final answer should run before finalization"
    );

    let turn = lock_unpoisoned(&session.latest_turn);
    assert_eq!(
        turn.as_ref().map(|turn| turn.status.clone()),
        Some(AgentTurnStatus::Completed)
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn agent_turn_tracks_budget_counters_through_multi_round_loop() {
    let workspace = setup_test_workspace("forge-budget-counters");
    std::fs::write(workspace.join("data.txt"), "test-data\n").expect("write data.txt");
    let harness = Arc::new(Harness::new(workspace.clone()));

    let response_1 = StreamResult {
        assistant_content: vec![
            serde_json::json!({"type": "text", "text": "读取中"}),
            serde_json::json!({
                "type": "tool_use", "id": "c1", "name": "read_file",
                "input": {"path": "data.txt"}
            }),
        ],
        tool_calls: vec![ToolCall {
            id: "c1".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "data.txt"}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    let response_2 = StreamResult {
        assistant_content: vec![
            serde_json::json!({"type": "text", "text": "再读一次"}),
            serde_json::json!({
                "type": "tool_use", "id": "c2", "name": "read_file",
                "input": {"path": "data.txt"}
            }),
        ],
        tool_calls: vec![ToolCall {
            id: "c2".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "data.txt"}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    let response_3 = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text", "text": "完成"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new(vec![response_1, response_2, response_3]));
    let session = AgentSession::new(
        "session-budget".to_string(),
        "deepseek".to_string(),
        adapter.clone(),
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    let emitter = crate::agent::event_sink::NoopEventEmitter;
    let turn_guard = session.reserve_turn().expect("reserve turn");

    session
        .send_message_with_emitter("连续读取两次", &emitter, vec![], None, None, turn_guard)
        .await
        .expect("turn succeeds");

    // Verify adapter called 3 times: 2 tool rounds + final answer
    assert_eq!(
        adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
        3,
        "2 tool rounds + 1 final answer"
    );

    // Verify budget counters on turn state
    let turn = lock_unpoisoned(&session.latest_turn);
    let turn = turn.as_ref().expect("latest turn");
    assert_eq!(turn.status, AgentTurnStatus::Completed);
    assert_eq!(
        turn.model_rounds, 3,
        "should count 3 model calls: 2 tool rounds + 1 final answer"
    );
    assert_eq!(turn.tool_call_count, 2, "should count 2 tool calls");
    assert_eq!(turn.failed_tool_count, 0, "no tools failed");

    // Verify metrics snapshot
    let metrics = session.latest_turn_usage_snapshot();
    assert_eq!(
        metrics.model_rounds, 3,
        "metrics should track 3 model rounds"
    );
    assert_eq!(
        metrics.tool_call_count, 2,
        "metrics should track 2 tool calls"
    );
    assert_eq!(
        metrics.failed_tool_count, 0,
        "metrics should track 0 failed tools"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

// ── CancellableFakeAdapter for cancel mid-turn testing ─────────

/// Adapter that returns tool_use on call 1, signals "ready" then waits for
/// the passed-in cancel token on call 2, returns Ok text on call 3+.
/// The cancel token comes from the session — `kill_with_emitter` fires it
/// via `notify_one()`, matching real HTTP adapter behavior.
struct CancellableFakeAdapter {
    call_count: std::sync::atomic::AtomicUsize,
    /// Set to true when the adapter reaches its blocking call (call 2).
    ready: std::sync::atomic::AtomicBool,
}

impl CancellableFakeAdapter {
    fn new() -> Self {
        Self {
            call_count: std::sync::atomic::AtomicUsize::new(0),
            ready: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl AiAdapter for CancellableFakeAdapter {
    async fn stream_message(
        &self,
        _session_id: &str,
        _messages: &[crate::adapters::base::ChatMessage],
        _app_handle: &tauri::AppHandle,
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        panic!("use call_with_emitter");
    }

    async fn call(
        &self,
        _messages: &[crate::adapters::base::ChatMessage],
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        let idx = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match idx {
            0 => Ok(StreamResult {
                assistant_content: vec![
                    serde_json::json!({"type": "text", "text": "读取中"}),
                    serde_json::json!({
                        "type": "tool_use", "id": "tc-1", "name": "read_file",
                        "input": {"path": "data.txt"}
                    }),
                ],
                tool_calls: vec![ToolCall {
                    id: "tc-1".to_string(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({"path": "data.txt"}),
                }],
                stop_reason: Some("tool_use".to_string()),
            }),
            1 => {
                // Signal that we've reached the blocking point, then wait
                // on the session's cancel token — the same token that
                // kill_with_emitter fires via notify_one().
                self.ready.store(true, std::sync::atomic::Ordering::SeqCst);
                cancel.notified().await;
                Err(AdapterError::Stream("cancelled".to_string()))
            }
            _ => Ok(StreamResult {
                assistant_content: vec![serde_json::json!({
                    "type": "text",
                    "text": "已恢复"
                })],
                tool_calls: vec![],
                stop_reason: Some("end_turn".to_string()),
            }),
        }
    }

    async fn call_with_emitter(
        &self,
        _session_id: &str,
        _messages: &[crate::adapters::base::ChatMessage],
        _emitter: &dyn EventEmitter,
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        self.call(_messages, cancel).await
    }

    fn model_id(&self) -> &str {
        "fake-cancel-model"
    }
    fn model_name(&self) -> &str {
        "Fake Cancel Model"
    }
}

struct CompactSummaryCancellableAdapter {
    summary_call_count: std::sync::atomic::AtomicUsize,
    model_call_count: std::sync::atomic::AtomicUsize,
    ready: std::sync::atomic::AtomicBool,
}

impl CompactSummaryCancellableAdapter {
    fn new() -> Self {
        Self {
            summary_call_count: std::sync::atomic::AtomicUsize::new(0),
            model_call_count: std::sync::atomic::AtomicUsize::new(0),
            ready: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl AiAdapter for CompactSummaryCancellableAdapter {
    async fn stream_message(
        &self,
        _session_id: &str,
        _messages: &[crate::adapters::base::ChatMessage],
        _app_handle: &tauri::AppHandle,
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        panic!("use stream_message_with_emitter");
    }

    async fn stream_message_with_emitter(
        &self,
        _session_id: &str,
        _messages: &[crate::adapters::base::ChatMessage],
        _emitter: &dyn EventEmitter,
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        self.model_call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text",
                "text": "should not be called after compact cancellation",
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        })
    }

    async fn call(
        &self,
        _messages: &[crate::adapters::base::ChatMessage],
        _cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        panic!("use compact_summary or stream_message_with_emitter");
    }

    async fn compact_summary(
        &self,
        _messages: &[crate::adapters::base::ChatMessage],
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        self.summary_call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.ready.store(true, std::sync::atomic::Ordering::SeqCst);
        cancel.notified().await;
        Err(AdapterError::Stream("Cancelled".to_string()))
    }

    fn model_id(&self) -> &str {
        "fake-compact-cancel-model"
    }

    fn model_name(&self) -> &str {
        "Fake Compact Cancel Model"
    }
}

#[tokio::test]
async fn kill_during_auto_compact_summary_cancels_without_model_call() {
    let workspace = setup_test_workspace("forge-kill-during-auto-compact");
    let adapter = Arc::new(CompactSummaryCancellableAdapter::new());
    let session = Arc::new(AgentSession::new(
        "session-kill-during-auto-compact".to_string(),
        "deepseek".to_string(),
        adapter.clone(),
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    ));
    let mut history = Vec::new();
    for index in 0..90 {
        if index % 2 == 0 {
            history.push(ChatMessage::user(&format!("older user message {index}")));
        } else {
            history.push(ChatMessage::assistant(serde_json::Value::String(format!(
                "older assistant message {index}"
            ))));
        }
    }
    session.restore_state(history, None, None, None, None);

    let emitter = Arc::new(crate::agent::event_sink::CollectingEventEmitter::new());
    let turn_guard = session.reserve_turn().expect("reserve turn");
    let session_for_turn = session.clone();
    let emitter_for_turn = emitter.clone();
    let handle = tokio::spawn(async move {
        session_for_turn
            .send_message_with_emitter(
                "continue",
                &*emitter_for_turn,
                vec![],
                None,
                None,
                turn_guard,
            )
            .await
    });

    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !adapter.ready.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("compact summary should reach cancellation point");

    let kill_emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    session.kill_with_emitter(&kill_emitter);

    let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle)
        .await
        .expect("killed turn should finish")
        .expect("task should not panic");

    assert!(result.is_err(), "killed compact turn should return error");
    assert_eq!(
        adapter
            .summary_call_count
            .load(std::sync::atomic::Ordering::SeqCst),
        1,
        "auto compact should start one summary request"
    );
    assert_eq!(
        adapter
            .model_call_count
            .load(std::sync::atomic::Ordering::SeqCst),
        0,
        "cancel during compact summary must not continue into the normal model call"
    );

    let turn = lock_unpoisoned(&session.latest_turn);
    let turn = turn.as_ref().expect("latest turn");
    assert_eq!(turn.status, AgentTurnStatus::Cancelled);

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn cancel_mid_turn_sets_cancelled_state_and_preserves_history() {
    // This test proves the cancel contract:
    //   1. Adapter returns tool_use, tool executes, loop enters round 2.
    //   2. During round 2 adapter call, session.running is set to false.
    //   3. Adapter returns cancelled, turn state is Cancelled.
    //   4. Message history preserves tool_use/tool_result pairing.
    //   5. Recovery after cancel succeeds.
    let workspace = setup_test_workspace("forge-cancel-mid-turn");
    let harness = Arc::new(Harness::new(workspace.clone()));
    std::fs::write(workspace.join("data.txt"), "cancel-test-data\n").expect("write data.txt");

    let adapter = Arc::new(CancellableFakeAdapter::new());
    let session = AgentSession::new(
        "session-cancel".to_string(),
        "deepseek".to_string(),
        adapter.clone(),
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    let session = Arc::new(session);
    let emitter = Arc::new(crate::agent::event_sink::CollectingEventEmitter::new());

    // Spawn the turn in a background task
    let turn_guard = session.reserve_turn().expect("reserve turn");
    let session2 = session.clone();
    let emitter2 = emitter.clone();
    let handle = tokio::spawn(async move {
        session2
            .send_message_with_emitter("读取 data.txt", &*emitter2, vec![], None, None, turn_guard)
            .await
    });

    // Wait for the adapter to reach its second call before cancelling.
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !adapter.ready.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("adapter should reach the second model call");

    // Cancel: set running to false and fire the session's cancel token
    // (this is what the cancel IPC does — same token the adapter waits on)
    session.running.store(false, Ordering::SeqCst);
    lock_unpoisoned(&session.cancel)
        .as_ref()
        .unwrap()
        .notify_one();

    // Wait for the turn to finish
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle)
        .await
        .expect("cancelled turn should finish")
        .expect("task should not panic");
    assert!(result.is_err(), "cancelled turn should return error");

    // Session running should be false (we set it directly)
    assert!(
        !session.running.load(Ordering::SeqCst),
        "session.running should be false after cancel"
    );

    {
        // Turn state should be cancelled
        let turn = lock_unpoisoned(&session.latest_turn);
        let turn = turn.as_ref().expect("latest turn should exist");
        assert_eq!(
            turn.status,
            AgentTurnStatus::Cancelled,
            "cancelled turn should be marked cancelled"
        );

        // Message history should have user + assistant(tool_use) + user(tool_result)
        let messages = lock_unpoisoned(&session.messages);
        assert!(
            messages.len() >= 3,
            "history should have user, assistant(tool_use), tool_result at minimum, got {}",
            messages.len()
        );

        // Verify tool_use/tool_result pairing is intact
        let assistant_msg = messages
            .iter()
            .find(|m| {
                m.role == "assistant"
                    && m.content.as_array().is_some_and(|blocks| {
                        blocks
                            .iter()
                            .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
                    })
            })
            .expect("should have assistant with tool_use");
        let tool_use_id = assistant_msg
            .content
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
            .and_then(|b| b.get("id"))
            .and_then(|v| v.as_str())
            .expect("tool_use id");

        let tool_result_msg = messages
            .iter()
            .find(|m| {
                m.role == "user"
                    && m.content.as_array().is_some_and(|blocks| {
                        blocks.iter().any(|b| {
                            b.get("tool_use_id").and_then(|v| v.as_str()) == Some(tool_use_id)
                        })
                    })
            })
            .expect("should have matching tool_result");
        let result_content = tool_result_msg
            .content
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b.get("tool_use_id").and_then(|v| v.as_str()) == Some(tool_use_id))
            .and_then(|b| b.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            result_content.contains("cancel-test-data"),
            "tool result should contain file content, got: {}",
            result_content
        );
    }

    // ── Recovery after cancel ──
    // Re-enable the session and try again
    session.running.store(true, Ordering::SeqCst);

    let emitter2 = crate::agent::event_sink::NoopEventEmitter;
    let turn_guard2 = session.reserve_turn().expect("reserve turn after cancel");
    let recovery_result = session
        .send_message_with_emitter("继续", &emitter2, vec![], None, None, turn_guard2)
        .await;

    assert!(
        recovery_result.is_ok(),
        "recovery after cancel should succeed: {:?}",
        recovery_result.err()
    );

    // Final turn should be completed
    let turn = lock_unpoisoned(&session.latest_turn);
    let turn = turn.as_ref().expect("latest turn");
    assert_eq!(
        turn.status,
        AgentTurnStatus::Completed,
        "recovery turn should be completed"
    );

    // Total adapter calls: 2 (first turn: tool_use + error) + 2 (recovery: text + final summary) = 4
    // But call 2 returns error which causes early return, so the final summary call doesn't happen.
    // So: call 0 (tool_use) + call 1 (cancel_error) + call 2 (recovery text) = 3
    // Actually, after recovery text (no tool_calls), the loop breaks, then the final summary
    // section checks if last_role is "tool" or "user" — the recovery response has text only,
    // so the last message is assistant. No final summary call needed.
    // Total: 3 calls
    assert_eq!(
        adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
        3,
        "adapter should be called 3 times: tool_use + cancel_error + recovery"
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn auto_compact_uses_model_generated_summary_before_model_call() {
    let workspace = setup_test_workspace("forge-auto-model-compact");
    let harness = Arc::new(Harness::new(workspace.clone()));
    let session_id = "session-auto-model-compact".to_string();

    let compact_summary_response = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "<summary>\nMODEL AUTO SUMMARY\n</summary>"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };
    let final_response = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "auto compact 后继续回答"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };
    let adapter = Arc::new(FakeAdapter::new(vec![
        compact_summary_response,
        final_response,
    ]));
    let session = AgentSession::new(
        session_id.clone(),
        "deepseek".to_string(),
        adapter.clone(),
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    {
        let mut msgs = lock_unpoisoned(&session.messages);
        for i in 0..45 {
            msgs.push(ChatMessage::user(&format!("历史用户消息 {}", i)));
            msgs.push(ChatMessage::assistant(serde_json::json!([
                { "type": "text", "text": format!("历史助手回复 {}", i) }
            ])));
        }
    }

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    let turn_guard = session.reserve_turn().expect("reserve turn");

    session
        .send_message_with_emitter("继续处理当前任务", &emitter, vec![], None, None, turn_guard)
        .await
        .expect("turn should complete after auto compact");

    assert_eq!(
        adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "auto compact should call the model once for summary and once for the normal answer"
    );
    let summary = lock_unpoisoned(&session.summary)
        .clone()
        .expect("summary should be persisted");
    assert!(summary.contains("MODEL AUTO SUMMARY"));

    let messages = lock_unpoisoned(&session.messages);
    assert!(
        messages.len() < 92,
        "auto compact should reduce persisted message history"
    );
    let final_assistant = messages
        .iter()
        .rev()
        .find(|message| message.role == "assistant")
        .expect("final assistant message");
    let final_text = final_summary_text(
        final_assistant
            .content
            .as_array()
            .expect("assistant content should be blocks"),
    );
    assert!(final_text.contains("auto compact 后继续回答"));
    drop(messages);

    let events = emitter.drain();
    assert!(events.iter().any(|event| matches!(
        event,
        StreamEvent::ContextCompacted {
            session_id,
            summary,
            ..
        } if session_id == "session-auto-model-compact"
            && summary.contains("MODEL AUTO SUMMARY")
    )));

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn manual_compact_skipped_emits_both_start_and_skipped_events() {
    // When manual compact is triggered but skipped (e.g., history too short),
    // both context_compact_start and context_compact_skipped should be emitted
    // so the frontend sees the full lifecycle.
    let workspace = std::env::temp_dir().join(format!(
        "forge-session-manual-compact-start-skipped-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-manual-compact-start-skipped".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );
    {
        let mut messages = lock_unpoisoned(&session.messages);
        for index in 0..12 {
            if index % 2 == 0 {
                messages.push(ChatMessage::user(&format!("user message {index}")));
            } else {
                messages.push(ChatMessage::assistant(serde_json::Value::String(format!(
                    "assistant message {index}"
                ))));
            }
        }
    }
    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();

    let result = session
        .compact_now_with_emitter(&emitter)
        .await
        .expect("manual compact should return skipped result");

    assert!(!result.compacted);
    let events = emitter.drain();

    let start_index = events.iter().position(|event| {
        matches!(
            event,
            StreamEvent::ContextCompactStart { session_id, .. }
                if session_id == "session-manual-compact-start-skipped"
        )
    });
    let skipped_index = events.iter().position(|event| {
        matches!(
            event,
            StreamEvent::ContextCompactSkipped { session_id, .. }
                if session_id == "session-manual-compact-start-skipped"
        )
    });

    assert!(
        start_index.is_some(),
        "should emit context_compact_start before skipped"
    );
    assert!(
        skipped_index.is_some(),
        "should emit context_compact_skipped"
    );
    assert!(
        start_index.unwrap() < skipped_index.unwrap(),
        "compact_start must come before compact_skipped"
    );

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn overflow_retry_compacts_and_turn_completes() {
    // This test proves that when the adapter returns a context overflow error,
    // the agent loop triggers compaction, retries, and completes successfully.
    //
    // Scenario:
    //   1. Session has 24 pre-seeded messages (above MIN_COMPACT_MESSAGES threshold)
    //   2. FakeAdapter call 0 → context_length_exceeded error
    //   3. Overflow retry: compact plan asks the model for a summary
    //   4. FakeAdapter call 2 → final text answer (no tool calls)
    //   5. Turn completes with correct state
    //
    // Verified:
    //   - Turn status: Completed
    //   - Adapter called exactly 3 times (overflow + summary + retry)
    //   - Messages compacted (fewer than initial)
    //   - Summary is set
    //   - ContextCompacted event emitted
    //   - overflow_retry_used prevents infinite retry

    let workspace = setup_test_workspace("forge-overflow-compact");
    let harness = Arc::new(Harness::new(workspace.clone()));
    let session_id = "session-overflow-compact".to_string();

    // Response after compaction retry: simple text, no tool calls
    let retry_response = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "已完成分析"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };
    let compact_summary_response = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "<summary>\nMODEL OVERFLOW SUMMARY\n</summary>"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new_with_errors(vec![
        // Call 0: context overflow error — triggers compaction
        Err(
            "context_length_exceeded: This model's maximum context length is 128000 tokens."
                .to_string(),
        ),
        // Call 1: semantic compact summary
        Ok(compact_summary_response),
        // Call 2: success after compaction retry
        Ok(retry_response),
    ]));

    let session = AgentSession::new(
        session_id.clone(),
        "openai".to_string(), // provider type for overflow detection
        adapter.clone(),
        harness.clone(),
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    // Pre-seed 24 messages to ensure compaction has enough material.
    // compact_messages_for_overflow_retry uses OVERFLOW_RETRY_RETAIN_RECENT_MESSAGES = 16,
    // and MIN_COMPACT_MESSAGES = 8. With 24 pre-seeded + 1 user message = 25 total,
    // split_at = 25 - 16 = 9 >= 8, so compaction will proceed:
    // first 9 messages compacted into summary, last 16 retained.
    {
        let mut msgs = lock_unpoisoned(&session.messages);
        for i in 0..12 {
            msgs.push(ChatMessage::user(&format!(
                "用户消息 {}: 请帮我分析代码结构",
                i
            )));
            msgs.push(ChatMessage::assistant(serde_json::json!([
                {
                    "type": "text",
                    "text": format!("助手回复 {}: 代码结构分析如下...", i)
                }
            ])));
        }
        // After send_message_with_emitter adds the user message, total = 25
    }

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    let turn_guard = session.reserve_turn().expect("reserve turn");

    let result = session
        .send_message_with_emitter(
            "请继续分析剩余文件",
            &emitter,
            vec![],
            None,
            None,
            turn_guard,
        )
        .await;

    assert!(
        result.is_ok(),
        "overflow retry turn should succeed: {:?}",
        result.err()
    );

    // 1. Turn should be completed
    {
        let turn = lock_unpoisoned(&session.latest_turn);
        let turn = turn.as_ref().expect("latest turn should exist");
        assert_eq!(
            turn.status,
            AgentTurnStatus::Completed,
            "turn should be completed after overflow retry"
        );
    }

    // 2. Messages should be compacted — fewer than the 25 we started with
    let msg_count = lock_unpoisoned(&session.messages).len();
    assert!(
        msg_count < 25,
        "messages should be compacted: expected < 25, got {}",
        msg_count
    );

    // 3. Summary should be set after compaction
    let summary = lock_unpoisoned(&session.summary).clone();
    assert!(
        summary.is_some(),
        "summary should be set after overflow compaction"
    );

    // 4. ContextCompacted event should be emitted
    let events = emitter.drain();
    let has_compacted_event = events.iter().any(|e| matches!(
            e,
            StreamEvent::ContextCompacted { session_id, .. } if session_id == "session-overflow-compact"
        ));
    assert!(
        has_compacted_event,
        "ContextCompacted event should be emitted for overflow retry, got: {:?}",
        events
            .iter()
            .map(std::mem::discriminant)
            .collect::<Vec<_>>()
    );

    // 5. Adapter should be called exactly 3 times: overflow error + summary + retry success
    assert_eq!(
        adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
        3,
        "adapter should be called exactly 3 times: overflow error + compact summary + retry"
    );

    // 6. Final assistant message should contain the retry response text
    let messages = lock_unpoisoned(&session.messages);
    let final_assistant = messages.iter().rev().find(|m| m.role == "assistant");
    assert!(
        final_assistant.is_some(),
        "should have a final assistant message"
    );
    let final_blocks = final_assistant
        .unwrap()
        .content
        .as_array()
        .expect("assistant content blocks");
    let final_text = final_blocks
        .iter()
        .find_map(|b| {
            if b.get("type").and_then(|v| v.as_str()) == Some("text") {
                b.get("text").and_then(|v| v.as_str())
            } else {
                None
            }
        })
        .expect("final text block");
    assert!(
        final_text.contains("已完成分析"),
        "final text should be from retry response, got: {}",
        final_text
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn tool_use_followed_by_overflow_retry_preserves_pairing_and_completes() {
    // This test proves the more realistic scenario where overflow happens AFTER
    // a tool round — the model first makes a tool call, the harness executes it,
    // then the second model call hits context overflow, compaction fires, and the
    // retry succeeds.
    //
    // Scenario:
    //   0. Pre-seed 24 messages (to ensure compaction threshold)
    //   1. FakeAdapter call 0 → tool_use (read_file)
    //   2. Harness executes read_file → tool_result added to history
    //   3. FakeAdapter call 1 → context_length_exceeded error
    //   4. Overflow compaction: model summary, tool_use/tool_result preserved
    //   5. FakeAdapter call 3 → final text answer
    //   6. Turn Completed
    //
    // Verified:
    //   - tool_use/tool_result pairing survives compaction
    //   - tool_result content is correct (contains file content)
    //   - Turn status: Completed
    //   - Adapter called exactly 4 times
    //   - ContextCompacted event emitted
    //   - Messages compacted

    let workspace = setup_test_workspace("forge-overflow-tool-combo");
    let harness = Arc::new(Harness::new(workspace.clone()));
    let session_id = "session-overflow-tool-combo".to_string();

    let tool_call_id = "call-read-overflow".to_string();

    // Response 0: model wants to read a file
    let response_tool_use = StreamResult {
        assistant_content: vec![
            serde_json::json!({
                "type": "text",
                "text": "让我先看源码"
            }),
            serde_json::json!({
                "type": "tool_use",
                "id": tool_call_id.clone(),
                "name": "read_file",
                "input": {"path": "src/main.rs"}
            }),
        ],
        tool_calls: vec![ToolCall {
            id: tool_call_id.clone(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "src/main.rs"}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    // Response 1: overflow error on second model call
    let overflow_error =
        "context_length_exceeded: This model's maximum context length is 128000 tokens.";

    // Response 2: semantic compact summary
    let response_compact_summary = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "<summary>\nMODEL TOOL OVERFLOW SUMMARY\n</summary>"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };

    // Response 3: success after compaction retry
    let response_final = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "源码分析完成，这是一个 hello world 程序"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new_with_errors(vec![
        Ok(response_tool_use),
        Err(overflow_error.to_string()),
        Ok(response_compact_summary),
        Ok(response_final),
    ]));

    let session = AgentSession::new(
        session_id.clone(),
        "openai".to_string(),
        adapter.clone(),
        harness.clone(),
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    // Pre-seed 24 messages. After send_message adds 1 user msg, and the tool round
    // adds assistant(tool_use) + user(tool_result), total = 27 before overflow.
    // split_at = 27 - 16 = 11 >= MIN_COMPACT_MESSAGES(8) → compaction proceeds.
    // The tool_use/tool_result pair is in the last 16 messages → preserved.
    {
        let mut msgs = lock_unpoisoned(&session.messages);
        for i in 0..12 {
            msgs.push(ChatMessage::user(&format!("用户消息 {}", i)));
            msgs.push(ChatMessage::assistant(serde_json::json!([
                { "type": "text", "text": format!("助手回复 {}", i) }
            ])));
        }
    }

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    let turn_guard = session.reserve_turn().expect("reserve turn");

    let result = session
        .send_message_with_emitter(
            "帮我看看 src/main.rs 的内容",
            &emitter,
            vec![],
            None,
            None,
            turn_guard,
        )
        .await;

    assert!(
        result.is_ok(),
        "tool_use + overflow retry turn should succeed: {:?}",
        result.err()
    );

    // 1. Turn should be completed
    {
        let turn = lock_unpoisoned(&session.latest_turn);
        let turn = turn.as_ref().expect("latest turn");
        assert_eq!(
            turn.status,
            AgentTurnStatus::Completed,
            "turn should be completed after tool_use + overflow retry"
        );
    }

    // 2. Messages should be compacted
    let messages = lock_unpoisoned(&session.messages);
    let msg_count = messages.len();
    assert!(
        msg_count < 27,
        "messages should be compacted: expected < 27, got {}",
        msg_count
    );

    // 3. tool_use/tool_result pairing should survive compaction:
    //    find the assistant message with tool_use, then the next user message
    //    should have a tool_result referencing the same id.
    let tool_use_idx = messages
        .iter()
        .position(|m| {
            m.role == "assistant"
                && m.content.as_array().is_some_and(|blocks| {
                    blocks
                        .iter()
                        .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
                })
        })
        .expect("should have assistant message with tool_use after compaction");

    let tool_result_msg = messages
        .get(tool_use_idx + 1)
        .expect("tool_result message should follow tool_use");
    assert_eq!(
        tool_result_msg.role, "user",
        "tool_result should be in a user message"
    );
    let result_blocks = tool_result_msg
        .content
        .as_array()
        .expect("tool_result content blocks");
    assert!(
        result_blocks.iter().any(|b| {
            b.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                && b.get("tool_use_id").and_then(|v| v.as_str()) == Some(tool_call_id.as_str())
        }),
        "tool_result should reference tool_use id '{}', got: {:?}",
        tool_call_id,
        result_blocks
    );

    // 4. Tool result should contain the file content
    let result_text = result_blocks
        .iter()
        .find_map(|b| {
            if b.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                b.get("content").and_then(|v| v.as_str()).map(String::from)
            } else {
                None
            }
        })
        .expect("tool_result content");
    assert!(
        result_text.contains("hello world"),
        "tool result should contain file content, got: {}",
        result_text
    );

    // 5. Summary should be set
    let summary = lock_unpoisoned(&session.summary).clone();
    assert!(summary.is_some(), "summary should be set after compaction");

    // 6. ContextCompacted event should be emitted
    let events = emitter.drain();
    let has_compacted = events.iter().any(|e| {
        matches!(
            e,
            StreamEvent::ContextCompacted { session_id, .. }
                if session_id == "session-overflow-tool-combo"
        )
    });
    assert!(has_compacted, "ContextCompacted event should be emitted");

    // 7. Adapter should be called exactly 4 times: tool_use + overflow + summary + retry
    assert_eq!(
        adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
        4,
        "adapter should be called 4 times: tool_use + overflow error + compact summary + retry"
    );

    // 8. Final assistant message from retry
    let final_assistant = messages.iter().rev().find(|m| m.role == "assistant");
    let final_blocks = final_assistant
        .and_then(|m| m.content.as_array())
        .expect("final assistant blocks");
    let final_text = final_blocks
        .iter()
        .find_map(|b| {
            if b.get("type").and_then(|v| v.as_str()) == Some("text") {
                b.get("text").and_then(|v| v.as_str())
            } else {
                None
            }
        })
        .expect("final text");
    assert!(
        final_text.contains("源码分析完成"),
        "final text should be from retry response, got: {}",
        final_text
    );

    let _ = std::fs::remove_dir_all(&workspace);
}
#[tokio::test]
async fn multiple_tool_calls_followed_by_overflow_preserves_all_pairings() {
    // This test proves that when the model returns multiple tool_calls in one
    // response, all results are properly paired, and overflow compaction after
    // the tool round preserves all pairings.
    //
    // Scenario:
    //   0. Pre-seed 24 messages
    //   1. FakeAdapter call 0 → 2 tool_calls (read_file × 2)
    //   2. Harness executes both → 2 tool_results added
    //   3. FakeAdapter call 1 → context_length_exceeded
    //   4. Overflow compaction → model summary, preserving both pairs
    //   5. FakeAdapter call 3 → final text
    //   6. Turn Completed

    let workspace = setup_test_workspace("forge-multi-tool-overflow");
    let harness = Arc::new(Harness::new(workspace.clone()));
    let session_id = "session-multi-tool-overflow".to_string();

    let tool_id_read = "call-read-multi".to_string();
    let tool_id_read2 = "call-read-multi-2".to_string();

    // Response 0: 2 tool_calls
    let response_multi_tool = StreamResult {
        assistant_content: vec![
            serde_json::json!({ "type": "text", "text": "让我同时看源码和检查编译" }),
            serde_json::json!({
                "type": "tool_use",
                "id": tool_id_read.clone(),
                "name": "read_file",
                "input": {"path": "src/main.rs"}
            }),
            serde_json::json!({
                "type": "tool_use",
                "id": tool_id_read2.clone(),
                "name": "read_file",
                "input": {"path": "src/main.rs"}
            }),
        ],
        tool_calls: vec![
            ToolCall {
                id: tool_id_read.clone(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "src/main.rs"}),
            },
            ToolCall {
                id: tool_id_read2.clone(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "src/main.rs"}),
            },
        ],
        stop_reason: Some("tool_use".to_string()),
    };

    let response_final = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "分析完成，编译通过"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };
    let response_compact_summary = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "<summary>\nMODEL MULTI TOOL OVERFLOW SUMMARY\n</summary>"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new_with_errors(vec![
        Ok(response_multi_tool),
        Err("context_length_exceeded: too many tokens".to_string()),
        Ok(response_compact_summary),
        Ok(response_final),
    ]));

    let session = AgentSession::new(
        session_id.clone(),
        "openai".to_string(),
        adapter.clone(),
        harness.clone(),
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    // Pre-seed 24 messages. After send_message adds 1 user + 2 tool rounds
    // (assistant with 2 tool_uses + user with 2 tool_results) = 28 total.
    // split_at = 28 - 16 = 12 >= 8 → compaction proceeds.
    {
        let mut msgs = lock_unpoisoned(&session.messages);
        for i in 0..12 {
            msgs.push(ChatMessage::user(&format!("消息 {}", i)));
            msgs.push(ChatMessage::assistant(serde_json::json!([
                { "type": "text", "text": format!("回复 {}", i) }
            ])));
        }
    }

    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    let turn_guard = session.reserve_turn().expect("reserve turn");

    let result = session
        .send_message_with_emitter(
            "读取源码并检查编译",
            &emitter,
            vec![],
            None,
            None,
            turn_guard,
        )
        .await;

    assert!(
        result.is_ok(),
        "multi-tool + overflow turn should succeed: {:?}",
        result.err()
    );

    // 1. Turn completed
    {
        let turn = lock_unpoisoned(&session.latest_turn);
        assert_eq!(turn.as_ref().unwrap().status, AgentTurnStatus::Completed);
    }

    // 2. Messages compacted
    let messages = lock_unpoisoned(&session.messages);
    assert!(
        messages.len() < 28,
        "messages should be compacted: expected < 28, got {}",
        messages.len()
    );

    // 3. Both tool_use/tool_result pairs survive compaction
    //    Find all tool_use blocks and verify each has a matching tool_result
    let tool_use_blocks: Vec<_> = messages
        .iter()
        .flat_map(|m| {
            m.content.as_array().into_iter().flat_map(|blocks| {
                blocks.iter().filter_map(|b| {
                    if b.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                        Some((
                            m.role.clone(),
                            b.get("id").and_then(|v| v.as_str()).map(String::from),
                            b.get("name").and_then(|v| v.as_str()).map(String::from),
                        ))
                    } else {
                        None
                    }
                })
            })
        })
        .collect();

    assert_eq!(
        tool_use_blocks.len(),
        2,
        "should have 2 tool_use blocks, got {}",
        tool_use_blocks.len()
    );

    // Verify each tool_use has a matching tool_result immediately after
    for (role, id, name) in &tool_use_blocks {
        assert_eq!(role, "assistant");
        let id = id.as_ref().expect("tool_use id");
        let name = name.as_ref().expect("tool_use name");

        // Find the tool_result for this id
        let has_matching_result = messages.iter().any(|m| {
            m.role == "user"
                && m.content.as_array().is_some_and(|blocks| {
                    blocks.iter().any(|b| {
                        b.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                            && b.get("tool_use_id").and_then(|v| v.as_str()) == Some(id.as_str())
                    })
                })
        });
        assert!(
            has_matching_result,
            "tool_use '{}' ({}) should have matching tool_result",
            name, id
        );
    }

    // 4. Tool results contain correct content
    let tool_result_contents: Vec<String> = messages
        .iter()
        .flat_map(|m| {
            m.content.as_array().into_iter().flat_map(|blocks| {
                blocks.iter().filter_map(|b| {
                    if b.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                        b.get("content").and_then(|v| v.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
            })
        })
        .collect();

    assert!(
        tool_result_contents
            .iter()
            .any(|c| c.contains("hello world")),
        "read_file result should contain file content"
    );

    // 5. Summary set
    assert!(lock_unpoisoned(&session.summary).is_some());

    // 6. Adapter called exactly 4 times: multi-tool + overflow + summary + retry
    assert_eq!(
        adapter.call_count.load(std::sync::atomic::Ordering::SeqCst),
        4
    );

    // 7. Final text from retry
    let final_text = messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .and_then(|m| m.content.as_array())
        .and_then(|blocks| {
            blocks.iter().find_map(|b| {
                if b.get("type").and_then(|v| v.as_str()) == Some("text") {
                    b.get("text").and_then(|v| v.as_str()).map(String::from)
                } else {
                    None
                }
            })
        })
        .expect("final text");
    assert!(final_text.contains("分析完成"));

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn snapshot_and_turn_state_bind_to_workspace_after_complete_turn() {
    // This test proves that after a complete FakeAdapter turn, the session's
    // turn state, message history, and snapshot all reflect the correct workspace.
    //
    // Verified:
    //   - Turn state is Completed
    //   - Latest turn has evidence (tool traces)
    //   - Message history has correct structure
    //   - Session snapshot serializes with correct session_id and workspace

    let workspace = setup_test_workspace("forge-snapshot-workspace");
    let harness = Arc::new(Harness::new(workspace.clone()));
    let session_id = "session-snapshot-ws".to_string();

    let tool_call_id = "call-read-snap".to_string();

    let response_tool = StreamResult {
        assistant_content: vec![
            serde_json::json!({ "type": "text", "text": "查看文件" }),
            serde_json::json!({
                "type": "tool_use",
                "id": tool_call_id.clone(),
                "name": "read_file",
                "input": {"path": "src/main.rs"}
            }),
        ],
        tool_calls: vec![ToolCall {
            id: tool_call_id.clone(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "src/main.rs"}),
        }],
        stop_reason: Some("tool_use".to_string()),
    };

    let response_final = StreamResult {
        assistant_content: vec![serde_json::json!({
            "type": "text",
            "text": "文件内容已确认"
        })],
        tool_calls: vec![],
        stop_reason: Some("end_turn".to_string()),
    };

    let adapter = Arc::new(FakeAdapter::new(vec![response_tool, response_final]));
    let session = AgentSession::new(
        session_id.clone(),
        "deepseek".to_string(),
        adapter.clone(),
        harness.clone(),
        "你是一个编程助手".to_string(),
        Some(128_000),
    );

    let emitter = crate::agent::event_sink::NoopEventEmitter;
    let turn_guard = session.reserve_turn().expect("reserve turn");

    let result = session
        .send_message_with_emitter("查看 src/main.rs", &emitter, vec![], None, None, turn_guard)
        .await;

    assert!(result.is_ok(), "turn should succeed: {:?}", result.err());

    // 1. Turn state is Completed
    {
        let turn = lock_unpoisoned(&session.latest_turn);
        let turn = turn.as_ref().expect("latest turn");
        assert_eq!(turn.status, AgentTurnStatus::Completed);
    }

    // 2. Message history: user, assistant(tool_use), user(tool_result), assistant(text)
    let messages = lock_unpoisoned(&session.messages);
    assert!(
        messages.len() >= 4,
        "expected >= 4 messages, got {}",
        messages.len()
    );
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[2].role, "user");
    // Last assistant message
    let last = messages.last().unwrap();
    assert_eq!(last.role, "assistant");

    // 3. tool_result references correct tool_use id
    let result_blocks = messages[2].content.as_array().expect("tool result blocks");
    assert!(result_blocks.iter().any(|b| {
        b.get("type").and_then(|v| v.as_str()) == Some("tool_result")
            && b.get("tool_use_id").and_then(|v| v.as_str()) == Some(tool_call_id.as_str())
    }));

    // 4. Session id is correct
    assert_eq!(session.id, "session-snapshot-ws");

    // 5. Working dir is the test workspace
    assert_eq!(session.harness.working_dir, workspace);

    // 6. Snapshot can be constructed (tests snapshot module separately, but
    //    verify the basic data needed for snapshot is available)
    let turn = lock_unpoisoned(&session.latest_turn);
    let turn = turn.as_ref().unwrap();
    // Turn should have tool evidence
    assert!(!turn.tools.is_empty(), "turn should have tool traces");

    // 7. System prompt was set
    let sp = lock_unpoisoned(&session.system_prompt);
    assert!(!sp.is_empty(), "system prompt should be set");

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn kill_with_emitter_cancels_inflight_turn_and_recovery_succeeds() {
    // This test proves the production cancel/stop path end-to-end:
    //   1. Spawn send_message_with_emitter — adapter blocks on session's cancel token.
    //   2. Call kill_with_emitter from test thread — fires cancel token.
    //   3. Adapter wakes → returns error → agent loop sees running=false →
    //      marks turn Cancelled (agent loop does this, not kill_with_emitter).
    //   4. Verify: turn=Cancelled, status=Stopped, SessionStopped event emitted.
    //   5. Verify: message history and tool_use/tool_result pairing preserved.
    //   6. After resume, recovery turn succeeds.
    //
    // This mirrors IPC kill_session: the IPC handler calls session.kill() which
    // sets running=false and fires the cancel token. The agent loop (running in
    // a separate tokio task) sees the token, the adapter returns, and the loop
    // marks the turn as Cancelled.

    let workspace = setup_test_workspace("forge-kill-concurrent");
    let harness = Arc::new(Harness::new(workspace.clone()));
    std::fs::write(workspace.join("data.txt"), "kill-concurrent-data\n").expect("write data.txt");

    let adapter = Arc::new(CancellableFakeAdapter::new());
    let session = Arc::new(AgentSession::new(
        "session-kill-concurrent".to_string(),
        "deepseek".to_string(),
        adapter.clone(),
        harness,
        "你是一个编程助手".to_string(),
        Some(128_000),
    ));

    let emitter = Arc::new(crate::agent::event_sink::CollectingEventEmitter::new());

    // 1. Spawn the turn — adapter will block on cancel token in call 2
    let turn_guard = session.reserve_turn().expect("reserve turn");
    let s2 = session.clone();
    let e2 = emitter.clone();
    let handle = tokio::spawn(async move {
        s2.send_message_with_emitter("读取 data.txt", &*e2, vec![], None, None, turn_guard)
            .await
    });

    // 2. Wait for adapter to reach its blocking point (call 2, waiting on cancel token)
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !adapter.ready.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("adapter should reach blocking call");

    // 3. Kill via emitter — fires cancel token, no latest_turn lock, no deadlock
    let kill_emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    session.kill_with_emitter(&kill_emitter);

    // 4. Wait for the spawned turn to finish
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle)
        .await
        .expect("killed turn should finish")
        .expect("task should not panic");
    assert!(result.is_err(), "killed turn should return error");

    // 5. Verify kill state
    assert!(!session.running.load(Ordering::SeqCst));
    assert_eq!(
        *lock_unpoisoned(&session.status),
        crate::agent::session::SessionStatus::Stopped
    );
    {
        let turn = lock_unpoisoned(&session.latest_turn);
        let turn = turn.as_ref().expect("latest turn");
        assert_eq!(
            turn.status,
            AgentTurnStatus::Cancelled,
            "agent loop should mark turn as Cancelled after kill"
        );
    }

    // 6. Verify SessionStopped event
    let kill_events = kill_emitter.drain();
    assert!(
        kill_events.iter().any(|e| matches!(
            e,
            crate::protocol::events::StreamEvent::SessionStopped { reason, .. }
                if reason == "killed"
        )),
        "kill should emit SessionStopped, got: {:?}",
        kill_events
            .iter()
            .map(std::mem::discriminant)
            .collect::<Vec<_>>()
    );

    // 7. Verify message history preserves tool_use/tool_result pairing
    {
        let messages = lock_unpoisoned(&session.messages);
        assert!(
            messages.len() >= 3,
            "should have user + assistant(tool_use) + user(tool_result), got {}",
            messages.len()
        );
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[2].role, "user");

        let tool_use_id = messages[1]
            .content
            .as_array()
            .and_then(|blocks| {
                blocks.iter().find_map(|b| {
                    if b.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                        b.get("id").and_then(|v| v.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
            })
            .expect("assistant should have tool_use");
        let result_blocks = messages[2].content.as_array().expect("tool result blocks");
        assert!(
            result_blocks.iter().any(|b| {
                b.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                    && b.get("tool_use_id").and_then(|v| v.as_str()) == Some(tool_use_id.as_str())
            }),
            "tool_result should reference tool_use id"
        );
    }

    // 8. Recovery — resume and send new turn
    let recovery_emitter = crate::agent::event_sink::CollectingEventEmitter::new();
    session.running.store(true, Ordering::SeqCst);
    *lock_unpoisoned(&session.status) = crate::agent::session::SessionStatus::Running;
    *lock_unpoisoned(&session.cancel) = Some(Arc::new(tokio::sync::Notify::new()));

    let turn_guard2 = session.reserve_turn().expect("reserve recovery turn");
    let recovery_result = session
        .send_message_with_emitter("继续", &recovery_emitter, vec![], None, None, turn_guard2)
        .await;
    assert!(
        recovery_result.is_ok(),
        "recovery should succeed: {:?}",
        recovery_result.err()
    );
    {
        let turn = lock_unpoisoned(&session.latest_turn);
        assert_eq!(
            turn.as_ref().unwrap().status,
            AgentTurnStatus::Completed,
            "recovery turn should be Completed"
        );
    }

    let _ = std::fs::remove_dir_all(&workspace);
}

// -- Loop guard recovery detail tests ----------------------------

#[test]
fn loop_guard_recovery_detail_contains_specific_advice_per_stop_reason() {
    use crate::agent::loop_guard::LoopStopReason;

    // ModelRoundLimit
    let detail = loop_guard_recovery_detail(&LoopStopReason::ModelRoundLimit);
    assert!(detail.contains("model_round_limit"));
    assert!(
        detail.contains("narrow the task") || detail.contains("accept partial results"),
        "model_round_limit should suggest narrowing or accepting partials: {detail}"
    );

    // ToolCallLimit
    let detail = loop_guard_recovery_detail(&LoopStopReason::ToolCallLimit);
    assert!(detail.contains("tool_call_limit"));
    assert!(
        detail.contains("reduce file scope") || detail.contains("limit tool actions"),
        "tool_call_limit should suggest reducing scope: {detail}"
    );

    // RepeatedCategoryBatch
    let detail = loop_guard_recovery_detail(&LoopStopReason::RepeatedCategoryBatch);
    assert!(detail.contains("repeated_category_batch"));
    assert!(
        detail.contains("stop exploring") || detail.contains("synthesize conclusions"),
        "repeated_category_batch should suggest synthesizing from existing reads: {detail}"
    );

    // RepeatedNoProgress
    let detail = loop_guard_recovery_detail(&LoopStopReason::RepeatedNoProgress);
    assert!(detail.contains("repeated_no_progress"));
    assert!(
        detail.contains("switch strategy") || detail.contains("ask for clarification"),
        "repeated_no_progress should suggest switching strategy: {detail}"
    );

    // CompactUnavailable
    let detail = loop_guard_recovery_detail(&LoopStopReason::CompactUnavailable);
    assert!(detail.contains("compact_unavailable"));
    assert!(
        detail.contains("compact") || detail.contains("split task"),
        "compact_unavailable should suggest compact or split: {detail}"
    );

    // RepeatedOverflow
    let detail = loop_guard_recovery_detail(&LoopStopReason::RepeatedOverflow);
    assert!(detail.contains("repeated_overflow"));
    assert!(
        detail.contains("compact") || detail.contains("reduce context"),
        "repeated_overflow should suggest compact or reduce context: {detail}"
    );

    // ToolLoopDetected
    let detail = loop_guard_recovery_detail(&LoopStopReason::ToolLoopDetected);
    assert!(detail.contains("tool_loop_detected"));
    assert!(
        detail.contains("different approach"),
        "tool_loop_detected should suggest a different approach: {detail}"
    );
}

#[test]
fn snapshot_includes_goal_ledger() {
    use crate::agent::goal_state::{GoalLedger, GoalStatus, GoalTaskStatus};

    let workspace = std::env::temp_dir().join(format!(
        "forge-session-goal-ledger-snapshot-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-goal".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );

    let ledger = GoalLedger::new_active("goal-1", "Ship feature", vec!["Step 1".to_string()], 10);
    session.set_goal_ledger(ledger);

    let snapshot = session.snapshot();
    let goal = snapshot
        .goal_ledger
        .as_ref()
        .unwrap()
        .current_goal()
        .unwrap();
    assert_eq!(goal.id, "goal-1");
    assert_eq!(goal.status, GoalStatus::Active);
    assert_eq!(goal.tasks[0].status, GoalTaskStatus::Pending);

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn restore_state_preserves_goal_ledger() {
    use crate::agent::goal_state::{GoalLedger, GoalTaskStatus};

    let workspace = std::env::temp_dir().join(format!(
        "forge-session-goal-ledger-restore-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-restore-goal".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );

    let mut ledger = GoalLedger::new_active(
        "goal-resume",
        "Persist goal",
        vec!["Task A".to_string(), "Task B".to_string()],
        10,
    );
    ledger.update_task_status("task-2", GoalTaskStatus::Completed, 20);

    session.restore_state(
        vec![ChatMessage::user("hello")],
        None,
        None,
        Some(ledger),
        None,
    );

    let current = session.current_goal().unwrap();
    assert_eq!(current.id, "goal-resume");
    assert_eq!(current.tasks[0].status, GoalTaskStatus::Pending);
    assert_eq!(current.tasks[1].status, GoalTaskStatus::Completed);

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn resume_normalizes_goal_ledger_in_progress_tasks() {
    use crate::agent::goal_state::{GoalLedger, GoalStatus, GoalTaskStatus};

    let workspace = std::env::temp_dir().join(format!(
        "forge-session-goal-ledger-resume-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter = Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat"));
    let session = AgentSession::new(
        "session-resume-norm".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        Some(128_000),
    );

    let mut ledger = GoalLedger::new_active(
        "goal-resume",
        "Persist goal",
        vec!["Task A".to_string(), "Task B".to_string()],
        10,
    );
    ledger.update_task_status("task-1", GoalTaskStatus::InProgress, 20);
    ledger.update_task_status("task-2", GoalTaskStatus::Completed, 20);
    session.set_goal_ledger(ledger);

    // Simulate what resume() does for the goal ledger without requiring an AppHandle
    session.normalize_goal_ledger_for_resume();

    let current = session.current_goal().unwrap();
    assert_eq!(current.status, GoalStatus::Active);
    assert_eq!(current.tasks[0].status, GoalTaskStatus::Pending);
    assert_eq!(
        current.tasks[0].resume_note.as_deref(),
        Some("task was in progress when the session was restored")
    );
    assert_eq!(current.tasks[1].status, GoalTaskStatus::Completed);

    let _ = std::fs::remove_dir_all(workspace);
}
