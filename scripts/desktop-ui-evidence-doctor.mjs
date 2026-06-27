#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import process from "node:process";
import { pathToFileURL } from "node:url";

import {
  collectScreenSnapshotSafe,
  evaluateDesktopUiEvidencePreflight,
} from "./desktop-ui-evidence-preflight.mjs";
import { DESKTOP_UI_EVIDENCE_PERMISSION_SCOPE } from "./desktop-ui-evidence-permission-scope.mjs";

const SCREEN_RECORDING_SETTINGS_URL =
  "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
const ACCESSIBILITY_SETTINGS_URL =
  "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

export function diagnoseDesktopUiEvidence({
  preflight = uncheckedPreflight(),
  controllingApps = ["Codex", "Warp", "Terminal"],
} = {}) {
  const blockers = [];
  const screenCaptureBlocked =
    preflight.status === "screen_capture_failed" ||
    preflight.status === "screen_capture_limited" ||
    preflight.screenSnapshot?.likelyBlank === true ||
    preflight.screenSnapshot?.ok === false;
  const accessibilityBlocked =
    preflight.status === "window_snapshot_failed" ||
    preflight.status === "observer_limited" ||
    windowSnapshotLooksLimited(preflight.windowSnapshot);

  if (screenCaptureBlocked) {
    blockers.push(screenRecordingBlocker({ preflight, controllingApps }));
  }
  if (accessibilityBlocked) {
    blockers.push(accessibilityBlocker({ preflight, controllingApps }));
  }

  const status = statusFor({ preflight, screenCaptureBlocked, accessibilityBlocked });
  const result = {
    status,
    canCollectLiveUiEvidence: preflight.canCollectLiveUiEvidence === true && blockers.length === 0,
    preflightStatus: preflight.status,
    platform: preflight.platform,
    blockers,
    commands: commandsFor({ blockers }),
    permissionScope: DESKTOP_UI_EVIDENCE_PERMISSION_SCOPE,
    nextStep: nextStepFor({ status }),
    openSettings: null,
    preflight,
    markdown: "",
  };
  result.markdown = renderMarkdown(result);
  return result;
}

function uncheckedPreflight() {
  return {
    status: "not_checked",
    canCollectLiveUiEvidence: null,
    platform: process.platform,
    reason: "Desktop UI evidence preflight was not supplied.",
    windowSnapshot: null,
    screenSnapshot: null,
    recommendations: ["Run `node scripts/desktop-ui-evidence-preflight.mjs --json --require-ready` first."],
  };
}

function windowSnapshotLooksLimited(windowSnapshot) {
  if (!windowSnapshot) return false;
  if (!windowSnapshot.ok) return true;
  const visibleRows = Array.isArray(windowSnapshot.rows)
    ? windowSnapshot.rows.filter((row) => row.visible)
    : [];
  return visibleRows.length > 0 && visibleRows.every((row) => Number(row.windowCount) === 0);
}

function screenRecordingBlocker({ preflight, controllingApps }) {
  const screen = preflight.screenSnapshot ?? {};
  const evidence = [
    `preflight status: ${preflight.status}`,
    screen.ok === false ? `screen capture error: ${screen.error ?? "unknown"}` : null,
    screen.likelyBlank === true ? "screen capture likely blank: true" : null,
    typeof screen.sizeBytes === "number" ? `screen bytes: ${screen.sizeBytes}` : null,
    typeof screen.compressedBytesPerPixel === "number"
      ? `compressed bytes per pixel: ${screen.compressedBytesPerPixel.toFixed(4)}`
      : null,
  ].filter(Boolean);

  return {
    id: "screen_recording_permission",
    severity: "blocking",
    title: "Screen Recording access is not trustworthy",
    evidence,
    actions: [
      `Open Screen Recording settings and grant access to the app running these scripts: ${controllingApps.join(", ")}.`,
      "Quit and reopen the controlling app after changing Screen Recording permission.",
      "Rerun `node scripts/desktop-ui-evidence-preflight.mjs --json --require-ready`.",
    ],
  };
}

function accessibilityBlocker({ preflight, controllingApps }) {
  const windowSnapshot = preflight.windowSnapshot ?? {};
  const visibleRows = Array.isArray(windowSnapshot.rows)
    ? windowSnapshot.rows.filter((row) => row.visible)
    : [];
  const evidence = [
    `preflight status: ${preflight.status}`,
    windowSnapshot.ok === false ? `window snapshot error: ${windowSnapshot.error ?? "unknown"}` : null,
    visibleRows.length > 0 ? `visible apps: ${visibleRows.map((row) => `${row.name}:${row.windowCount}`).join(", ")}` : null,
  ].filter(Boolean);

  return {
    id: "accessibility_permission",
    severity: "blocking",
    title: "Accessibility window observation is not trustworthy",
    evidence,
    actions: [
      `Open Accessibility settings and grant access to the app running these scripts: ${controllingApps.join(", ")}.`,
      "Quit and reopen the controlling app after changing Accessibility permission.",
      "Rerun `node scripts/desktop-ui-evidence-preflight.mjs --json --require-ready`.",
    ],
  };
}

function statusFor({ preflight, screenCaptureBlocked, accessibilityBlocked }) {
  if (preflight.status === "ready" && !screenCaptureBlocked && !accessibilityBlocked) return "ready";
  if (screenCaptureBlocked && accessibilityBlocked) return "needs_screen_recording_and_accessibility";
  if (screenCaptureBlocked) return "needs_screen_recording";
  if (accessibilityBlocked) return "needs_accessibility";
  if (preflight.platform !== "darwin") return "unsupported_platform";
  return "needs_manual_review";
}

function commandsFor({ blockers }) {
  const commands = [];
  if (blockers.some((blocker) => blocker.id === "screen_recording_permission")) {
    commands.push({
      label: "open Screen Recording settings",
      command: `open '${SCREEN_RECORDING_SETTINGS_URL}'`,
      kind: "open_settings",
      url: SCREEN_RECORDING_SETTINGS_URL,
    });
  }
  if (blockers.some((blocker) => blocker.id === "accessibility_permission")) {
    commands.push({
      label: "open Accessibility settings",
      command: `open '${ACCESSIBILITY_SETTINGS_URL}'`,
      kind: "open_settings",
      url: ACCESSIBILITY_SETTINGS_URL,
    });
  }
  commands.push({
    label: "rerun desktop UI evidence preflight",
    command: "node scripts/desktop-ui-evidence-preflight.mjs --json --require-ready",
  });
  commands.push({
    label: "rerun disposable loop status",
    command: "node scripts/phase8-disposable-loop-status.mjs --json",
  });
  commands.push({
    label: "rerun disposable loop live-ready hard gate",
    command: "node scripts/phase8-disposable-loop-status.mjs --json --require-live-ready",
  });
  return commands;
}

function nextStepFor({ status }) {
  if (status === "ready") {
    return "Desktop UI evidence is ready; run the next Phase 8 disposable loop row.";
  }
  if (status === "needs_screen_recording_and_accessibility") {
    return "Grant Screen Recording and Accessibility, restart the controlling app, then rerun the preflight.";
  }
  if (status === "needs_screen_recording") {
    return "Grant Screen Recording, restart the controlling app, then rerun the preflight.";
  }
  if (status === "needs_accessibility") {
    return "Grant Accessibility, restart the controlling app, then rerun the preflight.";
  }
  return "Run the preflight in a trusted desktop session or collect manual evidence outside this limited observer.";
}

export function openDesktopUiEvidenceSettings({ diagnosis, runner = execFileSync } = {}) {
  const settingsCommands = (diagnosis?.commands ?? []).filter(
    (entry) => entry.kind === "open_settings" && typeof entry.url === "string",
  );
  const opened = [];
  for (const entry of settingsCommands) {
    runner("open", [entry.url], {
      stdio: "ignore",
      timeout: 5_000,
    });
    opened.push({
      label: entry.label,
      url: entry.url,
    });
  }
  return {
    openedCount: opened.length,
    opened,
  };
}

function renderMarkdown(result) {
  const blockers = result.blockers.length > 0
    ? result.blockers.map((blocker) => {
        const evidence = blocker.evidence.map((entry) => `  - ${entry}`).join("\n");
        const actions = blocker.actions.map((entry) => `  - ${entry}`).join("\n");
        return `- ${blocker.title}\n\n  Evidence:\n${evidence}\n\n  Actions:\n${actions}`;
      }).join("\n\n")
    : "- none";
  const commands = result.commands.map((entry, index) => `${index + 1}. ${entry.label}\n\n   \`${entry.command}\``).join("\n\n");
  return `## Desktop UI Evidence Doctor

Status: ${result.status}
Preflight: ${result.preflightStatus}
Live UI evidence ready: ${result.canCollectLiveUiEvidence ? "yes" : "no"}

Permission scope: ${result.permissionScope.note}

Blockers:

${blockers}

Commands:

${commands}

Opened settings: ${result.openSettings ? result.openSettings.openedCount : 0}

Next step: ${result.nextStep}
`;
}

function printHelp() {
  console.log(`Usage: node scripts/desktop-ui-evidence-doctor.mjs [--json|--markdown] [--require-ready] [--open-settings]

Diagnoses why local desktop UI evidence cannot be trusted and prints concrete permission recovery commands.

Options:
  --json           Print machine-readable diagnosis.
  --markdown       Print markdown diagnosis.
  --require-ready  Exit non-zero unless live UI evidence collection appears ready.
  --open-settings  Open the relevant macOS Privacy & Security settings panes for the current blockers.
  -h, --help       Show this help.
`);
}

function collectCurrentPreflight() {
  return evaluateDesktopUiEvidencePreflight({
    screenSnapshot: process.platform === "darwin" ? collectScreenSnapshotSafe() : null,
  });
}

function main(argv = process.argv.slice(2)) {
  const json = argv.includes("--json");
  const markdown = argv.includes("--markdown");
  const requireReady = argv.includes("--require-ready");
  const shouldOpenSettings = argv.includes("--open-settings");
  if (argv.includes("-h") || argv.includes("--help")) {
    printHelp();
    return 0;
  }

  const result = diagnoseDesktopUiEvidence({ preflight: collectCurrentPreflight() });
  if (shouldOpenSettings) {
    result.openSettings = openDesktopUiEvidenceSettings({ diagnosis: result });
    result.markdown = renderMarkdown(result);
  }
  if (json) {
    console.log(JSON.stringify(result, null, 2));
  } else if (markdown || !json) {
    console.log(result.markdown);
  }
  return requireReady && !result.canCollectLiveUiEvidence ? 1 : 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
