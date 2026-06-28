#!/usr/bin/env node
import { constants, accessSync, existsSync, readFileSync } from "node:fs";
import { delimiter, dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath, pathToFileURL } from "node:url";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const ROOT_DIR = resolve(dirname(SCRIPT_PATH), "..");
const WEBDRIVER_CLIENT_PACKAGES = new Set(["selenium-webdriver", "webdriverio", "@wdio/cli"]);
const MACOS_OFFICIAL_DRIVER_GAP = "official macOS WKWebView WebDriver support";
const RESTART_HARNESS_SCRIPT_NAMES = ["test:tauri:restart", "test:desktop:restart", "e2e:tauri:restart"];

export function evaluateRestartHarness({
  platform = process.platform,
  commands = detectCommands(platform),
  packageNames = readWorkspacePackageNames(ROOT_DIR),
  restartHarnessCommand = detectRestartHarnessCommand(ROOT_DIR),
} = {}) {
  const packageSet = packageNames instanceof Set ? packageNames : new Set(packageNames);
  const webdriverClientPackages = [...packageSet].filter((name) => WEBDRIVER_CLIENT_PACKAGES.has(name));
  const hasWebdriverClient = webdriverClientPackages.length > 0;
  const hasTauriDriver = Boolean(commands["tauri-driver"]);
  const nativeDriverName = nativeDriverForPlatform(platform);
  const hasNativeDriver = nativeDriverName ? Boolean(commands[nativeDriverName]) : false;
  const hasRestartHarnessCommand = Boolean(restartHarnessCommand);

  const missing = [];
  if (!hasTauriDriver) missing.push("tauri-driver");
  if (nativeDriverName && !hasNativeDriver) missing.push(nativeDriverName);
  if (!hasWebdriverClient) missing.push("webdriver client package");
  if (platform !== "darwin" && !hasRestartHarnessCommand) {
    missing.push("desktop restart harness launch command");
  }

  if (platform === "darwin") {
    if (!missing.includes(MACOS_OFFICIAL_DRIVER_GAP)) missing.push(MACOS_OFFICIAL_DRIVER_GAP);
    return {
      status: "blocked_official_macos",
      canRunOfficialHarness: false,
      platform,
      missing,
      detected: {
        commands,
        webdriverClientPackages,
        restartHarnessCommand,
      },
      reason:
        "Forge treats true Tauri force-quit/reopen proof as manual on macOS until official WKWebView WebDriver support is available for this repo.",
      fallbackCommand: "npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts",
    };
  }

  const canRunOfficialHarness = hasTauriDriver && hasNativeDriver && hasWebdriverClient && hasRestartHarnessCommand;
  return {
    status: canRunOfficialHarness ? "ready_official_webdriver" : "missing_harness_dependencies",
    canRunOfficialHarness,
    platform,
    missing,
    detected: {
      commands,
      webdriverClientPackages,
      restartHarnessCommand,
    },
    reason: canRunOfficialHarness
      ? "Official Tauri/WebDriver restart harness dependencies appear available."
      : "Official Tauri/WebDriver restart harness dependencies are incomplete.",
    fallbackCommand: "npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts",
  };
}

export function detectCommands(platform = process.platform, envPath = process.env.PATH ?? "") {
  const candidates = new Set(["tauri-driver", "WebKitWebDriver", "msedgedriver"]);
  return Object.fromEntries([...candidates].map((command) => [command, findCommand(command, platform, envPath)]));
}

export function readWorkspacePackageNames(rootDir) {
  const packageFiles = [join(rootDir, "package.json"), join(rootDir, "apps", "desktop", "package.json")];
  const names = new Set();
  for (const packageFile of packageFiles) {
    if (!existsSync(packageFile)) continue;
    const parsed = JSON.parse(readFileSync(packageFile, "utf8"));
    for (const section of ["dependencies", "devDependencies", "optionalDependencies"]) {
      for (const name of Object.keys(parsed[section] ?? {})) {
        names.add(name);
      }
    }
  }
  return names;
}

export function detectRestartHarnessCommand(rootDir) {
  const packageFile = join(rootDir, "apps", "desktop", "package.json");
  if (!existsSync(packageFile)) return null;
  const parsed = JSON.parse(readFileSync(packageFile, "utf8"));
  const scripts = parsed.scripts ?? {};
  const scriptName = RESTART_HARNESS_SCRIPT_NAMES.find((candidate) => Object.hasOwn(scripts, candidate));
  return scriptName ? `npm --prefix apps/desktop run ${scriptName}` : null;
}

function nativeDriverForPlatform(platform) {
  if (platform === "linux") return "WebKitWebDriver";
  if (platform === "win32") return "msedgedriver";
  return null;
}

function findCommand(command, platform, envPath) {
  const pathParts = envPath.split(delimiter).filter(Boolean);
  const extensions = platform === "win32" ? ["", ".exe", ".cmd", ".bat"] : [""];
  for (const pathPart of pathParts) {
    for (const extension of extensions) {
      const candidate = join(pathPart, `${command}${extension}`);
      try {
        accessSync(candidate, constants.X_OK);
        return candidate;
      } catch {
        // Keep searching.
      }
    }
  }
  return null;
}

function printHuman(result) {
  console.log("Desktop restart harness preflight");
  console.log(`Status: ${result.status}`);
  console.log(`Platform: ${result.platform}`);
  console.log(`Official harness ready: ${result.canRunOfficialHarness ? "yes" : "no"}`);
  if (result.missing.length > 0) {
    console.log(`Missing: ${result.missing.join(", ")}`);
  }
  if (result.detected.restartHarnessCommand) {
    console.log(`Harness launch: ${result.detected.restartHarnessCommand}`);
  }
  console.log(`Fallback smoke: ${result.fallbackCommand}`);
  console.log(result.reason);
}

function printHelp() {
  console.log(`Usage: node scripts/desktop-restart-harness-preflight.mjs [--json] [--require-harness]

Checks whether this checkout has the official Tauri/WebDriver pieces needed for a true desktop force-quit/reopen smoke.

Options:
  --json             Print machine-readable status.
  --require-harness  Exit non-zero when the official harness is unavailable.
  -h, --help         Show this help.
`);
}

function main(argv = process.argv.slice(2)) {
  const json = argv.includes("--json");
  const requireHarness = argv.includes("--require-harness");
  if (argv.includes("-h") || argv.includes("--help")) {
    printHelp();
    return 0;
  }
  const result = evaluateRestartHarness();
  if (json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    printHuman(result);
  }
  return requireHarness && !result.canRunOfficialHarness ? 1 : 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
