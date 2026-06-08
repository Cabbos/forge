import { join } from "node:path";
import { bunSpawnRunner, type SpawnRunner } from "./spawn.ts";

export type HeadlessRequest = {
  prompt: string;
  provider: string;
  model: string;
  workspace_path: string;
};

export type BuildHeadlessRequestInput = {
  prompt: string;
  provider: string;
  model: string;
  workspacePath: string;
};

export type HeadlessCommandPlan = {
  command: string;
  args: string[];
};

export type RunHeadlessJsonInput = {
  forgeRepoRoot: string;
  request: HeadlessRequest;
  spawn?: SpawnRunner;
};

export function buildHeadlessRequest(input: BuildHeadlessRequestInput): HeadlessRequest {
  return {
    prompt: input.prompt,
    provider: input.provider,
    model: input.model,
    workspace_path: input.workspacePath,
  };
}

export function buildForgeHeadlessCommand(forgeRepoRoot: string): HeadlessCommandPlan {
  return {
    command: "cargo",
    args: [
      "run",
      "--manifest-path",
      join(forgeRepoRoot, "src-tauri", "Cargo.toml"),
      "--bin",
      "forge_eval_agent",
      "--quiet",
    ],
  };
}

export async function runHeadlessJson<T = unknown>(
  input: RunHeadlessJsonInput,
): Promise<T> {
  const plan = buildForgeHeadlessCommand(input.forgeRepoRoot);
  const spawn = input.spawn ?? bunSpawnRunner;
  const output = await spawn({
    command: plan.command,
    args: plan.args,
    cwd: input.forgeRepoRoot,
    stdin: `${JSON.stringify(input.request)}\n`,
  });

  if (output.exitCode !== 0) {
    const message = output.stderr.trim() || `Forge headless exited with code ${output.exitCode}`;
    throw new Error(message);
  }

  try {
    return JSON.parse(output.stdout) as T;
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Forge headless returned invalid JSON: ${detail}`);
  }
}
