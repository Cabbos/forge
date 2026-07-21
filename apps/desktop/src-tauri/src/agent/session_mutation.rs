//! Centralized session mutation helpers with shadow-mode journal integration.
//!
//! All durable conversation mutations (`messages`, `summary`) and the covered
//! runtime-state transitions flow through the helpers in this module. Each
//! helper journals the mutation first and applies the in-memory write second,
//! so the journal is always at least as complete as the in-memory state.
//!
//! # Shadow vs strict mode
//!
//! The journal runs in shadow mode by default: append failures are logged via
//! `app_log!` diagnostics and the in-memory mutation still applies, preserving
//! pre-journal behavior. Setting `FORGE_SESSION_JOURNAL_STRICT=1` (captured
//! when the handle is constructed) makes an append failure fatal to the
//! corresponding mutation: the helper returns `Err` and the in-memory write is
//! skipped. Strict mode exists for tests and diagnostics only.
//!
//! Journal events are NEVER streamed to the UI; `StreamEvent` remains the only
//! backend-to-frontend transport.
//!
//! # `RuntimeStateUpdated` append policy
//!
//! Journaled transitions (exhaustive for the states `AgentSession` owns
//! directly):
//!
//! - goal ledger set/cleared or task status synced (`set_goal_ledger`,
//!   `normalize_goal_ledger_for_resume`, `sync_goal_task_for_a2a`);
//! - A2A task added or status-changed inside `execute_tools` (delegated
//!   sub-agent lifecycle);
//! - latest-turn pointer advance at round completion (one mark per round in
//!   `run_agent_turn`, plus the final flush after `finalize_turn`).
//!
//! Coalescing: marks only set a dirty flag. `flush_session_runtime_state`
//! emits at most one `RuntimeStateUpdated` per flush, carrying the FULL
//! runtime-state payload (partial payloads would erase fields on replay). The
//! flush points are the event-loop "ticks": round completion and turn
//! finalization. Multiple mutations to the same field within one tick collapse
//! into the single flushed append with the final value.
//!
//! Never journaled (ephemeral): provider stream state, live senders,
//! cancellation handles, in-flight sampling progress, and UI-only flags.
//!
//! Deferred (owned outside `AgentSession`, not covered by this module yet):
//! `latest_workflow` and `latest_delivery` (owned by `AppState`), A2A review
//! decisions applied in `ipc/a2a_handlers.rs`, and the
//! `pending_confirms`/`active_tool_calls` descriptors (owned by the IPC
//! confirm/executor layers). These fields are ALWAYS empty in journaled
//! runtime state; the Task 5 parity comparator must exclude them from
//! comparison until their write sites are wired through a helper. Likewise
//! `repair_message_history` may drop dangling tool_use messages outside the
//! journaled helpers; it only fires on already-corrupt history and is a known
//! parity gap to close when the journal becomes authoritative.

use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::adapters::base::ChatMessage;
use crate::agent::a2a::bus::AgentA2ABus;
use crate::agent::goal_state::GoalLedger;
use crate::agent::session::AgentSession;
use crate::agent::session_guards::lock_unpoisoned;
use crate::agent::session_journal::{
    SessionJournalStore, SessionMutation, SessionMutationEnvelope, SessionRuntimeState,
    SESSION_JOURNAL_SCHEMA_VERSION,
};
use crate::agent::time::now_ms;
use crate::agent::turn_state::AgentTurnState;

/// Origin of a journaled mutation. Diagnostics metadata only — the on-disk
/// journal schema does not carry the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionMutationSource {
    UserInput,
    AssistantResponse,
    AutoContinuation,
    FinalSummary,
    ToolResults,
    Compaction,
    SnapshotRestoreBaseline,
    RoundCompletion,
}

impl SessionMutationSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SessionMutationSource::UserInput => "user_input",
            SessionMutationSource::AssistantResponse => "assistant_response",
            SessionMutationSource::AutoContinuation => "auto_continuation",
            SessionMutationSource::FinalSummary => "final_summary",
            SessionMutationSource::ToolResults => "tool_results",
            SessionMutationSource::Compaction => "compaction",
            SessionMutationSource::SnapshotRestoreBaseline => "snapshot_restore_baseline",
            SessionMutationSource::RoundCompletion => "round_completion",
        }
    }
}

/// Whether a journal append failure blocks the in-memory mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionJournalMode {
    /// Log diagnostics and preserve current behavior (default).
    Shadow,
    /// Fail the mutation; used by tests and `FORGE_SESSION_JOURNAL_STRICT=1`.
    Strict,
}

impl SessionJournalMode {
    fn as_str(&self) -> &'static str {
        match self {
            SessionJournalMode::Shadow => "shadow",
            SessionJournalMode::Strict => "strict",
        }
    }
}

pub(crate) fn session_journal_strict_from_env() -> bool {
    std::env::var("FORGE_SESSION_JOURNAL_STRICT")
        .map(|value| value.trim() == "1")
        .unwrap_or(false)
}

/// Root directory for session journals. Matches `snapshot.rs`'s resolution of
/// `~/.forge`; `FORGE_SESSION_JOURNAL_ROOT` overrides it for tests and
/// diagnostics.
pub(crate) fn session_journal_root() -> PathBuf {
    if let Ok(root) = std::env::var("FORGE_SESSION_JOURNAL_ROOT") {
        let trimmed = root.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    home_dir().join(".forge")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Dirty flags implementing the runtime-state coalescing policy: mutations
/// only mark fields; the flush emits a single full-payload append.
#[derive(Debug, Default)]
struct RuntimeStateScratch {
    goal_ledger: bool,
    a2a_state: bool,
    latest_turn: bool,
}

impl RuntimeStateScratch {
    fn any(&self) -> bool {
        self.goal_ledger || self.a2a_state || self.latest_turn
    }

    fn clear(&mut self) {
        self.goal_ledger = false;
        self.a2a_state = false;
        self.latest_turn = false;
    }
}

/// Shadow-mode wrapper around `SessionJournalStore`. Owns the strict/shadow
/// failure semantics and the runtime-state coalescing scratch.
pub(crate) struct SessionJournalHandle {
    store: SessionJournalStore,
    mode: SessionJournalMode,
    runtime_scratch: Mutex<RuntimeStateScratch>,
}

impl SessionJournalHandle {
    pub(crate) fn new(store: SessionJournalStore, mode: SessionJournalMode) -> Self {
        Self {
            store,
            mode,
            runtime_scratch: Mutex::new(RuntimeStateScratch::default()),
        }
    }

    pub(crate) fn from_env(store: SessionJournalStore) -> Self {
        let mode = if session_journal_strict_from_env() {
            SessionJournalMode::Strict
        } else {
            SessionJournalMode::Shadow
        };
        Self::new(store, mode)
    }

    pub(crate) fn store(&self) -> &SessionJournalStore {
        &self.store
    }

    fn append_envelope(
        &self,
        session_id: &str,
        mutation: SessionMutation,
        source: SessionMutationSource,
    ) -> Result<(), String> {
        let envelope = SessionMutationEnvelope {
            schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
            event_id: String::new(),
            session_id: session_id.to_string(),
            sequence: 0,
            created_at_ms: now_ms(),
            mutation,
        };
        match self.store.append(envelope) {
            Ok(()) => Ok(()),
            Err(error) => {
                crate::app_log!(
                    "WARN",
                    "[session_journal] append failed (mode={}, source={}, session_id={}): {:?}",
                    self.mode.as_str(),
                    source.as_str(),
                    session_id,
                    error
                );
                if self.mode == SessionJournalMode::Strict {
                    Err(format!(
                        "session journal append failed (source={}): {:?}",
                        source.as_str(),
                        error
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    fn append_initialized(
        &self,
        session_id: &str,
        provider: &str,
        model: &str,
        working_dir: &str,
    ) -> Result<(), String> {
        self.append_envelope(
            session_id,
            SessionMutation::SessionInitialized {
                provider: provider.to_string(),
                model: model.to_string(),
                working_dir: working_dir.to_string(),
            },
            SessionMutationSource::SnapshotRestoreBaseline,
        )
    }

    fn append_message(
        &self,
        session_id: &str,
        message: &ChatMessage,
        source: SessionMutationSource,
    ) -> Result<(), String> {
        self.append_envelope(
            session_id,
            SessionMutation::MessageAppended {
                message: message.clone(),
            },
            source,
        )
    }

    fn append_replacement(
        &self,
        session_id: &str,
        checkpoint_id: String,
        messages: &[ChatMessage],
        summary: &Option<String>,
        source: SessionMutationSource,
    ) -> Result<(), String> {
        self.append_envelope(
            session_id,
            SessionMutation::ConversationReplaced {
                checkpoint_id,
                messages: messages.to_vec(),
                summary: summary.clone(),
            },
            source,
        )
    }

    fn append_runtime_state(
        &self,
        session_id: &str,
        state: &SessionRuntimeState,
        source: SessionMutationSource,
    ) -> Result<(), String> {
        self.append_envelope(
            session_id,
            SessionMutation::RuntimeStateUpdated {
                state: state.clone(),
            },
            source,
        )
    }

    fn mark_goal_ledger_dirty(&self) {
        self.runtime_scratch.lock().goal_ledger = true;
    }

    fn mark_a2a_state_dirty(&self) {
        self.runtime_scratch.lock().a2a_state = true;
    }

    fn mark_latest_turn_dirty(&self) {
        self.runtime_scratch.lock().latest_turn = true;
    }

    fn runtime_state_dirty(&self) -> bool {
        self.runtime_scratch.lock().any()
    }

    /// Emit one coalesced `RuntimeStateUpdated` with the full payload when any
    /// covered field was marked dirty since the last successful flush.
    fn flush_runtime_state(
        &self,
        session_id: &str,
        state: &SessionRuntimeState,
        source: SessionMutationSource,
    ) -> Result<(), String> {
        let mut scratch = self.runtime_scratch.lock();
        if !scratch.any() {
            return Ok(());
        }
        let result = self.append_runtime_state(session_id, state, source);
        if result.is_ok() {
            scratch.clear();
        }
        result
    }

    /// Journal the baseline for a snapshot-restored session: one
    /// `ConversationReplaced` plus one full `RuntimeStateUpdated`, followed by
    /// a diagnostics record of the imported snapshot schema and sequence.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_restore_baseline(
        &self,
        session_id: &str,
        messages: &[ChatMessage],
        summary: &Option<String>,
        latest_turn: &Option<AgentTurnState>,
        goal_ledger: &Option<GoalLedger>,
        a2a_state: &Option<AgentA2ABus>,
        provenance: Option<SessionRestoreProvenance>,
    ) -> Result<(), String> {
        let checkpoint_id = format!("snapshot-restore-{}", uuid::Uuid::now_v7());
        self.append_replacement(
            session_id,
            checkpoint_id.clone(),
            messages,
            summary,
            SessionMutationSource::SnapshotRestoreBaseline,
        )?;
        let state = SessionRuntimeState {
            latest_turn: latest_turn.clone(),
            latest_workflow: None,
            latest_delivery: None,
            goal_ledger: goal_ledger.clone(),
            a2a_state: non_empty_a2a_state(a2a_state.clone()),
            pending_confirms: Vec::new(),
            active_tool_calls: Vec::new(),
        };
        self.append_runtime_state(
            session_id,
            &state,
            SessionMutationSource::SnapshotRestoreBaseline,
        )?;
        crate::app_log!(
            "INFO",
            "[session_journal] restore baseline recorded: session_id={}, checkpoint_id={}, imported_snapshot_schema={}, journal_sequence={}",
            session_id,
            checkpoint_id,
            provenance
                .map(|meta| meta.snapshot_schema_version.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            self.store.last_sequence()
        );
        Ok(())
    }
}

/// Provenance of a snapshot-imported restore, recorded in diagnostics metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SessionRestoreProvenance {
    pub snapshot_schema_version: u32,
}

fn non_empty_a2a_state(bus: Option<AgentA2ABus>) -> Option<AgentA2ABus> {
    bus.filter(|bus| !bus.tasks.is_empty() || !bus.messages.is_empty())
}

impl AgentSession {
    /// Attach a journal handle. Sessions constructed in tests start without a
    /// journal; production sessions get one from `initialize_session_journal`
    /// in the session builder.
    pub(crate) fn attach_session_journal(&self, handle: SessionJournalHandle) {
        *lock_unpoisoned(&self.session_journal) = Some(Arc::new(handle));
    }

    pub(crate) fn session_journal_handle(&self) -> Option<Arc<SessionJournalHandle>> {
        lock_unpoisoned(&self.session_journal).clone()
    }

    #[cfg(test)]
    pub(crate) fn session_journal_path(&self) -> Option<PathBuf> {
        self.session_journal_handle()
            .map(|handle| handle.store().path())
    }

    /// Construct and attach a journal at the default root (`~/.forge`, or
    /// `FORGE_SESSION_JOURNAL_ROOT`). Best-effort: failures log diagnostics and
    /// leave the session without a journal rather than failing session
    /// creation. Appends one `SessionInitialized` event when the journal is
    /// empty; an existing journal (same session id, earlier run) is continued.
    pub(crate) fn initialize_session_journal(&self) {
        let root = session_journal_root();
        let store = match SessionJournalStore::new(root, self.id.clone()) {
            Ok(store) => store,
            Err(error) => {
                crate::app_log!(
                    "WARN",
                    "[session_journal] journal disabled for session {}: {:?}",
                    self.id,
                    error
                );
                return;
            }
        };
        let handle = SessionJournalHandle::from_env(store);
        match handle.store().load() {
            Ok(result) if result.events.is_empty() => {
                let working_dir = self.harness.working_dir.to_string_lossy().to_string();
                if let Err(error) = handle.append_initialized(
                    &self.id,
                    &self.agent_type,
                    &self.model_id,
                    &working_dir,
                ) {
                    crate::app_log!(
                        "WARN",
                        "[session_journal] failed to record SessionInitialized for {}: {}",
                        self.id,
                        error
                    );
                }
            }
            Ok(result) => {
                crate::app_log!(
                    "INFO",
                    "[session_journal] continuing existing journal for session {}: {} events, last_sequence={}",
                    self.id,
                    result.events.len(),
                    result.events.last().map(|event| event.sequence).unwrap_or(0)
                );
            }
            Err(error) => {
                crate::app_log!(
                    "WARN",
                    "[session_journal] journal for session {} failed to load; appends will retry: {:?}",
                    self.id,
                    error
                );
            }
        }
        self.attach_session_journal(handle);
    }

    /// Journal-then-apply a single conversation append. In strict mode a
    /// journal failure skips the in-memory push and returns `Err`.
    pub(crate) fn append_conversation_message(
        &self,
        message: ChatMessage,
        source: SessionMutationSource,
    ) -> Result<(), String> {
        if let Some(journal) = self.session_journal_handle() {
            journal.append_message(&self.id, &message, source)?;
        }
        lock_unpoisoned(&self.messages).push(message);
        Ok(())
    }

    /// Journal-then-apply an ordered batch of conversation appends (e.g. the
    /// final-summary assistant message plus its synthetic tool results). Each
    /// message is journaled in order; the in-memory pushes share one lock so
    /// readers never observe a partially applied batch.
    pub(crate) fn append_conversation_messages(
        &self,
        messages: Vec<ChatMessage>,
        source: SessionMutationSource,
    ) -> Result<(), String> {
        if let Some(journal) = self.session_journal_handle() {
            for message in &messages {
                journal.append_message(&self.id, message, source)?;
            }
        }
        let mut guard = lock_unpoisoned(&self.messages);
        guard.extend(messages);
        Ok(())
    }

    /// Journal-then-apply a wholesale conversation replacement (compaction).
    /// Always one `ConversationReplaced` event — never a series of deletes.
    pub(crate) fn replace_conversation(
        &self,
        checkpoint_id: String,
        messages: Vec<ChatMessage>,
        summary: Option<String>,
        source: SessionMutationSource,
    ) -> Result<(), String> {
        if let Some(journal) = self.session_journal_handle() {
            journal.append_replacement(&self.id, checkpoint_id, &messages, &summary, source)?;
        }
        *lock_unpoisoned(&self.messages) = messages;
        *lock_unpoisoned(&self.summary) = summary;
        Ok(())
    }

    pub(crate) fn mark_goal_state_dirty(&self) {
        if let Some(journal) = self.session_journal_handle() {
            journal.mark_goal_ledger_dirty();
        }
    }

    pub(crate) fn mark_a2a_state_dirty(&self) {
        if let Some(journal) = self.session_journal_handle() {
            journal.mark_a2a_state_dirty();
        }
    }

    pub(crate) fn mark_latest_turn_dirty(&self) {
        if let Some(journal) = self.session_journal_handle() {
            journal.mark_latest_turn_dirty();
        }
    }

    /// Flush coalesced runtime-state marks as one full-payload
    /// `RuntimeStateUpdated`. Called at round completion and turn finalization
    /// (the event-loop ticks of the append policy).
    pub(crate) fn flush_session_runtime_state(
        &self,
        source: SessionMutationSource,
    ) -> Result<(), String> {
        let Some(journal) = self.session_journal_handle() else {
            return Ok(());
        };
        if !journal.runtime_state_dirty() {
            return Ok(());
        }
        let state = self.current_runtime_state();
        journal.flush_runtime_state(&self.id, &state, source)
    }

    /// Full committed runtime state owned by this session. Fields owned by the
    /// IPC/AppState layers (workflow, delivery, pending confirms, active tool
    /// calls) are intentionally empty — see module docs for the deferral.
    fn current_runtime_state(&self) -> SessionRuntimeState {
        let a2a_state = lock_unpoisoned(&self.a2a_bus).clone();
        SessionRuntimeState {
            latest_turn: lock_unpoisoned(&self.latest_turn).clone(),
            latest_workflow: None,
            latest_delivery: None,
            goal_ledger: lock_unpoisoned(&self.goal_ledger).clone(),
            a2a_state: non_empty_a2a_state(Some(a2a_state)),
            pending_confirms: Vec::new(),
            active_tool_calls: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::base::AiAdapter;
    use crate::agent::goal_state::GoalLedger;
    use crate::harness::Harness;
    use async_trait::async_trait;
    use std::sync::Arc;

    struct StubAdapter;

    #[async_trait]
    impl AiAdapter for StubAdapter {
        async fn call(
            &self,
            _messages: &[ChatMessage],
            _cancel: Arc<tokio::sync::Notify>,
        ) -> Result<crate::adapters::base::StreamResult, crate::adapters::base::AdapterError>
        {
            Ok(crate::adapters::base::StreamResult {
                assistant_content: vec![serde_json::json!("ok")],
                tool_calls: vec![],
                stop_reason: Some("stop".to_string()),
            })
        }

        fn model_id(&self) -> &str {
            "stub-model"
        }

        fn model_name(&self) -> &str {
            "Stub Model"
        }
    }

    fn test_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "forge-session-mutation-test-{label}-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&root).expect("root");
        root
    }

    fn test_session(label: &str) -> (AgentSession, PathBuf) {
        let workspace = test_root(&format!("{label}-workspace"));
        let harness = Arc::new(Harness::new(workspace));
        let session = AgentSession::new(
            "session-1".to_string(),
            "claude".to_string(),
            Arc::new(StubAdapter),
            harness,
            "system".to_string(),
            None,
        );
        let journal_root = test_root(label);
        (session, journal_root)
    }

    fn attach(session: &AgentSession, root: &std::path::Path, mode: SessionJournalMode) {
        let store = SessionJournalStore::new(root.to_path_buf(), session.id.clone())
            .expect("journal store");
        session.attach_session_journal(SessionJournalHandle::new(store, mode));
    }

    fn load_events(root: &std::path::Path, session_id: &str) -> Vec<SessionMutationEnvelope> {
        SessionJournalStore::new(root.to_path_buf(), session_id.to_string())
            .expect("loader store")
            .load()
            .expect("load")
            .events
    }

    /// A store whose root is a regular file: every append fails with an IO
    /// error, exercising shadow/strict failure semantics deterministically.
    fn broken_store(root: &std::path::Path, session_id: &str) -> SessionJournalStore {
        let file_root = root.join("not-a-directory");
        std::fs::write(&file_root, b"file, not a directory").expect("write blocker file");
        SessionJournalStore::new(file_root, session_id.to_string()).expect("broken store")
    }

    #[test]
    fn shadow_mode_preserves_mutation_when_journal_append_fails() {
        let (session, root) = test_session("shadow-failure");
        let store = broken_store(&root, &session.id);
        session
            .attach_session_journal(SessionJournalHandle::new(store, SessionJournalMode::Shadow));

        let result = session.append_conversation_message(
            ChatMessage::user("hello"),
            SessionMutationSource::UserInput,
        );

        assert!(result.is_ok(), "shadow mode must not fail the mutation");
        let messages = lock_unpoisoned(&session.messages).clone();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "hello");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn strict_mode_blocks_mutation_when_journal_append_fails() {
        let (session, root) = test_session("strict-failure");
        let store = broken_store(&root, &session.id);
        session
            .attach_session_journal(SessionJournalHandle::new(store, SessionJournalMode::Strict));

        let result = session.append_conversation_message(
            ChatMessage::user("hello"),
            SessionMutationSource::UserInput,
        );

        assert!(result.is_err(), "strict mode must fail the mutation");
        assert!(
            lock_unpoisoned(&session.messages).is_empty(),
            "strict mode must skip the in-memory push"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn strict_mode_blocks_replacement_when_journal_append_fails() {
        let (session, root) = test_session("strict-replacement");
        session
            .append_conversation_message(
                ChatMessage::user("original"),
                SessionMutationSource::UserInput,
            )
            .expect("no journal attached yet");
        let store = broken_store(&root, &session.id);
        session
            .attach_session_journal(SessionJournalHandle::new(store, SessionJournalMode::Strict));

        let result = session.replace_conversation(
            "checkpoint-1".to_string(),
            vec![ChatMessage::user("compacted")],
            Some("summary".to_string()),
            SessionMutationSource::Compaction,
        );

        assert!(result.is_err());
        let messages = lock_unpoisoned(&session.messages).clone();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "original");
        assert!(lock_unpoisoned(&session.summary).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn helper_without_journal_mutates_memory_and_succeeds() {
        let (session, root) = test_session("no-journal");
        session
            .append_conversation_message(ChatMessage::user("hi"), SessionMutationSource::UserInput)
            .expect("no journal is a no-op success");
        session
            .replace_conversation(
                "checkpoint-x".to_string(),
                vec![ChatMessage::user("new")],
                Some("s".to_string()),
                SessionMutationSource::Compaction,
            )
            .expect("no journal is a no-op success");
        let messages = lock_unpoisoned(&session.messages).clone();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "new");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn append_batch_journals_each_message_in_order() {
        let (session, root) = test_session("batch-order");
        attach(&session, &root, SessionJournalMode::Shadow);

        session
            .append_conversation_messages(
                vec![
                    ChatMessage::assistant(serde_json::json!(["summary"])),
                    ChatMessage::user("synthetic tool result"),
                ],
                SessionMutationSource::FinalSummary,
            )
            .expect("batch append");

        let events = load_events(&root, &session.id);
        assert_eq!(events.len(), 2);
        for (event, expected_role) in events.iter().zip(["assistant", "user"]) {
            match &event.mutation {
                SessionMutation::MessageAppended { message } => {
                    assert_eq!(message.role, expected_role)
                }
                other => panic!("expected MessageAppended, got {other:?}"),
            }
        }
        assert_eq!(lock_unpoisoned(&session.messages).len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn runtime_state_marks_coalesce_into_single_flush_with_final_value() {
        let (session, root) = test_session("coalesce");
        attach(&session, &root, SessionJournalMode::Shadow);

        session.set_goal_ledger(GoalLedger::new_active(
            "g1",
            "first",
            vec!["t1".to_string()],
            1,
        ));
        session.set_goal_ledger(GoalLedger::new_active(
            "g2",
            "final",
            vec!["t1".to_string(), "t2".to_string()],
            2,
        ));
        session
            .flush_session_runtime_state(SessionMutationSource::RoundCompletion)
            .expect("flush");

        let events = load_events(&root, &session.id);
        assert_eq!(events.len(), 1, "two goal marks must coalesce to one event");
        match &events[0].mutation {
            SessionMutation::RuntimeStateUpdated { state } => {
                let ledger = state.goal_ledger.as_ref().expect("goal ledger");
                let goal = ledger.active_goal().expect("active goal");
                assert_eq!(goal.objective, "final");
            }
            other => panic!("expected RuntimeStateUpdated, got {other:?}"),
        }

        // A flush with no new marks emits nothing.
        session
            .flush_session_runtime_state(SessionMutationSource::RoundCompletion)
            .expect("empty flush");
        assert_eq!(load_events(&root, &session.id).len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scripted_round_emits_expected_runtime_state_sequence() {
        let (session, root) = test_session("scripted-round");
        attach(&session, &root, SessionJournalMode::Shadow);

        // 1. Goal set.
        session.set_goal_ledger(GoalLedger::new_active(
            "g",
            "objective",
            vec!["t1".to_string()],
            1,
        ));
        session
            .flush_session_runtime_state(SessionMutationSource::RoundCompletion)
            .expect("flush goal");

        // 2. A2A task spawned.
        let task_id = {
            let mut bus = lock_unpoisoned(&session.a2a_bus);
            crate::agent::a2a::supervisor::assign_delegate_task(&mut bus, "research", "task", 2)
        };
        session.mark_a2a_state_dirty();
        session
            .flush_session_runtime_state(SessionMutationSource::RoundCompletion)
            .expect("flush a2a add");

        // 3. A2A status changed.
        {
            let mut bus = lock_unpoisoned(&session.a2a_bus);
            bus.start_task(&task_id, 3);
        }
        session.mark_a2a_state_dirty();
        session
            .flush_session_runtime_state(SessionMutationSource::RoundCompletion)
            .expect("flush a2a status");

        // 4. Latest-turn pointer advance at round completion.
        session.mark_latest_turn_dirty();
        session
            .flush_session_runtime_state(SessionMutationSource::RoundCompletion)
            .expect("flush turn");

        let events = load_events(&root, &session.id);
        assert_eq!(events.len(), 4, "one coalesced event per flush");
        let states: Vec<&SessionRuntimeState> = events
            .iter()
            .map(|event| match &event.mutation {
                SessionMutation::RuntimeStateUpdated { state } => state,
                other => panic!("expected RuntimeStateUpdated, got {other:?}"),
            })
            .collect();

        assert!(states[0].goal_ledger.is_some());
        assert!(states[0].a2a_state.is_none());

        assert_eq!(states[1].a2a_state.as_ref().unwrap().tasks.len(), 1);

        let task = states[2]
            .a2a_state
            .as_ref()
            .unwrap()
            .tasks
            .iter()
            .find(|task| task.task_id == task_id)
            .expect("task carried in journaled state");
        assert!(
            task.started_at_ms.is_some(),
            "status change must be carried"
        );

        // Full-payload semantics: earlier fields persist across later flushes.
        assert!(states[3].goal_ledger.is_some());
        assert!(states[3].a2a_state.is_some());
        // Deferred fields stay empty (see module docs).
        assert!(states[3].pending_confirms.is_empty());
        assert!(states[3].active_tool_calls.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn restore_state_journals_baseline_before_in_memory_restore() {
        let (session, root) = test_session("restore-baseline");
        attach(&session, &root, SessionJournalMode::Shadow);
        let ledger = GoalLedger::new_active("g", "restored goal", vec!["t1".to_string()], 7);

        session.restore_state_with_provenance(
            vec![
                ChatMessage::user("old"),
                ChatMessage::assistant(serde_json::json!(["reply"])),
            ],
            Some("prior summary".to_string()),
            None,
            Some(ledger),
            None,
            Some(SessionRestoreProvenance {
                snapshot_schema_version: 1,
            }),
        );

        let events = load_events(&root, &session.id);
        assert_eq!(events.len(), 2);
        match &events[0].mutation {
            SessionMutation::ConversationReplaced {
                checkpoint_id,
                messages,
                summary,
            } => {
                assert!(checkpoint_id.starts_with("snapshot-restore-"));
                assert_eq!(messages.len(), 2);
                assert_eq!(summary.as_deref(), Some("prior summary"));
            }
            other => panic!("expected ConversationReplaced baseline, got {other:?}"),
        }
        match &events[1].mutation {
            SessionMutation::RuntimeStateUpdated { state } => {
                assert!(state.goal_ledger.is_some());
            }
            other => panic!("expected RuntimeStateUpdated baseline, got {other:?}"),
        }
        assert_eq!(lock_unpoisoned(&session.messages).len(), 2);
        assert_eq!(
            lock_unpoisoned(&session.summary).as_deref(),
            Some("prior summary")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn initialize_session_journal_records_initialized_once() {
        let (session, root) = test_session("init-once");
        // Point the default root at our temp dir.
        std::env::set_var("FORGE_SESSION_JOURNAL_ROOT", &root);
        session.initialize_session_journal();
        session.initialize_session_journal();
        std::env::remove_var("FORGE_SESSION_JOURNAL_ROOT");

        let events = load_events(&root, &session.id);
        let init_count = events
            .iter()
            .filter(|event| matches!(event.mutation, SessionMutation::SessionInitialized { .. }))
            .count();
        assert_eq!(
            init_count, 1,
            "re-initialization must continue, not duplicate"
        );
        assert!(session.session_journal_path().is_some());
        let _ = std::fs::remove_dir_all(&root);
    }
}
