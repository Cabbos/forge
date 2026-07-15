use std::sync::Arc;
use std::time::Duration;

use crate::loop_runtime::headless::{derive_headless_resume_readiness, HeadlessResumeReadiness};
use crate::loop_runtime::types::{new_loop_event_id, now_millis};
use crate::loop_runtime::{
    evaluate_completion, BudgetSnapshot, HeadlessOwnerExecutorKind, HeadlessOwnerRun,
    HeadlessOwnerRunState, HeadlessOwnerSnapshotSource, LoopActionIntent, LoopActor,
    LoopEventEnvelope, LoopEventJournal, LoopRuntimeEvent, LoopTaskLease, LoopTaskProjection,
    LoopTaskProjectionStore, LoopTaskRecord, LoopTaskRecoveryKind, LoopTaskStatus,
    PolicyDecisionRecord, LOOP_RUNTIME_SCHEMA_VERSION,
};

pub const LOOP_RUNNER_POLL_INTERVAL_SECS: u64 = 5;
pub const LOOP_TASK_LEASE_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
pub const DEFAULT_LOOP_RUNNER_ID: &str = "gateway-loop-runner";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopTaskRunner {
    runner_id: String,
    owner_pid: u32,
    lease_timeout_ms: u64,
    #[cfg(test)]
    fake_executor_fixture: Option<FakeOwnerExecutorFixture>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LoopRunnerRunSummary {
    pub claimed_tasks: usize,
    pub interrupted_stale_leases: usize,
    pub waiting_for_input_tasks: usize,
    pub completion_evaluations: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LoopRunnerQueueStats {
    pub pending_loop_tasks: usize,
    pub running_loop_tasks: usize,
    pub stale_loop_task_leases: usize,
    pub orphaned_loop_tasks: usize,
    pub interrupted_loop_tasks: usize,
    pub recoverable_loop_tasks: usize,
    pub dry_run_headless_owner_runs: usize,
    pub waiting_headless_owner_runs: usize,
    pub denied_headless_owner_runs: usize,
    pub expired_headless_owner_runs: usize,
}

impl LoopTaskRunner {
    pub fn gateway_default() -> Self {
        Self {
            runner_id: DEFAULT_LOOP_RUNNER_ID.to_string(),
            owner_pid: std::process::id(),
            lease_timeout_ms: LOOP_TASK_LEASE_TIMEOUT_MS,
            #[cfg(test)]
            fake_executor_fixture: None,
        }
    }

    pub fn run_once(
        &self,
        journal: &LoopEventJournal,
        projection_store: &LoopTaskProjectionStore,
    ) -> Result<LoopRunnerRunSummary, String> {
        self.run_once_at(journal, projection_store, now_millis())
    }

    pub fn run_once_at(
        &self,
        journal: &LoopEventJournal,
        projection_store: &LoopTaskProjectionStore,
        now_ms: u64,
    ) -> Result<LoopRunnerRunSummary, String> {
        let mut summary = LoopRunnerRunSummary::default();
        let projection = projection_store.load_or_rebuild(journal)?;
        let events = journal.load_all()?;

        for task in projection
            .tasks
            .iter()
            .filter(|task| stale_running_task(task, now_ms))
        {
            let Some(lease) = task.lease.as_ref() else {
                continue;
            };
            let attempt = current_attempt_for_task(&events, &task.id).unwrap_or(1);
            let reason = format!(
                "stale lease {} expired at {}",
                lease.lease_id, lease.expires_at_ms
            );
            let event = self.envelope(
                task.id.clone(),
                LoopRuntimeEvent::TaskInterrupted {
                    task_id: task.id.clone(),
                    reason,
                },
                Some(lease.lease_id.clone()),
                Some(attempt),
                task.latest_event_id.clone(),
                Some(format!(
                    "runner:{}:interrupt_stale:{}:{}",
                    self.runner_id, task.id, lease.lease_id
                )),
                now_ms,
            );
            let interrupted = journal.append_idempotent(event)?.event;
            for owner_run in task.headless_owner_runs.iter().filter(|owner_run| {
                owner_run.lease_id == lease.lease_id
                    && nonterminal_headless_owner_state(owner_run.state)
            }) {
                let expired_reason = format!(
                    "stale lease {} expired at {}; coordinator dry run owner lease expired without execution",
                    lease.lease_id, lease.expires_at_ms
                );
                let expired = self.headless_owner_state_envelope(
                    task.id.clone(),
                    owner_run,
                    HeadlessOwnerRunState::Expired,
                    Some(expired_reason),
                    headless_owner_evidence_refs(owner_run),
                    Some(interrupted.event_id.clone()),
                    Some(format!(
                        "runner:{}:owner-dry-run-state:{}:{}:expired",
                        self.runner_id, task.id, owner_run.owner_run_id
                    )),
                    now_ms,
                );
                journal.append_idempotent(expired)?;
            }
            summary.interrupted_stale_leases += 1;
        }

        let projection = projection_store.rebuild_from_journal(journal)?;
        for task in projection
            .tasks
            .iter()
            .filter(|task| task.status == LoopTaskStatus::Pending)
        {
            self.process_pending_task(
                journal,
                projection_store,
                task,
                &events,
                now_ms,
                &mut summary,
            )?;
        }

        if summary.claimed_tasks > 0 || summary.interrupted_stale_leases > 0 {
            projection_store.rebuild_from_journal(journal)?;
        }

        Ok(summary)
    }

    pub fn queue_stats(projection: &LoopTaskProjection, now_ms: u64) -> LoopRunnerQueueStats {
        let mut stats = LoopRunnerQueueStats::default();
        for task in &projection.tasks {
            match task.status {
                LoopTaskStatus::Pending => stats.pending_loop_tasks += 1,
                LoopTaskStatus::Running => {
                    stats.running_loop_tasks += 1;
                    if stale_running_task(task, now_ms) {
                        stats.stale_loop_task_leases += 1;
                    }
                }
                _ => {}
            }
            if let Some(recovery) = task.recovery_state.as_ref() {
                if recovery.recoverable {
                    stats.recoverable_loop_tasks += 1;
                }
                match recovery.kind {
                    LoopTaskRecoveryKind::Orphaned => stats.orphaned_loop_tasks += 1,
                    LoopTaskRecoveryKind::Interrupted => stats.interrupted_loop_tasks += 1,
                }
            }
            for owner_run in &task.headless_owner_runs {
                if owner_run.executor_kind == HeadlessOwnerExecutorKind::DryRun {
                    stats.dry_run_headless_owner_runs += 1;
                }
                match owner_run.state {
                    HeadlessOwnerRunState::WaitingForInput
                    | HeadlessOwnerRunState::DryRunWaiting => {
                        stats.waiting_headless_owner_runs += 1;
                    }
                    HeadlessOwnerRunState::Denied => stats.denied_headless_owner_runs += 1,
                    HeadlessOwnerRunState::Expired => stats.expired_headless_owner_runs += 1,
                    _ => {}
                }
            }
        }
        stats
    }

    fn process_pending_task(
        &self,
        journal: &LoopEventJournal,
        projection_store: &LoopTaskProjectionStore,
        task: &LoopTaskRecord,
        events: &[LoopEventEnvelope],
        now_ms: u64,
        summary: &mut LoopRunnerRunSummary,
    ) -> Result<(), String> {
        let attempt = current_attempt_for_task(events, &task.id)
            .unwrap_or(0)
            .saturating_add(1);
        let start_idempotency_key =
            format!("runner:{}:start:{}:{attempt}", self.runner_id, task.id);
        let existing_start = if let Some(existing) = events
            .iter()
            .find(|event| event.idempotency_key.as_deref() == Some(start_idempotency_key.as_str()))
            .cloned()
        {
            Some(existing)
        } else {
            journal.find_by_idempotency_key(&start_idempotency_key)?
        };
        let start = if let Some(existing) = existing_start {
            existing
        } else {
            let lease = self.lease(now_ms);
            let start = self.envelope(
                task.id.clone(),
                LoopRuntimeEvent::TaskStarted {
                    task_id: task.id.clone(),
                    lease: lease.clone(),
                },
                Some(lease.lease_id.clone()),
                Some(attempt),
                task.latest_event_id.clone(),
                Some(start_idempotency_key),
                now_ms,
            );
            journal.append_idempotent(start)?.event
        };
        let (start_event_id, lease) = match &start.event {
            LoopRuntimeEvent::TaskStarted {
                task_id: started_task_id,
                lease,
            } if started_task_id == &task.id && start.attempt == Some(attempt) => {
                (start.event_id.clone(), lease.clone())
            }
            other => {
                return Err(format!(
                    "runner start idempotency returned unexpected event for task {}: {}",
                    task.id,
                    other.kind()
                ));
            }
        };
        summary.claimed_tasks += 1;

        let readiness = derive_headless_resume_readiness(
            task.headless_resume_mode,
            task.headless_resume_approval.as_ref(),
            now_ms,
        );
        let should_record_dry_run = matches!(
            readiness,
            HeadlessResumeReadiness::ApprovalRecordedLeasePending
        );
        let wait_causation_id = if should_record_dry_run {
            let policy_intent = coordinator_dry_run_policy_intent();
            let policy_decision = task
                .policy
                .decide(policy_intent.clone(), &BudgetSnapshot::empty());
            let policy_record = PolicyDecisionRecord {
                decision_id: format!("policy:{}:{attempt}:owner-dry-run", task.id),
                intent: policy_intent,
                allowed: policy_decision.allowed,
                reason: policy_decision.reason.clone(),
                actor: LoopActor::Runner {
                    runner_id: self.runner_id.clone(),
                },
                created_at_ms: now_ms,
            };
            let policy_event = self.envelope(
                task.id.clone(),
                LoopRuntimeEvent::PolicyDecisionRecorded {
                    task_id: task.id.clone(),
                    decision: policy_record.clone(),
                },
                Some(lease.lease_id.clone()),
                Some(attempt),
                Some(start_event_id.clone()),
                Some(format!(
                    "runner:{}:owner-dry-run-policy:{}:{attempt}",
                    self.runner_id, task.id
                )),
                now_ms,
            );
            let policy_event = journal.append_idempotent(policy_event)?.event;
            let budget_snapshot = task
                .latest_budget_snapshot
                .clone()
                .unwrap_or_else(BudgetSnapshot::empty);
            let budget_event = self.envelope(
                task.id.clone(),
                LoopRuntimeEvent::BudgetSnapshotRecorded {
                    task_id: task.id.clone(),
                    snapshot: budget_snapshot.clone(),
                },
                Some(lease.lease_id.clone()),
                Some(attempt),
                Some(policy_event.event_id.clone()),
                Some(format!(
                    "runner:{}:owner-dry-run-budget:{}:{attempt}",
                    self.runner_id, task.id
                )),
                now_ms,
            );
            let budget_event = journal.append_idempotent(budget_event)?.event;
            let budget_decision = budget_snapshot.decide(&task.budget);
            let human_gate_id = headless_resume_approval_event_id(events, task, attempt);
            let owner_run = self.headless_owner_run(
                task,
                &lease,
                attempt,
                budget_event.event_id.clone(),
                human_gate_id,
                policy_record.decision_id.clone(),
                budget_event.event_id.clone(),
                now_ms,
            );
            let owner_request = self.envelope(
                task.id.clone(),
                LoopRuntimeEvent::HeadlessOwnerRunRequested {
                    task_id: task.id.clone(),
                    owner_run: owner_run.clone(),
                },
                Some(lease.lease_id.clone()),
                Some(attempt),
                Some(budget_event.event_id.clone()),
                Some(owner_run.idempotency_key.clone()),
                now_ms,
            );
            let owner_request = journal.append_idempotent(owner_request)?.event;
            let owner_run = match &owner_request.event {
                LoopRuntimeEvent::HeadlessOwnerRunRequested { owner_run, .. } => owner_run.clone(),
                other => {
                    return Err(format!(
                        "owner run idempotency returned unexpected event for task {}: {}",
                        task.id,
                        other.kind()
                    ));
                }
            };
            let owner_state_created_at_ms = owner_request.created_at_ms;
            let owner_evidence_refs = headless_owner_evidence_refs(&owner_run);
            if !policy_record.allowed {
                let reason = format!(
                    "coordinator dry run denied before execution: {}",
                    policy_record.reason
                );
                let denied = self.headless_owner_state_envelope(
                    task.id.clone(),
                    &owner_run,
                    HeadlessOwnerRunState::Denied,
                    Some(reason),
                    owner_evidence_refs,
                    Some(owner_request.event_id.clone()),
                    Some(self.headless_owner_state_idempotency_key(&task.id, &owner_run, "denied")),
                    owner_state_created_at_ms,
                );
                journal.append_idempotent(denied)?.event.event_id
            } else if !budget_decision.allowed {
                let reason = format!(
                    "coordinator dry run denied before execution: {}",
                    budget_decision.reason
                );
                let denied = self.headless_owner_state_envelope(
                    task.id.clone(),
                    &owner_run,
                    HeadlessOwnerRunState::Denied,
                    Some(reason),
                    owner_evidence_refs,
                    Some(owner_request.event_id.clone()),
                    Some(self.headless_owner_state_idempotency_key(&task.id, &owner_run, "denied")),
                    owner_state_created_at_ms,
                );
                journal.append_idempotent(denied)?.event.event_id
            } else {
                let acquired = self.headless_owner_state_envelope(
                    task.id.clone(),
                    &owner_run,
                    HeadlessOwnerRunState::LeaseAcquired,
                    None,
                    owner_evidence_refs.clone(),
                    Some(owner_request.event_id.clone()),
                    Some(self.headless_owner_state_idempotency_key(
                        &task.id,
                        &owner_run,
                        "lease-acquired",
                    )),
                    owner_state_created_at_ms,
                );
                let acquired = journal.append_idempotent(acquired)?.event;
                #[cfg(test)]
                if let Some(fixture) = self.fake_executor_fixture.as_ref() {
                    self.record_fake_executor_outcome(
                        journal,
                        task,
                        &owner_run,
                        fixture,
                        owner_evidence_refs,
                        acquired.event_id,
                        owner_state_created_at_ms,
                    )?
                } else {
                    let waiting = self.headless_owner_state_envelope(
                        task.id.clone(),
                        &owner_run,
                        HeadlessOwnerRunState::WaitingForInput,
                        Some(coordinator_dry_run_waiting_reason(task)),
                        owner_evidence_refs,
                        Some(acquired.event_id),
                        Some(self.headless_owner_state_idempotency_key(
                            &task.id,
                            &owner_run,
                            "waiting-for-input",
                        )),
                        owner_state_created_at_ms,
                    );
                    journal.append_idempotent(waiting)?.event.event_id
                }
                #[cfg(not(test))]
                {
                    let waiting = self.headless_owner_state_envelope(
                        task.id.clone(),
                        &owner_run,
                        HeadlessOwnerRunState::WaitingForInput,
                        Some(coordinator_dry_run_waiting_reason(task)),
                        owner_evidence_refs,
                        Some(acquired.event_id),
                        Some(self.headless_owner_state_idempotency_key(
                            &task.id,
                            &owner_run,
                            "waiting-for-input",
                        )),
                        owner_state_created_at_ms,
                    );
                    journal.append_idempotent(waiting)?.event.event_id
                }
            }
        } else {
            start_event_id
        };

        let wait = self.envelope(
            task.id.clone(),
            LoopRuntimeEvent::TaskWaitingForInput {
                task_id: task.id.clone(),
                reason: waiting_reason(task, now_ms),
                waiting_at_ms: now_ms,
            },
            Some(lease.lease_id),
            Some(attempt),
            Some(wait_causation_id),
            Some(format!(
                "runner:{}:waiting_for_input:{}:{attempt}",
                self.runner_id, task.id
            )),
            now_ms,
        );
        let wait = journal.append_idempotent(wait)?.event;
        summary.waiting_for_input_tasks += 1;

        let projection = projection_store.rebuild_from_journal(journal)?;
        let task = projection
            .find(&task.id)
            .ok_or_else(|| format!("loop task disappeared after runner wait: {}", task.id))?;
        let result = evaluate_completion(task, &task.evidence);
        let completion = self.envelope(
            task.id.clone(),
            LoopRuntimeEvent::CompletionEvaluated {
                task_id: task.id.clone(),
                result,
            },
            None,
            Some(attempt),
            Some(wait.event_id),
            Some(format!(
                "runner:{}:completion:{}:{attempt}",
                self.runner_id, task.id
            )),
            now_ms,
        );
        journal.append_idempotent(completion)?;
        summary.completion_evaluations += 1;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn headless_owner_run(
        &self,
        task: &LoopTaskRecord,
        lease: &LoopTaskLease,
        attempt: u32,
        causation_id: String,
        human_gate_id: String,
        policy_decision_id: String,
        budget_snapshot_id: String,
        now_ms: u64,
    ) -> HeadlessOwnerRun {
        let snapshot_source = if task.session_id.is_some() {
            HeadlessOwnerSnapshotSource::CurrentDesktopSession
        } else {
            HeadlessOwnerSnapshotSource::Unavailable
        };
        let snapshot_ref = task.session_id.clone();
        let executor_kind = self.headless_owner_executor_kind();
        let executor_key = headless_owner_executor_key(executor_kind);
        let idempotency_key = format!(
            "runner:{}:owner-{executor_key}:{}:{attempt}",
            self.runner_id, task.id
        );
        let correlation_id = format!(
            "runner:{}:task:{}:attempt:{attempt}",
            self.runner_id, task.id
        );
        let evidence_refs = vec![
            human_gate_id.clone(),
            policy_decision_id.clone(),
            budget_snapshot_id.clone(),
            idempotency_key.clone(),
            correlation_id.clone(),
        ];

        HeadlessOwnerRun {
            owner_run_id: format!("owner-run:{}:{attempt}:{executor_key}", task.id),
            task_id: task.id.clone(),
            session_id: task.session_id.clone(),
            lease_id: lease.lease_id.clone(),
            attempt,
            state: HeadlessOwnerRunState::Requested,
            snapshot_source,
            snapshot_ref,
            human_gate_id,
            policy_decision_id,
            budget_snapshot_id,
            idempotency_key,
            correlation_id,
            causation_id: Some(causation_id),
            requested_by: format!("runner:{}", self.runner_id),
            requested_at_ms: now_ms,
            heartbeat_at_ms: None,
            expires_at_ms: lease.expires_at_ms,
            cancellation_reason: None,
            waiting_reason: None,
            executor_kind,
            evidence_refs,
        }
    }

    fn headless_owner_executor_kind(&self) -> HeadlessOwnerExecutorKind {
        #[cfg(test)]
        {
            if self.fake_executor_fixture.is_some() {
                return HeadlessOwnerExecutorKind::FakeExecutor;
            }
        }
        HeadlessOwnerExecutorKind::DryRun
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn record_fake_executor_outcome(
        &self,
        journal: &LoopEventJournal,
        task: &LoopTaskRecord,
        owner_run: &HeadlessOwnerRun,
        fixture: &FakeOwnerExecutorFixture,
        owner_evidence_refs: Vec<String>,
        lease_acquired_event_id: String,
        now_ms: u64,
    ) -> Result<String, String> {
        let running = self.headless_owner_state_envelope(
            task.id.clone(),
            owner_run,
            HeadlessOwnerRunState::FakeRunning,
            None,
            with_fake_evidence(owner_evidence_refs.clone(), "fake-executor:running"),
            Some(lease_acquired_event_id),
            Some(self.headless_owner_state_idempotency_key(&task.id, owner_run, "fake-running")),
            now_ms,
        );
        let running = journal.append_idempotent(running)?.event;
        let outcome = fixture.outcome_state();
        let outcome_evidence = fixture.evidence_refs(owner_evidence_refs);
        let outcome_event = self.headless_owner_state_envelope(
            task.id.clone(),
            owner_run,
            outcome.state,
            outcome.reason,
            outcome_evidence,
            Some(running.event_id),
            Some(self.headless_owner_state_idempotency_key(
                &task.id,
                owner_run,
                headless_owner_state_key(outcome.state),
            )),
            now_ms,
        );
        Ok(journal.append_idempotent(outcome_event)?.event.event_id)
    }

    fn headless_owner_state_idempotency_key(
        &self,
        task_id: &str,
        owner_run: &HeadlessOwnerRun,
        state_key: &str,
    ) -> String {
        format!(
            "runner:{}:owner-{}-state:{}:{}:{}",
            self.runner_id,
            headless_owner_executor_key(owner_run.executor_kind),
            task_id,
            owner_run.owner_run_id,
            state_key
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn headless_owner_state_envelope(
        &self,
        task_id: String,
        owner_run: &HeadlessOwnerRun,
        state: HeadlessOwnerRunState,
        reason: Option<String>,
        evidence_refs: Vec<String>,
        causation_id: Option<String>,
        idempotency_key: Option<String>,
        created_at_ms: u64,
    ) -> LoopEventEnvelope {
        let heartbeat_at_ms = matches!(
            state,
            HeadlessOwnerRunState::LeaseAcquired
                | HeadlessOwnerRunState::DryRunWaiting
                | HeadlessOwnerRunState::WaitingForInput
        )
        .then_some(created_at_ms);
        let cancellation_reason = matches!(
            state,
            HeadlessOwnerRunState::Denied
                | HeadlessOwnerRunState::Interrupted
                | HeadlessOwnerRunState::Cancelled
                | HeadlessOwnerRunState::Expired
                | HeadlessOwnerRunState::Failed
        )
        .then_some(reason.clone())
        .flatten();
        let waiting_reason = matches!(
            state,
            HeadlessOwnerRunState::Denied
                | HeadlessOwnerRunState::DryRunWaiting
                | HeadlessOwnerRunState::WaitingForInput
        )
        .then_some(reason)
        .flatten();

        self.envelope(
            task_id.clone(),
            LoopRuntimeEvent::HeadlessOwnerRunStateRecorded {
                task_id,
                owner_run_id: owner_run.owner_run_id.clone(),
                state,
                heartbeat_at_ms,
                cancellation_reason,
                waiting_reason,
                evidence_refs,
            },
            Some(owner_run.lease_id.clone()),
            Some(owner_run.attempt),
            causation_id,
            idempotency_key,
            created_at_ms,
        )
    }

    fn lease(&self, now_ms: u64) -> LoopTaskLease {
        LoopTaskLease {
            lease_id: new_loop_event_id().replace("evt-", "lease-"),
            owner_pid: self.owner_pid,
            acquired_at_ms: now_ms,
            expires_at_ms: now_ms.saturating_add(self.lease_timeout_ms),
            heartbeat_at_ms: now_ms,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn envelope(
        &self,
        task_id: String,
        event: LoopRuntimeEvent,
        lease_id: Option<String>,
        attempt: Option<u32>,
        causation_id: Option<String>,
        idempotency_key: Option<String>,
        created_at_ms: u64,
    ) -> LoopEventEnvelope {
        LoopEventEnvelope {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: new_loop_event_id(),
            task_id,
            sequence: 0,
            event,
            actor: LoopActor::Runner {
                runner_id: self.runner_id.clone(),
            },
            lease_id,
            attempt,
            correlation_id: Some(format!("runner:{}", self.runner_id)),
            causation_id,
            idempotency_key,
            created_at_ms,
        }
    }

    #[cfg(test)]
    fn new_for_test(runner_id: &str, owner_pid: u32, lease_timeout_ms: u64) -> Self {
        Self {
            runner_id: runner_id.to_string(),
            owner_pid,
            lease_timeout_ms,
            fake_executor_fixture: None,
        }
    }

    #[cfg(test)]
    fn with_fake_executor_fixture(mut self, fixture: FakeOwnerExecutorFixture) -> Self {
        self.fake_executor_fixture = Some(fixture);
        self
    }

    #[cfg(test)]
    fn claim_for_test(task: &mut LoopTaskRecord, owner_pid: u32) -> Result<LoopTaskLease, String> {
        if task.status != LoopTaskStatus::Pending {
            return Err(format!("task is not pending: {:?}", task.status));
        }
        let runner = Self::new_for_test("test-runner", owner_pid, LOOP_TASK_LEASE_TIMEOUT_MS);
        let now_ms = now_millis();
        let lease = runner.lease(now_ms);
        task.status = LoopTaskStatus::Running;
        task.lease = Some(lease.clone());
        task.updated_at_ms = now_ms;
        Ok(lease)
    }

    #[cfg(test)]
    fn queue_stats_for_test(tasks: Vec<LoopTaskRecord>, now_ms: u64) -> LoopRunnerQueueStats {
        Self::queue_stats(&LoopTaskProjection { tasks }, now_ms)
    }
}

pub async fn serve_loop_runner(
    journal: Arc<LoopEventJournal>,
    projection_store: Arc<LoopTaskProjectionStore>,
) -> Result<(), String> {
    let runner = LoopTaskRunner::gateway_default();
    loop {
        let summary = runner.run_once(&journal, &projection_store)?;
        if summary.claimed_tasks > 0 || summary.interrupted_stale_leases > 0 {
            log::info!(
                "loop runner claimed {} task(s), interrupted {} stale lease(s)",
                summary.claimed_tasks,
                summary.interrupted_stale_leases
            );
        }
        tokio::time::sleep(Duration::from_secs(LOOP_RUNNER_POLL_INTERVAL_SECS)).await;
    }
}

pub fn loop_runner_queue_stats(
    journal: &LoopEventJournal,
    projection_store: &LoopTaskProjectionStore,
) -> Result<LoopRunnerQueueStats, String> {
    let projection = projection_store.load_or_rebuild(journal)?;
    Ok(LoopTaskRunner::queue_stats(&projection, now_millis()))
}

fn stale_running_task(task: &LoopTaskRecord, now_ms: u64) -> bool {
    task.status == LoopTaskStatus::Running
        && task
            .lease
            .as_ref()
            .is_some_and(|lease| lease.expires_at_ms <= now_ms)
}

fn current_attempt_for_task(events: &[LoopEventEnvelope], task_id: &str) -> Option<u32> {
    events
        .iter()
        .filter(|event| event.task_id == task_id)
        .filter_map(|event| event.attempt)
        .max()
        .or_else(|| {
            let starts = events
                .iter()
                .filter(|event| {
                    event.task_id == task_id
                        && matches!(event.event, LoopRuntimeEvent::TaskStarted { .. })
                })
                .count();
            u32::try_from(starts).ok().filter(|starts| *starts > 0)
        })
}

fn nonterminal_headless_owner_state(state: HeadlessOwnerRunState) -> bool {
    !matches!(
        state,
        HeadlessOwnerRunState::Denied
            | HeadlessOwnerRunState::Interrupted
            | HeadlessOwnerRunState::Cancelled
            | HeadlessOwnerRunState::Expired
            | HeadlessOwnerRunState::Completed
            | HeadlessOwnerRunState::Failed
    )
}

fn headless_owner_evidence_refs(owner_run: &HeadlessOwnerRun) -> Vec<String> {
    vec![
        owner_run.human_gate_id.clone(),
        owner_run.policy_decision_id.clone(),
        owner_run.budget_snapshot_id.clone(),
        owner_run.idempotency_key.clone(),
        owner_run.correlation_id.clone(),
    ]
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct FakeOwnerExecutorFixture {
    outcome: FakeOwnerExecutorOutcome,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum FakeOwnerExecutorOutcome {
    Completed,
    PendingConfirmation { confirmation_id: String },
    PendingToolCall { tool_call_id: String },
    Interrupted { reason: String },
    Cancelled { reason: String },
    Expired { reason: String },
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct FakeOwnerExecutorOutcomeState {
    state: HeadlessOwnerRunState,
    reason: Option<String>,
}

#[cfg(test)]
impl FakeOwnerExecutorFixture {
    fn completed() -> Self {
        Self {
            outcome: FakeOwnerExecutorOutcome::Completed,
        }
    }

    fn pending_confirmation(confirmation_id: &str) -> Self {
        Self {
            outcome: FakeOwnerExecutorOutcome::PendingConfirmation {
                confirmation_id: confirmation_id.to_string(),
            },
        }
    }

    fn pending_tool_call(tool_call_id: &str) -> Self {
        Self {
            outcome: FakeOwnerExecutorOutcome::PendingToolCall {
                tool_call_id: tool_call_id.to_string(),
            },
        }
    }

    fn interrupted(reason: &str) -> Self {
        Self {
            outcome: FakeOwnerExecutorOutcome::Interrupted {
                reason: reason.to_string(),
            },
        }
    }

    fn cancelled(reason: &str) -> Self {
        Self {
            outcome: FakeOwnerExecutorOutcome::Cancelled {
                reason: reason.to_string(),
            },
        }
    }

    fn expired(reason: &str) -> Self {
        Self {
            outcome: FakeOwnerExecutorOutcome::Expired {
                reason: reason.to_string(),
            },
        }
    }

    fn outcome_state(&self) -> FakeOwnerExecutorOutcomeState {
        match &self.outcome {
            FakeOwnerExecutorOutcome::Completed => FakeOwnerExecutorOutcomeState {
                state: HeadlessOwnerRunState::Completed,
                reason: None,
            },
            FakeOwnerExecutorOutcome::PendingConfirmation { confirmation_id } => {
                FakeOwnerExecutorOutcomeState {
                    state: HeadlessOwnerRunState::WaitingForInput,
                    reason: Some(format!(
                        "fake executor blocked on pending confirmation {confirmation_id}; not auto-accepted and no completion recorded."
                    )),
                }
            }
            FakeOwnerExecutorOutcome::PendingToolCall { tool_call_id } => {
                FakeOwnerExecutorOutcomeState {
                    state: HeadlessOwnerRunState::WaitingForInput,
                    reason: Some(format!(
                        "fake executor blocked on pending tool call {tool_call_id}; not auto-accepted and no completion recorded."
                    )),
                }
            }
            FakeOwnerExecutorOutcome::Interrupted { reason } => FakeOwnerExecutorOutcomeState {
                state: HeadlessOwnerRunState::Interrupted,
                reason: Some(reason.clone()),
            },
            FakeOwnerExecutorOutcome::Cancelled { reason } => FakeOwnerExecutorOutcomeState {
                state: HeadlessOwnerRunState::Cancelled,
                reason: Some(reason.clone()),
            },
            FakeOwnerExecutorOutcome::Expired { reason } => FakeOwnerExecutorOutcomeState {
                state: HeadlessOwnerRunState::Expired,
                reason: Some(reason.clone()),
            },
        }
    }

    fn evidence_refs(&self, evidence_refs: Vec<String>) -> Vec<String> {
        match &self.outcome {
            FakeOwnerExecutorOutcome::Completed => {
                with_fake_evidence(evidence_refs, "fake-executor:completed")
            }
            FakeOwnerExecutorOutcome::PendingConfirmation { confirmation_id } => {
                with_fake_evidence(
                    evidence_refs,
                    &format!("fake-executor:pending-confirmation:{confirmation_id}"),
                )
            }
            FakeOwnerExecutorOutcome::PendingToolCall { tool_call_id } => with_fake_evidence(
                evidence_refs,
                &format!("fake-executor:pending-tool-call:{tool_call_id}"),
            ),
            FakeOwnerExecutorOutcome::Interrupted { .. } => {
                with_fake_evidence(evidence_refs, "fake-executor:interrupted")
            }
            FakeOwnerExecutorOutcome::Cancelled { .. } => {
                with_fake_evidence(evidence_refs, "fake-executor:cancelled")
            }
            FakeOwnerExecutorOutcome::Expired { .. } => {
                let refs = with_fake_evidence(evidence_refs, "fake-executor:expired");
                with_fake_evidence(refs, "fake-executor:waiting-evidence")
            }
        }
    }
}

#[cfg(test)]
fn with_fake_evidence(mut evidence_refs: Vec<String>, evidence_ref: &str) -> Vec<String> {
    evidence_refs.push(evidence_ref.to_string());
    evidence_refs
}

fn headless_owner_executor_key(executor_kind: HeadlessOwnerExecutorKind) -> &'static str {
    match executor_kind {
        HeadlessOwnerExecutorKind::DryRun => "dry-run",
        HeadlessOwnerExecutorKind::FakeExecutor => "fake-executor",
        HeadlessOwnerExecutorKind::None => "none",
        HeadlessOwnerExecutorKind::AgentSessionAdapter => "agent-session-adapter",
    }
}

fn headless_owner_state_key(state: HeadlessOwnerRunState) -> &'static str {
    match state {
        HeadlessOwnerRunState::Requested => "requested",
        HeadlessOwnerRunState::Denied => "denied",
        HeadlessOwnerRunState::Ready => "ready",
        HeadlessOwnerRunState::LeaseAcquired => "lease-acquired",
        HeadlessOwnerRunState::DryRunWaiting => "dry-run-waiting",
        HeadlessOwnerRunState::FakeRunning => "fake-running",
        HeadlessOwnerRunState::Running => "running",
        HeadlessOwnerRunState::WaitingForInput => "waiting-for-input",
        HeadlessOwnerRunState::Interrupted => "interrupted",
        HeadlessOwnerRunState::Cancelled => "cancelled",
        HeadlessOwnerRunState::Expired => "expired",
        HeadlessOwnerRunState::Completed => "completed",
        HeadlessOwnerRunState::Failed => "failed",
    }
}

fn coordinator_dry_run_policy_intent() -> LoopActionIntent {
    LoopActionIntent::ServiceLifecycle {
        service: "headless_owner_coordinator".to_string(),
        lifecycle_action: "coordinator_dry_run".to_string(),
        update_repair_allowlisted: false,
    }
}

fn headless_resume_approval_event_id(
    events: &[LoopEventEnvelope],
    task: &LoopTaskRecord,
    attempt: u32,
) -> String {
    events
        .iter()
        .find_map(|event| match &event.event {
            LoopRuntimeEvent::HeadlessResumeApprovalRecorded { task_id, approval }
                if task_id == &task.id && approval.task_id == task.id =>
            {
                Some(event.event_id.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| format!("headless-resume-approval:{}:{attempt}", task.id))
}

fn coordinator_dry_run_waiting_reason(task: &LoopTaskRecord) -> String {
    if let Some(session_id) = task.session_id.as_deref() {
        format!(
            "coordinator dry run acquired lease ownership for existing desktop session {session_id} and stopped at waiting_for_input; no headless AgentSession was created."
        )
    } else {
        "coordinator dry run acquired lease ownership but stopped at waiting_for_input because no desktop session snapshot is available; no headless AgentSession was created."
            .to_string()
    }
}

fn waiting_reason(task: &LoopTaskRecord, now_ms: u64) -> String {
    let readiness = derive_headless_resume_readiness(
        task.headless_resume_mode,
        task.headless_resume_approval.as_ref(),
        now_ms,
    );

    match readiness {
        HeadlessResumeReadiness::ApprovalRecordedLeasePending => {
            "Gateway loop runner sees headless resume approval is recorded for this task, but this derived readiness dry run is still lease/desktop owner pending; no headless AgentSession was created and autonomous resume remains disabled."
                .to_string()
        }
        HeadlessResumeReadiness::ApprovalExpired => {
            let expires_at_ms = task
                .headless_resume_approval
                .as_ref()
                .map(|approval| approval.expires_at_ms)
                .unwrap_or_default();
            format!(
                "Gateway loop runner sees headless resume approval expired at {expires_at_ms}; lease/desktop owner pending and waiting_for_input remains active; no headless AgentSession was created and autonomous resume remains disabled."
            )
        }
        HeadlessResumeReadiness::ApprovalRequired => {
            if let Some(session_id) = task.session_id.as_deref() {
                format!(
                    "Gateway loop runner is waiting for durable headless resume approval before any lease dry run; existing desktop session {session_id} remains the owner and no headless AgentSession was created."
                )
            } else {
                "Gateway loop runner is waiting for durable headless resume approval and a desktop owner before execution; no headless AgentSession was created."
                    .to_string()
            }
        }
        HeadlessResumeReadiness::DesktopOwnerRequired => {
            if let Some(session_id) = task.session_id.as_deref() {
                format!(
                    "Gateway loop runner is waiting for existing desktop session {session_id} to accept the next step; headless autonomous resume is disabled by default, requires durable human approval, and no headless AgentSession was created."
                )
            } else {
                "Gateway loop runner requires an existing desktop session owner before execution; headless autonomous resume is disabled by default, requires durable human approval, and no headless AgentSession was created."
                    .to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FakeOwnerExecutorFixture, LoopRunnerRunSummary, LoopTaskRunner};
    use crate::loop_runtime::{
        BudgetSnapshot, HeadlessOwnerExecutorKind, HeadlessOwnerRun, HeadlessOwnerRunState,
        HeadlessOwnerSnapshotSource, LoopActionIntent, LoopActor, LoopEventEnvelope,
        LoopEventJournal, LoopRuntimeEvent, LoopTaskLease, LoopTaskProjectionStore, LoopTaskRecord,
        LoopTaskStatus, LOOP_RUNTIME_SCHEMA_VERSION,
    };

    #[test]
    fn runner_claims_pending_task_with_lease() {
        let mut task = LoopTaskRecord::new_for_test("task-1", "run acceptance");

        let lease = LoopTaskRunner::claim_for_test(&mut task, 1234).unwrap();

        assert_eq!(task.status, LoopTaskStatus::Running);
        assert_eq!(task.lease.as_ref().unwrap().lease_id, lease.lease_id);
        assert_eq!(task.lease.as_ref().unwrap().owner_pid, 1234);
    }

    #[test]
    fn runner_records_waiting_for_input_without_auto_execution() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        journal
            .append(LoopEventEnvelope::task_created_for_test(
                "task-2",
                "continue a desktop-owned session",
            ))
            .unwrap();
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000);

        let summary = runner.run_once_at(&journal, &projection, 1_000).unwrap();

        assert_eq!(summary.claimed_tasks, 1);
        assert_eq!(summary.waiting_for_input_tasks, 1);
        assert_eq!(summary.completion_evaluations, 1);

        let task = projection
            .load_or_rebuild(&journal)
            .unwrap()
            .find("task-2")
            .cloned()
            .unwrap();
        assert_eq!(task.status, LoopTaskStatus::WaitingForInput);
        assert!(task.lease.is_none());
        assert!(task.headless_owner_runs.is_empty());
        assert_eq!(
            task.completion_result.as_ref().unwrap().reasons,
            vec!["task_waiting_for_input"]
        );

        let events = journal.load_all().unwrap();
        assert!(matches!(
            events[1].event,
            LoopRuntimeEvent::TaskStarted { .. }
        ));
        let reason = events
            .iter()
            .find_map(|event| match &event.event {
                LoopRuntimeEvent::TaskWaitingForInput { reason, .. } => Some(reason),
                _ => None,
            })
            .expect("waiting event");
        assert!(reason.contains("headless autonomous resume is disabled by default"));
        assert!(reason.contains("durable human approval"));
        assert!(reason.contains("no headless AgentSession was created"));
        assert!(matches!(
            events.last().unwrap().event,
            LoopRuntimeEvent::CompletionEvaluated { .. }
        ));
    }

    #[test]
    fn runner_reuses_existing_task_started_event_for_concurrent_retry() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        journal
            .append(LoopEventEnvelope::task_created_for_test(
                "task-concurrent-start",
                "same pending view can race with another runner",
            ))
            .unwrap();
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000);
        let stale_projection = projection.load_or_rebuild(&journal).unwrap();
        let stale_events = journal.load_all().unwrap();
        let stale_task = stale_projection
            .find("task-concurrent-start")
            .cloned()
            .unwrap();

        let mut first_summary = LoopRunnerRunSummary::default();
        runner
            .process_pending_task(
                &journal,
                &projection,
                &stale_task,
                &stale_events,
                1_000,
                &mut first_summary,
            )
            .unwrap();
        let first_started_lease = journal
            .load_all()
            .unwrap()
            .into_iter()
            .find_map(|event| match event.event {
                LoopRuntimeEvent::TaskStarted { lease, .. } => Some(lease),
                _ => None,
            })
            .expect("task started lease");

        let mut retry_summary = LoopRunnerRunSummary::default();
        runner
            .process_pending_task(
                &journal,
                &projection,
                &stale_task,
                &stale_events,
                2_000,
                &mut retry_summary,
            )
            .unwrap();

        let events = journal.load_all().unwrap();
        let started_leases: Vec<_> = events
            .iter()
            .filter_map(|event| match &event.event {
                LoopRuntimeEvent::TaskStarted { lease, .. } => Some(lease.lease_id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(started_leases, vec![first_started_lease.lease_id.as_str()]);
        assert_eq!(retry_summary.claimed_tasks, 1);
        assert_eq!(retry_summary.waiting_for_input_tasks, 1);
    }

    #[test]
    fn approved_pending_task_records_headless_owner_dry_run_before_waiting() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        let mut task = LoopTaskRecord::new_for_test(
            "task-approved",
            "approved but bounded by coordinator dry run",
        );
        task.session_id = Some("desktop-session-1".to_string());
        task.policy.allow_service_lifecycle = true;
        journal.append(task_created_event_for_test(task)).unwrap();
        let approval_event = journal
            .append(LoopEventEnvelope::headless_resume_approval_recorded(
                "task-approved".to_string(),
                crate::loop_runtime::HeadlessResumeApproval {
                    task_id: "task-approved".to_string(),
                    approved_by: "human-reviewer".to_string(),
                    approved_at_ms: 10,
                    scope: "task".to_string(),
                    expires_at_ms: 60_010,
                },
                Some("test".to_string()),
                Some("headless:task-approved".to_string()),
            ))
            .unwrap()
            .event;
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000);

        let summary = runner.run_once_at(&journal, &projection, 1_000).unwrap();

        assert_eq!(summary.claimed_tasks, 1);
        assert_eq!(summary.waiting_for_input_tasks, 1);
        assert_eq!(summary.completion_evaluations, 1);
        let task = projection
            .load_or_rebuild(&journal)
            .unwrap()
            .find("task-approved")
            .cloned()
            .unwrap();
        assert_eq!(task.status, LoopTaskStatus::WaitingForInput);
        assert!(task.lease.is_none());
        assert!(task.headless_resume_approval.is_some());
        assert_eq!(task.headless_owner_runs.len(), 1);
        let owner_run = &task.headless_owner_runs[0];
        assert_eq!(owner_run.owner_run_id, "owner-run:task-approved:1:dry-run");
        assert_eq!(owner_run.session_id.as_deref(), Some("desktop-session-1"));
        assert_eq!(
            owner_run.snapshot_source,
            HeadlessOwnerSnapshotSource::CurrentDesktopSession
        );
        assert_eq!(owner_run.snapshot_ref.as_deref(), Some("desktop-session-1"));
        assert_eq!(owner_run.executor_kind, HeadlessOwnerExecutorKind::DryRun);
        assert_eq!(owner_run.state, HeadlessOwnerRunState::WaitingForInput);
        assert_eq!(
            owner_run.idempotency_key,
            "runner:runner-1:owner-dry-run:task-approved:1"
        );
        assert_eq!(
            owner_run.correlation_id,
            "runner:runner-1:task:task-approved:attempt:1"
        );
        let events = journal.load_all().unwrap();
        let policy = events
            .iter()
            .find_map(|event| match &event.event {
                LoopRuntimeEvent::PolicyDecisionRecorded { decision, .. } => {
                    Some((event, decision))
                }
                _ => None,
            })
            .expect("policy decision recorded");
        let started = events
            .iter()
            .find(|event| matches!(event.event, LoopRuntimeEvent::TaskStarted { .. }))
            .expect("task started event");
        assert_eq!(
            policy.0.lease_id.as_deref(),
            Some(owner_run.lease_id.as_str())
        );
        assert_eq!(policy.0.attempt, Some(1));
        assert_eq!(
            policy.0.causation_id.as_deref(),
            Some(started.event_id.as_str())
        );
        assert_eq!(
            policy.0.idempotency_key.as_deref(),
            Some("runner:runner-1:owner-dry-run-policy:task-approved:1")
        );
        assert!(policy.1.allowed);
        assert_eq!(
            policy.1.intent,
            LoopActionIntent::ServiceLifecycle {
                service: "headless_owner_coordinator".to_string(),
                lifecycle_action: "coordinator_dry_run".to_string(),
                update_repair_allowlisted: false,
            }
        );
        let budget = events
            .iter()
            .find(|event| matches!(event.event, LoopRuntimeEvent::BudgetSnapshotRecorded { .. }))
            .expect("budget snapshot recorded");
        assert_eq!(
            budget.lease_id.as_deref(),
            Some(owner_run.lease_id.as_str())
        );
        assert_eq!(budget.attempt, Some(1));
        assert_eq!(
            budget.causation_id.as_deref(),
            Some(policy.0.event_id.as_str())
        );
        assert_eq!(
            budget.idempotency_key.as_deref(),
            Some("runner:runner-1:owner-dry-run-budget:task-approved:1")
        );
        assert_eq!(owner_run.human_gate_id, approval_event.event_id);
        assert_eq!(owner_run.policy_decision_id, policy.1.decision_id);
        assert_eq!(owner_run.budget_snapshot_id, budget.event_id);
        assert_eq!(
            owner_run.causation_id.as_deref(),
            Some(budget.event_id.as_str())
        );
        assert_eq!(
            owner_run.evidence_refs,
            vec![
                owner_run.human_gate_id.clone(),
                owner_run.policy_decision_id.clone(),
                owner_run.budget_snapshot_id.clone(),
                "runner:runner-1:owner-dry-run:task-approved:1".to_string(),
                "runner:runner-1:task:task-approved:attempt:1".to_string(),
            ]
        );
        let started_lease_id = events
            .iter()
            .find_map(|event| match &event.event {
                LoopRuntimeEvent::TaskStarted { lease, .. } => Some(lease.lease_id.as_str()),
                _ => None,
            })
            .expect("task started event");
        let requested = events
            .iter()
            .find_map(|event| match &event.event {
                LoopRuntimeEvent::HeadlessOwnerRunRequested { owner_run, .. } => Some(owner_run),
                _ => None,
            })
            .expect("headless owner run requested");
        assert_eq!(requested.lease_id, started_lease_id);
        assert_eq!(requested.attempt, 1);
        let owner_request = events
            .iter()
            .find(|event| {
                matches!(
                    event.event,
                    LoopRuntimeEvent::HeadlessOwnerRunRequested { .. }
                )
            })
            .expect("owner request event");
        assert_eq!(
            owner_request.causation_id.as_deref(),
            Some(budget.event_id.as_str())
        );
        assert_eq!(
            requested.evidence_refs,
            vec![
                owner_run.human_gate_id.clone(),
                owner_run.policy_decision_id.clone(),
                owner_run.budget_snapshot_id.clone(),
                "runner:runner-1:owner-dry-run:task-approved:1".to_string(),
                "runner:runner-1:task:task-approved:attempt:1".to_string(),
            ]
        );

        let states: Vec<_> = events
            .iter()
            .filter_map(|event| match &event.event {
                LoopRuntimeEvent::HeadlessOwnerRunStateRecorded {
                    state,
                    waiting_reason,
                    evidence_refs,
                    ..
                } => Some((*state, waiting_reason.clone(), evidence_refs.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(states.len(), 2);
        assert_eq!(states[0].0, HeadlessOwnerRunState::LeaseAcquired);
        assert_eq!(states[1].0, HeadlessOwnerRunState::WaitingForInput);
        assert!(states[1]
            .1
            .as_deref()
            .unwrap()
            .contains("coordinator dry run"));
        assert!(states[1]
            .1
            .as_deref()
            .unwrap()
            .contains("no headless AgentSession was created"));
        assert!(states[1].2.contains(&owner_run.budget_snapshot_id));

        let waiting = events
            .iter()
            .find_map(|event| match &event.event {
                LoopRuntimeEvent::TaskWaitingForInput { reason, .. } => Some(reason),
                _ => None,
            })
            .expect("waiting event");
        assert!(waiting.contains("headless resume approval is recorded"));
        assert!(waiting.contains("lease/desktop owner pending"));
        assert!(waiting.contains("no headless AgentSession was created"));
    }

    #[test]
    fn budget_denied_pending_task_without_approval_keeps_wait_only_behavior() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        journal
            .append(LoopEventEnvelope::task_created_for_test(
                "task-budget-denied",
                "budget should block coordinator dry run",
            ))
            .unwrap();
        journal
            .append(budget_snapshot_event_for_test(
                "task-budget-denied",
                BudgetSnapshot {
                    budget_exceeded: true,
                    model_call_in_flight: false,
                    tool_call_started: false,
                    long_running_tool_supports_cancel: false,
                    model_rounds_used: 40,
                    tool_calls_used: 0,
                    elapsed_ms: 1_000,
                    input_tokens: None,
                    output_tokens: None,
                    estimated_cost_micros: None,
                    has_unknown_token_usage: true,
                    has_unknown_cost: true,
                },
            ))
            .unwrap();
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000);

        let summary = runner.run_once_at(&journal, &projection, 1_000).unwrap();

        assert_eq!(summary.claimed_tasks, 1);
        assert_eq!(summary.waiting_for_input_tasks, 1);
        let task = projection
            .load_or_rebuild(&journal)
            .unwrap()
            .find("task-budget-denied")
            .cloned()
            .unwrap();
        assert_eq!(task.status, LoopTaskStatus::WaitingForInput);
        assert!(task.lease.is_none());
        assert!(task.headless_owner_runs.is_empty());

        let events = journal.load_all().unwrap();
        assert!(!events.iter().any(|event| matches!(
            event.event,
            LoopRuntimeEvent::HeadlessOwnerRunRequested { .. }
                | LoopRuntimeEvent::HeadlessOwnerRunStateRecorded { .. }
        )));
    }

    #[test]
    fn approved_pending_task_with_default_policy_records_denied_headless_owner_dry_run() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        journal
            .append(LoopEventEnvelope::task_created_for_test(
                "task-policy-denied",
                "default policy should block coordinator dry run",
            ))
            .unwrap();
        journal
            .append(LoopEventEnvelope::headless_resume_approval_recorded(
                "task-policy-denied".to_string(),
                crate::loop_runtime::HeadlessResumeApproval {
                    task_id: "task-policy-denied".to_string(),
                    approved_by: "human-reviewer".to_string(),
                    approved_at_ms: 10,
                    scope: "task".to_string(),
                    expires_at_ms: 60_010,
                },
                Some("test".to_string()),
                Some("headless:task-policy-denied".to_string()),
            ))
            .unwrap();
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000);

        let summary = runner.run_once_at(&journal, &projection, 1_000).unwrap();

        assert_eq!(summary.claimed_tasks, 1);
        assert_eq!(summary.waiting_for_input_tasks, 1);
        let task = projection
            .load_or_rebuild(&journal)
            .unwrap()
            .find("task-policy-denied")
            .cloned()
            .unwrap();
        assert_eq!(task.headless_owner_runs.len(), 1);
        assert_eq!(
            task.headless_owner_runs[0].state,
            HeadlessOwnerRunState::Denied
        );
        assert_eq!(
            task.headless_owner_runs[0].waiting_reason.as_deref(),
            Some(
                "coordinator dry run denied before execution: service_lifecycle_requires_human_approval"
            )
        );

        let events = journal.load_all().unwrap();
        assert!(events.iter().any(|event| matches!(
            event.event,
            LoopRuntimeEvent::HeadlessOwnerRunRequested { .. }
        )));
        assert!(!events.iter().any(|event| matches!(
            event.event,
            LoopRuntimeEvent::HeadlessOwnerRunStateRecorded {
                state: HeadlessOwnerRunState::LeaseAcquired,
                ..
            }
        )));
        let decision = events
            .iter()
            .find_map(|event| match &event.event {
                LoopRuntimeEvent::PolicyDecisionRecorded { decision, .. } => Some(decision),
                _ => None,
            })
            .expect("policy decision");
        assert!(!decision.allowed);
        assert_eq!(decision.reason, "service_lifecycle_requires_human_approval");
    }

    #[test]
    fn approved_policy_allowed_budget_denied_records_denied_headless_owner_dry_run() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        let mut task = LoopTaskRecord::new_for_test(
            "task-budget-denied-approved",
            "budget should block coordinator dry run after approval",
        );
        task.policy.allow_service_lifecycle = true;
        journal.append(task_created_event_for_test(task)).unwrap();
        journal
            .append(LoopEventEnvelope::headless_resume_approval_recorded(
                "task-budget-denied-approved".to_string(),
                crate::loop_runtime::HeadlessResumeApproval {
                    task_id: "task-budget-denied-approved".to_string(),
                    approved_by: "human-reviewer".to_string(),
                    approved_at_ms: 10,
                    scope: "task".to_string(),
                    expires_at_ms: 60_010,
                },
                Some("test".to_string()),
                Some("headless:task-budget-denied-approved".to_string()),
            ))
            .unwrap();
        journal
            .append(budget_snapshot_event_for_test(
                "task-budget-denied-approved",
                BudgetSnapshot {
                    budget_exceeded: true,
                    model_call_in_flight: false,
                    tool_call_started: false,
                    long_running_tool_supports_cancel: false,
                    model_rounds_used: 40,
                    tool_calls_used: 0,
                    elapsed_ms: 1_000,
                    input_tokens: None,
                    output_tokens: None,
                    estimated_cost_micros: None,
                    has_unknown_token_usage: true,
                    has_unknown_cost: true,
                },
            ))
            .unwrap();
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000);

        let summary = runner.run_once_at(&journal, &projection, 1_000).unwrap();

        assert_eq!(summary.claimed_tasks, 1);
        assert_eq!(summary.waiting_for_input_tasks, 1);
        let task = projection
            .load_or_rebuild(&journal)
            .unwrap()
            .find("task-budget-denied-approved")
            .cloned()
            .unwrap();
        assert_eq!(task.headless_owner_runs.len(), 1);
        let owner_run = &task.headless_owner_runs[0];
        assert_eq!(owner_run.state, HeadlessOwnerRunState::Denied);
        assert_eq!(
            owner_run.waiting_reason.as_deref(),
            Some(
                "coordinator dry run denied before execution: budget_exceeded_requires_human_approval"
            )
        );

        let events = journal.load_all().unwrap();
        assert!(!events.iter().any(|event| matches!(
            event.event,
            LoopRuntimeEvent::HeadlessOwnerRunStateRecorded {
                state: HeadlessOwnerRunState::LeaseAcquired,
                ..
            }
        )));
        let decision = events
            .iter()
            .find_map(|event| match &event.event {
                LoopRuntimeEvent::PolicyDecisionRecorded { decision, .. } => Some(decision),
                _ => None,
            })
            .expect("policy decision");
        assert!(decision.allowed);
        assert_eq!(decision.reason, "allowed_by_service_lifecycle_allowlist");
    }

    #[test]
    fn runner_marks_expired_headless_approval_as_waiting_for_input_without_resuming() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        journal
            .append(LoopEventEnvelope::task_created_for_test(
                "task-expired",
                "expired approval should not resume",
            ))
            .unwrap();
        journal
            .append(LoopEventEnvelope::headless_resume_approval_recorded(
                "task-expired".to_string(),
                crate::loop_runtime::HeadlessResumeApproval {
                    task_id: "task-expired".to_string(),
                    approved_by: "human-reviewer".to_string(),
                    approved_at_ms: 10,
                    scope: "task".to_string(),
                    expires_at_ms: 500,
                },
                Some("test".to_string()),
                Some("headless:task-expired".to_string()),
            ))
            .unwrap();
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000);

        let summary = runner.run_once_at(&journal, &projection, 1_000).unwrap();

        assert_eq!(summary.claimed_tasks, 1);
        assert_eq!(summary.waiting_for_input_tasks, 1);
        let task = projection
            .load_or_rebuild(&journal)
            .unwrap()
            .find("task-expired")
            .cloned()
            .unwrap();
        assert_eq!(task.status, LoopTaskStatus::WaitingForInput);
        assert!(task.lease.is_none());

        let events = journal.load_all().unwrap();
        let waiting = events
            .iter()
            .find_map(|event| match &event.event {
                LoopRuntimeEvent::TaskWaitingForInput { reason, .. } => Some(reason),
                _ => None,
            })
            .expect("waiting event");
        assert!(waiting.contains("headless resume approval expired at 500"));
        assert!(waiting.contains("lease/desktop owner pending"));
        assert!(waiting.contains("no headless AgentSession was created"));
    }

    #[test]
    fn runner_interrupts_stale_running_lease_without_resuming() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        journal
            .append(LoopEventEnvelope::task_created_for_test(
                "task-3",
                "recover after gateway restart",
            ))
            .unwrap();
        journal
            .append(stale_started_event("task-3", "lease-stale", 100, 200))
            .unwrap();
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000);

        let summary = runner.run_once_at(&journal, &projection, 1_000).unwrap();

        assert_eq!(summary.interrupted_stale_leases, 1);
        assert_eq!(summary.claimed_tasks, 0);
        let task = projection
            .load_or_rebuild(&journal)
            .unwrap()
            .find("task-3")
            .cloned()
            .unwrap();
        assert_eq!(task.status, LoopTaskStatus::Interrupted);
        assert!(task.lease.is_none());
        assert!(task
            .outcome
            .as_ref()
            .unwrap()
            .message
            .contains("stale lease"));
    }

    #[test]
    fn runner_expires_nonterminal_headless_owner_run_for_stale_lease() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        journal
            .append(LoopEventEnvelope::task_created_for_test(
                "task-stale-owner",
                "recover stale owner",
            ))
            .unwrap();
        journal
            .append(stale_started_event(
                "task-stale-owner",
                "lease-owner-stale",
                100,
                200,
            ))
            .unwrap();
        journal
            .append(headless_owner_run_requested_event_for_test(
                owner_run_for_test("task-stale-owner", "lease-owner-stale", 1),
            ))
            .unwrap();
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000);

        let summary = runner.run_once_at(&journal, &projection, 1_000).unwrap();

        assert_eq!(summary.interrupted_stale_leases, 1);
        let task = projection
            .load_or_rebuild(&journal)
            .unwrap()
            .find("task-stale-owner")
            .cloned()
            .unwrap();
        assert_eq!(task.status, LoopTaskStatus::Interrupted);
        assert_eq!(task.headless_owner_runs.len(), 1);
        assert_eq!(
            task.headless_owner_runs[0].state,
            HeadlessOwnerRunState::Expired
        );
        assert!(task.headless_owner_runs[0]
            .cancellation_reason
            .as_deref()
            .unwrap()
            .contains("stale lease"));
    }

    #[test]
    fn runner_queue_stats_counts_pending_running_and_stale_leases() {
        let pending = LoopTaskRecord::new_for_test("task-pending", "pending");
        let mut running = LoopTaskRecord::new_for_test("task-running", "running");
        running.status = LoopTaskStatus::Running;
        running.lease = Some(LoopTaskLease {
            lease_id: "lease-live".to_string(),
            owner_pid: 1,
            acquired_at_ms: 100,
            expires_at_ms: 2_000,
            heartbeat_at_ms: 100,
        });
        let mut stale = LoopTaskRecord::new_for_test("task-stale", "stale");
        stale.status = LoopTaskStatus::Running;
        stale.lease = Some(LoopTaskLease {
            lease_id: "lease-stale".to_string(),
            owner_pid: 2,
            acquired_at_ms: 100,
            expires_at_ms: 500,
            heartbeat_at_ms: 100,
        });

        let stats = LoopTaskRunner::queue_stats_for_test(vec![pending, running, stale], 1_000);

        assert_eq!(stats.pending_loop_tasks, 1);
        assert_eq!(stats.running_loop_tasks, 2);
        assert_eq!(stats.stale_loop_task_leases, 1);
    }

    #[test]
    fn runner_queue_stats_exposes_headless_owner_dry_run_states() {
        let mut task = LoopTaskRecord::new_for_test("task-owner-stats", "stats");
        task.headless_owner_runs.push(owner_run_for_test(
            "task-owner-stats",
            "lease-owner-stats",
            1,
        ));
        task.headless_owner_runs[0].state = HeadlessOwnerRunState::WaitingForInput;

        let stats = LoopTaskRunner::queue_stats_for_test(vec![task], 1_000);

        assert_eq!(stats.dry_run_headless_owner_runs, 1);
        assert_eq!(stats.waiting_headless_owner_runs, 1);
        assert_eq!(stats.denied_headless_owner_runs, 0);
        assert_eq!(stats.expired_headless_owner_runs, 0);
    }

    #[test]
    fn approved_policy_budget_allowed_fake_executor_records_running_and_completed_chain() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        append_approved_policy_allowed_task(&journal, "task-fake-completed");
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000)
            .with_fake_executor_fixture(FakeOwnerExecutorFixture::completed());

        let summary = runner.run_once_at(&journal, &projection, 1_000).unwrap();

        assert_eq!(summary.claimed_tasks, 1);
        assert_eq!(summary.waiting_for_input_tasks, 1);
        assert_eq!(summary.completion_evaluations, 1);
        let task = projection
            .load_or_rebuild(&journal)
            .unwrap()
            .find("task-fake-completed")
            .cloned()
            .unwrap();
        assert_eq!(task.status, LoopTaskStatus::WaitingForInput);
        assert_eq!(
            task.completion_result.as_ref().unwrap().reasons,
            vec!["task_waiting_for_input"]
        );
        assert!(!task.completion_result.as_ref().unwrap().commit_eligible);
        assert_eq!(
            task.completion_result.as_ref().unwrap().commit_blockers,
            vec!["task_waiting_for_input"]
        );
        assert_eq!(task.headless_owner_runs.len(), 1);
        let owner_run = &task.headless_owner_runs[0];
        assert_eq!(
            owner_run.executor_kind,
            HeadlessOwnerExecutorKind::FakeExecutor
        );
        assert_eq!(owner_run.state, HeadlessOwnerRunState::Completed);
        assert!(owner_run.cancellation_reason.is_none());
        assert!(owner_run.waiting_reason.is_none());
        assert!(owner_run
            .evidence_refs
            .contains(&"fake-executor:completed".to_string()));

        let events = journal.load_all().unwrap();
        let started_lease = events
            .iter()
            .find_map(|event| match &event.event {
                LoopRuntimeEvent::TaskStarted { lease, .. } => Some(lease.lease_id.as_str()),
                _ => None,
            })
            .expect("task started");
        let states = owner_state_events(&events);
        assert_eq!(
            states.iter().map(|state| state.0).collect::<Vec<_>>(),
            vec![
                HeadlessOwnerRunState::LeaseAcquired,
                HeadlessOwnerRunState::FakeRunning,
                HeadlessOwnerRunState::Completed,
            ]
        );
        for (_, event) in states {
            assert_eq!(event.lease_id.as_deref(), Some(started_lease));
            assert_eq!(event.attempt, Some(1));
            assert!(event
                .idempotency_key
                .as_deref()
                .unwrap()
                .starts_with("runner:runner-1:owner-fake-executor-state:task-fake-completed:"));
        }
        assert!(!events
            .iter()
            .any(|event| matches!(event.event, LoopRuntimeEvent::TaskCanceled { .. })));
    }

    #[test]
    fn fake_executor_pending_confirmation_blocks_without_auto_accepting_or_completing() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        append_approved_policy_allowed_task(&journal, "task-fake-confirmation");
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000)
            .with_fake_executor_fixture(FakeOwnerExecutorFixture::pending_confirmation(
                "confirm-write-1",
            ));

        runner.run_once_at(&journal, &projection, 1_000).unwrap();

        let task = projection
            .load_or_rebuild(&journal)
            .unwrap()
            .find("task-fake-confirmation")
            .cloned()
            .unwrap();
        let owner_run = task.headless_owner_runs.first().unwrap();
        assert_eq!(
            owner_run.executor_kind,
            HeadlessOwnerExecutorKind::FakeExecutor
        );
        assert_eq!(owner_run.state, HeadlessOwnerRunState::WaitingForInput);
        assert!(owner_run
            .waiting_reason
            .as_deref()
            .unwrap()
            .contains("pending confirmation confirm-write-1"));
        assert!(owner_run
            .waiting_reason
            .as_deref()
            .unwrap()
            .contains("not auto-accepted"));
        assert!(owner_run.cancellation_reason.is_none());

        let events = journal.load_all().unwrap();
        let states = owner_state_events(&events);
        assert_eq!(
            states.iter().map(|state| state.0).collect::<Vec<_>>(),
            vec![
                HeadlessOwnerRunState::LeaseAcquired,
                HeadlessOwnerRunState::FakeRunning,
                HeadlessOwnerRunState::WaitingForInput,
            ]
        );
    }

    #[test]
    fn fake_executor_pending_tool_call_blocks_without_auto_accepting_or_completing() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        append_approved_policy_allowed_task(&journal, "task-fake-tool-call");
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000)
            .with_fake_executor_fixture(FakeOwnerExecutorFixture::pending_tool_call(
                "tool-call-shell-1",
            ));

        runner.run_once_at(&journal, &projection, 1_000).unwrap();

        let task = projection
            .load_or_rebuild(&journal)
            .unwrap()
            .find("task-fake-tool-call")
            .cloned()
            .unwrap();
        let owner_run = task.headless_owner_runs.first().unwrap();
        assert_eq!(
            owner_run.executor_kind,
            HeadlessOwnerExecutorKind::FakeExecutor
        );
        assert_eq!(owner_run.state, HeadlessOwnerRunState::WaitingForInput);
        assert!(owner_run
            .waiting_reason
            .as_deref()
            .unwrap()
            .contains("pending tool call tool-call-shell-1"));
        assert!(owner_run
            .waiting_reason
            .as_deref()
            .unwrap()
            .contains("not auto-accepted"));
        let events = journal.load_all().unwrap();
        assert!(!owner_state_events(&events)
            .iter()
            .any(|state| state.0 == HeadlessOwnerRunState::Completed));
    }

    #[test]
    fn fake_executor_records_interrupted_cancelled_and_expired_terminal_states() {
        for (task_id, fixture, expected_state, expected_reason) in [
            (
                "task-fake-interrupted",
                FakeOwnerExecutorFixture::interrupted("manual pause requested"),
                HeadlessOwnerRunState::Interrupted,
                "manual pause requested",
            ),
            (
                "task-fake-cancelled",
                FakeOwnerExecutorFixture::cancelled("user cancelled pending run"),
                HeadlessOwnerRunState::Cancelled,
                "user cancelled pending run",
            ),
            (
                "task-fake-expired",
                FakeOwnerExecutorFixture::expired("lease expired while waiting"),
                HeadlessOwnerRunState::Expired,
                "lease expired while waiting",
            ),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
            let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
            append_approved_policy_allowed_task(&journal, task_id);
            let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000)
                .with_fake_executor_fixture(fixture);

            runner.run_once_at(&journal, &projection, 1_000).unwrap();

            let task = projection
                .load_or_rebuild(&journal)
                .unwrap()
                .find(task_id)
                .cloned()
                .unwrap();
            let owner_run = task.headless_owner_runs.first().unwrap();
            assert_eq!(
                owner_run.executor_kind,
                HeadlessOwnerExecutorKind::FakeExecutor
            );
            assert_eq!(owner_run.state, expected_state);
            assert_eq!(
                owner_run.cancellation_reason.as_deref(),
                Some(expected_reason)
            );
            if expected_state == HeadlessOwnerRunState::Expired {
                assert!(owner_run
                    .evidence_refs
                    .contains(&"fake-executor:waiting-evidence".to_string()));
            }
            let events = journal.load_all().unwrap();
            assert!(owner_state_events(&events)
                .iter()
                .any(|state| state.0 == HeadlessOwnerRunState::FakeRunning));
        }
    }

    #[test]
    fn fake_executor_retry_from_same_stale_pending_view_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        append_approved_policy_allowed_task(&journal, "task-fake-idempotent");
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000)
            .with_fake_executor_fixture(FakeOwnerExecutorFixture::completed());
        let stale_projection = projection.load_or_rebuild(&journal).unwrap();
        let stale_events = journal.load_all().unwrap();
        let stale_task = stale_projection
            .find("task-fake-idempotent")
            .cloned()
            .unwrap();

        let mut first_summary = LoopRunnerRunSummary::default();
        runner
            .process_pending_task(
                &journal,
                &projection,
                &stale_task,
                &stale_events,
                1_000,
                &mut first_summary,
            )
            .unwrap();
        let first_events = journal.load_all().unwrap();
        let first_owner = first_events
            .iter()
            .find_map(|event| match &event.event {
                LoopRuntimeEvent::HeadlessOwnerRunRequested { owner_run, .. } => Some(owner_run),
                _ => None,
            })
            .unwrap()
            .clone();

        let mut retry_summary = LoopRunnerRunSummary::default();
        runner
            .process_pending_task(
                &journal,
                &projection,
                &stale_task,
                &stale_events,
                2_000,
                &mut retry_summary,
            )
            .unwrap();

        let events = journal.load_all().unwrap();
        let owner_requests: Vec<_> = events
            .iter()
            .filter(|event| {
                matches!(
                    event.event,
                    LoopRuntimeEvent::HeadlessOwnerRunRequested { .. }
                )
            })
            .collect();
        let owner_states = owner_state_events(&events);
        assert_eq!(owner_requests.len(), 1);
        assert_eq!(owner_states.len(), 3);
        let task = projection
            .load_or_rebuild(&journal)
            .unwrap()
            .find("task-fake-idempotent")
            .cloned()
            .unwrap();
        let owner_run = task.headless_owner_runs.first().unwrap();
        assert_eq!(task.headless_owner_runs.len(), 1);
        assert_eq!(owner_run.owner_run_id, first_owner.owner_run_id);
        assert_eq!(owner_run.lease_id, first_owner.lease_id);
        assert_eq!(owner_run.attempt, first_owner.attempt);
        assert_eq!(owner_run.state, HeadlessOwnerRunState::Completed);
    }

    fn task_created_event_for_test(task: LoopTaskRecord) -> LoopEventEnvelope {
        LoopEventEnvelope {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: format!("event-{}-created", task.id),
            task_id: task.id.clone(),
            sequence: 0,
            event: LoopRuntimeEvent::TaskCreated { task },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id: None,
            causation_id: None,
            idempotency_key: None,
            created_at_ms: 1,
        }
    }

    fn budget_snapshot_event_for_test(
        task_id: &str,
        snapshot: BudgetSnapshot,
    ) -> LoopEventEnvelope {
        LoopEventEnvelope {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: format!("event-{task_id}-budget"),
            task_id: task_id.to_string(),
            sequence: 0,
            event: LoopRuntimeEvent::BudgetSnapshotRecorded {
                task_id: task_id.to_string(),
                snapshot,
            },
            actor: LoopActor::Runner {
                runner_id: "test-runner".to_string(),
            },
            lease_id: None,
            attempt: None,
            correlation_id: None,
            causation_id: None,
            idempotency_key: None,
            created_at_ms: 2,
        }
    }

    fn owner_run_for_test(task_id: &str, lease_id: &str, attempt: u32) -> HeadlessOwnerRun {
        HeadlessOwnerRun {
            owner_run_id: format!("owner-run:{task_id}:{attempt}:dry-run"),
            task_id: task_id.to_string(),
            session_id: Some("desktop-session-1".to_string()),
            lease_id: lease_id.to_string(),
            attempt,
            state: HeadlessOwnerRunState::LeaseAcquired,
            snapshot_source: HeadlessOwnerSnapshotSource::CurrentDesktopSession,
            snapshot_ref: Some("desktop-session-1".to_string()),
            human_gate_id: format!("human-gate:{task_id}:{attempt}:headless-resume"),
            policy_decision_id: format!("policy:{task_id}:{attempt}:dry-run"),
            budget_snapshot_id: format!("budget:{task_id}:{attempt}:preflight"),
            idempotency_key: format!("runner:runner-1:owner-dry-run:{task_id}:{attempt}"),
            correlation_id: format!("runner:runner-1:task:{task_id}:attempt:{attempt}"),
            causation_id: Some(format!("event-{task_id}-started")),
            requested_by: "runner:runner-1".to_string(),
            requested_at_ms: 1_000,
            heartbeat_at_ms: Some(1_000),
            expires_at_ms: 61_000,
            cancellation_reason: None,
            waiting_reason: None,
            executor_kind: HeadlessOwnerExecutorKind::DryRun,
            evidence_refs: vec![
                format!("human-gate:{task_id}:{attempt}:headless-resume"),
                format!("policy:{task_id}:{attempt}:dry-run"),
                format!("budget:{task_id}:{attempt}:preflight"),
                format!("runner:runner-1:owner-dry-run:{task_id}:{attempt}"),
                format!("runner:runner-1:task:{task_id}:attempt:{attempt}"),
            ],
        }
    }

    fn headless_owner_run_requested_event_for_test(
        owner_run: HeadlessOwnerRun,
    ) -> LoopEventEnvelope {
        LoopEventEnvelope {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: format!("event-{}-owner-requested", owner_run.task_id),
            task_id: owner_run.task_id.clone(),
            sequence: 0,
            event: LoopRuntimeEvent::HeadlessOwnerRunRequested {
                task_id: owner_run.task_id.clone(),
                owner_run: owner_run.clone(),
            },
            actor: LoopActor::Runner {
                runner_id: "runner-1".to_string(),
            },
            lease_id: Some(owner_run.lease_id.clone()),
            attempt: Some(owner_run.attempt),
            correlation_id: Some(owner_run.correlation_id.clone()),
            causation_id: owner_run.causation_id.clone(),
            idempotency_key: Some(owner_run.idempotency_key.clone()),
            created_at_ms: owner_run.requested_at_ms,
        }
    }

    fn append_approved_policy_allowed_task(journal: &LoopEventJournal, task_id: &str) {
        let mut task = LoopTaskRecord::new_for_test(task_id, "approved fake executor fixture");
        task.session_id = Some("desktop-session-1".to_string());
        task.policy.allow_service_lifecycle = true;
        journal.append(task_created_event_for_test(task)).unwrap();
        journal
            .append(LoopEventEnvelope::headless_resume_approval_recorded(
                task_id.to_string(),
                crate::loop_runtime::HeadlessResumeApproval {
                    task_id: task_id.to_string(),
                    approved_by: "human-reviewer".to_string(),
                    approved_at_ms: 10,
                    scope: "task".to_string(),
                    expires_at_ms: 60_010,
                },
                Some("test".to_string()),
                Some(format!("headless:{task_id}")),
            ))
            .unwrap();
    }

    fn owner_state_events(
        events: &[LoopEventEnvelope],
    ) -> Vec<(HeadlessOwnerRunState, &LoopEventEnvelope)> {
        events
            .iter()
            .filter_map(|event| match &event.event {
                LoopRuntimeEvent::HeadlessOwnerRunStateRecorded { state, .. } => {
                    Some((*state, event))
                }
                _ => None,
            })
            .collect()
    }

    fn stale_started_event(
        task_id: &str,
        lease_id: &str,
        acquired_at_ms: u64,
        expires_at_ms: u64,
    ) -> LoopEventEnvelope {
        LoopEventEnvelope {
            schema_version: crate::loop_runtime::LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: format!("event-{task_id}-started"),
            task_id: task_id.to_string(),
            sequence: 0,
            event: LoopRuntimeEvent::TaskStarted {
                task_id: task_id.to_string(),
                lease: LoopTaskLease {
                    lease_id: lease_id.to_string(),
                    owner_pid: 99,
                    acquired_at_ms,
                    expires_at_ms,
                    heartbeat_at_ms: acquired_at_ms,
                },
            },
            actor: crate::loop_runtime::LoopActor::Runner {
                runner_id: "old-runner".to_string(),
            },
            lease_id: Some(lease_id.to_string()),
            attempt: Some(1),
            correlation_id: None,
            causation_id: None,
            idempotency_key: None,
            created_at_ms: acquired_at_ms,
        }
    }
}
