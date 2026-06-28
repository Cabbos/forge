#!/usr/bin/env node
/**
 * Gateway Webhook Smoke
 *
 * Starts a temporary Forge gateway, sends one TCP JSON-line trigger to
 * 127.0.0.1:2021, verifies it through the Unix-socket gateway RPC, then
 * cancels the trigger and removes the temporary HOME.
 */

import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import net from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { randomUUID } from "node:crypto";

export const WEBHOOK_HOST = "127.0.0.1";
export const WEBHOOK_PORT = 2021;

export function parseArgs(argv) {
  const options = {
    dryRun: false,
    keepHome: false,
    message: "forge gateway webhook smoke",
    profileId: null,
    provider: null,
    model: null,
    workspacePath: null,
    timeoutMs: 30_000,
    help: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const nextValue = () => {
      index += 1;
      if (index >= argv.length) {
        throw new Error(`${arg} requires a value.`);
      }
      return argv[index];
    };

    if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "--keep-home") {
      options.keepHome = true;
    } else if (arg === "--message" || arg === "-m") {
      options.message = nextValue();
    } else if (arg === "--profile" || arg === "-p") {
      options.profileId = nextValue();
    } else if (arg === "--provider") {
      options.provider = nextValue();
    } else if (arg === "--model") {
      options.model = nextValue();
    } else if (arg === "--workspace" || arg === "-w") {
      options.workspacePath = nextValue();
    } else if (arg === "--timeout-ms") {
      options.timeoutMs = parsePositiveInteger("--timeout-ms", nextValue());
    } else if (arg === "--help" || arg === "-h") {
      options.help = true;
    } else {
      throw new Error(`Unknown option: ${arg}`);
    }
  }

  options.message = cleanRequired(options.message, "message");
  options.profileId = cleanOptional(options.profileId);
  options.provider = cleanOptional(options.provider);
  options.model = cleanOptional(options.model);
  options.workspacePath = cleanOptional(options.workspacePath);

  return options;
}

export function buildWebhookPayload(options) {
  const payload = {
    message: cleanRequired(options.message, "message"),
  };
  assignOptional(payload, "profile_id", options.profileId);
  assignOptional(payload, "provider", options.provider);
  assignOptional(payload, "model", options.model);
  assignOptional(payload, "workspace_path", options.workspacePath);
  return payload;
}

export function parseWebhookAck(line) {
  let ack;
  try {
    ack = JSON.parse(line);
  } catch (error) {
    throw new Error(`invalid webhook ack: ${error.message}`);
  }

  if (ack?.ok !== true || typeof ack.id !== "string" || ack.id.trim() === "") {
    throw new Error(`webhook ack missing ok/id: ${line}`);
  }

  return { ok: true, id: ack.id };
}

export function buildGatewayRequest(method, params = null) {
  const request = {
    id: randomUUID().replaceAll("-", ""),
    method,
  };
  if (params != null) {
    request.params = params;
  }
  return `${JSON.stringify(request)}\n`;
}

export function buildGatewayEnv({ baseEnv = process.env, smokeHome }) {
  const originalHome = baseEnv.HOME ?? baseEnv.USERPROFILE ?? process.cwd();
  return {
    ...baseEnv,
    HOME: smokeHome,
    CARGO_HOME: baseEnv.CARGO_HOME ?? join(originalHome, ".cargo"),
    RUSTUP_HOME: baseEnv.RUSTUP_HOME ?? join(originalHome, ".rustup"),
    RUST_LOG: baseEnv.RUST_LOG ?? "warn",
  };
}

export function buildDryRunPlan({
  desktopRoot,
  message,
  profileId,
  provider,
  model,
  workspacePath,
  timeoutMs,
  keepHome,
  homeDir,
}) {
  const smokeHome = homeDir ?? join(tmpdir(), "forge-gateway-webhook-smoke-home");
  const payload = buildWebhookPayload({
    message,
    profileId,
    provider,
    model,
    workspacePath,
  });

  return {
    desktopRoot,
    homeDir: smokeHome,
    keepHome: Boolean(keepHome),
    tcpHost: WEBHOOK_HOST,
    tcpPort: WEBHOOK_PORT,
    socketPath: join(smokeHome, ".forge", "gateway.sock"),
    gatewayCommand: [
      "cargo",
      "run",
      "--manifest-path",
      join(desktopRoot, "src-tauri", "Cargo.toml"),
      "--bin",
      "gateway",
      "--quiet",
    ],
    payload,
    timeoutMs,
  };
}

async function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`[smoke-gateway-webhook] ${error.message}`);
    console.error(usage());
    process.exit(1);
  }

  if (options.help) {
    console.log(usage());
    return;
  }

  const desktopRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const tempHome = options.keepHome
    ? (process.env.HOME ?? process.cwd())
    : mkdtempSync(join(tmpdir(), "forge-gateway-webhook-home-"));
  const plan = buildDryRunPlan({
    desktopRoot,
    ...options,
    homeDir: tempHome,
  });

  if (options.dryRun) {
    printDryRun(plan);
    if (!options.keepHome) rmSync(tempHome, { recursive: true, force: true });
    return;
  }

  let gateway = null;
  try {
    const portFree = await tcpPortIsFree(plan.tcpHost, plan.tcpPort, 250);
    if (!portFree) {
      throw new Error(
        `TCP ${plan.tcpHost}:${plan.tcpPort} is already in use. Stop the running gateway before this isolated smoke.`,
      );
    }

    console.log("[smoke-gateway-webhook] Starting temporary gateway...");
    gateway = spawn(plan.gatewayCommand[0], plan.gatewayCommand.slice(1), {
      cwd: desktopRoot,
      env: buildGatewayEnv({ baseEnv: process.env, smokeHome: plan.homeDir }),
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stderr = "";
    gateway.stderr?.on("data", (chunk) => {
      stderr += chunk.toString();
    });

    await waitForGateway(plan, stderrRef(() => stderr));

    console.log("[smoke-gateway-webhook] Sending webhook trigger...");
    const ackLine = await sendTcpLine(
      plan.tcpHost,
      plan.tcpPort,
      `${JSON.stringify(plan.payload)}\n`,
      plan.timeoutMs,
    );
    const ack = parseWebhookAck(ackLine);

    const listReply = await sendUnixLine(
      plan.socketPath,
      buildGatewayRequest("list_pending_triggers"),
      plan.timeoutMs,
    );
    const triggers = parseGatewayResult(listReply, "list_pending_triggers");
    const queued = Array.isArray(triggers)
      ? triggers.find((trigger) => trigger?.id === ack.id)
      : null;
    if (!queued) {
      throw new Error(`Trigger ${ack.id} was acknowledged but not visible in gateway queue.`);
    }

    await sendUnixLine(
      plan.socketPath,
      buildGatewayRequest("cancel_trigger", { trigger_id: ack.id }),
      plan.timeoutMs,
    );

    console.log(`[smoke-gateway-webhook] PASS trigger=${ack.id}`);
  } finally {
    if (gateway) {
      gateway.kill("SIGTERM");
      await waitForExit(gateway, 3_000).catch(() => gateway.kill("SIGKILL"));
    }
    if (!options.keepHome) {
      rmSync(tempHome, { recursive: true, force: true });
    }
  }
}

function parseGatewayResult(line, method) {
  let reply;
  try {
    reply = JSON.parse(line);
  } catch (error) {
    throw new Error(`invalid gateway ${method} reply: ${error.message}`);
  }
  if (reply?.error) {
    throw new Error(`gateway ${method} error: ${reply.error.message ?? "unknown"}`);
  }
  return reply?.result;
}

async function waitForGateway(plan, getStderr) {
  const deadline = Date.now() + plan.timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      await sendUnixLine(plan.socketPath, buildGatewayRequest("ping"), 500);
      return;
    } catch (error) {
      lastError = error;
      await delay(250);
    }
  }
  throw new Error(
    `gateway did not become ready within ${plan.timeoutMs}ms: ${lastError?.message ?? "timeout"}\n${getStderr()}`,
  );
}

function sendTcpLine(host, port, line, timeoutMs) {
  return sendLine(() => net.createConnection({ host, port }), line, timeoutMs);
}

function sendUnixLine(socketPath, line, timeoutMs) {
  return sendLine(() => net.createConnection(socketPath), line, timeoutMs);
}

function sendLine(createSocket, line, timeoutMs) {
  return new Promise((resolvePromise, reject) => {
    const socket = createSocket();
    let buffer = "";
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error("socket timed out"));
    }, timeoutMs);

    socket.setEncoding("utf8");
    socket.on("connect", () => socket.write(line));
    socket.on("data", (chunk) => {
      buffer += chunk;
      const newline = buffer.indexOf("\n");
      if (newline >= 0) {
        clearTimeout(timer);
        socket.end();
        resolvePromise(buffer.slice(0, newline));
      }
    });
    socket.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    socket.on("end", () => {
      if (buffer.trim()) {
        clearTimeout(timer);
        resolvePromise(buffer.trim());
      }
    });
  });
}

function tcpPortIsFree(host, port, timeoutMs) {
  return new Promise((resolvePromise) => {
    const socket = net.createConnection({ host, port });
    const timer = setTimeout(() => {
      socket.destroy();
      resolvePromise(true);
    }, timeoutMs);
    socket.on("connect", () => {
      clearTimeout(timer);
      socket.end();
      resolvePromise(false);
    });
    socket.on("error", () => {
      clearTimeout(timer);
      resolvePromise(true);
    });
  });
}

function waitForExit(child, timeoutMs) {
  return new Promise((resolvePromise, reject) => {
    const timer = setTimeout(() => reject(new Error("process exit timed out")), timeoutMs);
    child.once("exit", () => {
      clearTimeout(timer);
      resolvePromise();
    });
  });
}

function stderrRef(getter) {
  return getter;
}

function printDryRun(plan) {
  console.log("[smoke-gateway-webhook] DRY RUN");
  console.log(`  home: ${plan.homeDir}`);
  console.log(`  socket: ${plan.socketPath}`);
  console.log(`  tcp: ${plan.tcpHost}:${plan.tcpPort}`);
  console.log(`  command: ${plan.gatewayCommand.join(" ")}`);
  console.log(`  payload: ${JSON.stringify(plan.payload)}`);
  console.log("  Pass criteria:");
  console.log("    - gateway ping succeeds over Unix socket");
  console.log("    - TCP webhook returns { ok: true, id }");
  console.log("    - list_pending_triggers contains the acknowledged trigger id");
  console.log("    - cleanup cancels the smoke trigger and removes temp HOME");
}

function assignOptional(target, key, value) {
  const cleaned = cleanOptional(value);
  if (cleaned) target[key] = cleaned;
}

function cleanOptional(value) {
  if (value == null) return null;
  const trimmed = String(value).trim();
  return trimmed || null;
}

function cleanRequired(value, label) {
  const trimmed = cleanOptional(value);
  if (!trimmed) throw new Error(`${label} must not be empty.`);
  return trimmed;
}

function parsePositiveInteger(flag, value) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${flag} must be a positive integer.`);
  }
  return parsed;
}

function delay(ms) {
  return new Promise((resolvePromise) => setTimeout(resolvePromise, ms));
}

function usage() {
  return [
    "Usage: node scripts/smoke-gateway-webhook.mjs [options]",
    "",
    "Options:",
    "  --dry-run                Print the smoke plan without starting gateway",
    "  --message, -m <text>     Trigger message to send",
    "  --profile, -p <id>       Optional profile id",
    "  --provider <name>        Optional provider override",
    "  --model <name>           Optional model override",
    "  --workspace, -w <path>   Optional workspace path",
    "  --timeout-ms <ms>        Gateway readiness/socket timeout",
    "  --keep-home              Use current HOME instead of an isolated temp HOME",
  ].join("\n");
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main().catch((error) => {
    console.error(`[smoke-gateway-webhook] ERROR: ${error.message}`);
    process.exit(1);
  });
}
