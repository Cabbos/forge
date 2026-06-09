import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

import {
  parseArtifactFilename,
  loadArtifacts,
  summarizeArtifact,
  summarizeArtifacts,
  compareReports,
  buildComparisons,
  formatReport,
  formatTaskDetails,
} from "./eval-report.mjs";

function makeArtifact({ timestamp, suite, provider, report }) {
  return {
    report: report ?? {
      total_tasks: 1,
      success_rate: 1.0,
      verification_pass_rate: 1.0,
      scope_violation_rate: 0.0,
      avg_duration_ms: 5000,
      avg_model_rounds: 5,
      avg_confirm_requests: 2,
      avg_repair_attempts_used: 0,
      avg_validation_attempts: 1,
      failure_categories: {},
      tasks: [],
      continuity: {},
    },
    traces: [],
    timestamp,
    suite,
    provider,
  };
}

function writeArtifact(dir, timestamp, suite, provider, reportOverrides = {}) {
  const base = makeArtifact({ timestamp, suite, provider });
  const artifact = { ...base, report: { ...base.report, ...reportOverrides } };
  const filename = `${timestamp}-${suite}-${provider}.json`;
  writeFileSync(join(dir, filename), JSON.stringify(artifact, null, 2));
  return filename;
}

// ── parseArtifactFilename ──

test("parses valid artifact filename", () => {
  const result = parseArtifactFilename("/path/2026-06-09T05-11-25Z-forge-session-forge.json");
  assert.equal(result.timestamp, "2026-06-09T05-11-25Z");
  assert.equal(result.suite, "forge-session");
  assert.equal(result.provider, "forge");
});

test("parses filename with hyphenated suite", () => {
  const result = parseArtifactFilename("2026-06-09T01-02-03Z-continuity-pipeline-mock.json");
  assert.equal(result.suite, "continuity-pipeline");
  assert.equal(result.provider, "mock");
});

test("returns null for non-matching filename", () => {
  assert.equal(parseArtifactFilename("report.json"), null);
  assert.equal(parseArtifactFilename("2026-06-09-forge-session-forge.json"), null);
});

// ── loadArtifacts ──

test("loads artifacts from directory", () => {
  const dir = mkdtempSync(join(tmpdir(), "eval-report-load-"));
  try {
    writeArtifact(dir, "2026-06-09T01-00-00Z", "forge-session", "forge");
    writeArtifact(dir, "2026-06-09T02-00-00Z", "forge-session", "forge");
    writeArtifact(dir, "2026-06-09T03-00-00Z", "continuity", "mock");

    const artifacts = loadArtifacts(dir);
    assert.equal(artifacts.length, 3);
    assert.equal(artifacts[0].suite, "forge-session");
    assert.equal(artifacts[2].suite, "continuity");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("returns empty array for missing directory", () => {
  assert.deepEqual(loadArtifacts("/nonexistent/path"), []);
});

test("ignores unreadable or non-JSON files", () => {
  const dir = mkdtempSync(join(tmpdir(), "eval-report-bad-"));
  try {
    writeArtifact(dir, "2026-06-09T01-00-00Z", "forge-session", "forge");
    writeFileSync(join(dir, "corrupt.json"), "not json");
    writeFileSync(join(dir, "readme.txt"), "hello");

    const artifacts = loadArtifacts(dir);
    assert.equal(artifacts.length, 1);
    assert.equal(artifacts[0].suite, "forge-session");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// ── summarizeArtifact ──

test("summarizeArtifact extracts key metrics", () => {
  const summary = summarizeArtifact({
    timestamp: "2026-06-09T01-00-00Z",
    suite: "forge-session",
    provider: "forge",
    filename: "test.json",
    report: {
      total_tasks: 2,
      success_rate: 0.5,
      verification_pass_rate: 0.75,
      scope_violation_rate: 0.25,
      avg_model_rounds: 12,
      avg_duration_ms: 15000,
      failure_categories: { timeout: 1 },
    },
    traceCount: 2,
  });

  assert.equal(summary.totalTasks, 2);
  assert.equal(summary.successRate, 0.5);
  assert.equal(summary.verificationPassRate, 0.75);
  assert.equal(summary.scopeViolationRate, 0.25);
  assert.equal(summary.avgModelRounds, 12);
  assert.equal(summary.avgDurationMs, 15000);
  assert.deepEqual(summary.failureCategories, { timeout: 1 });
  assert.equal(summary.traceCount, 2);
});

// ── compareReports ──

test("compareReports detects success_rate drop", () => {
  const current = summarizeArtifact({
    timestamp: "2026-06-09T02-00-00Z",
    suite: "forge-session",
    provider: "forge",
    filename: "c.json",
    report: {
      total_tasks: 1,
      success_rate: 0.5,
      verification_pass_rate: 1.0,
      scope_violation_rate: 0.0,
      avg_model_rounds: 10,
      avg_duration_ms: 5000,
      failure_categories: {},
    },
    traceCount: 1,
  });
  const previous = summarizeArtifact({
    timestamp: "2026-06-09T01-00-00Z",
    suite: "forge-session",
    provider: "forge",
    filename: "p.json",
    report: {
      total_tasks: 1,
      success_rate: 1.0,
      verification_pass_rate: 1.0,
      scope_violation_rate: 0.0,
      avg_model_rounds: 10,
      avg_duration_ms: 5000,
      failure_categories: {},
    },
    traceCount: 1,
  });

  const comp = compareReports(current, previous);
  assert.equal(comp.hasRegression, true);
  assert.equal(comp.changes.length, 1);
  assert.equal(comp.changes[0].metric, "success_rate");
  assert.equal(comp.changes[0].severity, "critical");
});

test("compareReports detects critical success_rate drop", () => {
  const current = summarizeArtifact({
    timestamp: "2026-06-09T02-00-00Z", suite: "s", provider: "p", filename: "c.json",
    report: { total_tasks: 1, success_rate: 0.0, verification_pass_rate: 0, scope_violation_rate: 0, avg_model_rounds: 1, avg_duration_ms: 1, failure_categories: {} },
    traceCount: 0,
  });
  const previous = summarizeArtifact({
    timestamp: "2026-06-09T01-00-00Z", suite: "s", provider: "p", filename: "p.json",
    report: { total_tasks: 1, success_rate: 1.0, verification_pass_rate: 1, scope_violation_rate: 0, avg_model_rounds: 1, avg_duration_ms: 1, failure_categories: {} },
    traceCount: 0,
  });

  const comp = compareReports(current, previous);
  assert.equal(comp.hasRegression, true);
  assert.equal(comp.changes[0].severity, "critical");
});

test("compareReports detects scope_violation_rate increase", () => {
  const current = summarizeArtifact({
    timestamp: "2026-06-09T02-00-00Z", suite: "s", provider: "p", filename: "c.json",
    report: { total_tasks: 1, success_rate: 1.0, verification_pass_rate: 1, scope_violation_rate: 0.5, avg_model_rounds: 1, avg_duration_ms: 1, failure_categories: {} },
    traceCount: 0,
  });
  const previous = summarizeArtifact({
    timestamp: "2026-06-09T01-00-00Z", suite: "s", provider: "p", filename: "p.json",
    report: { total_tasks: 1, success_rate: 1.0, verification_pass_rate: 1, scope_violation_rate: 0.0, avg_model_rounds: 1, avg_duration_ms: 1, failure_categories: {} },
    traceCount: 0,
  });

  const comp = compareReports(current, previous);
  assert.equal(comp.hasRegression, true);
  assert.equal(comp.changes[0].metric, "scope_violation_rate");
});

test("compareReports detects model_rounds spike", () => {
  const current = summarizeArtifact({
    timestamp: "2026-06-09T02-00-00Z", suite: "s", provider: "p", filename: "c.json",
    report: { total_tasks: 1, success_rate: 1.0, verification_pass_rate: 1, scope_violation_rate: 0, avg_model_rounds: 50, avg_duration_ms: 1, failure_categories: {} },
    traceCount: 0,
  });
  const previous = summarizeArtifact({
    timestamp: "2026-06-09T01-00-00Z", suite: "s", provider: "p", filename: "p.json",
    report: { total_tasks: 1, success_rate: 1.0, verification_pass_rate: 1, scope_violation_rate: 0, avg_model_rounds: 10, avg_duration_ms: 1, failure_categories: {} },
    traceCount: 0,
  });

  const comp = compareReports(current, previous);
  assert.equal(comp.hasRegression, true);
  assert.equal(comp.changes[0].metric, "avg_model_rounds");
});

test("compareReports detects new failure categories", () => {
  const current = summarizeArtifact({
    timestamp: "2026-06-09T02-00-00Z", suite: "s", provider: "p", filename: "c.json",
    report: { total_tasks: 1, success_rate: 1.0, verification_pass_rate: 1, scope_violation_rate: 0, avg_model_rounds: 1, avg_duration_ms: 1, failure_categories: { timeout: 1 } },
    traceCount: 0,
  });
  const previous = summarizeArtifact({
    timestamp: "2026-06-09T01-00-00Z", suite: "s", provider: "p", filename: "p.json",
    report: { total_tasks: 1, success_rate: 1.0, verification_pass_rate: 1, scope_violation_rate: 0, avg_model_rounds: 1, avg_duration_ms: 1, failure_categories: {} },
    traceCount: 0,
  });

  const comp = compareReports(current, previous);
  assert.equal(comp.hasRegression, true);
  assert.equal(comp.changes[0].metric, "failure_category");
  assert.equal(comp.changes[0].current, "timeout");
});

test("compareReports returns no regression when metrics stable", () => {
  const current = summarizeArtifact({
    timestamp: "2026-06-09T02-00-00Z", suite: "s", provider: "p", filename: "c.json",
    report: { total_tasks: 1, success_rate: 1.0, verification_pass_rate: 1, scope_violation_rate: 0, avg_model_rounds: 10, avg_duration_ms: 5000, failure_categories: {} },
    traceCount: 0,
  });
  const previous = summarizeArtifact({
    timestamp: "2026-06-09T01-00-00Z", suite: "s", provider: "p", filename: "p.json",
    report: { total_tasks: 1, success_rate: 1.0, verification_pass_rate: 1, scope_violation_rate: 0, avg_model_rounds: 10, avg_duration_ms: 5000, failure_categories: {} },
    traceCount: 0,
  });

  const comp = compareReports(current, previous);
  assert.equal(comp.hasRegression, false);
  assert.equal(comp.changes.length, 0);
});

test("compareReports handles no previous run", () => {
  const current = summarizeArtifact({
    timestamp: "2026-06-09T02-00-00Z", suite: "s", provider: "p", filename: "c.json",
    report: { total_tasks: 1, success_rate: 1.0, verification_pass_rate: 1, scope_violation_rate: 0, avg_model_rounds: 1, avg_duration_ms: 1, failure_categories: {} },
    traceCount: 0,
  });

  const comp = compareReports(current, null);
  assert.equal(comp.hasRegression, false);
  assert.match(comp.message, /No previous run/);
});

// ── buildComparisons ──

test("buildComparisons groups by suite+provider", () => {
  const summaries = [
    { timestamp: "2026-06-09T01-00-00Z", suite: "forge-session", provider: "forge", totalTasks: 1, successRate: 1.0, verificationPassRate: 1.0, scopeViolationRate: 0, avgModelRounds: 10, avgDurationMs: 5000, failureCategories: {}, traceCount: 0, filename: "a" },
    { timestamp: "2026-06-09T02-00-00Z", suite: "forge-session", provider: "forge", totalTasks: 1, successRate: 0.5, verificationPassRate: 1.0, scopeViolationRate: 0, avgModelRounds: 10, avgDurationMs: 5000, failureCategories: {}, traceCount: 0, filename: "b" },
    { timestamp: "2026-06-09T03-00-00Z", suite: "continuity", provider: "mock", totalTasks: 1, successRate: 1.0, verificationPassRate: 1.0, scopeViolationRate: 0, avgModelRounds: 5, avgDurationMs: 2000, failureCategories: {}, traceCount: 0, filename: "c" },
  ];

  const comps = buildComparisons(summaries);
  assert.equal(comps.length, 1); // only forge-session has 2 runs
  assert.equal(comps[0].hasRegression, true);
});

// ── formatReport ──

test("formatReport includes summary lines", () => {
  const text = formatReport({
    summaries: [
      { timestamp: "2026-06-09T01-00-00Z", suite: "forge-session", provider: "forge", totalTasks: 1, successRate: 1.0, verificationPassRate: 1.0, scopeViolationRate: 0.0, avgModelRounds: 10, avgDurationMs: 5000, failureCategories: {}, traceCount: 0, filename: "a" },
    ],
    comparisons: [{ hasRegression: false, changes: [], message: "No regressions." }],
    totalCount: 3,
  });

  assert.match(text, /Forge Eval Report/);
  assert.match(text, /Total artifacts/);
  assert.match(text, /forge-session/);
  assert.match(text, /forge/);
  assert.match(text, /success_rate=1.00/);
});

test("formatReport highlights regressions", () => {
  const text = formatReport({
    summaries: [
      { timestamp: "2026-06-09T01-00-00Z", suite: "s", provider: "p", totalTasks: 1, successRate: 1.0, verificationPassRate: 1.0, scopeViolationRate: 0.0, avgModelRounds: 10, avgDurationMs: 5000, failureCategories: {}, traceCount: 0, filename: "a" },
      { timestamp: "2026-06-09T02-00-00Z", suite: "s", provider: "p", totalTasks: 1, successRate: 0.0, verificationPassRate: 0.0, scopeViolationRate: 1.0, avgModelRounds: 60, avgDurationMs: 5000, failureCategories: { timeout: 1 }, traceCount: 0, filename: "b" },
    ],
    comparisons: [
      {
        hasRegression: true,
        changes: [
          { metric: "success_rate", direction: "down", previous: 1.0, current: 0.0, delta: 1.0, severity: "critical" },
          { metric: "scope_violation_rate", direction: "up", previous: 0.0, current: 1.0, delta: 1.0, severity: "critical" },
        ],
        message: "Regressions detected.",
      },
    ],
    totalCount: 2,
  });

  assert.match(text, /REGRESSIONS DETECTED/);
  assert.match(text, /success_rate/);
  assert.match(text, /scope_violation_rate/);
});

// ── formatTaskDetails ──

test("formatTaskDetails shows per-task metrics", () => {
  const lines = formatTaskDetails({
    report: {
      tasks: [
        {
          task_id: "capitalize",
          passed: true,
          failure_category: "none",
          failure_reason: null,
          model_rounds: 16,
          confirm_requests: 11,
          repair_attempts_used: 1,
          validation_attempts: 2,
          scope_violations: [],
          changed_files: ["src/capitalize.ts", "src/capitalize.test.ts"],
          duration_ms: 71216,
        },
      ],
    },
  });
  assert.equal(lines.length, 2);
  assert.match(lines[0], /capitalize/);
  assert.match(lines[0], /rounds=16/);
  assert.match(lines[0], /confirms=11/);
  assert.match(lines[0], /repairs=1/);
  assert.match(lines[0], /validations=2/);
  assert.match(lines[1], /changed_files:/);
});

test("formatTaskDetails highlights scope violations", () => {
  const lines = formatTaskDetails({
    report: {
      tasks: [
        {
          task_id: "bad-scope",
          passed: false,
          failure_category: "scope_violation",
          failure_reason: "Changed files violated eval scope",
          model_rounds: 8,
          confirm_requests: 3,
          repair_attempts_used: 0,
          validation_attempts: 1,
          scope_violations: ["unexpected_change:package-lock.json"],
          changed_files: ["package-lock.json"],
          duration_ms: 12000,
        },
      ],
    },
  });
  assert.equal(lines.length, 4);
  assert.match(lines[0], /bad-scope/);
  assert.match(lines[0], /❌/);
  assert.match(lines[0], /scope_violation/);
  assert.match(lines[1], /reason:/);
  assert.match(lines[2], /scope_violations:/);
  assert.match(lines[3], /changed_files:/);
});

test("formatTaskDetails filters to failures only when requested", () => {
  const lines = formatTaskDetails(
    {
      report: {
        tasks: [
          { task_id: "ok", passed: true, failure_category: "none", model_rounds: 5, confirm_requests: 2, repair_attempts_used: 0, validation_attempts: 1, scope_violations: [], changed_files: ["a.ts"], duration_ms: 5000 },
          { task_id: "bad", passed: false, failure_category: "verification_failed", model_rounds: 20, confirm_requests: 10, repair_attempts_used: 2, validation_attempts: 3, scope_violations: [], changed_files: ["b.ts"], duration_ms: 120000 },
        ],
      },
    },
    { failuresOnly: true }
  );
  assert.equal(lines.length, 2);
  assert.match(lines[0], /bad/);
  assert.doesNotMatch(lines[0], /ok/);
  assert.match(lines[1], /changed_files:/);
});

test("formatTaskDetails returns empty for empty tasks", () => {
  const lines = formatTaskDetails({ report: { tasks: [] } });
  assert.equal(lines.length, 0);
});

// ── CLI integration ──

test("CLI prints report for artifacts directory", () => {
  const dir = mkdtempSync(join(tmpdir(), "eval-report-cli-"));
  try {
    writeArtifact(dir, "2026-06-09T01-00-00Z", "forge-session", "forge", {
      success_rate: 1.0,
      scope_violation_rate: 0.0,
      avg_model_rounds: 10,
      avg_duration_ms: 5000,
    });
    writeArtifact(dir, "2026-06-09T02-00-00Z", "forge-session", "forge", {
      success_rate: 0.5,
      scope_violation_rate: 0.5,
      avg_model_rounds: 30,
      avg_duration_ms: 15000,
    });

    const scriptPath = join(process.cwd(), "scripts", "eval-report.mjs");
    const result = spawnSync(process.execPath, [scriptPath, dir], {
      encoding: "utf8",
    });

    assert.equal(result.status, 2, "Should exit 2 on regression");
    assert.match(result.stdout, /Forge Eval Report/);
    assert.match(result.stdout, /success_rate=1\.00/);
    assert.match(result.stdout, /success_rate=0\.50/);
    assert.match(result.stdout, /REGRESSIONS DETECTED/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("CLI --latest prints only most recent run", () => {
  const dir = mkdtempSync(join(tmpdir(), "eval-report-latest-"));
  try {
    writeArtifact(dir, "2026-06-09T01-00-00Z", "forge-session", "forge", {
      success_rate: 1.0,
      scope_violation_rate: 0.0,
      avg_model_rounds: 10,
      avg_duration_ms: 5000,
    });
    writeArtifact(dir, "2026-06-09T02-00-00Z", "forge-session", "forge", {
      success_rate: 1.0,
      scope_violation_rate: 0.0,
      avg_model_rounds: 12,
      avg_duration_ms: 6000,
    });

    const scriptPath = join(process.cwd(), "scripts", "eval-report.mjs");
    const result = spawnSync(process.execPath, [scriptPath, dir, "--latest"], {
      encoding: "utf8",
    });

    assert.equal(result.status, 0, `Should exit 0: ${result.stderr}`);
    assert.match(result.stdout, /Forge Eval Report/);
    // Should only show the latest run
    const lines = result.stdout.split("\n").filter((l) => l.includes("success_rate="));
    assert.equal(lines.length, 1, "Should show only 1 run with --latest");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("CLI --failures shows only failed tasks", () => {
  const dir = mkdtempSync(join(tmpdir(), "eval-report-failures-"));
  try {
    writeArtifact(dir, "2026-06-09T01-00-00Z", "forge-session", "forge", {
      success_rate: 0.5,
      scope_violation_rate: 0.5,
      avg_model_rounds: 20,
      avg_duration_ms: 15000,
      tasks: [
        { task_id: "ok-task", passed: true, failure_category: "none", failure_reason: null, model_rounds: 5, confirm_requests: 2, repair_attempts_used: 0, validation_attempts: 1, scope_violations: [], changed_files: ["a.ts"], duration_ms: 5000 },
        { task_id: "fail-task", passed: false, failure_category: "verification_failed", failure_reason: "npm test failed", model_rounds: 20, confirm_requests: 10, repair_attempts_used: 2, validation_attempts: 3, scope_violations: [], changed_files: ["b.ts"], duration_ms: 120000 },
      ],
    });

    const scriptPath = join(process.cwd(), "scripts", "eval-report.mjs");
    const result = spawnSync(process.execPath, [scriptPath, dir, "--failures"], {
      encoding: "utf8",
    });

    assert.equal(result.status, 0, `Should exit 0 (single artifact, no regression comparison): ${result.stderr}`);
    assert.match(result.stdout, /fail-task/);
    assert.doesNotMatch(result.stdout, /ok-task/);
    assert.match(result.stdout, /verification_failed/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("CLI exits 0 when no artifacts found", () => {
  const dir = mkdtempSync(join(tmpdir(), "eval-report-empty-"));
  try {
    const scriptPath = join(process.cwd(), "scripts", "eval-report.mjs");
    const result = spawnSync(process.execPath, [scriptPath, dir], {
      encoding: "utf8",
    });

    assert.equal(result.status, 0);
    assert.match(result.stdout, /No eval artifacts found/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
