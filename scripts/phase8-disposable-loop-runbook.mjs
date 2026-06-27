#!/usr/bin/env node
import process from "node:process";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { archiveDisposableLoopEvidence } from "./archive-disposable-loop-evidence.mjs";
import { createDisposableLoopManualTemplate } from "./create-disposable-loop-manual-json.mjs";
import { evaluateDisposableLoopProject } from "./disposable-loop-preflight.mjs";

const DEFAULT_PROJECT_PATH = "/Users/cabbos/project/forge-test-app-phase8-clean";
const DEFAULT_MANUAL_DIR = "/tmp";

export function generatePhase8DisposableLoopRunbook({
  projectPath = DEFAULT_PROJECT_PATH,
  row = "1",
  manualDir = DEFAULT_MANUAL_DIR,
  date = currentDate(),
} = {}) {
  const normalizedRow = String(row);
  const resolvedProjectPath = resolve(projectPath);
  const preflight = evaluateDisposableLoopProject({ projectPath: resolvedProjectPath });
  const archive = archiveDisposableLoopEvidence({
    projectPath: resolvedProjectPath,
    row: normalizedRow,
    date,
    dryRun: true,
  });
  const manualPath = resolve(manualDir, `phase8-row-${normalizedRow}-manual.json`);
  const prompt = createDisposableLoopManualTemplate({ row: normalizedRow })["Forge prompt"];
  const commands = buildCommands({ projectPath: resolvedProjectPath, row: normalizedRow, manualPath });
  const status = preflight.readyForLoop ? archive.validationStatus : "project_not_ready";

  const result = {
    status,
    readyForLiveRun: preflight.readyForLoop,
    projectPath: resolvedProjectPath,
    row: normalizedRow,
    date,
    prompt,
    manualPath,
    commands,
    preflight: {
      status: preflight.status,
      readyForLoop: preflight.readyForLoop,
      issues: preflight.issues,
      dirtyFiles: preflight.git.dirtyFiles,
    },
    archiveDryRun: {
      status: archive.status,
      validationStatus: archive.validationStatus,
      validationPass: archive.validationPass,
      paths: archive.paths,
    },
    nextStep: nextStep({ preflight, archive, row: normalizedRow, manualPath }),
    markdown: "",
  };
  result.markdown = renderMarkdown(result);
  return result;
}

function buildCommands({ projectPath, row, manualPath }) {
  return [
    {
      label: "create manual evidence template",
      command: commandText("node", ["scripts/create-disposable-loop-manual-json.mjs", "--row", row, "--out", manualPath]),
    },
    {
      label: "open this project in Forge",
      command: `Open Forge project: ${projectPath}`,
    },
    {
      label: "send this row prompt in Forge",
      command: "Paste the prompt from this runbook into Forge.",
    },
    {
      label: "collect evidence packet",
      command: commandText("node", [
        "scripts/collect-disposable-loop-evidence.mjs",
        "--markdown",
        "--project",
        projectPath,
        "--row",
        row,
        "--run-build",
      ]),
    },
    {
      label: "review manual evidence",
      command: commandText("node", [
        "scripts/review-disposable-loop-manual-json.mjs",
        "--json",
        "--row",
        row,
        "--manual-json",
        manualPath,
        "--require-complete",
      ]),
    },
    {
      label: "validate strict evidence",
      command: commandText("node", [
        "scripts/validate-disposable-loop-evidence.mjs",
        "--json",
        "--project",
        projectPath,
        "--row",
        row,
        "--manual-json",
        manualPath,
        "--run-build",
        "--require-complete",
      ]),
    },
    {
      label: "archive strict evidence",
      command: commandText("node", [
        "scripts/archive-disposable-loop-evidence.mjs",
        "--json",
        "--project",
        projectPath,
        "--row",
        row,
        "--manual-json",
        manualPath,
        "--run-build",
        "--require-complete",
      ]),
    },
  ];
}

function nextStep({ preflight, archive, row, manualPath }) {
  if (!preflight.readyForLoop) {
    return "Resolve project preflight issues before running the live Forge row.";
  }
  if (archive.validationStatus === "pending_live_evidence") {
    return `Create ${manualPath}, run row #${row} in Forge, fill the manual JSON fields, review them, then run validate and archive commands.`;
  }
  return "Follow the commands in order and keep the generated archive files with the beta evidence.";
}

function renderMarkdown(result) {
  const commands = result.commands.map((entry, index) => `${index + 1}. ${entry.label}\n\n   \`${entry.command}\``).join("\n\n");
  return `## Phase 8 Disposable Loop Runbook - Row ${result.row}

Status: ${result.status}
Project: \`${result.projectPath}\`
Manual JSON: \`${result.manualPath}\`

Prompt:

\`\`\`text
${result.prompt}
\`\`\`

Commands:

${commands}

Next step: ${result.nextStep}
`;
}

function printHelp() {
  console.log(`Usage: node scripts/phase8-disposable-loop-runbook.mjs [--json|--markdown] [--project <path>] [--row <1|2|3|all>] [--manual-dir <path>] [--date YYYY-MM-DD]

Prints the row-by-row Phase 8 live Forge runbook using the existing preflight, collector, validator, and archive helpers.

Options:
  --json             Print machine-readable runbook.
  --markdown         Print markdown runbook.
  --project PATH     Prepared disposable project. Defaults to ${DEFAULT_PROJECT_PATH}
  --row VALUE        Row scope: 1, 2, 3, or all. Defaults to 1.
  --manual-dir PATH  Directory for generated manual JSON templates. Defaults to ${DEFAULT_MANUAL_DIR}
  --date YYYY-MM-DD  Evidence date. Defaults to today.
  -h, --help         Show this help.
`);
}

function parseArgs(argv) {
  const options = {
    json: false,
    markdown: false,
    projectPath: DEFAULT_PROJECT_PATH,
    row: "1",
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
    } else if (arg === "--row") {
      const value = argv[index + 1];
      if (!["all", "1", "2", "3"].includes(value)) throw new Error("--row must be one of: all, 1, 2, 3");
      options.row = value;
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

function commandText(command, args) {
  return [command, ...args].map(shellToken).join(" ");
}

function shellToken(value) {
  const text = String(value);
  return /^[A-Za-z0-9_./:=@-]+$/.test(text) ? text : `'${text.replaceAll("'", "'\\''")}'`;
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

  const result = generatePhase8DisposableLoopRunbook(options);
  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    console.log(result.markdown);
  }
  return result.readyForLiveRun ? 0 : 1;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
