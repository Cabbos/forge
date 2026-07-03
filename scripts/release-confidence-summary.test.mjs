import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  buildReleaseConfidenceSummary,
  renderReleaseConfidenceMarkdown,
} from "./release-confidence-summary.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "release-confidence-summary.mjs");

const acceptanceMatrix = {
  schemaVersion: 1,
  workingDirectory: root.replace(/\/$/, ""),
  domains: [
    { id: "runtime", label: "Runtime", gateCount: 1 },
    { id: "gateway", label: "Gateway", gateCount: 1 },
    { id: "eval", label: "Eval", gateCount: 1 },
  ],
  gates: [
    {
      index: 1,
      domain: "runtime",
      label: "runtime authority fast gate",
      command: "cargo test runtime",
      ciDefault: true,
    },
    {
      index: 2,
      domain: "gateway",
      label: "gateway parity and degraded fallback smoke",
      command: "cargo test gateway",
      tier: "runtime-core",
      runtimeCost: "medium",
      manualRequirement: false,
      ciDefault: true,
    },
    {
      index: 3,
      domain: "eval",
      label: "desktop eval promotion evidence smoke",
      command: "npm run test:eval",
      ciDefault: false,
    },
  ],
};

test("builds a release confidence summary from acceptance and eval evidence", () => {
  const summary = buildReleaseConfidenceSummary({
    acceptanceMatrix,
    gateResults: {
      gates: [
        { label: "runtime authority fast gate", status: "passed", duration_ms: 1200 },
        {
          label: "gateway parity and degraded fallback smoke",
          status: "failed",
          reason: "gateway degraded fallback fixture failed",
        },
        { label: "desktop eval promotion evidence smoke", status: "manual", reason: "manual evidence pending" },
      ],
    },
    evalReport: {
      report: {
        total_tasks: 4,
        success_rate: 0.75,
        score_summary: {
          forge_confirmation_correctness_ok: 1,
          forge_gateway_degraded_fallback_ok: 0.5,
        },
      },
    },
    boundaries: {
      declared: ["gateway_can_resume=false"],
      required: [
        { id: "gateway_can_resume=false", domain: "gateway" },
        { id: "direct-write owner disabled", domain: "gateway" },
      ],
    },
  });

  assert.equal(summary.schemaVersion, 1);
  assert.equal(summary.status, "failed");
  assert.equal(summary.acceptance.totalGates, 3);
  assert.equal(summary.acceptance.ciDefault.totalGates, 2);
  assert.deepEqual(summary.acceptance.ciDefault.passedGates.map((gate) => gate.label), [
    "runtime authority fast gate",
  ]);
  assert.deepEqual(summary.acceptance.ciDefault.failedGates.map((gate) => gate.label), [
    "gateway parity and degraded fallback smoke",
  ]);
  assert.deepEqual(summary.acceptance.ciDefault.manualEvidenceGates, []);
  assert.deepEqual(summary.acceptance.ciDefault.unknownGates, []);
  assert.deepEqual(summary.acceptance.failedGates.map((gate) => gate.label), [
    "gateway parity and degraded fallback smoke",
  ]);
  assert.deepEqual(
    summary.acceptance.failedGates.map(({ label, tier, runtimeCost, manualRequirement }) => ({
      label,
      tier,
      runtimeCost,
      manualRequirement,
    })),
    [
      {
        label: "gateway parity and degraded fallback smoke",
        tier: "runtime-core",
        runtimeCost: "medium",
        manualRequirement: false,
      },
    ],
  );
  assert.deepEqual(summary.acceptance.manualEvidenceGates.map((gate) => gate.label), [
    "desktop eval promotion evidence smoke",
  ]);
  assert.deepEqual(summary.eval.failingScores, [
    { name: "forge_gateway_degraded_fallback_ok", score: 0.5, domain: "gateway" },
  ]);
  assert.deepEqual(summary.affectedDomains.sort(), ["eval", "gateway"]);
  assert.deepEqual(summary.undeclaredBoundaries, [
    { id: "direct-write owner disabled", domain: "gateway" },
  ]);
});

test("renders release confidence markdown for PR evidence", () => {
  const summary = buildReleaseConfidenceSummary({
    acceptanceMatrix,
    gateResults: {
      gates: [{ label: "runtime authority fast gate", status: "passed" }],
    },
    evalReport: { report: { total_tasks: 1, success_rate: 1, score_summary: {} } },
  });

  const markdown = renderReleaseConfidenceMarkdown(summary);

  assert.match(markdown, /^# Forge Release Confidence/m);
  assert.match(markdown, /Status: attention_required/);
  assert.match(markdown, /Unknown gates: 2/);
  assert.match(markdown, /CI-default gates: 2/);
  assert.match(markdown, /CI-default passed: 1/);
  assert.match(markdown, /CI-default failed: 0/);
  assert.match(markdown, /CI-default unknown: 1/);
  assert.match(markdown, /Affected domains: eval, gateway/);
  assert.match(
    markdown,
    /desktop eval promotion evidence smoke \(domain: eval; status: unknown; tier: unknown; cost: unknown; manual: unknown\)/,
  );
});

test("summarizes failing task-level eval scores when score summary is absent", () => {
  const summary = buildReleaseConfidenceSummary({
    acceptanceMatrix,
    gateResults: {
      gates: acceptanceMatrix.gates.map((gate) => ({ label: gate.label, status: "passed" })),
    },
    evalReport: {
      report: {
        total_tasks: 2,
        success_rate: 1,
        task_metrics: [
          {
            task_id: "task-1",
            scores: {
              forge_completion_eligibility_evidence_ok: { score: 0, label: "conflict" },
              forge_memory_recall_quality_ok: { score: 0.5, label: "partial" },
            },
          },
          {
            task_id: "task-2",
            scores: {
              forge_completion_eligibility_evidence_ok: { score: 1, label: "ok" },
            },
          },
        ],
      },
    },
  });

  assert.equal(summary.status, "attention_required");
  assert.deepEqual(summary.eval.failingScores, [
    {
      name: "forge_completion_eligibility_evidence_ok",
      score: 0,
      domain: "runtime",
      failingTasks: ["task-1"],
    },
    {
      name: "forge_memory_recall_quality_ok",
      score: 0.5,
      domain: "memory",
      failingTasks: ["task-1"],
    },
  ]);
  assert.deepEqual(summary.affectedDomains, ["memory", "runtime"]);

  const markdown = renderReleaseConfidenceMarkdown(summary);
  assert.match(markdown, /forge_completion_eligibility_evidence_ok: 0 \(runtime; tasks: task-1\)/);
});

test("summarizes acceptance gates by domain and tier", () => {
  const matrix = {
    gates: [
      {
        label: "runtime authority fast gate",
        domain: "runtime",
        tier: "runtime-core",
        ciDefault: true,
      },
      {
        label: "gateway parity and degraded fallback smoke",
        domain: "gateway",
        tier: "runtime-core",
        ciDefault: true,
      },
      {
        label: "manual desktop restart smoke protocol",
        domain: "ui-evidence",
        tier: "manual-evidence",
        manualRequirement: true,
        ciDefault: false,
      },
      {
        label: "desktop eval promotion evidence smoke",
        domain: "eval",
        tier: "full-release",
        ciDefault: false,
      },
    ],
  };
  const summary = buildReleaseConfidenceSummary({
    acceptanceMatrix: matrix,
    gateResults: {
      gates: [
        { label: "runtime authority fast gate", status: "passed" },
        { label: "gateway parity and degraded fallback smoke", status: "failed" },
        { label: "manual desktop restart smoke protocol", status: "manual" },
      ],
    },
    evalReport: { report: { total_tasks: 1, success_rate: 1, score_summary: {} } },
  });

  assert.deepEqual(summary.acceptance.domainBreakdown, [
    {
      domain: "eval",
      totalGates: 1,
      passedGates: 0,
      failedGates: 0,
      manualEvidenceGates: 0,
      unknownGates: 1,
    },
    {
      domain: "gateway",
      totalGates: 1,
      passedGates: 0,
      failedGates: 1,
      manualEvidenceGates: 0,
      unknownGates: 0,
    },
    {
      domain: "runtime",
      totalGates: 1,
      passedGates: 1,
      failedGates: 0,
      manualEvidenceGates: 0,
      unknownGates: 0,
    },
    {
      domain: "ui-evidence",
      totalGates: 1,
      passedGates: 0,
      failedGates: 0,
      manualEvidenceGates: 1,
      unknownGates: 0,
    },
  ]);
  assert.deepEqual(summary.acceptance.tierBreakdown, [
    {
      tier: "full-release",
      totalGates: 1,
      passedGates: 0,
      failedGates: 0,
      manualEvidenceGates: 0,
      unknownGates: 1,
    },
    {
      tier: "manual-evidence",
      totalGates: 1,
      passedGates: 0,
      failedGates: 0,
      manualEvidenceGates: 1,
      unknownGates: 0,
    },
    {
      tier: "runtime-core",
      totalGates: 2,
      passedGates: 1,
      failedGates: 1,
      manualEvidenceGates: 0,
      unknownGates: 0,
    },
  ]);

  const markdown = renderReleaseConfidenceMarkdown(summary);
  assert.match(markdown, /### Acceptance Domains/);
  assert.match(markdown, /gateway: total 1, passed 0, failed 1, manual 0, unknown 0/);
  assert.match(markdown, /### Acceptance Tiers/);
  assert.match(markdown, /runtime-core: total 2, passed 1, failed 1, manual 0, unknown 0/);
});

test("summarizes gate-results execution completeness", () => {
  const summary = buildReleaseConfidenceSummary({
    acceptanceMatrix,
    gateResults: {
      status: "failed",
      selectedGateCount: 3,
      executedGateCount: 2,
      failedGateCount: 1,
      gates: [
        { label: "runtime authority fast gate", status: "passed" },
        {
          label: "gateway parity and degraded fallback smoke",
          status: "failed",
          reason: "exit_code_1",
        },
      ],
    },
    evalReport: { report: { total_tasks: 1, success_rate: 1, score_summary: {} } },
  });

  assert.deepEqual(summary.acceptance.execution, {
    available: true,
    status: "failed",
    selectedGateCount: 3,
    executedGateCount: 2,
    failedGateCount: 1,
    incomplete: true,
  });

  const markdown = renderReleaseConfidenceMarkdown(summary);
  assert.match(markdown, /### Acceptance Execution/);
  assert.match(markdown, /Status: failed/);
  assert.match(markdown, /Selected gates: 3/);
  assert.match(markdown, /Executed gates: 2/);
  assert.match(markdown, /Incomplete execution: yes/);
});

test("preserves gate-results execution reason evidence", () => {
  const summary = buildReleaseConfidenceSummary({
    acceptanceMatrix,
    gateResults: {
      status: "failed",
      reason: "runner stopped after gateway parity failed",
      selectedGateCount: 3,
      executedGateCount: 2,
      failedGateCount: 1,
      gates: [
        { label: "runtime authority fast gate", status: "passed" },
        { label: "gateway parity and degraded fallback smoke", status: "failed" },
      ],
    },
    evalReport: { report: { total_tasks: 1, success_rate: 1, score_summary: {} } },
  });

  assert.equal(summary.acceptance.execution.reason, "runner stopped after gateway parity failed");

  const markdown = renderReleaseConfidenceMarkdown(summary);
  assert.match(markdown, /Reason: runner stopped after gateway parity failed/);
});

test("flags declared capability claims that do not have acceptance evidence", () => {
  const summary = buildReleaseConfidenceSummary({
    acceptanceMatrix,
    gateResults: {
      gates: acceptanceMatrix.gates.map((gate) => ({ label: gate.label, status: "passed" })),
    },
    evalReport: { report: { total_tasks: 1, success_rate: 1, score_summary: {} } },
    boundaries: {
      capabilityClaims: [
        {
          id: "gateway read-only owner",
          domain: "gateway",
          evidenceGate: "gateway read-only owner smoke",
        },
      ],
    },
  });

  assert.equal(summary.status, "attention_required");
  assert.deepEqual(summary.affectedDomains, ["gateway"]);
  assert.deepEqual(summary.capabilityEvidence.missing, [
    {
      id: "gateway read-only owner",
      domain: "gateway",
      kind: "acceptance_gate",
      evidence: "gateway read-only owner smoke",
      reason: "missing_acceptance_gate",
    },
  ]);

  const markdown = renderReleaseConfidenceMarkdown(summary);
  assert.match(markdown, /gateway read-only owner \(gateway\): missing acceptance gate "gateway read-only owner smoke"/);
});

test("flags capability claims whose eval evidence score is failing", () => {
  const summary = buildReleaseConfidenceSummary({
    acceptanceMatrix,
    gateResults: {
      gates: acceptanceMatrix.gates.map((gate) => ({ label: gate.label, status: "passed" })),
    },
    evalReport: {
      report: {
        total_tasks: 1,
        success_rate: 1,
        score_summary: {
          forge_gateway_runtime_safety_ok: 0,
        },
      },
    },
    boundaries: {
      capabilityClaims: [
        {
          id: "gateway read-only owner",
          domain: "gateway",
          evidenceScore: "forge_gateway_runtime_safety_ok",
        },
      ],
    },
  });

  assert.equal(summary.status, "attention_required");
  assert.deepEqual(summary.capabilityEvidence.missing, [
    {
      id: "gateway read-only owner",
      domain: "gateway",
      kind: "eval_score",
      evidence: "forge_gateway_runtime_safety_ok",
      reason: "failing_eval_score",
      score: 0,
    },
  ]);

  const markdown = renderReleaseConfidenceMarkdown(summary);
  assert.match(markdown, /gateway read-only owner \(gateway\): failing eval score "forge_gateway_runtime_safety_ok" \(score: 0\)/);
});

test("flags capability claims whose acceptance gate evidence is failing", () => {
  const summary = buildReleaseConfidenceSummary({
    acceptanceMatrix,
    gateResults: {
      gates: [
        { label: "runtime authority fast gate", status: "passed" },
        {
          label: "gateway parity and degraded fallback smoke",
          status: "failed",
          reason: "gateway degraded fallback fixture failed",
        },
        { label: "desktop eval promotion evidence smoke", status: "passed" },
      ],
    },
    evalReport: { report: { total_tasks: 1, success_rate: 1, score_summary: {} } },
    boundaries: {
      capabilityClaims: [
        {
          id: "gateway degraded fallback",
          domain: "gateway",
          evidenceGate: "gateway parity and degraded fallback smoke",
        },
      ],
    },
  });

  assert.equal(summary.status, "failed");
  assert.deepEqual(summary.capabilityEvidence.missing, [
    {
      id: "gateway degraded fallback",
      domain: "gateway",
      kind: "acceptance_gate",
      evidence: "gateway parity and degraded fallback smoke",
      reason: "failing_acceptance_gate",
      status: "failed",
    },
  ]);

  const markdown = renderReleaseConfidenceMarkdown(summary);
  assert.match(markdown, /gateway degraded fallback \(gateway\): failing acceptance gate "gateway parity and degraded fallback smoke" \(status: failed\)/);
});

test("summarizes passing capability evidence references", () => {
  const summary = buildReleaseConfidenceSummary({
    acceptanceMatrix,
    gateResults: {
      gates: acceptanceMatrix.gates.map((gate) => ({ label: gate.label, status: "passed" })),
    },
    evalReport: {
      report: {
        total_tasks: 1,
        success_rate: 1,
        score_summary: {
          forge_gateway_runtime_safety_ok: 1,
        },
      },
    },
    boundaries: {
      capabilityClaims: [
        {
          id: "gateway read-only owner",
          domain: "gateway",
          evidenceGate: "gateway parity and degraded fallback smoke",
          evidenceScore: "forge_gateway_runtime_safety_ok",
        },
      ],
    },
  });

  assert.equal(summary.status, "passed");
  assert.deepEqual(summary.capabilityEvidence.missing, []);
  assert.deepEqual(summary.capabilityEvidence.passed, [
    {
      id: "gateway read-only owner",
      domain: "gateway",
      evidence: [
        {
          kind: "acceptance_gate",
          evidence: "gateway parity and degraded fallback smoke",
          status: "passed",
        },
        {
          kind: "eval_score",
          evidence: "forge_gateway_runtime_safety_ok",
          score: 1,
        },
      ],
    },
  ]);

  const markdown = renderReleaseConfidenceMarkdown(summary);
  assert.match(markdown, /Verified Capability Evidence/);
  assert.match(markdown, /gateway read-only owner \(gateway\): acceptance gate "gateway parity and degraded fallback smoke"; eval score "forge_gateway_runtime_safety_ok" \(score: 1\)/);
});

test("does not verify capability claims when gate results are missing", () => {
  const summary = buildReleaseConfidenceSummary({
    acceptanceMatrix,
    gateResults: {
      gates: [{ label: "runtime authority fast gate", status: "passed" }],
    },
    evalReport: { report: { total_tasks: 1, success_rate: 1, score_summary: {} } },
    boundaries: {
      capabilityClaims: [
        {
          id: "gateway degraded fallback",
          domain: "gateway",
          evidenceGate: "gateway parity and degraded fallback smoke",
        },
      ],
    },
  });

  assert.equal(summary.status, "attention_required");
  assert.deepEqual(summary.capabilityEvidence.passed, []);
  assert.deepEqual(summary.capabilityEvidence.missing, [
    {
      id: "gateway degraded fallback",
      domain: "gateway",
      kind: "acceptance_gate",
      evidence: "gateway parity and degraded fallback smoke",
      reason: "missing_acceptance_result",
      status: "unknown",
    },
  ]);

  const markdown = renderReleaseConfidenceMarkdown(summary);
  assert.match(markdown, /gateway degraded fallback \(gateway\): missing acceptance result "gateway parity and degraded fallback smoke"/);
});

test("cli emits machine-readable release confidence summary", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "forge-release-confidence-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));

  const matrixPath = join(dir, "acceptance.json");
  const gatesPath = join(dir, "gate-results.json");
  const evalPath = join(dir, "eval-report.json");
  writeFileSync(matrixPath, JSON.stringify(acceptanceMatrix), "utf8");
  writeFileSync(
    gatesPath,
    JSON.stringify({ gates: [{ label: "runtime authority fast gate", status: "passed" }] }),
    "utf8",
  );
  writeFileSync(
    evalPath,
    JSON.stringify({ report: { total_tasks: 1, success_rate: 1, score_summary: {} } }),
    "utf8",
  );

  const output = execFileSync(
    process.execPath,
    [scriptPath, "--json", "--acceptance-json", matrixPath, "--gate-results", gatesPath, "--eval-report", evalPath],
    { cwd: root, encoding: "utf8" },
  );
  const parsed = JSON.parse(output);

  assert.equal(parsed.acceptance.totalGates, 3);
  assert.equal(parsed.acceptance.passedGates.length, 1);
  assert.equal(parsed.acceptance.unknownGates.length, 2);
});

test("cli can scope release confidence to CI-default gates", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "forge-release-confidence-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));

  const matrixPath = join(dir, "acceptance.json");
  const gatesPath = join(dir, "gate-results.json");
  const evalPath = join(dir, "eval-report.json");
  writeFileSync(matrixPath, JSON.stringify(acceptanceMatrix), "utf8");
  writeFileSync(
    gatesPath,
    JSON.stringify({
      gates: acceptanceMatrix.gates
        .filter(({ ciDefault }) => ciDefault)
        .map((gate) => ({ label: gate.label, status: "passed" })),
    }),
    "utf8",
  );
  writeFileSync(
    evalPath,
    JSON.stringify({ report: { total_tasks: 1, success_rate: 1, score_summary: {} } }),
    "utf8",
  );

  const output = execFileSync(
    process.execPath,
    [
      scriptPath,
      "--json",
      "--ci-default-only",
      "--acceptance-json",
      matrixPath,
      "--gate-results",
      gatesPath,
      "--eval-report",
      evalPath,
    ],
    { cwd: root, encoding: "utf8" },
  );
  const parsed = JSON.parse(output);

  assert.equal(parsed.acceptanceScope, "ci-default");
  assert.equal(parsed.status, "passed");
  assert.equal(parsed.acceptance.totalGates, 2);
  assert.equal(parsed.acceptance.passedGates.length, 2);
  assert.equal(parsed.acceptance.unknownGates.length, 0);
  assert.equal(parsed.acceptance.ciDefault.totalGates, 2);
});

test("cli can summarize self-describing gate results without acceptance matrix", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "forge-release-confidence-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));

  const gatesPath = join(dir, "gate-results.json");
  const evalPath = join(dir, "eval-report.json");
  writeFileSync(
    gatesPath,
    JSON.stringify({
      gates: [
        {
          label: "runtime authority fast gate",
          command: "cargo test runtime",
          domain: "runtime",
          tier: "runtime-core",
          runtimeCost: "medium",
          manualRequirement: false,
          ciDefault: true,
          status: "passed",
        },
        {
          label: "gateway parity and degraded fallback smoke",
          command: "cargo test gateway",
          domain: "gateway",
          tier: "runtime-core",
          runtimeCost: "medium",
          manualRequirement: false,
          ciDefault: true,
          status: "failed",
          reason: "exit_code_1",
        },
      ],
    }),
    "utf8",
  );
  writeFileSync(
    evalPath,
    JSON.stringify({ report: { total_tasks: 1, success_rate: 1, score_summary: {} } }),
    "utf8",
  );

  const output = execFileSync(
    process.execPath,
    [scriptPath, "--json", "--no-acceptance-matrix", "--gate-results", gatesPath, "--eval-report", evalPath],
    { cwd: root, encoding: "utf8" },
  );
  const parsed = JSON.parse(output);

  assert.equal(parsed.status, "failed");
  assert.equal(parsed.acceptance.totalGates, 2);
  assert.equal(parsed.acceptance.passedGates.length, 1);
  assert.equal(parsed.acceptance.failedGates.length, 1);
  assert.deepEqual(parsed.acceptance.failedGates.map(({ label, domain, reason }) => ({ label, domain, reason })), [
    {
      label: "gateway parity and degraded fallback smoke",
      domain: "gateway",
      reason: "exit_code_1",
    },
  ]);
  assert.equal(parsed.acceptance.ciDefault.totalGates, 2);
  assert.deepEqual(parsed.affectedDomains, ["gateway"]);
});

test("cli can write dashboard JSON and Markdown artifacts to an output directory", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "forge-release-confidence-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));

  const matrixPath = join(dir, "acceptance.json");
  const gatesPath = join(dir, "gate-results.json");
  const evalPath = join(dir, "eval-report.json");
  const reportDir = join(dir, "confidence-report");
  writeFileSync(matrixPath, JSON.stringify(acceptanceMatrix), "utf8");
  writeFileSync(
    gatesPath,
    JSON.stringify({ gates: acceptanceMatrix.gates.map((gate) => ({ label: gate.label, status: "passed" })) }),
    "utf8",
  );
  writeFileSync(
    evalPath,
    JSON.stringify({ report: { total_tasks: 1, success_rate: 1, score_summary: {} } }),
    "utf8",
  );

  const stdout = execFileSync(
    process.execPath,
    [
      scriptPath,
      "--markdown",
      "--acceptance-json",
      matrixPath,
      "--gate-results",
      gatesPath,
      "--eval-report",
      evalPath,
      "--out-dir",
      reportDir,
    ],
    { cwd: root, encoding: "utf8" },
  );

  const jsonPath = join(reportDir, "release-confidence-summary.json");
  const markdownPath = join(reportDir, "release-confidence-summary.md");
  assert.equal(existsSync(jsonPath), true);
  assert.equal(existsSync(markdownPath), true);
  assert.equal(JSON.parse(readFileSync(jsonPath, "utf8")).status, "passed");
  assert.equal(readFileSync(markdownPath, "utf8"), stdout);
});

test("cli can fail when release confidence needs attention", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "forge-release-confidence-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));

  const matrixPath = join(dir, "acceptance.json");
  const gatesPath = join(dir, "gate-results.json");
  const evalPath = join(dir, "eval-report.json");
  const boundariesPath = join(dir, "boundaries.json");
  writeFileSync(matrixPath, JSON.stringify(acceptanceMatrix), "utf8");
  writeFileSync(
    gatesPath,
    JSON.stringify({ gates: acceptanceMatrix.gates.map((gate) => ({ label: gate.label, status: "passed" })) }),
    "utf8",
  );
  writeFileSync(
    evalPath,
    JSON.stringify({ report: { total_tasks: 1, success_rate: 1, score_summary: {} } }),
    "utf8",
  );
  writeFileSync(
    boundariesPath,
    JSON.stringify({
      capabilityClaims: [
        {
          id: "gateway read-only owner",
          domain: "gateway",
          evidenceGate: "gateway read-only owner smoke",
        },
      ],
    }),
    "utf8",
  );

  const result = spawnSync(
    process.execPath,
    [
      scriptPath,
      "--json",
      "--fail-on-attention",
      "--acceptance-json",
      matrixPath,
      "--gate-results",
      gatesPath,
      "--eval-report",
      evalPath,
      "--boundaries-json",
      boundariesPath,
    ],
    { cwd: root, encoding: "utf8" },
  );

  assert.equal(result.status, 1);
  assert.match(result.stderr, /Release confidence status: attention_required/);
  assert.equal(JSON.parse(result.stdout).status, "attention_required");
});
