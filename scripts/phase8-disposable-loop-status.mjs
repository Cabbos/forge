#!/usr/bin/env node
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

import {
  collectScreenSnapshotSafe,
  evaluateDesktopUiEvidencePreflight,
} from "./desktop-ui-evidence-preflight.mjs";
import { evaluatePhase8LiveReadyGate } from "./phase8-live-ready-gate.mjs";
import { generatePhase8DisposableLoopRunbook } from "./phase8-disposable-loop-runbook.mjs";

const DEFAULT_PROJECT_PATH = "/Users/cabbos/project/forge-test-app-phase8-clean";
const DEFAULT_OUT_DIR = "apps/desktop/docs/product/evidence/phase8-disposable-loop";
const DEFAULT_MANUAL_DIR = "/tmp";
const ROWS = ["1", "2", "3"];

export function generatePhase8DisposableLoopStatus({
  projectPath = DEFAULT_PROJECT_PATH,
  outDir = DEFAULT_OUT_DIR,
  manualDir = DEFAULT_MANUAL_DIR,
  date = currentDate(),
  uiEvidencePreflight = uncheckedDesktopUiEvidencePreflight(),
} = {}) {
  const resolvedProjectPath = resolve(projectPath);
  const resolvedOutDir = resolve(outDir);
  const rowStatuses = ROWS.map((row) => {
    const archive = latestArchiveForRow({ outDir: resolvedOutDir, row });
    if (archive.complete) {
      return {
        row,
        status: "archived_complete",
        complete: true,
        readyForLiveRun: false,
        archive,
        runbook: null,
      };
    }

    const runbook = generatePhase8DisposableLoopRunbook({
      projectPath: resolvedProjectPath,
      row,
      manualDir,
      date,
      uiEvidencePreflight,
    });
    return {
      row,
      status: runbook.status,
      complete: false,
      readyForLiveRun: runbook.readyForLiveRun,
      archive,
      runbook: summarizeRunbook(runbook),
    };
  });

  const nextRowStatus = rowStatuses.find((entry) => !entry.complete) ?? null;
  const status = statusForRows(rowStatuses, nextRowStatus);
  const result = {
    status,
    projectPath: resolvedProjectPath,
    outDir: resolvedOutDir,
    date,
    uiEvidencePreflight,
    nextRow: nextRowStatus?.row ?? null,
    readyForLiveRun: Boolean(nextRowStatus?.readyForLiveRun),
    rows: rowStatuses,
    nextCommands: nextRowStatus?.runbook?.commands ?? [],
    recoveryCommands: nextRowStatus?.runbook?.recoveryCommands ?? uiEvidencePreflight.recoveryCommands ?? [],
    nextStep: nextStep(status, nextRowStatus),
    markdown: "",
  };
  result.liveReadyGate = evaluatePhase8LiveReadyGate(result);
  result.markdown = renderMarkdown(result);
  return result;
}

export function phase8DisposableLoopStatusExitCode(result, { requireLiveReady = false } = {}) {
  if (result.status === "project_not_ready") return 1;
  if (!requireLiveReady) return 0;
  const gate = result.liveReadyGate ?? evaluatePhase8LiveReadyGate(result);
  return gate.pass ? 0 : 1;
}

function latestArchiveForRow({ outDir, row }) {
  const fallback = {
    complete: false,
    validationComplete: false,
    validationPath: null,
    evidencePath: null,
    markdownPath: null,
    validationStatus: null,
    validationPass: null,
    missingFiles: [],
    basename: null,
  };
  if (!existsSync(outDir)) return fallback;

  const archives = readdirSync(outDir)
    .filter((file) => file.endsWith(`-row-${row}.validation.json`))
    .map((file) => readValidationArchive({ outDir, file }))
    .filter(Boolean)
    .sort((a, b) => a.basename.localeCompare(b.basename));

  return archives.findLast((entry) => entry.complete) ?? archives.at(-1) ?? fallback;
}

function readValidationArchive({ outDir, file }) {
  const validationPath = join(outDir, file);
  try {
    const validation = JSON.parse(readFileSync(validationPath, "utf8"));
    const base = basename(file, ".validation.json");
    const evidencePath = join(outDir, `${base}.evidence.json`);
    const markdownPath = join(outDir, `${base}.md`);
    const missingFiles = [evidencePath, markdownPath].filter((path) => !existsSync(path));
    const validationComplete = validation?.pass === true && validation?.status === "complete";
    return {
      complete: validationComplete && missingFiles.length === 0,
      validationComplete,
      validationPath,
      evidencePath,
      markdownPath,
      validationStatus: validation?.status ?? null,
      validationPass: validation?.pass ?? null,
      missingFiles,
      basename: base,
    };
  } catch {
    return null;
  }
}

function summarizeRunbook(runbook) {
  return {
    status: runbook.status,
    uiEvidencePreflight: runbook.uiEvidencePreflight,
    manualPath: runbook.manualPath,
    prompt: runbook.prompt,
    archivePaths: runbook.archiveDryRun.paths,
    commands: runbook.commands,
    recoveryCommands: runbook.recoveryCommands,
    nextStep: runbook.nextStep,
  };
}

function statusForRows(rows, nextRowStatus) {
  if (rows.every((entry) => entry.complete)) return "complete";
  if (nextRowStatus?.status === "ui_evidence_not_ready") return "ui_evidence_not_ready";
  if (!nextRowStatus?.readyForLiveRun) return "project_not_ready";
  return "ready_for_live_row";
}

function nextStep(status, nextRowStatus) {
  if (status === "complete") {
    return "All Phase 8 disposable loop rows have complete archived evidence.";
  }
  if (status === "ui_evidence_not_ready") {
    return `Resolve desktop UI evidence preflight for row #${nextRowStatus?.row ?? "?"}, or run the row manually in a trusted desktop session before finalizing evidence.`;
  }
  if (!nextRowStatus?.readyForLiveRun) {
    return `Resolve row #${nextRowStatus?.row ?? "?"} readiness issues before running the live Forge row.`;
  }
  return `Run row #${nextRowStatus.row} with the listed runbook commands, then archive strict evidence.`;
}

function renderMarkdown(result) {
  const rows = result.rows.map((entry) => {
    const archive = entry.archive.complete
      ? `archived at \`${entry.archive.validationPath}\``
      : entry.archive.validationComplete && entry.archive.missingFiles?.length > 0
        ? `latest validation complete but missing archive sidecars: ${entry.archive.missingFiles.map((file) => `\`${file}\``).join(", ")}`
      : entry.archive.validationPath
        ? `latest validation ${entry.archive.validationStatus ?? "unknown"} at \`${entry.archive.validationPath}\``
        : "no archive yet";
    return `- Row #${entry.row}: ${entry.status} (${archive})`;
  }).join("\n");
  const commands = result.nextCommands.length > 0
    ? result.nextCommands.map((entry, index) => `${index + 1}. ${entry.label}\n\n   \`${entry.command}\``).join("\n\n")
    : "(none)";
  const recoveryCommands = renderRecoveryCommands(result.recoveryCommands);

  return `## Phase 8 Disposable Loop Status

Status: ${result.status}
Project: \`${result.projectPath}\`
Evidence dir: \`${result.outDir}\`
UI evidence preflight: ${result.uiEvidencePreflight.status}
Live-ready gate: ${result.liveReadyGate.pass ? "pass" : "blocked"} (${result.liveReadyGate.reason})
Next row: ${result.nextRow ? `#${result.nextRow}` : "(none)"}

Rows:
${rows}

Next commands:

${commands}
${recoveryCommands}

Next step: ${result.nextStep}
`;
}

function renderRecoveryCommands(recoveryCommands = []) {
  if (recoveryCommands.length === 0) return "";
  const commands = recoveryCommands.map((entry) => `- ${entry.label}: \`${entry.command}\``).join("\n");
  return `
Recovery commands:

${commands}
`;
}

function printHelp() {
  console.log(`Usage: node scripts/phase8-disposable-loop-status.mjs [--json|--markdown] [--project <path>] [--out-dir <path>] [--manual-dir <path>] [--date YYYY-MM-DD] [--skip-ui-preflight] [--require-live-ready]

Reports Phase 8 disposable loop row archive coverage and the next live row runbook commands.

Options:
  --json             Print machine-readable status.
  --markdown         Print markdown status.
  --project PATH     Prepared disposable project. Defaults to ${DEFAULT_PROJECT_PATH}
  --out-dir PATH     Evidence archive directory. Defaults to ${DEFAULT_OUT_DIR}
  --manual-dir PATH  Directory for generated manual JSON templates. Defaults to ${DEFAULT_MANUAL_DIR}
  --date YYYY-MM-DD  Evidence date for next-row runbooks. Defaults to today.
  --skip-ui-preflight
                     Do not run local desktop UI evidence preflight before printing.
  --require-live-ready
                     Exit nonzero unless the next row is verified ready for live Forge evidence or all rows are complete.
  -h, --help         Show this help.
`);
}

function parseArgs(argv) {
  const options = {
    json: false,
    markdown: false,
    projectPath: DEFAULT_PROJECT_PATH,
    outDir: DEFAULT_OUT_DIR,
    manualDir: DEFAULT_MANUAL_DIR,
    date: currentDate(),
    skipUiPreflight: false,
    requireLiveReady: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") {
      options.json = true;
    } else if (arg === "--markdown") {
      options.markdown = true;
    } else if (arg === "--project") {
      const value = argv[index + 1];
      if (!value) throw new Error("--project requires a path");
      options.projectPath = value;
      index += 1;
    } else if (arg === "--out-dir") {
      const value = argv[index + 1];
      if (!value) throw new Error("--out-dir requires a path");
      options.outDir = value;
      index += 1;
    } else if (arg === "--manual-dir") {
      const value = argv[index + 1];
      if (!value) throw new Error("--manual-dir requires a path");
      options.manualDir = value;
      index += 1;
    } else if (arg === "--date") {
      const value = argv[index + 1];
      if (!value) throw new Error("--date requires a value");
      options.date = value;
      index += 1;
    } else if (arg === "--skip-ui-preflight") {
      options.skipUiPreflight = true;
    } else if (arg === "--require-live-ready") {
      options.requireLiveReady = true;
    } else if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function uncheckedDesktopUiEvidencePreflight() {
  return {
    status: "not_checked",
    canCollectLiveUiEvidence: null,
    platform: process.platform,
    reason: "Desktop UI evidence preflight was not run by this pure generator call.",
    windowSnapshot: null,
    screenSnapshot: null,
    recoveryCommands: [],
    recommendations: ["Run `node scripts/desktop-ui-evidence-preflight.mjs --json --require-ready` before collecting live UI evidence."],
  };
}

function collectDesktopUiEvidencePreflight() {
  return evaluateDesktopUiEvidencePreflight({
    screenSnapshot: process.platform === "darwin" ? collectScreenSnapshotSafe() : null,
  });
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
  } catch (error) {
    console.error(error.message);
    return 2;
  }

  if (options.help) {
    printHelp();
    return 0;
  }

  const result = generatePhase8DisposableLoopStatus({
    ...options,
    uiEvidencePreflight: options.skipUiPreflight
      ? uncheckedDesktopUiEvidencePreflight()
      : collectDesktopUiEvidencePreflight(),
  });
  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    console.log(result.markdown);
  }
  return phase8DisposableLoopStatusExitCode(result, {
    requireLiveReady: options.requireLiveReady,
  });
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
