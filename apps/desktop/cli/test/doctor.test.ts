import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, test } from "bun:test";
import { runDoctor } from "../src/commands/doctor.ts";
import {
  defaultEvalRunnerRoot,
  defaultForgeRepoRoot,
  hasEvalRunner,
  isForgeRepoRoot,
} from "../src/lib/paths.ts";
import type { SpawnRunner } from "../src/lib/spawn.ts";

const tempDirs: string[] = [];

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    rmSync(dir, { recursive: true, force: true });
  }
});

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

function createTempDir(prefix: string) {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  tempDirs.push(dir);
  return dir;
}

function createForgeRoot() {
  const forgeRoot = createTempDir("forge-cli-doctor-forge-");
  mkdirSync(join(forgeRoot, "src-tauri"), { recursive: true });
  writeFileSync(join(forgeRoot, "src-tauri", "Cargo.toml"), "[package]\nname = \"forge-test\"\n");
  writeFileSync(join(forgeRoot, "package.json"), "{}\n");
  return forgeRoot;
}

function createEvalRunnerRoot() {
  const evalRunnerRoot = createTempDir("forge-cli-doctor-eval-");
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

  test("FORGE_EVAL_RUNNER_PATH takes priority over FORGE_EVAL_RUNNER_ROOT", () => {
    const forgeRoot = createForgeRoot();
    const pathDir = createEvalRunnerRoot();
    const rootDir = createEvalRunnerRoot();

    expect(
      defaultEvalRunnerRoot(forgeRoot, {
        FORGE_EVAL_RUNNER_PATH: pathDir,
        FORGE_EVAL_RUNNER_ROOT: rootDir,
      }),
    ).toBe(pathDir);
  });

  test("defaults to sibling eval-runner in monorepo", () => {
    const forgeRoot = createForgeRoot();
    const expected = resolve(forgeRoot, "..", "eval-runner");

    expect(defaultEvalRunnerRoot(forgeRoot, {})).toBe(expected);
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

  test("passes command checks through the injected spawn runner", async () => {
    const { io } = createIo();
    const forgeRoot = createForgeRoot();
    const evalRunnerRoot = createEvalRunnerRoot();
    const cwd = createTempDir("forge-cli-doctor-cwd-");
    const calls: Parameters<SpawnRunner>[0][] = [];
    const previousParentEnv = process.env.FORGE_DOCTOR_TEST_PARENT;
    const spawn: SpawnRunner = async (input) => {
      calls.push(input);
      return { exitCode: 0, stdout: `${input.command} 1.0.0\n`, stderr: "" };
    };

    process.env.FORGE_DOCTOR_TEST_PARENT = "parent";
    try {
      const code = await runDoctor(["--json"], {
        io,
        spawn,
        cwd,
        env: {
          FORGE_REPO_ROOT: forgeRoot,
          FORGE_EVAL_RUNNER_ROOT: evalRunnerRoot,
          FORGE_DOCTOR_TEST_CHILD: "child",
        },
      });

      expect(code).toBe(0);
      expect(calls.map((call) => [call.command, call.args])).toEqual([
        ["bun", ["--version"]],
        ["cargo", ["--version"]],
      ]);
      expect(calls.every((call) => call.cwd === cwd)).toBe(true);
      expect(calls.every((call) => call.env?.FORGE_DOCTOR_TEST_PARENT === "parent")).toBe(true);
      expect(calls.every((call) => call.env?.FORGE_DOCTOR_TEST_CHILD === "child")).toBe(true);
    } finally {
      if (previousParentEnv === undefined) {
        delete process.env.FORGE_DOCTOR_TEST_PARENT;
      } else {
        process.env.FORGE_DOCTOR_TEST_PARENT = previousParentEnv;
      }
    }
  });

  test("prints failing human output and returns non-zero", async () => {
    const { io, stdout } = createIo();
    const emptyRoot = createTempDir("forge-cli-doctor-empty-");

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
