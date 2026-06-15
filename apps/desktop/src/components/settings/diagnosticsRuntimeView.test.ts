import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  buildGatewayRuntimeSummary,
  buildGatewayTriggerInput,
} from "./diagnosticsRuntimeView.ts";

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
