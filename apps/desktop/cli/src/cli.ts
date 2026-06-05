import { runDoctor } from "./commands/doctor.ts";
import { runCommand } from "./commands/run.ts";
import type { SpawnRunner } from "./lib/spawn.ts";

export type CliIo = {
  stdout: (text: string) => void;
  stderr: (text: string) => void;
};

export type CliDeps = {
  io?: CliIo;
  spawn?: SpawnRunner;
  cwd?: string;
  env?: Record<string, string | undefined>;
};

const defaultIo: CliIo = {
  stdout: (text) => process.stdout.write(text),
  stderr: (text) => process.stderr.write(text),
};

export async function runCli(argv: string[], deps: CliDeps = {}): Promise<number> {
  const io = deps.io ?? defaultIo;
  const [command, ...rest] = argv;

  if (!command || command === "--help" || command === "-h") {
    io.stdout(helpText());
    return 0;
  }

  if (command === "doctor") {
    return runDoctor(rest, deps);
  }

  if (command === "run") {
    return runCommand(rest, deps);
  }

  io.stderr(`Unknown command: ${command}\n\n${helpText()}`);
  return 1;
}

export function helpText(): string {
  return [
    "Usage: forge <command> [options]",
    "",
    "Commands:",
    "  doctor        Check local Forge CLI readiness",
    "  run           Run one prompt through Forge headless",
    "",
  ].join("\n");
}
