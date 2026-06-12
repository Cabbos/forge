import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, test } from "bun:test";
import { runCli } from "../src/cli.ts";
import type { SpawnRunner } from "../src/lib/spawn.ts";

const tempDirs: string[] = [];

function trackTempDir(prefix: string) {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  tempDirs.push(dir);
  return dir;
}

function createIo() {
  const stdout: string[] = [];
  const stderr: string[] = [];
  return {
    stdout,
    stderr,
    io: {
      stdout: (text: string) => stdout.push(text),
      stderr: (text: string) => stderr.push(text),
    },
  };
}

function createForgeRoot() {
  const forgeRoot = trackTempDir("forge-cli-route-forge-");
  mkdirSync(join(forgeRoot, "src-tauri"), { recursive: true });
  writeFileSync(join(forgeRoot, "src-tauri", "Cargo.toml"), "[package]\nname = \"forge-test\"\n");
  writeFileSync(join(forgeRoot, "package.json"), "{}\n");
  return forgeRoot;
}

function createEvalRunnerRoot() {
  const evalRunnerRoot = trackTempDir("forge-cli-route-eval-");
  mkdirSync(join(evalRunnerRoot, "app"), { recursive: true });
  mkdirSync(join(evalRunnerRoot, "eval_cases"), { recursive: true });
  return evalRunnerRoot;
}

function passingSpawnRunner(): SpawnRunner {
  return async () => ({ exitCode: 0, stdout: "", stderr: "" });
}

describe("runCli", () => {
  afterEach(() => {
    for (const dir of tempDirs.splice(0)) {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test("prints help when no command is provided", async () => {
    const { io, stdout } = createIo();

    const code = await runCli([], { io });

    expect(code).toBe(0);
    expect(stdout.join("")).toContain("Usage: forge <command>");
    expect(stdout.join("")).toContain("doctor");
    expect(stdout.join("")).toContain("run");
  });

  test("routes doctor command", async () => {
    const { io, stdout } = createIo();
    const forgeRoot = createForgeRoot();
    const evalRunnerRoot = createEvalRunnerRoot();

    const code = await runCli(["doctor"], {
      io,
      spawn: passingSpawnRunner(),
      env: {
        FORGE_REPO_ROOT: forgeRoot,
        FORGE_EVAL_RUNNER_ROOT: evalRunnerRoot,
      },
    });

    expect(code).toBe(0);
    expect(stdout.join("")).toContain("Forge doctor");
  });

  test("routes run command (requires --prompt)", async () => {
    const { io, stderr } = createIo();

    // Without --prompt, runCommand returns 1 with error message
    const code = await runCli(["run"], { io });

    expect(code).toBe(1);
    expect(stderr.join("")).toContain("prompt is required");
  });

  test("routes service command", async () => {
    const { io, stderr } = createIo();

    // No subcommand — should show usage
    const code = await runCli(["service"], { io });

    expect(code).toBe(1);
    expect(stderr.join("")).toContain("Usage: forge service");
  });

  test("returns non-zero for unknown commands", async () => {
    const { io, stderr } = createIo();

    const code = await runCli(["unknown"], { io });

    expect(code).toBe(1);
    expect(stderr.join("")).toContain("Unknown command: unknown");
  });
});
