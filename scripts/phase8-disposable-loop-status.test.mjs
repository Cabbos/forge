import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { generatePhase8DisposableLoopStatus } from "./phase8-disposable-loop-status.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "phase8-disposable-loop-status.mjs");

test("reports row #1 as next when no archive exists", (t) => {
  const projectPath = createStatusProject(t);
  const outDir = mkdtempSync(join(tmpdir(), "forge-loop-status-out-"));
  t.after(() => rmSync(outDir, { recursive: true, force: true }));

  const result = generatePhase8DisposableLoopStatus({
    projectPath,
    outDir,
    manualDir: "/tmp",
    date: "2026-06-28",
  });

  assert.equal(result.status, "ready_for_live_row");
  assert.equal(result.uiEvidencePreflight.status, "not_checked");
  assert.equal(result.nextRow, "1");
  assert.equal(result.rows[0].status, "pending_live_evidence");
  assert.ok(result.nextCommands.some((entry) => entry.command.includes("--row 1")));
  assert.match(result.markdown, /Next row: #1/);
});

test("reports UI evidence blocker without marking project as not ready", (t) => {
  const projectPath = createStatusProject(t);
  const outDir = mkdtempSync(join(tmpdir(), "forge-loop-status-out-"));
  t.after(() => rmSync(outDir, { recursive: true, force: true }));

  const result = generatePhase8DisposableLoopStatus({
    projectPath,
    outDir,
    manualDir: "/tmp",
    date: "2026-06-28",
    uiEvidencePreflight: {
      status: "screen_capture_limited",
      canCollectLiveUiEvidence: false,
      platform: "darwin",
      reason: "blank screenshot",
      windowSnapshot: null,
      screenSnapshot: null,
      recoveryCommands: [
        {
          label: "diagnose desktop UI evidence",
          command: "node scripts/desktop-ui-evidence-doctor.mjs --markdown",
        },
        {
          label: "open relevant macOS privacy settings",
          command: "node scripts/desktop-ui-evidence-doctor.mjs --markdown --open-settings",
        },
      ],
      recommendations: [],
    },
  });

  assert.equal(result.status, "ui_evidence_not_ready");
  assert.equal(result.readyForLiveRun, false);
  assert.equal(result.rows[0].status, "ui_evidence_not_ready");
  assert.equal(result.rows[0].runbook.uiEvidencePreflight.status, "screen_capture_limited");
  assert.ok(result.rows[0].runbook.recoveryCommands.some((entry) => entry.command.includes("--open-settings")));
  assert.ok(result.recoveryCommands.some((entry) => entry.command.includes("--open-settings")));
  assert.ok(result.nextCommands.some((entry) => entry.command.includes("desktop-ui-evidence-preflight.mjs")));
  assert.ok(result.nextCommands.some((entry) => entry.command.includes("desktop-ui-evidence-doctor.mjs")));
  assert.match(result.nextStep, /trusted desktop session/);
  assert.match(result.markdown, /Recovery commands:/);
  assert.match(result.markdown, /desktop-ui-evidence-doctor\.mjs --markdown --open-settings/);
});

test("skips completed rows and reports the next incomplete row", (t) => {
  const projectPath = createStatusProject(t);
  const outDir = mkdtempSync(join(tmpdir(), "forge-loop-status-out-"));
  t.after(() => rmSync(outDir, { recursive: true, force: true }));
  writeCompleteArchive(outDir, "1");

  const result = generatePhase8DisposableLoopStatus({
    projectPath,
    outDir,
    manualDir: "/tmp",
    date: "2026-06-28",
  });

  assert.equal(result.status, "ready_for_live_row");
  assert.equal(result.nextRow, "2");
  assert.equal(result.rows[0].status, "archived_complete");
  assert.equal(result.rows[1].status, "pending_live_evidence");
  assert.ok(result.nextCommands.some((entry) => entry.command.includes("--row 2")));
});

test("reports complete when all row validations are archived", (t) => {
  const projectPath = createStatusProject(t);
  const outDir = mkdtempSync(join(tmpdir(), "forge-loop-status-out-"));
  t.after(() => rmSync(outDir, { recursive: true, force: true }));
  for (const row of ["1", "2", "3"]) writeCompleteArchive(outDir, row);

  const result = generatePhase8DisposableLoopStatus({
    projectPath,
    outDir,
    manualDir: "/tmp",
    date: "2026-06-28",
  });

  assert.equal(result.status, "complete");
  assert.equal(result.nextRow, null);
  assert.deepEqual(result.nextCommands, []);
  assert.match(result.markdown, /All Phase 8 disposable loop rows/);
});

test("cli json prints machine-readable status", (t) => {
  const projectPath = createStatusProject(t);
  const outDir = mkdtempSync(join(tmpdir(), "forge-loop-status-out-"));
  t.after(() => rmSync(outDir, { recursive: true, force: true }));

  const output = execFileSync(
    process.execPath,
    [
      scriptPath,
      "--json",
      "--project",
      projectPath,
      "--out-dir",
      outDir,
      "--date",
      "2026-06-28",
      "--skip-ui-preflight",
    ],
    {
      cwd: root,
      encoding: "utf8",
    },
  );
  const parsed = JSON.parse(output);

  assert.equal(parsed.status, "ready_for_live_row");
  assert.equal(parsed.nextRow, "1");
  assert.equal(parsed.rows.length, 3);
  assert.deepEqual(parsed.recoveryCommands, []);
});

function writeCompleteArchive(outDir, row) {
  const base = `2026-06-28-row-${row}`;
  writeFileSync(
    join(outDir, `${base}.validation.json`),
    `${JSON.stringify({ status: "complete", pass: true, row }, null, 2)}\n`,
  );
  writeFileSync(join(outDir, `${base}.evidence.json`), "{}\n");
  writeFileSync(join(outDir, `${base}.md`), "# evidence\n");
}

function createStatusProject(t) {
  const projectPath = mkdtempSync(join(tmpdir(), "forge-loop-status-project-"));
  t.after(() => rmSync(projectPath, { recursive: true, force: true }));

  mkdirSync(join(projectPath, "src"), { recursive: true });
  writeFileSync(
    join(projectPath, "package.json"),
    `${JSON.stringify(
      {
        name: "forge-test-app",
        version: "0.1.0",
        private: true,
        type: "module",
        scripts: {
          build: "node -e \"console.log('build ok')\"",
        },
      },
      null,
      2,
    )}\n`,
  );
  writeFileSync(join(projectPath, "src", "App.tsx"), "export default function App() {\n  return <button>Demo</button>;\n}\n");
  writeFileSync(join(projectPath, "src", "styles.css"), "button { opacity: 1; }\n");
  git(projectPath, ["init"]);
  git(projectPath, ["config", "user.email", "forge@example.local"]);
  git(projectPath, ["config", "user.name", "Forge Test"]);
  git(projectPath, ["add", "."]);
  git(projectPath, ["commit", "-m", "initial status project"]);
  return projectPath;
}

function git(cwd, args) {
  execFileSync("git", args, {
    cwd,
    stdio: "ignore",
  });
}
