import { describe, expect, test } from "bun:test";
import { triggerCommand } from "../src/commands/trigger.ts";
import type { SpawnRunner } from "../src/lib/spawn.ts";

describe("triggerCommand", () => {
  test("rejects unknown subcommands", async () => {
    const stderr: string[] = [];
    const code = await triggerCommand(["unknown"], {
      io: {
        stdout: () => {},
        stderr: (text) => stderr.push(text),
      },
    });

    expect(code).toBe(1);
    expect(stderr.join("")).toContain("Usage: forge trigger");
  });

  test("forwards enqueue arguments to forge_trigger", async () => {
    const calls: Parameters<SpawnRunner>[0][] = [];
    const stdout: string[] = [];
    const spawn: SpawnRunner = async (input) => {
      calls.push(input);
      return { exitCode: 0, stdout: "Queued trigger trigger-1\n", stderr: "" };
    };

    const code = await triggerCommand(
      [
        "enqueue",
        "--message",
        "run digest",
        "--profile",
        "ops",
        "--provider",
        "openai",
        "--model",
        "gpt-5",
        "--workspace",
        "/repo/workspace",
      ],
      {
        io: {
          stdout: (text) => stdout.push(text),
          stderr: () => {},
        },
        spawn,
        cwd: "/repo/forge",
      },
    );

    expect(code).toBe(0);
    expect(stdout.join("")).toContain("Queued trigger trigger-1");
    expect(calls).toHaveLength(1);
    expect(calls[0]).toEqual({
      command: "cargo",
      args: [
        "run",
        "--manifest-path",
        "/repo/forge/src-tauri/Cargo.toml",
        "--bin",
        "forge_trigger",
        "--",
        "enqueue",
        "--message",
        "run digest",
        "--profile",
        "ops",
        "--provider",
        "openai",
        "--model",
        "gpt-5",
        "--workspace",
        "/repo/workspace",
      ],
      cwd: "/repo/forge",
    });
  });

  test("accepts status dashboard list runs replay and show subcommands", async () => {
    const validCommands = ["status", "dashboard", "list", "runs", "replay", "show"];

    for (const command of validCommands) {
      const calls: Parameters<SpawnRunner>[0][] = [];
      const spawn: SpawnRunner = async (input) => {
        calls.push(input);
        return { exitCode: 0, stdout: `${command} ok\n`, stderr: "" };
      };

      const args =
        command === "replay" || command === "show"
          ? [command, "--run-id", "run-1"]
          : [command];
      const code = await triggerCommand(args, {
        io: {
          stdout: () => {},
          stderr: () => {},
        },
        spawn,
        cwd: "/repo/forge",
      });

      expect(code).toBe(0);
      expect(calls[0]?.args).toContain("forge_trigger");
      expect(calls[0]?.args).toContain(command);
      if (command === "replay" || command === "show") {
        expect(calls[0]?.args).toContain("--run-id");
        expect(calls[0]?.args).toContain("run-1");
      }
    }
  });

  test("passes through stderr on failure", async () => {
    const stderr: string[] = [];
    const spawn: SpawnRunner = async () => ({
      exitCode: 1,
      stdout: "",
      stderr: "gateway offline\n",
    });

    const code = await triggerCommand(["status"], {
      io: {
        stdout: () => {},
        stderr: (text) => stderr.push(text),
      },
      spawn,
      cwd: "/repo/forge",
    });

    expect(code).toBe(1);
    expect(stderr.join("")).toContain("gateway offline");
  });
});
