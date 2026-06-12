import type { CliDeps } from "../cli.ts";
import { bunSpawnRunner, type SpawnRunner } from "../lib/spawn.ts";
import {
  defaultEvalRunnerRoot,
  defaultForgeRepoRoot,
  hasEvalRunner,
  isForgeRepoRoot,
} from "../lib/paths.ts";
import { existsSync, readdirSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";

export async function runDoctor(_argv: string[], deps: CliDeps = {}): Promise<number> {
  const argv = _argv;
  const io = deps.io ?? defaultIo;
  const env = { ...process.env, ...deps.env };
  const spawn = deps.spawn ?? bunSpawnRunner;
  const cwd = deps.cwd ?? defaultForgeRepoRoot(env);
  const forgeRepoRoot = defaultForgeRepoRoot(env);
  const evalRunnerRoot = defaultEvalRunnerRoot(forgeRepoRoot, env);
  const homeDir = env.HOME ?? homedir();

  // ── Existing checks (stable order, unchanged) ─────────────────────────
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
    // ── Phase 2: diagnostics mirror checks ───────────────────────────────
    forgeConfigCheck(homeDir),
    forgeAppDataCheck(homeDir),
    forgeSessionsCheck(homeDir),
    forgeLogsCheck(homeDir),
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
  name:
    | "bun"
    | "cargo"
    | "forge_repo_root"
    | "forge_eval_runner"
    | "forge_config"
    | "forge_app_data"
    | "forge_sessions"
    | "forge_logs";
  ok: boolean;
  message: string;
};

type CommandCheckName = "bun" | "cargo";

const defaultIo = {
  stdout: (text: string) => process.stdout.write(text),
  stderr: (text: string) => process.stderr.write(text),
};

// ── Phase 2: file/directory checks (no runtime required) ──────────────────

function forgeConfigCheck(homeDir: string): DoctorCheck {
  const configPath = join(homeDir, ".forge", "config.json");
  if (!existsSync(configPath)) {
    return {
      name: "forge_config",
      ok: true,
      message: `No config at ${configPath} (fresh install).`,
    };
  }
  try {
    const raw = readFileSync(configPath, "utf-8");
    const config = JSON.parse(raw);
    const providers = Object.keys(config.api_keys ?? {});
    const setCount = providers.filter(
      (p: string) => typeof config.api_keys[p] === "string" && config.api_keys[p].length > 0,
    ).length;
    return {
      name: "forge_config",
      ok: true,
      message: `Config readable — ${setCount}/${providers.length} provider(s) have API keys.`,
    };
  } catch (error) {
    return {
      name: "forge_config",
      ok: false,
      message: `Config corrupted: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
}

function forgeAppDataCheck(homeDir: string): DoctorCheck {
  const appStatePath = join(homeDir, ".forge", "app-state.json");
  if (!existsSync(appStatePath)) {
    return {
      name: "forge_app_data",
      ok: true,
      message: "App metadata not present (fresh install).",
    };
  }
  try {
    const raw = readFileSync(appStatePath, "utf-8");
    const metadata = JSON.parse(raw);
    const workspaceCount = Array.isArray(metadata.workspaces) ? metadata.workspaces.length : 0;
    return {
      name: "forge_app_data",
      ok: true,
      message: `App metadata readable — ${workspaceCount} workspace(s).`,
    };
  } catch (error) {
    return {
      name: "forge_app_data",
      ok: false,
      message: `App metadata corrupted: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
}

function forgeSessionsCheck(homeDir: string): DoctorCheck {
  const sessionsDir = join(homeDir, ".forge", "sessions");
  if (!existsSync(sessionsDir)) {
    return {
      name: "forge_sessions",
      ok: true,
      message: "No session snapshots (fresh install).",
    };
  }
  try {
    const entries = readdirSync(sessionsDir, { withFileTypes: true });
    const jsonFiles = entries.filter(
      (e) => e.isFile() && e.name.endsWith(".json"),
    );
    const total = jsonFiles.length;

    let corruptCount = 0;
    for (const file of jsonFiles) {
      try {
        JSON.parse(readFileSync(join(sessionsDir, file.name), "utf-8"));
      } catch {
        corruptCount++;
      }
    }
    const readableCount = total - corruptCount;

    if (total === 0) {
      return {
        name: "forge_sessions",
        ok: true,
        message: "No session snapshots found.",
      };
    }
    if (corruptCount > 0 && readableCount === 0) {
      return {
        name: "forge_sessions",
        ok: false,
        message: `All ${total} snapshot(s) corrupted.`,
      };
    }
    if (corruptCount > 0) {
      return {
        name: "forge_sessions",
        ok: false,
        message: `${readableCount} readable snapshot(s), ${corruptCount} corrupted.`,
      };
    }
    return {
      name: "forge_sessions",
      ok: true,
      message: `${readableCount} session snapshot(s) readable.`,
    };
  } catch (error) {
    return {
      name: "forge_sessions",
      ok: false,
      message: `Cannot read snapshots: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
}

function forgeLogsCheck(homeDir: string): DoctorCheck {
  const forgeDir = join(homeDir, ".forge");
  const logPath = join(forgeDir, "app.log");

  if (!existsSync(forgeDir)) {
    return {
      name: "forge_logs",
      ok: false,
      message: `Data directory ${forgeDir} does not exist.`,
    };
  }

  const dataDirWritable = (() => {
    try {
      const testPath = join(forgeDir, `.doctor_write_test-${process.pid}`);
      writeFileSync(testPath, "ok");
      unlinkSync(testPath);
      return true;
    } catch {
      return false;
    }
  })();

  if (!dataDirWritable) {
    return {
      name: "forge_logs",
      ok: false,
      message: `Data directory ${forgeDir} is not writable.`,
    };
  }

  if (!existsSync(logPath)) {
    return {
      name: "forge_logs",
      ok: true,
      message: "Log file not yet created (no sessions have run).",
    };
  }

  return {
    name: "forge_logs",
    ok: true,
    message: `Log file present at ${logPath}.`,
  };
}

// ── Helpers ────────────────────────────────────────────────────────────────

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
