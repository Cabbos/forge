use serde::{Deserialize, Serialize};

use crate::loop_runtime::runner::LoopRunnerQueueStats;
use crate::loop_runtime::{LoopTaskRecord, LoopTaskRecoveryKind, LoopTaskStatus};

const WEBHOOK_LISTENER_TASK: &str = "webhook_listener";
const TRIGGER_RUNNER_TASK: &str = "trigger_runner";
const LOOP_RUNNER_TASK: &str = "loop_runner";
const SCHEDULER_TICK_TASK: &str = "scheduler_tick";
const DASHBOARD_HTTP_TASK: &str = "dashboard_http";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeHealthSnapshot {
    pub ok: bool,
    pub generated_at_ms: u64,
    pub active_runs: RuntimeActiveRunHealth,
    pub pending_confirmations: RuntimePendingConfirmationHealth,
    pub loop_tasks: RuntimeLoopTaskHealth,
    pub gateway_queue: RuntimeGatewayQueueHealth,
    pub scheduler_queue: RuntimeSchedulerQueueHealth,
    pub runtime_tasks: RuntimeTaskHealth,
    pub last_replay: RuntimeReplayHealth,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_recovery_action: Option<RuntimeRecoveryActionSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeActiveRunHealth {
    pub active_sessions: usize,
    pub running_loop_tasks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePendingConfirmationHealth {
    pub count: usize,
    pub available: bool,
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeLoopTaskHealth {
    pub total: usize,
    pub pending: usize,
    pub running: usize,
    pub waiting_for_input: usize,
    pub waiting_for_review: usize,
    pub completed: usize,
    pub failed: usize,
    pub canceled: usize,
    pub interrupted: usize,
    pub stale_leases: usize,
    pub orphaned: usize,
    pub recoverable: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeGatewayQueueHealth {
    pub pending_triggers: usize,
    pub claimed_triggers: usize,
    pub pending_session_inputs: usize,
    pub dead_letter_runs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeSchedulerQueueHealth {
    pub running: bool,
    pub pending_tasks: usize,
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeTaskHealth {
    pub total: usize,
    pub running: usize,
    pub failed: usize,
    pub webhook_listener_running: bool,
    pub trigger_runner_running: bool,
    pub loop_runner_running: bool,
    pub scheduler_tick_running: bool,
    pub dashboard_http_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeReplayHealth {
    pub ok: bool,
    pub task_count: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeRecoveryActionSnapshot {
    pub task_id: String,
    pub kind: LoopTaskRecoveryKind,
    pub reason: String,
    pub notice: String,
    pub recorded_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeObservedTask {
    pub name: String,
    pub running: bool,
    pub failed: bool,
}

#[derive(Debug, Clone)]
pub struct RuntimeHealthSnapshotInput<'a> {
    pub generated_at_ms: u64,
    pub active_sessions: usize,
    pub loop_tasks: &'a [LoopTaskRecord],
    pub loop_stats: LoopRunnerQueueStats,
    pub pending_triggers: usize,
    pub claimed_triggers: usize,
    pub pending_session_inputs: usize,
    pub dead_letter_runs: usize,
    pub runtime_tasks: &'a [RuntimeObservedTask],
    pub last_replay: RuntimeReplayHealth,
}

pub fn default_runtime_health_snapshot() -> RuntimeHealthSnapshot {
    RuntimeHealthSnapshot {
        ok: false,
        generated_at_ms: 0,
        active_runs: RuntimeActiveRunHealth {
            active_sessions: 0,
            running_loop_tasks: 0,
        },
        pending_confirmations: RuntimePendingConfirmationHealth {
            count: 0,
            available: false,
            source: "gateway_status_unavailable".to_string(),
        },
        loop_tasks: RuntimeLoopTaskHealth::default(),
        gateway_queue: RuntimeGatewayQueueHealth {
            pending_triggers: 0,
            claimed_triggers: 0,
            pending_session_inputs: 0,
            dead_letter_runs: 0,
        },
        scheduler_queue: RuntimeSchedulerQueueHealth {
            running: false,
            pending_tasks: 0,
            source: "gateway_status_unavailable".to_string(),
        },
        runtime_tasks: RuntimeTaskHealth::default(),
        last_replay: RuntimeReplayHealth {
            ok: false,
            task_count: 0,
            message: "Runtime health snapshot is unavailable.".to_string(),
        },
        last_recovery_action: None,
    }
}

impl RuntimeHealthSnapshot {
    pub fn from_gateway_input(input: RuntimeHealthSnapshotInput<'_>) -> Self {
        let runtime_tasks = RuntimeTaskHealth::from_tasks(input.runtime_tasks);
        let scheduler_queue = RuntimeSchedulerQueueHealth {
            running: runtime_task_running(input.runtime_tasks, SCHEDULER_TICK_TASK),
            pending_tasks: 0,
            source: "gateway_runtime_task".to_string(),
        };
        Self {
            ok: input.last_replay.ok,
            generated_at_ms: input.generated_at_ms,
            active_runs: RuntimeActiveRunHealth {
                active_sessions: input.active_sessions,
                running_loop_tasks: input.loop_stats.running_loop_tasks,
            },
            pending_confirmations: RuntimePendingConfirmationHealth {
                count: 0,
                available: false,
                source: "gateway_status_cannot_read_desktop_pending_confirms".to_string(),
            },
            loop_tasks: RuntimeLoopTaskHealth::from_tasks(input.loop_tasks, input.loop_stats),
            gateway_queue: RuntimeGatewayQueueHealth {
                pending_triggers: input.pending_triggers,
                claimed_triggers: input.claimed_triggers,
                pending_session_inputs: input.pending_session_inputs,
                dead_letter_runs: input.dead_letter_runs,
            },
            scheduler_queue,
            runtime_tasks,
            last_replay: input.last_replay,
            last_recovery_action: latest_recovery_action(input.loop_tasks),
        }
    }
}

impl RuntimeLoopTaskHealth {
    fn from_tasks(tasks: &[LoopTaskRecord], stats: LoopRunnerQueueStats) -> Self {
        let mut health = Self {
            total: tasks.len(),
            pending: stats.pending_loop_tasks,
            running: stats.running_loop_tasks,
            stale_leases: stats.stale_loop_task_leases,
            orphaned: stats.orphaned_loop_tasks,
            interrupted: stats.interrupted_loop_tasks,
            recoverable: stats.recoverable_loop_tasks,
            ..Self::default()
        };
        for task in tasks {
            match task.status {
                LoopTaskStatus::WaitingForInput => health.waiting_for_input += 1,
                LoopTaskStatus::WaitingForReview => health.waiting_for_review += 1,
                LoopTaskStatus::Completed => health.completed += 1,
                LoopTaskStatus::Failed => health.failed += 1,
                LoopTaskStatus::Canceled => health.canceled += 1,
                _ => {}
            }
        }
        health
    }
}

impl RuntimeTaskHealth {
    fn from_tasks(tasks: &[RuntimeObservedTask]) -> Self {
        Self {
            total: tasks.len(),
            running: tasks.iter().filter(|task| task.running).count(),
            failed: tasks.iter().filter(|task| task.failed).count(),
            webhook_listener_running: runtime_task_running(tasks, WEBHOOK_LISTENER_TASK),
            trigger_runner_running: runtime_task_running(tasks, TRIGGER_RUNNER_TASK),
            loop_runner_running: runtime_task_running(tasks, LOOP_RUNNER_TASK),
            scheduler_tick_running: runtime_task_running(tasks, SCHEDULER_TICK_TASK),
            dashboard_http_running: runtime_task_running(tasks, DASHBOARD_HTTP_TASK),
        }
    }
}

fn runtime_task_running(tasks: &[RuntimeObservedTask], name: &str) -> bool {
    tasks
        .iter()
        .any(|task| task.name == name && task.running && !task.failed)
}

fn latest_recovery_action(tasks: &[LoopTaskRecord]) -> Option<RuntimeRecoveryActionSnapshot> {
    tasks
        .iter()
        .filter_map(|task| {
            task.recovery_state
                .as_ref()
                .map(|recovery| (task, recovery))
        })
        .max_by_key(|(_, recovery)| recovery.recorded_at_ms)
        .map(|(task, recovery)| RuntimeRecoveryActionSnapshot {
            task_id: task.id.clone(),
            kind: recovery.kind,
            reason: recovery.reason.clone(),
            notice: recovery.notice.clone(),
            recorded_at_ms: recovery.recorded_at_ms,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loop_runtime::{LoopTaskRecoveryState, LoopTaskStatus};

    #[test]
    fn runtime_health_snapshot_counts_loop_gateway_and_recovery_facts() {
        let pending = LoopTaskRecord::new_for_test("task-pending", "pending");
        let mut running = LoopTaskRecord::new_for_test("task-running", "running");
        running.status = LoopTaskStatus::Running;
        running.recovery_state = Some(LoopTaskRecoveryState::interrupted(
            "stale lease lease-1 expired at 10",
            20,
            Some("event-1".to_string()),
        ));

        let snapshot = RuntimeHealthSnapshot::from_gateway_input(RuntimeHealthSnapshotInput {
            generated_at_ms: 100,
            active_sessions: 2,
            loop_tasks: &[pending, running],
            loop_stats: LoopRunnerQueueStats {
                pending_loop_tasks: 1,
                running_loop_tasks: 1,
                stale_loop_task_leases: 1,
                orphaned_loop_tasks: 1,
                recoverable_loop_tasks: 1,
                ..LoopRunnerQueueStats::default()
            },
            pending_triggers: 3,
            claimed_triggers: 1,
            pending_session_inputs: 2,
            dead_letter_runs: 1,
            runtime_tasks: &[RuntimeObservedTask {
                name: SCHEDULER_TICK_TASK.to_string(),
                running: true,
                failed: false,
            }],
            last_replay: RuntimeReplayHealth {
                ok: true,
                task_count: 2,
                message: "Loop projection replay succeeded.".to_string(),
            },
        });

        assert!(snapshot.ok);
        assert_eq!(snapshot.active_runs.active_sessions, 2);
        assert_eq!(snapshot.loop_tasks.pending, 1);
        assert_eq!(snapshot.loop_tasks.running, 1);
        assert_eq!(snapshot.loop_tasks.stale_leases, 1);
        assert_eq!(snapshot.loop_tasks.orphaned, 1);
        assert_eq!(snapshot.loop_tasks.recoverable, 1);
        assert_eq!(snapshot.gateway_queue.pending_triggers, 3);
        assert!(snapshot.scheduler_queue.running);
        assert_eq!(
            snapshot
                .last_recovery_action
                .as_ref()
                .map(|action| action.task_id.as_str()),
            Some("task-running")
        );
    }
}
