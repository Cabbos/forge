import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { createDisposableLoopManualTemplate } from "./create-disposable-loop-manual-json.mjs";
import { finalizeDisposableLoopRow } from "./finalize-disposable-loop-row.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "finalize-disposable-loop-row.mjs");

test("reports pending when manual evidence is missing", () => {
  const result = finalizeDisposableLoopRow({ row: "1", requireComplete: false });

  assert.equal(result.status, "pending_manual_evidence");
  assert.equal(result.manualReview.pass, false);
  assert.equal(result.archive, null);
});

test("dry-run finalizes complete row evidence without writing archive files", (t) => {
  const { projectPath, manualValues, outDir } = createCompleteRowProject(t);

  const result = finalizeDisposableLoopRow({
    projectPath,
    row: "1",
    manualValues,
    runBuild: true,
    dryRun: true,
    requireComplete: true,
    outDir,
    date: "2026-06-28",
  });

  assert.equal(result.status, "dry_run_ready");
  assert.equal(result.manualReview.pass, true);
  assert.equal(result.archive.validationStatus, "complete");
  assert.equal(existsSync(join(outDir, "2026-06-28-row-1.validation.json")), false);
});

test("cli require-complete exits nonzero when manual evidence is missing", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--json", "--row", "1", "--require-complete", "--dry-run"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 1);
  const parsed = JSON.parse(result.stdout);
  assert.equal(parsed.status, "manual_review_failed");
  assert.equal(parsed.archive, null);
});

test("cli archives complete row evidence when not dry-run", (t) => {
  const { projectPath, manualValues, manualPath, outDir } = createCompleteRowProject(t);
  writeFileSync(manualPath, `${JSON.stringify(manualValues, null, 2)}\n`);

  const output = execFileSync(
    process.execPath,
    [
      scriptPath,
      "--json",
      "--project",
      projectPath,
      "--row",
      "1",
      "--manual-json",
      manualPath,
      "--run-build",
      "--require-complete",
      "--out-dir",
      outDir,
      "--date",
      "2026-06-28",
    ],
    {
      cwd: root,
      encoding: "utf8",
    },
  );
  const parsed = JSON.parse(output);

  assert.equal(parsed.status, "archived");
  assert.equal(parsed.archive.validationStatus, "complete");
  assert.equal(existsSync(join(outDir, "2026-06-28-row-1.validation.json")), true);
});

function createCompleteRowProject(t) {
  const dir = mkdtempSync(join(tmpdir(), "forge-finalize-row-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));

  const projectPath = join(dir, "forge-test-app");
  const outDir = join(dir, "out");
  const manualPath = join(dir, "manual.json");
  mkdirSync(join(projectPath, "src"), { recursive: true });
  mkdirSync(outDir, { recursive: true });
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
  git(projectPath, ["commit", "-m", "initial finalize project"]);
  appendFileSync(join(projectPath, "src", "styles.css"), "\nbutton:active { transform: scale(0.97); }\n");

  const manualValues = {
    ...createDisposableLoopManualTemplate({ row: "1" }),
    "Forge final answer": "Added visible click feedback and build passed.",
    "Confirmation behavior": "No confirmation card appeared after Full Access.",
    "Screenshot or transcript reference": "row 1 transcript",
    "Row #1 visible feedback fix result": "Button now visibly depresses on click.",
  };

  return { projectPath, manualValues, manualPath, outDir };
}

function git(cwd, args) {
  execFileSync("git", args, {
    cwd,
    stdio: "ignore",
  });
}
