import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { join } from "node:path";
import test from "node:test";

import {
  diagnoseDesktopUiEvidence,
  openDesktopUiEvidenceSettings,
  runDesktopUiEvidenceRecoveryChecks,
} from "./desktop-ui-evidence-doctor.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "desktop-ui-evidence-doctor.mjs");

test("diagnoses both screen recording and accessibility blockers", () => {
  const result = diagnoseDesktopUiEvidence({
    preflight: {
      status: "screen_capture_limited",
      canCollectLiveUiEvidence: false,
      platform: "darwin",
      reason: "blank screenshot",
      windowSnapshot: {
        ok: true,
        rows: [
          { name: "Codex", visible: true, windowCount: 0 },
          { name: "Google Chrome", visible: true, windowCount: 0 },
        ],
        error: null,
      },
      screenSnapshot: {
        ok: true,
        width: 2940,
        height: 1912,
        sizeBytes: 106_973,
        compressedBytesPerPixel: 0.019,
        likelyBlank: true,
        error: null,
      },
      recommendations: [],
    },
    controllingApps: ["Codex"],
  });

  assert.equal(result.status, "needs_screen_recording_and_accessibility");
  assert.equal(result.canCollectLiveUiEvidence, false);
  assert.deepEqual(result.blockers.map((blocker) => blocker.id), [
    "screen_recording_permission",
    "accessibility_permission",
  ]);
  assert.ok(
    result.blockers.every((blocker) =>
      blocker.actions.some((entry) => entry.includes("phase8-disposable-loop-status.mjs --json --require-live-ready")),
    ),
  );
  assert.equal(result.permissionScope.kind, "macos_privacy");
  assert.match(result.permissionScope.note, /Forge Trust\/Full Access does not grant macOS Screen Recording or Accessibility/);
  assert.ok(result.commands.some((entry) => entry.command.includes("Privacy_ScreenCapture")));
  assert.ok(result.commands.some((entry) => entry.command.includes("Privacy_Accessibility")));
  assert.ok(result.commands.some((entry) => entry.command.includes("phase8-disposable-loop-status.mjs --json --require-live-ready")));
  assert.match(result.nextStep, /live-ready hard gate/);
  assert.match(result.markdown, /Grant Screen Recording and Accessibility/);
  assert.match(result.markdown, /Forge Trust\/Full Access does not grant macOS Screen Recording or Accessibility/);
  assert.match(result.markdown, /phase8-disposable-loop-status\.mjs --json --require-live-ready/);
});

test("diagnoses a screen recording-only blocker", () => {
  const result = diagnoseDesktopUiEvidence({
    preflight: {
      status: "screen_capture_failed",
      canCollectLiveUiEvidence: false,
      platform: "darwin",
      reason: "not authorized",
      windowSnapshot: {
        ok: true,
        rows: [{ name: "Codex", visible: true, windowCount: 1 }],
        error: null,
      },
      screenSnapshot: {
        ok: false,
        error: "not authorized",
      },
      recommendations: [],
    },
  });

  assert.equal(result.status, "needs_screen_recording");
  assert.deepEqual(result.blockers.map((blocker) => blocker.id), ["screen_recording_permission"]);
});

test("diagnoses an accessibility-only blocker", () => {
  const result = diagnoseDesktopUiEvidence({
    preflight: {
      status: "window_snapshot_failed",
      canCollectLiveUiEvidence: false,
      platform: "darwin",
      reason: "not authorized",
      windowSnapshot: {
        ok: false,
        rows: [],
        error: "not authorized",
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
      recommendations: [],
    },
  });

  assert.equal(result.status, "needs_accessibility");
  assert.deepEqual(result.blockers.map((blocker) => blocker.id), ["accessibility_permission"]);
});

test("reports ready when preflight evidence is collectible", () => {
  const result = diagnoseDesktopUiEvidence({
    preflight: {
      status: "ready",
      canCollectLiveUiEvidence: true,
      platform: "darwin",
      reason: "ok",
      windowSnapshot: {
        ok: true,
        rows: [{ name: "Codex", visible: true, windowCount: 1 }],
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
      recommendations: [],
    },
  });

  assert.equal(result.status, "ready");
  assert.equal(result.canCollectLiveUiEvidence, true);
  assert.deepEqual(result.blockers, []);
});

test("opens only settings panes for current blockers when requested", () => {
  const diagnosis = diagnoseDesktopUiEvidence({
    preflight: {
      status: "screen_capture_limited",
      canCollectLiveUiEvidence: false,
      platform: "darwin",
      reason: "blank screenshot",
      windowSnapshot: {
        ok: true,
        rows: [{ name: "Codex", visible: true, windowCount: 0 }],
        error: null,
      },
      screenSnapshot: {
        ok: true,
        width: 2940,
        height: 1912,
        sizeBytes: 106_973,
        compressedBytesPerPixel: 0.019,
        likelyBlank: true,
        error: null,
      },
      recommendations: [],
    },
  });
  const calls = [];
  const result = openDesktopUiEvidenceSettings({
    diagnosis,
    runner: (command, args) => calls.push({ command, args }),
  });

  assert.equal(result.openedCount, 2);
  assert.deepEqual(calls.map((call) => call.command), ["open", "open"]);
  assert.ok(calls[0].args[0].includes("Privacy_ScreenCapture"));
  assert.ok(calls[1].args[0].includes("Privacy_Accessibility"));
});

test("does not open settings when no blocker has a settings URL", () => {
  const diagnosis = diagnoseDesktopUiEvidence({
    preflight: {
      status: "ready",
      canCollectLiveUiEvidence: true,
      platform: "darwin",
      reason: "ok",
      windowSnapshot: {
        ok: true,
        rows: [{ name: "Codex", visible: true, windowCount: 1 }],
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
      recommendations: [],
    },
  });
  const calls = [];
  const result = openDesktopUiEvidenceSettings({
    diagnosis,
    runner: (command, args) => calls.push({ command, args }),
  });

  assert.equal(result.openedCount, 0);
  assert.deepEqual(calls, []);
});

test("runs strict preflight and live-ready gate recovery checks", () => {
  const calls = [];
  const result = runDesktopUiEvidenceRecoveryChecks({
    runner: (command, args) => {
      calls.push({ command, args });
      if (args.some((arg) => arg.endsWith("desktop-ui-evidence-preflight.mjs"))) {
        return JSON.stringify({
          status: "ready",
          canCollectLiveUiEvidence: true,
          reason: "screenshots and windows are observable",
        });
      }
      return JSON.stringify({
        status: "ready_for_live_row",
        readyForLiveRun: true,
        liveReadyGate: {
          pass: true,
          reason: "ready",
        },
      });
    },
  });

  assert.equal(result.status, "passed");
  assert.equal(result.passed, true);
  assert.deepEqual(
    calls.map((call) => call.args.join(" ")),
    [
      "scripts/desktop-ui-evidence-preflight.mjs --json --require-ready",
      "scripts/phase8-disposable-loop-status.mjs --json --require-live-ready",
    ],
  );
  assert.ok(result.checks.every((check) => check.ok));
  assert.deepEqual(result.checks[0].summary, {
    status: "ready",
    canCollectLiveUiEvidence: true,
    reason: "screenshots and windows are observable",
  });
  assert.deepEqual(result.checks[1].summary, {
    status: "ready_for_live_row",
    readyForLiveRun: true,
    liveReadyGatePass: true,
    liveReadyGateReason: "ready",
  });
});

test("reports failing recovery checks without skipping the remaining gate", () => {
  const calls = [];
  const result = runDesktopUiEvidenceRecoveryChecks({
    runner: (command, args) => {
      calls.push({ command, args });
      if (args.some((arg) => arg.endsWith("desktop-ui-evidence-preflight.mjs"))) {
        const error = new Error("preflight failed");
        error.status = 1;
        error.stdout = JSON.stringify({
          status: "screen_capture_limited",
          canCollectLiveUiEvidence: false,
          reason: "blank screenshot",
        });
        error.stderr = "screen capture blocked";
        throw error;
      }
      return JSON.stringify({
        status: "ui_evidence_not_ready",
        readyForLiveRun: false,
        liveReadyGate: {
          pass: false,
          reason: "ui_evidence_not_ready",
        },
      });
    },
  });

  assert.equal(result.status, "failed");
  assert.equal(result.passed, false);
  assert.equal(result.checks.length, 2);
  assert.equal(calls.length, 2);
  assert.equal(result.checks[0].ok, false);
  assert.equal(result.checks[0].exitCode, 1);
  assert.match(result.checks[0].stderr, /screen capture blocked/);
  assert.deepEqual(result.checks[0].summary, {
    status: "screen_capture_limited",
    canCollectLiveUiEvidence: false,
    reason: "blank screenshot",
  });
  assert.equal(result.checks[1].ok, true);
  assert.deepEqual(result.checks[1].summary, {
    status: "ui_evidence_not_ready",
    readyForLiveRun: false,
    liveReadyGatePass: false,
    liveReadyGateReason: "ui_evidence_not_ready",
  });
});

test("cli markdown emits a diagnosis", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--markdown"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 0);
  if (result.stdout.includes("Live UI evidence ready: yes")) {
    assert.match(result.stdout, /Live UI evidence ready: yes/);
  } else {
    assert.match(result.stdout, /Desktop UI Evidence Doctor/);
    assert.match(result.stdout, /Commands:/);
  }
});

test("cli json emits machine-readable diagnosis", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--json"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 0);
  const parsed = JSON.parse(result.stdout);
  assert.equal(typeof parsed.status, "string");
  assert.equal(typeof parsed.canCollectLiveUiEvidence, "boolean");
  assert.ok(Array.isArray(parsed.commands));
  assert.ok(parsed.commands.some((entry) => entry.command.includes("phase8-disposable-loop-status.mjs --json --require-live-ready")));
  assert.equal(parsed.openSettings, null);
});

test("require-ready exits nonzero only when evidence is not ready", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--json", "--require-ready"], {
    cwd: root,
    encoding: "utf8",
  });
  const parsed = JSON.parse(result.stdout);

  if (parsed.canCollectLiveUiEvidence) {
    assert.equal(result.status, 0);
  } else {
    assert.notEqual(result.status, 0);
  }
});

test("help exits successfully", () => {
  const output = execFileSync(process.execPath, [scriptPath, "--help"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.match(output, /desktop-ui-evidence-doctor/);
  assert.match(output, /--open-settings/);
  assert.match(output, /--run-checks/);
});
