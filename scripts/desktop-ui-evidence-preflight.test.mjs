import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { join } from "node:path";
import test from "node:test";

import { evaluateDesktopUiEvidencePreflight } from "./desktop-ui-evidence-preflight.mjs";

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
  assert.match(result.reason, /zero windows/);
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
  });

  assert.equal(result.status, "ready");
  assert.equal(result.canCollectLiveUiEvidence, true);
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
});

test("cli json emits a machine-readable status", () => {
  const output = execFileSync(process.execPath, [scriptPath, "--json"], {
    cwd: root,
    encoding: "utf8",
  });
  const parsed = JSON.parse(output);

  assert.equal(typeof parsed.status, "string");
  assert.equal(typeof parsed.canCollectLiveUiEvidence, "boolean");
  assert.equal(parsed.platform, process.platform);
});

test("require-ready exits nonzero when the current observer is not ready", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--require-ready"], {
    cwd: root,
    encoding: "utf8",
  });

  if (result.status === 0) {
    assert.match(result.stdout, /Live UI evidence ready: yes/);
  } else {
    assert.match(result.stdout, /Live UI evidence ready: no/);
  }
});
