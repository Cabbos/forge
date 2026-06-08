import type { CliDeps } from "../cli.ts";
import { bunSpawnRunner, type SpawnRunner } from "../lib/spawn.ts";
import {
  defaultEvalRunnerRoot,
  defaultForgeRepoRoot,
  hasEvalRunner,
  isForgeRepoRoot,
} from "../lib/paths.ts";

export async function runDoctor(_argv: string[], deps: CliDeps = {}): Promise<number> {
  const argv = _argv;
  const io = deps.io ?? defaultIo;
  const env = { ...process.env, ...deps.env };
  const spawn = deps.spawn ?? bunSpawnRunner;
  const cwd = deps.cwd ?? defaultForgeRepoRoot(env);
  const forgeRepoRoot = defaultForgeRepoRoot(env);
  const evalRunnerRoot = defaultEvalRunnerRoot(forgeRepoRoot, env);
  const checks: DoctorCheck[] = [
    await commandCheck("bun", "bun", spawn, cwd, env),
    await commandCheck("cargo", "cargo", spawn, cwd, env),
    {
      name: "forge_repo_root",
      ok: isForgeRepoRoot(forgeRepoRoot),
      message: forgeRepoRoot,
    },
    {
      name: "forge_eval_runner",
      ok: hasEvalRunner(evalRunnerRoot),
      message: evalRunnerRoot,
    },
  ];
  const ok = checks.every((check) => check.ok);

  if (argv.includes("--json")) {
    io.stdout(`${JSON.stringify({ ok, checks }, null, 2)}\n`);
  } else {
    io.stdout(formatHumanReport(checks));
  }

  return ok ? 0 : 1;
}

export type DoctorCheck = {
  name: "bun" | "cargo" | "forge_repo_root" | "forge_eval_runner";
  ok: boolean;
  message: string;
};

type CommandCheckName = "bun" | "cargo";

const defaultIo = {
  stdout: (text: string) => process.stdout.write(text),
  stderr: (text: string) => process.stderr.write(text),
};

async function commandCheck(
  name: CommandCheckName,
  command: string,
  spawn: SpawnRunner,
  cwd: string,
  env: Record<string, string | undefined>,
): Promise<DoctorCheck> {
  try {
    const result = await spawn({ command, args: ["--version"], cwd, env });
    return {
      name,
      ok: result.exitCode === 0,
      message: firstLine(result.stdout) || firstLine(result.stderr) || `exit ${result.exitCode}`,
    };
  } catch (error) {
    return {
      name,
      ok: false,
      message: error instanceof Error ? error.message : String(error),
    };
  }
}

function formatHumanReport(checks: DoctorCheck[]): string {
  const lines = ["Forge doctor", ""];
  for (const check of checks) {
    lines.push(`${check.ok ? "PASS" : "FAIL"} ${check.name} ${check.message}`);
  }
  lines.push("");
  return lines.join("\n");
}

function firstLine(text: string): string {
  return text.trim().split("\n")[0] ?? "";
}
