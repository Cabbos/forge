import { describe, expect, test } from "bun:test";
import { runCli } from "../src/cli.ts";

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

describe("runCli", () => {
  test("prints help when no command is provided", async () => {
    const { io, stdout } = createIo();

    const code = await runCli([], { io });

    expect(code).toBe(0);
    expect(stdout.join("")).toContain("Usage: forge <command>");
    expect(stdout.join("")).toContain("doctor");
    expect(stdout.join("")).toContain("run");
  });

  test("returns non-zero for unknown commands", async () => {
    const { io, stderr } = createIo();

    const code = await runCli(["unknown"], { io });

    expect(code).toBe(1);
    expect(stderr.join("")).toContain("Unknown command: unknown");
  });
});
