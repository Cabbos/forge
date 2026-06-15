import { runDoctor } from "./commands/doctor.ts";
import { runCommand } from "./commands/run.ts";
import { serviceCommand } from "./commands/service.ts";
import { sessionCommand } from "./commands/session.ts";
import { triggerCommand } from "./commands/trigger.ts";
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
  const commandDeps: CliDeps = { ...deps, io };
  const [command, ...rest] = argv;

  if (!command || command === "--help" || command === "-h") {
    io.stdout(helpText());
    return 0;
  }

  if (command === "doctor") {
    return runDoctor(rest, commandDeps);
  }

  if (command === "run") {
    return runCommand(rest, commandDeps);
  }

  if (command === "service") {
    return serviceCommand(rest, commandDeps);
  }

  if (command === "session") {
    return sessionCommand(rest, commandDeps);
  }

  if (command === "trigger") {
    return triggerCommand(rest, commandDeps);
  }

  io.stderr(`Unknown command: ${command}\n\n${helpText()}`);
  return 1;
}

export function helpText(): string {
  return [
    "Usage: forge <command> [options]",
    "",
    "Commands:",
    "  doctor          Check local Forge CLI readiness",
    "  run             Run one prompt through Forge headless",
    "  service         Manage Forge gateway service (install|uninstall|start|stop|restart|status)",
    "  session         Inspect gateway sessions and local session store",
    "  trigger         Enqueue and inspect gateway triggers",
    "",
    "Run options:",
    "  --profile, -p <id>    Use named profile for provider/model/workspace",
    "  --provider <name>     AI provider (default: deepseek)",
    "  --model, -m <name>    Model name (default: deepseek-chat)",
    "  --workspace, -w <path> Working directory",
    "  --prompt <text>       The prompt to send",
    "",
  ].join("\n");
}
