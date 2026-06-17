import { describe, it } from "node:test";
import assert from "node:assert";
import { deriveBackgroundTaskStatus } from "./backgroundTaskStatus.ts";
import type { AgentA2AProjection, AgentA2ATaskProjection, LoopTaskRecord } from "./protocol.ts";
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
    child_task_ids: overrides.child_task_ids ?? [],
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

function loopTask(overrides: Partial<LoopTaskRecord> = {}): LoopTaskRecord {
  return {
    id: overrides.id ?? "loop-1",
    goal: overrides.goal ?? "Ship Level 3 runtime UI",
    status: overrides.status ?? "waiting_for_input",
    owner: overrides.owner ?? { kind: "gateway" },
    policy: overrides.policy ?? {},
    budget: overrides.budget ?? {},
    completion_contract: overrides.completion_contract ?? {},
    created_at_ms: overrides.created_at_ms ?? 1,
    updated_at_ms: overrides.updated_at_ms ?? 2,
    session_id: overrides.session_id ?? null,
    profile_id: overrides.profile_id ?? null,
    workspace_path: overrides.workspace_path ?? null,
    lease: overrides.lease ?? null,
    open_gates: overrides.open_gates ?? [],
    evidence: overrides.evidence ?? [],
    policy_decisions: overrides.policy_decisions ?? [],
    latest_budget_snapshot: overrides.latest_budget_snapshot ?? null,
    latest_event_id: overrides.latest_event_id ?? null,
    outcome: overrides.outcome ?? null,
    completion_result: overrides.completion_result ?? null,
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
    assert.deepStrictEqual(result.notifications.map((item) => `${item.kind}:${item.title}:${item.detail}`), [
      "alert:Gateway disconnected:Gateway is not responding",
      "review:Background worker:等待人工审阅",
      "agent:Background worker:子任务运行中",
      "scheduler:Daily check:3600s 间隔",
    ]);
    assert.strictEqual(result.hasAgentWork, true);
  });

  it("surfaces waiting loop runtime tasks as background work", () => {
    const result = deriveBackgroundTaskStatus({
      agentA2A: null,
      scheduler: { tasks: [], recent_history: [], load_error: null },
      healthAlerts: [],
      loopTasks: [
        loopTask({
          id: "loop-waiting",
          goal: "Ship Level 3 runtime UI",
          status: "waiting_for_input",
          outcome: { message: "Requires existing desktop session owner." },
        }),
      ],
    });

    assert.strictEqual(result.visible, true);
    assert.deepStrictEqual(result.items.map((item) => item.label), ["1 Loop 任务"]);
    assert.deepStrictEqual(result.tasks.map((item) => `${item.kind}:${item.title}:${item.detail}`), [
      "loop:Ship Level 3 runtime UI:Requires existing desktop session owner.",
    ]);
    assert.deepStrictEqual(result.notifications.map((item) => `${item.kind}:${item.title}`), [
      "loop:Ship Level 3 runtime UI",
    ]);
    assert.strictEqual(result.hasAgentWork, true);
  });

  it("hides terminal loop runtime tasks from background work", () => {
    const result = deriveBackgroundTaskStatus({
      agentA2A: null,
      scheduler: { tasks: [], recent_history: [], load_error: null },
      healthAlerts: [],
      loopTasks: [
        loopTask({ id: "loop-complete", status: "completed" }),
        loopTask({ id: "loop-canceled", status: "canceled" }),
        loopTask({ id: "loop-failed", status: "failed" }),
      ],
    });

    assert.strictEqual(result.visible, false);
    assert.deepStrictEqual(result.items, []);
    assert.deepStrictEqual(result.tasks, []);
    assert.strictEqual(result.hasAgentWork, false);
  });

  it("keeps running waiting review and interrupted loop tasks visible", () => {
    const result = deriveBackgroundTaskStatus({
      agentA2A: null,
      scheduler: { tasks: [], recent_history: [], load_error: null },
      healthAlerts: [],
      loopTasks: [
        loopTask({ id: "loop-running", goal: "Running loop", status: "running" }),
        loopTask({ id: "loop-input", goal: "Waiting input loop", status: "waiting_for_input" }),
        loopTask({ id: "loop-review", goal: "Waiting review loop", status: "waiting_for_review" }),
        loopTask({ id: "loop-interrupted", goal: "Interrupted loop", status: "interrupted" }),
      ],
    });

    assert.strictEqual(result.visible, true);
    assert.deepStrictEqual(result.items.map((item) => item.label), ["4 Loop 任务"]);
    assert.deepStrictEqual(result.tasks.map((item) => item.title), [
      "Running loop",
      "Waiting input loop",
      "Waiting review loop",
      "Interrupted loop",
    ]);
    assert.strictEqual(result.hasAgentWork, true);
  });
});
