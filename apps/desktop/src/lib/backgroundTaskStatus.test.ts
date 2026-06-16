import { describe, it } from "node:test";
import assert from "node:assert";
import { deriveBackgroundTaskStatus } from "./backgroundTaskStatus.ts";
import type { AgentA2AProjection, AgentA2ATaskProjection } from "./protocol.ts";
import type { SchedulerListPayload, ScheduledTask } from "./tauri.ts";
import type { RuntimeHealthAlert } from "../store/types.ts";

function task(overrides: Partial<AgentA2ATaskProjection> = {}): AgentA2ATaskProjection {
  return {
    task_id: overrides.task_id ?? "task-1",
    agent_id: overrides.agent_id ?? "agent-1",
    role: overrides.role ?? "worker",
    execution_mode: overrides.execution_mode ?? "worktree_worker",
    status: overrides.status ?? "running",
    title: overrides.title ?? "Background worker",
    messages: overrides.messages ?? [],
    latest_message: overrides.latest_message ?? null,
    failure_message: overrides.failure_message ?? null,
    updated_at_ms: overrides.updated_at_ms ?? 0,
    artifact_count: overrides.artifact_count ?? 0,
    latest_artifact_kind: overrides.latest_artifact_kind ?? null,
    latest_artifact_title: overrides.latest_artifact_title ?? null,
    needs_human_review: overrides.needs_human_review ?? null,
    reason_codes: overrides.reason_codes ?? [],
    tests_passed: overrides.tests_passed ?? null,
    diff_truncated: overrides.diff_truncated ?? null,
    worktree_path: overrides.worktree_path ?? null,
    cleaned_up: overrides.cleaned_up ?? null,
    suggested_action: overrides.suggested_action ?? null,
    parent_task_id: overrides.parent_task_id ?? null,
    created_at_ms: overrides.created_at_ms ?? 0,
    started_at_ms: overrides.started_at_ms ?? null,
    ended_at_ms: overrides.ended_at_ms ?? null,
    duration_ms: overrides.duration_ms ?? null,
    retryable: overrides.retryable ?? null,
    failure_kind: overrides.failure_kind ?? null,
    resume_note: overrides.resume_note ?? null,
    latest_progress: overrides.latest_progress ?? null,
    lease_owner: overrides.lease_owner ?? null,
    lease_acquired_at_ms: overrides.lease_acquired_at_ms ?? null,
    lease_expires_at_ms: overrides.lease_expires_at_ms ?? null,
    last_heartbeat_at_ms: overrides.last_heartbeat_at_ms ?? null,
    attempt_count: overrides.attempt_count ?? 0,
    max_attempts: overrides.max_attempts ?? 3,
    diff_available: overrides.diff_available ?? null,
    changed_file_count: overrides.changed_file_count ?? null,
    changed_files: overrides.changed_files ?? [],
    test_report_excerpt: overrides.test_report_excerpt ?? null,
  };
}

function projection(tasks: AgentA2ATaskProjection[]): AgentA2AProjection {
  return {
    running_count: tasks.filter((item) => item.status === "running").length,
    completed_count: tasks.filter((item) => item.status === "completed").length,
    failed_count: tasks.filter((item) => item.status === "failed").length,
    interrupted_count: tasks.filter((item) => item.status === "interrupted").length,
    tasks,
  };
}

function scheduledTask(overrides: Partial<ScheduledTask> = {}): ScheduledTask {
  return {
    id: overrides.id ?? "task-1",
    title: overrides.title ?? "Daily check",
    text: overrides.text ?? "Run acceptance check",
    enabled: overrides.enabled ?? true,
    interval_seconds: overrides.interval_seconds ?? 3600,
    next_run_at_ms: overrides.next_run_at_ms ?? 1_800_000_000_000,
    last_run_at_ms: overrides.last_run_at_ms ?? null,
    created_at_ms: overrides.created_at_ms ?? 1_800_000_000_000,
    updated_at_ms: overrides.updated_at_ms ?? 1_800_000_000_000,
    tags: overrides.tags ?? [],
    profile_id: overrides.profile_id ?? null,
    last_error: overrides.last_error ?? null,
  };
}

describe("deriveBackgroundTaskStatus", () => {
  it("hides the bar when there is no background work", () => {
    const result = deriveBackgroundTaskStatus({
      agentA2A: null,
      scheduler: { tasks: [], recent_history: [], load_error: null },
      healthAlerts: [],
    });

    assert.strictEqual(result.visible, false);
    assert.deepStrictEqual(result.items, []);
  });

  it("summarizes agents, review queue, scheduler tasks, and health alerts", () => {
    const scheduler: SchedulerListPayload = {
      tasks: [
        scheduledTask({ id: "enabled", enabled: true }),
        scheduledTask({ id: "disabled", enabled: false }),
      ],
      recent_history: [],
      load_error: null,
    };
    const healthAlerts: RuntimeHealthAlert[] = [
      {
        alert_id: "gateway-down",
        session_id: "session",
        level: "critical",
        title: "Gateway disconnected",
        message: "Gateway is not responding",
      },
    ];

    const result = deriveBackgroundTaskStatus({
      agentA2A: projection([
        task({ task_id: "running", status: "running" }),
        task({ task_id: "review", status: "completed", needs_human_review: true }),
      ]),
      scheduler,
      healthAlerts,
    });

    assert.strictEqual(result.visible, true);
    assert.deepStrictEqual(result.items.map((item) => item.label), [
      "1 子任务运行",
      "1 待审阅",
      "1 调度任务",
      "1 告警",
    ]);
    assert.deepStrictEqual(result.tasks.map((item) => item.title), [
      "Background worker",
      "Background worker",
      "Daily check",
      "Gateway disconnected",
    ]);
    assert.deepStrictEqual(result.tasks.map((item) => item.kind), [
      "agent",
      "review",
      "scheduler",
      "alert",
    ]);
    assert.strictEqual(result.hasAgentWork, true);
  });
});
