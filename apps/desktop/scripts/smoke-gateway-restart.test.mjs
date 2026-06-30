import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

import { buildGatewayRestartPlan } from "./smoke-gateway-restart.mjs";

const desktopRoot = new URL("..", import.meta.url).pathname.replace(/\/$/, "");
const repoRoot = new URL("../../..", import.meta.url).pathname.replace(/\/$/, "");

test("buildGatewayRestartPlan points at isolated restart smoke stores", () => {
  const root = "/tmp/forge-gateway-restart";
  const plan = buildGatewayRestartPlan({ root });

  assert.equal(plan.home, join(root, "home"));
  assert.equal(plan.triggerStorePath, join(root, "home", ".forge", "triggers.json"));
  assert.equal(plan.runStorePath, join(root, "home", ".forge", "trigger-runs.json"));
  assert.match(plan.gatewayCommand.join(" "), /cargo run .*--bin gateway/);
  assert.deepEqual(plan.gatewayCommand, [
    "cargo",
    "run",
    "--manifest-path",
    join(repoRoot, "apps", "desktop", "src-tauri", "Cargo.toml"),
    "--bin",
    "gateway",
    "--quiet",
  ]);
});

test("dry-run json prints the restart plan and exits successfully", () => {
  const result = spawnSync(process.execPath, ["scripts/smoke-gateway-restart.mjs", "--json", "--dry-run"], {
    cwd: desktopRoot,
    encoding: "utf8",
  });

  assert.equal(result.status, 0);
  assert.equal(result.stderr, "");
  const payload = JSON.parse(result.stdout);
  assert.equal(payload.ok, true);
  assert.equal(payload.dryRun, true);
  assert.equal(payload.plan.home.endsWith("/home"), true);
  assert.equal(payload.plan.triggerStorePath.endsWith("/home/.forge/triggers.json"), true);
  assert.equal(payload.plan.runStorePath.endsWith("/home/.forge/trigger-runs.json"), true);
});

test("default dry-run roots are fresh for each invocation", () => {
  const first = spawnSync(process.execPath, ["scripts/smoke-gateway-restart.mjs", "--json", "--dry-run"], {
    cwd: desktopRoot,
    encoding: "utf8",
  });
  const second = spawnSync(process.execPath, ["scripts/smoke-gateway-restart.mjs", "--json", "--dry-run"], {
    cwd: desktopRoot,
    encoding: "utf8",
  });

  assert.equal(first.status, 0);
  assert.equal(second.status, 0);
  assert.notEqual(JSON.parse(first.stdout).plan.root, JSON.parse(second.stdout).plan.root);
});

test("json without dry-run refuses live restart execution", () => {
  const result = spawnSync(process.execPath, ["scripts/smoke-gateway-restart.mjs", "--json"], {
    cwd: desktopRoot,
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /Live gateway restart smoke is not implemented/);
  assert.equal(result.stdout, "");
});

test("live mode refuses to claim restart execution", () => {
  const result = spawnSync(process.execPath, ["scripts/smoke-gateway-restart.mjs"], {
    cwd: desktopRoot,
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /Live gateway restart smoke is not implemented/);
  assert.doesNotMatch(result.stdout, /restarted/i);
});

test("package.json exposes the gateway restart smoke command", () => {
  const pkg = JSON.parse(readFileSync(join(desktopRoot, "package.json"), "utf8"));

  assert.equal(
    pkg.scripts["smoke:gateway:restart"],
    "node scripts/smoke-gateway-restart.mjs",
  );
});
