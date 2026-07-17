//! Runtime status and dashboard snapshot builders.

use super::triggers::*;
use super::*;

pub(super) fn handle_runtime_status(state: &GatewayState, id: String) -> GatewayReply {
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(build_runtime_status(state)).unwrap(),
    })
}

pub(super) fn handle_dashboard_snapshot(state: &GatewayState, id: String) -> GatewayReply {
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(build_dashboard_snapshot(state)).unwrap(),
    })
}

pub(super) fn build_dashboard_snapshot(state: &GatewayState) -> GatewayDashboardSnapshot {
    let loop_load = load_loop_tasks_and_stats(state);
    let status = build_runtime_status_with_loop_stats(
        state,
        &loop_load.tasks,
        loop_load.stats,
        loop_load.replay.clone(),
    );
    let sessions = state.list_sessions();
    let queued_triggers = state.trigger_store.list();
    let recent_runs = status.recent_runs.clone();
    let recent_session_inputs = status.recent_session_inputs.clone();
    let event_log =
        build_dashboard_event_log(&recent_runs, &recent_session_inputs, &status.runtime_tasks);

    GatewayDashboardSnapshot {
        ok: status.ok,
        generated_at_ms: now_millis(),
        status,
        loop_tasks: loop_load.tasks,
        sessions,
        queued_triggers,
        recent_runs,
        recent_session_inputs,
        event_log,
    }
}

pub(super) fn build_dashboard_event_log(
    runs: &[TriggerRunRecord],
    session_inputs: &[SessionInputCompletionRecord],
    runtime_tasks: &[GatewayRuntimeTaskStatus],
) -> Vec<GatewayDashboardEventLogEntry> {
    let mut entries = Vec::with_capacity(runs.len() + session_inputs.len() + runtime_tasks.len());
    for run in runs {
        entries.push(GatewayDashboardEventLogEntry {
            kind: "trigger_run".to_string(),
            id: run.id.clone(),
            message: format!("{}: {}", run.status, run.message),
            at_ms: run.ended_at_ms.max(run.started_at_ms),
            session_id: run.session_id.clone(),
        });
    }
    for input in session_inputs {
        entries.push(GatewayDashboardEventLogEntry {
            kind: "session_input_completed".to_string(),
            id: input.input_id.clone(),
            message: input.message_preview.clone(),
            at_ms: input.completed_at_ms.max(input.received_at_ms),
            session_id: Some(input.session_id.clone()),
        });
    }
    for task in runtime_tasks {
        let Some(error) = task.last_error.as_deref() else {
            continue;
        };
        entries.push(GatewayDashboardEventLogEntry {
            kind: "runtime_task_failed".to_string(),
            id: task.name.clone(),
            message: error.to_string(),
            at_ms: task.last_started_at_ms.unwrap_or_default(),
            session_id: None,
        });
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.at_ms));
    entries.truncate(50);
    entries
}

pub(super) fn build_runtime_status(state: &GatewayState) -> GatewayRuntimeStatus {
    let loop_load = load_loop_tasks_and_stats(state);
    build_runtime_status_with_loop_stats(state, &loop_load.tasks, loop_load.stats, loop_load.replay)
}

pub(super) struct LoopTaskRuntimeLoad {
    tasks: Vec<LoopTaskRecord>,
    stats: LoopRunnerQueueStats,
    replay: RuntimeReplayHealth,
}

pub(super) fn load_loop_tasks_and_stats(state: &GatewayState) -> LoopTaskRuntimeLoad {
    match state
        .loop_task_projection_store
        .load_or_rebuild(&state.loop_event_journal)
    {
        Ok(projection) => {
            let task_count = projection.tasks.len();
            let stats = LoopTaskRunner::queue_stats(&projection, now_millis());
            LoopTaskRuntimeLoad {
                tasks: projection.tasks,
                stats,
                replay: RuntimeReplayHealth {
                    ok: true,
                    task_count,
                    message: "Loop projection replay succeeded.".to_string(),
                },
            }
        }
        Err(error) => {
            let message = format!("Loop projection replay failed: {error}");
            log::warn!("failed to load loop task projection for runtime status: {error}");
            LoopTaskRuntimeLoad {
                tasks: Vec::new(),
                stats: LoopRunnerQueueStats::default(),
                replay: RuntimeReplayHealth {
                    ok: false,
                    task_count: 0,
                    message,
                },
            }
        }
    }
}

pub(super) fn build_runtime_status_with_loop_stats(
    state: &GatewayState,
    loop_tasks: &[LoopTaskRecord],
    loop_stats: LoopRunnerQueueStats,
    loop_replay: RuntimeReplayHealth,
) -> GatewayRuntimeStatus {
    let triggers = state.trigger_store.list();
    let runs = state.trigger_run_store.list();
    let pending_triggers = count_pending_triggers(&triggers);
    let claimed_triggers = triggers.len().saturating_sub(pending_triggers);
    let dead_letter_runs = runs
        .iter()
        .filter(|run| run.status == "dead_letter")
        .count();
    let runtime_tasks = state.runtime_tasks();
    let loop_runner = loop_runner_status(&runtime_tasks).to_string();
    let degraded_mode = gateway_degraded_mode_status(&runtime_tasks);
    let active_sessions = state.active_sessions();
    let pending_session_inputs = state.session_input_store.list().len();
    let observed_runtime_tasks = observed_runtime_tasks(&runtime_tasks);
    let runtime_health = RuntimeHealthSnapshot::from_gateway_input(RuntimeHealthSnapshotInput {
        generated_at_ms: now_millis(),
        active_sessions,
        loop_tasks,
        loop_stats,
        pending_triggers,
        claimed_triggers,
        pending_session_inputs,
        dead_letter_runs,
        runtime_tasks: &observed_runtime_tasks,
        last_replay: loop_replay,
    });
    let message = if degraded_mode.active {
        format!(
            "Gateway runtime is reachable, but degraded mode is active: {}",
            degraded_mode.reason
        )
    } else {
        "Gateway runtime is reachable.".to_string()
    };

    GatewayRuntimeStatus {
        ok: true,
        message,
        ownership: default_gateway_ownership_capability(),
        degraded_mode,
        runtime_health,
        uptime_seconds: state.uptime_seconds(),
        active_sessions,
        pending_triggers,
        pending_session_inputs,
        loop_runner,
        pending_loop_tasks: loop_stats.pending_loop_tasks,
        running_loop_tasks: loop_stats.running_loop_tasks,
        stale_loop_task_leases: loop_stats.stale_loop_task_leases,
        orphaned_loop_tasks: loop_stats.orphaned_loop_tasks,
        interrupted_loop_tasks: loop_stats.interrupted_loop_tasks,
        recoverable_loop_tasks: loop_stats.recoverable_loop_tasks,
        dry_run_headless_owner_runs: loop_stats.dry_run_headless_owner_runs,
        waiting_headless_owner_runs: loop_stats.waiting_headless_owner_runs,
        denied_headless_owner_runs: loop_stats.denied_headless_owner_runs,
        expired_headless_owner_runs: loop_stats.expired_headless_owner_runs,
        claimed_triggers,
        dead_letter_runs,
        recent_runs: runs.into_iter().take(20).collect(),
        recent_session_inputs: state.session_input_store.recent_completions(20),
        runtime_tasks,
    }
}

pub(super) fn observed_runtime_tasks(
    tasks: &[GatewayRuntimeTaskStatus],
) -> Vec<RuntimeObservedTask> {
    tasks
        .iter()
        .map(|task| RuntimeObservedTask {
            name: task.name.clone(),
            running: task.running,
            failed: task.last_error.is_some(),
        })
        .collect()
}

pub(super) fn gateway_degraded_mode_status(
    runtime_tasks: &[GatewayRuntimeTaskStatus],
) -> GatewayDegradedModeStatus {
    let Some(reason) = runtime_task_failure_reason(runtime_tasks) else {
        return default_gateway_degraded_mode_status();
    };

    GatewayDegradedModeStatus {
        active: true,
        reason,
        fallback: "desktop_runtime".to_string(),
        input_policy:
            "Queued session input stays pending until the owning desktop runtime accepts it."
                .to_string(),
        confirmation_policy: "Pending confirmations stay with the owning desktop runtime."
            .to_string(),
        recovery_command: default_gateway_degraded_recovery_command(),
    }
}

pub(super) fn runtime_task_failure_reason(
    runtime_tasks: &[GatewayRuntimeTaskStatus],
) -> Option<String> {
    runtime_tasks.iter().find_map(|task| {
        task.last_error
            .as_ref()
            .map(|error| format!("runtime task '{}' failed: {}", task.name, error.trim()))
    })
}

pub(super) fn loop_runner_status(runtime_tasks: &[GatewayRuntimeTaskStatus]) -> &'static str {
    let Some(task) = runtime_tasks
        .iter()
        .find(|task| task.name == LOOP_RUNNER_TASK)
    else {
        return "stopped";
    };
    if task.last_error.is_some() {
        "failed"
    } else if task.running {
        "started"
    } else {
        "stopped"
    }
}

pub(super) fn default_runtime_task_map() -> HashMap<String, GatewayRuntimeTaskStatus> {
    [
        WEBHOOK_LISTENER_TASK,
        TRIGGER_RUNNER_TASK,
        LOOP_RUNNER_TASK,
        SCHEDULER_TICK_TASK,
        DASHBOARD_HTTP_TASK,
    ]
    .into_iter()
    .map(|name| {
        (
            name.to_string(),
            GatewayRuntimeTaskStatus {
                name: name.to_string(),
                running: false,
                last_started_at_ms: None,
                last_error: None,
            },
        )
    })
    .collect()
}

pub(super) fn ordered_runtime_tasks(
    tasks: HashMap<String, GatewayRuntimeTaskStatus>,
) -> Vec<GatewayRuntimeTaskStatus> {
    let mut ordered = Vec::with_capacity(tasks.len());
    for name in [
        WEBHOOK_LISTENER_TASK,
        TRIGGER_RUNNER_TASK,
        LOOP_RUNNER_TASK,
        SCHEDULER_TICK_TASK,
        DASHBOARD_HTTP_TASK,
    ] {
        if let Some(status) = tasks.get(name) {
            ordered.push(status.clone());
        }
    }

    let mut extras = tasks
        .into_iter()
        .filter(|(name, _)| {
            ![
                WEBHOOK_LISTENER_TASK,
                TRIGGER_RUNNER_TASK,
                LOOP_RUNNER_TASK,
                SCHEDULER_TICK_TASK,
                DASHBOARD_HTTP_TASK,
            ]
            .contains(&name.as_str())
        })
        .map(|(_, status)| status)
        .collect::<Vec<_>>();
    extras.sort_by(|a, b| a.name.cmp(&b.name));
    ordered.extend(extras);
    ordered
}
