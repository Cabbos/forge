#!/usr/bin/env node
import { readFileSync } from "node:fs";
import process from "node:process";
import { pathToFileURL } from "node:url";

import { collectDisposableLoopEvidence } from "./collect-disposable-loop-evidence.mjs";

const REQUIRED_MANUAL_FIELDS = new Set([
  "Forge prompt",
  "Forge final answer",
  "Confirmation behavior",
  "Screenshot or transcript reference",
]);

export function validateDisposableLoopEvidence(evidence, { row = evidence?.row ?? "all", requireComplete = false } = {}) {
  const normalizedRow = String(row);
  const issues = [];

  if (!evidence || typeof evidence !== "object") {
    return {
      status: "invalid_evidence",
      pass: false,
      row: normalizedRow,
      requireComplete,
      issues: [{ code: "invalid_evidence", message: "Evidence must be a JSON object." }],
      summary: {},
    };
  }

  if (!evidence.preflight?.readyForLoop && !isExpectedPostRunDirtyEvidence(evidence)) {
    issues.push({ code: "project_not_ready", message: "Preflight does not show readyForLoop: true." });
  }

  if (normalizedRow === "1") validateRow1(evidence, issues);
  if (normalizedRow === "2") validateRow2(evidence, issues);
  if (normalizedRow === "3") validateRow3(evidence, issues);
  if (normalizedRow === "all") validateAggregate(evidence, issues);

  validateManualFields(evidence, issues);

  const blockingIssues = requireComplete ? issues : issues.filter((issue) => issue.alwaysBlocking);
  const pass = blockingIssues.length === 0;
  const status = statusForValidation({ evidence, issues, pass, requireComplete });

  return {
    status,
    pass,
    row: normalizedRow,
    requireComplete,
    issues,
    blockingIssues,
    summary: {
      changedFiles: changedFiles(evidence).map((entry) => entry.file),
      buildRan: Boolean(evidence.build?.ran),
      buildSuccess: evidence.build?.success ?? null,
      manualFieldsMissing: issues.filter((issue) => issue.code === "manual_field_missing").map((issue) => issue.field),
    },
  };
}

function validateRow1(evidence, issues) {
  const files = changedFiles(evidence);
  if (files.length === 0) {
    issues.push({ code: "row1_no_changes", message: "Row #1 requires a visible feedback fix with at least one changed file." });
  }
  if (files.some((entry) => !entry.file.startsWith("src/"))) {
    issues.push({ code: "row1_unexpected_file", message: "Row #1 changes should stay inside src/ for the demo fix." });
  }
  requireSuccessfulBuild(evidence, issues, "row1_build_missing");
}

function validateRow2(evidence, issues) {
  const files = changedFiles(evidence);
  if (files.length === 0) {
    issues.push({ code: "row2_no_changes", message: "Row #2 requires at least one style-file change." });
  }
  const nonStyleFiles = files.filter((entry) => !isStyleFile(entry.file));
  if (nonStyleFiles.length > 0) {
    issues.push({
      code: "row2_non_style_file",
      message: "Row #2 must only change style files.",
      files: nonStyleFiles.map((entry) => entry.file),
    });
  }
}

function validateRow3(evidence, issues) {
  const files = changedFiles(evidence);
  if (files.length > 0) {
    issues.push({
      code: "row3_changed_files",
      message: "Row #3 is command-only and should leave no file diff.",
      files: files.map((entry) => entry.file),
    });
  }
  requireSuccessfulBuild(evidence, issues, "row3_build_missing");
}

function validateAggregate(evidence, issues) {
  if (evidence.status === "no_changes_yet") {
    issues.push({
      code: "live_rows_not_run",
      message: "The prepared project has no changes yet; live Forge rows #1-#3 are still pending.",
    });
  }
}

function validateManualFields(evidence, issues) {
  const fields = Array.isArray(evidence.manualFields) ? evidence.manualFields : [];
  for (const label of REQUIRED_MANUAL_FIELDS) {
    const field = fields.find((entry) => entry?.label === label);
    if (!field || !String(field.value ?? "").trim()) {
      issues.push({
        code: "manual_field_missing",
        field: label,
        message: `Manual evidence field is missing: ${label}.`,
      });
    }
  }
}

function requireSuccessfulBuild(evidence, issues, missingCode) {
  if (!evidence.build?.ran) {
    issues.push({ code: missingCode, message: "Build/check output has not been captured." });
    return;
  }
  if (!evidence.build.success) {
    issues.push({
      code: "build_failed",
      alwaysBlocking: true,
      message: "Build/check command was captured but did not pass.",
    });
  }
}

function statusForValidation({ evidence, issues, pass, requireComplete }) {
  if (pass && issues.length === 0) return "complete";
  if (issues.some((issue) => issue.code === "build_failed")) return "build_failed";
  if (pass && !requireComplete) return "pending_live_evidence";
  if (evidence?.status === "no_changes_yet") return "pending_live_evidence";
  return pass ? "pending_manual_fields" : "incomplete";
}

function changedFiles(evidence) {
  return Array.isArray(evidence.git?.changedFiles) ? evidence.git.changedFiles : [];
}

function isExpectedPostRunDirtyEvidence(evidence) {
  const files = changedFiles(evidence);
  const issues = Array.isArray(evidence.preflight?.issues) ? evidence.preflight.issues : [];
  return files.length > 0 && issues.length > 0 && issues.every((issue) => issue?.code === "dirty_worktree");
}

function isStyleFile(file) {
  return /\.(css|scss|sass|less|pcss)$/i.test(file);
}

function printHelp() {
  console.log(`Usage: node scripts/validate-disposable-loop-evidence.mjs [--json] [--evidence-json <path>] [--project <path>] [--manual-json <path>] [--row <all|1|2|3>] [--run-build] [--require-complete]

Validates Phase 8 disposable loop evidence from collector JSON or the current project state.

Options:
  --json                Print machine-readable validation.
  --evidence-json PATH  Read evidence JSON produced by collect-disposable-loop-evidence.mjs.
  --project PATH        Project path to collect when --evidence-json is omitted.
  --manual-json PATH    JSON object keyed by manual evidence field label.
  --row VALUE           Row scope: all, 1, 2, or 3. Defaults to evidence row or all.
  --run-build           Run build/check while collecting current project evidence.
  --require-complete    Exit non-zero unless all required evidence is complete.
  -h, --help            Show this help.
`);
}

function parseArgs(argv) {
  const options = {
    json: false,
    evidenceJson: null,
    manualJson: null,
    projectPath: undefined,
    row: undefined,
    runBuild: false,
    requireComplete: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") {
      options.json = true;
    } else if (arg === "--run-build") {
      options.runBuild = true;
    } else if (arg === "--require-complete") {
      options.requireComplete = true;
    } else if (arg === "--evidence-json") {
      const value = argv[index + 1];
      if (!value) throw new Error("--evidence-json requires a path");
      options.evidenceJson = value;
      index += 1;
    } else if (arg === "--manual-json") {
      const value = argv[index + 1];
      if (!value) throw new Error("--manual-json requires a path");
      options.manualJson = value;
      index += 1;
    } else if (arg === "--project") {
      const value = argv[index + 1];
      if (!value) throw new Error("--project requires a path");
      options.projectPath = value;
      index += 1;
    } else if (arg === "--row") {
      const value = argv[index + 1];
      if (!["all", "1", "2", "3"].includes(value)) throw new Error("--row must be one of: all, 1, 2, 3");
      options.row = value;
      index += 1;
    } else if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function readEvidence(options) {
  if (options.evidenceJson) {
    return JSON.parse(readFileSync(options.evidenceJson, "utf8"));
  }
  return collectDisposableLoopEvidence({
    projectPath: options.projectPath,
    row: options.row ?? "all",
    runBuild: options.runBuild,
    manualValues: readManualValues(options.manualJson),
  });
}

function readManualValues(path) {
  if (!path) return {};
  const parsed = JSON.parse(readFileSync(path, "utf8"));
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("--manual-json must contain a JSON object");
  }
  return parsed;
}

function printHuman(result) {
  console.log("Disposable loop evidence validation");
  console.log(`Status: ${result.status}`);
  console.log(`Row: ${result.row}`);
  console.log(`Pass: ${result.pass ? "yes" : "no"}`);
  if (result.issues.length > 0) {
    console.log("Issues:");
    for (const issue of result.issues) {
      console.log(`- ${issue.code}: ${issue.message}`);
    }
  }
}

function main(argv = process.argv.slice(2)) {
  let options;
  try {
    options = parseArgs(argv);
  } catch (error) {
    console.error(error.message);
    return 2;
  }

  if (options.help) {
    printHelp();
    return 0;
  }

  const evidence = readEvidence(options);
  const result = validateDisposableLoopEvidence(evidence, {
    row: options.row ?? evidence.row ?? "all",
    requireComplete: options.requireComplete,
  });

  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    printHuman(result);
  }

  return result.pass ? 0 : 1;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
