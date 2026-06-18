use crate::loop_runtime::headless::HeadlessResumeMode;
use crate::loop_runtime::journal::LoopEventJournal;
use crate::loop_runtime::types::{
    LoopEventEnvelope, LoopRuntimeEvent, LoopTaskOutcome, LoopTaskRecord, LoopTaskStatus,
    LOOP_RUNTIME_SCHEMA_VERSION,
};
use crate::loop_runtime::{HumanGateDecisionKind, HumanGateStatus};
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
                LoopRuntimeEvent::TaskStarted { task_id, lease } => {
                    if task_id != &event.task_id {
                        return Err(format!(
                            "task started id mismatch: envelope {}, payload {}",
                            event.task_id, task_id
                        ));
                    }
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!("task started before creation: {task_id}"));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    task.status = LoopTaskStatus::Running;
                    task.lease = Some(lease.clone());
                    task.updated_at_ms = event.created_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                }
                LoopRuntimeEvent::TaskWaitingForInput {
                    task_id,
                    reason,
                    waiting_at_ms,
                } => {
                    if task_id != &event.task_id {
                        return Err(format!(
                            "task waiting for input id mismatch: envelope {}, payload {}",
                            event.task_id, task_id
                        ));
                    }
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!("task waiting for input before creation: {task_id}"));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    task.status = LoopTaskStatus::WaitingForInput;
                    task.lease = None;
                    task.updated_at_ms = *waiting_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                    task.outcome = Some(LoopTaskOutcome {
                        status: LoopTaskStatus::WaitingForInput,
                        message: reason.clone(),
                        completed_at_ms: *waiting_at_ms,
                    });
                }
                LoopRuntimeEvent::TaskInterrupted { task_id, reason } => {
                    if task_id != &event.task_id {
                        return Err(format!(
                            "task interrupted id mismatch: envelope {}, payload {}",
                            event.task_id, task_id
                        ));
                    }
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!("task interrupted before creation: {task_id}"));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    task.status = LoopTaskStatus::Interrupted;
                    task.lease = None;
                    task.updated_at_ms = event.created_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                    task.outcome = Some(LoopTaskOutcome {
                        status: LoopTaskStatus::Interrupted,
                        message: reason.clone(),
                        completed_at_ms: event.created_at_ms,
                    });
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
                LoopRuntimeEvent::HumanGateRequested { gate } => {
                    let Some(task) = tasks.get_mut(&event.task_id) else {
                        return Err(format!(
                            "human gate requested before task creation: {}",
                            event.task_id
                        ));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    if let Some(existing) = task
                        .open_gates
                        .iter()
                        .find(|existing| existing.gate_id == gate.gate_id)
                    {
                        if existing == gate {
                            continue;
                        }
                        return Err(format!("duplicate human gate requested: {}", gate.gate_id));
                    }
                    task.status = LoopTaskStatus::WaitingForReview;
                    task.updated_at_ms = gate.requested_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                    task.open_gates.push(gate.clone());
                }
                LoopRuntimeEvent::HumanGateResolved {
                    gate_id,
                    decision,
                    resolved_at_ms,
                } => {
                    let Some(task) = tasks.get_mut(&event.task_id) else {
                        return Err(format!(
                            "human gate resolved before task creation: {}",
                            event.task_id
                        ));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    let Some(gate_index) = task
                        .open_gates
                        .iter()
                        .position(|gate| gate.gate_id == *gate_id)
                    else {
                        return Err(format!("human gate resolved before request: {gate_id}"));
                    };
                    let resolved_status = match decision.kind {
                        HumanGateDecisionKind::Approved => HumanGateStatus::Approved,
                        HumanGateDecisionKind::Denied => HumanGateStatus::Denied,
                        HumanGateDecisionKind::Canceled => HumanGateStatus::Canceled,
                    };
                    task.open_gates[gate_index].status = resolved_status;
                    task.open_gates[gate_index].decision = Some(decision.clone());
                    task.open_gates.remove(gate_index);
                    match decision.kind {
                        HumanGateDecisionKind::Approved => {
                            if task.open_gates.is_empty()
                                && task.status == LoopTaskStatus::WaitingForReview
                            {
                                task.status = LoopTaskStatus::Pending;
                            }
                        }
                        HumanGateDecisionKind::Denied | HumanGateDecisionKind::Canceled => {
                            task.status = LoopTaskStatus::WaitingForReview;
                            let label = match decision.kind {
                                HumanGateDecisionKind::Denied => "denied",
                                HumanGateDecisionKind::Canceled => "canceled",
                                HumanGateDecisionKind::Approved => unreachable!(),
                            };
                            task.outcome = Some(LoopTaskOutcome {
                                status: LoopTaskStatus::WaitingForReview,
                                message: format!("human gate {gate_id} {label}"),
                                completed_at_ms: *resolved_at_ms,
                            });
                        }
                    }
                    task.updated_at_ms = *resolved_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                }
                LoopRuntimeEvent::EvidenceRecorded { task_id, evidence } => {
                    if task_id != &event.task_id {
                        return Err(format!(
                            "evidence task id mismatch: envelope {}, payload {}",
                            event.task_id, task_id
                        ));
                    }
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!("evidence recorded before task creation: {task_id}"));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    if let Some(existing) = task
                        .evidence
                        .iter()
                        .find(|existing| evidence_id(existing) == evidence_id(evidence))
                    {
                        if existing == evidence {
                            continue;
                        }
                        return Err(format!(
                            "duplicate evidence recorded: {}",
                            evidence_id(evidence)
                        ));
                    }
                    task.updated_at_ms = event.created_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                    task.evidence.push(evidence.clone());
                }
                LoopRuntimeEvent::HeadlessResumeApprovalRecorded { task_id, approval } => {
                    if task_id != &event.task_id || approval.task_id != *task_id {
                        return Err(format!(
                            "headless resume approval task id mismatch: envelope {}, payload {}, approval {}",
                            event.task_id, task_id, approval.task_id
                        ));
                    }
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!(
                            "headless resume approval recorded before task creation: {task_id}"
                        ));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    if let Some(existing) = task.headless_resume_approval.as_ref() {
                        if existing == approval {
                            continue;
                        }
                        return Err(format!(
                            "duplicate headless resume approval recorded: {task_id}"
                        ));
                    }
                    task.headless_resume_mode = HeadlessResumeMode::ApprovedForTask;
                    task.headless_resume_approval = Some(approval.clone());
                    task.updated_at_ms = approval.approved_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                }
                LoopRuntimeEvent::PolicyDecisionRecorded { task_id, decision } => {
                    if task_id != &event.task_id {
                        return Err(format!(
                            "policy decision task id mismatch: envelope {}, payload {}",
                            event.task_id, task_id
                        ));
                    }
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!(
                            "policy decision recorded before task creation: {task_id}"
                        ));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    if let Some(existing) = task
                        .policy_decisions
                        .iter()
                        .find(|existing| existing.decision_id == decision.decision_id)
                    {
                        if existing == decision {
                            continue;
                        }
                        return Err(format!(
                            "duplicate policy decision recorded: {}",
                            decision.decision_id
                        ));
                    }
                    task.updated_at_ms = decision.created_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                    task.policy_decisions.push(decision.clone());
                }
                LoopRuntimeEvent::BudgetSnapshotRecorded { task_id, snapshot } => {
                    if task_id != &event.task_id {
                        return Err(format!(
                            "budget snapshot task id mismatch: envelope {}, payload {}",
                            event.task_id, task_id
                        ));
                    }
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!(
                            "budget snapshot recorded before task creation: {task_id}"
                        ));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    task.updated_at_ms = event.created_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                    task.latest_budget_snapshot = Some(snapshot.clone());
                }
                LoopRuntimeEvent::SubagentFileIoRecorded { task_id, .. }
                | LoopRuntimeEvent::UsageLedgerRecorded { task_id, .. } => {
                    if task_id != &event.task_id {
                        return Err(format!(
                            "telemetry task id mismatch: envelope {}, payload {}",
                            event.task_id, task_id
                        ));
                    }
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!(
                            "telemetry recorded before task creation: {task_id}"
                        ));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    task.updated_at_ms = event.created_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                }
                LoopRuntimeEvent::CompletionEvaluated { task_id, result } => {
                    if task_id != &event.task_id {
                        return Err(format!(
                            "completion evaluated task id mismatch: envelope {}, payload {}",
                            event.task_id, task_id
                        ));
                    }
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!(
                            "completion evaluated before task creation: {task_id}"
                        ));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    task.updated_at_ms = event.created_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                    task.completion_result = Some(result.clone());
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

fn evidence_id(evidence: &crate::loop_runtime::EvidenceRecord) -> &str {
    match evidence {
        crate::loop_runtime::EvidenceRecord::Command { evidence_id, .. }
        | crate::loop_runtime::EvidenceRecord::GitNexus { evidence_id, .. }
        | crate::loop_runtime::EvidenceRecord::Commit { evidence_id, .. }
        | crate::loop_runtime::EvidenceRecord::Docs { evidence_id, .. }
        | crate::loop_runtime::EvidenceRecord::Review { evidence_id, .. }
        | crate::loop_runtime::EvidenceRecord::Budget { evidence_id, .. } => evidence_id,
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
        BudgetSnapshot, HeadlessResumeApproval, HeadlessResumeMode, LoopActionIntent, LoopActor,
        LoopCompletionResult, LoopCompletionStatus, LoopEventEnvelope, LoopEventJournal,
        LoopRuntimeEvent, LoopTaskLease, LoopTaskProjection, LoopTaskProjectionStore,
        LoopTaskStatus, PolicyDecisionRecord, LOOP_RUNTIME_SCHEMA_VERSION,
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
    fn evidence_recorded_replays_into_task_projection() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let mut evidence = LoopEventEnvelope::evidence_recorded(
            "loop-1".to_string(),
            crate::loop_runtime::EvidenceRecord::command_for_test("build:desktop", true),
            None,
            None,
        );
        evidence.sequence = 2;

        let projection = LoopTaskProjection::from_events(&[created, evidence]).unwrap();

        assert_eq!(projection.tasks[0].evidence.len(), 1);
    }

    #[test]
    fn task_started_projects_existing_non_terminal_task_to_running_and_stores_full_lease() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let lease = lease_for_test("lease-1");
        let started = event_for_test(
            "loop-1",
            2,
            LoopRuntimeEvent::TaskStarted {
                task_id: "loop-1".to_string(),
                lease: lease.clone(),
            },
        );

        let projection = LoopTaskProjection::from_events(&[created, started.clone()]).unwrap();

        assert_eq!(projection.tasks[0].status, LoopTaskStatus::Running);
        assert_eq!(projection.tasks[0].lease, Some(lease));
        assert_eq!(projection.tasks[0].updated_at_ms, started.created_at_ms);
        assert_eq!(
            projection.tasks[0].latest_event_id,
            Some(started.event_id.clone())
        );
    }

    #[test]
    fn task_waiting_for_input_projects_and_replays() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let mut waiting = event_for_test(
            "loop-1",
            2,
            LoopRuntimeEvent::TaskWaitingForInput {
                task_id: "loop-1".to_string(),
                reason: "needs human decision".to_string(),
                waiting_at_ms: 3,
            },
        );
        waiting.created_at_ms = 4;

        let projection = LoopTaskProjection::from_events(&[created, waiting.clone()]).unwrap();

        assert_eq!(projection.tasks[0].status, LoopTaskStatus::WaitingForInput);
        assert_eq!(projection.tasks[0].lease, None);
        assert_eq!(
            projection.tasks[0].outcome.as_ref().unwrap().message,
            "needs human decision"
        );
        assert_eq!(projection.tasks[0].updated_at_ms, 3);
        assert_eq!(
            projection.tasks[0].latest_event_id,
            Some(waiting.event_id.clone())
        );
    }

    #[test]
    fn task_interrupted_projects_clears_lease_and_records_outcome() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let started = event_for_test(
            "loop-1",
            2,
            LoopRuntimeEvent::TaskStarted {
                task_id: "loop-1".to_string(),
                lease: lease_for_test("lease-1"),
            },
        );
        let mut interrupted = event_for_test(
            "loop-1",
            3,
            LoopRuntimeEvent::TaskInterrupted {
                task_id: "loop-1".to_string(),
                reason: "runner stopped".to_string(),
            },
        );
        interrupted.created_at_ms = 5;

        let projection =
            LoopTaskProjection::from_events(&[created, started, interrupted.clone()]).unwrap();

        assert_eq!(projection.tasks[0].status, LoopTaskStatus::Interrupted);
        assert_eq!(projection.tasks[0].lease, None);
        let outcome = projection.tasks[0].outcome.as_ref().unwrap();
        assert_eq!(outcome.status, LoopTaskStatus::Interrupted);
        assert_eq!(outcome.message, "runner stopped");
        assert_eq!(outcome.completed_at_ms, 5);
        assert_eq!(
            projection.tasks[0].latest_event_id,
            Some(interrupted.event_id.clone())
        );
    }

    #[test]
    fn policy_decision_recorded_appends_rebuilds_and_deduplicates_by_decision_id() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let decision = policy_decision_for_test("decision-1", true, "allowed");
        let recorded = event_for_test(
            "loop-1",
            2,
            LoopRuntimeEvent::PolicyDecisionRecorded {
                task_id: "loop-1".to_string(),
                decision: decision.clone(),
            },
        );
        let mut duplicate = recorded.clone();
        duplicate.sequence = 3;
        duplicate.event_id = "event-loop-1-policy-duplicate".to_string();

        let projection =
            LoopTaskProjection::from_events(&[created.clone(), recorded.clone(), duplicate])
                .unwrap();

        assert_eq!(projection.tasks[0].policy_decisions, vec![decision]);

        let mut conflicting = event_for_test(
            "loop-1",
            3,
            LoopRuntimeEvent::PolicyDecisionRecorded {
                task_id: "loop-1".to_string(),
                decision: policy_decision_for_test("decision-1", false, "blocked"),
            },
        );
        conflicting.event_id = "event-loop-1-policy-conflict".to_string();
        let error = LoopTaskProjection::from_events(&[created, recorded, conflicting]).unwrap_err();

        assert!(error.contains("duplicate policy decision recorded"));
    }

    #[test]
    fn headless_resume_approval_replays_and_deduplicates_by_task() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let approval = headless_approval_for_test("loop-1", "human-reviewer", 7);
        let recorded = event_for_test(
            "loop-1",
            2,
            LoopRuntimeEvent::HeadlessResumeApprovalRecorded {
                task_id: "loop-1".to_string(),
                approval: approval.clone(),
            },
        );
        let mut duplicate = recorded.clone();
        duplicate.sequence = 3;
        duplicate.event_id = "event-loop-1-headless-approval-duplicate".to_string();

        let projection =
            LoopTaskProjection::from_events(&[created.clone(), recorded.clone(), duplicate])
                .unwrap();

        let task = &projection.tasks[0];
        assert_eq!(
            task.headless_resume_mode,
            HeadlessResumeMode::ApprovedForTask
        );
        assert_eq!(task.headless_resume_approval.as_ref(), Some(&approval));
        assert_eq!(task.updated_at_ms, approval.approved_at_ms);
        assert_eq!(task.latest_event_id, Some(recorded.event_id.clone()));
        let serialized = serde_json::to_value(&recorded.event).unwrap();
        assert_eq!(
            serialized["type"],
            serde_json::Value::String("headless_resume_approval_recorded".to_string())
        );
        assert_eq!(recorded.event.kind(), "headless_resume_approval_recorded");

        let conflicting = event_for_test(
            "loop-1",
            3,
            LoopRuntimeEvent::HeadlessResumeApprovalRecorded {
                task_id: "loop-1".to_string(),
                approval: headless_approval_for_test("loop-1", "different-reviewer", 8),
            },
        );
        let error = LoopTaskProjection::from_events(&[created, recorded, conflicting]).unwrap_err();

        assert!(error.contains("duplicate headless resume approval recorded"));
    }

    #[test]
    fn budget_snapshot_recorded_updates_latest_budget_snapshot() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let snapshot = BudgetSnapshot {
            budget_exceeded: false,
            model_call_in_flight: false,
            tool_call_started: true,
            long_running_tool_supports_cancel: false,
            model_rounds_used: 2,
            tool_calls_used: 3,
            elapsed_ms: 4000,
            input_tokens: None,
            output_tokens: None,
            estimated_cost_micros: None,
            has_unknown_token_usage: true,
            has_unknown_cost: true,
        };
        let budget = event_for_test(
            "loop-1",
            2,
            LoopRuntimeEvent::BudgetSnapshotRecorded {
                task_id: "loop-1".to_string(),
                snapshot: snapshot.clone(),
            },
        );

        let projection = LoopTaskProjection::from_events(&[created, budget]).unwrap();

        assert_eq!(projection.tasks[0].latest_budget_snapshot, Some(snapshot));
    }

    #[test]
    fn completion_evaluated_updates_completion_result() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let result = LoopCompletionResult {
            status: LoopCompletionStatus::Blocked,
            reasons: vec!["missing_required_check:test".to_string()],
        };
        let completion = event_for_test(
            "loop-1",
            2,
            LoopRuntimeEvent::CompletionEvaluated {
                task_id: "loop-1".to_string(),
                result: result.clone(),
            },
        );

        let projection = LoopTaskProjection::from_events(&[created, completion]).unwrap();

        assert_eq!(projection.tasks[0].completion_result, Some(result));
    }

    #[test]
    fn evidence_recorded_with_mismatched_envelope_task_id_errors() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let mut evidence = LoopEventEnvelope::evidence_recorded(
            "loop-2".to_string(),
            crate::loop_runtime::EvidenceRecord::command_for_test("build:desktop", true),
            None,
            None,
        );
        evidence.task_id = "loop-1".to_string();
        evidence.sequence = 2;

        let error = LoopTaskProjection::from_events(&[created, evidence]).unwrap_err();

        assert!(error.contains("evidence task id mismatch"));
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

    fn event_for_test(task_id: &str, sequence: u64, event: LoopRuntimeEvent) -> LoopEventEnvelope {
        LoopEventEnvelope {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: format!("event-{task_id}-{sequence}"),
            task_id: task_id.to_string(),
            sequence,
            event,
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id: None,
            causation_id: None,
            idempotency_key: None,
            created_at_ms: sequence,
        }
    }

    fn lease_for_test(lease_id: &str) -> LoopTaskLease {
        LoopTaskLease {
            lease_id: lease_id.to_string(),
            owner_pid: 123,
            acquired_at_ms: 2,
            expires_at_ms: 200,
            heartbeat_at_ms: 3,
        }
    }

    fn policy_decision_for_test(
        decision_id: &str,
        allowed: bool,
        reason: &str,
    ) -> PolicyDecisionRecord {
        PolicyDecisionRecord {
            decision_id: decision_id.to_string(),
            intent: LoopActionIntent::Commit {
                completion_contract_satisfied: false,
                passing_evidence: false,
            },
            allowed,
            reason: reason.to_string(),
            actor: LoopActor::Gateway,
            created_at_ms: 4,
        }
    }

    fn headless_approval_for_test(
        task_id: &str,
        approved_by: &str,
        approved_at_ms: u64,
    ) -> HeadlessResumeApproval {
        HeadlessResumeApproval {
            task_id: task_id.to_string(),
            approved_by: approved_by.to_string(),
            approved_at_ms,
            scope: "task".to_string(),
            expires_at_ms: approved_at_ms + 60_000,
        }
    }
}
