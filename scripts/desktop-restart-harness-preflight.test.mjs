import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { detectRestartHarnessCommand, evaluateRestartHarness } from "./desktop-restart-harness-preflight.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "desktop-restart-harness-preflight.mjs");

test("macOS remains explicit manual-only restart evidence", () => {
  const result = evaluateRestartHarness({
    platform: "darwin",
    commands: {
      "tauri-driver": null,
      WebKitWebDriver: null,
      msedgedriver: null,
    },
    packageNames: new Set(),
  });

  assert.equal(result.status, "blocked_official_macos");
  assert.equal(result.canRunOfficialHarness, false);
  assert.match(result.reason, /manual on macOS/);
  assert.deepEqual(result.missing, [
    "tauri-driver",
    "webdriver client package",
    "official macOS WKWebView WebDriver support",
  ]);
});

test("macOS reports the official WKWebView driver gap even when repo dependencies exist", () => {
  const result = evaluateRestartHarness({
    platform: "darwin",
    commands: {
      "tauri-driver": "/usr/local/bin/tauri-driver",
      WebKitWebDriver: null,
      msedgedriver: null,
    },
    packageNames: new Set(["selenium-webdriver"]),
  });

  assert.equal(result.status, "blocked_official_macos");
  assert.equal(result.canRunOfficialHarness, false);
  assert.deepEqual(result.missing, ["official macOS WKWebView WebDriver support"]);
  assert.match(result.reason, /WKWebView/);
});

test("Linux reports ready when official driver pieces are present", () => {
  const result = evaluateRestartHarness({
    platform: "linux",
    commands: {
      "tauri-driver": "/usr/bin/tauri-driver",
      WebKitWebDriver: "/usr/bin/WebKitWebDriver",
      msedgedriver: null,
    },
    packageNames: new Set(["selenium-webdriver"]),
    restartHarnessCommand: "npm --prefix apps/desktop run test:tauri:restart",
  });

  assert.equal(result.status, "ready_official_webdriver");
  assert.equal(result.canRunOfficialHarness, true);
  assert.deepEqual(result.missing, []);
});

test("non-macOS does not report ready without a restart harness launch command", () => {
  const result = evaluateRestartHarness({
    platform: "linux",
    commands: {
      "tauri-driver": "/usr/bin/tauri-driver",
      WebKitWebDriver: "/usr/bin/WebKitWebDriver",
      msedgedriver: null,
    },
    packageNames: new Set(["selenium-webdriver"]),
    restartHarnessCommand: null,
  });

  assert.equal(result.status, "missing_harness_dependencies");
  assert.equal(result.canRunOfficialHarness, false);
  assert.deepEqual(result.missing, ["desktop restart harness launch command"]);
});

test("non-macOS reports missing harness dependencies precisely", () => {
  const result = evaluateRestartHarness({
    platform: "linux",
    commands: {
      "tauri-driver": "/usr/bin/tauri-driver",
      WebKitWebDriver: null,
      msedgedriver: null,
    },
    packageNames: new Set(),
  });

  assert.equal(result.status, "missing_harness_dependencies");
  assert.equal(result.canRunOfficialHarness, false);
  assert.deepEqual(result.missing, [
    "WebKitWebDriver",
    "webdriver client package",
    "desktop restart harness launch command",
  ]);
});

test("detects a configured restart harness launch command", () => {
  const tempRoot = mkdtempSync(join(tmpdir(), "forge-restart-harness-"));
  try {
    const desktopDir = join(tempRoot, "apps", "desktop");
    mkdirSync(desktopDir, { recursive: true });
    writeFileSync(
      join(desktopDir, "package.json"),
      JSON.stringify({
        scripts: {
          "test:desktop:restart": "wdio run ./restart.conf.ts",
        },
      }),
    );

    assert.equal(
      detectRestartHarnessCommand(tempRoot),
      "npm --prefix apps/desktop run test:desktop:restart",
    );
  } finally {
    rmSync(tempRoot, { force: true, recursive: true });
  }
});

test("json mode emits machine-readable current preflight result", () => {
  const output = execFileSync(process.execPath, [scriptPath, "--json"], {
    cwd: root,
    encoding: "utf8",
  });
  const result = JSON.parse(output);

  assert.equal(typeof result.status, "string");
  assert.equal(typeof result.platform, "string");
  assert.equal(typeof result.canRunOfficialHarness, "boolean");
  assert.ok(Array.isArray(result.missing));
  assert.equal(result.fallbackCommand, "npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts");
});
