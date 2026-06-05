import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import { runDoctor } from "../src/commands/doctor.ts";
import {
  defaultEvalRunnerRoot,
  defaultForgeRepoRoot,
  hasEvalRunner,
  isForgeRepoRoot,
} from "../src/lib/paths.ts";
import type { SpawnRunner } from "../src/lib/spawn.ts";

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
  const forgeRoot = mkdtempSync(join(tmpdir(), "forge-cli-doctor-forge-"));
  mkdirSync(join(forgeRoot, "src-tauri"), { recursive: true });
  writeFileSync(join(forgeRoot, "src-tauri", "Cargo.toml"), "[package]\nname = \"forge-test\"\n");
  writeFileSync(join(forgeRoot, "package.json"), "{}\n");
  return forgeRoot;
}

function createEvalRunnerRoot() {
  const evalRunnerRoot = mkdtempSync(join(tmpdir(), "forge-cli-doctor-eval-"));
  mkdirSync(join(evalRunnerRoot, "app"), { recursive: true });
  mkdirSync(join(evalRunnerRoot, "eval_cases"), { recursive: true });
  return evalRunnerRoot;
}

function passingSpawnRunner(): SpawnRunner {
  return async () => ({ exitCode: 0, stdout: "", stderr: "" });
}

function failingSpawnRunner(): SpawnRunner {
  return async () => ({ exitCode: 1, stdout: "", stderr: "missing\n" });
}

describe("path helpers", () => {
  test("uses explicit Forge and eval runner roots from env", () => {
    const forgeRoot = createForgeRoot();
    const evalRunnerRoot = createEvalRunnerRoot();

    expect(defaultForgeRepoRoot({ FORGE_REPO_ROOT: forgeRoot })).toBe(forgeRoot);
    expect(defaultEvalRunnerRoot(forgeRoot, { FORGE_EVAL_RUNNER_ROOT: evalRunnerRoot })).toBe(
      evalRunnerRoot,
    );
    expect(isForgeRepoRoot(forgeRoot)).toBe(true);
    expect(hasEvalRunner(evalRunnerRoot)).toBe(true);
  });
});

describe("runDoctor", () => {
  test("prints passing JSON checks in stable order", async () => {
    const { io, stdout } = createIo();
    const forgeRoot = createForgeRoot();
    const evalRunnerRoot = createEvalRunnerRoot();

    const code = await runDoctor(["--json"], {
      io,
      spawn: passingSpawnRunner(),
      env: {
        FORGE_REPO_ROOT: forgeRoot,
        FORGE_EVAL_RUNNER_ROOT: evalRunnerRoot,
      },
    });

    const report = JSON.parse(stdout.join(""));

    expect(code).toBe(0);
    expect(report.ok).toBe(true);
    expect(report.checks.map((check: { name: string }) => check.name)).toEqual([
      "bun",
      "cargo",
      "forge_repo_root",
      "forge_eval_runner",
    ]);
  });

  test("prints failing human output and returns non-zero", async () => {
    const { io, stdout } = createIo();
    const emptyRoot = mkdtempSync(join(tmpdir(), "forge-cli-doctor-empty-"));

    const code = await runDoctor([], {
      io,
      spawn: failingSpawnRunner(),
      env: {
        FORGE_REPO_ROOT: emptyRoot,
        FORGE_EVAL_RUNNER_ROOT: emptyRoot,
      },
    });

    expect(code).toBe(1);
    expect(stdout.join("")).toContain("Forge doctor");
    expect(stdout.join("")).toContain("FAIL");
  });
});
