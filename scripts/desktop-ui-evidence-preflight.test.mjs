import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { join } from "node:path";
import test from "node:test";

import {
  collectScreenSnapshotSafe,
  evaluateDesktopUiEvidencePreflight,
} from "./desktop-ui-evidence-preflight.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "desktop-ui-evidence-preflight.mjs");

test("reports observer_limited when visible known apps expose zero windows", () => {
  const result = evaluateDesktopUiEvidencePreflight({
    platform: "darwin",
    windowSnapshot: {
      ok: true,
      rows: [
        { name: "Google Chrome", visible: true, windowCount: 0 },
        { name: "Codex", visible: true, windowCount: 0 },
        { name: "forge", visible: true, windowCount: 0 },
      ],
      error: null,
    },
  });

  assert.equal(result.status, "observer_limited");
  assert.equal(result.canCollectLiveUiEvidence, false);
  assert.equal(result.permissionScope.kind, "macos_privacy");
  assert.match(result.permissionScope.note, /Forge Trust\/Full Access does not grant macOS Screen Recording or Accessibility/);
  assert.match(result.reason, /zero windows/);
  assert.ok(result.recoveryCommands.some((entry) => entry.command.includes("desktop-ui-evidence-doctor.mjs --markdown")));
  assert.ok(result.recoveryCommands.some((entry) => entry.command.includes("--open-settings")));
  assert.ok(result.recoveryCommands.some((entry) => entry.command.includes("desktop-ui-evidence-preflight.mjs --json --require-ready")));
  assert.ok(result.recoveryCommands.some((entry) => entry.command.includes("phase8-disposable-loop-status.mjs --json --require-live-ready")));
});

test("reports screen_capture_limited when screenshots are likely blank", () => {
  const result = evaluateDesktopUiEvidencePreflight({
    platform: "darwin",
    windowSnapshot: {
      ok: true,
      rows: [{ name: "Google Chrome", visible: true, windowCount: 1 }],
      error: null,
    },
    screenSnapshot: {
      ok: true,
      width: 1920,
      height: 1080,
      sizeBytes: 100_000,
      compressedBytesPerPixel: 0.048,
      likelyBlank: true,
      error: null,
    },
  });

  assert.equal(result.status, "screen_capture_limited");
  assert.equal(result.canCollectLiveUiEvidence, false);
  assert.match(result.reason, /blank image/);
  assert.ok(result.recoveryCommands.some((entry) => entry.command.includes("desktop-ui-evidence-doctor.mjs --markdown")));
  assert.ok(result.recoveryCommands.some((entry) => entry.command.includes("phase8-disposable-loop-status.mjs --json --require-live-ready")));
});

test("reports screen capture failure separately from Forge runtime status", () => {
  const result = evaluateDesktopUiEvidencePreflight({
    platform: "darwin",
    windowSnapshot: {
      ok: true,
      rows: [{ name: "Google Chrome", visible: true, windowCount: 1 }],
      error: null,
    },
    screenSnapshot: {
      ok: false,
      width: 0,
      height: 0,
      sizeBytes: 0,
      compressedBytesPerPixel: null,
      likelyBlank: false,
      error: "not authorized",
    },
  });

  assert.equal(result.status, "screen_capture_failed");
  assert.equal(result.canCollectLiveUiEvidence, false);
  assert.ok(result.recoveryCommands.some((entry) => entry.command.includes("--open-settings")));
});

test("reports ready when at least one visible app window is observable", () => {
  const result = evaluateDesktopUiEvidencePreflight({
    platform: "darwin",
    windowSnapshot: {
      ok: true,
      rows: [
        { name: "Google Chrome", visible: true, windowCount: 2 },
        { name: "forge", visible: true, windowCount: 1 },
      ],
      error: null,
    },
    screenSnapshot: {
      ok: true,
      width: 1920,
      height: 1080,
      sizeBytes: 1_000_000,
      compressedBytesPerPixel: 0.48,
      likelyBlank: false,
      error: null,
    },
  });

  assert.equal(result.status, "ready");
  assert.equal(result.canCollectLiveUiEvidence, true);
  assert.deepEqual(result.recoveryCommands, []);
});

test("reports snapshot failure separately from Forge runtime status", () => {
  const result = evaluateDesktopUiEvidencePreflight({
    platform: "darwin",
    windowSnapshot: {
      ok: false,
      rows: [],
      error: "not authorized",
    },
  });

  assert.equal(result.status, "window_snapshot_failed");
  assert.equal(result.canCollectLiveUiEvidence, false);
  assert.ok(result.recoveryCommands.some((entry) => entry.command.includes("desktop-ui-evidence-doctor.mjs")));
});

test("cli json emits a machine-readable status", () => {
  const output = execFileSync(process.execPath, [scriptPath, "--json", "--skip-screen-capture"], {
    cwd: root,
    encoding: "utf8",
  });
  const parsed = JSON.parse(output);

  assert.equal(typeof parsed.status, "string");
  assert.equal(typeof parsed.canCollectLiveUiEvidence, "boolean");
  assert.equal(parsed.platform, process.platform);
  assert.equal(parsed.permissionScope.kind, "macos_privacy");
  assert.ok(Array.isArray(parsed.recoveryCommands));
});

test("screen snapshot collector returns structured status", { skip: process.platform !== "darwin" }, () => {
  const result = collectScreenSnapshotSafe();

  assert.equal(typeof result.ok, "boolean");
  assert.equal(typeof result.likelyBlank, "boolean");
  assert.equal(typeof result.sizeBytes, "number");
});

test("require-ready exits nonzero when the current observer is not ready", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--require-ready", "--skip-screen-capture"], {
    cwd: root,
    encoding: "utf8",
  });

  if (result.status === 0) {
    assert.match(result.stdout, /Live UI evidence ready: yes/);
  } else {
    assert.match(result.stdout, /Live UI evidence ready: no/);
    assert.match(result.stdout, /Recovery commands:/);
  }
});
