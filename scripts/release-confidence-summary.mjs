#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const root = join(__dirname, "..");

const PASS_STATUSES = new Set(["pass", "passed", "ok", "success", "succeeded"]);
const FAIL_STATUSES = new Set(["fail", "failed", "error", "errored", "timeout", "timed_out"]);
const MANUAL_STATUSES = new Set(["manual", "manual_pending", "pending_manual", "skipped_manual"]);

export function buildReleaseConfidenceSummary({
  acceptanceMatrix,
  gateResults = null,
  evalReport = null,
  boundaries = null,
  generatedAt = null,
  acceptanceScope = "all",
} = {}) {
  const normalizedAcceptanceScope = normalizeAcceptanceScope(acceptanceScope);
  const allGates = Array.isArray(acceptanceMatrix?.gates)
    ? acceptanceMatrix.gates
    : gatesFromGateResults(gateResults);
  const gates =
    normalizedAcceptanceScope === "ci-default"
      ? allGates.filter((gate) => gate?.ciDefault === true)
      : allGates;
  const resultByLabel = new Map();
  for (const result of asArray(gateResults?.gates)) {
    if (typeof result?.label === "string" && result.label) {
      resultByLabel.set(result.label, result);
    }
  }

  const acceptance = summarizeAcceptance(gates, resultByLabel);
  const evalSummary = summarizeEval(evalReport);
  const undeclaredBoundaries = summarizeUndeclaredBoundaries(boundaries);
  const capabilityEvidence = summarizeCapabilityEvidence({
    claims: boundaries?.capabilityClaims ?? boundaries?.claims,
    gates,
    resultByLabel,
    evalReport,
  });
  const affectedDomains = summarizeAffectedDomains({
    acceptance,
    evalSummary,
    undeclaredBoundaries,
    capabilityEvidence,
  });
  const status = summarizeStatus({ acceptance, evalSummary, undeclaredBoundaries, capabilityEvidence });

  return {
    schemaVersion: 1,
    generatedAt: generatedAt ?? new Date().toISOString(),
    acceptanceScope: normalizedAcceptanceScope,
    status,
    affectedDomains,
    acceptance,
    eval: evalSummary,
    undeclaredBoundaries,
    capabilityEvidence,
  };
}

function normalizeAcceptanceScope(scope) {
  return scope === "ci-default" ? "ci-default" : "all";
}

function gatesFromGateResults(gateResults) {
  return asArray(gateResults?.gates)
    .filter((gate) => typeof gate?.label === "string" && gate.label)
    .map((gate, index) => ({
      index: Number(gate.index ?? index + 1),
      label: gate.label,
      command: stringValue(gate.command) ?? "",
      domain: stringValue(gate.domain) ?? "unknown",
      tier: stringValue(gate.tier) ?? "unknown",
      runtimeCost: stringValue(gate.runtimeCost) ?? "unknown",
      manualRequirement: gate.manualRequirement === true,
      ciDefault: gate.ciDefault === true,
    }));
}

export function renderReleaseConfidenceMarkdown(summary) {
  const lines = [
    "# Forge Release Confidence",
    "",
    `Status: ${summary.status}`,
    `Affected domains: ${summary.affectedDomains.length ? summary.affectedDomains.join(", ") : "none"}`,
    "",
    "## Acceptance",
    "",
    `Total gates: ${summary.acceptance.totalGates}`,
    `Passed gates: ${summary.acceptance.passedGates.length}`,
    `Failed gates: ${summary.acceptance.failedGates.length}`,
    `Manual evidence gates: ${summary.acceptance.manualEvidenceGates.length}`,
    `Unknown gates: ${summary.acceptance.unknownGates.length}`,
    `CI-default gates: ${summary.acceptance.ciDefault?.totalGates ?? 0}`,
    `CI-default passed: ${summary.acceptance.ciDefault?.passedGates?.length ?? 0}`,
    `CI-default failed: ${summary.acceptance.ciDefault?.failedGates?.length ?? 0}`,
    `CI-default unknown: ${summary.acceptance.ciDefault?.unknownGates?.length ?? 0}`,
  ];

  appendGateList(lines, "Failed Gate Details", summary.acceptance.failedGates);
  appendGateList(lines, "Manual Evidence", summary.acceptance.manualEvidenceGates);
  appendGateList(lines, "Unknown Gate Results", summary.acceptance.unknownGates);

  lines.push("", "## Eval", "");
  if (summary.eval.available) {
    lines.push(
      `Total tasks: ${summary.eval.totalTasks}`,
      `Success rate: ${formatPercent(summary.eval.successRate)}`,
      `Failing scores: ${summary.eval.failingScores.length}`,
    );
    if (summary.eval.failingScores.length > 0) {
      lines.push("");
      for (const score of summary.eval.failingScores) {
        const tasks = asArray(score.failingTasks);
        const taskSuffix = tasks.length ? `; tasks: ${tasks.join(", ")}` : "";
        lines.push(`- ${score.name}: ${score.score} (${score.domain}${taskSuffix})`);
      }
    }
  } else {
    lines.push("Eval report: not provided");
  }

  lines.push("", "## Undeclared Boundaries", "");
  if (summary.undeclaredBoundaries.length === 0) {
    lines.push("None detected from provided boundary evidence.");
  } else {
    for (const boundary of summary.undeclaredBoundaries) {
      lines.push(`- ${boundary.id} (${boundary.domain})`);
    }
  }

  lines.push("", "## Capability Evidence", "");
  const missingCapabilityEvidence = asArray(summary.capabilityEvidence?.missing);
  const passedCapabilityEvidence = asArray(summary.capabilityEvidence?.passed);
  if (passedCapabilityEvidence.length > 0) {
    lines.push("### Verified Capability Evidence", "");
    for (const item of passedCapabilityEvidence) {
      lines.push(`- ${item.id} (${item.domain}): ${capabilityEvidencePassedMessage(item)}`);
    }
    lines.push("");
  }
  if (missingCapabilityEvidence.length === 0) {
    lines.push("All declared capability claims have matching evidence references.");
  } else {
    for (const item of missingCapabilityEvidence) {
      lines.push(`- ${item.id} (${item.domain}): ${capabilityEvidenceMessage(item)}`);
    }
  }

  return `${lines.join("\n")}\n`;
}

function summarizeAcceptance(gates, resultByLabel) {
  const passedGates = [];
  const failedGates = [];
  const manualEvidenceGates = [];
  const unknownGates = [];

  for (const gate of gates) {
    const result = resultByLabel.get(gate.label);
    const normalizedStatus = normalizeStatus(result?.status);
    const item = {
      label: gate.label,
      domain: gate.domain ?? "unknown",
      tier: stringValue(gate.tier) ?? "unknown",
      runtimeCost: stringValue(gate.runtimeCost) ?? "unknown",
      manualRequirement:
        typeof gate.manualRequirement === "boolean" ? gate.manualRequirement : "unknown",
      status: normalizedStatus,
      reason: result?.reason ?? result?.message ?? null,
      ciDefault: gate.ciDefault === true,
    };

    if (PASS_STATUSES.has(normalizedStatus)) {
      passedGates.push(item);
    } else if (FAIL_STATUSES.has(normalizedStatus)) {
      failedGates.push(item);
    } else if (MANUAL_STATUSES.has(normalizedStatus)) {
      manualEvidenceGates.push(item);
    } else {
      unknownGates.push(item);
    }
  }

  return {
    totalGates: gates.length,
    passedGates,
    failedGates,
    manualEvidenceGates,
    unknownGates,
    ciDefault: {
      totalGates: gates.filter((gate) => gate.ciDefault === true).length,
      passedGates: passedGates.filter((gate) => gate.ciDefault),
      failedGates: failedGates.filter((gate) => gate.ciDefault),
      manualEvidenceGates: manualEvidenceGates.filter((gate) => gate.ciDefault),
      unknownGates: unknownGates.filter((gate) => gate.ciDefault),
    },
  };
}

function summarizeEval(evalReport) {
  const report = evalReport?.report ?? evalReport;
  if (!report || typeof report !== "object") {
    return {
      available: false,
      totalTasks: 0,
      successRate: null,
      failingScores: [],
    };
  }

  const scoreSummary = report.score_summary ?? report.scoreSummary ?? {};
  const failingScores = Object.keys(scoreSummary).length
    ? failingScoresFromScoreSummary(scoreSummary)
    : failingScoresFromTaskMetrics(report);

  return {
    available: true,
    totalTasks: Number(report.total_tasks ?? report.totalTasks ?? 0),
    successRate: typeof report.success_rate === "number" ? report.success_rate : report.successRate ?? null,
    failingScores,
  };
}

function failingScoresFromScoreSummary(scoreSummary) {
  return Object.entries(scoreSummary)
    .filter(([, score]) => typeof score === "number" && score < 1)
    .map(([name, score]) => ({
      name,
      score,
      domain: domainForScore(name),
    }))
    .sort(compareScoreNames);
}

function failingScoresFromTaskMetrics(report) {
  const failingByName = new Map();
  for (const task of taskMetricEntries(report)) {
    const taskId = String(task.task_id ?? task.taskId ?? task.id ?? "unknown");
    for (const [name, rawScore] of Object.entries(task.scores ?? {})) {
      const score = scoreValue(rawScore);
      if (typeof score !== "number" || score >= 1) continue;
      const existing = failingByName.get(name) ?? {
        name,
        score,
        domain: domainForScore(name),
        failingTasks: [],
      };
      existing.score = Math.min(existing.score, score);
      if (!existing.failingTasks.includes(taskId)) {
        existing.failingTasks.push(taskId);
      }
      failingByName.set(name, existing);
    }
  }
  return [...failingByName.values()]
    .map((score) => ({ ...score, failingTasks: score.failingTasks.sort() }))
    .sort(compareScoreNames);
}

function taskMetricEntries(report) {
  return asArray(report.task_metrics ?? report.taskMetrics ?? report.metrics);
}

function scoreValue(rawScore) {
  if (typeof rawScore === "number") return rawScore;
  if (rawScore && typeof rawScore === "object" && typeof rawScore.score === "number") {
    return rawScore.score;
  }
  return null;
}

function compareScoreNames(left, right) {
  return left.name.localeCompare(right.name);
}

function summarizeUndeclaredBoundaries(boundaries) {
  const declared = new Set(
    asArray(boundaries?.declared).map((entry) =>
      typeof entry === "string" ? entry : String(entry?.id ?? ""),
    ),
  );
  return asArray(boundaries?.required)
    .map((entry) =>
      typeof entry === "string"
        ? { id: entry, domain: "unknown" }
        : { id: String(entry?.id ?? ""), domain: String(entry?.domain ?? "unknown") },
    )
    .filter((entry) => entry.id && !declared.has(entry.id));
}

function summarizeCapabilityEvidence({ claims, gates, resultByLabel, evalReport }) {
  const gateLabels = new Set(
    asArray(gates)
      .map((gate) => stringValue(gate?.label))
      .filter(Boolean),
  );
  const scoreNames = scoreNamesFromEvalReport(evalReport);
  const scoreValues = scoreValuesFromEvalReport(evalReport);
  const missing = [];
  const passed = [];

  for (const rawClaim of asArray(claims)) {
    const claim = normalizeCapabilityClaim(rawClaim);
    if (!claim.id) continue;

    let evidenceReferences = 0;
    const passedEvidence = [];
    const missingBeforeClaim = missing.length;
    if (claim.evidenceGate) {
      evidenceReferences += 1;
      if (!gateLabels.has(claim.evidenceGate)) {
        missing.push({
          id: claim.id,
          domain: claim.domain,
          kind: "acceptance_gate",
          evidence: claim.evidenceGate,
          reason: "missing_acceptance_gate",
        });
      } else {
        const result = resultByLabel?.get(claim.evidenceGate);
        if (!result) {
          missing.push({
            id: claim.id,
            domain: claim.domain,
            kind: "acceptance_gate",
            evidence: claim.evidenceGate,
            reason: "missing_acceptance_result",
            status: "unknown",
          });
        } else {
          const status = normalizeStatus(result.status);
          if (!PASS_STATUSES.has(status)) {
            missing.push({
              id: claim.id,
              domain: claim.domain,
              kind: "acceptance_gate",
              evidence: claim.evidenceGate,
              reason: "failing_acceptance_gate",
              status,
            });
          } else {
            passedEvidence.push({
              kind: "acceptance_gate",
              evidence: claim.evidenceGate,
              status,
            });
          }
        }
      }
    }

    if (claim.evidenceScore) {
      evidenceReferences += 1;
      if (!scoreNames.has(claim.evidenceScore)) {
        missing.push({
          id: claim.id,
          domain: claim.domain,
          kind: "eval_score",
          evidence: claim.evidenceScore,
          reason: "missing_eval_score",
        });
      } else {
        const score = scoreValues.get(claim.evidenceScore);
        if (typeof score === "number" && score < 1) {
          missing.push({
            id: claim.id,
            domain: claim.domain,
            kind: "eval_score",
            evidence: claim.evidenceScore,
            reason: "failing_eval_score",
            score,
          });
        } else {
          passedEvidence.push({
            kind: "eval_score",
            evidence: claim.evidenceScore,
            score,
          });
        }
      }
    }

    if (evidenceReferences === 0) {
      missing.push({
        id: claim.id,
        domain: claim.domain,
        kind: "evidence_reference",
        evidence: null,
        reason: "missing_evidence_reference",
      });
    }
    if (evidenceReferences > 0 && missing.length === missingBeforeClaim) {
      passed.push({
        id: claim.id,
        domain: claim.domain,
        evidence: passedEvidence,
      });
    }
  }

  return { passed, missing };
}

function normalizeCapabilityClaim(rawClaim) {
  if (typeof rawClaim === "string") {
    return {
      id: rawClaim,
      domain: "unknown",
      evidenceGate: null,
      evidenceScore: null,
    };
  }
  return {
    id: String(rawClaim?.id ?? ""),
    domain: String(rawClaim?.domain ?? "unknown"),
    evidenceGate: stringValue(rawClaim?.evidenceGate ?? rawClaim?.acceptanceGate),
    evidenceScore: stringValue(rawClaim?.evidenceScore ?? rawClaim?.evalScore),
  };
}

function scoreNamesFromEvalReport(evalReport) {
  const report = evalReport?.report ?? evalReport;
  const names = new Set();
  if (!report || typeof report !== "object") return names;

  for (const name of Object.keys(report.score_summary ?? report.scoreSummary ?? {})) {
    names.add(name);
  }
  for (const task of taskMetricEntries(report)) {
    for (const name of Object.keys(task.scores ?? {})) {
      names.add(name);
    }
  }
  return names;
}

function scoreValuesFromEvalReport(evalReport) {
  const report = evalReport?.report ?? evalReport;
  const values = new Map();
  if (!report || typeof report !== "object") return values;

  for (const [name, rawScore] of Object.entries(report.score_summary ?? report.scoreSummary ?? {})) {
    const score = scoreValue(rawScore);
    if (typeof score === "number") {
      values.set(name, score);
    }
  }
  for (const task of taskMetricEntries(report)) {
    for (const [name, rawScore] of Object.entries(task.scores ?? {})) {
      const score = scoreValue(rawScore);
      if (typeof score !== "number") continue;
      values.set(name, Math.min(values.get(name) ?? score, score));
    }
  }
  return values;
}

function summarizeAffectedDomains({ acceptance, evalSummary, undeclaredBoundaries, capabilityEvidence }) {
  const domains = new Set();
  for (const gate of [
    ...acceptance.failedGates,
    ...acceptance.manualEvidenceGates,
    ...acceptance.unknownGates,
  ]) {
    domains.add(gate.domain);
  }
  for (const score of evalSummary.failingScores) {
    domains.add(score.domain);
  }
  for (const boundary of undeclaredBoundaries) {
    domains.add(boundary.domain);
  }
  for (const item of asArray(capabilityEvidence?.missing)) {
    domains.add(item.domain);
  }
  return [...domains].filter(Boolean).sort();
}

function summarizeStatus({ acceptance, evalSummary, undeclaredBoundaries, capabilityEvidence }) {
  if (acceptance.failedGates.length > 0) {
    return "failed";
  }
  if (
    acceptance.manualEvidenceGates.length > 0 ||
    acceptance.unknownGates.length > 0 ||
    evalSummary.failingScores.length > 0 ||
    undeclaredBoundaries.length > 0 ||
    asArray(capabilityEvidence?.missing).length > 0 ||
    !evalSummary.available
  ) {
    return "attention_required";
  }
  return "passed";
}

function normalizeStatus(status) {
  return String(status ?? "unknown")
    .trim()
    .toLowerCase()
    .replace(/[\s-]+/g, "_");
}

function domainForScore(name) {
  if (name.includes("gateway")) return "gateway";
  if (name.includes("memory")) return "memory";
  if (name.includes("a2a") || name.includes("recovery") || name.includes("completion")) {
    return "runtime";
  }
  if (name.includes("permission") || name.includes("confirmation")) return "permission";
  if (name.includes("usage") || name.includes("context")) return "usage-context";
  return "eval";
}

function capabilityEvidenceMessage(item) {
  if (item.reason === "missing_acceptance_gate") {
    return `missing acceptance gate "${item.evidence}"`;
  }
  if (item.reason === "missing_acceptance_result") {
    return `missing acceptance result "${item.evidence}"`;
  }
  if (item.reason === "failing_acceptance_gate") {
    return `failing acceptance gate "${item.evidence}" (status: ${item.status})`;
  }
  if (item.reason === "missing_eval_score") {
    return `missing eval score "${item.evidence}"`;
  }
  if (item.reason === "failing_eval_score") {
    return `failing eval score "${item.evidence}" (score: ${item.score})`;
  }
  return "missing evidence reference";
}

function capabilityEvidencePassedMessage(item) {
  return asArray(item.evidence)
    .map((evidence) => {
      if (evidence.kind === "acceptance_gate") {
        return `acceptance gate "${evidence.evidence}"`;
      }
      if (evidence.kind === "eval_score") {
        return `eval score "${evidence.evidence}" (score: ${evidence.score})`;
      }
      return String(evidence.evidence ?? evidence.kind ?? "evidence");
    })
    .join("; ");
}

function stringValue(value) {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return trimmed.length ? trimmed : null;
}

function appendGateList(lines, title, gates) {
  if (gates.length === 0) return;
  lines.push("", `### ${title}`, "");
  for (const gate of gates) {
    const reason = gate.reason ? ` - ${gate.reason}` : "";
    lines.push(
      `- ${gate.label} (domain: ${gate.domain}; status: ${gate.status}; tier: ${gate.tier}; cost: ${gate.runtimeCost}; manual: ${gate.manualRequirement})${reason}`,
    );
  }
}

function formatPercent(value) {
  if (typeof value !== "number") return "unknown";
  return `${(value * 100).toFixed(1)}%`;
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function readJsonFile(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function loadAcceptanceMatrix(path) {
  if (path) return readJsonFile(path);
  const output = execFileSync(join(root, "scripts", "acceptance.sh"), ["--list-json"], {
    cwd: root,
    encoding: "utf8",
  });
  return JSON.parse(output);
}

function parseArgs(argv) {
  const options = {
    format: "markdown",
    acceptanceJson: null,
    noAcceptanceMatrix: false,
    gateResults: null,
    evalReport: null,
    boundariesJson: null,
    outJson: null,
    outMarkdown: null,
    ciDefaultOnly: false,
    failOnAttention: false,
    help: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") {
      options.format = "json";
    } else if (arg === "--markdown") {
      options.format = "markdown";
    } else if (arg === "--acceptance-json") {
      options.acceptanceJson = requireValue(argv, (index += 1), arg);
    } else if (arg === "--no-acceptance-matrix") {
      options.noAcceptanceMatrix = true;
    } else if (arg === "--gate-results") {
      options.gateResults = requireValue(argv, (index += 1), arg);
    } else if (arg === "--eval-report") {
      options.evalReport = requireValue(argv, (index += 1), arg);
    } else if (arg === "--boundaries-json") {
      options.boundariesJson = requireValue(argv, (index += 1), arg);
    } else if (arg === "--out-json") {
      options.outJson = requireValue(argv, (index += 1), arg);
    } else if (arg === "--out-md") {
      options.outMarkdown = requireValue(argv, (index += 1), arg);
    } else if (arg === "--ci-default-only") {
      options.ciDefaultOnly = true;
    } else if (arg === "--fail-on-attention") {
      options.failOnAttention = true;
    } else if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return options;
}

function requireValue(argv, index, flag) {
  const value = argv[index];
  if (!value) throw new Error(`${flag} requires a path`);
  return value;
}

function printUsage() {
  console.log(`Usage: node scripts/release-confidence-summary.mjs [--json|--markdown] [--acceptance-json PATH|--no-acceptance-matrix] [--gate-results PATH] [--eval-report PATH] [--boundaries-json PATH] [--out-json PATH] [--out-md PATH] [--ci-default-only] [--fail-on-attention]

Builds a release confidence summary from acceptance matrix metadata plus optional gate-result, eval-report, and boundary evidence.

Options:
  --json                  Print JSON summary.
  --markdown              Print Markdown summary (default).
  --acceptance-json PATH  Acceptance matrix JSON; defaults to scripts/acceptance.sh --list-json.
  --no-acceptance-matrix  Derive gates only from self-describing --gate-results metadata.
  --gate-results PATH     JSON with gates: [{ label, status, reason?, domain?, tier?, ciDefault? }].
  --eval-report PATH      Eval-runner BacktestReport JSON or { report } wrapper.
  --boundaries-json PATH  JSON with declared, required, and capabilityClaims boundary lists.
  --out-json PATH         Also write JSON summary to a file.
  --out-md PATH           Also write Markdown summary to a file.
  --ci-default-only       Summarize only gates marked ciDefault in the acceptance matrix.
  --fail-on-attention     Exit 1 when the summary status is failed or attention_required.
`);
}

function main(argv) {
  const options = parseArgs(argv);
  if (options.help) {
    printUsage();
    return 0;
  }

  const gateResults = options.gateResults && existsSync(options.gateResults) ? readJsonFile(options.gateResults) : null;
  const summary = buildReleaseConfidenceSummary({
    acceptanceMatrix: options.noAcceptanceMatrix ? null : loadAcceptanceMatrix(options.acceptanceJson),
    gateResults,
    evalReport: options.evalReport && existsSync(options.evalReport) ? readJsonFile(options.evalReport) : null,
    boundaries:
      options.boundariesJson && existsSync(options.boundariesJson)
        ? readJsonFile(options.boundariesJson)
        : null,
    acceptanceScope: options.ciDefaultOnly ? "ci-default" : "all",
  });
  const json = `${JSON.stringify(summary, null, 2)}\n`;
  const markdown = renderReleaseConfidenceMarkdown(summary);

  if (options.outJson) writeFileSync(options.outJson, json, "utf8");
  if (options.outMarkdown) writeFileSync(options.outMarkdown, markdown, "utf8");

  process.stdout.write(options.format === "json" ? json : markdown);
  if (options.failOnAttention && summary.status !== "passed") {
    console.error(`Release confidence status: ${summary.status}`);
    return 1;
  }
  return 0;
}

if (process.argv[1] === __filename) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 2;
  }
}
