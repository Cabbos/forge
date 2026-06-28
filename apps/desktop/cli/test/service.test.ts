import { describe, expect, test } from "bun:test";
import { serviceCommand } from "../src/commands/service.ts";
import type { SpawnRunner } from "../src/lib/spawn.ts";

describe("serviceCommand", () => {
  test("rejects unknown subcommand", async () => {
    const stderr: string[] = [];
    const code = await serviceCommand(["unknown"], {
      io: {
        stdout: () => {},
        stderr: (t) => stderr.push(t),
      },
    });
    expect(code).toBe(1);
    expect(stderr.join("")).toContain("Usage: forge service");
  });

  test("rejects empty subcommand", async () => {
    const stderr: string[] = [];
    const code = await serviceCommand([], {
      io: {
        stdout: () => {},
        stderr: (t) => stderr.push(t),
      },
    });
    expect(code).toBe(1);
    expect(stderr.join("")).toContain("Usage: forge service");
  });

  test("accepts all valid subcommands", async () => {
    const validCommands = ["install", "uninstall", "start", "stop", "restart", "status"];

    for (const cmd of validCommands) {
      const stdout: string[] = [];
      const spawn: SpawnRunner = async (input) => {
        expect(input.args).toContain("forge_service");
        expect(input.args).toContain(cmd);
        return { exitCode: 0, stdout: `ok: ${cmd}`, stderr: "" };
      };

      const code = await serviceCommand([cmd], {
        io: {
          stdout: (t) => stdout.push(t),
          stderr: () => {},
        },
        spawn,
        cwd: "/repo/forge",
      });

      expect(code).toBe(0);
      expect(stdout.join("")).toContain(`ok: ${cmd}`);
    }
  });

  test("passes through stderr on failure", async () => {
    const stderr: string[] = [];
    const spawn: SpawnRunner = async () => ({
      exitCode: 1,
      stdout: "",
      stderr: "something went wrong\n",
    });

    const code = await serviceCommand(["install"], {
      io: {
        stdout: () => {},
        stderr: (t) => stderr.push(t),
      },
      spawn,
      cwd: "/repo/forge",
    });

    expect(code).toBe(1);
    expect(stderr.join("")).toContain("something went wrong");
  });
});
