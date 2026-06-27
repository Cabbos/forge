#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

import { DESKTOP_UI_EVIDENCE_PERMISSION_SCOPE } from "./desktop-ui-evidence-permission-scope.mjs";

const KNOWN_WINDOWED_APPS = ["Google Chrome", "Codex", "Finder"];
const BLANK_SCREENSHOT_BYTES_PER_PIXEL = 0.08;
const DOCTOR_COMMAND = "node scripts/desktop-ui-evidence-doctor.mjs --markdown";
const DOCTOR_OPEN_SETTINGS_COMMAND = "node scripts/desktop-ui-evidence-doctor.mjs --markdown --open-settings";
const PREFLIGHT_REQUIRE_READY_COMMAND = "node scripts/desktop-ui-evidence-preflight.mjs --json --require-ready";
const LIVE_READY_HARD_GATE_COMMAND = "node scripts/phase8-disposable-loop-status.mjs --json --require-live-ready";

export function evaluateDesktopUiEvidencePreflight({
  platform = process.platform,
  windowSnapshot = collectWindowSnapshotSafe(),
  screenSnapshot = null,
  requiredApps = KNOWN_WINDOWED_APPS,
} = {}) {
  if (platform !== "darwin") {
    return {
      status: "unsupported_platform",
      canCollectLiveUiEvidence: false,
      platform,
      reason: "Desktop UI evidence preflight currently checks macOS window observability only.",
      windowSnapshot,
      screenSnapshot,
      permissionScope: DESKTOP_UI_EVIDENCE_PERMISSION_SCOPE,
      recoveryCommands: recoveryCommands(),
      recommendations: ["Use the platform-specific desktop UI harness for live evidence."],
    };
  }

  if (screenSnapshot && !screenSnapshot.ok) {
    return {
      status: "screen_capture_failed",
      canCollectLiveUiEvidence: false,
      platform,
      reason: "macOS screen capture failed, so screenshot-based live UI evidence is not trustworthy.",
      windowSnapshot,
      screenSnapshot,
      permissionScope: DESKTOP_UI_EVIDENCE_PERMISSION_SCOPE,
      recoveryCommands: recoveryCommands({ includeOpenSettings: true }),
      recommendations: [
        "Grant Screen Recording permission to the controlling app or collect the live row manually.",
        `Run \`${DOCTOR_COMMAND}\` for the concrete recovery checklist.`,
        "Do not treat missing screenshots as Forge runtime evidence.",
      ],
    };
  }

  if (screenSnapshot?.likelyBlank) {
    return {
      status: "screen_capture_limited",
      canCollectLiveUiEvidence: false,
      platform,
      reason:
        "macOS screen capture produced a likely blank image; screenshot-based live UI evidence is not trustworthy.",
      windowSnapshot,
      screenSnapshot,
      permissionScope: DESKTOP_UI_EVIDENCE_PERMISSION_SCOPE,
      recoveryCommands: recoveryCommands({ includeOpenSettings: true }),
      recommendations: [
        "Grant Screen Recording permission to the controlling app or collect the live row manually.",
        `Run \`${DOCTOR_COMMAND}\` for the concrete recovery checklist.`,
        "Keep Phase 8 row status as pending until final-answer, diff, build, and confirmation evidence is captured.",
      ],
    };
  }

  if (!windowSnapshot.ok) {
    return {
      status: "window_snapshot_failed",
      canCollectLiveUiEvidence: false,
      platform,
      reason: "System Events could not enumerate visible process windows.",
      windowSnapshot,
      screenSnapshot,
      permissionScope: DESKTOP_UI_EVIDENCE_PERMISSION_SCOPE,
      recoveryCommands: recoveryCommands({ includeOpenSettings: true }),
      recommendations: [
        "Grant Accessibility permission to the controlling app or run the live row manually.",
        `Run \`${DOCTOR_COMMAND}\` for the concrete recovery checklist.`,
        "Do not treat missing screenshots or window counts as Forge runtime evidence.",
      ],
    };
  }

  const visibleRows = windowSnapshot.rows.filter((row) => row.visible);
  const knownVisibleRows = visibleRows.filter((row) => requiredApps.includes(row.name));
  const anyKnownWindow = knownVisibleRows.some((row) => row.windowCount > 0);
  const anyWindow = visibleRows.some((row) => row.windowCount > 0);

  if (!anyKnownWindow && !anyWindow && knownVisibleRows.length > 0) {
    return {
      status: "observer_limited",
      canCollectLiveUiEvidence: false,
      platform,
      reason:
        "Visible apps were found, but System Events reported zero windows for known windowed apps; local UI automation is not a trustworthy live-evidence source.",
      windowSnapshot,
      screenSnapshot,
      permissionScope: DESKTOP_UI_EVIDENCE_PERMISSION_SCOPE,
      recoveryCommands: recoveryCommands({ includeOpenSettings: true }),
      recommendations: [
        "Run Forge live rows manually or from a desktop session with Accessibility/Screen Recording permissions.",
        `Run \`${DOCTOR_COMMAND}\` for the concrete recovery checklist.`,
        "Keep Phase 8 row status as pending until final-answer, diff, build, and confirmation evidence is captured.",
      ],
    };
  }

  return {
    status: "ready",
    canCollectLiveUiEvidence: true,
    platform,
    reason: "System Events can observe at least one visible app window.",
    windowSnapshot,
    screenSnapshot,
    permissionScope: DESKTOP_UI_EVIDENCE_PERMISSION_SCOPE,
    recoveryCommands: [],
    recommendations: [],
  };
}

export function normalizeDesktopUiEvidenceRecoveryCommands(commands = [], { includeOpenSettings = false } = {}) {
  const normalized = [...commands];
  const defaults = recoveryCommands({ includeOpenSettings });
  for (const entry of defaults) {
    if (!normalized.some((existing) => existing.command === entry.command)) {
      normalized.push(entry);
    }
  }
  return normalized;
}

function recoveryCommands({ includeOpenSettings = false } = {}) {
  const commands = [
    {
      label: "diagnose desktop UI evidence",
      command: DOCTOR_COMMAND,
    },
  ];
  if (includeOpenSettings) {
    commands.push({
      label: "open relevant macOS privacy settings",
      command: DOCTOR_OPEN_SETTINGS_COMMAND,
    });
  }
  commands.push(
    {
      label: "rerun desktop UI evidence preflight",
      command: PREFLIGHT_REQUIRE_READY_COMMAND,
    },
    {
      label: "rerun disposable loop live-ready hard gate",
      command: LIVE_READY_HARD_GATE_COMMAND,
    },
  );
  return commands;
}

export function collectWindowSnapshotSafe() {
  try {
    const output = execFileSync("osascript", ["-e", windowSnapshotAppleScript()], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      timeout: 5_000,
    });
    return {
      ok: true,
      rows: parseWindowSnapshot(output),
      error: null,
    };
  } catch (error) {
    return {
      ok: false,
      rows: [],
      error: String(error.stderr || error.message || error),
    };
  }
}

export function collectScreenSnapshotSafe() {
  const dir = mkdtempSync(join(tmpdir(), "forge-ui-evidence-"));
  const file = join(dir, "screen.png");
  try {
    execFileSync("screencapture", ["-x", file], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      timeout: 5_000,
    });
    const sizeBytes = statSync(file).size;
    const { width, height } = readImageDimensions(file);
    const pixelCount = width * height;
    const compressedBytesPerPixel = pixelCount > 0 ? sizeBytes / pixelCount : null;
    const likelyBlank =
      compressedBytesPerPixel !== null &&
      compressedBytesPerPixel > 0 &&
      compressedBytesPerPixel < BLANK_SCREENSHOT_BYTES_PER_PIXEL;

    return {
      ok: true,
      width,
      height,
      sizeBytes,
      compressedBytesPerPixel,
      likelyBlank,
      error: null,
    };
  } catch (error) {
    return {
      ok: false,
      width: 0,
      height: 0,
      sizeBytes: 0,
      compressedBytesPerPixel: null,
      likelyBlank: false,
      error: String(error.stderr || error.message || error),
    };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function readImageDimensions(file) {
  try {
    const output = execFileSync("sips", ["-g", "pixelWidth", "-g", "pixelHeight", file], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      timeout: 5_000,
    });
    const width = Number.parseInt(output.match(/pixelWidth:\s*(\d+)/)?.[1] ?? "0", 10);
    const height = Number.parseInt(output.match(/pixelHeight:\s*(\d+)/)?.[1] ?? "0", 10);
    return { width, height };
  } catch {
    return { width: 0, height: 0 };
  }
}

function windowSnapshotAppleScript() {
  return `tell application "System Events"
set output to ""
repeat with p in (every process whose visible is true)
  try
    set output to output & (name of p) & "\t" & ((visible of p) as text) & "\t" & ((count of windows of p) as text) & linefeed
  end try
end repeat
return output
end tell`;
}

function parseWindowSnapshot(output) {
  return String(output)
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const [name, visible, windowCount] = line.split("\t");
      return {
        name,
        visible: visible === "true",
        windowCount: Number.parseInt(windowCount, 10) || 0,
      };
    });
}

function printHelp() {
  console.log(`Usage: node scripts/desktop-ui-evidence-preflight.mjs [--json] [--require-ready] [--skip-screen-capture]

Checks whether this local macOS session can observe desktop UI windows and screenshots well enough to collect live Forge UI evidence.

Options:
  --json                 Print machine-readable status.
  --require-ready        Exit non-zero unless live UI evidence collection appears ready.
  --skip-screen-capture  Check only Accessibility window enumeration.
  -h, --help             Show this help.
`);
}

function printHuman(result) {
  console.log("Desktop UI evidence preflight");
  console.log(`Status: ${result.status}`);
  console.log(`Live UI evidence ready: ${result.canCollectLiveUiEvidence ? "yes" : "no"}`);
  console.log(result.reason);
  if (result.recommendations.length > 0) {
    console.log("Recommendations:");
    for (const recommendation of result.recommendations) {
      console.log(`- ${recommendation}`);
    }
  }
  if (result.recoveryCommands?.length > 0) {
    console.log("Recovery commands:");
    for (const entry of result.recoveryCommands) {
      console.log(`- ${entry.label}: ${entry.command}`);
    }
  }
}

function main(argv = process.argv.slice(2)) {
  const json = argv.includes("--json");
  const requireReady = argv.includes("--require-ready");
  const skipScreenCapture = argv.includes("--skip-screen-capture");
  if (argv.includes("-h") || argv.includes("--help")) {
    printHelp();
    return 0;
  }

  const result = evaluateDesktopUiEvidencePreflight({
    screenSnapshot:
      process.platform === "darwin" && !skipScreenCapture ? collectScreenSnapshotSafe() : null,
  });
  if (json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    printHuman(result);
  }
  return requireReady && !result.canCollectLiveUiEvidence ? 1 : 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
