#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync, symlinkSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

import { evaluateDisposableLoopProject } from "./disposable-loop-preflight.mjs";

const DEFAULT_SOURCE_PATH = "/Users/cabbos/project/forge-test-app";

export function prepareDisposableLoopProject({
  sourcePath = DEFAULT_SOURCE_PATH,
  targetPath = defaultTargetForSource(sourcePath),
  dryRun = false,
  linkNodeModules = true,
} = {}) {
  const resolvedSourcePath = resolve(sourcePath);
  const resolvedTargetPath = resolve(targetPath);
  const result = {
    status: "pending",
    prepared: false,
    dryRun,
    sourcePath: resolvedSourcePath,
    targetPath: resolvedTargetPath,
    targetExists: existsSync(resolvedTargetPath),
    source: null,
    target: null,
    nodeModules: {
      sourceExists: existsSync(join(resolvedSourcePath, "node_modules")),
      linked: false,
      requested: linkNodeModules,
    },
    commands: [],
    issues: [],
    nextStep: null,
  };

  const source = evaluateDisposableLoopProject({ projectPath: resolvedSourcePath });
  result.source = summarizePreflight(source);

  if (!source.git.isRepo) {
    addIssue(result, "source_not_git_repo", "Source disposable project is not a readable git worktree.");
    return finalize(result);
  }

  if (!source.package.exists || !source.package.valid || !source.package.hasBuildScript) {
    addIssue(result, "source_not_buildable", "Source disposable project does not expose a valid package build script.");
    return finalize(result);
  }

  if (source.requiredFiles.some((entry) => !entry.exists)) {
    addIssue(result, "source_missing_required_files", "Source disposable project is missing required demo files.");
    return finalize(result);
  }

  if (pathsEqual(resolvedSourcePath, resolvedTargetPath)) {
    addIssue(result, "target_matches_source", "Target path must differ from the source project path.");
    return finalize(result);
  }

  if (result.targetExists) {
    result.target = summarizePreflight(evaluateDisposableLoopProject({ projectPath: resolvedTargetPath }));
    if (result.target.readyForLoop) {
      result.prepared = true;
      result.status = "target_ready";
      result.nextStep = "Open the existing ready target project in Forge for fresh live evidence.";
      return result;
    }
    addIssue(result, "target_exists_not_ready", "Target path already exists but is not ready for the live loop.");
    return finalize(result);
  }

  result.commands.push(`git -C ${shellQuote(resolvedSourcePath)} worktree add --detach ${shellQuote(resolvedTargetPath)} HEAD`);
  if (linkNodeModules && result.nodeModules.sourceExists) {
    result.commands.push(`ln -s ${shellQuote(join(resolvedSourcePath, "node_modules"))} ${shellQuote(join(resolvedTargetPath, "node_modules"))}`);
  }

  if (dryRun) {
    result.status = "dry_run_ready";
    result.nextStep = "Run the prepare command without --dry-run, then open the target project in Forge for the live edit/build loop.";
    return result;
  }

  mkdirSync(dirname(resolvedTargetPath), { recursive: true });
  let worktreeCreated = false;
  try {
    runGit(resolvedSourcePath, ["worktree", "add", "--detach", resolvedTargetPath, "HEAD"]);
    worktreeCreated = true;
    if (linkNodeModules && result.nodeModules.sourceExists) {
      symlinkSync(join(resolvedSourcePath, "node_modules"), join(resolvedTargetPath, "node_modules"), "dir");
      result.nodeModules.linked = true;
    }
  } catch (error) {
    cleanupFailedTarget(resolvedSourcePath, resolvedTargetPath, worktreeCreated);
    addIssue(result, "prepare_failed", commandErrorMessage(error));
    return finalize(result);
  }

  result.target = summarizePreflight(evaluateDisposableLoopProject({ projectPath: resolvedTargetPath }));
  result.prepared = result.target.readyForLoop;
  result.status = result.prepared ? "prepared" : "prepared_but_not_ready";
  result.nextStep = result.prepared
    ? "Open the prepared target project in Forge and run Phase 8 rows #1-#3 for fresh live evidence."
    : "Inspect target preflight issues before using it as live evidence.";
  return result;
}

function summarizePreflight(result) {
  return {
    status: result.status,
    readyForLoop: result.readyForLoop,
    projectPath: result.projectPath,
    gitRoot: result.git.root,
    gitClean: result.git.clean,
    dirtyFiles: result.git.dirtyFiles,
    packageName: result.package.name,
    hasBuildScript: result.package.hasBuildScript,
    requiredFiles: result.requiredFiles,
    issues: result.issues,
  };
}

function finalize(result) {
  result.status = result.issues[0]?.code ?? "unknown";
  result.nextStep = nextStepForIssue(result.issues[0]);
  return result;
}

function addIssue(result, code, message) {
  result.issues.push({ code, message });
}

function nextStepForIssue(issue) {
  switch (issue?.code) {
    case "target_exists_not_ready":
      return "Choose a new target path, or inspect and fix the existing target project before using it as evidence.";
    case "source_not_git_repo":
      return "Choose a git-backed disposable source project.";
    case "source_not_buildable":
      return "Choose a source project with a valid package build script.";
    case "source_missing_required_files":
      return "Choose a source project with the expected demo files for Phase 8 rows #1-#3.";
    case "target_matches_source":
      return "Choose a separate target path so source changes are preserved.";
    default:
      return "Resolve the prepare issue, then rerun the helper.";
  }
}

function defaultTargetForSource(sourcePath) {
  const resolvedSourcePath = resolve(sourcePath);
  return join(dirname(resolvedSourcePath), `${basename(resolvedSourcePath)}-phase8-clean`);
}

function runGit(cwd, args) {
  return execFileSync("git", args, {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}

function cleanupFailedTarget(sourcePath, targetPath, worktreeCreated) {
  if (worktreeCreated) {
    try {
      runGit(sourcePath, ["worktree", "remove", "--force", targetPath]);
      return;
    } catch {
      // Fall back to filesystem cleanup below.
    }
  }
  rmSync(targetPath, { recursive: true, force: true });
}

function pathsEqual(left, right) {
  return resolve(left) === resolve(right);
}

function commandErrorMessage(error) {
  if (error?.stderr) return String(error.stderr).trim();
  if (error?.message) return error.message;
  return String(error);
}

function shellQuote(value) {
  return `'${String(value).replaceAll("'", "'\\''")}'`;
}

function printHuman(result) {
  console.log("Prepare disposable edit/build loop project");
  console.log(`Status: ${result.status}`);
  console.log(`Source: ${result.sourcePath}`);
  console.log(`Target: ${result.targetPath}`);
  console.log(`Dry run: ${result.dryRun ? "yes" : "no"}`);
  if (result.source) {
    console.log(`Source clean: ${result.source.gitClean ? "yes" : "no"}`);
    if (result.source.dirtyFiles.length > 0) {
      console.log(`Source dirty files preserved: ${result.source.dirtyFiles.join(", ")}`);
    }
  }
  if (result.commands.length > 0) {
    console.log("Commands:");
    for (const command of result.commands) console.log(`  ${command}`);
  }
  if (result.target) {
    console.log(`Target ready: ${result.target.readyForLoop ? "yes" : "no"}`);
  }
  if (result.issues.length > 0) {
    console.log(`Issues: ${result.issues.map((issue) => issue.code).join(", ")}`);
  }
  console.log(`Next step: ${result.nextStep}`);
}

function printHelp() {
  console.log(`Usage: node scripts/prepare-disposable-loop-project.mjs [--json] [--dry-run] [--source <path>] [--target <path>] [--no-link-node-modules]

Creates a clean git worktree from a disposable project's HEAD for the Phase 8 live edit/build loop.
The source project is not reset, stashed, or otherwise modified.

Options:
  --json                  Print machine-readable status.
  --dry-run               Print the planned preparation without creating the target.
  --source PATH           Source project. Defaults to ${DEFAULT_SOURCE_PATH}
  --target PATH           Target clean worktree. Defaults to <source>-phase8-clean.
  --no-link-node-modules  Do not symlink source node_modules into the target.
  -h, --help              Show this help.
`);
}

function parseArgs(argv) {
  const options = {
    json: false,
    dryRun: false,
    sourcePath: DEFAULT_SOURCE_PATH,
    targetPath: null,
    linkNodeModules: true,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") {
      options.json = true;
    } else if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "--no-link-node-modules") {
      options.linkNodeModules = false;
    } else if (arg === "--source") {
      const value = argv[index + 1];
      if (!value) throw new Error("--source requires a path");
      options.sourcePath = value;
      index += 1;
    } else if (arg === "--target") {
      const value = argv[index + 1];
      if (!value) throw new Error("--target requires a path");
      options.targetPath = value;
      index += 1;
    } else if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  if (!options.targetPath) options.targetPath = defaultTargetForSource(options.sourcePath);
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

  const result = prepareDisposableLoopProject(options);
  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    printHuman(result);
  }
  return result.issues.length > 0 && !options.dryRun ? 1 : 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
