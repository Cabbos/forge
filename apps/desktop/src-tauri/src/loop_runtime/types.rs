use crate::loop_runtime::gates::{HumanGateDecision, HumanGateRecord, HumanGateType};
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
    pub correlation_id: Option<String>,
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
            correlation_id,
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
            correlation_id,
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
            correlation_id,
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
            correlation_id,
            idempotency_key,
            created_at_ms: now_millis(),
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
            correlation_id: None,
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
            correlation_id: None,
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
            correlation_id: None,
            idempotency_key: None,
            created_at_ms: 2,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoopRuntimeEvent {
    TaskCreated {
        task: LoopTaskRecord,
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
}

impl LoopRuntimeEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::TaskCreated { .. } => "task_created",
            Self::TaskCanceled { .. } => "task_canceled",
            Self::HumanGateRequested { .. } => "human_gate_requested",
            Self::HumanGateResolved { .. } => "human_gate_resolved",
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
    pub budget: LoopBudget,
    pub completion_contract: LoopCompletionContract,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease: Option<LoopTaskLease>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_gates: Vec<HumanGateRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_event_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<LoopTaskOutcome>,
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
            budget: budget.unwrap_or_else(LoopBudget::default_for_background_task),
            completion_contract: completion_contract
                .unwrap_or_else(LoopCompletionContract::default_for_background_task),
            created_at_ms: now,
            updated_at_ms: now,
            lease: None,
            open_gates: Vec::new(),
            latest_event_id: None,
            outcome: None,
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
            budget: LoopBudget::default_for_background_task(),
            completion_contract: LoopCompletionContract::default_for_background_task(),
            created_at_ms: 1,
            updated_at_ms: 1,
            lease: None,
            open_gates: Vec::new(),
            latest_event_id: None,
            outcome: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoopTaskStatus {
    Pending,
    Running,
    WaitingForReview,
    Completed,
    Failed,
    Canceled,
}

impl LoopTaskStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }
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
