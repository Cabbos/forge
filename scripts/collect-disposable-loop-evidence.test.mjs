import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { collectDisposableLoopEvidence } from "./collect-disposable-loop-evidence.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "collect-disposable-loop-evidence.mjs");

test("reports a ready clean project with no changes yet", (t) => {
  const projectPath = createEvidenceProject(t);

  const result = collectDisposableLoopEvidence({ projectPath, date: "2026-06-27" });

  assert.equal(result.status, "no_changes_yet");
  assert.equal(result.preflight.readyForLoop, true);
  assert.equal(result.git.clean, true);
  assert.deepEqual(result.git.changedFiles, []);
  assert.match(result.markdown, /Status: no_changes_yet/);
  assert.match(result.markdown, /Forge final answer:/);
});

test("captures changed files and diff summary", (t) => {
  const projectPath = createEvidenceProject(t);
  appendFileSync(join(projectPath, "src", "styles.css"), "\nbutton:active { transform: scale(0.97); }\n");

  const result = collectDisposableLoopEvidence({ projectPath, row: "2", date: "2026-06-27" });

  assert.equal(result.status, "changes_detected");
  assert.equal(result.git.clean, false);
  assert.deepEqual(result.git.changedFiles.map((entry) => entry.file), ["src/styles.css"]);
  assert.match(result.git.unstagedNameStatus, /M\tsrc\/styles\.css/);
  assert.match(result.git.unstagedStat, /src\/styles\.css/);
  assert.match(result.markdown, /Row #2 style-only polish result:/);
});

test("run-build captures successful build output", (t) => {
  const projectPath = createEvidenceProject(t);

  const result = collectDisposableLoopEvidence({ projectPath, runBuild: true, date: "2026-06-27" });

  assert.equal(result.status, "no_changes_yet");
  assert.equal(result.build.ran, true);
  assert.equal(result.build.success, true);
  assert.equal(result.build.exitCode, 0);
  assert.match(result.build.outputTail, /build ok/);
  assert.match(result.markdown, /passed: `npm --prefix/);
});

test("cli markdown mode emits evidence template", (t) => {
  const projectPath = createEvidenceProject(t);
  appendFileSync(join(projectPath, "src", "App.tsx"), "\nexport const touched = true;\n");

  const output = execFileSync(
    process.execPath,
    [scriptPath, "--markdown", "--project", projectPath, "--row", "1"],
    {
      cwd: root,
      encoding: "utf8",
    },
  );

  assert.match(output, /Phase 8 Disposable Loop Evidence/);
  assert.match(output, /src\/App\.tsx/);
  assert.match(output, /Row #1 visible feedback fix result:/);
});

function createEvidenceProject(t) {
  const projectPath = mkdtempSync(join(tmpdir(), "forge-loop-evidence-"));
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
  git(projectPath, ["commit", "-m", "initial evidence project"]);

  return projectPath;
}

function git(cwd, args) {
  execFileSync("git", args, {
    cwd,
    stdio: "ignore",
  });
}
