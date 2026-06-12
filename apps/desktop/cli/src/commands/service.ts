import { join } from "node:path";
import type { CliDeps } from "../cli.ts";
import { bunSpawnRunner } from "../lib/spawn.ts";

const VALID_COMMANDS = ["install", "uninstall", "start", "stop", "restart", "status"] as const;

export async function serviceCommand(argv: string[], deps: CliDeps = {}): Promise<number> {
  const sub = argv[0];
  if (!sub || !(VALID_COMMANDS as readonly string[]).includes(sub)) {
    deps.io?.stderr(
      `Usage: forge service <${VALID_COMMANDS.join("|")}>\n`,
    );
    return 1;
  }

  const forgeRepoRoot = deps.cwd || process.cwd();
  const spawn = deps.spawn ?? bunSpawnRunner;

  const result = await spawn({
    command: "cargo",
    args: [
      "run",
      "--manifest-path",
      join(forgeRepoRoot, "src-tauri", "Cargo.toml"),
      "--bin",
      "forge_service",
      "--",
      sub,
    ],
    cwd: forgeRepoRoot,
  });

  if (result.stdout) {
    deps.io?.stdout(result.stdout);
  }
  if (result.stderr) {
    deps.io?.stderr(result.stderr);
  }

  return result.exitCode;
}
