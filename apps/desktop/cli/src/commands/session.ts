import { join } from "node:path";
import type { CliDeps } from "../cli.ts";
import { bunSpawnRunner } from "../lib/spawn.ts";

export async function sessionCommand(argv: string[], deps: CliDeps = {}): Promise<number> {
  const sub = argv[0] || "list";
  const supported = new Set(["list", "attach", "show", "stats", "search", "export", "prune"]);
  if (!supported.has(sub)) {
    deps.io?.stderr("Usage: forge session list|attach|show|stats|search|export|prune\n");
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
      "--",
      ...argv,
    ],
    cwd: forgeRepoRoot,
  });

  if (result.stdout) deps.io?.stdout(result.stdout);
  if (result.stderr) deps.io?.stderr(result.stderr);
  return result.exitCode;
}
