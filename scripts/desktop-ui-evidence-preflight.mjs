#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import process from "node:process";
import { pathToFileURL } from "node:url";

const KNOWN_WINDOWED_APPS = ["Google Chrome", "Codex", "Finder"];

export function evaluateDesktopUiEvidencePreflight({
  platform = process.platform,
  windowSnapshot = collectWindowSnapshotSafe(),
  requiredApps = KNOWN_WINDOWED_APPS,
} = {}) {
  if (platform !== "darwin") {
    return {
      status: "unsupported_platform",
      canCollectLiveUiEvidence: false,
      platform,
      reason: "Desktop UI evidence preflight currently checks macOS window observability only.",
      windowSnapshot,
      recommendations: ["Use the platform-specific desktop UI harness for live evidence."],
    };
  }

  if (!windowSnapshot.ok) {
    return {
      status: "window_snapshot_failed",
      canCollectLiveUiEvidence: false,
      platform,
      reason: "System Events could not enumerate visible process windows.",
      windowSnapshot,
      recommendations: [
        "Grant Accessibility permission to the controlling app or run the live row manually.",
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
      recommendations: [
        "Run Forge live rows manually or from a desktop session with Accessibility/Screen Recording permissions.",
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
    recommendations: [],
  };
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
  console.log(`Usage: node scripts/desktop-ui-evidence-preflight.mjs [--json] [--require-ready]

Checks whether this local macOS session can observe desktop UI windows well enough to collect live Forge UI evidence.

Options:
  --json           Print machine-readable status.
  --require-ready  Exit non-zero unless live UI evidence collection appears ready.
  -h, --help       Show this help.
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
}

function main(argv = process.argv.slice(2)) {
  const json = argv.includes("--json");
  const requireReady = argv.includes("--require-ready");
  if (argv.includes("-h") || argv.includes("--help")) {
    printHelp();
    return 0;
  }

  const result = evaluateDesktopUiEvidencePreflight();
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
