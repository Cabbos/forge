import { describe, expect, test } from "bun:test";
import {
  buildForgeHeadlessCommand,
  buildHeadlessRequest,
  runHeadlessJson,
} from "../src/lib/headless.ts";
import { renderJson, renderRunSummary } from "../src/lib/output.ts";
import type { SpawnRunner } from "../src/lib/spawn.ts";
import { parseRunArgs, runCommand } from "../src/commands/run.ts";

describe("headless helpers", () => {
  test("builds the Forge headless cargo command", () => {
    expect(buildForgeHeadlessCommand("/repo/forge")).toEqual({
      command: "cargo",
      args: [
        "run",
        "--manifest-path",
        "/repo/forge/src-tauri/Cargo.toml",
        "--bin",
        "forge_eval_agent",
        "--quiet",
      ],
    });
  });

  test("builds the Forge headless JSON request", () => {
    const prompt = "make the smallest useful change";
    const provider = "forge";
    const model = "local-forge";
    const workspacePath = "/repo/workspace";

    expect(buildHeadlessRequest({ prompt, provider, model, workspacePath })).toEqual({
      prompt,
      provider,
      model,
      workspace_path: workspacePath,
    });
  });

  test("builds headless request with optional profile_id", () => {
    const request = buildHeadlessRequest({
      prompt: "test",
      provider: "forge",
      model: "local-forge",
      workspacePath: "/repo",
      profileId: "my-profile",
    });
    expect(request.profile_id).toBe("my-profile");
    // Verify it serializes properly
    const json = JSON.stringify(request);
    expect(json).toContain('"profile_id":"my-profile"');
  });

  test("runs headless through injected spawn, writes JSON stdin, and parses JSON stdout", async () => {
    const calls: Parameters<SpawnRunner>[0][] = [];
    const request = buildHeadlessRequest({
      prompt: "ship a tiny fix",
      provider: "forge",
      model: "local-forge",
      workspacePath: "/repo/workspace",
    });
    const spawn: SpawnRunner = async (input) => {
      calls.push(input);
      return {
        exitCode: 0,
        stdout: JSON.stringify({ ok: true, final_answer: "done" }),
        stderr: "",
      };
    };

    const result = await runHeadlessJson({ forgeRepoRoot: "/repo/forge", request, spawn });

    expect(result).toEqual({ ok: true, final_answer: "done" });
    expect(calls).toHaveLength(1);
    expect(calls[0]).toEqual({
      command: "cargo",
      args: [
        "run",
        "--manifest-path",
        "/repo/forge/src-tauri/Cargo.toml",
        "--bin",
        "forge_eval_agent",
        "--quiet",
      ],
      cwd: "/repo/forge",
      stdin: `${JSON.stringify(request)}\n`,
    });
    expect(calls[0]?.stdin).toContain("ship a tiny fix");
  });

  test("throws stderr when headless exits non-zero", async () => {
    const request = buildHeadlessRequest({
      prompt: "fail please",
      provider: "forge",
      model: "local-forge",
      workspacePath: "/repo/workspace",
    });
    const spawn: SpawnRunner = async () => ({
      exitCode: 2,
      stdout: "",
      stderr: "provider failed\n",
    });

    await expect(runHeadlessJson({ forgeRepoRoot: "/repo/forge", request, spawn })).rejects.toThrow(
      "provider failed",
    );
  });

  test("throws an exit-code message when non-zero headless has no stderr", async () => {
    const request = buildHeadlessRequest({
      prompt: "fail silently",
      provider: "forge",
      model: "local-forge",
      workspacePath: "/repo/workspace",
    });
    const spawn: SpawnRunner = async () => ({
      exitCode: 3,
      stdout: "",
      stderr: "",
    });

    await expect(runHeadlessJson({ forgeRepoRoot: "/repo/forge", request, spawn })).rejects.toThrow(
      "exited with code 3",
    );
  });

  test("throws a useful message for invalid JSON stdout", async () => {
    const request = buildHeadlessRequest({
      prompt: "bad json",
      provider: "forge",
      model: "local-forge",
      workspacePath: "/repo/workspace",
    });
    const spawn: SpawnRunner = async () => ({
      exitCode: 0,
      stdout: "not-json",
      stderr: "",
    });

    await expect(runHeadlessJson({ forgeRepoRoot: "/repo/forge", request, spawn })).rejects.toThrow(
      "invalid JSON",
    );
  });
});

// ── parseRunArgs ─────────────────────────────────────────────────────────────

describe("parseRunArgs", () => {
  test("parses --profile flag with long form", () => {
    const result = parseRunArgs(["--profile", "work", "--prompt", "hello"]);
    expect(result.profileId).toBe("work");
    expect(result.prompt).toBe("hello");
  });

  test("parses -p short flag", () => {
    const result = parseRunArgs(["-p", "work", "--prompt", "hello"]);
    expect(result.profileId).toBe("work");
    expect(result.prompt).toBe("hello");
  });

  test("parses positional prompt without --prompt flag", () => {
    const result = parseRunArgs(["--profile", "work", "do something"]);
    expect(result.profileId).toBe("work");
    expect(result.prompt).toBe("do something");
  });

  test("rejects --profile with missing value (end of args)", () => {
    expect(() => parseRunArgs(["--profile"])).toThrow(
      "--profile requires a value",
    );
  });

  test("rejects --profile with next arg being another flag", () => {
    expect(() => parseRunArgs(["--profile", "--prompt", "hello"])).toThrow(
      "--profile requires a value",
    );
  });

  test("rejects -p with missing value", () => {
    expect(() => parseRunArgs(["-p"])).toThrow(
      "--profile requires a value",
    );
  });

  test("rejects empty prompt", () => {
    expect(() => parseRunArgs([])).toThrow("prompt is required");
    expect(() => parseRunArgs(["--profile", "w"])).toThrow("prompt is required");
    expect(() => parseRunArgs(["--prompt", "  "])).toThrow("prompt is required");
  });

  test("parses --provider and --model alongside --profile", () => {
    const result = parseRunArgs([
      "--profile", "work",
      "--provider", "anthropic",
      "--model", "claude-opus-4-8",
      "--prompt", "hello",
    ]);
    expect(result.profileId).toBe("work");
    expect(result.provider).toBe("anthropic");
    expect(result.model).toBe("claude-opus-4-8");
    expect(result.prompt).toBe("hello");
  });

  test("parses -m short flag for model", () => {
    const result = parseRunArgs(["-p", "w", "-m", "gpt-5", "--prompt", "x"]);
    expect(result.profileId).toBe("w");
    expect(result.model).toBe("gpt-5");
  });

  test("parses --workspace / -w flag", () => {
    const result = parseRunArgs(["--workspace", "/tmp/proj", "--prompt", "x"]);
    expect(result.workspacePath).toBe("/tmp/proj");
  });

  test("profileId is undefined when --profile not provided", () => {
    const result = parseRunArgs(["--prompt", "hello"]);
    expect(result.profileId).toBeUndefined();
  });

  test("provider and model default to undefined when not provided", () => {
    const result = parseRunArgs(["--prompt", "hello"]);
    expect(result.provider).toBeUndefined();
    expect(result.model).toBeUndefined();
  });

  test("first positional arg is prompt when no --prompt flag", () => {
    const result = parseRunArgs(["ship the fix"]);
    expect(result.prompt).toBe("ship the fix");
  });

  test("skips unknown flags without crashing", () => {
    const result = parseRunArgs(["--unknown-flag", "value", "--prompt", "hello"]);
    expect(result.prompt).toBe("hello");
  });
});

// ── runCommand integration ────────────────────────────────────────────────────

describe("runCommand", () => {
  test("returns 0 and prints JSON on successful headless run", async () => {
    const stdout: string[] = [];
    const stderr: string[] = [];
    const spawn: SpawnRunner = async () => ({
      exitCode: 0,
      stdout: JSON.stringify({ ok: true }),
      stderr: "",
    });

    const exitCode = await runCommand(
      ["--profile", "work", "--prompt", "ship it"],
      {
        io: {
          stdout: (t) => stdout.push(t),
          stderr: (t) => stderr.push(t),
        },
        spawn,
        cwd: "/repo/forge",
      },
    );

    expect(exitCode).toBe(0);
    expect(stderr).toHaveLength(0);
    const combined = stdout.join("");
    expect(combined).toContain("Starting headless run with profile: work");
    expect(combined).toContain('"ok": true');
  });

  test("returns 1 and prints error on parse failure", async () => {
    const stderr: string[] = [];
    const exitCode = await runCommand([], {
      io: {
        stdout: () => {},
        stderr: (t) => stderr.push(t),
      },
    });

    expect(exitCode).toBe(1);
    expect(stderr.join("")).toContain("prompt is required");
  });

  test("returns 1 and prints error when profile value is missing", async () => {
    const stderr: string[] = [];
    const exitCode = await runCommand(["--profile", "--prompt", "hello"], {
      io: {
        stdout: () => {},
        stderr: (t) => stderr.push(t),
      },
    });

    expect(exitCode).toBe(1);
    expect(stderr.join("")).toContain("--profile requires a value");
  });

  test("runs without --profile flag (profileId undefined)", async () => {
    const stdout: string[] = [];
    const spawn: SpawnRunner = async () => ({
      exitCode: 0,
      stdout: JSON.stringify({ ok: true }),
      stderr: "",
    });

    const exitCode = await runCommand(["--prompt", "hello"], {
      io: {
        stdout: (t) => stdout.push(t),
        stderr: () => {},
      },
      spawn,
      cwd: "/repo/forge",
    });

    expect(exitCode).toBe(0);
    expect(stdout.join("")).toContain("profile: (none)");
  });
});
