#!/usr/bin/env node
/**
 * Worktree Worker Smoke Test — Phase 6
 *
 * This script exercises the full delegate_task(mode="worktree_worker") path
 * through the Forge headless eval agent. It:
 *
 * 1. Creates a temporary git repo with an initial commit.
 * 2. Runs `forge_eval_agent` with a prompt that should trigger worktree_worker.
 * 3. Parses the trace output and validates that the A2A projection contains
 *    all required WorktreeWorkerSummary fields:
 *    - diff_available, diff_truncated
 *    - tests_passed, needs_human_review
 *    - suggested_action, reason_codes
 *    - worktree_path, cleaned_up
 *
 * Usage:
 *   node scripts/smoke-worktree-worker.mjs --dry-run
 *   node scripts/smoke-worktree-worker.mjs --require-key
 *
 * Pass criteria (real mode):
 *   - Agent emits at least one agent_a2a_updated event.
 *   - At least one task has execution_mode == "worktree_worker".
 *   - The worktree task's projection contains non-null worktree metadata.
 *
 * Fail criteria:
 *   - No worktree_worker task was created.
 *   - Missing required fields in the projection.
 *   - Agent error or timeout.
 */

import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, mkdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const FORGE_EVAL_AGENT_CMD = [
  "cargo",
  "run",
  "--manifest-path",
  join(repoRoot, "src-tauri", "Cargo.toml"),
  "--bin",
  "forge_eval_agent",
  "--quiet",
];

function parseArgs(argv) {
  return {
    dryRun: argv.includes("--dry-run"),
    requireKey: argv.includes("--require-key"),
    timeoutSecs: argv.includes("--timeout")
      ? parseInt(argv[argv.indexOf("--timeout") + 1], 10)
      : 120,
    maxModelRounds: argv.includes("--max-model-rounds")
      ? parseInt(argv[argv.indexOf("--max-model-rounds") + 1], 10)
      : 20,
  };
}

function checkApiKey() {
  const envKeys = [
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "DEEPSEEK_API_KEY",
    "OPENAI_API_KEY",
    "OPENROUTER_API_KEY",
  ];
  for (const key of envKeys) {
    const value = process.env[key];
    if (typeof value === "string" && value.trim().length > 0) {
      return { ok: true, source: key };
    }
  }
  try {
    const configPath = join(process.env.HOME ?? "/", ".forge", "config.json");
    const config = JSON.parse(readFileSync(configPath, "utf8"));
    const keys = config.api_keys ?? {};
    for (const [provider, key] of Object.entries(keys)) {
      if (typeof key === "string" && key.trim().length > 0) {
        return { ok: true, source: `config:${provider}` };
      }
    }
  } catch {
    // ignore
  }
  return { ok: false };
}

function initGitRepo(dir) {
  const run = (cmd, args = []) => {
    const result = spawnSync(cmd, args, { cwd: dir, encoding: "utf8" });
    if (result.status !== 0) {
      throw new Error(`${cmd} ${args.join(" ")} failed: ${result.stderr}`);
    }
  };
  run("git", ["init"]);
  run("git", ["config", "user.email", "smoke@test"])
  run("git", ["config", "user.name", "Smoke Test"]);
  writeFileSync(join(dir, "README.md"), "# smoke\n");
  run("git", ["add", "."]);
  run("git", ["commit", "-m", "init", "--no-gpg-sign"]);
  return dir;
}

function buildRequest(workspacePath, timeoutSecs, maxModelRounds) {
  return JSON.stringify(
    {
      prompt:
        "In this workspace, create a new file src/greet.ts that exports a function greet(name: string): string returning 'Hello, {name}!'. " +
        "Then create src/greet.test.ts with a test that calls greet('World') and asserts the result is 'Hello, World!'. " +
        "Run the tests with npm test (or jest, or vitest — whichever is available) and report the result. " +
        "Use delegate_task with mode=worktree_worker to delegate the implementation to a sub-agent in an isolated worktree.",
      workspace_path: workspacePath,
      task: {
        id: "smoke-worktree-worker",
        timeout_secs: timeoutSecs,
        max_model_rounds: maxModelRounds,
      },
    },
    null,
    2,
  );
}

function validateTrace(trace) {
  const errors = [];

  if (!trace.raw_events || !Array.isArray(trace.raw_events)) {
    errors.push("Missing or invalid raw_events array");
    return errors;
  }

  const a2aEvents = trace.raw_events.filter(
    (e) => e?.event_type === "agent_a2a_updated",
  );

  if (a2aEvents.length === 0) {
    errors.push("No agent_a2a_updated events found in trace");
    return errors;
  }

  const worktreeTasks = [];
  for (const event of a2aEvents) {
    const state = event.state;
    if (!state || !Array.isArray(state.tasks)) continue;
    for (const task of state.tasks) {
      if (task.execution_mode === "worktree_worker") {
        worktreeTasks.push(task);
      }
    }
  }

  if (worktreeTasks.length === 0) {
    errors.push("No worktree_worker task found in any agent_a2a_updated event");
    return errors;
  }

  const task = worktreeTasks[worktreeTasks.length - 1];

  const requiredFields = [
    { key: "needs_human_review", type: "boolean" },
    { key: "tests_passed", type: "boolean" },
    { key: "diff_truncated", type: "boolean" },
    { key: "worktree_path", type: "string" },
    { key: "cleaned_up", type: "boolean" },
    { key: "suggested_action", type: "string" },
  ];

  for (const { key, type } of requiredFields) {
    const value = task[key];
    if (value === null || value === undefined) {
      errors.push(`Missing required field: ${key}`);
    } else if (type === "boolean" && typeof value !== "boolean") {
      errors.push(`Field ${key} should be boolean, got ${typeof value}`);
    } else if (type === "string" && typeof value !== "string") {
      errors.push(`Field ${key} should be string, got ${typeof value}`);
    }
  }

  if (!Array.isArray(task.reason_codes)) {
    errors.push("reason_codes should be an array");
  }

  return errors;
}

function main() {
  const args = parseArgs(process.argv.slice(2));

  if (args.requireKey) {
    const keyCheck = checkApiKey();
    if (!keyCheck.ok) {
      console.error(
        "[smoke-worktree-worker] ERROR: No API key found.\n" +
          "  Set ANTHROPIC_API_KEY, DEEPSEEK_API_KEY, OPENAI_API_KEY, or OPENROUTER_API_KEY.\n" +
          "  Or add a key to ~/.forge/config.json\n" +
          "  To preview the plan without a key, use --dry-run.",
      );
      process.exit(1);
    }
    console.log(`[smoke-worktree-worker] Using API key from: ${keyCheck.source}`);
  }

  const tmpDir = mkdtempSync(join(tmpdir(), "forge-wt-smoke-"));
  const workspacePath = join(tmpDir, "workspace");
  mkdirSync(workspacePath, { recursive: true });

  if (args.dryRun) {
    console.log("[smoke-worktree-worker] DRY RUN");
    console.log(`  workspace: ${workspacePath}`);
    console.log(`  command: ${FORGE_EVAL_AGENT_CMD.join(" ")}`);
    console.log(`  timeout: ${args.timeoutSecs}s`);
    console.log(`  max_model_rounds: ${args.maxModelRounds}`);
    const request = buildRequest(workspacePath, args.timeoutSecs, args.maxModelRounds);
    console.log("  request JSON preview:");
    console.log(request.slice(0, 500) + "...");
    console.log("  Pass criteria:");
    console.log("    - agent_a2a_updated event contains worktree_worker task");
    console.log("    - task projection has needs_human_review, tests_passed, diff_truncated,");
    console.log("      worktree_path, cleaned_up, suggested_action, reason_codes");
    rmSync(tmpDir, { recursive: true, force: true });
    process.exit(0);
  }

  console.log(`[smoke-worktree-worker] Initializing git repo at ${workspacePath}...`);
  initGitRepo(workspacePath);

  const request = buildRequest(workspacePath, args.timeoutSecs, args.maxModelRounds);

  console.log("[smoke-worktree-worker] Running forge_eval_agent...");
  const startTime = Date.now();
  const result = spawnSync(FORGE_EVAL_AGENT_CMD[0], FORGE_EVAL_AGENT_CMD.slice(1), {
    input: request,
    encoding: "utf8",
    timeout: (args.timeoutSecs + 60) * 1000,
    env: { ...process.env, RUST_LOG: "warn" },
  });
  const elapsed = Date.now() - startTime;

  if (result.error) {
    console.error(`[smoke-worktree-worker] Spawn error: ${result.error.message}`);
    rmSync(tmpDir, { recursive: true, force: true });
    process.exit(1);
  }

  if (result.status !== 0) {
    console.error(`[smoke-worktree-worker] Agent exited with code ${result.status}`);
    console.error(result.stderr);
    rmSync(tmpDir, { recursive: true, force: true });
    process.exit(1);
  }

  let trace;
  try {
    trace = JSON.parse(result.stdout);
  } catch (error) {
    console.error(`[smoke-worktree-worker] Failed to parse agent output: ${error.message}`);
    console.error("stdout preview:", result.stdout.slice(0, 500));
    rmSync(tmpDir, { recursive: true, force: true });
    process.exit(1);
  }

  console.log(`[smoke-worktree-worker] Agent completed in ${elapsed}ms`);
  console.log(`[smoke-worktree-worker] model_rounds: ${trace.model_rounds ?? "N/A"}`);
  console.log(`[smoke-worktree-worker] failure_category: ${trace.failure_category ?? "N/A"}`);

  const errors = validateTrace(trace);

  if (errors.length > 0) {
    console.error("[smoke-worktree-worker] VALIDATION FAILED:");
    for (const error of errors) {
      console.error(`  ✗ ${error}`);
    }
    rmSync(tmpDir, { recursive: true, force: true });
    process.exit(1);
  }

  console.log("[smoke-worktree-worker] All validations passed ✓");
  console.log("[smoke-worktree-worker] Found worktree_worker task with required metadata fields.");

  rmSync(tmpDir, { recursive: true, force: true });
  process.exit(0);
}

main();
