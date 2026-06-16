import { describe, expect, test } from "bun:test";
import {
  buildForgeHeadlessCommand,
  buildHeadlessRequest,
  runHeadlessJson,
} from "../src/lib/headless.ts";
import { renderJson, renderRunSummary } from "../src/lib/output.ts";
import type { SpawnRunner } from "../src/lib/spawn.ts";

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

describe("run output helpers", () => {
  test("renders pretty JSON with trailing newline", () => {
    expect(renderJson({ ok: true })).toBe('{\n  "ok": true\n}\n');
  });

  test("renders a compact run summary", () => {
    expect(
      renderRunSummary({
        provider: "forge",
        model: "local-forge",
        changed_files: ["src/a.ts", "src/b.ts"],
        verification_result: { passed: true },
        final_answer: "Applied the change.",
      }),
    ).toBe(
      "Provider: forge\nModel: local-forge\nChanged files: 2\nValidation: passed\nFinal answer: Applied the change.\n",
    );
  });
});
