#[cfg(test)]
mod tests {
    use crate::adapters::base::ToolCall;
    use crate::agent::a2a::bus::AgentA2ABus;
    use crate::agent::a2a::types::{
        AgentExecutionMode, AgentParentSessionContext, AgentRole, AgentTaskId,
    };
    use crate::agent::session::tools::{
        canonical_json, delegate_parent_task_id_from_input, tool_batch_signature,
        tool_category_signature,
    };

    fn tc(name: &str, input: serde_json::Value) -> ToolCall {
        ToolCall {
            id: format!("id-{name}"),
            name: name.to_string(),
            input,
        }
    }

    #[test]
    fn canonical_json_sorts_object_keys() {
        let value = serde_json::json!({"b": 2, "a": 1});
        assert_eq!(canonical_json(&value), "{\"a\":1,\"b\":2}");
    }

    #[test]
    fn canonical_json_sorts_nested_keys() {
        let value = serde_json::json!({"z": {"b": 2, "a": 1}});
        assert_eq!(canonical_json(&value), "{\"z\":{\"a\":1,\"b\":2}}");
    }

    #[test]
    fn canonical_json_handles_arrays_and_scalars() {
        let value = serde_json::json!([{"c": 3}, {"a": 1, "b": 2}, true]);
        assert_eq!(canonical_json(&value), "[{\"c\":3},{\"a\":1,\"b\":2},true]");
    }

    #[test]
    fn tool_batch_signature_is_deterministic() {
        let calls = vec![
            tc("read_file", serde_json::json!({"path": "b.rs"})),
            tc("read_file", serde_json::json!({"path": "a.rs"})),
        ];
        let sig = tool_batch_signature(&calls);
        assert!(sig.contains("read_file"));
        assert!(sig.starts_with("read_file:"));
        // Order should be sorted by the canonical JSON representation.
        let lines: Vec<&str> = sig.lines().collect();
        assert!(lines[0] < lines[1]);
    }

    #[test]
    fn tool_category_signature_deduplicates_names() {
        let calls = vec![
            tc("read_file", serde_json::json!({"path": "a.rs"})),
            tc("read_file", serde_json::json!({"path": "b.rs"})),
            tc("run_shell", serde_json::json!({"command": "echo hi"})),
        ];
        assert_eq!(tool_category_signature(&calls), "read_file,run_shell");
    }

    #[test]
    fn tool_category_signature_empty_for_no_calls() {
        assert_eq!(tool_category_signature(&[]), "");
    }

    #[test]
    fn delegate_parent_task_id_from_input_accepts_existing_parent() {
        let mut bus = AgentA2ABus::default();
        let parent_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Parent task",
            "Plan child work",
            10,
        );
        let input = serde_json::json!({
            "task": "Run child task",
            "parent_task_id": parent_id.as_str(),
        });

        let resolved =
            delegate_parent_task_id_from_input(&input, &bus, "session-1").expect("resolve parent");

        assert_eq!(resolved.as_ref(), Some(&parent_id));
    }

    #[test]
    fn delegate_parent_task_id_from_input_explicit_parent_wins_over_root_planning_flag() {
        let mut bus = AgentA2ABus::default();
        let explicit_parent_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Explicit parent task",
            "Plan explicit child work",
            10,
        );
        let context_parent_id = bus.assign_task(
            AgentRole::Reviewer,
            AgentExecutionMode::ReadOnly,
            "Context parent task",
            "Plan context child work",
            20,
        );
        bus.set_parent_session_context(AgentParentSessionContext {
            parent_session_id: "session-1".to_string(),
            active_parent_task_id: context_parent_id,
            root_task_id: AgentTaskId::new("root-task"),
            selection_reason: "active parent selected".to_string(),
            updated_at_ms: 30,
        });
        let input = serde_json::json!({
            "task": "Run explicit child task",
            "parent_task_id": explicit_parent_id.as_str(),
            "root_planning_task": true,
        });

        let resolved =
            delegate_parent_task_id_from_input(&input, &bus, "session-1").expect("resolve parent");

        assert_eq!(resolved.as_ref(), Some(&explicit_parent_id));
    }

    #[test]
    fn delegate_parent_task_id_from_input_rejects_missing_parent() {
        let bus = AgentA2ABus::default();
        let input = serde_json::json!({
            "task": "Run child task",
            "parent_task_id": "missing-parent",
        });

        assert!(delegate_parent_task_id_from_input(&input, &bus, "session-1").is_err());
    }

    #[test]
    fn delegate_parent_task_id_from_input_ignores_empty_parent() {
        let bus = AgentA2ABus::default();
        let input = serde_json::json!({
            "task": "Run child task",
            "parent_task_id": "  ",
        });

        assert_eq!(
            delegate_parent_task_id_from_input(&input, &bus, "session-1")
                .expect("empty parent is absent"),
            None
        );
    }

    #[test]
    fn delegate_parent_task_id_from_input_uses_matching_parent_session_context() {
        let mut bus = AgentA2ABus::default();
        let parent_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Parent task",
            "Plan child work",
            10,
        );
        bus.set_parent_session_context(AgentParentSessionContext {
            parent_session_id: "session-1".to_string(),
            active_parent_task_id: parent_id.clone(),
            root_task_id: parent_id.clone(),
            selection_reason: "active parent selected".to_string(),
            updated_at_ms: 20,
        });
        let input = serde_json::json!({
            "task": "Run child task",
        });

        let resolved =
            delegate_parent_task_id_from_input(&input, &bus, "session-1").expect("resolve parent");

        assert_eq!(resolved.as_ref(), Some(&parent_id));
    }

    #[test]
    fn delegate_parent_task_id_from_input_ignores_other_session_context() {
        let mut bus = AgentA2ABus::default();
        let parent_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Parent task",
            "Plan child work",
            10,
        );
        bus.set_parent_session_context(AgentParentSessionContext {
            parent_session_id: "other-session".to_string(),
            active_parent_task_id: parent_id,
            root_task_id: AgentTaskId::new("root-task"),
            selection_reason: "active parent selected".to_string(),
            updated_at_ms: 20,
        });
        let input = serde_json::json!({
            "task": "Run child task",
        });

        let resolved =
            delegate_parent_task_id_from_input(&input, &bus, "session-1").expect("resolve parent");

        assert_eq!(resolved, None);
    }

    #[test]
    fn delegate_parent_task_id_from_input_ignores_missing_active_parent_context() {
        let mut bus = AgentA2ABus::default();
        bus.set_parent_session_context(AgentParentSessionContext {
            parent_session_id: "session-1".to_string(),
            active_parent_task_id: AgentTaskId::new("missing-parent"),
            root_task_id: AgentTaskId::new("missing-root"),
            selection_reason: "active parent selected".to_string(),
            updated_at_ms: 20,
        });
        let input = serde_json::json!({
            "task": "Run child task",
        });

        let resolved =
            delegate_parent_task_id_from_input(&input, &bus, "session-1").expect("resolve parent");

        assert_eq!(resolved, None);
    }

    #[test]
    fn delegate_parent_task_id_from_input_ignores_context_for_root_planning_task() {
        let mut bus = AgentA2ABus::default();
        let parent_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Parent task",
            "Plan child work",
            10,
        );
        bus.set_parent_session_context(AgentParentSessionContext {
            parent_session_id: "session-1".to_string(),
            active_parent_task_id: parent_id,
            root_task_id: AgentTaskId::new("root-task"),
            selection_reason: "active parent selected".to_string(),
            updated_at_ms: 20,
        });
        let input = serde_json::json!({
            "task": "Plan a root task",
            "root_planning_task": true,
        });

        let resolved =
            delegate_parent_task_id_from_input(&input, &bus, "session-1").expect("resolve parent");

        assert_eq!(resolved, None);
    }

    // ── Journal shadow-mode tool-result mutation ──────────────────────────

    #[tokio::test]
    async fn ordered_tool_result_batch_is_one_journal_mutation() {
        use crate::adapters::base::{AdapterError, AiAdapter, ChatMessage, StreamResult};
        use crate::agent::event_sink::{CollectingEventEmitter, EventEmitter};
        use crate::agent::session::{AgentSession, AgentTurnRunRequest};
        use crate::agent::session_guards::try_begin_turn;
        use crate::agent::session_journal::{SessionJournalStore, SessionMutation};
        use crate::agent::session_mutation::{SessionJournalHandle, SessionJournalMode};
        use crate::harness::Harness;
        use async_trait::async_trait;
        use std::collections::VecDeque;
        use std::sync::{Arc, Mutex};

        struct BatchAdapter {
            queue: Mutex<VecDeque<Result<StreamResult, AdapterError>>>,
        }

        #[async_trait]
        impl AiAdapter for BatchAdapter {
            async fn call(
                &self,
                _messages: &[ChatMessage],
                _cancel: Arc<tokio::sync::Notify>,
            ) -> Result<StreamResult, AdapterError> {
                self.queue.lock().unwrap().pop_front().unwrap_or_else(|| {
                    Ok(StreamResult {
                        assistant_content: vec![serde_json::json!("done")],
                        tool_calls: vec![],
                        stop_reason: Some("stop".to_string()),
                    })
                })
            }

            fn model_id(&self) -> &str {
                "batch-model"
            }

            fn model_name(&self) -> &str {
                "Batch Model"
            }
        }

        let workspace = std::env::temp_dir().join(format!(
            "forge-tools-journal-workspace-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::write(workspace.join("a.txt"), "alpha").expect("write a");
        std::fs::write(workspace.join("b.txt"), "beta").expect("write b");
        let journal_root =
            std::env::temp_dir().join(format!("forge-tools-journal-root-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&journal_root).expect("journal root");

        let adapter = Arc::new(BatchAdapter {
            queue: Mutex::new(
                vec![
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
                    Ok(StreamResult {
                        assistant_content: vec![serde_json::json!("both read")],
                        tool_calls: vec![],
                        stop_reason: Some("stop".to_string()),
                    }),
                ]
                .into_iter()
                .collect(),
            ),
        });

        let harness = Arc::new(Harness::new(workspace.clone()));
        let session = AgentSession::new(
            "session-1".to_string(),
            "claude".to_string(),
            adapter,
            harness,
            "system".to_string(),
            None,
        );
        let store = SessionJournalStore::new(journal_root.clone(), session.id.clone())
            .expect("journal store");
        session
            .attach_session_journal(SessionJournalHandle::new(store, SessionJournalMode::Shadow));

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

        let mutations: Vec<SessionMutation> =
            SessionJournalStore::new(journal_root.clone(), session.id.clone())
                .expect("loader store")
                .load()
                .expect("journal load")
                .events
                .into_iter()
                .map(|event| event.mutation)
                .collect();
        let tool_result_appends: Vec<&ChatMessage> = mutations
            .iter()
            .filter_map(|mutation| match mutation {
                SessionMutation::MessageAppended { message } => Some(message),
                _ => None,
            })
            .filter(|message| {
                message.content.as_array().is_some_and(|blocks| {
                    blocks.iter().any(|block| {
                        block.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                    })
                })
            })
            .collect();

        assert_eq!(
            tool_result_appends.len(),
            1,
            "the ordered tool-result batch must be exactly one journal mutation: {mutations:?}"
        );
        let blocks = tool_result_appends[0]
            .content
            .as_array()
            .expect("tool result blocks");
        let ids: Vec<&str> = blocks
            .iter()
            .filter_map(|block| block.get("tool_use_id").and_then(|v| v.as_str()))
            .collect();
        assert_eq!(ids, vec!["tu_a", "tu_b"], "order must be preserved");

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&journal_root);
    }
}
