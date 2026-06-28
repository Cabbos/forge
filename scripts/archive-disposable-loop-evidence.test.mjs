import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { archiveDisposableLoopEvidence } from "./archive-disposable-loop-evidence.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "archive-disposable-loop-evidence.mjs");

test("dry-run reports archive paths without writing files", (t) => {
  const { projectPath, outDir } = createArchiveProject(t);

  const result = archiveDisposableLoopEvidence({ projectPath, outDir, row: "all", date: "2026-06-28", dryRun: true });

  assert.equal(result.status, "dry_run_ready");
  assert.equal(existsSync(result.paths.markdown), false);
  assert.equal(result.paths.markdown.endsWith("2026-06-28-row-all.md"), true);
});

test("archives complete row #1 evidence", (t) => {
  const { projectPath, outDir } = createArchiveProject(t);
  appendFileSync(join(projectPath, "src", "styles.css"), "\nbutton:active { transform: scale(0.97); }\n");

  const result = archiveDisposableLoopEvidence({
    projectPath,
    outDir,
    row: "1",
    date: "2026-06-28",
    runBuild: true,
    requireComplete: true,
    manualValues: completeManualValues(),
  });

  assert.equal(result.status, "archived");
  assert.equal(result.validationPass, true);
  assert.equal(existsSync(result.paths.evidenceJson), true);
  assert.equal(existsSync(result.paths.markdown), true);
  assert.equal(existsSync(result.paths.validationJson), true);
  assert.match(readFileSync(result.paths.markdown, "utf8"), /Forge final answer: Changed feedback styles and build passed\./);
  assert.equal(JSON.parse(readFileSync(result.paths.validationJson, "utf8")).status, "complete");
});

test("strict archive refuses incomplete evidence", (t) => {
  const { projectPath, outDir } = createArchiveProject(t);

  const result = archiveDisposableLoopEvidence({
    projectPath,
    outDir,
    row: "1",
    date: "2026-06-28",
    requireComplete: true,
  });

  assert.equal(result.status, "validation_failed");
  assert.equal(result.validationPass, false);
  assert.equal(existsSync(result.paths.markdown), false);
});

test("cli archives complete evidence with manual json", (t) => {
  const { projectPath, outDir, tempDir } = createArchiveProject(t);
  appendFileSync(join(projectPath, "src", "styles.css"), "\nbutton:active { transform: scale(0.97); }\n");
  const manualPath = join(tempDir, "manual.json");
  writeFileSync(manualPath, `${JSON.stringify(completeManualValues(), null, 2)}\n`);

  const output = execFileSync(
    process.execPath,
    [
      scriptPath,
      "--json",
      "--project",
      projectPath,
      "--out-dir",
      outDir,
      "--row",
      "1",
      "--date",
      "2026-06-28",
      "--manual-json",
      manualPath,
      "--run-build",
      "--require-complete",
    ],
    {
      cwd: root,
      encoding: "utf8",
    },
  );
  const parsed = JSON.parse(output);

  assert.equal(parsed.status, "archived");
  assert.equal(existsSync(parsed.paths.markdown), true);
});

test("cli exits nonzero when strict validation fails", (t) => {
  const { projectPath, outDir } = createArchiveProject(t);
  const result = spawnSync(
    process.execPath,
    [scriptPath, "--json", "--project", projectPath, "--out-dir", outDir, "--row", "1", "--require-complete"],
    {
      cwd: root,
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 1);
  assert.equal(JSON.parse(result.stdout).status, "validation_failed");
});

function createArchiveProject(t) {
  const tempDir = mkdtempSync(join(tmpdir(), "forge-archive-evidence-"));
  t.after(() => rmSync(tempDir, { recursive: true, force: true }));

  const projectPath = join(tempDir, "forge-test-app");
  const outDir = join(tempDir, "evidence");
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
  git(projectPath, ["commit", "-m", "initial archive project"]);

  return { projectPath, outDir, tempDir };
}

function completeManualValues() {
  return {
    "Forge prompt": "/fix @src/App.tsx",
    "Forge final answer": "Changed feedback styles and build passed.",
    "Confirmation behavior": "No confirmation card appeared under Full Access.",
    "Screenshot or transcript reference": "screenshot-row-1.png",
  };
}

function git(cwd, args) {
  execFileSync("git", args, {
    cwd,
    stdio: "ignore",
  });
}
