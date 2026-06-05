import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export type PathEnv = Record<string, string | undefined>;

export function cliPackageRoot(): string {
  return resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
}

export function defaultForgeRepoRoot(env: PathEnv = process.env): string {
  if (env.FORGE_REPO_ROOT) {
    return resolve(env.FORGE_REPO_ROOT);
  }
  return resolve(cliPackageRoot(), "..");
}

export function defaultEvalRunnerRoot(forgeRepoRoot: string, env: PathEnv = process.env): string {
  if (env.FORGE_EVAL_RUNNER_ROOT) {
    return resolve(env.FORGE_EVAL_RUNNER_ROOT);
  }
  return resolve(forgeRepoRoot, "..", "forge-eval-runner");
}

export function isForgeRepoRoot(path: string): boolean {
  return existsSync(join(path, "src-tauri", "Cargo.toml")) && existsSync(join(path, "package.json"));
}

export function hasEvalRunner(path: string): boolean {
  return existsSync(join(path, "app")) && existsSync(join(path, "eval_cases"));
}
