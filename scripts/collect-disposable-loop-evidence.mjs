#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import process from "node:process";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { evaluateDisposableLoopProject } from "./disposable-loop-preflight.mjs";

const DEFAULT_PROJECT_PATH = "/Users/cabbos/project/forge-test-app-phase8-clean";
const DEFAULT_BUILD_ARGS = ["--prefix", DEFAULT_PROJECT_PATH, "run", "build"];

export function collectDisposableLoopEvidence({
  projectPath = DEFAULT_PROJECT_PATH,
  row = "all",
  runBuild = false,
  includeDiff = false,
  date = currentDate(),
} = {}) {
  const resolvedProjectPath = resolve(projectPath);
  const preflight = evaluateDisposableLoopProject({ projectPath: resolvedProjectPath });
  const git = collectGitEvidence(resolvedProjectPath);
  const build = runBuild ? runBuildEvidence(resolvedProjectPath) : skippedBuildEvidence(resolvedProjectPath);

  const result = {
    status: statusForEvidence(preflight, git, build),
    projectPath: resolvedProjectPath,
    row,
    date,
    preflight: summarizePreflight(preflight),
    git,
    build,
    manualFields: manualFieldsForRow(row),
  };

  if (includeDiff) {
    result.diff = {
      unstaged: runGitOptional(resolvedProjectPath, ["diff", "--"]),
      staged: runGitOptional(resolvedProjectPath, ["diff", "--cached", "--"]),
    };
  }

  result.markdown = renderMarkdownEvidence(result);
  return result;
}

function collectGitEvidence(projectPath) {
  const statusShort = runGitOptional(projectPath, ["status", "--short"]);
  const branchStatus = runGitOptional(projectPath, ["status", "--short", "--branch"]);
  const changedFiles = parseStatusFiles(statusShort);
  const unstagedNameStatus = runGitOptional(projectPath, ["diff", "--name-status"]);
  const stagedNameStatus = runGitOptional(projectPath, ["diff", "--cached", "--name-status"]);
  const unstagedStat = runGitOptional(projectPath, ["diff", "--stat"]);
  const stagedStat = runGitOptional(projectPath, ["diff", "--cached", "--stat"]);

  return {
    branchStatus,
    statusShort,
    clean: changedFiles.length === 0,
    changedFiles,
    unstagedNameStatus,
    stagedNameStatus,
    unstagedStat,
    stagedStat,
  };
}

function runBuildEvidence(projectPath) {
  const args = ["--prefix", projectPath, "run", "build"];
  try {
    const output = execFileSync("npm", args, {
      cwd: projectPath,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    return {
      ran: true,
      command: commandText("npm", args),
      success: true,
      exitCode: 0,
      outputTail: tailLines(output, 25),
    };
  } catch (error) {
    const output = `${error.stdout ?? ""}${error.stderr ?? ""}`;
    return {
      ran: true,
      command: commandText("npm", args),
      success: false,
      exitCode: typeof error.status === "number" ? error.status : 1,
      outputTail: tailLines(output, 25),
    };
  }
}

function skippedBuildEvidence(projectPath) {
  return {
    ran: false,
    command: commandText("npm", ["--prefix", projectPath, "run", "build"]),
    success: null,
    exitCode: null,
    outputTail: "",
  };
}

function statusForEvidence(preflight, git, build) {
  if (!preflight.readyForLoop && git.clean) return "project_not_ready";
  if (build.ran && !build.success) return "build_failed";
  if (git.clean) return "no_changes_yet";
  return "changes_detected";
}

function summarizePreflight(result) {
  return {
    status: result.status,
    readyForLoop: result.readyForLoop,
    issues: result.issues,
    hasBuildScript: result.package.hasBuildScript,
    requiredFiles: result.requiredFiles,
  };
}

function parseStatusFiles(statusShort) {
  if (!statusShort.trim()) return [];
  return statusShort
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => {
      const status = line.slice(0, 2);
      const rawPath = line.slice(3).trim();
      const renamedPath = rawPath.includes(" -> ") ? rawPath.split(" -> ").at(-1) : rawPath;
      return {
        status,
        file: renamedPath,
        raw: line,
      };
    });
}

function manualFieldsForRow(row) {
  const fields = [
    "Forge prompt",
    "Forge final answer",
    "Confirmation behavior",
    "Screenshot or transcript reference",
  ];
  if (row === "1" || row === "all") fields.push("Row #1 visible feedback fix result");
  if (row === "2" || row === "all") fields.push("Row #2 style-only polish result");
  if (row === "3" || row === "all") fields.push("Row #3 command-only check result");
  return fields.map((label) => ({ label, value: "" }));
}

function renderMarkdownEvidence(result) {
  const changedFiles = result.git.changedFiles.length > 0
    ? result.git.changedFiles.map((entry) => `- ${entry.status.trim() || "??"} ${entry.file}`).join("\n")
    : "- (none)";
  const diffStat = [result.git.stagedStat, result.git.unstagedStat].filter(Boolean).join("\n") || "(none)";
  const nameStatus = [result.git.stagedNameStatus, result.git.unstagedNameStatus].filter(Boolean).join("\n") || "(none)";
  const buildSummary = result.build.ran
    ? `${result.build.success ? "passed" : "failed"}: \`${result.build.command}\``
    : `not run: \`${result.build.command}\``;
  const buildOutput = result.build.outputTail ? fenced(result.build.outputTail) : "(not captured)";
  const manualFields = result.manualFields.map((field) => `- ${field.label}:`).join("\n");

  return `## Phase 8 Disposable Loop Evidence - ${result.date}

Status: ${result.status}
Project: \`${result.projectPath}\`
Row scope: ${result.row}

Preflight:
- readyForLoop: ${result.preflight.readyForLoop}
- status: ${result.preflight.status}

Git state:
- branch/status: ${inlineCodeOrNone(result.git.branchStatus)}
- changed files:
${changedFiles}

Name status:
${fenced(nameStatus)}

Diff stat:
${fenced(diffStat)}

Build/check:
- ${buildSummary}
- output:
${buildOutput}

Manual Forge evidence to paste:
${manualFields}
`;
}

function fenced(value) {
  return `\`\`\`text\n${value}\n\`\`\``;
}

function inlineCodeOrNone(value) {
  return value ? `\`${value.replaceAll("`", "'")}\`` : "`(none)`";
}

function runGitOptional(cwd, args) {
  try {
    return execFileSync("git", args, {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trimEnd();
  } catch {
    return "";
  }
}

function tailLines(value, maxLines) {
  const lines = String(value).trim().split(/\r?\n/).filter(Boolean);
  return lines.slice(-maxLines).join("\n");
}

function commandText(command, args) {
  return [command, ...args].map(shellToken).join(" ");
}

function shellToken(value) {
  const text = String(value);
  return /^[A-Za-z0-9_./:=@-]+$/.test(text) ? text : `'${text.replaceAll("'", "'\\''")}'`;
}

function currentDate() {
  return new Date().toISOString().slice(0, 10);
}

function printHelp() {
  console.log(`Usage: node scripts/collect-disposable-loop-evidence.mjs [--json|--markdown] [--project <path>] [--row <all|1|2|3>] [--run-build] [--include-diff]

Collects git diff, changed-file, optional build/check output, and markdown placeholders for Phase 8 disposable loop evidence.

Options:
  --json          Print machine-readable evidence, including markdown.
  --markdown      Print only the markdown evidence template.
  --project PATH  Project path to inspect. Defaults to ${DEFAULT_PROJECT_PATH}
  --row VALUE     Row scope to label: all, 1, 2, or 3. Defaults to all.
  --run-build     Run npm --prefix <project> run build and capture the output tail.
  --include-diff  Include full staged/unstaged diff text in JSON output.
  -h, --help      Show this help.

Default build command: npm ${DEFAULT_BUILD_ARGS.join(" ")}
`);
}

function parseArgs(argv) {
  const options = {
    json: false,
    markdown: false,
    projectPath: DEFAULT_PROJECT_PATH,
    row: "all",
    runBuild: false,
    includeDiff: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") {
      options.json = true;
    } else if (arg === "--markdown") {
      options.markdown = true;
    } else if (arg === "--run-build") {
      options.runBuild = true;
    } else if (arg === "--include-diff") {
      options.includeDiff = true;
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

  const result = collectDisposableLoopEvidence(options);
  if (options.markdown) {
    console.log(result.markdown);
  } else if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    console.log(result.markdown);
  }
  return result.build.ran && result.build.success === false ? 1 : 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
