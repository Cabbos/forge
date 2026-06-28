use crate::loop_runtime::budget::{BudgetSnapshot, LoopUsageLedger};
use crate::loop_runtime::gates::{HumanGateDecision, HumanGateRecord, HumanGateType};
use crate::loop_runtime::headless::{
    HeadlessOwnerRun, HeadlessOwnerRunState, HeadlessResumeApproval, HeadlessResumeMode,
};
use crate::loop_runtime::policy::LoopActionIntent;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub const LOOP_RUNTIME_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopEventEnvelope {
    pub schema_version: u32,
    pub event_id: String,
    pub task_id: String,
    pub sequence: u64,
    pub event: LoopRuntimeEvent,
    pub actor: LoopActor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    pub created_at_ms: u64,
}

impl LoopEventEnvelope {
    pub fn task_created(
        task: LoopTaskRecord,
        correlation_id: Option<String>,
        idempotency_key: Option<String>,
    ) -> Self {
        Self {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: new_loop_event_id(),
            task_id: task.id.clone(),
            sequence: 0,
            event: LoopRuntimeEvent::TaskCreated { task },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id,
            causation_id: None,
            idempotency_key,
            created_at_ms: now_millis(),
        }
    }

    pub fn task_canceled(
        task_id: String,
        reason: Option<String>,
        correlation_id: Option<String>,
        idempotency_key: Option<String>,
    ) -> Self {
        Self {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: new_loop_event_id(),
            task_id: task_id.clone(),
            sequence: 0,
            event: LoopRuntimeEvent::TaskCanceled {
                task_id,
                reason,
                canceled_at_ms: now_millis(),
            },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id,
            causation_id: None,
            idempotency_key,
            created_at_ms: now_millis(),
        }
    }

    pub fn human_gate_requested(
        task_id: String,
        gate_id: String,
        gate_type: HumanGateType,
        prompt: String,
        correlation_id: Option<String>,
        idempotency_key: Option<String>,
    ) -> Self {
        let gate = HumanGateRecord::new(gate_id, gate_type, prompt);
        Self {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: new_loop_event_id(),
            task_id,
            sequence: 0,
            event: LoopRuntimeEvent::HumanGateRequested { gate },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id,
            causation_id: None,
            idempotency_key,
            created_at_ms: now_millis(),
        }
    }

    pub fn human_gate_resolved(
        task_id: String,
        gate_id: String,
        decision: HumanGateDecision,
        correlation_id: Option<String>,
        idempotency_key: Option<String>,
    ) -> Self {
        Self {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: new_loop_event_id(),
            task_id,
            sequence: 0,
            event: LoopRuntimeEvent::HumanGateResolved {
                gate_id,
                resolved_at_ms: decision.decided_at_ms,
                decision,
            },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id,
            causation_id: None,
            idempotency_key,
            created_at_ms: now_millis(),
        }
    }

    pub fn evidence_recorded(
        task_id: String,
        evidence: EvidenceRecord,
        correlation_id: Option<String>,
        idempotency_key: Option<String>,
    ) -> Self {
        Self {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: new_loop_event_id(),
            task_id: task_id.clone(),
            sequence: 0,
            event: LoopRuntimeEvent::EvidenceRecorded { task_id, evidence },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id,
            causation_id: None,
            idempotency_key,
            created_at_ms: now_millis(),
        }
    }

    pub fn headless_resume_approval_recorded(
        task_id: String,
        approval: HeadlessResumeApproval,
        correlation_id: Option<String>,
        idempotency_key: Option<String>,
    ) -> Self {
        Self {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: new_loop_event_id(),
            task_id: task_id.clone(),
            sequence: 0,
            event: LoopRuntimeEvent::HeadlessResumeApprovalRecorded { task_id, approval },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id,
            causation_id: None,
            idempotency_key,
            created_at_ms: now_millis(),
        }
    }

    pub fn headless_owner_run_requested(owner_run: HeadlessOwnerRun) -> Self {
        let task_id = owner_run.task_id.clone();
        let lease_id = owner_run.lease_id.clone();
        let attempt = owner_run.attempt;
        let correlation_id = owner_run.correlation_id.clone();
        let causation_id = owner_run.causation_id.clone();
        let idempotency_key = owner_run.idempotency_key.clone();
        let requested_at_ms = owner_run.requested_at_ms;
        Self {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: new_loop_event_id(),
            task_id: task_id.clone(),
            event: LoopRuntimeEvent::HeadlessOwnerRunRequested { task_id, owner_run },
            sequence: 0,
            actor: LoopActor::Gateway,
            lease_id: Some(lease_id),
            attempt: Some(attempt),
            correlation_id: Some(correlation_id),
            causation_id,
            idempotency_key: Some(idempotency_key),
            created_at_ms: requested_at_ms,
        }
    }

    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    #[cfg(test)]
    pub fn task_created_for_test(task_id: &str, goal: &str) -> Self {
        Self {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: format!("event-{task_id}-created"),
            task_id: task_id.to_string(),
            sequence: 1,
            event: LoopRuntimeEvent::TaskCreated {
                task: LoopTaskRecord::new_for_test(task_id, goal),
            },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id: None,
            causation_id: None,
            idempotency_key: None,
            created_at_ms: 1,
        }
    }

    #[cfg(test)]
    pub fn task_canceled_for_test(task_id: &str, reason: &str) -> Self {
        Self {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: format!("event-{task_id}-canceled-{reason}"),
            task_id: task_id.to_string(),
            sequence: 0,
            event: LoopRuntimeEvent::TaskCanceled {
                task_id: task_id.to_string(),
                reason: Some(reason.to_string()),
                canceled_at_ms: 2,
            },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id: None,
            causation_id: None,
            idempotency_key: None,
            created_at_ms: 2,
        }
    }

    #[cfg(test)]
    pub fn human_gate_requested_for_test(
        task_id: &str,
        gate_id: &str,
        gate_type: HumanGateType,
        prompt: &str,
    ) -> Self {
        Self {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: format!("event-{task_id}-gate-{gate_id}-requested"),
            task_id: task_id.to_string(),
            sequence: 2,
            event: LoopRuntimeEvent::HumanGateRequested {
                gate: HumanGateRecord {
                    gate_id: gate_id.to_string(),
                    gate_type,
                    prompt: prompt.to_string(),
                    status: crate::loop_runtime::HumanGateStatus::Open,
                    requested_at_ms: 2,
                    decision: None,
                },
            },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id: None,
            causation_id: None,
            idempotency_key: None,
            created_at_ms: 2,
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoopRuntimeEvent {
    TaskCreated {
        task: LoopTaskRecord,
    },
    TaskStarted {
        task_id: String,
        lease: LoopTaskLease,
    },
    TaskWaitingForInput {
        task_id: String,
        reason: String,
        waiting_at_ms: u64,
    },
    TaskInterrupted {
        task_id: String,
        reason: String,
    },
    TaskCanceled {
        task_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        canceled_at_ms: u64,
    },
    HumanGateRequested {
        gate: HumanGateRecord,
    },
    HumanGateResolved {
        gate_id: String,
        decision: HumanGateDecision,
        resolved_at_ms: u64,
    },
    EvidenceRecorded {
        task_id: String,
        evidence: EvidenceRecord,
    },
    HeadlessResumeApprovalRecorded {
        task_id: String,
        approval: HeadlessResumeApproval,
    },
    HeadlessOwnerRunRequested {
        task_id: String,
        owner_run: HeadlessOwnerRun,
    },
    HeadlessOwnerRunStateRecorded {
        task_id: String,
        owner_run_id: String,
        state: HeadlessOwnerRunState,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        heartbeat_at_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cancellation_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        waiting_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        evidence_refs: Vec<String>,
    },
    PolicyDecisionRecorded {
        task_id: String,
        decision: PolicyDecisionRecord,
    },
    BudgetSnapshotRecorded {
        task_id: String,
        snapshot: BudgetSnapshot,
    },
    SubagentFileIoRecorded {
        task_id: String,
        child_task_id: String,
        path: String,
        operation: String,
    },
    UsageLedgerRecorded {
        task_id: String,
        usage: LoopUsageLedger,
    },
    CompletionEvaluated {
        task_id: String,
        result: LoopCompletionResult,
    },
}

impl LoopRuntimeEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::TaskCreated { .. } => "task_created",
            Self::TaskStarted { .. } => "task_started",
            Self::TaskWaitingForInput { .. } => "task_waiting_for_input",
            Self::TaskInterrupted { .. } => "task_interrupted",
            Self::TaskCanceled { .. } => "task_canceled",
            Self::HumanGateRequested { .. } => "human_gate_requested",
            Self::HumanGateResolved { .. } => "human_gate_resolved",
            Self::EvidenceRecorded { .. } => "evidence_recorded",
            Self::HeadlessResumeApprovalRecorded { .. } => "headless_resume_approval_recorded",
            Self::HeadlessOwnerRunRequested { .. } => "headless_owner_run_requested",
            Self::HeadlessOwnerRunStateRecorded { .. } => "headless_owner_run_state_recorded",
            Self::PolicyDecisionRecorded { .. } => "policy_decision_recorded",
            Self::BudgetSnapshotRecorded { .. } => "budget_snapshot_recorded",
            Self::SubagentFileIoRecorded { .. } => "subagent_file_io_recorded",
            Self::UsageLedgerRecorded { .. } => "usage_ledger_recorded",
            Self::CompletionEvaluated { .. } => "completion_evaluated",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoopActor {
    Gateway,
    Desktop,
    Runner { runner_id: String },
    User { source: String },
    Subagent { a2a_task_id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopTaskRecord {
    pub id: String,
    pub goal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
    pub status: LoopTaskStatus,
    pub owner: LoopTaskOwner,
    pub policy: LoopPolicy,
    #[serde(default)]
    pub headless_resume_mode: HeadlessResumeMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headless_resume_approval: Option<HeadlessResumeApproval>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headless_owner_runs: Vec<HeadlessOwnerRun>,
    pub budget: LoopBudget,
    pub completion_contract: LoopCompletionContract,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease: Option<LoopTaskLease>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_gates: Vec<HumanGateRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_decisions: Vec<PolicyDecisionRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_budget_snapshot: Option<BudgetSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_event_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<LoopTaskOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_result: Option<LoopCompletionResult>,
}

impl LoopTaskRecord {
    pub fn new(
        goal: String,
        session_id: Option<String>,
        profile_id: Option<String>,
        workspace_path: Option<String>,
        policy: Option<LoopPolicy>,
        budget: Option<LoopBudget>,
        completion_contract: Option<LoopCompletionContract>,
    ) -> Self {
        let now = now_millis();
        Self {
            id: new_loop_task_id(),
            goal,
            session_id,
            profile_id,
            workspace_path,
            status: LoopTaskStatus::Pending,
            owner: LoopTaskOwner::Gateway,
            policy: policy.unwrap_or_else(LoopPolicy::default_for_background_task),
            headless_resume_mode: HeadlessResumeMode::Disabled,
            headless_resume_approval: None,
            headless_owner_runs: Vec::new(),
            budget: budget.unwrap_or_else(LoopBudget::default_for_background_task),
            completion_contract: completion_contract
                .unwrap_or_else(LoopCompletionContract::default_for_background_task),
            created_at_ms: now,
            updated_at_ms: now,
            lease: None,
            open_gates: Vec::new(),
            evidence: Vec::new(),
            policy_decisions: Vec::new(),
            latest_budget_snapshot: None,
            latest_event_id: None,
            outcome: None,
            completion_result: None,
        }
    }

    #[cfg(test)]
    pub fn new_for_test(task_id: &str, goal: &str) -> Self {
        Self {
            id: task_id.to_string(),
            goal: goal.to_string(),
            session_id: None,
            profile_id: None,
            workspace_path: None,
            status: LoopTaskStatus::Pending,
            owner: LoopTaskOwner::Gateway,
            policy: LoopPolicy::default_for_background_task(),
            headless_resume_mode: HeadlessResumeMode::Disabled,
            headless_resume_approval: None,
            headless_owner_runs: Vec::new(),
            budget: LoopBudget::default_for_background_task(),
            completion_contract: LoopCompletionContract::default_for_background_task(),
            created_at_ms: 1,
            updated_at_ms: 1,
            lease: None,
            open_gates: Vec::new(),
            evidence: Vec::new(),
            policy_decisions: Vec::new(),
            latest_budget_snapshot: None,
            latest_event_id: None,
            outcome: None,
            completion_result: None,
        }
    }

    #[cfg(test)]
    pub fn with_completion_contract(mut self, contract: LoopCompletionContract) -> Self {
        self.completion_contract = contract;
        self
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoopTaskStatus {
    Pending,
    Running,
    WaitingForReview,
    WaitingForInput,
    Completed,
    Failed,
    Canceled,
    Interrupted,
}

impl LoopTaskStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyDecisionRecord {
    pub decision_id: String,
    pub intent: LoopActionIntent,
    pub allowed: bool,
    pub reason: String,
    pub actor: LoopActor,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoopTaskOwner {
    Gateway,
    Session { session_id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopPolicy {
    pub mode: String,
    pub allow_workspace_reads: bool,
    pub allow_test_and_doc_edits: bool,
    pub allow_runtime_edits: bool,
    pub allow_dependency_install: bool,
    pub allow_commit: bool,
    pub allow_push: bool,
    pub allow_destructive_filesystem: bool,
    pub allow_service_lifecycle: bool,
}

impl LoopPolicy {
    pub fn default_for_background_task() -> Self {
        Self {
            mode: "background_task".to_string(),
            allow_workspace_reads: true,
            allow_test_and_doc_edits: true,
            allow_runtime_edits: false,
            allow_dependency_install: false,
            allow_commit: false,
            allow_push: false,
            allow_destructive_filesystem: false,
            allow_service_lifecycle: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopBudget {
    pub max_model_rounds: u32,
    pub max_tool_calls: u32,
    pub max_elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_estimated_cost_micros: Option<u64>,
}

impl LoopBudget {
    pub fn default_for_background_task() -> Self {
        Self {
            max_model_rounds: 40,
            max_tool_calls: 120,
            max_elapsed_ms: 7_200_000,
            max_estimated_cost_micros: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopCompletionContract {
    #[serde(default)]
    pub required_checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_gitnexus_risk: Option<String>,
    pub require_docs: bool,
    pub require_commit: bool,
    pub require_review_decision: bool,
    pub stop_on_budget_exceeded: bool,
}

impl LoopCompletionContract {
    pub fn default_for_background_task() -> Self {
        Self {
            required_checks: Vec::new(),
            max_gitnexus_risk: None,
            require_docs: false,
            require_commit: false,
            require_review_decision: false,
            stop_on_budget_exceeded: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EvidenceRecord {
    Command {
        evidence_id: String,
        check_name: String,
        command: String,
        exit_code: i32,
        success: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        artifact_hash: Option<String>,
    },
    GitNexus {
        evidence_id: String,
        risk: String,
        changed_symbols: u32,
        affected_processes: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        report_hash: Option<String>,
    },
    Commit {
        evidence_id: String,
        commit_sha: String,
        summary: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        human_gate_id: Option<String>,
    },
    Docs {
        evidence_id: String,
        paths: Vec<String>,
    },
    Review {
        evidence_id: String,
        gate_id: String,
        decision: HumanGateDecision,
    },
    Budget {
        evidence_id: String,
        budget_exceeded: bool,
    },
}

#[cfg(test)]
impl EvidenceRecord {
    pub fn command_for_test(check_name: &str, success: bool) -> Self {
        Self::Command {
            evidence_id: format!("evidence-command-{check_name}"),
            check_name: check_name.to_string(),
            command: check_name.to_string(),
            exit_code: if success { 0 } else { 1 },
            success,
            artifact_hash: None,
        }
    }

    pub fn gitnexus_for_test(risk: &str) -> Self {
        Self::GitNexus {
            evidence_id: format!("evidence-gitnexus-{risk}"),
            risk: risk.to_string(),
            changed_symbols: 1,
            affected_processes: 0,
            report_hash: None,
        }
    }

    pub fn docs_for_test(paths: Vec<&str>) -> Self {
        Self::Docs {
            evidence_id: "evidence-docs".to_string(),
            paths: paths.into_iter().map(str::to_string).collect(),
        }
    }

    pub fn commit_for_test(commit_sha: &str) -> Self {
        Self::Commit {
            evidence_id: "evidence-commit".to_string(),
            commit_sha: commit_sha.to_string(),
            summary: "test commit".to_string(),
            human_gate_id: None,
        }
    }

    pub fn budget_for_test(budget_exceeded: bool) -> Self {
        Self::Budget {
            evidence_id: "evidence-budget".to_string(),
            budget_exceeded,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoopCompletionStatus {
    Complete,
    Blocked,
    WaitingForReview,
    FailedBudget,
    FailedRisk,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoopReviewStatus {
    #[default]
    NotRequired,
    ReadyForReview,
    Approved,
    Rejected,
    Blocked,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopCompletionResult {
    pub status: LoopCompletionStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub review_status: LoopReviewStatus,
    #[serde(default)]
    pub commit_eligible: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commit_blockers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human_gate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_review_decision: Option<HumanGateDecision>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopTaskLease {
    #[serde(default, alias = "owner_id")]
    pub lease_id: String,
    #[serde(default)]
    pub owner_pid: u32,
    #[serde(default, alias = "claimed_at_ms")]
    pub acquired_at_ms: u64,
    pub expires_at_ms: u64,
    #[serde(default)]
    pub heartbeat_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopTaskOutcome {
    pub status: LoopTaskStatus,
    pub message: String,
    pub completed_at_ms: u64,
}

pub fn new_loop_task_id() -> String {
    format!("loop-{}", uuid::Uuid::now_v7().simple())
}

pub fn new_loop_event_id() -> String {
    format!("evt-{}", uuid::Uuid::now_v7().simple())
}

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use crate::loop_runtime::{
        HeadlessResumeMode, LoopActor, LoopEventEnvelope, LoopRuntimeEvent, LoopTaskRecord,
        LOOP_RUNTIME_SCHEMA_VERSION,
    };

    #[test]
    fn old_minimal_loop_event_envelope_json_without_runtime_metadata_deserializes() {
        let json = serde_json::json!({
            "schema_version": LOOP_RUNTIME_SCHEMA_VERSION,
            "event_id": "evt-old",
            "task_id": "loop-old",
            "sequence": 1,
            "event": {
                "type": "task_created",
                "task": LoopTaskRecord::new_for_test("loop-old", "old task")
            },
            "actor": { "kind": "gateway" },
            "created_at_ms": 1
        });

        let envelope: LoopEventEnvelope = serde_json::from_value(json).unwrap();

        assert_eq!(envelope.task_id, "loop-old");
        assert_eq!(envelope.lease_id, None);
        assert_eq!(envelope.attempt, None);
        assert_eq!(envelope.causation_id, None);
        assert!(matches!(envelope.actor, LoopActor::Gateway));
        assert!(matches!(
            envelope.event,
            LoopRuntimeEvent::TaskCreated { .. }
        ));
    }

    #[test]
    fn old_loop_task_record_json_without_headless_fields_defaults_to_disabled() {
        let mut value =
            serde_json::to_value(LoopTaskRecord::new_for_test("loop-old", "old task")).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("headless_resume_mode");
        object.remove("headless_resume_approval");

        let task: LoopTaskRecord = serde_json::from_value(value).unwrap();

        assert_eq!(task.headless_resume_mode, HeadlessResumeMode::Disabled);
        assert!(task.headless_resume_approval.is_none());
    }
}
