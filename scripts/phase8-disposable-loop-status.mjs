#!/usr/bin/env node
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

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
    });
    return {
      row,
      status: runbook.readyForLiveRun ? runbook.status : "project_not_ready",
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
    nextRow: nextRowStatus?.row ?? null,
    readyForLiveRun: Boolean(nextRowStatus?.readyForLiveRun),
    rows: rowStatuses,
    nextCommands: nextRowStatus?.runbook?.commands ?? [],
    nextStep: nextStep(status, nextRowStatus),
    markdown: "",
  };
  result.markdown = renderMarkdown(result);
  return result;
}

function latestArchiveForRow({ outDir, row }) {
  const fallback = {
    complete: false,
    validationPath: null,
    evidencePath: null,
    markdownPath: null,
    validationStatus: null,
    validationPass: null,
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
    return {
      complete: validation?.pass === true && validation?.status === "complete",
      validationPath,
      evidencePath: join(outDir, `${base}.evidence.json`),
      markdownPath: join(outDir, `${base}.md`),
      validationStatus: validation?.status ?? null,
      validationPass: validation?.pass ?? null,
      basename: base,
    };
  } catch {
    return null;
  }
}

function summarizeRunbook(runbook) {
  return {
    status: runbook.status,
    manualPath: runbook.manualPath,
    prompt: runbook.prompt,
    archivePaths: runbook.archiveDryRun.paths,
    commands: runbook.commands,
    nextStep: runbook.nextStep,
  };
}

function statusForRows(rows, nextRowStatus) {
  if (rows.every((entry) => entry.complete)) return "complete";
  if (!nextRowStatus?.readyForLiveRun) return "project_not_ready";
  return "ready_for_live_row";
}

function nextStep(status, nextRowStatus) {
  if (status === "complete") {
    return "All Phase 8 disposable loop rows have complete archived evidence.";
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
      : entry.archive.validationPath
        ? `latest validation ${entry.archive.validationStatus ?? "unknown"} at \`${entry.archive.validationPath}\``
        : "no archive yet";
    return `- Row #${entry.row}: ${entry.status} (${archive})`;
  }).join("\n");
  const commands = result.nextCommands.length > 0
    ? result.nextCommands.map((entry, index) => `${index + 1}. ${entry.label}\n\n   \`${entry.command}\``).join("\n\n")
    : "(none)";

  return `## Phase 8 Disposable Loop Status

Status: ${result.status}
Project: \`${result.projectPath}\`
Evidence dir: \`${result.outDir}\`
Next row: ${result.nextRow ? `#${result.nextRow}` : "(none)"}

Rows:
${rows}

Next commands:

${commands}

Next step: ${result.nextStep}
`;
}

function printHelp() {
  console.log(`Usage: node scripts/phase8-disposable-loop-status.mjs [--json|--markdown] [--project <path>] [--out-dir <path>] [--manual-dir <path>] [--date YYYY-MM-DD]

Reports Phase 8 disposable loop row archive coverage and the next live row runbook commands.

Options:
  --json             Print machine-readable status.
  --markdown         Print markdown status.
  --project PATH     Prepared disposable project. Defaults to ${DEFAULT_PROJECT_PATH}
  --out-dir PATH     Evidence archive directory. Defaults to ${DEFAULT_OUT_DIR}
  --manual-dir PATH  Directory for generated manual JSON templates. Defaults to ${DEFAULT_MANUAL_DIR}
  --date YYYY-MM-DD  Evidence date for next-row runbooks. Defaults to today.
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
    } else if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
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

  const result = generatePhase8DisposableLoopStatus(options);
  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    console.log(result.markdown);
  }
  return result.status === "project_not_ready" ? 1 : 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
