use std::sync::Arc;
use std::time::Duration;

use crate::loop_runtime::types::{new_loop_event_id, now_millis};
use crate::loop_runtime::{
    evaluate_completion, LoopActor, LoopEventEnvelope, LoopEventJournal, LoopRuntimeEvent,
    LoopTaskLease, LoopTaskProjection, LoopTaskProjectionStore, LoopTaskRecord, LoopTaskStatus,
    LOOP_RUNTIME_SCHEMA_VERSION,
};

pub const LOOP_RUNNER_POLL_INTERVAL_SECS: u64 = 5;
pub const LOOP_TASK_LEASE_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
pub const DEFAULT_LOOP_RUNNER_ID: &str = "gateway-loop-runner";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopTaskRunner {
    runner_id: String,
    owner_pid: u32,
    lease_timeout_ms: u64,
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
}

impl LoopTaskRunner {
    pub fn gateway_default() -> Self {
        Self {
            runner_id: DEFAULT_LOOP_RUNNER_ID.to_string(),
            owner_pid: std::process::id(),
            lease_timeout_ms: LOOP_TASK_LEASE_TIMEOUT_MS,
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
            journal.append_idempotent(event)?;
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
            Some(format!(
                "runner:{}:start:{}:{attempt}",
                self.runner_id, task.id
            )),
            now_ms,
        );
        let start = journal.append_idempotent(start)?.event;
        summary.claimed_tasks += 1;

        let wait = self.envelope(
            task.id.clone(),
            LoopRuntimeEvent::TaskWaitingForInput {
                task_id: task.id.clone(),
                reason: waiting_reason(task),
                waiting_at_ms: now_ms,
            },
            Some(lease.lease_id),
            Some(attempt),
            Some(start.event_id),
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

    fn lease(&self, now_ms: u64) -> LoopTaskLease {
        LoopTaskLease {
            lease_id: new_loop_event_id().replace("evt-", "lease-"),
            owner_pid: self.owner_pid,
            acquired_at_ms: now_ms,
            expires_at_ms: now_ms.saturating_add(self.lease_timeout_ms),
            heartbeat_at_ms: now_ms,
        }
    }

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
        }
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

fn waiting_reason(task: &LoopTaskRecord) -> String {
    if task.headless_resume_approval.is_some() {
        return "Gateway loop runner sees headless resume approval is recorded for this task, but Task 4A only records policy intent; no headless AgentSession was created and autonomous resume remains disabled."
            .to_string();
    }
    if let Some(session_id) = task.session_id.as_deref() {
        format!(
            "Gateway loop runner is waiting for existing desktop session {session_id} to accept the next step; headless autonomous resume is disabled by default, requires durable human approval, and no headless AgentSession was created."
        )
    } else {
        "Gateway loop runner requires an existing desktop session owner before execution; headless autonomous resume is disabled by default, requires durable human approval, and no headless AgentSession was created."
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::LoopTaskRunner;
    use crate::loop_runtime::{
        LoopEventEnvelope, LoopEventJournal, LoopRuntimeEvent, LoopTaskLease,
        LoopTaskProjectionStore, LoopTaskRecord, LoopTaskStatus,
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

        let summary = runner.run_once(&journal, &projection).unwrap();

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
        assert_eq!(
            task.completion_result.as_ref().unwrap().reasons,
            vec!["task_waiting_for_input"]
        );

        let events = journal.load_all().unwrap();
        assert!(matches!(
            events[1].event,
            LoopRuntimeEvent::TaskStarted { .. }
        ));
        assert!(matches!(
            events[2].event,
            LoopRuntimeEvent::TaskWaitingForInput { .. }
        ));
        let LoopRuntimeEvent::TaskWaitingForInput { reason, .. } = &events[2].event else {
            unreachable!("expected waiting event");
        };
        assert!(reason.contains("headless autonomous resume is disabled by default"));
        assert!(reason.contains("durable human approval"));
        assert!(reason.contains("no headless AgentSession was created"));
        assert!(matches!(
            events[3].event,
            LoopRuntimeEvent::CompletionEvaluated { .. }
        ));
    }

    #[test]
    fn runner_still_waits_without_headless_agent_session_after_approval_recorded() {
        let temp = tempfile::tempdir().unwrap();
        let journal = LoopEventJournal::new(temp.path().join("loop-events.jsonl"));
        let projection = LoopTaskProjectionStore::new(temp.path().join("loop-tasks.json"));
        journal
            .append(LoopEventEnvelope::task_created_for_test(
                "task-approved",
                "approved but bounded",
            ))
            .unwrap();
        journal
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
            .unwrap();
        let runner = LoopTaskRunner::new_for_test("runner-1", 4321, 60_000);

        let summary = runner.run_once(&journal, &projection).unwrap();

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

        let events = journal.load_all().unwrap();
        let waiting = events
            .iter()
            .find_map(|event| match &event.event {
                LoopRuntimeEvent::TaskWaitingForInput { reason, .. } => Some(reason),
                _ => None,
            })
            .expect("waiting event");
        assert!(waiting.contains("headless resume approval is recorded"));
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
