//! Pure, deterministic projection of a session mutation journal into state.
//!
//! This module contains no filesystem I/O. It replays a slice of
//! `SessionMutationEnvelope` events into a `SessionProjection`, which can then
//! be converted to an `AgentSessionSnapshot` for parity checks against the
//! legacy snapshot store.

use crate::adapters::base::ChatMessage;
use crate::agent::session_journal::{
    SessionMutation, SessionMutationEnvelope, SessionRuntimeState,
};
use crate::agent::snapshot::AgentSessionSnapshot;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SessionProjection {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub working_dir: String,
    pub messages: Vec<ChatMessage>,
    pub summary: Option<String>,
    pub runtime: SessionRuntimeState,
    pub last_sequence: u64,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

impl SessionProjection {
    pub(crate) fn from_events(events: &[SessionMutationEnvelope]) -> Result<Self, String> {
        let mut projection: Option<SessionProjection> = None;
        for event in events {
            validate_next_sequence(projection.as_ref(), event)?;
            apply_event(&mut projection, event)?;
        }
        projection.ok_or_else(|| "session journal has no initialization event".to_string())
    }

    /// Convert this projection into the closest faithful `AgentSessionSnapshot`.
    ///
    /// Mapping decisions:
    /// - Identity fields (`session_id`, `provider`, `model`, `working_dir`) come
    ///   directly from the `SessionInitialized` event.
    /// - `messages` and `summary` are the latest values after replaying all
    ///   `MessageAppended` and `ConversationReplaced` events.
    /// - Runtime fields come from the latest `RuntimeStateUpdated` event, applied
    ///   wholesale.
    /// - `context_window_tokens` is not captured by the journal, so it is always
    ///   `None`.
    /// - `created_at_ms`/`updated_at_ms` are taken from the first and last event
    ///   timestamps respectively. These are expected to drift from snapshot
    ///   wall-clock times and are ignored by the parity comparator.
    pub(crate) fn to_snapshot(&self) -> AgentSessionSnapshot {
        let mut snapshot = AgentSessionSnapshot::new(
            self.session_id.clone(),
            self.provider.clone(),
            self.model.clone(),
            self.working_dir.clone(),
            self.messages.clone(),
            self.summary.clone(),
            None, // context_window_tokens is not captured by the journal
        );
        if let Some(turn) = &self.runtime.latest_turn {
            snapshot = snapshot.with_latest_turn(turn.clone());
        }
        if let Some(workflow) = &self.runtime.latest_workflow {
            snapshot = snapshot.with_latest_workflow(workflow.clone());
        }
        if let Some(delivery) = &self.runtime.latest_delivery {
            snapshot = snapshot.with_latest_delivery(delivery.clone());
        }
        if let Some(goal) = &self.runtime.goal_ledger {
            snapshot = snapshot.with_goal_ledger(goal.clone());
        }
        if let Some(a2a) = &self.runtime.a2a_state {
            snapshot = snapshot.with_a2a_state(a2a.clone());
        }
        snapshot = snapshot.with_pending_confirms(self.runtime.pending_confirms.clone());
        snapshot = snapshot.with_active_tool_calls(self.runtime.active_tool_calls.clone());
        snapshot.created_at_ms = self.created_at_ms;
        snapshot.updated_at_ms = self.updated_at_ms;
        snapshot
    }
}

fn validate_next_sequence(
    projection: Option<&SessionProjection>,
    event: &SessionMutationEnvelope,
) -> Result<(), String> {
    if let Some(projection) = projection {
        let expected = projection.last_sequence.saturating_add(1);
        if event.sequence != expected {
            return Err(format!(
                "sequence gap at event {}: expected {}, found {}",
                event.event_id, expected, event.sequence
            ));
        }
    }
    Ok(())
}

fn apply_event(
    projection: &mut Option<SessionProjection>,
    event: &SessionMutationEnvelope,
) -> Result<(), String> {
    match &event.mutation {
        SessionMutation::SessionInitialized {
            provider,
            model,
            working_dir,
        } => {
            if projection.is_some() {
                return Err("duplicate SessionInitialized event".to_string());
            }
            *projection = Some(SessionProjection {
                session_id: event.session_id.clone(),
                provider: provider.clone(),
                model: model.clone(),
                working_dir: working_dir.clone(),
                messages: Vec::new(),
                summary: None,
                runtime: SessionRuntimeState {
                    latest_turn: None,
                    latest_workflow: None,
                    latest_delivery: None,
                    goal_ledger: None,
                    a2a_state: None,
                    pending_confirms: Vec::new(),
                    active_tool_calls: Vec::new(),
                },
                last_sequence: event.sequence,
                created_at_ms: event.created_at_ms,
                updated_at_ms: event.created_at_ms,
            });
        }
        SessionMutation::MessageAppended { message } => {
            let projection = projection
                .as_mut()
                .ok_or("MessageAppended before SessionInitialized")?;
            projection.messages.push(message.clone());
            projection.last_sequence = event.sequence;
            projection.updated_at_ms = event.created_at_ms;
        }
        SessionMutation::ConversationReplaced {
            checkpoint_id: _,
            messages,
            summary,
        } => {
            let projection = projection
                .as_mut()
                .ok_or("ConversationReplaced before SessionInitialized")?;
            projection.messages = messages.clone();
            projection.summary = summary.clone();
            projection.last_sequence = event.sequence;
            projection.updated_at_ms = event.created_at_ms;
        }
        SessionMutation::RuntimeStateUpdated { state } => {
            let projection = projection
                .as_mut()
                .ok_or("RuntimeStateUpdated before SessionInitialized")?;
            projection.runtime = state.clone();
            projection.last_sequence = event.sequence;
            projection.updated_at_ms = event.created_at_ms;
        }
    }
    Ok(())
}

/// Compare two snapshots for parity, ignoring only `created_at_ms` and
/// `updated_at_ms` skew. The journal does not preserve wall-clock timestamps, so
/// the only normalized fields are those two. All other fields, including
/// `schema_version`, `messages`, runtime state, and descriptors, must match
/// exactly.
pub(crate) fn snapshots_parity_equivalent(
    left: &AgentSessionSnapshot,
    right: &AgentSessionSnapshot,
) -> bool {
    let mut left_value = serde_json::to_value(left).expect("left snapshot serializes");
    let mut right_value = serde_json::to_value(right).expect("right snapshot serializes");
    if let Some(map) = left_value.as_object_mut() {
        map.remove("created_at_ms");
        map.remove("updated_at_ms");
    }
    if let Some(map) = right_value.as_object_mut() {
        map.remove("created_at_ms");
        map.remove("updated_at_ms");
    }
    left_value == right_value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::snapshot::{
        ActiveToolCallDescriptor, ActiveToolCallStatus, PendingConfirmDescriptor,
    };

    fn initialized(sequence: u64) -> SessionMutationEnvelope {
        SessionMutationEnvelope {
            schema_version: 1,
            event_id: format!("init-{sequence}"),
            session_id: "session-1".to_string(),
            sequence,
            created_at_ms: sequence,
            mutation: SessionMutation::SessionInitialized {
                provider: "openai".to_string(),
                model: "gpt-5".to_string(),
                working_dir: "/workspace".to_string(),
            },
        }
    }

    fn appended(sequence: u64, message: ChatMessage) -> SessionMutationEnvelope {
        SessionMutationEnvelope {
            schema_version: 1,
            event_id: format!("append-{sequence}"),
            session_id: "session-1".to_string(),
            sequence,
            created_at_ms: sequence,
            mutation: SessionMutation::MessageAppended { message },
        }
    }

    fn replaced(
        sequence: u64,
        checkpoint_id: &str,
        messages: Vec<ChatMessage>,
        summary: Option<&str>,
    ) -> SessionMutationEnvelope {
        SessionMutationEnvelope {
            schema_version: 1,
            event_id: format!("replace-{sequence}"),
            session_id: "session-1".to_string(),
            sequence,
            created_at_ms: sequence,
            mutation: SessionMutation::ConversationReplaced {
                checkpoint_id: checkpoint_id.to_string(),
                messages,
                summary: summary.map(String::from),
            },
        }
    }

    fn runtime(sequence: u64, state: SessionRuntimeState) -> SessionMutationEnvelope {
        SessionMutationEnvelope {
            schema_version: 1,
            event_id: format!("runtime-{sequence}"),
            session_id: "session-1".to_string(),
            sequence,
            created_at_ms: sequence,
            mutation: SessionMutation::RuntimeStateUpdated { state },
        }
    }

    fn runtime_with_pending(question: &str) -> SessionRuntimeState {
        SessionRuntimeState {
            latest_turn: None,
            latest_workflow: None,
            latest_delivery: None,
            goal_ledger: None,
            a2a_state: None,
            pending_confirms: vec![PendingConfirmDescriptor::new(
                format!("confirm-{question}"),
                question.to_string(),
                "ask_user".to_string(),
                100,
            )],
            active_tool_calls: Vec::new(),
        }
    }

    fn message_text(message: &ChatMessage) -> Option<&str> {
        message.content.as_str()
    }

    #[test]
    fn initialized_session_builds_projection() {
        let events = vec![initialized(1)];

        let projection = SessionProjection::from_events(&events).unwrap();

        assert_eq!(projection.session_id, "session-1");
        assert_eq!(projection.provider, "openai");
        assert_eq!(projection.model, "gpt-5");
        assert_eq!(projection.working_dir, "/workspace");
        assert!(projection.messages.is_empty());
        assert!(projection.summary.is_none());
        assert_eq!(projection.last_sequence, 1);
    }

    #[test]
    fn append_order_preserves_message_sequence() {
        let events = vec![
            initialized(1),
            appended(2, ChatMessage::user("first")),
            appended(3, ChatMessage::user("second")),
        ];

        let projection = SessionProjection::from_events(&events).unwrap();

        assert_eq!(projection.messages.len(), 2);
        assert_eq!(message_text(&projection.messages[0]), Some("first"));
        assert_eq!(message_text(&projection.messages[1]), Some("second"));
        assert_eq!(projection.last_sequence, 3);
    }

    #[test]
    fn conversation_replacement_discards_pre_checkpoint_messages() {
        let events = vec![
            initialized(1),
            appended(2, ChatMessage::user("old")),
            replaced(
                3,
                "checkpoint-1",
                vec![ChatMessage::user("retained")],
                Some("summary"),
            ),
            appended(4, ChatMessage::assistant(serde_json::json!("new"))),
        ];

        let projection = SessionProjection::from_events(&events).unwrap();

        assert_eq!(projection.messages.len(), 2);
        assert_eq!(
            projection.messages[0].content,
            serde_json::json!("retained")
        );
        assert_eq!(projection.messages[1].content, serde_json::json!("new"));
        assert_eq!(projection.summary.as_deref(), Some("summary"));
        assert_eq!(projection.last_sequence, 4);
    }

    #[test]
    fn runtime_state_replacement_overwrites_prior_state() {
        let events = vec![
            initialized(1),
            runtime(2, runtime_with_pending("first")),
            runtime(3, runtime_with_pending("second")),
        ];

        let projection = SessionProjection::from_events(&events).unwrap();

        assert_eq!(projection.runtime.pending_confirms.len(), 1);
        assert_eq!(projection.runtime.pending_confirms[0].question, "second");
        assert_eq!(projection.last_sequence, 3);
    }

    #[test]
    fn missing_initialization_returns_error() {
        let events: Vec<SessionMutationEnvelope> = vec![];
        let result = SessionProjection::from_events(&events);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no initialization"));
    }

    #[test]
    fn events_without_initialization_return_error() {
        let events = vec![appended(1, ChatMessage::user("orphan"))];
        let result = SessionProjection::from_events(&events);
        assert!(result.is_err());
    }

    #[test]
    fn duplicate_initialization_returns_error() {
        let events = vec![initialized(1), initialized(2)];
        let result = SessionProjection::from_events(&events);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("duplicate"));
    }

    #[test]
    fn sequence_gap_returns_error() {
        let events = vec![
            initialized(1),
            appended(2, ChatMessage::user("ok")),
            appended(4, ChatMessage::user("gap")),
        ];
        let result = SessionProjection::from_events(&events);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("sequence gap"));
    }

    #[test]
    fn truncated_generation_replay_from_sequence_zero() {
        let events = vec![
            initialized(0),
            appended(1, ChatMessage::user("after-baseline")),
        ];

        let projection = SessionProjection::from_events(&events).unwrap();

        assert_eq!(projection.last_sequence, 1);
        assert_eq!(projection.messages.len(), 1);
    }

    #[test]
    fn to_snapshot_matches_equivalent_snapshot() {
        let pending = PendingConfirmDescriptor::new(
            "confirm-1".to_string(),
            "Allow write?".to_string(),
            "file_write".to_string(),
            42,
        );
        let tool_input = serde_json::json!({"path": "file.txt", "content": "hello"});
        let active = ActiveToolCallDescriptor::new(
            "tool-1".to_string(),
            "write_to_file".to_string(),
            tool_input.clone(),
            200,
        )
        .with_status(ActiveToolCallStatus::AwaitingResult);
        let runtime_state = SessionRuntimeState {
            latest_turn: None,
            latest_workflow: None,
            latest_delivery: None,
            goal_ledger: None,
            a2a_state: None,
            pending_confirms: vec![pending.clone()],
            active_tool_calls: vec![active.clone()],
        };
        let events = vec![
            initialized(1),
            appended(2, ChatMessage::user("hello")),
            runtime(3, runtime_state),
        ];

        let projection = SessionProjection::from_events(&events).unwrap();
        let from_projection = projection.to_snapshot();

        let mut expected = AgentSessionSnapshot::new(
            "session-1".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "/workspace".to_string(),
            vec![ChatMessage::user("hello")],
            None,
            None,
        )
        .with_pending_confirms(vec![pending])
        .with_active_tool_calls(vec![active]);
        expected.created_at_ms = projection.created_at_ms;
        expected.updated_at_ms = projection.updated_at_ms;

        assert!(
            snapshots_parity_equivalent(&from_projection, &expected),
            "projection snapshot should match equivalent snapshot except timestamps"
        );
    }

    #[test]
    fn parity_comparator_detects_real_differences() {
        let base = AgentSessionSnapshot::new(
            "session-1".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "/workspace".to_string(),
            vec![ChatMessage::user("hello")],
            None,
            None,
        );
        let different_messages = AgentSessionSnapshot::new(
            "session-1".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "/workspace".to_string(),
            vec![ChatMessage::user("goodbye")],
            None,
            None,
        );
        let different_provider = AgentSessionSnapshot::new(
            "session-1".to_string(),
            "anthropic".to_string(),
            "gpt-5".to_string(),
            "/workspace".to_string(),
            vec![ChatMessage::user("hello")],
            None,
            None,
        );

        assert!(snapshots_parity_equivalent(&base, &base));
        assert!(!snapshots_parity_equivalent(&base, &different_messages));
        assert!(!snapshots_parity_equivalent(&base, &different_provider));
    }
}
