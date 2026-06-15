import { describe, expect, test } from "bun:test";
import { sessionCommand } from "../src/commands/session.ts";
import type { SpawnRunner } from "../src/lib/spawn.ts";

describe("sessionCommand", () => {
  test("passes supported session store subcommands to forge_session", async () => {
    const cases = [
      ["list"],
      ["stats"],
      ["search", "launch"],
      ["export"],
      ["prune", "--keep", "25"],
      ["attach", "session-1"],
      ["show", "session-1"],
    ];

    for (const args of cases) {
      const spawn: SpawnRunner = async (input) => {
        expect(input.args).toContain("forge_session");
        for (const arg of args) expect(input.args).toContain(arg);
        return { exitCode: 0, stdout: `ok ${args[0]}\n`, stderr: "" };
      };
      const stdout: string[] = [];

      const code = await sessionCommand(args, {
        io: {
          stdout: (text) => stdout.push(text),
          stderr: () => {},
        },
        spawn,
        cwd: "/repo/forge",
      });

      expect(code).toBe(0);
      expect(stdout.join("")).toContain(`ok ${args[0]}`);
    }
  });

  test("rejects unsupported session subcommands", async () => {
    const stderr: string[] = [];
    const code = await sessionCommand(["rename"], {
      io: {
        stdout: () => {},
        stderr: (text) => stderr.push(text),
      },
    });

    expect(code).toBe(1);
    expect(stderr.join("")).toContain("Usage: forge session");
  });
});
