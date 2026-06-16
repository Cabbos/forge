use crate::loop_runtime::journal::LoopEventJournal;
use crate::loop_runtime::types::{
    LoopEventEnvelope, LoopRuntimeEvent, LoopTaskOutcome, LoopTaskRecord, LoopTaskStatus,
    LOOP_RUNTIME_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopTaskProjectionFile {
    pub schema_version: u32,
    pub tasks: Vec<LoopTaskRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopTaskProjection {
    pub tasks: Vec<LoopTaskRecord>,
}

impl LoopTaskProjection {
    pub fn empty() -> Self {
        Self { tasks: Vec::new() }
    }

    pub fn from_events(events: &[LoopEventEnvelope]) -> Result<Self, String> {
        let mut tasks = HashMap::<String, LoopTaskRecord>::new();
        let mut sequences = HashMap::<String, u64>::new();
        for event in events {
            validate_next_sequence(&mut sequences, event)?;
            match &event.event {
                LoopRuntimeEvent::TaskCreated { task } => {
                    if let Some(existing) = tasks.get(&task.id) {
                        if existing == task {
                            continue;
                        }
                        return Err(format!("duplicate task created: {}", task.id));
                    }
                    tasks.insert(task.id.clone(), task.clone());
                }
                LoopRuntimeEvent::TaskCanceled {
                    task_id,
                    reason,
                    canceled_at_ms,
                } => {
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!("task canceled before creation: {task_id}"));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    task.status = LoopTaskStatus::Canceled;
                    task.updated_at_ms = *canceled_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                    task.outcome = Some(LoopTaskOutcome {
                        status: LoopTaskStatus::Canceled,
                        message: reason
                            .clone()
                            .unwrap_or_else(|| "loop task canceled".to_string()),
                        completed_at_ms: *canceled_at_ms,
                    });
                    task.lease = None;
                }
            }
        }

        let mut tasks = tasks.into_values().collect::<Vec<_>>();
        tasks.sort_by(|left, right| {
            left.created_at_ms
                .cmp(&right.created_at_ms)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(Self { tasks })
    }

    pub fn find(&self, task_id: &str) -> Option<&LoopTaskRecord> {
        self.tasks.iter().find(|task| task.id == task_id)
    }
}

fn validate_next_sequence(
    sequences: &mut HashMap<String, u64>,
    event: &LoopEventEnvelope,
) -> Result<(), String> {
    let previous = sequences.get(&event.task_id).copied().unwrap_or(0);
    let expected = previous + 1;
    if event.sequence != expected {
        return Err(format!(
            "invalid loop event sequence for task {}: got {}, expected {} after {}",
            event.task_id, event.sequence, expected, previous
        ));
    }
    sequences.insert(event.task_id.clone(), event.sequence);
    Ok(())
}

impl From<LoopTaskProjectionFile> for LoopTaskProjection {
    fn from(file: LoopTaskProjectionFile) -> Self {
        Self { tasks: file.tasks }
    }
}

impl From<&LoopTaskProjection> for LoopTaskProjectionFile {
    fn from(projection: &LoopTaskProjection) -> Self {
        Self {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            tasks: projection.tasks.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoopTaskProjectionStore {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl LoopTaskProjectionStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn persistent_default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self::persistent_at(PathBuf::from(home).join(".forge").join("loop-tasks.json"))
    }

    pub fn persistent_at(path: PathBuf) -> Self {
        Self::new(path)
    }

    pub fn load_or_rebuild(
        &self,
        journal: &LoopEventJournal,
    ) -> Result<LoopTaskProjection, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "loop task projection lock poisoned".to_string())?;
        self.rebuild_from_journal_unlocked(journal)
    }

    pub fn rebuild_from_journal(
        &self,
        journal: &LoopEventJournal,
    ) -> Result<LoopTaskProjection, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "loop task projection lock poisoned".to_string())?;
        self.rebuild_from_journal_unlocked(journal)
    }

    fn rebuild_from_journal_unlocked(
        &self,
        journal: &LoopEventJournal,
    ) -> Result<LoopTaskProjection, String> {
        let events = journal.load_all()?;
        let projection = LoopTaskProjection::from_events(&events)?;
        self.save_unlocked(&projection)?;
        Ok(projection)
    }

    pub fn save(&self, projection: &LoopTaskProjection) -> Result<(), String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "loop task projection lock poisoned".to_string())?;
        self.save_unlocked(projection)
    }

    fn save_unlocked(&self, projection: &LoopTaskProjection) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create loop task projection dir: {error}"))?;
        }
        let file = LoopTaskProjectionFile::from(projection);
        let json = serde_json::to_string_pretty(&file)
            .map_err(|error| format!("serialize loop task projection: {error}"))?;
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, json.as_bytes())
            .map_err(|error| format!("write loop task projection tmp: {error}"))?;
        std::fs::rename(&tmp, &self.path)
            .map_err(|error| format!("replace loop task projection: {error}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::loop_runtime::projection::LoopTaskProjectionFile;
    use crate::loop_runtime::{
        LoopEventEnvelope, LoopEventJournal, LoopTaskProjection, LoopTaskProjectionStore,
        LoopTaskStatus, LOOP_RUNTIME_SCHEMA_VERSION,
    };

    #[test]
    fn corrupt_projection_rebuilds_from_journal() {
        let temp = tempfile::tempdir().unwrap();
        let journal_path = temp.path().join("loop-events.jsonl");
        let projection_path = temp.path().join("loop-tasks.json");
        let journal = LoopEventJournal::new(journal_path);
        journal
            .append(LoopEventEnvelope::task_created_for_test(
                "loop-1",
                "ship runtime",
            ))
            .unwrap();
        std::fs::write(&projection_path, "{not json").unwrap();

        let projection = LoopTaskProjectionStore::new(projection_path)
            .load_or_rebuild(&journal)
            .unwrap();

        assert_eq!(projection.tasks.len(), 1);
        assert_eq!(projection.tasks[0].status, LoopTaskStatus::Pending);
    }

    #[test]
    fn missing_journal_overrides_stale_projection_cache() {
        let temp = tempfile::tempdir().unwrap();
        let journal_path = temp.path().join("loop-events.jsonl");
        let projection_path = temp.path().join("loop-tasks.json");
        let stale = LoopTaskProjection {
            tasks: vec![crate::loop_runtime::LoopTaskRecord::new_for_test(
                "loop-stale",
                "stale cache",
            )],
        };
        LoopTaskProjectionStore::new(projection_path.clone())
            .save(&stale)
            .unwrap();

        let projection = LoopTaskProjectionStore::new(projection_path)
            .load_or_rebuild(&LoopEventJournal::new(journal_path))
            .unwrap();

        assert!(projection.tasks.is_empty());
    }

    #[test]
    fn stale_projection_cache_rebuilds_after_journal_append() {
        let temp = tempfile::tempdir().unwrap();
        let journal_path = temp.path().join("loop-events.jsonl");
        let projection_path = temp.path().join("loop-tasks.json");
        std::fs::write(
            &projection_path,
            serde_json::to_string(&LoopTaskProjectionFile {
                schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
                tasks: Vec::new(),
            })
            .unwrap(),
        )
        .unwrap();
        let journal = LoopEventJournal::new(journal_path);
        journal
            .append(LoopEventEnvelope::task_created_for_test(
                "loop-new",
                "new journal event",
            ))
            .unwrap();

        let projection = LoopTaskProjectionStore::new(projection_path)
            .load_or_rebuild(&journal)
            .unwrap();

        assert_eq!(projection.tasks.len(), 1);
        assert_eq!(projection.tasks[0].id, "loop-new");
    }

    #[test]
    fn duplicate_task_created_for_same_task_errors() {
        let first = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let mut second = LoopEventEnvelope::task_created_for_test("loop-1", "second");
        second.sequence = 2;

        let error = LoopTaskProjection::from_events(&[first, second]).unwrap_err();

        assert!(error.contains("duplicate task created"));
    }

    #[test]
    fn duplicate_identical_task_created_is_ignored() {
        let first = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let mut second = first.clone();
        second.sequence = 2;

        let projection = LoopTaskProjection::from_events(&[first, second]).unwrap();

        assert_eq!(projection.tasks.len(), 1);
        assert_eq!(projection.tasks[0].goal, "first");
    }

    #[test]
    fn terminal_task_ignores_later_events() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let mut canceled = LoopEventEnvelope::task_canceled_for_test("loop-1", "done");
        canceled.sequence = 2;
        let mut later_cancel = LoopEventEnvelope::task_canceled_for_test("loop-1", "too late");
        later_cancel.sequence = 3;

        let projection =
            LoopTaskProjection::from_events(&[created, canceled, later_cancel]).unwrap();

        assert_eq!(projection.tasks[0].status, LoopTaskStatus::Canceled);
        assert_eq!(
            projection.tasks[0].outcome.as_ref().unwrap().message,
            "done"
        );
    }

    #[test]
    fn out_of_order_sequence_errors() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let mut canceled = LoopEventEnvelope::task_canceled_for_test("loop-1", "done");
        canceled.sequence = 2;
        let mut out_of_order = LoopEventEnvelope::task_canceled_for_test("loop-1", "late");
        out_of_order.sequence = 1;

        let error =
            LoopTaskProjection::from_events(&[created, canceled, out_of_order]).unwrap_err();

        assert!(error.contains("task loop-1"));
        assert!(error.contains("got 1"));
        assert!(error.contains("expected 3"));
    }

    #[test]
    fn skipped_sequence_errors() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let mut canceled = LoopEventEnvelope::task_canceled_for_test("loop-1", "done");
        canceled.sequence = 3;

        let error = LoopTaskProjection::from_events(&[created, canceled]).unwrap_err();

        assert!(error.contains("task loop-1"));
        assert!(error.contains("got 3"));
        assert!(error.contains("expected 2"));
    }

    #[test]
    fn duplicate_sequence_errors() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let mut canceled = LoopEventEnvelope::task_canceled_for_test("loop-1", "done");
        canceled.sequence = 1;

        let error = LoopTaskProjection::from_events(&[created, canceled]).unwrap_err();

        assert!(error.contains("task loop-1"));
        assert!(error.contains("got 1"));
        assert!(error.contains("expected 2"));
    }

    #[test]
    fn zero_first_sequence_errors() {
        let mut created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        created.sequence = 0;

        let error = LoopTaskProjection::from_events(&[created]).unwrap_err();

        assert!(error.contains("task loop-1"));
        assert!(error.contains("got 0"));
        assert!(error.contains("expected 1"));
    }
}
