use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeadlessResumeMode {
    #[default]
    Disabled,
    RequireHumanApproval,
    ApprovedForTask,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeadlessOwnerRunState {
    #[default]
    Requested,
    Denied,
    Ready,
    LeaseAcquired,
    DryRunWaiting,
    FakeRunning,
    Running,
    WaitingForInput,
    Interrupted,
    Cancelled,
    Expired,
    Completed,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeadlessOwnerSnapshotSource {
    #[default]
    Unavailable,
    CurrentDesktopSession,
    PersistedSessionSnapshot,
    WorkspaceSnapshot,
    RestoredHeadlessSnapshot,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeadlessOwnerExecutorKind {
    #[default]
    None,
    DryRun,
    FakeExecutor,
    AgentSessionAdapter,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeadlessOwnerRun {
    pub owner_run_id: String,
    pub task_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    pub lease_id: String,
    pub attempt: u32,
    #[serde(default)]
    pub state: HeadlessOwnerRunState,
    #[serde(default)]
    pub snapshot_source: HeadlessOwnerSnapshotSource,
    #[serde(default)]
    pub snapshot_ref: Option<String>,
    pub human_gate_id: String,
    pub policy_decision_id: String,
    pub budget_snapshot_id: String,
    pub idempotency_key: String,
    pub correlation_id: String,
    #[serde(default)]
    pub causation_id: Option<String>,
    pub requested_by: String,
    pub requested_at_ms: u64,
    #[serde(default)]
    pub heartbeat_at_ms: Option<u64>,
    pub expires_at_ms: u64,
    #[serde(default)]
    pub cancellation_reason: Option<String>,
    #[serde(default)]
    pub waiting_reason: Option<String>,
    #[serde(default)]
    pub executor_kind: HeadlessOwnerExecutorKind,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

impl HeadlessOwnerRun {
    pub fn validate_authorization_bundle(&self) -> Result<(), String> {
        for (field_name, value) in [
            ("owner_run_id", self.owner_run_id.as_str()),
            ("task_id", self.task_id.as_str()),
            ("lease_id", self.lease_id.as_str()),
            ("human_gate_id", self.human_gate_id.as_str()),
            ("policy_decision_id", self.policy_decision_id.as_str()),
            ("budget_snapshot_id", self.budget_snapshot_id.as_str()),
            ("idempotency_key", self.idempotency_key.as_str()),
            ("correlation_id", self.correlation_id.as_str()),
            ("requested_by", self.requested_by.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field_name} must not be blank"));
            }
        }

        if self.attempt == 0 {
            return Err("attempt must be greater than zero".to_string());
        }

        if self.expires_at_ms <= self.requested_at_ms {
            return Err("expires_at_ms must be greater than requested_at_ms".to_string());
        }

        Ok(())
    }

    pub fn matches_idempotency_key(&self, task_id: &str, idempotency_key: &str) -> bool {
        self.task_id == task_id && self.idempotency_key == idempotency_key
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeadlessResumeApproval {
    pub task_id: String,
    pub approved_by: String,
    pub approved_at_ms: u64,
    pub scope: String,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeadlessAgentLease {
    pub task_id: String,
    pub session_id: String,
    pub lease_id: String,
    pub owner_pid: u32,
    pub expires_at_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadlessResumeReadiness {
    DesktopOwnerRequired,
    ApprovalRequired,
    ApprovalRecordedLeasePending,
    ApprovalExpired,
}

pub fn derive_headless_resume_readiness(
    mode: HeadlessResumeMode,
    approval: Option<&HeadlessResumeApproval>,
    now_ms: u64,
) -> HeadlessResumeReadiness {
    if let Some(approval) = approval {
        if approval.expires_at_ms <= now_ms {
            return HeadlessResumeReadiness::ApprovalExpired;
        }
        return HeadlessResumeReadiness::ApprovalRecordedLeasePending;
    }

    match mode {
        HeadlessResumeMode::RequireHumanApproval | HeadlessResumeMode::ApprovedForTask => {
            HeadlessResumeReadiness::ApprovalRequired
        }
        HeadlessResumeMode::Disabled => HeadlessResumeReadiness::DesktopOwnerRequired,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        derive_headless_resume_readiness, HeadlessOwnerExecutorKind, HeadlessOwnerRun,
        HeadlessOwnerRunState, HeadlessOwnerSnapshotSource, HeadlessResumeApproval,
        HeadlessResumeMode, HeadlessResumeReadiness,
    };
    use serde_json::json;

    #[test]
    fn headless_resume_readiness_requires_desktop_owner_when_disabled_without_approval() {
        let readiness = derive_headless_resume_readiness(HeadlessResumeMode::Disabled, None, 1_000);

        assert_eq!(readiness, HeadlessResumeReadiness::DesktopOwnerRequired);
    }

    #[test]
    fn headless_resume_readiness_waits_for_approval_when_policy_requires_human_approval() {
        let readiness =
            derive_headless_resume_readiness(HeadlessResumeMode::RequireHumanApproval, None, 1_000);

        assert_eq!(readiness, HeadlessResumeReadiness::ApprovalRequired);
    }

    #[test]
    fn headless_resume_readiness_is_lease_pending_after_unexpired_approval() {
        let approval = approval_for_test(500, 1_500);

        let readiness = derive_headless_resume_readiness(
            HeadlessResumeMode::ApprovedForTask,
            Some(&approval),
            1_000,
        );

        assert_eq!(
            readiness,
            HeadlessResumeReadiness::ApprovalRecordedLeasePending
        );
    }

    #[test]
    fn headless_resume_readiness_is_expired_when_approval_expiry_has_passed() {
        let approval = approval_for_test(500, 1_000);

        let readiness = derive_headless_resume_readiness(
            HeadlessResumeMode::ApprovedForTask,
            Some(&approval),
            1_000,
        );

        assert_eq!(readiness, HeadlessResumeReadiness::ApprovalExpired);
    }

    #[test]
    fn headless_owner_contract_serializes_all_lifecycle_sources_and_executor_kinds() {
        let state_cases = [
            (HeadlessOwnerRunState::Requested, "requested"),
            (HeadlessOwnerRunState::Denied, "denied"),
            (HeadlessOwnerRunState::Ready, "ready"),
            (HeadlessOwnerRunState::LeaseAcquired, "lease_acquired"),
            (HeadlessOwnerRunState::DryRunWaiting, "dry_run_waiting"),
            (HeadlessOwnerRunState::FakeRunning, "fake_running"),
            (HeadlessOwnerRunState::Running, "running"),
            (HeadlessOwnerRunState::WaitingForInput, "waiting_for_input"),
            (HeadlessOwnerRunState::Interrupted, "interrupted"),
            (HeadlessOwnerRunState::Cancelled, "cancelled"),
            (HeadlessOwnerRunState::Expired, "expired"),
            (HeadlessOwnerRunState::Completed, "completed"),
            (HeadlessOwnerRunState::Failed, "failed"),
        ];
        for (state, expected) in state_cases {
            assert_eq!(serde_json::to_value(state).unwrap(), json!(expected));
        }

        let snapshot_source_cases = [
            (HeadlessOwnerSnapshotSource::Unavailable, "unavailable"),
            (
                HeadlessOwnerSnapshotSource::CurrentDesktopSession,
                "current_desktop_session",
            ),
            (
                HeadlessOwnerSnapshotSource::PersistedSessionSnapshot,
                "persisted_session_snapshot",
            ),
            (
                HeadlessOwnerSnapshotSource::WorkspaceSnapshot,
                "workspace_snapshot",
            ),
            (
                HeadlessOwnerSnapshotSource::RestoredHeadlessSnapshot,
                "restored_headless_snapshot",
            ),
        ];
        for (snapshot_source, expected) in snapshot_source_cases {
            assert_eq!(serde_json::to_value(snapshot_source).unwrap(), json!(expected));
        }

        let executor_kind_cases = [
            (HeadlessOwnerExecutorKind::None, "none"),
            (HeadlessOwnerExecutorKind::DryRun, "dry_run"),
            (HeadlessOwnerExecutorKind::FakeExecutor, "fake_executor"),
            (
                HeadlessOwnerExecutorKind::AgentSessionAdapter,
                "agent_session_adapter",
            ),
        ];
        for (executor_kind, expected) in executor_kind_cases {
            assert_eq!(serde_json::to_value(executor_kind).unwrap(), json!(expected));
        }

        let owner_run = owner_run_for_test();
        let json = serde_json::to_value(owner_run).unwrap();
        assert_eq!(json["state"], "waiting_for_input");
        assert_eq!(json["snapshot_source"], "persisted_session_snapshot");
        assert_eq!(json["executor_kind"], "dry_run");
    }

    #[test]
    fn headless_owner_contract_deserializes_backcompat_defaults() {
        let owner_run: HeadlessOwnerRun = serde_json::from_value(json!({
            "owner_run_id": "owner-run-1",
            "task_id": "task-headless",
            "lease_id": "lease-1",
            "attempt": 1,
            "human_gate_id": "gate-1",
            "policy_decision_id": "policy-1",
            "budget_snapshot_id": "budget-1",
            "idempotency_key": "idem-1",
            "correlation_id": "corr-1",
            "requested_by": "controller",
            "requested_at_ms": 1_000,
            "expires_at_ms": 2_000
        }))
        .unwrap();

        assert_eq!(owner_run.state, HeadlessOwnerRunState::Requested);
        assert_eq!(
            owner_run.snapshot_source,
            HeadlessOwnerSnapshotSource::Unavailable
        );
        assert_eq!(owner_run.executor_kind, HeadlessOwnerExecutorKind::None);
        assert_eq!(owner_run.session_id, None);
        assert_eq!(owner_run.snapshot_ref, None);
        assert_eq!(owner_run.causation_id, None);
        assert_eq!(owner_run.heartbeat_at_ms, None);
        assert_eq!(owner_run.cancellation_reason, None);
        assert_eq!(owner_run.waiting_reason, None);
        assert!(owner_run.evidence_refs.is_empty());
    }

    #[test]
    fn headless_owner_contract_rejects_missing_human_gate_policy_budget_or_idempotency() {
        for (field_name, clear_field) in [
            ("human_gate_id", clear_human_gate_id as fn(&mut HeadlessOwnerRun)),
            ("policy_decision_id", clear_policy_decision_id),
            ("budget_snapshot_id", clear_budget_snapshot_id),
            ("idempotency_key", clear_idempotency_key),
        ] {
            let mut owner_run = owner_run_for_test();
            clear_field(&mut owner_run);

            let error = owner_run.validate_authorization_bundle().unwrap_err();

            assert!(
                error.contains(field_name),
                "expected {field_name} validation error, got {error}"
            );
        }
    }

    #[test]
    fn headless_owner_contract_matches_duplicate_task_and_idempotency_key() {
        let owner_run = owner_run_for_test();

        assert!(owner_run.matches_idempotency_key("task-headless", "idem-1"));
        assert!(!owner_run.matches_idempotency_key("task-other", "idem-1"));
        assert!(!owner_run.matches_idempotency_key("task-headless", "idem-other"));
    }

    #[test]
    fn headless_owner_contract_does_not_change_existing_resume_approval_shape() {
        let approval = approval_for_test(1_000, 2_000);

        let json = serde_json::to_value(approval).unwrap();

        assert_eq!(
            json,
            json!({
                "task_id": "task-headless",
                "approved_by": "human-reviewer",
                "approved_at_ms": 1_000,
                "scope": "task",
                "expires_at_ms": 2_000
            })
        );
    }

    fn approval_for_test(approved_at_ms: u64, expires_at_ms: u64) -> HeadlessResumeApproval {
        HeadlessResumeApproval {
            task_id: "task-headless".to_string(),
            approved_by: "human-reviewer".to_string(),
            approved_at_ms,
            scope: "task".to_string(),
            expires_at_ms,
        }
    }

    fn owner_run_for_test() -> HeadlessOwnerRun {
        HeadlessOwnerRun {
            owner_run_id: "owner-run-1".to_string(),
            task_id: "task-headless".to_string(),
            session_id: Some("session-1".to_string()),
            lease_id: "lease-1".to_string(),
            attempt: 1,
            state: HeadlessOwnerRunState::WaitingForInput,
            snapshot_source: HeadlessOwnerSnapshotSource::PersistedSessionSnapshot,
            snapshot_ref: Some("projection-1".to_string()),
            human_gate_id: "gate-1".to_string(),
            policy_decision_id: "policy-1".to_string(),
            budget_snapshot_id: "budget-1".to_string(),
            idempotency_key: "idem-1".to_string(),
            correlation_id: "corr-1".to_string(),
            causation_id: Some("cause-1".to_string()),
            requested_by: "controller".to_string(),
            requested_at_ms: 1_000,
            heartbeat_at_ms: Some(1_100),
            expires_at_ms: 2_000,
            cancellation_reason: None,
            waiting_reason: Some("human approval required".to_string()),
            executor_kind: HeadlessOwnerExecutorKind::DryRun,
            evidence_refs: vec!["evidence-1".to_string()],
        }
    }

    fn clear_human_gate_id(owner_run: &mut HeadlessOwnerRun) {
        owner_run.human_gate_id.clear();
    }

    fn clear_policy_decision_id(owner_run: &mut HeadlessOwnerRun) {
        owner_run.policy_decision_id.clear();
    }

    fn clear_budget_snapshot_id(owner_run: &mut HeadlessOwnerRun) {
        owner_run.budget_snapshot_id.clear();
    }

    fn clear_idempotency_key(owner_run: &mut HeadlessOwnerRun) {
        owner_run.idempotency_key.clear();
    }
}
