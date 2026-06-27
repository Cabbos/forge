import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { generatePhase8DisposableLoopRunbook } from "./phase8-disposable-loop-runbook.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "phase8-disposable-loop-runbook.mjs");

test("runbook reports ready project and row commands", (t) => {
  const projectPath = createRunbookProject(t);
  const result = generatePhase8DisposableLoopRunbook({
    projectPath,
    row: "1",
    manualDir: "/tmp",
    date: "2026-06-28",
  });

  assert.equal(result.readyForLiveRun, true);
  assert.equal(result.status, "pending_live_evidence");
  assert.match(result.prompt, /^\/fix @src\/App\.tsx/);
  assert.equal(result.manualPath, "/tmp/phase8-row-1-manual.json");
  assert.ok(result.commands.some((entry) => entry.command.includes("review-disposable-loop-manual-json.mjs")));
  assert.ok(result.commands.some((entry) => entry.command.includes("validate-disposable-loop-evidence.mjs")));
  assert.ok(result.commands.some((entry) => entry.command.includes("--manual-json /tmp/phase8-row-1-manual.json")));
  assert.match(result.markdown, /Phase 8 Disposable Loop Runbook - Row 1/);
});

test("cli json prints machine-readable runbook", (t) => {
  const projectPath = createRunbookProject(t);
  const output = execFileSync(
    process.execPath,
    [scriptPath, "--json", "--project", projectPath, "--row", "2", "--date", "2026-06-28"],
    {
      cwd: root,
      encoding: "utf8",
    },
  );
  const parsed = JSON.parse(output);

  assert.equal(parsed.row, "2");
  assert.equal(parsed.readyForLiveRun, true);
  assert.match(parsed.prompt, /CSS layout polish/);
  assert.match(parsed.markdown, /Row 2/);
});

test("cli markdown includes ordered commands", (t) => {
  const projectPath = createRunbookProject(t);
  const output = execFileSync(
    process.execPath,
    [scriptPath, "--markdown", "--project", projectPath, "--row", "3"],
    {
      cwd: root,
      encoding: "utf8",
    },
  );

  assert.match(output, /send this row prompt in Forge/);
  assert.match(output, /archive-disposable-loop-evidence\.mjs/);
});

test("cli rejects invalid row", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--row", "4"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /--row must be one of/);
});

function createRunbookProject(t) {
  const projectPath = mkdtempSync(join(tmpdir(), "forge-runbook-project-"));
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
  git(projectPath, ["commit", "-m", "initial runbook project"]);
  return projectPath;
}

function git(cwd, args) {
  execFileSync("git", args, {
    cwd,
    stdio: "ignore",
  });
}
