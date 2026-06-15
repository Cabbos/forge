import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  buildDryRunPlan,
  buildGatewayEnv,
  buildGatewayRequest,
  buildWebhookPayload,
  parseArgs,
  parseWebhookAck,
} from "./smoke-gateway-webhook.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const desktopRoot = join(scriptDir, "..");

test("parseArgs builds a safe default dry-run plan", () => {
  const options = parseArgs(["--dry-run", "--message", " run digest "]);

  assert.equal(options.dryRun, true);
  assert.equal(options.message, "run digest");
  assert.equal(options.provider, null);
  assert.equal(options.model, null);
  assert.equal(options.keepHome, false);
});

test("buildWebhookPayload trims metadata and preserves explicit message", () => {
  const payload = buildWebhookPayload({
    message: " run digest ",
    profileId: " ops ",
    provider: " openai ",
    model: " gpt-5 ",
    workspacePath: " /repo/workspace ",
  });

  assert.deepEqual(payload, {
    message: "run digest",
    profile_id: "ops",
    provider: "openai",
    model: "gpt-5",
    workspace_path: "/repo/workspace",
  });
});

test("parseWebhookAck requires ok and id", () => {
  assert.deepEqual(parseWebhookAck('{"ok":true,"id":"trigger-1"}'), {
    ok: true,
    id: "trigger-1",
  });
  assert.throws(() => parseWebhookAck('{"ok":false}'), /missing ok\/id/);
  assert.throws(() => parseWebhookAck("not-json"), /invalid webhook ack/);
});

test("buildGatewayRequest creates json-line RPC payloads", () => {
  const line = buildGatewayRequest("list_pending_triggers", { limit: 5 });

  assert.match(line, /"method":"list_pending_triggers"/);
  assert.match(line, /"limit":5/);
  assert.match(line, /\n$/);
});

test("buildDryRunPlan points at isolated gateway smoke surfaces", () => {
  const plan = buildDryRunPlan({
    desktopRoot,
    message: "run digest",
    profileId: "ops",
    provider: "openai",
    model: "gpt-5",
    workspacePath: "/repo/workspace",
    timeoutMs: 10_000,
    keepHome: false,
  });

  assert.equal(plan.tcpHost, "127.0.0.1");
  assert.equal(plan.tcpPort, 2021);
  assert.match(plan.gatewayCommand.join(" "), /--bin gateway/);
  assert.match(plan.socketPath, /\.forge\/gateway\.sock$/);
  assert.deepEqual(plan.payload.provider, "openai");
});

test("buildGatewayEnv isolates Forge HOME while preserving rustup homes", () => {
  const env = buildGatewayEnv({
    baseEnv: {
      HOME: "/Users/tester",
      PATH: "/bin",
    },
    smokeHome: "/tmp/forge-smoke-home",
  });

  assert.equal(env.HOME, "/tmp/forge-smoke-home");
  assert.equal(env.CARGO_HOME, "/Users/tester/.cargo");
  assert.equal(env.RUSTUP_HOME, "/Users/tester/.rustup");
  assert.equal(env.PATH, "/bin");
});

test("package.json exposes the gateway webhook smoke command", () => {
  const pkg = JSON.parse(readFileSync(join(desktopRoot, "package.json"), "utf8"));

  assert.equal(
    pkg.scripts["smoke:gateway:webhook"],
    "node scripts/smoke-gateway-webhook.mjs",
  );
});
