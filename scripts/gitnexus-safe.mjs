#!/usr/bin/env node
import { spawn } from "node:child_process";
import process from "node:process";

export const DEFAULT_TIMEOUT_MS = 60_000;
export const TIMEOUT_EXIT_CODE = 124;

export function fallbackReportTemplate() {
  return [
    "GitNexus fallback impact report",
    "",
    "- Command attempted:",
    "- Timeout or error:",
    "- Index freshness checked:",
    "- Symbols searched:",
    "- Files inspected:",
    "- Direct callers found:",
    "- Tests selected:",
    "- Affected authority domains:",
    "- Residual risk:",
  ].join("\n");
}

export function fallbackInstructions({ commandText, reason }) {
  return [
    `GitNexus safe wrapper stopped: ${reason}.`,
    commandText ? `Command: ${commandText}` : null,
    "",
    "Use this fallback only after a GitNexus command or tool is unavailable, stale, or timed out.",
    fallbackReportTemplate(),
    "",
    "Index refresh hint:",
    "pnpm --allow-build=@ladybugdb/core --allow-build=gitnexus --allow-build=tree-sitter --allow-build=tree-sitter-kotlin dlx gitnexus@latest analyze --index-only",
  ]
    .filter(Boolean)
    .join("\n");
}

export function parseArgs(argv) {
  let timeoutMs = DEFAULT_TIMEOUT_MS;
  let printTemplate = false;
  const command = [];

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--timeout-ms") {
      const rawTimeout = argv[index + 1];
      if (!rawTimeout) {
        throw new Error("Missing value for --timeout-ms");
      }
      const parsed = Number(rawTimeout);
      if (!Number.isInteger(parsed) || parsed <= 0) {
        throw new Error(`Invalid --timeout-ms value: ${rawTimeout}`);
      }
      timeoutMs = parsed;
      index += 1;
      continue;
    }
    if (arg === "--print-template") {
      printTemplate = true;
      continue;
    }
    if (arg === "-h" || arg === "--help") {
      return { help: true, timeoutMs, printTemplate, command: [] };
    }
    if (arg === "--") {
      command.push(...argv.slice(index + 1));
      break;
    }
    command.push(arg);
  }

  return { help: false, timeoutMs, printTemplate, command };
}

export function usage() {
  return [
    "Usage:",
    "  node scripts/gitnexus-safe.mjs [--timeout-ms <ms>] -- <gitnexus command...>",
    "  node scripts/gitnexus-safe.mjs --print-template",
    "",
    "Default timeout: 60000 ms.",
  ].join("\n");
}

export function runCommandWithTimeout({ command, timeoutMs = DEFAULT_TIMEOUT_MS, cwd = process.cwd() }) {
  return new Promise((resolve) => {
    const [program, ...args] = command;
    const commandText = command.join(" ");
    const child = spawn(program, args, {
      cwd,
      env: process.env,
      stdio: "inherit",
    });
    let settled = false;
    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      child.kill("SIGTERM");
      process.stderr.write(
        `${fallbackInstructions({
          commandText,
          reason: `timed out after ${timeoutMs} ms`,
        })}\n`,
      );
      resolve(TIMEOUT_EXIT_CODE);
    }, timeoutMs);

    child.on("error", (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      process.stderr.write(
        `${fallbackInstructions({
          commandText,
          reason: error.message,
        })}\n`,
      );
      resolve(1);
    });

    child.on("exit", (code, signal) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      const exitCode = code ?? (signal ? 1 : 0);
      if (exitCode !== 0) {
        process.stderr.write(
          `${fallbackInstructions({
            commandText,
            reason: signal ? `terminated by ${signal}` : `exited with code ${exitCode}`,
          })}\n`,
        );
      }
      resolve(exitCode);
    });
  });
}

async function runCli() {
  let parsed;
  try {
    parsed = parseArgs(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${error.message}\n${usage()}\n`);
    return 2;
  }

  if (parsed.help) {
    process.stdout.write(`${usage()}\n`);
    return 0;
  }
  if (parsed.printTemplate) {
    process.stdout.write(`${fallbackReportTemplate()}\n`);
    return 0;
  }
  if (parsed.command.length === 0) {
    process.stderr.write(`Missing GitNexus command.\n${usage()}\n\n${fallbackReportTemplate()}\n`);
    return 2;
  }

  return runCommandWithTimeout({
    command: parsed.command,
    timeoutMs: parsed.timeoutMs,
  });
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const exitCode = await runCli();
  process.exitCode = exitCode;
}
