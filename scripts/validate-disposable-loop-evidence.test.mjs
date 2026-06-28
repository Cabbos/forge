import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { appendFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { validateDisposableLoopEvidence } from "./validate-disposable-loop-evidence.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "validate-disposable-loop-evidence.mjs");

test("clean collector output is pending but non-blocking by default", () => {
  const result = validateDisposableLoopEvidence(baseEvidence({ status: "no_changes_yet", changedFiles: [] }));

  assert.equal(result.status, "pending_live_evidence");
  assert.equal(result.pass, true);
  assert.ok(result.issues.some((issue) => issue.code === "live_rows_not_run"));
});

test("row #1 passes when code change, build, and manual fields are present", () => {
  const result = validateDisposableLoopEvidence(
    baseEvidence({
      row: "1",
      status: "changes_detected",
      changedFiles: [{ status: "M ", file: "src/styles.css", raw: " M src/styles.css" }],
      build: { ran: true, success: true, exitCode: 0, command: "npm run build", outputTail: "ok" },
      fillManualFields: true,
    }),
    { row: "1", requireComplete: true },
  );

  assert.equal(result.status, "complete");
  assert.equal(result.pass, true);
  assert.deepEqual(result.blockingIssues, []);
});

test("row #2 fails strict validation when non-style files changed", () => {
  const result = validateDisposableLoopEvidence(
    baseEvidence({
      row: "2",
      status: "changes_detected",
      changedFiles: [{ status: "M ", file: "src/App.tsx", raw: " M src/App.tsx" }],
      fillManualFields: true,
    }),
    { row: "2", requireComplete: true },
  );

  assert.equal(result.status, "incomplete");
  assert.equal(result.pass, false);
  assert.ok(result.issues.some((issue) => issue.code === "row2_non_style_file"));
});

test("row #3 fails strict validation when command-only run changes files", () => {
  const result = validateDisposableLoopEvidence(
    baseEvidence({
      row: "3",
      status: "changes_detected",
      changedFiles: [{ status: "M ", file: "src/styles.css", raw: " M src/styles.css" }],
      build: { ran: true, success: true, exitCode: 0, command: "npm run build", outputTail: "ok" },
      fillManualFields: true,
    }),
    { row: "3", requireComplete: true },
  );

  assert.equal(result.pass, false);
  assert.ok(result.issues.some((issue) => issue.code === "row3_changed_files"));
});

test("cli require-complete exits nonzero for incomplete evidence json", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "forge-validate-evidence-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const evidencePath = join(dir, "evidence.json");
  writeFileSync(evidencePath, `${JSON.stringify(baseEvidence({ row: "1", status: "no_changes_yet", changedFiles: [] }))}\n`);

  const result = spawnSync(process.execPath, [scriptPath, "--json", "--evidence-json", evidencePath, "--row", "1", "--require-complete"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 1);
  const parsed = JSON.parse(result.stdout);
  assert.equal(parsed.pass, false);
  assert.ok(parsed.issues.some((issue) => issue.code === "row1_no_changes"));
});

test("cli json mode reports pending evidence as pass by default", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "forge-validate-evidence-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const evidencePath = join(dir, "evidence.json");
  writeFileSync(evidencePath, `${JSON.stringify(baseEvidence({ status: "no_changes_yet", changedFiles: [] }))}\n`);

  const output = execFileSync(process.execPath, [scriptPath, "--json", "--evidence-json", evidencePath], {
    cwd: root,
    encoding: "utf8",
  });
  const parsed = JSON.parse(output);

  assert.equal(parsed.status, "pending_live_evidence");
  assert.equal(parsed.pass, true);
});

test("cli can validate strict project evidence with manual json", (t) => {
  const { projectPath, manualPath } = createStrictProjectEvidence(t);

  const output = execFileSync(
    process.execPath,
    [
      scriptPath,
      "--json",
      "--project",
      projectPath,
      "--manual-json",
      manualPath,
      "--row",
      "1",
      "--run-build",
      "--require-complete",
    ],
    {
      cwd: root,
      encoding: "utf8",
    },
  );
  const parsed = JSON.parse(output);

  assert.equal(parsed.status, "complete");
  assert.equal(parsed.pass, true);
});

function baseEvidence({
  row = "all",
  status = "changes_detected",
  changedFiles = [],
  build = { ran: false, success: null, exitCode: null, command: "npm run build", outputTail: "" },
  fillManualFields = false,
} = {}) {
  return {
    status,
    row,
    projectPath: "/tmp/forge-test-app",
    preflight: {
      status: "ready",
      readyForLoop: true,
      issues: [],
      hasBuildScript: true,
      requiredFiles: [],
    },
    git: {
      clean: changedFiles.length === 0,
      changedFiles,
      branchStatus: "## HEAD (no branch)",
      statusShort: changedFiles.map((entry) => entry.raw).join("\n"),
      unstagedNameStatus: "",
      stagedNameStatus: "",
      unstagedStat: "",
      stagedStat: "",
    },
    build,
    manualFields: [
      { label: "Forge prompt", value: fillManualFields ? "prompt" : "" },
      { label: "Forge final answer", value: fillManualFields ? "final" : "" },
      { label: "Confirmation behavior", value: fillManualFields ? "none" : "" },
      { label: "Screenshot or transcript reference", value: fillManualFields ? "screenshot" : "" },
    ],
  };
}

function createStrictProjectEvidence(t) {
  const dir = mkdtempSync(join(tmpdir(), "forge-validate-project-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));

  const projectPath = join(dir, "forge-test-app");
  const manualPath = join(dir, "manual.json");
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
  git(projectPath, ["commit", "-m", "initial validation project"]);
  appendFileSync(join(projectPath, "src", "styles.css"), "\nbutton:active { transform: scale(0.97); }\n");
  writeFileSync(
    manualPath,
    `${JSON.stringify(
      {
        "Forge prompt": "/fix @src/App.tsx",
        "Forge final answer": "Changed feedback styles and build passed.",
        "Confirmation behavior": "No confirmation card appeared under Full Access.",
        "Screenshot or transcript reference": "screenshot-row-1.png",
      },
      null,
      2,
    )}\n`,
  );

  return { projectPath, manualPath };
}

function git(cwd, args) {
  execFileSync("git", args, {
    cwd,
    stdio: "ignore",
  });
}
