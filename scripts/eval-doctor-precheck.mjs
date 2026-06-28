#!/usr/bin/env node

/**
 * Eval-runner pre-check: runs the CLI doctor in JSON mode and prints
 * a concise diagnostics summary before launching eval-runner pytest.
 *
 * Usage: node scripts/eval-doctor-precheck.mjs
 *
 * Exits 0 if the doctor passes or if the failure is solely due to a
 * fresh install (missing ~/.forge directory, no log file yet).
 * Exits non-zero for hard failures (corrupted config, unreadable files).
 */

import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

// Use process.cwd() since this script is always run from the monorepo root
// (e.g., "node scripts/eval-doctor-precheck.mjs" from the repo root).
const repoRoot = process.cwd();
const cliEntry = resolve(repoRoot, "apps", "desktop", "cli", "src", "index.ts");

export function runDoctor() {
  const result = spawnSync("bun", ["run", cliEntry, "doctor", "--json"], {
    cwd: repoRoot,
    encoding: "utf-8",
    stdio: ["ignore", "pipe", "pipe"],
    timeout: 30_000,
  });
  return {
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
    status: result.status,
  };
}

export function classifyDoctorReport(report) {
  const checks = report?.checks ?? [];
  const failChecks = checks.filter((check) => check.ok === false);
  const softFails = failChecks.filter((check) => isSoftFailure(check));
  const hardFails = failChecks.filter((check) => !isSoftFailure(check));

  return { failChecks, softFails, hardFails };
}

export function exitCodeForDoctorReport(report) {
  return classifyDoctorReport(report).hardFails.length > 0 ? 1 : 0;
}

function isSoftFailure(check) {
  if (check.name !== "forge_logs") return false;
  return /fresh install|does not exist|not yet created/i.test(String(check.message ?? ""));
}

export function main() {
  const { stdout, stderr } = runDoctor();

  if (stderr && !stdout) {
    console.error("eval-doctor-precheck: Doctor exited with no JSON output.");
    console.error(stderr);
    // Keep apps/eval-runner independently runnable if the local CLI runtime is absent.
    return 0;
  }

  let report;
  try {
    report = JSON.parse(stdout.trim() || "{}");
  } catch {
    if (stderr) console.error(stderr);
    console.log("[eval-precheck] Doctor unavailable — proceeding with eval.");
    // Fresh install: bun or CLI path may not be available.
    return 0;
  }

  const { failChecks, softFails, hardFails } = classifyDoctorReport(report);

  if (failChecks.length === 0) {
    console.log("[eval-precheck] Doctor passed — all checks OK.");
    return 0;
  }

  if (softFails.length > 0 && hardFails.length === 0) {
    console.log(
      "[eval-precheck] Fresh install detected (%s) — proceeding with eval.",
      softFails.map((c) => c.name).join(", "),
    );
    return 0;
  }

  if (hardFails.length > 0) {
    console.error("[eval-precheck] Doctor found hard failures:");
    for (const check of hardFails) {
      console.error(`  FAIL ${check.name}: ${check.message}`);
    }
    console.error("[eval-precheck] Fix doctor failures before running eval.");
  }

  if (softFails.length > 0) {
    console.log(
      "[eval-precheck] Soft warnings: %s",
      softFails.map((c) => c.name).join(", "),
    );
  }

  return exitCodeForDoctorReport(report);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = main();
}
