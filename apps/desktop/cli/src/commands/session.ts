import { join } from "node:path";
import type { CliDeps } from "../cli.ts";
import { bunSpawnRunner } from "../lib/spawn.ts";

export async function sessionCommand(argv: string[], deps: CliDeps = {}): Promise<number> {
  const sub = argv[0] || "list";
  if (sub !== "list") {
    deps.io?.stderr("Usage: forge session list\n");
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
      "forge_session",
    ],
    cwd: forgeRepoRoot,
  });

  if (result.stdout) deps.io?.stdout(result.stdout);
  if (result.stderr) deps.io?.stderr(result.stderr);
  return result.exitCode;
}
