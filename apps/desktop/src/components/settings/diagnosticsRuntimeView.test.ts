import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { buildGatewayRuntimeSummary } from "./diagnosticsRuntimeView.ts";

describe("buildGatewayRuntimeSummary", () => {
  it("summarizes reachable runtime queues and dead letters", () => {
    const summary = buildGatewayRuntimeSummary({
      ok: true,
      message: "Gateway runtime is reachable.",
      uptime_seconds: 42,
      active_sessions: 2,
      pending_triggers: 3,
      claimed_triggers: 1,
      dead_letter_runs: 1,
      recent_runs: [],
    });

    assert.equal(summary.tone, "warn");
    assert.equal(summary.statusText, "有积压");
    assert.equal(summary.counts, "3 pending · 1 claimed · 1 dead-letter");
  });

  it("marks offline runtime as unavailable", () => {
    const summary = buildGatewayRuntimeSummary({
      ok: false,
      message: "Gateway unavailable",
      uptime_seconds: 0,
      active_sessions: 0,
      pending_triggers: 0,
      claimed_triggers: 0,
      dead_letter_runs: 0,
      recent_runs: [],
    });

    assert.equal(summary.tone, "fail");
    assert.equal(summary.statusText, "不可用");
    assert.equal(summary.counts, "0 pending · 0 claimed · 0 dead-letter");
  });
});
