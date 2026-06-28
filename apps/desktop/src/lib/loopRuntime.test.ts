import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  deriveHeadlessResumeReadinessFromState,
  headlessResumeReadinessForTask,
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

  it("surfaces ready-for-review while commit remains human-gated", () => {
    const summary = summarizeLoopTask(loopTask({
      status: "waiting_for_review",
      completion_result: {
        status: "waiting_for_review",
        reasons: ["missing_review_decision"],
        review_status: "ready_for_review",
        commit_eligible: false,
        commit_blockers: ["missing_human_review"],
      },
    }));

    assert.equal(summary.label, "ready for human review");
    assert.equal(summary.commitEligible, false);
    assert.deepEqual(summary.commitBlockers, ["missing_human_review"]);
    assert.match(summary.detail, /commit remains human-gated/);
  });

  it("surfaces commit eligibility after approved human review", () => {
    const summary = summarizeLoopTask(loopTask({
      status: "completed",
      completion_result: {
        status: "complete",
        reasons: [],
        review_status: "approved",
        commit_eligible: true,
        commit_blockers: [],
        human_gate_id: "gate-1",
        last_review_decision: { kind: "approved", reason: null },
      },
    }));

    assert.equal(summary.label, "commit eligible after human review");
    assert.equal(summary.commitEligible, true);
    assert.equal(summary.humanGateId, "gate-1");
    assert.match(summary.detail, /commit remains human-gated/);
  });

  it("surfaces rejected review as a commit blocker", () => {
    const summary = summarizeLoopTask(loopTask({
      status: "waiting_for_review",
      completion_result: {
        status: "waiting_for_review",
        reasons: ["review_rejected:needs tests"],
        review_status: "rejected",
        commit_eligible: false,
        commit_blockers: ["review_rejected:needs tests"],
        human_gate_id: "gate-1",
        last_review_decision: { kind: "denied", reason: "needs tests" },
      },
    }));

    assert.equal(summary.label, "blocked by review");
    assert.equal(summary.commitEligible, false);
    assert.deepEqual(summary.commitBlockers, ["review_rejected:needs tests"]);
    assert.match(summary.detail, /needs tests/);
  });

  it("does not label no-review completed tasks as human review work", () => {
    const summary = summarizeLoopTask(loopTask({
      status: "completed",
      completion_result: {
        status: "complete",
        reasons: [],
        review_status: "not_required",
        commit_eligible: true,
        commit_blockers: [],
      },
    }));

    assert.notEqual(summary.label, "ready for human review");
    assert.notEqual(summary.label, "blocked by review");
    assert.notEqual(summary.label, "commit eligible after human review");
    assert.equal(summary.label, "完成");
    assert.equal(summary.needsHumanDecision, false);
    assert.doesNotMatch(summary.detail, /commit remains human-gated/);
    assert.doesNotMatch(summary.detail, /ready for human review|blocked by review|commit eligible after human review/);
    assert.equal(summary.reviewStatus, "not_required");
    assert.equal(summary.commitEligible, true);
    assert.deepEqual(summary.commitBlockers, []);
  });

  it("derives desktop-owner readiness without changing waiting-for-input status", () => {
    const task = loopTask({
      status: "waiting_for_input",
      headless_resume_mode: "disabled",
      headless_resume_approval: null,
    });

    const readiness = headlessResumeReadinessForTask(task, { nowMs: 1_000 });
    const summary = summarizeLoopTask(task, { nowMs: 1_000 });

    assert.ok(readiness);
    assert.equal(readiness.state, "desktop_owner_required");
    assert.equal(summary.label, "等待输入");
    assert.equal(summary.rawTask.status, "waiting_for_input");
    assert.equal(summary.headlessResumeReadiness?.state, "desktop_owner_required");
    assert.match(summary.headlessResumeReadiness?.detail ?? "", /desktop owner/i);
  });

  it("derives pure headless readiness from mode and approval even for pending tasks", () => {
    const task = loopTask({
      status: "pending",
      headless_resume_mode: "approved_for_task",
      headless_resume_approval: {
        task_id: "loop-1",
        approved_by: "human-reviewer",
        approved_at_ms: 500,
        scope: "task",
        expires_at_ms: 2_000,
      },
    });

    const readiness = deriveHeadlessResumeReadinessFromState(
      task.headless_resume_mode,
      task.headless_resume_approval,
      1_000,
    );
    const wrapperReadiness = headlessResumeReadinessForTask(task, { nowMs: 1_000 });
    const summary = summarizeLoopTask(task, { nowMs: 1_000 });

    assert.equal(readiness.state, "approval_recorded_lease_pending");
    assert.equal(wrapperReadiness, null);
    assert.equal(summary.headlessResumeReadiness, null);
  });

  it("derives approval-recorded lease-pending readiness without automatic continuation text", () => {
    const summary = summarizeLoopTask(loopTask({
      status: "waiting_for_input",
      headless_resume_mode: "approved_for_task",
      headless_resume_approval: {
        task_id: "loop-1",
        approved_by: "human-reviewer",
        approved_at_ms: 500,
        scope: "task",
        expires_at_ms: 2_000,
      },
    }), { nowMs: 1_000 });

    assert.equal(summary.label, "等待输入");
    assert.equal(summary.rawTask.status, "waiting_for_input");
    assert.equal(summary.headlessResumeReadiness?.state, "approval_recorded_lease_pending");
    assert.match(summary.headlessResumeReadiness?.detail ?? "", /lease\/desktop owner pending/i);
    assert.doesNotMatch(
      [
        summary.headlessResumeReadiness?.label,
        summary.headlessResumeReadiness?.detail,
        summary.detail,
      ].join(" "),
      /will continue automatically|continue automatically|自动继续/i,
    );
  });

  it("derives expired approval readiness from approval expiry", () => {
    const summary = summarizeLoopTask(loopTask({
      status: "waiting_for_input",
      headless_resume_mode: "approved_for_task",
      headless_resume_approval: {
        task_id: "loop-1",
        approved_by: "human-reviewer",
        approved_at_ms: 500,
        scope: "task",
        expires_at_ms: 1_000,
      },
    }), { nowMs: 1_000 });

    assert.equal(summary.rawTask.status, "waiting_for_input");
    assert.equal(summary.headlessResumeReadiness?.state, "approval_expired");
    assert.match(summary.headlessResumeReadiness?.detail ?? "", /approval expired/i);
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
          provider_id: "anthropic",
          model: "claude-sonnet",
          input_tokens: 1200,
          output_tokens: null,
          cache_read_tokens: 20,
          cache_creation_tokens: null,
          reasoning_tokens: 7,
          estimated_cost_micros: null,
          pricing_source: null,
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
        providerId: "anthropic",
        model: "claude-sonnet",
        source: null,
        reason: null,
        inputTokens: 1200,
        outputTokens: null,
        cacheReadTokens: 20,
        cacheCreationTokens: null,
        reasoningTokens: 7,
        estimatedCostMicros: null,
        pricingSource: null,
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
      providerId: null,
      model: "claude-sonnet",
      source: "anthropic",
      reason: "provider_omitted",
      inputTokens: null,
      outputTokens: null,
      cacheReadTokens: null,
      cacheCreationTokens: null,
      reasoningTokens: null,
      estimatedCostMicros: null,
      pricingSource: null,
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
    headless_resume_mode: overrides.headless_resume_mode ?? "disabled",
    headless_resume_approval: overrides.headless_resume_approval ?? null,
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
