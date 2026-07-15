import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

import {
  DEFAULT_TIMEOUT_MS,
  TIMEOUT_EXIT_CODE,
  fallbackReportTemplate,
  parseArgs,
} from "./gitnexus-safe.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "gitnexus-safe.mjs");

test("fallback template includes required report fields", () => {
  const template = fallbackReportTemplate();

  assert.match(template, /Command attempted:/);
  assert.match(template, /Timeout or error:/);
  assert.match(template, /Symbols searched:/);
  assert.match(template, /Files inspected:/);
  assert.match(template, /Direct callers found:/);
  assert.match(template, /Tests selected:/);
  assert.match(template, /Residual risk:/);
});

test("parseArgs keeps a default 60 second timeout", () => {
  assert.deepEqual(parseArgs(["--", "gitnexus", "analyze"]), {
    help: false,
    timeoutMs: DEFAULT_TIMEOUT_MS,
    printTemplate: false,
    command: ["gitnexus", "analyze"],
  });
});

test("parseArgs accepts an explicit timeout", () => {
  assert.deepEqual(parseArgs(["--timeout-ms", "250", "--", "gitnexus", "status"]), {
    help: false,
    timeoutMs: 250,
    printTemplate: false,
    command: ["gitnexus", "status"],
  });
});

test("print-template mode is available without a command", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/gitnexus-safe.mjs should exist");

  const result = spawnSync(process.execPath, [scriptPath, "--print-template"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 0);
  assert.match(result.stdout, /GitNexus fallback impact report/);
  assert.match(result.stdout, /Affected authority domains:/);
});

test("successful commands pass through stdout", () => {
  const result = spawnSync(
    process.execPath,
    [scriptPath, "--", process.execPath, "-e", "console.log('gitnexus-ok')"],
    {
      cwd: root,
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0);
  assert.match(result.stdout, /gitnexus-ok/);
  assert.equal(result.stderr, "");
});

test("timed out commands exit 124 and print fallback instructions", () => {
  const result = spawnSync(
    process.execPath,
    [
      scriptPath,
      "--timeout-ms",
      "50",
      "--",
      process.execPath,
      "-e",
      "setTimeout(() => {}, 1000)",
    ],
    {
      cwd: root,
      encoding: "utf8",
    },
  );

  assert.equal(result.status, TIMEOUT_EXIT_CODE);
  assert.match(result.stderr, /timed out after 50 ms/);
  assert.match(result.stderr, /GitNexus fallback impact report/);
  assert.match(result.stderr, /pnpm --allow-build=@ladybugdb\/core/);
});

test("failing commands print fallback instructions", () => {
  const result = spawnSync(
    process.execPath,
    [scriptPath, "--", process.execPath, "-e", "process.exit(7)"],
    {
      cwd: root,
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 7);
  assert.match(result.stderr, /exited with code 7/);
  assert.match(result.stderr, /Symbols searched:/);
});
