import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  runtimeFactsForSubagentTask,
  runtimeFactsFromSubagents,
  summarizeLoopTask,
} from "./loopRuntime.ts";
import type { LoopTaskRecord } from "./protocol.ts";

describe("summarizeLoopTask", () => {
  it("labels missing required checks as verification blockers", () => {
    const summary = summarizeLoopTask(loopTask({
      status: "waiting_for_review",
      completion_result: {
        status: "blocked",
        reasons: ["missing_required_check:build:desktop"],
      },
    }));

    assert.equal(summary.label, "等待验证");
    assert.match(summary.detail, /build:desktop/);
    assert.equal(summary.tone, "review");
  });

  it("keeps waiting-for-input explicit instead of implying autonomous resume", () => {
    const summary = summarizeLoopTask(loopTask({
      status: "waiting_for_input",
      outcome: {
        message: "Gateway loop runner requires an existing desktop session owner before execution.",
      },
    }));

    assert.equal(summary.label, "等待输入");
    assert.match(summary.detail, /desktop session/);
    assert.equal(summary.needsHumanDecision, true);
  });

  it("surfaces budget warnings from latest budget snapshot", () => {
    const summary = summarizeLoopTask(loopTask({
      status: "running",
      latest_budget_snapshot: {
        budget_exceeded: true,
        model_rounds_used: 8,
        tool_calls_used: 24,
        elapsed_ms: 120_000,
        has_unknown_cost: true,
      },
    }));

    assert.equal(summary.budgetWarning, "预算已触发，成本未知");
    assert.equal(summary.detail, "8 轮模型 / 24 次工具 / 2m 0s");
  });
});

describe("runtimeFactsFromSubagents", () => {
  it("renders file IO and usage facts for a loop task", () => {
    const facts = runtimeFactsFromSubagents([
      {
        loop_task_id: "loop-1",
        task_id: "a2a-1",
        latest_event: { type: "file_io", operation: "diff_observed", path: "src/main.rs" },
      },
      {
        loop_task_id: "loop-1",
        task_id: "a2a-2",
        latest_event: {
          type: "usage_recorded",
          model: "claude-sonnet",
          input_tokens: 1200,
          output_tokens: null,
          estimated_cost_micros: null,
        },
      },
      {
        loop_task_id: "loop-other",
        task_id: "a2a-3",
        latest_event: { type: "file_io", operation: "diff_observed", path: "ignored.rs" },
      },
    ]);

    assert.deepEqual(facts, [
      {
        id: "file:a2a-1:src/main.rs:diff_observed",
        kind: "file_io",
        label: "diff_observed",
        detail: "src/main.rs",
      },
      {
        id: "usage:a2a-2:claude-sonnet",
        kind: "usage",
        label: "claude-sonnet",
        detail: "input 1200 / output unknown / cost unknown",
        model: "claude-sonnet",
        source: null,
        reason: null,
        inputTokens: 1200,
        outputTokens: null,
        estimatedCostMicros: null,
        inputTokensUnknown: false,
        outputTokensUnknown: true,
        costUnknown: true,
      },
    ]);
  });

  it("renders facts for a single subagent task", () => {
    const facts = runtimeFactsForSubagentTask([
      {
        loop_task_id: "loop-1",
        task_id: "a2a-1",
        latest_event: { type: "file_io", operation: "worktree_created", path: "/tmp/worktree" },
      },
      {
        loop_task_id: "loop-1",
        task_id: "a2a-2",
        latest_event: { type: "file_io", operation: "diff_observed", path: "src/lib.rs" },
      },
    ], "a2a-2");

    assert.deepEqual(facts, [
      {
        id: "file:a2a-2:src/lib.rs:diff_observed",
        kind: "file_io",
        label: "diff_observed",
        detail: "src/lib.rs",
      },
    ]);
  });

  it("renders single task facts even when loop task id is unknown", () => {
    const facts = runtimeFactsForSubagentTask([
      {
        loop_task_id: null,
        task_id: "a2a-1",
        latest_event: { type: "file_io", operation: "diff_observed", path: "src/main.rs" },
      },
    ], "a2a-1");

    assert.deepEqual(facts, [
      {
        id: "file:a2a-1:src/main.rs:diff_observed",
        kind: "file_io",
        label: "diff_observed",
        detail: "src/main.rs",
      },
    ]);
  });

  it("renders provider omitted and pricing unknown usage reasons distinctly", () => {
    const facts = runtimeFactsFromSubagents([
      {
        loop_task_id: "loop-1",
        task_id: "a2a-provider-omitted",
        latest_event: {
          type: "usage_recorded",
          model: "claude-sonnet",
          source: "anthropic",
          reason: "provider_omitted",
          input_tokens: null,
          output_tokens: null,
          estimated_cost_micros: null,
        },
      },
      {
        loop_task_id: "loop-1",
        task_id: "a2a-pricing-unknown",
        latest_event: {
          type: "usage_recorded",
          model: "mystery-model",
          source: "openai_compatible",
          reason: "pricing_unknown",
          input_tokens: 1200,
          output_tokens: 300,
          estimated_cost_micros: null,
        },
      },
    ], "loop-1");

    assert.deepEqual(facts.map((fact) => fact.detail), [
      "input unknown / output unknown / cost unknown / provider omitted",
      "input 1200 / output 300 / cost unknown / pricing unknown",
    ]);
  });

  it("exposes provider usage facts with structured fields and unknown flags", () => {
    const facts = runtimeFactsFromSubagents([
      {
        loop_task_id: "loop-1",
        task_id: "a2a-usage",
        latest_event: {
          type: "usage_recorded",
          model: "claude-sonnet",
          source: "anthropic",
          reason: "provider_omitted",
          input_tokens: null,
          output_tokens: null,
          estimated_cost_micros: null,
        },
      },
    ], "loop-1");

    assert.deepEqual(facts[0], {
      id: "usage:a2a-usage:claude-sonnet",
      kind: "usage",
      label: "claude-sonnet",
      detail: "input unknown / output unknown / cost unknown / provider omitted",
      model: "claude-sonnet",
      source: "anthropic",
      reason: "provider_omitted",
      inputTokens: null,
      outputTokens: null,
      estimatedCostMicros: null,
      inputTokensUnknown: true,
      outputTokensUnknown: true,
      costUnknown: true,
    });
  });
});

function loopTask(overrides: Partial<LoopTaskRecord>): LoopTaskRecord {
  return {
    id: overrides.id ?? "loop-1",
    goal: overrides.goal ?? "Ship runtime UI",
    status: overrides.status ?? "pending",
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
