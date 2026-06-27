#!/usr/bin/env node
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

import { collectDisposableLoopEvidence } from "./collect-disposable-loop-evidence.mjs";
import { validateDisposableLoopEvidence } from "./validate-disposable-loop-evidence.mjs";

const DEFAULT_PROJECT_PATH = "/Users/cabbos/project/forge-test-app-phase8-clean";
const DEFAULT_OUT_DIR = "apps/desktop/docs/product/evidence/phase8-disposable-loop";

export function archiveDisposableLoopEvidence({
  projectPath = DEFAULT_PROJECT_PATH,
  row = "all",
  runBuild = false,
  includeDiff = false,
  manualValues = {},
  date = currentDate(),
  outDir = DEFAULT_OUT_DIR,
  dryRun = false,
  requireComplete = false,
} = {}) {
  const evidence = collectDisposableLoopEvidence({
    projectPath,
    row,
    runBuild,
    includeDiff,
    manualValues,
    date,
  });
  const validation = validateDisposableLoopEvidence(evidence, { row, requireComplete });
  const paths = archivePaths({ outDir, date, row });
  const result = {
    status: "pending",
    dryRun,
    requireComplete,
    evidenceStatus: evidence.status,
    validationStatus: validation.status,
    validationPass: validation.pass,
    row,
    date,
    paths,
    validation,
  };

  if (requireComplete && !validation.pass) {
    result.status = "validation_failed";
    return result;
  }

  if (dryRun) {
    result.status = validation.pass ? "dry_run_ready" : "dry_run_pending";
    return result;
  }

  mkdirSync(resolve(outDir), { recursive: true });
  writeFileSync(paths.evidenceJson, `${JSON.stringify(evidence, null, 2)}\n`);
  writeFileSync(paths.markdown, evidence.markdown);
  writeFileSync(paths.validationJson, `${JSON.stringify(validation, null, 2)}\n`);
  result.status = validation.pass ? "archived" : "archived_pending";
  return result;
}

function archivePaths({ outDir, date, row }) {
  const basename = `${date}-row-${row}`;
  const resolvedOutDir = resolve(outDir);
  return {
    outDir: resolvedOutDir,
    evidenceJson: join(resolvedOutDir, `${basename}.evidence.json`),
    markdown: join(resolvedOutDir, `${basename}.md`),
    validationJson: join(resolvedOutDir, `${basename}.validation.json`),
  };
}

function currentDate() {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function printHelp() {
  console.log(`Usage: node scripts/archive-disposable-loop-evidence.mjs [--json] [--dry-run] [--project <path>] [--row <all|1|2|3>] [--manual-json <path>] [--run-build] [--include-diff] [--require-complete] [--out-dir <path>] [--date YYYY-MM-DD]

Archives Phase 8 disposable loop evidence as JSON, markdown, and validation JSON.

Options:
  --json                Print machine-readable archive status.
  --dry-run             Report paths and validation without writing files.
  --project PATH        Project path to inspect. Defaults to ${DEFAULT_PROJECT_PATH}
  --row VALUE           Row scope: all, 1, 2, or 3. Defaults to all.
  --manual-json PATH    JSON object keyed by manual evidence field label.
  --run-build           Run npm --prefix <project> run build while collecting.
  --include-diff        Include full diff text in the evidence JSON.
  --require-complete    Do not archive unless validation passes strictly.
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
    row: "all",
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
      if (!["all", "1", "2", "3"].includes(value)) throw new Error("--row must be one of: all, 1, 2, 3");
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
  if (!path) return {};
  const parsed = JSON.parse(readFileSync(path, "utf8"));
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("--manual-json must contain a JSON object");
  }
  return parsed;
}

function printHuman(result) {
  console.log("Disposable loop evidence archive");
  console.log(`Status: ${result.status}`);
  console.log(`Validation: ${result.validationStatus} (${result.validationPass ? "pass" : "fail"})`);
  console.log(`Markdown: ${result.paths.markdown}`);
  console.log(`Evidence JSON: ${result.paths.evidenceJson}`);
  console.log(`Validation JSON: ${result.paths.validationJson}`);
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

  const result = archiveDisposableLoopEvidence(options);
  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    printHuman(result);
  }
  return result.status === "validation_failed" ? 1 : 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
