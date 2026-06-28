#!/usr/bin/env node
import { readFileSync } from "node:fs";
import process from "node:process";
import { pathToFileURL } from "node:url";

import { archiveDisposableLoopEvidence } from "./archive-disposable-loop-evidence.mjs";
import { reviewDisposableLoopManualJson } from "./review-disposable-loop-manual-json.mjs";

const DEFAULT_PROJECT_PATH = "/Users/cabbos/project/forge-test-app-phase8-clean";
const DEFAULT_OUT_DIR = "apps/desktop/docs/product/evidence/phase8-disposable-loop";

export function finalizeDisposableLoopRow({
  projectPath = DEFAULT_PROJECT_PATH,
  row = "1",
  manualValues = null,
  runBuild = false,
  includeDiff = false,
  date = currentDate(),
  outDir = DEFAULT_OUT_DIR,
  dryRun = false,
  requireComplete = false,
} = {}) {
  const manualReview = reviewDisposableLoopManualJson({
    row,
    manualValues,
    requireComplete,
  });
  const result = {
    status: "pending",
    dryRun,
    requireComplete,
    row: String(row),
    date,
    manualReview,
    archive: null,
    nextStep: "",
  };

  if (!manualReview.pass) {
    result.status = requireComplete ? "manual_review_failed" : "pending_manual_evidence";
    result.nextStep = "Fill the manual JSON fields, rerun manual evidence review, then finalize again.";
    return result;
  }

  const archive = archiveDisposableLoopEvidence({
    projectPath,
    row,
    runBuild,
    includeDiff,
    manualValues,
    date,
    outDir,
    dryRun,
    requireComplete: true,
  });
  result.archive = archive;
  result.status = archive.status;
  result.nextStep = nextStepForArchive(archive);
  return result;
}

function nextStepForArchive(archive) {
  if (archive.status === "archived") {
    return "Strict row evidence is archived. Run the status helper to choose the next row.";
  }
  if (archive.status === "dry_run_ready") {
    return "Dry-run passed. Rerun without --dry-run to archive strict row evidence.";
  }
  return "Resolve validation issues before archiving strict row evidence.";
}

function printHelp() {
  console.log(`Usage: node scripts/finalize-disposable-loop-row.mjs [--json] [--dry-run] [--project <path>] [--row <1|2|3>] [--manual-json <path>] [--run-build] [--include-diff] [--require-complete] [--out-dir <path>] [--date YYYY-MM-DD]

Reviews manual evidence, runs strict validation, and archives a Phase 8 disposable loop row when complete.

Options:
  --json                Print machine-readable finalization status.
  --dry-run             Review and validate without writing archive files.
  --project PATH        Project path to inspect. Defaults to ${DEFAULT_PROJECT_PATH}
  --row VALUE           Row scope: 1, 2, or 3. Defaults to 1.
  --manual-json PATH    JSON object keyed by manual evidence field label.
  --run-build           Run npm --prefix <project> run build while collecting.
  --include-diff        Include full diff text in the evidence JSON.
  --require-complete    Exit non-zero unless manual review and strict validation pass.
  --out-dir PATH        Archive directory. Defaults to ${DEFAULT_OUT_DIR}
  --date YYYY-MM-DD     Evidence date. Defaults to today.
  -h, --help            Show this help.
`);
}

function parseArgs(argv) {
  const options = {
    json: false,
    dryRun: false,
    projectPath: DEFAULT_PROJECT_PATH,
    row: "1",
    manualJson: null,
    runBuild: false,
    includeDiff: false,
    requireComplete: false,
    outDir: DEFAULT_OUT_DIR,
    date: currentDate(),
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") {
      options.json = true;
    } else if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "--run-build") {
      options.runBuild = true;
    } else if (arg === "--include-diff") {
      options.includeDiff = true;
    } else if (arg === "--require-complete") {
      options.requireComplete = true;
    } else if (arg === "--project") {
      const value = argv[index + 1];
      if (!value) throw new Error("--project requires a path");
      options.projectPath = value;
      index += 1;
    } else if (arg === "--row") {
      const value = argv[index + 1];
      if (!["1", "2", "3"].includes(value)) throw new Error("--row must be one of: 1, 2, 3");
      options.row = value;
      index += 1;
    } else if (arg === "--manual-json") {
      const value = argv[index + 1];
      if (!value) throw new Error("--manual-json requires a path");
      options.manualJson = value;
      index += 1;
    } else if (arg === "--out-dir") {
      const value = argv[index + 1];
      if (!value) throw new Error("--out-dir requires a path");
      options.outDir = value;
      index += 1;
    } else if (arg === "--date") {
      const value = argv[index + 1];
      if (!value) throw new Error("--date requires a value");
      options.date = value;
      index += 1;
    } else if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function readManualValues(path) {
  if (!path) return null;
  const parsed = JSON.parse(readFileSync(path, "utf8"));
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("--manual-json must contain a JSON object");
  }
  return parsed;
}

function printHuman(result) {
  console.log("Disposable loop row finalization");
  console.log(`Status: ${result.status}`);
  console.log(`Manual review: ${result.manualReview.status} (${result.manualReview.pass ? "pass" : "fail"})`);
  if (result.archive) {
    console.log(`Archive: ${result.archive.status}`);
    console.log(`Validation: ${result.archive.validationStatus} (${result.archive.validationPass ? "pass" : "fail"})`);
  }
  console.log(result.nextStep);
}

function currentDate() {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function main(argv = process.argv.slice(2)) {
  let options;
  try {
    options = parseArgs(argv);
    options.manualValues = readManualValues(options.manualJson);
  } catch (error) {
    console.error(error.message);
    return 2;
  }

  if (options.help) {
    printHelp();
    return 0;
  }

  const result = finalizeDisposableLoopRow(options);
  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    printHuman(result);
  }
  return options.requireComplete && result.status !== "archived" && result.status !== "dry_run_ready" ? 1 : 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
