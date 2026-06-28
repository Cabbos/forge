#[cfg(test)]
mod tests {
    use crate::adapters::base::{AdapterError, AiAdapter, StreamResult};
    use crate::agent::event_sink::CollectingEventEmitter;
    use crate::agent::loop_guard::LoopStopReason;
    use crate::agent::session::{AgentSession, AgentTurnRunRequest};
    use crate::agent::session_guards::try_begin_turn;
    use crate::agent::turn_state::AgentTurnStatus;
    use crate::harness::Harness;
    use async_trait::async_trait;
    use std::collections::VecDeque;
    use std::sync::{atomic::Ordering, Arc, Mutex};

    fn temp_workspace(name: &str) -> std::path::PathBuf {
        let workspace = std::env::temp_dir().join(format!(
            "forge-session-loop-test-{name}-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        workspace
    }

    /// Mock adapter that returns queued `StreamResult`s from `call()`.
    /// This drives both `stream_message_with_emitter` and `call` through the
    /// default trait implementations.
    struct QueuedAdapter {
        queue: Mutex<VecDeque<Result<StreamResult, AdapterError>>>,
        model_id: String,
        model_name: String,
    }

    impl QueuedAdapter {
        fn new(results: Vec<Result<StreamResult, AdapterError>>) -> Self {
            Self {
                queue: Mutex::new(results.into_iter().collect()),
                model_id: "mock-model".to_string(),
                model_name: "Mock Model".to_string(),
            }
        }
    }

    #[async_trait]
    impl AiAdapter for QueuedAdapter {
        async fn call(
            &self,
            _messages: &[crate::adapters::base::ChatMessage],
            _cancel: Arc<tokio::sync::Notify>,
        ) -> Result<StreamResult, AdapterError> {
            self.queue.lock().unwrap().pop_front().unwrap_or_else(|| {
                Ok(StreamResult {
                    assistant_content: vec![serde_json::json!("default")],
                    tool_calls: vec![],
                    stop_reason: Some("stop".to_string()),
                })
            })
        }

        fn model_id(&self) -> &str {
            &self.model_id
        }

        fn model_name(&self) -> &str {
            &self.model_name
        }
    }

    fn no_tool_result(content: &str) -> StreamResult {
        StreamResult {
            assistant_content: vec![serde_json::json!(content)],
            tool_calls: vec![],
            stop_reason: Some("stop".to_string()),
        }
    }

    fn make_session(workspace: &std::path::Path, adapter: Arc<dyn AiAdapter>) -> AgentSession {
        let harness = Arc::new(Harness::new(workspace.to_path_buf()));
        AgentSession::new(
            "session-1".to_string(),
            "claude".to_string(),
            adapter,
            harness,
            "You are a helpful assistant".to_string(),
            None,
        )
    }

    // ── Pure helpers ──────────────────────────────────────────────────────

    #[test]
    fn final_summary_text_collects_plain_text_blocks() {
        use crate::agent::session::r#loop::final_summary_text;
        let blocks = vec![
            serde_json::json!("hello "),
            serde_json::json!({"type": "text", "text": "world"}),
        ];
        assert_eq!(final_summary_text(&blocks), "hello world");
    }

    #[test]
    fn final_summary_text_ignores_non_text_blocks() {
        use crate::agent::session::r#loop::final_summary_text;
        let blocks = vec![serde_json::json!({"type": "tool_use", "name": "read"})];
        assert_eq!(final_summary_text(&blocks), "");
    }

    #[test]
    fn loop_guard_recovery_detail_contains_reason_and_action() {
        use crate::agent::session::r#loop::loop_guard_recovery_detail;
        let detail = loop_guard_recovery_detail(&LoopStopReason::ModelRoundLimit);
        assert!(detail.contains("model_round_limit"));
        assert!(detail.contains("narrow the task"));
    }

    // ── setup_turn ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn setup_turn_appends_user_message_and_sets_status() {
        let workspace = temp_workspace("setup");
        let adapter = Arc::new(QueuedAdapter::new(vec![]));
        let session = make_session(&workspace, adapter);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let hidden = session
            .setup_turn("hello tests", vec![], None, None, &*emitter)
            .await;

        assert!(hidden.is_empty());

        let messages = session.messages.lock().clone();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "hello tests");

        let latest = session.latest_turn.lock().clone().expect("turn created");
        assert_eq!(latest.status, AgentTurnStatus::GatheringContext);

        let _ = std::fs::remove_dir_all(&workspace);
    }

    // ── run_agent_turn (no tool calls) ─────────────────────────────────────

    #[tokio::test]
    async fn run_agent_turn_with_no_tool_calls_completes() {
        let workspace = temp_workspace("run-no-tools");
        let adapter = Arc::new(QueuedAdapter::new(vec![
            Ok(no_tool_result("I can help.")),
            Ok(no_tool_result("Done.")),
        ]));
        let session = make_session(&workspace, adapter);
        let emitter: Arc<dyn crate::agent::event_sink::EventEmitter> =
            Arc::new(CollectingEventEmitter::new());

        let guard = try_begin_turn(session.turn_inflight.clone()).expect("begin turn");
        let request = AgentTurnRunRequest {
            text: "say hi",
            hidden_contexts: vec![],
            turn_metadata: None,
            activation_text: None,
            _turn_guard: guard,
            emitter: &*emitter,
            tool_emitter: Some(emitter.clone()),
            app_handle: None,
        };

        session.run_agent_turn(request).await.expect("turn ok");

        assert!(
            session.running.load(Ordering::SeqCst),
            "session should still be running"
        );

        let latest = session.latest_turn.lock().clone().expect("turn exists");
        assert!(
            matches!(latest.status, AgentTurnStatus::Completed),
            "expected Completed, got {:?}",
            latest.status
        );

        // Assistant content should have been appended twice (round + final summary).
        let messages = session.messages.lock().clone();
        let assistant_texts: Vec<String> = messages
            .iter()
            .filter(|m| m.role == "assistant")
            .map(|m| {
                if let Some(arr) = m.content.as_array() {
                    crate::agent::session::r#loop::final_summary_text(arr)
                } else {
                    m.content.as_str().unwrap_or("").to_string()
                }
            })
            .collect();
        assert!(
            assistant_texts
                .iter()
                .any(|s| s.contains("I can help.") || s.contains("Done.")),
            "assistant texts: {:?}",
            assistant_texts
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn run_agent_turn_executes_read_tool_and_continues() {
        let workspace = temp_workspace("run-read-tool");
        std::fs::write(workspace.join("file.txt"), "tool payload").expect("write");

        let adapter = Arc::new(QueuedAdapter::new(vec![
            Ok(StreamResult {
                assistant_content: vec![serde_json::json!({
                    "type": "tool_use",
                    "id": "tu_1",
                    "name": "read_file",
                    "input": {"path": "file.txt"}
                })],
                tool_calls: vec![crate::adapters::base::ToolCall {
                    id: "tu_1".to_string(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({"path": "file.txt"}),
                }],
                stop_reason: Some("tool_use".to_string()),
            }),
            Ok(no_tool_result("I read the file.")),
            Ok(no_tool_result("Done.")),
        ]));
        let session = make_session(&workspace, adapter);
        let emitter: Arc<dyn crate::agent::event_sink::EventEmitter> =
            Arc::new(CollectingEventEmitter::new());

        let guard = try_begin_turn(session.turn_inflight.clone()).expect("begin turn");
        let request = AgentTurnRunRequest {
            text: "read the file",
            hidden_contexts: vec![],
            turn_metadata: None,
            activation_text: None,
            _turn_guard: guard,
            emitter: &*emitter,
            tool_emitter: Some(emitter.clone()),
            app_handle: None,
        };

        session.run_agent_turn(request).await.expect("turn ok");

        let messages = session.messages.lock().clone();
        // Should contain user prompt, assistant tool_use, tool_result, final assistant.
        let has_tool_result = messages.iter().any(|m| {
            m.role == "user"
                && m.content.as_array().is_some_and(|arr| {
                    arr.iter()
                        .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_result"))
                })
        });
        assert!(has_tool_result, "tool_result should be in history");

        let latest = session.latest_turn.lock().clone().expect("turn exists");
        assert!(
            matches!(latest.status, AgentTurnStatus::Completed),
            "expected Completed, got {:?}",
            latest.status
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn run_agent_turn_stops_when_session_not_running() {
        let workspace = temp_workspace("not-running");
        let adapter = Arc::new(QueuedAdapter::new(vec![]));
        let session = make_session(&workspace, adapter);
        session.running.store(false, Ordering::SeqCst);

        let guard = try_begin_turn(session.turn_inflight.clone()).expect("begin turn");
        let emitter: Arc<dyn crate::agent::event_sink::EventEmitter> =
            Arc::new(CollectingEventEmitter::new());
        let request = AgentTurnRunRequest {
            text: "ignored",
            hidden_contexts: vec![],
            turn_metadata: None,
            activation_text: None,
            _turn_guard: guard,
            emitter: &*emitter,
            tool_emitter: Some(emitter.clone()),
            app_handle: None,
        };

        let err = session
            .run_agent_turn(request)
            .await
            .expect_err("should fail");
        assert!(err.contains("not running"));

        let _ = std::fs::remove_dir_all(&workspace);
    }
}
