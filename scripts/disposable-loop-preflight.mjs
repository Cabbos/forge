#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { existsSync, lstatSync, readFileSync, realpathSync } from "node:fs";
import { join, resolve } from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

const DEFAULT_PROJECT_PATH = "/Users/cabbos/project/forge-test-app";
const REQUIRED_FILES = ["src/App.tsx", "src/styles.css"];

export function evaluateDisposableLoopProject({ projectPath = DEFAULT_PROJECT_PATH } = {}) {
  const resolvedProjectPath = resolve(projectPath);
  const result = {
    status: "ready",
    readyForLoop: false,
    projectPath: resolvedProjectPath,
    requiredFiles: REQUIRED_FILES.map((file) => ({ file, exists: false })),
    git: {
      isRepo: false,
      root: null,
      rootMatchesProject: false,
      branchStatus: null,
      dirtyFiles: [],
      clean: false,
    },
    package: {
      exists: false,
      valid: false,
      name: null,
      scripts: {},
      hasBuildScript: false,
      hasDevScript: false,
    },
    issues: [],
    nextStep: null,
  };

  if (!existsSync(resolvedProjectPath)) {
    addIssue(result, "missing_project", "Disposable project path does not exist.");
    return finalize(result);
  }

  let stat;
  try {
    stat = lstatSync(resolvedProjectPath);
  } catch (error) {
    addIssue(result, "project_unreadable", `Disposable project path cannot be read: ${error.message}`);
    return finalize(result);
  }

  if (!stat.isDirectory()) {
    addIssue(result, "not_directory", "Disposable project path is not a directory.");
    return finalize(result);
  }

  evaluateGitState(result);
  evaluatePackage(result);
  evaluateRequiredFiles(result);

  return finalize(result);
}

function evaluateGitState(result) {
  try {
    const root = runGit(result.projectPath, ["rev-parse", "--show-toplevel"]);
    const branchStatus = runGit(result.projectPath, ["status", "--short", "--branch"]);
    const shortStatus = runGit(result.projectPath, ["status", "--short"]);
    const dirtyFiles = shortStatus.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);

    result.git.isRepo = true;
    result.git.root = root;
    result.git.rootMatchesProject = pathsEqual(root, result.projectPath);
    result.git.branchStatus = branchStatus;
    result.git.dirtyFiles = dirtyFiles;
    result.git.clean = dirtyFiles.length === 0;

    if (!result.git.rootMatchesProject) {
      addIssue(result, "git_root_mismatch", "Git root does not match the disposable project path.");
    }
    if (!result.git.clean) {
      addIssue(result, "dirty_worktree", "Disposable project has existing git changes.");
    }
  } catch (error) {
    result.git.error = gitErrorMessage(error);
    addIssue(result, "not_git_repo", "Disposable project is not a readable git worktree.");
  }
}

function evaluatePackage(result) {
  const packagePath = join(result.projectPath, "package.json");
  if (!existsSync(packagePath)) {
    addIssue(result, "missing_package_json", "Disposable project is missing package.json.");
    return;
  }

  result.package.exists = true;
  try {
    const parsed = JSON.parse(readFileSync(packagePath, "utf8"));
    const scripts = typeof parsed.scripts === "object" && parsed.scripts ? parsed.scripts : {};
    result.package.valid = true;
    result.package.name = typeof parsed.name === "string" ? parsed.name : null;
    result.package.scripts = Object.fromEntries(
      Object.entries(scripts).filter(([, value]) => typeof value === "string"),
    );
    result.package.hasBuildScript = typeof scripts.build === "string" && scripts.build.trim().length > 0;
    result.package.hasDevScript = typeof scripts.dev === "string" && scripts.dev.trim().length > 0;

    if (!result.package.hasBuildScript) {
      addIssue(result, "missing_build_script", "Disposable project package.json has no build script.");
    }
  } catch (error) {
    addIssue(result, "invalid_package_json", `Disposable project package.json cannot be parsed: ${error.message}`);
  }
}

function evaluateRequiredFiles(result) {
  result.requiredFiles = REQUIRED_FILES.map((file) => ({
    file,
    exists: existsSync(join(result.projectPath, file)),
  }));

  const missing = result.requiredFiles.filter((entry) => !entry.exists).map((entry) => entry.file);
  if (missing.length > 0) {
    addIssue(result, "missing_required_files", `Disposable project is missing required files: ${missing.join(", ")}`);
  }
}

function finalize(result) {
  result.readyForLoop = result.issues.length === 0;
  result.status = result.readyForLoop ? "ready" : result.issues[0].code;
  result.nextStep = result.readyForLoop
    ? "Start the Phase 8 disposable edit/build loop in Forge and record final-answer, diff, build/check, and confirmation evidence."
    : nextStepForIssue(result.issues[0]);
  return result;
}

function addIssue(result, code, message) {
  result.issues.push({ code, message });
}

function nextStepForIssue(issue) {
  switch (issue.code) {
    case "missing_project":
      return "Create or select the disposable project before running the live Forge loop.";
    case "dirty_worktree":
      return "Record, commit, stash, or reset existing disposable-project changes before treating a live run as fresh evidence.";
    case "not_git_repo":
      return "Initialize or choose a git-backed disposable project so changed-file and diff evidence is auditable.";
    case "git_root_mismatch":
      return "Use the git worktree root as the disposable project path before starting the live loop.";
    case "missing_build_script":
      return "Add or choose a project with a package build script so row #3 has a clear check command.";
    default:
      return "Resolve the preflight issue before starting the live Forge loop.";
  }
}

function pathsEqual(left, right) {
  return canonicalExistingPath(left) === canonicalExistingPath(right);
}

function canonicalExistingPath(path) {
  try {
    return realpathSync(path);
  } catch {
    return resolve(path);
  }
}

function runGit(cwd, args) {
  return execFileSync("git", args, {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}

function gitErrorMessage(error) {
  if (error?.stderr) return String(error.stderr).trim();
  if (error?.message) return error.message;
  return String(error);
}

function printHuman(result) {
  console.log("Disposable edit/build loop preflight");
  console.log(`Status: ${result.status}`);
  console.log(`Project: ${result.projectPath}`);
  console.log(`Ready for live loop: ${result.readyForLoop ? "yes" : "no"}`);
  if (result.git.isRepo) {
    console.log(`Git root: ${result.git.root}`);
    console.log(`Git clean: ${result.git.clean ? "yes" : "no"}`);
    if (result.git.dirtyFiles.length > 0) {
      console.log(`Dirty files: ${result.git.dirtyFiles.join(", ")}`);
    }
  }
  if (result.package.exists) {
    console.log(`Package: ${result.package.name ?? "(unnamed)"}`);
    console.log(`Build script: ${result.package.hasBuildScript ? result.package.scripts.build : "(missing)"}`);
  }
  if (result.issues.length > 0) {
    console.log(`Issues: ${result.issues.map((issue) => issue.code).join(", ")}`);
  }
  console.log(`Next step: ${result.nextStep}`);
}

function printHelp() {
  console.log(`Usage: node scripts/disposable-loop-preflight.mjs [--json] [--project <path>] [--require-ready]

Checks whether the disposable project is ready for the Phase 8 live edit/build loop.

Options:
  --json           Print machine-readable status.
  --project PATH   Project path to inspect. Defaults to ${DEFAULT_PROJECT_PATH}
  --require-ready  Exit non-zero when the project is not ready for a fresh live loop.
  -h, --help       Show this help.
`);
}

function parseArgs(argv) {
  const options = {
    json: false,
    requireReady: false,
    projectPath: DEFAULT_PROJECT_PATH,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") {
      options.json = true;
    } else if (arg === "--require-ready") {
      options.requireReady = true;
    } else if (arg === "--project") {
      const value = argv[index + 1];
      if (!value) throw new Error("--project requires a path");
      options.projectPath = value;
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

  const result = evaluateDisposableLoopProject({ projectPath: options.projectPath });
  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    printHuman(result);
  }

  return options.requireReady && !result.readyForLoop ? 1 : 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
