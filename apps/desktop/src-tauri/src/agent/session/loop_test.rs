#[cfg(test)]
mod tests {
    use crate::adapters::base::{AdapterError, AiAdapter, ChatMessage, StreamResult, ToolCall};
    use crate::agent::event_sink::{CollectingEventEmitter, EventEmitter};
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

    // ── Journal shadow-mode mutation ordering ─────────────────────────────

    use crate::agent::session_journal::{
        SessionJournalStore, SessionMutation, SessionMutationEnvelope,
        SESSION_JOURNAL_SCHEMA_VERSION,
    };
    use crate::agent::session_mutation::{SessionJournalHandle, SessionJournalMode};
    use crate::agent::session_projection::SessionProjection;
    use crate::ipc::session_lifecycle::snapshots_restore_parity_equivalent;

    fn journal_root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "forge-session-loop-journal-{name}-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&root).expect("journal root");
        root
    }

    fn attach_journal(session: &AgentSession, root: &std::path::Path) {
        let store = SessionJournalStore::new(root.to_path_buf(), session.id.clone())
            .expect("journal store");
        session
            .attach_session_journal(SessionJournalHandle::new(store, SessionJournalMode::Shadow));
    }

    fn attach_initialized_journal(session: &AgentSession, root: &std::path::Path) {
        let store = SessionJournalStore::new(root.to_path_buf(), session.id.clone())
            .expect("journal store");
        if store.load().map(|r| r.events.is_empty()).unwrap_or(true) {
            let _ = store.append(SessionMutationEnvelope {
                schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
                event_id: String::new(),
                session_id: session.id.clone(),
                sequence: 0,
                created_at_ms: 1,
                mutation: SessionMutation::SessionInitialized {
                    provider: session.agent_type.clone(),
                    model: session.model_id.clone(),
                    working_dir: session.harness.working_dir.to_string_lossy().to_string(),
                },
            });
        }
        session
            .attach_session_journal(SessionJournalHandle::new(store, SessionJournalMode::Shadow));
    }

    fn assert_journal_parity(session: &AgentSession, journal_root: &std::path::Path) {
        let events = SessionJournalStore::new(journal_root.to_path_buf(), session.id.clone())
            .expect("loader store")
            .load()
            .expect("journal load")
            .events;
        let projection = SessionProjection::from_events(&events)
            .expect("projection")
            .to_snapshot();
        let snapshot = session.snapshot();
        assert!(
            snapshots_restore_parity_equivalent(&snapshot, &projection),
            "journal replay must parity-match saved snapshot\nsnapshot: {snapshot:?}\nprojection: {projection:?}"
        );
    }

    fn journal_events(root: &std::path::Path, session_id: &str) -> Vec<SessionMutation> {
        SessionJournalStore::new(root.to_path_buf(), session_id.to_string())
            .expect("loader store")
            .load()
            .expect("journal load")
            .events
            .into_iter()
            .map(|event| event.mutation)
            .collect()
    }

    fn journaled_messages(
        mutations: &[SessionMutation],
    ) -> Vec<crate::adapters::base::ChatMessage> {
        mutations
            .iter()
            .filter_map(|mutation| match mutation {
                SessionMutation::MessageAppended { message } => Some(message.clone()),
                _ => None,
            })
            .collect()
    }

    /// Adapter that, on the first provider call, checks whether the user
    /// message has already been journaled. Proves the journal append happens
    /// before the provider call.
    struct JournalProbeAdapter {
        journal_path: std::path::PathBuf,
        expected_user_text: String,
        observed: Mutex<Option<bool>>,
        fallback: QueuedAdapter,
    }

    #[async_trait]
    impl AiAdapter for JournalProbeAdapter {
        async fn call(
            &self,
            messages: &[crate::adapters::base::ChatMessage],
            cancel: Arc<tokio::sync::Notify>,
        ) -> Result<StreamResult, AdapterError> {
            {
                let mut observed = self.observed.lock().unwrap();
                if observed.is_none() {
                    let found = std::fs::read_to_string(&self.journal_path)
                        .map(|raw| raw.contains(&self.expected_user_text))
                        .unwrap_or(false);
                    *observed = Some(found);
                }
            }
            self.fallback.call(messages, cancel).await
        }

        fn model_id(&self) -> &str {
            self.fallback.model_id()
        }

        fn model_name(&self) -> &str {
            self.fallback.model_name()
        }
    }

    #[tokio::test]
    async fn user_message_is_journaled_before_provider_call() {
        let workspace = temp_workspace("journal-user-first");
        let root = journal_root("user-first");
        let journal_path = root
            .join("sessions")
            .join("session-1")
            .join("mutations.jsonl");
        let adapter = Arc::new(JournalProbeAdapter {
            journal_path,
            expected_user_text: "probe me".to_string(),
            observed: Mutex::new(None),
            fallback: QueuedAdapter::new(vec![Ok(no_tool_result("done"))]),
        });
        let session = make_session(&workspace, adapter.clone());
        attach_journal(&session, &root);
        let emitter: Arc<dyn crate::agent::event_sink::EventEmitter> =
            Arc::new(CollectingEventEmitter::new());

        let guard = try_begin_turn(session.turn_inflight.clone()).expect("begin turn");
        session
            .run_agent_turn(AgentTurnRunRequest {
                text: "probe me",
                hidden_contexts: vec![],
                turn_metadata: None,
                activation_text: None,
                _turn_guard: guard,
                emitter: &*emitter,
                tool_emitter: Some(emitter.clone()),
                app_handle: None,
            })
            .await
            .expect("turn ok");

        assert_eq!(
            *adapter.observed.lock().unwrap(),
            Some(true),
            "user message must be journaled before the provider call"
        );

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn assistant_and_tool_result_messages_are_journaled_in_order() {
        let workspace = temp_workspace("journal-order");
        std::fs::write(workspace.join("file.txt"), "tool payload").expect("write");
        let root = journal_root("order");

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
            Ok(no_tool_result("Final summary.")),
        ]));
        let session = make_session(&workspace, adapter);
        attach_journal(&session, &root);
        let emitter: Arc<dyn crate::agent::event_sink::EventEmitter> =
            Arc::new(CollectingEventEmitter::new());

        let guard = try_begin_turn(session.turn_inflight.clone()).expect("begin turn");
        session
            .run_agent_turn(AgentTurnRunRequest {
                text: "read the file",
                hidden_contexts: vec![],
                turn_metadata: None,
                activation_text: None,
                _turn_guard: guard,
                emitter: &*emitter,
                tool_emitter: Some(emitter.clone()),
                app_handle: None,
            })
            .await
            .expect("turn ok");

        let mutations = journal_events(&root, &session.id);
        let messages = journaled_messages(&mutations);
        assert!(
            messages.len() >= 3,
            "expected user, assistant, tool-result appends; got {messages:?}"
        );
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "read the file");
        assert_eq!(messages[1].role, "assistant");
        let tool_result_index = messages.iter().position(|message| {
            message.role == "user"
                && message.content.as_array().is_some_and(|blocks| {
                    blocks.iter().any(|block| {
                        block.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                    })
                })
        });
        assert!(
            tool_result_index.is_some(),
            "tool result must be journaled: {messages:?}"
        );
        // The assistant message is journaled before tool dispatch, which is
        // what produces the tool-result message.
        assert!(tool_result_index.unwrap() > 1);
        // The final-summary assistant message is journaled too.
        let final_summary = messages.last().expect("final summary journaled");
        assert_eq!(final_summary.role, "assistant");

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn auto_continuation_prompts_are_journaled() {
        let workspace = temp_workspace("journal-continuation");
        let root = journal_root("continuation");
        let adapter = Arc::new(QueuedAdapter::new(vec![
            Ok(no_tool_result("working")),
            Ok(no_tool_result("still working")),
            Ok(no_tool_result("more work")),
            Ok(no_tool_result("wrapping up")),
        ]));
        let session = make_session(&workspace, adapter);
        attach_journal(&session, &root);
        session.set_goal_ledger(crate::agent::goal_state::GoalLedger::new_active(
            "goal-1",
            "finish everything",
            vec!["pending task".to_string()],
            1,
        ));
        let emitter: Arc<dyn crate::agent::event_sink::EventEmitter> =
            Arc::new(CollectingEventEmitter::new());

        let guard = try_begin_turn(session.turn_inflight.clone()).expect("begin turn");
        session
            .run_agent_turn(AgentTurnRunRequest {
                text: "do the work",
                hidden_contexts: vec![],
                turn_metadata: None,
                activation_text: None,
                _turn_guard: guard,
                emitter: &*emitter,
                tool_emitter: Some(emitter.clone()),
                app_handle: None,
            })
            .await
            .expect("turn ok");

        let mutations = journal_events(&root, &session.id);
        let messages = journaled_messages(&mutations);
        let continuations = messages
            .iter()
            .filter(|message| {
                message.role == "user"
                    && message
                        .content
                        .as_str()
                        .is_some_and(|text| text.contains("Please continue working"))
            })
            .count();
        assert_eq!(
            continuations,
            crate::agent::session::MAX_AUTO_CONTINUATIONS,
            "every auto-continuation prompt must be journaled: {messages:?}"
        );

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn compaction_journals_single_conversation_replacement() {
        let workspace = temp_workspace("journal-compaction");
        let root = journal_root("compaction");
        let adapter = Arc::new(QueuedAdapter::new(vec![]));
        let session = make_session(&workspace, adapter);
        attach_journal(&session, &root);
        let emitter = CollectingEventEmitter::new();

        let compacted_messages = vec![crate::adapters::base::ChatMessage::user("condensed")];
        let compacted = crate::agent::auto_compact::CompactResult {
            messages: compacted_messages.clone(),
            summary: Some("condensed summary".to_string()),
            stats: Some(crate::agent::auto_compact::CompactStats {
                summary: "condensed summary".to_string(),
                retained_messages: 1,
                compacted_messages: 5,
                estimated_tokens_before: 10_000,
                estimated_tokens_after: 2_000,
            }),
            attempted: true,
            skipped_reason: None,
        };
        let stats = compacted.stats.clone().expect("stats");
        session.apply_compaction_emitter(&compacted, &stats, "manual_compact", &emitter);

        let mutations = journal_events(&root, &session.id);
        let replacements: Vec<_> = mutations
            .iter()
            .filter(|mutation| matches!(mutation, SessionMutation::ConversationReplaced { .. }))
            .collect();
        assert_eq!(
            replacements.len(),
            1,
            "compaction must journal exactly one ConversationReplaced: {mutations:?}"
        );
        match &replacements[0] {
            SessionMutation::ConversationReplaced {
                checkpoint_id,
                messages,
                summary,
            } => {
                assert!(checkpoint_id.starts_with("compact-manual_compact-"));
                assert_eq!(messages.len(), 1);
                assert_eq!(summary.as_deref(), Some("condensed summary"));
            }
            other => unreachable!("filtered above: {other:?}"),
        }
        assert!(
            journaled_messages(&mutations).is_empty(),
            "compaction must never journal per-message appends or deletes"
        );

        let messages = session.messages.lock().clone();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "condensed");
        assert_eq!(session.summary.lock().as_deref(), Some("condensed summary"));

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Shadow replay parity corpus ─────────────────────────────────────────
    //
    // Each test drives a real AgentSession through a scenario with the journal
    // attached, then proves that replaying the on-disk journal produces a
    // snapshot parity-equivalent to the in-memory snapshot. The comparator is
    // the same positive allowlist used by restore selection:
    // session_id/provider/model/working_dir, messages, summary, latest_turn,
    // goal_ledger, and a2a_state. Pending confirms, active tool calls, workflow,
    // delivery, context_window_tokens, and bookkeeping metadata are excluded.

    #[tokio::test]
    async fn plain_chat_journal_parity_matches_snapshot() {
        let workspace = temp_workspace("parity-plain");
        let root = journal_root("parity-plain");
        let adapter = Arc::new(QueuedAdapter::new(vec![
            Ok(no_tool_result("I can help.")),
            Ok(no_tool_result("Done.")),
        ]));
        let session = make_session(&workspace, adapter);
        attach_initialized_journal(&session, &root);
        let emitter: Arc<dyn EventEmitter> = Arc::new(CollectingEventEmitter::new());

        let guard = try_begin_turn(session.turn_inflight.clone()).expect("begin turn");
        session
            .run_agent_turn(AgentTurnRunRequest {
                text: "say hi",
                hidden_contexts: vec![],
                turn_metadata: None,
                activation_text: None,
                _turn_guard: guard,
                emitter: &*emitter,
                tool_emitter: Some(emitter.clone()),
                app_handle: None,
            })
            .await
            .expect("turn ok");

        assert_journal_parity(&session, &root);

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn multiple_tool_calls_journal_parity_matches_snapshot() {
        let workspace = temp_workspace("parity-multi-tool");
        std::fs::write(workspace.join("a.txt"), "alpha").expect("write a");
        std::fs::write(workspace.join("b.txt"), "beta").expect("write b");
        let root = journal_root("parity-multi-tool");

        let adapter = Arc::new(QueuedAdapter::new(vec![
            Ok(StreamResult {
                assistant_content: vec![
                    serde_json::json!({"type": "tool_use", "id": "tu_a", "name": "read_file", "input": {"path": "a.txt"}}),
                    serde_json::json!({"type": "tool_use", "id": "tu_b", "name": "read_file", "input": {"path": "b.txt"}}),
                ],
                tool_calls: vec![
                    ToolCall {
                        id: "tu_a".to_string(),
                        name: "read_file".to_string(),
                        input: serde_json::json!({"path": "a.txt"}),
                    },
                    ToolCall {
                        id: "tu_b".to_string(),
                        name: "read_file".to_string(),
                        input: serde_json::json!({"path": "b.txt"}),
                    },
                ],
                stop_reason: Some("tool_use".to_string()),
            }),
            Ok(no_tool_result("I read both files.")),
            Ok(no_tool_result("Done.")),
        ]));
        let session = make_session(&workspace, adapter);
        attach_initialized_journal(&session, &root);
        let emitter: Arc<dyn EventEmitter> = Arc::new(CollectingEventEmitter::new());

        let guard = try_begin_turn(session.turn_inflight.clone()).expect("begin turn");
        session
            .run_agent_turn(AgentTurnRunRequest {
                text: "read both files",
                hidden_contexts: vec![],
                turn_metadata: None,
                activation_text: None,
                _turn_guard: guard,
                emitter: &*emitter,
                tool_emitter: Some(emitter.clone()),
                app_handle: None,
            })
            .await
            .expect("turn ok");

        assert_journal_parity(&session, &root);

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn permission_denial_journal_parity_matches_snapshot() {
        let workspace = temp_workspace("parity-deny");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let root = journal_root("parity-deny");

        let pending_confirms: Arc<
            tokio::sync::RwLock<
                std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>,
            >,
        > = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
        let harness = Arc::new(Harness::new_with_pending(
            workspace.clone(),
            pending_confirms.clone(),
        ));
        let adapter = Arc::new(QueuedAdapter::new(vec![
            Ok(StreamResult {
                assistant_content: vec![serde_json::json!({
                    "type": "tool_use",
                    "id": "tu-write",
                    "name": "write_to_file",
                    "input": {"path": "secret.txt", "content": "should not write"}
                })],
                tool_calls: vec![ToolCall {
                    id: "tu-write".to_string(),
                    name: "write_to_file".to_string(),
                    input: serde_json::json!({"path": "secret.txt", "content": "should not write"}),
                }],
                stop_reason: Some("tool_use".to_string()),
            }),
            Ok(no_tool_result("I was denied.")),
            Ok(no_tool_result("Done.")),
        ]));
        let session = AgentSession::new(
            "session-deny".to_string(),
            "claude".to_string(),
            adapter,
            harness,
            "system".to_string(),
            None,
        );
        attach_initialized_journal(&session, &root);

        struct DenyPendingEmitter {
            pending_confirms: Arc<
                tokio::sync::RwLock<
                    std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>,
                >,
            >,
        }
        impl EventEmitter for DenyPendingEmitter {
            fn emit(&self, event: crate::protocol::events::StreamEvent) {
                if let crate::protocol::events::StreamEvent::ConfirmAsk { block_id, .. } = event {
                    let pending_confirms = self.pending_confirms.clone();
                    tokio::spawn(async move {
                        for _ in 0..100 {
                            if let Some(sender) = pending_confirms.write().await.remove(&block_id) {
                                let _ = sender.send(false);
                                return;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        }
                    });
                }
            }
        }

        let emitter: Arc<dyn EventEmitter> = Arc::new(DenyPendingEmitter { pending_confirms });
        let turn_guard = session.reserve_turn().expect("reserve turn");
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            session.send_message_with_shared_emitter(
                "write a file",
                emitter,
                vec![],
                None,
                None,
                turn_guard,
            ),
        )
        .await
        .expect("turn should not hang")
        .expect("agent turn should succeed");

        assert_journal_parity(&session, &root);

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn compaction_journal_parity_matches_snapshot() {
        let workspace = temp_workspace("parity-compaction");
        let root = journal_root("parity-compaction");
        let adapter = Arc::new(QueuedAdapter::new(vec![]));
        let session = make_session(&workspace, adapter);
        attach_initialized_journal(&session, &root);
        let emitter = CollectingEventEmitter::new();

        // Seed a few messages first so compaction actually replaces them.
        session
            .append_conversation_message(
                ChatMessage::user("first"),
                crate::agent::session_mutation::SessionMutationSource::UserInput,
            )
            .expect("append first");
        session
            .append_conversation_message(
                ChatMessage::assistant(serde_json::json!("second")),
                crate::agent::session_mutation::SessionMutationSource::AssistantResponse,
            )
            .expect("append second");

        let compacted_messages = vec![ChatMessage::user("condensed")];
        let compacted = crate::agent::auto_compact::CompactResult {
            messages: compacted_messages.clone(),
            summary: Some("condensed summary".to_string()),
            stats: Some(crate::agent::auto_compact::CompactStats {
                summary: "condensed summary".to_string(),
                retained_messages: 1,
                compacted_messages: 2,
                estimated_tokens_before: 10_000,
                estimated_tokens_after: 2_000,
            }),
            attempted: true,
            skipped_reason: None,
        };
        let stats = compacted.stats.clone().expect("stats");
        session.apply_compaction_emitter(&compacted, &stats, "manual_compact", &emitter);

        assert_journal_parity(&session, &root);

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a2a_state_journal_parity_matches_snapshot() {
        let workspace = temp_workspace("parity-a2a");
        let root = journal_root("parity-a2a");
        let adapter = Arc::new(QueuedAdapter::new(vec![Ok(no_tool_result("Ack."))]));
        let session = make_session(&workspace, adapter);
        attach_initialized_journal(&session, &root);

        // Restore A2A state into the session; restore_state journals a baseline
        // (ConversationReplaced + RuntimeStateUpdated) before applying memory.
        let mut bus = crate::agent::a2a::bus::AgentA2ABus::default();
        let task_id = crate::agent::a2a::supervisor::assign_delegate_task(
            &mut bus,
            "Review parity",
            "Check A2A state round-trips through the journal",
            10,
        );
        bus.complete_task(&task_id, "looks good", 20);
        session.restore_state(
            vec![ChatMessage::user("plan review")],
            None,
            None,
            None,
            Some(bus),
        );

        // A subsequent turn flushes the runtime-state mutation again.
        let emitter: Arc<dyn EventEmitter> = Arc::new(CollectingEventEmitter::new());
        let guard = try_begin_turn(session.turn_inflight.clone()).expect("begin turn");
        session
            .run_agent_turn(AgentTurnRunRequest {
                text: "continue",
                hidden_contexts: vec![],
                turn_metadata: None,
                activation_text: None,
                _turn_guard: guard,
                emitter: &*emitter,
                tool_emitter: Some(emitter.clone()),
                app_handle: None,
            })
            .await
            .expect("turn ok");

        assert_journal_parity(&session, &root);

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&root);
    }
}
