import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, basename } from "node:path";

const ARTIFACT_PATTERN = /^(\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}Z)-(.+)-(.+)\.json$/;

export function parseArtifactFilename(filename) {
  const match = basename(filename).match(ARTIFACT_PATTERN);
  if (!match) return null;
  const [, timestamp, suite, provider] = match;
  return {
    filename: basename(filename),
    filepath: filename,
    timestamp: timestamp,
    suite,
    provider,
  };
}

export function loadArtifacts(artifactsDir) {
  if (!existsSync(artifactsDir)) {
    return [];
  }
  const files = readdirSync(artifactsDir)
    .filter((name) => name.endsWith(".json"))
    .map((name) => parseArtifactFilename(join(artifactsDir, name)))
    .filter(Boolean)
    .sort((a, b) => a.timestamp.localeCompare(b.timestamp));

  for (const file of files) {
    try {
      const content = JSON.parse(readFileSync(file.filepath, "utf8"));
      file.report = content.report ?? null;
      file.traceCount = Array.isArray(content.traces) ? content.traces.length : 0;
    } catch {
      file.report = null;
      file.traceCount = 0;
    }
  }
  return files.filter((f) => f.report !== null);
}

export function summarizeArtifact(artifact) {
  const r = artifact.report;
  return {
    timestamp: artifact.timestamp,
    suite: artifact.suite,
    provider: artifact.provider,
    filename: artifact.filename,
    totalTasks: r.total_tasks ?? 0,
    successRate: r.success_rate ?? 0,
    verificationPassRate: r.verification_pass_rate ?? 0,
    scopeViolationRate: r.scope_violation_rate ?? 0,
    avgModelRounds: r.avg_model_rounds ?? 0,
    avgDurationMs: r.avg_duration_ms ?? 0,
    failureCategories: r.failure_categories ?? {},
    traceCount: artifact.traceCount ?? 0,
  };
}

export function summarizeArtifacts(artifacts, { limit = 10 } = {}) {
  const summaries = artifacts
    .slice(-limit)
    .map(summarizeArtifact);
  return { summaries, totalCount: artifacts.length };
}

export function evaluateQualityGate(summary) {
  const failures = [];
  if (summary.totalTasks <= 0) {
    failures.push({
      metric: "total_tasks",
      expected: ">0",
      current: summary.totalTasks,
      severity: "critical",
    });
  }
  if (summary.successRate < 1) {
    failures.push({
      metric: "success_rate",
      expected: 1,
      current: summary.successRate,
      severity: "critical",
    });
  }
  if (summary.verificationPassRate < 1) {
    failures.push({
      metric: "verification_pass_rate",
      expected: 1,
      current: summary.verificationPassRate,
      severity: "critical",
    });
  }
  if (summary.scopeViolationRate > 0) {
    failures.push({
      metric: "scope_violation_rate",
      expected: 0,
      current: summary.scopeViolationRate,
      severity: "critical",
    });
  }
  return failures;
}

export function compareReports(current, previous) {
  if (!previous) {
    return { hasRegression: false, changes: [], message: "No previous run to compare." };
  }

  const changes = [];

  // success_rate drop
  const srDrop = previous.successRate - current.successRate;
  if (srDrop > 0) {
    changes.push({
      metric: "success_rate",
      direction: "down",
      previous: previous.successRate,
      current: current.successRate,
      delta: srDrop,
      severity: srDrop >= 0.5 ? "critical" : "warning",
    });
  }

  // scope_violation_rate increase
  const svIncrease = current.scopeViolationRate - previous.scopeViolationRate;
  if (svIncrease > 0) {
    changes.push({
      metric: "scope_violation_rate",
      direction: "up",
      previous: previous.scopeViolationRate,
      current: current.scopeViolationRate,
      delta: svIncrease,
      severity: svIncrease >= 0.5 ? "critical" : "warning",
    });
  }

  // model_rounds spike (> 2x)
  if (previous.avgModelRounds > 0) {
    const ratio = current.avgModelRounds / previous.avgModelRounds;
    if (ratio > 2) {
      changes.push({
        metric: "avg_model_rounds",
        direction: "up",
        previous: previous.avgModelRounds,
        current: current.avgModelRounds,
        delta: current.avgModelRounds - previous.avgModelRounds,
        severity: "warning",
      });
    }
  }

  // duration spike (> 3x)
  if (previous.avgDurationMs > 0) {
    const ratio = current.avgDurationMs / previous.avgDurationMs;
    if (ratio > 3) {
      changes.push({
        metric: "avg_duration_ms",
        direction: "up",
        previous: previous.avgDurationMs,
        current: current.avgDurationMs,
        delta: current.avgDurationMs - previous.avgDurationMs,
        severity: "warning",
      });
    }
  }

  // new failure categories
  const prevFailures = Object.keys(previous.failureCategories ?? {});
  const currFailures = Object.keys(current.failureCategories ?? {});
  const newFailures = currFailures.filter((f) => !prevFailures.includes(f));
  for (const f of newFailures) {
    changes.push({
      metric: "failure_category",
      direction: "new",
      previous: null,
      current: f,
      delta: null,
      severity: "warning",
    });
  }

  const hasRegression = changes.some((c) => c.severity === "critical" || c.severity === "warning");
  return { hasRegression, changes, message: hasRegression ? "Regressions detected." : "No regressions." };
}

export function buildComparisons(summaries) {
  const comparisons = [];
  // Group by suite+provider, compare consecutive runs
  const groups = new Map();
  for (const s of summaries) {
    const key = `${s.suite}:${s.provider}`;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(s);
  }
  for (const [, runs] of groups) {
    for (let i = 1; i < runs.length; i++) {
      comparisons.push(compareReports(runs[i], runs[i - 1]));
    }
  }
  return comparisons;
}

export function buildLatestComparisons(allSummaries, latestSummaries) {
  const comparisons = [];
  for (const latest of latestSummaries) {
    const runs = allSummaries.filter(
      (s) => s.suite === latest.suite && s.provider === latest.provider
    );
    const latestIndex = runs.findIndex(
      (s) => s.timestamp === latest.timestamp && s.filename === latest.filename
    );
    comparisons.push(compareReports(latest, latestIndex > 0 ? runs[latestIndex - 1] : null));
  }
  return comparisons;
}

export function formatTaskDetails(artifact, { failuresOnly = false } = {}) {
  const tasks = artifact.report?.tasks ?? [];
  const lines = [];
  for (const t of tasks) {
    const isFailure = !t.passed || (t.failure_category && t.failure_category !== "none");
    if (failuresOnly && !isFailure) continue;

    const status = t.passed ? "✅" : "❌";
    const category = t.failure_category && t.failure_category !== "none"
      ? `category=${t.failure_category}`
      : "";
    const parts = [
      `  ${status} ${t.task_id}`,
      `rounds=${t.model_rounds ?? 0}`,
      `confirms=${t.confirm_requests ?? 0}`,
      `repairs=${t.repair_attempts_used ?? 0}`,
      `validations=${t.validation_attempts ?? 0}`,
    ];
    if (category) parts.push(category);
    if (t.scope_violations?.length > 0) parts.push(`scope_violations=${t.scope_violations.length}`);
    lines.push(parts.join("  "));

    if (t.failure_reason) {
      lines.push(`      reason: ${t.failure_reason}`);
    }
    if (t.scope_violations?.length > 0) {
      lines.push(`      scope_violations: ${t.scope_violations.join(", ")}`);
      if (t.expected_files_changed?.length > 0) {
        lines.push(`      expected: ${t.expected_files_changed.join(", ")}`);
      }
      if (t.forbidden_files_changed?.length > 0) {
        lines.push(`      forbidden: ${t.forbidden_files_changed.join(", ")}`);
      }
    }
    if (t.changed_files?.length > 0) {
      lines.push(`      changed_files: ${t.changed_files.join(", ")}`);
    }
  }
  return lines;
}

export function formatReport({
  summaries,
  comparisons,
  totalCount,
  artifacts = [],
  failuresOnly = false,
  qualityGateFailures = null,
}) {
  const lines = [];
  lines.push("╔══════════════════════════════════════════════════════════════╗");
  lines.push("║           Forge Eval Report                                  ║");
  lines.push("╚══════════════════════════════════════════════════════════════╝");
  lines.push(`Total artifacts on disk: ${totalCount}`);
  lines.push("");

  for (const s of summaries) {
    lines.push(`─ ${s.timestamp}  ${s.suite} / ${s.provider} ─`);
    lines.push(`  success_rate=${s.successRate.toFixed(2)}  verification=${s.verificationPassRate.toFixed(2)}  scope_violation=${s.scopeViolationRate.toFixed(2)}`);
    lines.push(`  avg_model_rounds=${s.avgModelRounds.toFixed(1)}  avg_duration=${(s.avgDurationMs / 1000).toFixed(1)}s  tasks=${s.totalTasks}`);
    const fc = Object.entries(s.failureCategories);
    if (fc.length > 0) {
      lines.push(`  failures: ${fc.map(([k, v]) => `${k}=${v}`).join(", ")}`);
    }

    // Per-task details
    const artifact = artifacts.find(
      (a) => a.timestamp === s.timestamp && a.suite === s.suite && a.provider === s.provider
    );
    if (artifact) {
      const taskLines = formatTaskDetails(artifact, { failuresOnly });
      if (taskLines.length > 0) {
        lines.push(failuresOnly ? "  Failed tasks:" : "  Tasks:");
        lines.push(...taskLines);
      }
    }
    lines.push("");
  }

  if (Array.isArray(qualityGateFailures)) {
    if (qualityGateFailures.length > 0) {
      lines.push("❌ CURRENT EVAL QUALITY GATE FAILED");
      for (const failure of qualityGateFailures) {
        lines.push(`  🔴 ${failure.metric}: ${formatQualityGateFailure(failure)}`);
      }
    } else {
      lines.push("✅ Current eval quality gate passed.");
    }
    lines.push("");
  }

  const anyRegression = comparisons.some((c) => c.hasRegression);
  if (anyRegression) {
    lines.push("⚠️  HISTORICAL REGRESSIONS DETECTED");
    for (const comp of comparisons) {
      for (const ch of comp.changes) {
        const emoji = ch.severity === "critical" ? "🔴" : "🟡";
        lines.push(`  ${emoji} ${ch.metric}: ${formatChange(ch)}`);
      }
    }
  } else {
    lines.push("✅ No regressions detected between consecutive runs.");
  }

  return lines.join("\n");
}

function formatQualityGateFailure(failure) {
  const expected = typeof failure.expected === "number" ? failure.expected.toFixed(2) : failure.expected;
  const current = typeof failure.current === "number" ? failure.current.toFixed(2) : failure.current;
  return `expected ${expected}, got ${current}`;
}

function formatChange(change) {
  if (change.metric === "failure_category") {
    return `new category "${change.current}"`;
  }
  const prev = typeof change.previous === "number" ? change.previous.toFixed(2) : change.previous;
  const curr = typeof change.current === "number" ? change.current.toFixed(2) : change.current;
  return `${prev} → ${curr} (Δ ${change.delta >= 0 ? "+" : ""}${typeof change.delta === "number" ? change.delta.toFixed(2) : change.delta})`;
}

function defaultArtifactsDir() {
  const scriptDir = new URL(".", import.meta.url).pathname;
  return join(scriptDir, "..", "artifacts", "eval-runs");
}

function runCli(argv) {
  const args = argv.slice(2);
  const latestOnly = args.includes("--latest");
  const failuresOnly = args.includes("--failures");
  const nonFlagArgs = args.filter((a) => !a.startsWith("-"));
  const artifactsDir = nonFlagArgs[0] ?? defaultArtifactsDir();

  const artifacts = loadArtifacts(artifactsDir);
  if (artifacts.length === 0) {
    console.log(`No eval artifacts found in ${artifactsDir}`);
    return 0;
  }

  const { summaries: allSummaries, totalCount } = summarizeArtifacts(artifacts, {
    limit: artifacts.length,
  });
  const summaries = latestOnly ? allSummaries.slice(-1) : allSummaries.slice(-10);
  const comparisons = latestOnly
    ? buildLatestComparisons(allSummaries, summaries)
    : buildComparisons(summaries);
  const qualityGateFailures = latestOnly
    ? summaries.flatMap((summary) => evaluateQualityGate(summary))
    : null;

  console.log(formatReport({
    summaries,
    comparisons,
    totalCount,
    artifacts,
    failuresOnly,
    qualityGateFailures,
  }));
  return comparisons.some((c) => c.hasRegression) || qualityGateFailures?.length > 0 ? 2 : 0;
}

if (import.meta.url === new URL(process.argv[1], "file:").href) {
  process.exitCode = runCli(process.argv);
}
