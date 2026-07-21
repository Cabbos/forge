use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::adapters::base::ChatMessage;
use crate::agent::a2a::bus::AgentA2ABus;
use crate::agent::goal_state::GoalLedger;
use crate::agent::snapshot::{ActiveToolCallDescriptor, PendingConfirmDescriptor};
use crate::agent::turn_state::AgentTurnState;
use crate::protocol::events::DeliverySummary;
use crate::workflow::WorkflowState;

pub(crate) const SESSION_JOURNAL_SCHEMA_VERSION: u32 = 1;
pub(crate) const DEFAULT_TRUNCATION_THRESHOLD_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SessionMutationEnvelope {
    pub schema_version: u32,
    pub event_id: String,
    pub session_id: String,
    pub sequence: u64,
    pub created_at_ms: u64,
    pub mutation: SessionMutation,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub(crate) enum SessionMutation {
    SessionInitialized {
        provider: String,
        model: String,
        working_dir: String,
    },
    MessageAppended {
        message: ChatMessage,
    },
    ConversationReplaced {
        checkpoint_id: String,
        messages: Vec<ChatMessage>,
        summary: Option<String>,
    },
    RuntimeStateUpdated {
        state: SessionRuntimeState,
    },
}

/// Serializable snapshot of runtime state that can be persisted in the journal.
/// Mirrors the additive runtime fields of `AgentSessionSnapshot` without provider
/// secrets, live senders, cancellation handles, or `AppHandle` values.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SessionRuntimeState {
    #[serde(default)]
    pub latest_turn: Option<AgentTurnState>,
    #[serde(default)]
    pub latest_workflow: Option<WorkflowState>,
    #[serde(default)]
    pub latest_delivery: Option<DeliverySummary>,
    #[serde(default)]
    pub goal_ledger: Option<GoalLedger>,
    #[serde(default)]
    pub a2a_state: Option<AgentA2ABus>,
    #[serde(default)]
    pub pending_confirms: Vec<PendingConfirmDescriptor>,
    #[serde(default)]
    pub active_tool_calls: Vec<ActiveToolCallDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JournalDamage {
    TornFinalLine { line: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JournalError {
    Io(String),
    LockPoisoned,
    CorruptInteriorLine {
        line: usize,
    },
    UnknownSchemaVersion {
        line: usize,
        version: u32,
    },
    MismatchedSessionId {
        line: Option<usize>,
        expected: String,
        found: String,
    },
    SequenceGap {
        line: Option<usize>,
        expected: u64,
        found: u64,
    },
    DuplicateSequence {
        line: Option<usize>,
        found: u64,
    },
    UnsafeSessionId(String),
    SchemaVersionMismatch {
        expected: u32,
        found: u32,
    },
    TruncationBaselineNotFound {
        checkpoint_id: String,
    },
}

pub(crate) type JournalLoadError = JournalError;
pub(crate) type JournalAppendError = JournalError;
pub(crate) type JournalTruncateError = JournalError;

#[derive(Debug)]
pub(crate) struct JournalLoadResult {
    pub events: Vec<SessionMutationEnvelope>,
    pub damage: Option<JournalDamage>,
}

pub(crate) struct TruncateCandidate {
    pub checkpoint_id: String,
    pub baseline_sequence: u64,
}

#[derive(Debug)]
pub(crate) struct SessionJournalStore {
    root: PathBuf,
    session_id: String,
    lock: Arc<Mutex<()>>,
}

impl SessionJournalStore {
    pub(crate) fn new(root: PathBuf, session_id: String) -> Result<Self, JournalAppendError> {
        if !is_safe_session_id(&session_id) {
            return Err(JournalError::UnsafeSessionId(session_id));
        }
        let path = journal_path(&root, &session_id);
        let lock = shared_lock_for_path(&path);
        Ok(Self {
            root,
            session_id,
            lock,
        })
    }

    pub(crate) fn path(&self) -> PathBuf {
        journal_path(&self.root, &self.session_id)
    }

    fn session_dir(&self) -> PathBuf {
        session_dir(&self.root, &self.session_id)
    }

    fn generation_path(&self, generation: u32) -> PathBuf {
        self.session_dir()
            .join(format!("mutations.gen{generation}.jsonl"))
    }

    pub(crate) fn append(
        &self,
        mut envelope: SessionMutationEnvelope,
    ) -> Result<(), JournalAppendError> {
        let _guard = self.lock.lock().map_err(|_| JournalError::LockPoisoned)?;
        if envelope.schema_version != SESSION_JOURNAL_SCHEMA_VERSION {
            return Err(JournalError::SchemaVersionMismatch {
                expected: SESSION_JOURNAL_SCHEMA_VERSION,
                found: envelope.schema_version,
            });
        }
        if envelope.session_id != self.session_id {
            return Err(JournalError::MismatchedSessionId {
                line: None,
                expected: self.session_id.clone(),
                found: envelope.session_id,
            });
        }
        let load_result = self.load_unlocked()?;
        if load_result.damage.is_some() {
            self.truncate_torn_line()?;
        }
        let last_sequence = load_result
            .events
            .last()
            .map(|event| event.sequence)
            .unwrap_or(0);
        let expected_sequence = last_sequence + 1;
        if envelope.sequence == 0 {
            envelope.sequence = expected_sequence;
        } else if envelope.sequence != expected_sequence {
            return Err(JournalError::SequenceGap {
                line: None,
                expected: expected_sequence,
                found: envelope.sequence,
            });
        }
        if envelope.event_id.is_empty() {
            envelope.event_id = uuid::Uuid::now_v7().to_string();
        }
        self.append_prepared(&envelope)?;
        Ok(())
    }

    pub(crate) fn load(&self) -> Result<JournalLoadResult, JournalLoadError> {
        let _guard = self.lock.lock().map_err(|_| JournalError::LockPoisoned)?;
        self.load_unlocked()
    }

    fn load_unlocked(&self) -> Result<JournalLoadResult, JournalLoadError> {
        let path = self.path();
        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(JournalLoadResult {
                    events: Vec::new(),
                    damage: None,
                });
            }
            Err(error) => return Err(JournalError::Io(format!("read session journal: {error}"))),
        };
        if raw.is_empty() {
            return Ok(JournalLoadResult {
                events: Vec::new(),
                damage: None,
            });
        }
        let lines: Vec<&str> = raw.split_inclusive('\n').collect();
        let mut events: Vec<SessionMutationEnvelope> = Vec::new();
        let mut damage = None;
        for (index, line_with_newline) in lines.iter().enumerate() {
            let line_number = index + 1;
            let is_last = index == lines.len() - 1;
            let trimmed = line_with_newline.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !line_with_newline.ends_with('\n') {
                if is_last {
                    damage = Some(JournalDamage::TornFinalLine { line: line_number });
                    break;
                }
                return Err(JournalError::CorruptInteriorLine { line: line_number });
            }
            let line = line_with_newline
                .strip_suffix('\n')
                .unwrap_or(line_with_newline);
            let envelope: SessionMutationEnvelope = serde_json::from_str(line)
                .map_err(|_error| JournalError::CorruptInteriorLine { line: line_number })?;
            if envelope.schema_version != SESSION_JOURNAL_SCHEMA_VERSION {
                return Err(JournalError::UnknownSchemaVersion {
                    line: line_number,
                    version: envelope.schema_version,
                });
            }
            if envelope.session_id != self.session_id {
                return Err(JournalError::MismatchedSessionId {
                    line: Some(line_number),
                    expected: self.session_id.clone(),
                    found: envelope.session_id,
                });
            }
            if !events.is_empty() {
                let expected_sequence = events.last().expect("events not empty").sequence + 1;
                if envelope.sequence != expected_sequence {
                    if envelope.sequence <= events.last().expect("events not empty").sequence {
                        return Err(JournalError::DuplicateSequence {
                            line: Some(line_number),
                            found: envelope.sequence,
                        });
                    }
                    return Err(JournalError::SequenceGap {
                        line: Some(line_number),
                        expected: expected_sequence,
                        found: envelope.sequence,
                    });
                }
            }
            events.push(envelope);
        }
        Ok(JournalLoadResult { events, damage })
    }

    fn truncate_torn_line(&self) -> Result<(), JournalError> {
        let path = self.path();
        let raw = std::fs::read_to_string(&path)
            .map_err(|error| JournalError::Io(format!("read journal for torn repair: {error}")))?;
        if let Some(position) = raw.rfind('\n') {
            let prefix = &raw[..=position];
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, prefix)
                .map_err(|error| JournalError::Io(format!("write torn repair tmp: {error}")))?;
            std::fs::rename(&tmp, &path).map_err(|error| {
                JournalError::Io(format!("replace journal after torn repair: {error}"))
            })?;
        } else {
            std::fs::remove_file(&path)
                .map_err(|error| JournalError::Io(format!("remove torn journal: {error}")))?;
        }
        Ok(())
    }

    fn append_prepared(&self, envelope: &SessionMutationEnvelope) -> Result<(), JournalError> {
        let path = self.path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| JournalError::Io(format!("create journal dir: {error}")))?;
        }
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .map_err(|error| JournalError::Io(format!("open journal: {error}")))?;
        let json = serde_json::to_string(envelope)
            .map_err(|error| JournalError::Io(format!("serialize mutation: {error}")))?;
        file.write_all(json.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|error| JournalError::Io(format!("append mutation: {error}")))?;
        Ok(())
    }

    pub(crate) fn should_truncate(
        &self,
        size_threshold: u64,
    ) -> Result<Option<TruncateCandidate>, JournalLoadError> {
        let _guard = self.lock.lock().map_err(|_| JournalError::LockPoisoned)?;
        let path = self.path();
        let size = match std::fs::metadata(&path) {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(JournalError::Io(format!("stat journal: {error}"))),
        };
        if size <= size_threshold {
            return Ok(None);
        }
        let load_result = self.load_unlocked()?;
        if load_result.damage.is_some() {
            return Ok(None);
        }
        let candidate = load_result.events.iter().rev().find_map(|event| {
            if let SessionMutation::ConversationReplaced { checkpoint_id, .. } = &event.mutation {
                Some(TruncateCandidate {
                    checkpoint_id: checkpoint_id.clone(),
                    baseline_sequence: event.sequence,
                })
            } else {
                None
            }
        });
        Ok(candidate)
    }

    pub(crate) fn truncate(&self, checkpoint_id: &str) -> Result<(), JournalTruncateError> {
        let _guard = self.lock.lock().map_err(|_| JournalError::LockPoisoned)?;
        let load_result = self.load_unlocked()?;
        if load_result.damage.is_some() {
            return Err(JournalError::Io(
                "cannot truncate a journal with recorded damage".to_string(),
            ));
        }
        let baseline_index = load_result
            .events
            .iter()
            .position(|event| {
                matches!(
                    &event.mutation,
                    SessionMutation::ConversationReplaced { checkpoint_id: candidate, .. }
                    if candidate == checkpoint_id
                )
            })
            .ok_or_else(|| JournalError::TruncationBaselineNotFound {
                checkpoint_id: checkpoint_id.to_string(),
            })?;
        let baseline_event = load_result.events[baseline_index].clone();
        let original_init = load_result
            .events
            .first()
            .ok_or_else(|| JournalError::Io("journal has no initial event".to_string()))?;
        let SessionMutation::SessionInitialized {
            provider,
            model,
            working_dir,
        } = &original_init.mutation
        else {
            return Err(JournalError::Io(
                "journal first event is not SessionInitialized".to_string(),
            ));
        };

        let current_generation = self.current_generation_number()?;
        let next_generation = current_generation + 1;
        let new_gen_path = self.generation_path(next_generation);
        let tmp_path = new_gen_path.with_extension("tmp");

        {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|error| JournalError::Io(format!("create new generation tmp: {error}")))?;
            let synthetic_init = SessionMutationEnvelope {
                schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
                event_id: uuid::Uuid::now_v7().to_string(),
                session_id: self.session_id.clone(),
                sequence: baseline_event.sequence.saturating_sub(1),
                created_at_ms: now_ms(),
                mutation: SessionMutation::SessionInitialized {
                    provider: provider.clone(),
                    model: model.clone(),
                    working_dir: working_dir.clone(),
                },
            };
            self.write_line(&mut file, &synthetic_init)?;
            self.write_line(&mut file, &baseline_event)?;
            for event in &load_result.events[baseline_index + 1..] {
                self.write_line(&mut file, event)?;
            }
            file.sync_all()
                .map_err(|error| JournalError::Io(format!("sync new generation: {error}")))?;
        }
        std::fs::rename(&tmp_path, &new_gen_path).map_err(|error| {
            let _ = std::fs::remove_file(&tmp_path);
            JournalError::Io(format!("commit new generation: {error}"))
        })?;

        let active_path = self.path();
        let archived_path = self.generation_path(current_generation);
        if let Err(error) = std::fs::rename(&active_path, &archived_path) {
            let _ = std::fs::remove_file(&new_gen_path);
            return Err(JournalError::Io(format!(
                "archive current generation: {error}"
            )));
        }
        if let Err(error) = std::fs::rename(&new_gen_path, &active_path) {
            let _ = std::fs::rename(&archived_path, &active_path);
            let _ = std::fs::remove_file(&new_gen_path);
            return Err(JournalError::Io(format!(
                "activate new generation: {error}"
            )));
        }
        Ok(())
    }

    fn write_line(
        &self,
        file: &mut std::fs::File,
        envelope: &SessionMutationEnvelope,
    ) -> Result<(), JournalError> {
        let json = serde_json::to_string(envelope)
            .map_err(|error| JournalError::Io(format!("serialize generation line: {error}")))?;
        file.write_all(json.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|error| JournalError::Io(format!("write generation line: {error}")))
    }

    fn current_generation_number(&self) -> Result<u32, JournalError> {
        let max_archived = self.max_archived_generation_number()?;
        Ok(max_archived.map(|generation| generation + 1).unwrap_or(0))
    }

    fn max_archived_generation_number(&self) -> Result<Option<u32>, JournalError> {
        let dir = self.session_dir();
        if !dir.exists() {
            return Ok(None);
        }
        let mut max_generation: Option<u32> = None;
        for entry in std::fs::read_dir(&dir)
            .map_err(|error| JournalError::Io(format!("read session dir: {error}")))?
        {
            let entry =
                entry.map_err(|error| JournalError::Io(format!("read dir entry: {error}")))?;
            if !entry
                .file_type()
                .map_err(|error| JournalError::Io(format!("read file type: {error}")))?
                .is_file()
            {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some(body) = name.strip_prefix("mutations.gen") else {
                continue;
            };
            let Some(body) = body.strip_suffix(".jsonl") else {
                continue;
            };
            if let Ok(generation) = body.parse::<u32>() {
                max_generation = Some(match max_generation {
                    Some(current) => current.max(generation),
                    None => generation,
                });
            }
        }
        Ok(max_generation)
    }
}

fn journal_path(root: &Path, session_id: &str) -> PathBuf {
    session_dir(root, session_id).join("mutations.jsonl")
}

fn session_dir(root: &Path, session_id: &str) -> PathBuf {
    root.join("sessions").join(session_id)
}

fn is_safe_session_id(session_id: &str) -> bool {
    let sanitized: String = session_id
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || *character == '-' || *character == '_'
        })
        .collect();
    !sanitized.is_empty() && sanitized == session_id
}

fn shared_lock_for_path(path: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();
    let key = normalize_journal_lock_path(path);
    let mut locks = LOCKS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(key, Arc::downgrade(&lock));
    lock
}

fn normalize_journal_lock_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn test_store(_label: &str) -> SessionJournalStore {
        let temp = tempfile::tempdir().unwrap();
        let store =
            SessionJournalStore::new(temp.path().to_path_buf(), "session-1".to_string()).unwrap();
        // Leak the temp dir so tests do not need to keep it alive; the OS cleans it on exit.
        Box::leak(Box::new(temp));
        store
    }

    fn test_initialized() -> SessionMutationEnvelope {
        SessionMutationEnvelope {
            schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
            event_id: "init".to_string(),
            session_id: "session-1".to_string(),
            sequence: 0,
            created_at_ms: 1,
            mutation: SessionMutation::SessionInitialized {
                provider: "openai".to_string(),
                model: "gpt-5".to_string(),
                working_dir: "/workspace".to_string(),
            },
        }
    }

    fn test_message(text: &str) -> SessionMutationEnvelope {
        SessionMutationEnvelope {
            schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
            event_id: format!("msg-{text}"),
            session_id: "session-1".to_string(),
            sequence: 0,
            created_at_ms: 2,
            mutation: SessionMutation::MessageAppended {
                message: ChatMessage::user(text),
            },
        }
    }

    fn append_raw(path: &Path, bytes: &[u8]) {
        use std::io::Write;
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
    }

    #[test]
    fn append_assigns_monotonic_sequences() {
        let store = test_store("monotonic");
        store.append(test_initialized()).unwrap();
        store.append(test_message("hello")).unwrap();
        store.append(test_message("world")).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.events.len(), 3);
        assert_eq!(loaded.events[0].sequence, 1);
        assert_eq!(loaded.events[1].sequence, 2);
        assert_eq!(loaded.events[2].sequence, 3);
    }

    #[test]
    fn concurrent_appends_serialize_without_gaps_or_duplicates() {
        let store = test_store("concurrent");
        store.append(test_initialized()).unwrap();

        let barrier = Arc::new(Barrier::new(8));
        let store = Arc::new(store);
        let mut handles = Vec::new();
        for index in 0..8 {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                store
                    .append(test_message(&format!("concurrent-{index}")))
                    .unwrap();
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }

        let loaded = store.load().unwrap();
        assert_eq!(loaded.events.len(), 9);
        let sequences: Vec<u64> = loaded.events.iter().map(|event| event.sequence).collect();
        let unique: std::collections::HashSet<_> = sequences.iter().copied().collect();
        assert_eq!(unique.len(), sequences.len());
        for window in sequences.windows(2) {
            assert_eq!(window[1], window[0] + 1);
        }
    }

    #[test]
    fn torn_final_line_is_reported_but_valid_prefix_replays() {
        let store = test_store("torn-final");
        store.append(test_initialized()).unwrap();
        append_raw(&store.path(), br#"{"schema_version":1"#);

        let loaded = store.load().unwrap();
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(
            loaded.damage,
            Some(JournalDamage::TornFinalLine { line: 2 })
        );
    }

    #[test]
    fn corrupt_interior_line_blocks_authoritative_replay() {
        let store = test_store("corrupt-interior");
        store.append(test_initialized()).unwrap();
        append_raw(
            &store.path(),
            br#"{"schema_version":1,"event_id":"bad","session_id":"session-1","sequence":2,"created_at_ms":2,"mutation":{"type":"message_appended","message":{"role":"user","content":"bad"}}}
"#,
        );
        append_raw(
            &store.path(),
            br#"this is not json
"#,
        );
        append_raw(
            &store.path(),
            br#"{"schema_version":1,"event_id":"ok","session_id":"session-1","sequence":4,"created_at_ms":4,"mutation":{"type":"message_appended","message":{"role":"user","content":"ok"}}}
"#,
        );

        let error = store.load().expect_err("corrupt interior should fail");
        assert_eq!(error, JournalLoadError::CorruptInteriorLine { line: 3 });
    }

    #[test]
    fn unknown_schema_version_blocks_replay() {
        let store = test_store("unknown-schema");
        store.append(test_initialized()).unwrap();
        append_raw(
            &store.path(),
            br#"{"schema_version":999,"event_id":"future","session_id":"session-1","sequence":2,"created_at_ms":2,"mutation":{"type":"message_appended","message":{"role":"user","content":"future"}}}
"#,
        );

        let error = store.load().expect_err("unknown schema should fail");
        assert_eq!(
            error,
            JournalLoadError::UnknownSchemaVersion {
                line: 2,
                version: 999,
            }
        );
    }

    #[test]
    fn unknown_mutation_variant_blocks_replay() {
        let store = test_store("unknown-mutation");
        store.append(test_initialized()).unwrap();
        append_raw(
            &store.path(),
            br#"{"schema_version":1,"event_id":"future","session_id":"session-1","sequence":2,"created_at_ms":2,"mutation":{"type":"future_mutation"}}
"#,
        );

        let error = store.load().expect_err("unknown mutation should fail");
        assert!(matches!(
            error,
            JournalLoadError::CorruptInteriorLine { line: 2 }
        ));
    }

    #[test]
    fn unsafe_session_id_is_rejected_before_filesystem_access() {
        let temp = tempfile::tempdir().unwrap();
        let result =
            SessionJournalStore::new(temp.path().to_path_buf(), "../etc-passwd".to_string());
        match result {
            Err(JournalAppendError::UnsafeSessionId(id)) => assert_eq!(id, "../etc-passwd"),
            other => panic!("expected UnsafeSessionId error, got {other:?}"),
        }
        assert!(!temp.path().join("sessions").exists());
    }

    #[test]
    fn append_rejects_mismatched_session_id() {
        let store = test_store("mismatched");
        let mut envelope = test_initialized();
        envelope.session_id = "other-session".to_string();
        let error = store
            .append(envelope)
            .expect_err("mismatched id should fail");
        assert_eq!(
            error,
            JournalAppendError::MismatchedSessionId {
                line: None,
                expected: "session-1".to_string(),
                found: "other-session".to_string(),
            }
        );
    }

    #[test]
    fn append_rejects_sequence_gap() {
        let store = test_store("sequence-gap");
        store.append(test_initialized()).unwrap();
        let mut envelope = test_message("forced");
        envelope.sequence = 5;
        let error = store
            .append(envelope)
            .expect_err("sequence gap should fail");
        assert_eq!(
            error,
            JournalAppendError::SequenceGap {
                line: None,
                expected: 2,
                found: 5,
            }
        );
    }

    #[test]
    fn append_recovers_from_torn_final_line() {
        let store = test_store("torn-recover");
        store.append(test_initialized()).unwrap();
        append_raw(&store.path(), br#"{"schema_version":1"#);

        store.append(test_message("after-torn")).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.events.len(), 2);
        assert_eq!(loaded.events[1].sequence, 2);
        assert!(loaded.damage.is_none());
    }

    #[test]
    fn empty_store_loads_cleanly() {
        let store = test_store("empty");
        let loaded = store.load().unwrap();
        assert!(loaded.events.is_empty());
        assert!(loaded.damage.is_none());
    }

    #[test]
    fn runtime_state_roundtrips() {
        use crate::agent::turn_state::{AgentTurnState, AgentTurnStatus};
        use crate::workflow::{classify_workflow, WorkflowRoute};

        let store = test_store("runtime");
        store.append(test_initialized()).unwrap();

        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/workspace".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "agent-core".to_string(),
            "phase-1".to_string(),
            "Build turn state".to_string(),
        );
        turn.mark_status(AgentTurnStatus::Completed);
        let workflow = classify_workflow("session-1", "实现功能", 42);

        let runtime = SessionRuntimeState {
            latest_turn: Some(turn),
            latest_workflow: Some(workflow),
            latest_delivery: None,
            goal_ledger: None,
            a2a_state: None,
            pending_confirms: Vec::new(),
            active_tool_calls: Vec::new(),
        };
        let envelope = SessionMutationEnvelope {
            schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
            event_id: "runtime".to_string(),
            session_id: "session-1".to_string(),
            sequence: 0,
            created_at_ms: 3,
            mutation: SessionMutation::RuntimeStateUpdated { state: runtime },
        };
        store.append(envelope).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.events.len(), 2);
        let restored = loaded.events[1].mutation.clone();
        assert!(matches!(
            restored,
            SessionMutation::RuntimeStateUpdated { .. }
        ));
        if let SessionMutation::RuntimeStateUpdated { state } = restored {
            assert!(state.latest_turn.is_some());
            assert!(matches!(
                state.latest_workflow.as_ref().unwrap().route,
                WorkflowRoute::Workflow | WorkflowRoute::Direct | WorkflowRoute::Recovery
            ));
        }
    }

    #[test]
    fn should_truncate_finds_latest_checkpoint_when_size_exceeds_threshold() {
        let store = test_store("should-truncate");
        store.append(test_initialized()).unwrap();
        store.append(test_message("a")).unwrap();
        let baseline = SessionMutationEnvelope {
            schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
            event_id: "compact".to_string(),
            session_id: "session-1".to_string(),
            sequence: 0,
            created_at_ms: 3,
            mutation: SessionMutation::ConversationReplaced {
                checkpoint_id: "checkpoint-1".to_string(),
                messages: vec![ChatMessage::user("summary")],
                summary: Some("summary".to_string()),
            },
        };
        store.append(baseline).unwrap();
        store.append(test_message("after-baseline")).unwrap();

        // Threshold small enough that the journal exceeds it.
        let candidate = store.should_truncate(1).unwrap();
        assert!(candidate.is_some());
        let candidate = candidate.unwrap();
        assert_eq!(candidate.checkpoint_id, "checkpoint-1");
        assert_eq!(candidate.baseline_sequence, 3);
    }

    #[test]
    fn should_not_truncate_when_no_checkpoint_exists() {
        let store = test_store("no-truncate");
        store.append(test_initialized()).unwrap();
        store.append(test_message("a")).unwrap();

        let candidate = store.should_truncate(1).unwrap();
        assert!(candidate.is_none());
    }

    #[test]
    fn truncation_preserves_post_baseline_sequences() {
        let store = test_store("truncate-preserves");
        store.append(test_initialized()).unwrap();
        store.append(test_message("before")).unwrap();
        let baseline = SessionMutationEnvelope {
            schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
            event_id: "compact".to_string(),
            session_id: "session-1".to_string(),
            sequence: 0,
            created_at_ms: 3,
            mutation: SessionMutation::ConversationReplaced {
                checkpoint_id: "checkpoint-1".to_string(),
                messages: vec![ChatMessage::user("baseline")],
                summary: Some("summary".to_string()),
            },
        };
        store.append(baseline).unwrap();
        store.append(test_message("after-1")).unwrap();
        store.append(test_message("after-2")).unwrap();

        store.truncate("checkpoint-1").unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.events.len(), 4);
        assert_eq!(loaded.events[0].sequence, 2);
        assert!(matches!(
            loaded.events[0].mutation,
            SessionMutation::SessionInitialized { .. }
        ));
        assert_eq!(loaded.events[1].sequence, 3);
        assert!(matches!(
            loaded.events[1].mutation,
            SessionMutation::ConversationReplaced { .. }
        ));
        assert_eq!(loaded.events[2].sequence, 4);
        assert_eq!(loaded.events[3].sequence, 5);

        let archived = store.session_dir().join("mutations.gen0.jsonl");
        assert!(archived.exists());
    }

    #[test]
    fn truncated_generation_replay_equals_full_journal_from_baseline_forward() {
        let store = test_store("truncate-equality");
        store.append(test_initialized()).unwrap();
        store.append(test_message("m1")).unwrap();
        let baseline = SessionMutationEnvelope {
            schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
            event_id: "compact".to_string(),
            session_id: "session-1".to_string(),
            sequence: 0,
            created_at_ms: 3,
            mutation: SessionMutation::ConversationReplaced {
                checkpoint_id: "checkpoint-1".to_string(),
                messages: vec![ChatMessage::user("baseline")],
                summary: Some("summary".to_string()),
            },
        };
        store.append(baseline).unwrap();
        store.append(test_message("m2")).unwrap();
        store.append(test_message("m3")).unwrap();

        let full = store.load().unwrap();
        store.truncate("checkpoint-1").unwrap();
        let truncated = store.load().unwrap();

        assert_eq!(truncated.events.len(), 4);
        // First synthetic init, then baseline, then post-baseline.
        assert!(matches!(
            truncated.events[1].mutation,
            SessionMutation::ConversationReplaced { .. }
        ));
        let post_baseline: Vec<_> = full
            .events
            .iter()
            .skip_while(|event| {
                !matches!(
                    event.mutation,
                    SessionMutation::ConversationReplaced { checkpoint_id: ref cid, .. }
                    if cid == "checkpoint-1"
                )
            })
            .cloned()
            .collect();
        assert_eq!(truncated.events[1..].len(), post_baseline.len());
        for (truncated_event, original_event) in
            truncated.events[1..].iter().zip(post_baseline.iter())
        {
            assert_eq!(truncated_event.sequence, original_event.sequence);
            assert_eq!(truncated_event.mutation, original_event.mutation);
        }
    }

    #[test]
    fn failed_truncation_leaves_original_journal_untouched() {
        let store = test_store("truncate-fail");
        store.append(test_initialized()).unwrap();
        store.append(test_message("before")).unwrap();
        let baseline = SessionMutationEnvelope {
            schema_version: SESSION_JOURNAL_SCHEMA_VERSION,
            event_id: "compact".to_string(),
            session_id: "session-1".to_string(),
            sequence: 0,
            created_at_ms: 3,
            mutation: SessionMutation::ConversationReplaced {
                checkpoint_id: "checkpoint-1".to_string(),
                messages: vec![ChatMessage::user("baseline")],
                summary: Some("summary".to_string()),
            },
        };
        store.append(baseline).unwrap();

        let original_path = store.path();
        let original_bytes = std::fs::read(&original_path).unwrap();

        // Force the archive rename to fail by creating a directory at the target path.
        let archive_target = store.generation_path(0);
        std::fs::create_dir_all(&archive_target).unwrap();

        let result = store.truncate("checkpoint-1");
        assert!(result.is_err());

        assert!(original_path.exists());
        let restored_bytes = std::fs::read(&original_path).unwrap();
        assert_eq!(restored_bytes, original_bytes);
        assert!(!store.generation_path(1).exists());
    }
}
