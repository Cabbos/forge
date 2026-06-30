import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  buildGatewaySessionRows,
  buildGatewaySessionEventRows,
  buildGatewaySessionInput,
  buildGatewayRuntimeSummary,
  buildGatewayTriggerRunRows,
  buildGatewayTriggerRows,
  buildGatewayTriggerInput,
  formatGatewayTriggerRunDetail,
} from "./diagnosticsRuntimeView.ts";

describe("buildGatewayRuntimeSummary", () => {
  it("summarizes reachable runtime queues and dead letters", () => {
    const summary = buildGatewayRuntimeSummary({
      ok: true,
      message: "Gateway runtime is reachable.",
      uptime_seconds: 42,
      active_sessions: 2,
      pending_triggers: 3,
      pending_session_inputs: 2,
      claimed_triggers: 1,
      dead_letter_runs: 1,
      recent_runs: [],
      runtime_tasks: [
        { name: "webhook_listener", running: true },
        { name: "trigger_runner", running: true },
      ],
    });

    assert.equal(summary.tone, "warn");
    assert.equal(summary.statusText, "有积压");
    assert.equal(summary.counts, "3 pending · 2 inputs · 1 claimed · 1 dead-letter · 2/2 loops");
  });

  it("marks offline runtime as unavailable", () => {
    const summary = buildGatewayRuntimeSummary({
      ok: false,
      message: "Gateway unavailable",
      uptime_seconds: 0,
      active_sessions: 0,
      pending_triggers: 0,
      pending_session_inputs: 0,
      claimed_triggers: 0,
      dead_letter_runs: 0,
      recent_runs: [],
      runtime_tasks: [],
    });

    assert.equal(summary.tone, "fail");
    assert.equal(summary.statusText, "不可用");
    assert.equal(summary.counts, "0 pending · 0 inputs · 0 claimed · 0 dead-letter");
  });

  it("warns when a gateway runtime task is stopped", () => {
    const summary = buildGatewayRuntimeSummary({
      ok: true,
      message: "Gateway runtime is reachable.",
      uptime_seconds: 42,
      active_sessions: 0,
      pending_triggers: 0,
      pending_session_inputs: 0,
      claimed_triggers: 0,
      dead_letter_runs: 0,
      recent_runs: [],
      runtime_tasks: [
        { name: "webhook_listener", running: true },
        { name: "trigger_runner", running: false },
        { name: "scheduler_tick", running: true },
      ],
    });

    assert.equal(summary.tone, "warn");
    assert.equal(summary.statusText, "后台循环异常");
    assert.equal(summary.counts, "0 pending · 0 inputs · 0 claimed · 0 dead-letter · 2/3 loops");
  });

  it("warns when session input inbox has pending entries", () => {
    const summary = buildGatewayRuntimeSummary({
      ok: true,
      message: "Gateway runtime is reachable.",
      uptime_seconds: 42,
      active_sessions: 1,
      pending_triggers: 0,
      pending_session_inputs: 2,
      claimed_triggers: 0,
      dead_letter_runs: 0,
      recent_runs: [],
      runtime_tasks: [],
    });

    assert.equal(summary.tone, "warn");
    assert.equal(summary.statusText, "有积压");
    assert.equal(summary.counts, "0 pending · 2 inputs · 0 claimed · 0 dead-letter");
  });

  it("includes nonzero loop task and headless owner counters", () => {
    const summary = buildGatewayRuntimeSummary({
      ok: true,
      message: "Gateway runtime is reachable.",
      uptime_seconds: 42,
      active_sessions: 1,
      pending_triggers: 0,
      pending_session_inputs: 0,
      claimed_triggers: 0,
      dead_letter_runs: 0,
      pending_loop_tasks: 2,
      running_loop_tasks: 1,
      stale_loop_task_leases: 1,
      dry_run_headless_owner_runs: 3,
      waiting_headless_owner_runs: 2,
      denied_headless_owner_runs: 1,
      expired_headless_owner_runs: 1,
      recent_runs: [],
      runtime_tasks: [],
    });

    assert.equal(summary.tone, "warn");
    assert.equal(summary.statusText, "有积压");
    assert.equal(
      summary.counts,
      "0 pending · 0 inputs · 0 claimed · 0 dead-letter · 2 loop pending · 1 loop running · 1 stale lease · 3 owner dry-runs · 2 owners waiting · 1 owner denied · 1 owner expired",
    );
  });
});

describe("buildGatewaySessionRows", () => {
  it("orders live sessions before stale and restored sessions", () => {
    const rows = buildGatewaySessionRows(
      [
        {
          session_id: "restored-1",
          provider: "claude",
          model: "sonnet",
          workspace_path: "/repo/restored",
          created_at_ms: 10,
          owner_pid: null,
          last_seen_at_ms: null,
          restored_from_registry: true,
        },
        {
          session_id: "stale-1",
          provider: "openai",
          model: "gpt-5",
          workspace_path: "/repo/stale",
          created_at_ms: 20,
          owner_pid: 100,
          last_seen_at_ms: 1_000,
          restored_from_registry: false,
        },
        {
          session_id: "live-1",
          provider: "openai",
          model: "gpt-5",
          workspace_path: "/repo/live",
          created_at_ms: 30,
          owner_pid: 200,
          last_seen_at_ms: 299_000,
          restored_from_registry: false,
        },
      ],
      302_000,
    );

    assert.deepEqual(
      rows.map((row) => [row.id, row.stateLabel, row.runtime, row.workspacePath]),
      [
        ["live-1", "live", "openai/gpt-5", "/repo/live"],
        ["stale-1", "stale", "openai/gpt-5", "/repo/stale"],
        ["restored-1", "restored", "claude/sonnet", "/repo/restored"],
      ],
    );
  });

  it("formats sparse session metadata without throwing", () => {
    const rows = buildGatewaySessionRows(
      [
        {
          session_id: "session-1",
          provider: "",
          model: "  ",
          workspace_path: "  ",
          created_at_ms: 42,
          owner_pid: null,
          last_seen_at_ms: null,
          restored_from_registry: false,
        },
      ],
      100,
    );

    assert.equal(rows[0].runtime, "-");
    assert.equal(rows[0].workspacePath, null);
    assert.equal(rows[0].subtitle, "pid=- · last_seen=- · created=42");
  });
});

describe("buildGatewaySessionEventRows", () => {
  it("formats event tail cursor metadata and compact previews", () => {
    const view = buildGatewaySessionEventRows({
      ok: true,
      session_id: "session-1",
      events: [
        {
          event_type: "user_message",
          block_id: "user-1",
          content: "continue the work",
        },
        {
          event_type: "tool_call_start",
          block_id: "tool-1",
          tool_name: "shell",
        },
      ],
      next_cursor: 4,
      total_events: 6,
      cursor_reset: false,
    });

    assert.equal(view.summary, "2 events · next=4 · total=6");
    assert.deepEqual(view.rows, [
      {
        id: "user-1",
        eventType: "user_message",
        label: "user_message",
        preview: "continue the work",
      },
      {
        id: "tool-1",
        eventType: "tool_call_start",
        label: "tool_call_start",
        preview: "shell",
      },
    ]);
  });

  it("marks cursor resets and handles empty tails", () => {
    const view = buildGatewaySessionEventRows({
      ok: true,
      session_id: "session-1",
      events: [],
      next_cursor: 0,
      total_events: 0,
      cursor_reset: true,
    });

    assert.equal(view.summary, "0 events · next=0 · total=0 · reset");
    assert.deepEqual(view.rows, []);
  });
});

describe("buildGatewaySessionInput", () => {
  it("trims session id and message for gateway input enqueue", () => {
    const result = buildGatewaySessionInput({
      sessionId: " session-1 ",
      message: " continue the work ",
    });

    assert.deepEqual(result, {
      input: {
        sessionId: "session-1",
        message: "continue the work",
      },
      error: null,
    });
  });

  it("rejects blank session input messages", () => {
    const result = buildGatewaySessionInput({
      sessionId: "session-1",
      message: "   ",
    });

    assert.equal(result.input, null);
    assert.equal(result.error, "Message is required.");
  });
});

describe("buildGatewayTriggerInput", () => {
  it("trims trigger form fields and maps them to gateway params", () => {
    const result = buildGatewayTriggerInput({
      message: "  run dashboard digest  ",
      profileId: " ops ",
      provider: " openai ",
      model: " gpt-5 ",
      workspacePath: " /repo/workspace ",
    });

    assert.deepEqual(result, {
      input: {
        message: "run dashboard digest",
        profile_id: "ops",
        provider: "openai",
        model: "gpt-5",
        workspace_path: "/repo/workspace",
      },
      error: null,
    });
  });

  it("rejects blank trigger messages", () => {
    const result = buildGatewayTriggerInput({
      message: "   ",
      profileId: "",
      provider: "",
      model: "",
      workspacePath: "",
    });

    assert.equal(result.input, null);
    assert.equal(result.error, "Message is required.");
  });
});

describe("buildGatewayTriggerRows", () => {
  it("orders pending before claimed triggers and formats metadata", () => {
    const rows = buildGatewayTriggerRows([
      {
        id: "claimed-1",
        message: "claimed work item",
        profile_id: "ops",
        provider: "openai",
        model: "gpt-5",
        workspace_path: "/repo",
        attempt_count: 2,
        claimed_at_ms: 1234,
        received_at_ms: 20,
      },
      {
        id: "pending-1",
        message: "pending work item",
        profile_id: null,
        provider: null,
        model: null,
        workspace_path: null,
        attempt_count: 0,
        claimed_at_ms: null,
        received_at_ms: 30,
      },
    ]);

    assert.deepEqual(
      rows.map((row) => [row.id, row.stateLabel, row.subtitle, row.message]),
      [
        ["pending-1", "pending", "profile=- · attempts=0 · received=30", "pending work item"],
        ["claimed-1", "claimed", "profile=ops · openai/gpt-5 · attempts=2 · received=20", "claimed work item"],
      ],
    );
  });

  it("truncates long trigger messages for compact diagnostics rows", () => {
    const rows = buildGatewayTriggerRows([
      {
        id: "long-1",
        message: "x".repeat(140),
        profile_id: "ops",
        provider: null,
        model: null,
        workspace_path: null,
        attempt_count: 1,
        claimed_at_ms: null,
        received_at_ms: 10,
      },
    ]);

    assert.equal(rows[0].message.length, 121);
    assert.match(rows[0].message, /…$/u);
  });
});

describe("buildGatewayTriggerRunRows", () => {
  it("marks runs with trigger metadata as replayable", () => {
    const rows = buildGatewayTriggerRunRows([
      {
        id: "run-1",
        trigger_id: "trigger-1",
        session_id: "gateway-session-1",
        attempt: 2,
        status: "dead_letter",
        message: "provider offline",
        started_at_ms: 20,
        ended_at_ms: 25,
        executor_kind: "eval_headless",
        failure_category: "runner_error",
        lease_expires_at_ms: 300_010,
        trigger_message: "run digest",
        profile_id: "ops",
        provider: "openai",
        model: "gpt-5",
        workspace_path: "/repo",
      },
    ]);

    assert.deepEqual(rows, [
      {
        id: "run-1",
        title: "dead_letter · attempt 2",
        subtitle:
          "trigger=trigger-1 · session=gateway-session-1 · profile=ops · openai/gpt-5 · executor=eval_headless · failure=runner_error · lease_expires=300010",
        message: "provider offline",
        canReplay: true,
      },
    ]);
  });

  it("keeps legacy runs visible but not replayable", () => {
    const rows = buildGatewayTriggerRunRows([
      {
        id: "run-legacy",
        trigger_id: "trigger-legacy",
        attempt: 1,
        status: "completed",
        message: "legacy ok",
        started_at_ms: 10,
        ended_at_ms: 11,
      },
    ]);

    assert.equal(rows[0].canReplay, false);
    assert.equal(rows[0].subtitle, "trigger=trigger-legacy · profile=-");
  });

  it("formats run detail evidence fields for diagnostics inspection", () => {
    assert.equal(
      formatGatewayTriggerRunDetail({
        id: "run-detail",
        trigger_id: "trigger-detail",
        session_id: "gateway-session-1",
        attempt: 3,
        status: "dead_letter",
        message: "provider offline",
        started_at_ms: 10,
        ended_at_ms: 20,
        executor_kind: "eval_headless",
        failure_category: "runner_error",
        lease_expires_at_ms: 300_010,
        trigger_message: "run digest",
        profile_id: "ops",
        provider: "openai",
        model: "gpt-5",
        workspace_path: "/repo",
      }),
      "session=gateway-session-1 · executor=eval_headless · failure=runner_error · lease_expires=300010 · started=10 · ended=20 · workspace=/repo · trigger_message=run digest",
    );
  });
});
