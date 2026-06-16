use crate::loop_runtime::types::{LoopEventEnvelope, LoopRuntimeEvent};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct LoopEventJournal {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendResult {
    pub appended: bool,
    pub event: LoopEventEnvelope,
}

impl LoopEventJournal {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn persistent_default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self::persistent_at(PathBuf::from(home).join(".forge").join("loop-events.jsonl"))
    }

    pub fn persistent_at(path: PathBuf) -> Self {
        Self::new(path)
    }

    pub fn load_all(&self) -> Result<Vec<LoopEventEnvelope>, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "loop event journal lock poisoned".to_string())?;
        self.load_all_unlocked()
    }

    fn load_all_unlocked(&self) -> Result<Vec<LoopEventEnvelope>, String> {
        let raw = match std::fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("read loop event journal: {error}")),
        };
        raw.lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return None;
                }
                Some(
                    serde_json::from_str::<LoopEventEnvelope>(trimmed).map_err(|error| {
                        format!("corrupt loop event journal line {}: {error}", index + 1)
                    }),
                )
            })
            .collect()
    }

    pub fn append(&self, event: LoopEventEnvelope) -> Result<AppendResult, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "loop event journal lock poisoned".to_string())?;
        let existing = self.load_all_unlocked()?;
        let event = self.prepare_event(event, &existing);
        self.append_prepared(&event)?;
        Ok(AppendResult {
            appended: true,
            event,
        })
    }

    pub fn append_idempotent(&self, event: LoopEventEnvelope) -> Result<AppendResult, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "loop event journal lock poisoned".to_string())?;
        let existing = self.load_all_unlocked()?;
        if let Some(key) = event.idempotency_key.as_deref() {
            if let Some(found) = existing
                .iter()
                .find(|existing| existing.idempotency_key.as_deref() == Some(key))
            {
                if event_payload_fingerprint(found)? == event_payload_fingerprint(&event)? {
                    return Ok(AppendResult {
                        appended: false,
                        event: found.clone(),
                    });
                }
                return Err(format!("idempotency conflict for key: {key}"));
            }
        }

        let event = self.prepare_event(event, &existing);
        self.append_prepared(&event)?;
        Ok(AppendResult {
            appended: true,
            event,
        })
    }

    pub fn find_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<LoopEventEnvelope>, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "loop event journal lock poisoned".to_string())?;
        Ok(self
            .load_all_unlocked()?
            .into_iter()
            .find(|event| event.idempotency_key.as_deref() == Some(idempotency_key)))
    }

    fn prepare_event(
        &self,
        mut event: LoopEventEnvelope,
        existing: &[LoopEventEnvelope],
    ) -> LoopEventEnvelope {
        let next_sequence = existing
            .iter()
            .filter(|existing| existing.task_id == event.task_id)
            .map(|existing| existing.sequence)
            .max()
            .unwrap_or(0)
            + 1;
        event.sequence = next_sequence;
        event
    }

    fn append_prepared(&self, event: &LoopEventEnvelope) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create loop event journal dir: {error}"))?;
        }
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)
            .map_err(|error| format!("open loop event journal: {error}"))?;
        let json = serde_json::to_string(event)
            .map_err(|error| format!("serialize loop event: {error}"))?;
        file.write_all(json.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|error| format!("append loop event: {error}"))?;
        Ok(())
    }
}

fn event_payload_fingerprint(event: &LoopEventEnvelope) -> Result<String, String> {
    let payload = match &event.event {
        LoopRuntimeEvent::TaskCreated { task } => serde_json::json!({
            "type": "task_created",
            "goal": task.goal,
            "session_id": task.session_id,
            "profile_id": task.profile_id,
            "workspace_path": task.workspace_path,
            "owner": task.owner,
            "policy": task.policy,
            "budget": task.budget,
            "completion_contract": task.completion_contract,
        }),
        LoopRuntimeEvent::TaskCanceled {
            task_id,
            reason,
            canceled_at_ms: _,
        } => serde_json::json!({
            "type": "task_canceled",
            "task_id": task_id,
            "reason": reason,
        }),
    };
    serde_json::to_string(&payload)
        .map_err(|error| format!("serialize loop event payload: {error}"))
}

#[cfg(test)]
mod tests {
    use crate::loop_runtime::{
        LoopEventEnvelope, LoopEventJournal, LoopTaskProjection, LoopTaskStatus,
    };
    use std::collections::HashSet;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn loop_event_journal_appends_and_replays_created_task() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path.clone());
        let event = LoopEventEnvelope::task_created_for_test("loop-1", "ship Level 3 runtime");

        journal.append(event.clone()).unwrap();

        let loaded = LoopEventJournal::new(path).load_all().unwrap();
        assert_eq!(loaded, vec![event.clone()]);

        let projection = LoopTaskProjection::from_events(&loaded).unwrap();
        assert_eq!(projection.tasks[0].id, "loop-1");
        assert_eq!(projection.tasks[0].status, LoopTaskStatus::Pending);
    }

    #[test]
    fn duplicate_idempotency_key_does_not_append_twice() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);
        let event = LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime")
            .with_idempotency_key("create:profile-settings-acceptance");

        let first = journal.append_idempotent(event.clone()).unwrap();
        let second = journal.append_idempotent(event).unwrap();

        assert!(first.appended);
        assert!(!second.appended);
        assert_eq!(journal.load_all().unwrap().len(), 1);
    }

    #[test]
    fn journal_assigns_monotonic_sequence_per_task() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);

        journal
            .append_idempotent(
                LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime")
                    .with_idempotency_key("create:loop-1"),
            )
            .unwrap();
        journal
            .append_idempotent(
                LoopEventEnvelope::task_canceled_for_test("loop-1", "done")
                    .with_idempotency_key("cancel:loop-1:done"),
            )
            .unwrap();

        let loaded = journal.load_all().unwrap();
        assert_eq!(loaded[0].sequence, 1);
        assert_eq!(loaded[1].sequence, 2);
    }

    #[test]
    fn conflicting_idempotency_key_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);

        journal
            .append_idempotent(
                LoopEventEnvelope::task_created_for_test("loop-1", "first")
                    .with_idempotency_key("create:same-key"),
            )
            .unwrap();
        let error = journal
            .append_idempotent(
                LoopEventEnvelope::task_created_for_test("loop-2", "second")
                    .with_idempotency_key("create:same-key"),
            )
            .unwrap_err();

        assert!(error.to_string().contains("idempotency conflict"));
        assert_eq!(journal.load_all().unwrap().len(), 1);
    }

    #[test]
    fn same_idempotency_key_with_different_create_payload_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = LoopEventJournal::new(path);

        journal
            .append_idempotent(
                LoopEventEnvelope::task_created_for_test("loop-1", "first")
                    .with_idempotency_key("create:same-key"),
            )
            .unwrap();
        let error = journal
            .append_idempotent(
                LoopEventEnvelope::task_created_for_test("loop-1", "second")
                    .with_idempotency_key("create:same-key"),
            )
            .unwrap_err();

        assert!(error.to_string().contains("idempotency conflict"));
        assert_eq!(journal.load_all().unwrap().len(), 1);
    }

    #[test]
    fn concurrent_duplicate_idempotency_key_appends_once() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let journal = Arc::new(LoopEventJournal::new(path));
        let barrier = Arc::new(Barrier::new(16));
        let mut handles = Vec::new();

        for _ in 0..16 {
            let journal = Arc::clone(&journal);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                let event = LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime")
                    .with_idempotency_key("create:loop-1");
                barrier.wait();
                journal.append_idempotent(event).unwrap()
            }));
        }

        let results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        let loaded = journal.load_all().unwrap();
        let sequences = loaded
            .iter()
            .map(|event| event.sequence)
            .collect::<HashSet<_>>();

        assert_eq!(results.iter().filter(|result| result.appended).count(), 1);
        assert_eq!(loaded.len(), 1);
        assert_eq!(sequences.len(), loaded.len());
    }

    #[test]
    fn corrupt_journal_line_reports_line_number() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("loop-events.jsonl");
        let valid = serde_json::to_string(&LoopEventEnvelope::task_created_for_test(
            "loop-1",
            "ship runtime",
        ))
        .unwrap();
        std::fs::write(&path, format!("{valid}\n{{not json\n")).unwrap();

        let error = LoopEventJournal::new(path).load_all().unwrap_err();

        assert!(error.contains("line 2"));
    }
}
