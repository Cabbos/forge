use crate::loop_runtime::headless::{HeadlessOwnerRun, HeadlessResumeMode};
use crate::loop_runtime::journal::LoopEventJournal;
use crate::loop_runtime::types::{
    LoopEventEnvelope, LoopRuntimeEvent, LoopTaskOutcome, LoopTaskRecord, LoopTaskRecoveryState,
    LoopTaskStatus, LOOP_RUNTIME_SCHEMA_VERSION,
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
                    task.recovery_state = None;
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
                LoopRuntimeEvent::TaskRequeued {
                    task_id,
                    reason: _,
                    requeued_at_ms,
                } => {
                    if task_id != &event.task_id {
                        return Err(format!(
                            "task requeued id mismatch: envelope {}, payload {}",
                            event.task_id, task_id
                        ));
                    }
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!("task requeued before creation: {task_id}"));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    task.status = LoopTaskStatus::Pending;
                    task.lease = None;
                    task.recovery_state = None;
                    task.outcome = None;
                    task.completion_result = None;
                    task.updated_at_ms = *requeued_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
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
                    task.recovery_state = Some(LoopTaskRecoveryState::interrupted(
                        reason.clone(),
                        event.created_at_ms,
                        Some(event.event_id.clone()),
                    ));
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
                    task.recovery_state = None;
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
                LoopRuntimeEvent::HeadlessOwnerRunRequested { task_id, owner_run } => {
                    if task_id != &event.task_id || owner_run.task_id != *task_id {
                        return Err(format!(
                            "headless owner run task id mismatch: envelope {}, payload {}, owner run {}",
                            event.task_id, task_id, owner_run.task_id
                        ));
                    }
                    validate_headless_owner_request_envelope(event, owner_run)?;
                    owner_run.validate_authorization_bundle().map_err(|error| {
                        format!("headless owner run authorization invalid: {error}")
                    })?;
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!(
                            "headless owner run requested before task creation: {task_id}"
                        ));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    if let Some(existing) = task.headless_owner_runs.iter().find(|existing| {
                        existing.owner_run_id == owner_run.owner_run_id
                            || existing.matches_idempotency_key(task_id, &owner_run.idempotency_key)
                    }) {
                        if headless_owner_request_fields_match(existing, owner_run) {
                            continue;
                        }
                        return Err(format!(
                            "duplicate headless owner run requested: {}",
                            owner_run.owner_run_id
                        ));
                    }
                    task.updated_at_ms = owner_run.requested_at_ms;
                    task.latest_event_id = Some(event.event_id.clone());
                    task.headless_owner_runs.push(owner_run.clone());
                }
                LoopRuntimeEvent::HeadlessOwnerRunStateRecorded {
                    task_id,
                    owner_run_id,
                    state,
                    heartbeat_at_ms,
                    cancellation_reason,
                    waiting_reason,
                    evidence_refs,
                } => {
                    if task_id != &event.task_id {
                        return Err(format!(
                            "headless owner run state task id mismatch: envelope {}, payload {}",
                            event.task_id, task_id
                        ));
                    }
                    let Some(task) = tasks.get_mut(task_id) else {
                        return Err(format!(
                            "headless owner run state recorded before task creation: {task_id}"
                        ));
                    };
                    if task.status.is_terminal() {
                        continue;
                    }
                    let Some(run_index) = task
                        .headless_owner_runs
                        .iter()
                        .position(|owner_run| owner_run.owner_run_id == *owner_run_id)
                    else {
                        return Err(format!(
                            "headless owner run state recorded before request: {owner_run_id}"
                        ));
                    };
                    validate_headless_owner_state_envelope(
                        event,
                        &task.headless_owner_runs[run_index],
                    )?;
                    let owner_run = &mut task.headless_owner_runs[run_index];
                    owner_run.state = *state;
                    owner_run.heartbeat_at_ms = *heartbeat_at_ms;
                    owner_run.cancellation_reason = cancellation_reason.clone();
                    owner_run.waiting_reason = waiting_reason.clone();
                    owner_run.evidence_refs = evidence_refs.clone();
                    task.updated_at_ms = heartbeat_at_ms.unwrap_or(event.created_at_ms);
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
                LoopRuntimeEvent::SubagentFileIoRecorded { task_id, .. } => {
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
                LoopRuntimeEvent::UsageLedgerRecorded { task_id, usage } => {
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
                    task.latest_usage_ledger = Some(usage.clone());
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

fn validate_headless_owner_request_envelope(
    event: &LoopEventEnvelope,
    owner_run: &HeadlessOwnerRun,
) -> Result<(), String> {
    if event.lease_id.as_deref() != Some(owner_run.lease_id.as_str()) {
        return Err(format!(
            "headless owner run lease_id mismatch: envelope {:?}, owner run {}",
            event.lease_id, owner_run.lease_id
        ));
    }
    if event.attempt != Some(owner_run.attempt) {
        return Err(format!(
            "headless owner run attempt mismatch: envelope {:?}, owner run {}",
            event.attempt, owner_run.attempt
        ));
    }
    if event.idempotency_key.as_deref() != Some(owner_run.idempotency_key.as_str()) {
        return Err(format!(
            "headless owner run idempotency mismatch: envelope {:?}, owner run {}",
            event.idempotency_key, owner_run.idempotency_key
        ));
    }
    Ok(())
}

fn validate_headless_owner_state_envelope(
    event: &LoopEventEnvelope,
    owner_run: &HeadlessOwnerRun,
) -> Result<(), String> {
    if event.lease_id.as_deref() != Some(owner_run.lease_id.as_str()) {
        return Err(format!(
            "headless owner run lease_id mismatch: envelope {:?}, owner run {}",
            event.lease_id, owner_run.lease_id
        ));
    }
    if event.attempt != Some(owner_run.attempt) {
        return Err(format!(
            "headless owner run attempt mismatch: envelope {:?}, owner run {}",
            event.attempt, owner_run.attempt
        ));
    }
    Ok(())
}

fn headless_owner_request_fields_match(
    existing: &HeadlessOwnerRun,
    requested: &HeadlessOwnerRun,
) -> bool {
    existing.task_id == requested.task_id
        && existing.session_id == requested.session_id
        && existing.lease_id == requested.lease_id
        && existing.attempt == requested.attempt
        && existing.snapshot_source == requested.snapshot_source
        && existing.snapshot_ref == requested.snapshot_ref
        && existing.human_gate_id == requested.human_gate_id
        && existing.policy_decision_id == requested.policy_decision_id
        && existing.budget_snapshot_id == requested.budget_snapshot_id
        && existing.idempotency_key == requested.idempotency_key
        && existing.correlation_id == requested.correlation_id
        && existing.causation_id == requested.causation_id
        && existing.requested_by == requested.requested_by
        && existing.executor_kind == requested.executor_kind
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
        BudgetSnapshot, HeadlessOwnerRun, HeadlessOwnerRunState, HeadlessResumeApproval,
        HeadlessResumeMode, LoopActionIntent, LoopActor, LoopCompletionResult,
        LoopCompletionStatus, LoopEventEnvelope, LoopEventJournal, LoopReviewStatus,
        LoopRuntimeEvent, LoopTaskLease, LoopTaskProjection, LoopTaskProjectionStore,
        LoopTaskRecord, LoopTaskStatus, PolicyDecisionRecord, LOOP_RUNTIME_SCHEMA_VERSION,
    };
    use serde_json::json;

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
    fn task_requeued_projects_waiting_task_back_to_pending() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "first");
        let waiting = event_for_test(
            "loop-1",
            2,
            LoopRuntimeEvent::TaskWaitingForInput {
                task_id: "loop-1".to_string(),
                reason: "needs human decision".to_string(),
                waiting_at_ms: 3,
            },
        );
        let mut requeued = event_for_test(
            "loop-1",
            3,
            LoopRuntimeEvent::TaskRequeued {
                task_id: "loop-1".to_string(),
                reason: "operator requested safe retry".to_string(),
                requeued_at_ms: 5,
            },
        );
        requeued.created_at_ms = 6;

        let projection = LoopTaskProjection::from_events(&[created, waiting, requeued.clone()])
            .expect("projection replays task requeue");

        assert_eq!(projection.tasks[0].status, LoopTaskStatus::Pending);
        assert_eq!(projection.tasks[0].lease, None);
        assert!(projection.tasks[0].outcome.is_none());
        assert!(projection.tasks[0].completion_result.is_none());
        assert_eq!(projection.tasks[0].updated_at_ms, 5);
        assert_eq!(
            projection.tasks[0].latest_event_id,
            Some(requeued.event_id.clone())
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
    fn headless_owner_run_request_replays_serializes_kind_and_deduplicates_request_facts() {
        let created = LoopEventEnvelope::task_created_for_test("loop-owner", "headless owner");
        let owner_run = owner_run_for_test("owner-run-1", "loop-owner", "owner-idem-1");
        let requested = headless_owner_run_requested_event_for_test(2, &owner_run);
        let mut same_run_duplicate = requested.clone();
        same_run_duplicate.sequence = 3;
        same_run_duplicate.event_id = "event-loop-owner-owner-run-1-requested-duplicate".into();
        let mut regenerated_owner_run =
            owner_run_for_test("owner-run-regenerated", "loop-owner", "owner-idem-1");
        regenerated_owner_run.requested_at_ms = 5_000;
        regenerated_owner_run.expires_at_ms = 9_000;
        let same_idempotency_duplicate =
            headless_owner_run_requested_event_for_test(4, &regenerated_owner_run);

        let projection = LoopTaskProjection::from_events(&[
            created,
            requested.clone(),
            same_run_duplicate,
            same_idempotency_duplicate,
        ])
        .unwrap();

        assert_eq!(requested.event.kind(), "headless_owner_run_requested");
        assert_eq!(
            serde_json::to_value(&requested.event).unwrap()["type"],
            json!("headless_owner_run_requested")
        );
        assert_eq!(projection.tasks[0].headless_owner_runs.len(), 1);
        assert_eq!(
            projection.tasks[0].headless_owner_runs[0].owner_run_id,
            "owner-run-1"
        );
        assert_eq!(
            projection.tasks[0].headless_owner_runs[0].idempotency_key,
            "owner-idem-1"
        );
        assert_eq!(
            projection.tasks[0].headless_owner_runs[0].requested_at_ms,
            owner_run.requested_at_ms
        );
        assert_eq!(
            projection.tasks[0].headless_owner_runs[0].expires_at_ms,
            owner_run.expires_at_ms
        );
    }

    #[test]
    fn headless_owner_run_state_events_update_existing_run_without_duplication() {
        let created = LoopEventEnvelope::task_created_for_test("loop-owner", "headless owner");
        let owner_run = owner_run_for_test("owner-run-1", "loop-owner", "owner-idem-1");
        let requested = headless_owner_run_requested_event_for_test(2, &owner_run);
        let lease_acquired = headless_owner_run_state_event_for_test(
            3,
            "loop-owner",
            "owner-run-1",
            HeadlessOwnerRunState::LeaseAcquired,
            Some(1_200),
            None,
            None,
            vec!["lease-evidence".to_string()],
        );
        let waiting = headless_owner_run_state_event_for_test(
            4,
            "loop-owner",
            "owner-run-1",
            HeadlessOwnerRunState::WaitingForInput,
            Some(1_300),
            None,
            Some("owner requires input".to_string()),
            vec!["waiting-evidence".to_string()],
        );
        let interrupted = headless_owner_run_state_event_for_test(
            5,
            "loop-owner",
            "owner-run-1",
            HeadlessOwnerRunState::Interrupted,
            Some(1_400),
            Some("desktop interrupted".to_string()),
            Some("owner requires input".to_string()),
            vec!["interrupted-evidence".to_string()],
        );

        let projection = LoopTaskProjection::from_events(&[
            created,
            requested,
            lease_acquired,
            waiting,
            interrupted.clone(),
        ])
        .unwrap();

        assert_eq!(
            interrupted.event.kind(),
            "headless_owner_run_state_recorded"
        );
        assert_eq!(projection.tasks[0].headless_owner_runs.len(), 1);
        let projected = &projection.tasks[0].headless_owner_runs[0];
        assert_eq!(projected.state, HeadlessOwnerRunState::Interrupted);
        assert_eq!(projected.heartbeat_at_ms, Some(1_400));
        assert_eq!(
            projected.cancellation_reason.as_deref(),
            Some("desktop interrupted")
        );
        assert_eq!(
            projected.waiting_reason.as_deref(),
            Some("owner requires input")
        );
        assert_eq!(projected.evidence_refs, vec!["interrupted-evidence"]);
    }

    #[test]
    fn headless_owner_run_projection_rejects_envelope_mismatches_and_state_before_request() {
        let created = LoopEventEnvelope::task_created_for_test("loop-owner", "headless owner");
        let owner_run = owner_run_for_test("owner-run-1", "loop-owner", "owner-idem-1");

        let mut mismatched_lease = headless_owner_run_requested_event_for_test(2, &owner_run);
        mismatched_lease.lease_id = Some("lease-other".to_string());
        let error =
            LoopTaskProjection::from_events(&[created.clone(), mismatched_lease]).unwrap_err();
        assert!(error.contains("headless owner run lease_id mismatch"));

        let mut mismatched_attempt = headless_owner_run_requested_event_for_test(2, &owner_run);
        mismatched_attempt.attempt = Some(99);
        let error =
            LoopTaskProjection::from_events(&[created.clone(), mismatched_attempt]).unwrap_err();
        assert!(error.contains("headless owner run attempt mismatch"));

        let mut mismatched_idempotency = headless_owner_run_requested_event_for_test(2, &owner_run);
        mismatched_idempotency.idempotency_key = Some("owner-idem-other".to_string());
        let error = LoopTaskProjection::from_events(&[created.clone(), mismatched_idempotency])
            .unwrap_err();
        assert!(error.contains("headless owner run idempotency mismatch"));

        let state_before_request = headless_owner_run_state_event_for_test(
            2,
            "loop-owner",
            "owner-run-1",
            HeadlessOwnerRunState::Expired,
            None,
            Some("lease expired".to_string()),
            None,
            Vec::new(),
        );
        let error = LoopTaskProjection::from_events(&[created, state_before_request]).unwrap_err();
        assert!(error.contains("headless owner run state recorded before request"));
    }

    #[test]
    fn old_loop_task_record_json_defaults_headless_owner_runs_to_empty() {
        let task: LoopTaskRecord = serde_json::from_value(json!({
            "id": "loop-old",
            "goal": "old projection",
            "status": "pending",
            "owner": { "kind": "gateway" },
            "policy": {
                "mode": "background_task",
                "allow_workspace_reads": true,
                "allow_test_and_doc_edits": true,
                "allow_runtime_edits": false,
                "allow_dependency_install": false,
                "allow_commit": false,
                "allow_push": false,
                "allow_destructive_filesystem": false,
                "allow_service_lifecycle": false
            },
            "budget": {
                "max_model_rounds": 40,
                "max_tool_calls": 120,
                "max_elapsed_ms": 7200000
            },
            "completion_contract": {
                "required_checks": [],
                "require_docs": false,
                "require_commit": false,
                "require_review_decision": false,
                "stop_on_budget_exceeded": true
            },
            "created_at_ms": 1,
            "updated_at_ms": 1
        }))
        .unwrap();

        assert!(task.headless_owner_runs.is_empty());
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
            review_status: LoopReviewStatus::Blocked,
            commit_eligible: false,
            commit_blockers: vec!["missing_required_check:test".to_string()],
            human_gate_id: None,
            last_review_decision: None,
            eligibility_facts: crate::loop_runtime::LoopCompletionEligibilityFacts::default(),
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

    fn owner_run_for_test(
        owner_run_id: &str,
        task_id: &str,
        idempotency_key: &str,
    ) -> HeadlessOwnerRun {
        HeadlessOwnerRun {
            owner_run_id: owner_run_id.to_string(),
            task_id: task_id.to_string(),
            session_id: Some("session-owner".to_string()),
            lease_id: "lease-owner-1".to_string(),
            attempt: 1,
            state: HeadlessOwnerRunState::Requested,
            snapshot_source:
                crate::loop_runtime::HeadlessOwnerSnapshotSource::PersistedSessionSnapshot,
            snapshot_ref: Some("snapshot-owner-1".to_string()),
            human_gate_id: "gate-owner-1".to_string(),
            policy_decision_id: "policy-owner-1".to_string(),
            budget_snapshot_id: "budget-owner-1".to_string(),
            idempotency_key: idempotency_key.to_string(),
            correlation_id: "corr-owner-1".to_string(),
            causation_id: Some("cause-owner-1".to_string()),
            requested_by: "controller".to_string(),
            requested_at_ms: 1_000,
            heartbeat_at_ms: None,
            expires_at_ms: 2_000,
            cancellation_reason: None,
            waiting_reason: None,
            executor_kind: crate::loop_runtime::HeadlessOwnerExecutorKind::DryRun,
            evidence_refs: vec!["request-evidence".to_string()],
        }
    }

    fn headless_owner_run_requested_event_for_test(
        sequence: u64,
        owner_run: &HeadlessOwnerRun,
    ) -> LoopEventEnvelope {
        serde_json::from_value(json!({
            "schema_version": LOOP_RUNTIME_SCHEMA_VERSION,
            "event_id": format!("event-{}-{}-requested-{sequence}", owner_run.task_id, owner_run.owner_run_id),
            "task_id": owner_run.task_id,
            "sequence": sequence,
            "event": {
                "type": "headless_owner_run_requested",
                "task_id": owner_run.task_id,
                "owner_run": owner_run
            },
            "actor": { "kind": "gateway" },
            "lease_id": owner_run.lease_id,
            "attempt": owner_run.attempt,
            "correlation_id": owner_run.correlation_id,
            "causation_id": owner_run.causation_id,
            "idempotency_key": owner_run.idempotency_key,
            "created_at_ms": owner_run.requested_at_ms
        }))
        .unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    fn headless_owner_run_state_event_for_test(
        sequence: u64,
        task_id: &str,
        owner_run_id: &str,
        state: HeadlessOwnerRunState,
        heartbeat_at_ms: Option<u64>,
        cancellation_reason: Option<String>,
        waiting_reason: Option<String>,
        evidence_refs: Vec<String>,
    ) -> LoopEventEnvelope {
        serde_json::from_value(json!({
            "schema_version": LOOP_RUNTIME_SCHEMA_VERSION,
            "event_id": format!("event-{task_id}-{owner_run_id}-state-{sequence}"),
            "task_id": task_id,
            "sequence": sequence,
            "event": {
                "type": "headless_owner_run_state_recorded",
                "task_id": task_id,
                "owner_run_id": owner_run_id,
                "state": state,
                "heartbeat_at_ms": heartbeat_at_ms,
                "cancellation_reason": cancellation_reason,
                "waiting_reason": waiting_reason,
                "evidence_refs": evidence_refs
            },
            "actor": { "kind": "gateway" },
            "lease_id": "lease-owner-1",
            "attempt": 1,
            "created_at_ms": sequence
        }))
        .unwrap()
    }
}
