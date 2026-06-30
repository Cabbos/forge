#!/usr/bin/env node

import { tmpdir } from "node:os";
import { mkdtempSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const scriptDir = dirname(scriptPath);
const repoRoot = resolve(scriptDir, "..", "..", "..");

export function buildGatewayRestartPlan({ root }) {
  const home = join(root, "home");

  return {
    root,
    home,
    gatewayCommand: [
      "cargo",
      "run",
      "--manifest-path",
      join(repoRoot, "apps", "desktop", "src-tauri", "Cargo.toml"),
      "--bin",
      "gateway",
      "--quiet",
    ],
    triggerStorePath: join(home, ".forge", "triggers.json"),
    runStorePath: join(home, ".forge", "trigger-runs.json"),
  };
}

function parseArgs(argv) {
  const options = {
    dryRun: false,
    json: false,
    root: null,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "--json") {
      options.json = true;
    } else if (arg === "--root") {
      index += 1;
      if (index >= argv.length || argv[index].trim() === "") {
        throw new Error("--root requires a value.");
      }
      options.root = argv[index];
    } else {
      throw new Error(`Unknown option: ${arg}`);
    }
  }

  return options;
}

function printJson(payload) {
  process.stdout.write(`${JSON.stringify(payload, null, 2)}\n`);
}

function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`[smoke-gateway-restart] ${error.message}`);
    process.exit(1);
  }

  const root = options.root ?? mkdtempSync(join(tmpdir(), "forge-gateway-restart-"));
  const plan = buildGatewayRestartPlan({ root });
  if (options.dryRun) {
    printJson({ ok: true, dryRun: options.dryRun, plan });
    return;
  }

  console.error(
    "[smoke-gateway-restart] Live gateway restart smoke is not implemented; refusing to claim restart execution.",
  );
  process.exit(2);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
