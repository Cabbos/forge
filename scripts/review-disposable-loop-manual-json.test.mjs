import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { createDisposableLoopManualTemplate } from "./create-disposable-loop-manual-json.mjs";
import { reviewDisposableLoopManualJson } from "./review-disposable-loop-manual-json.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "review-disposable-loop-manual-json.mjs");

test("passes complete row manual evidence", () => {
  const manualValues = {
    ...createDisposableLoopManualTemplate({ row: "1" }),
    "Forge final answer": "Added active/pressed feedback and build passed.",
    "Confirmation behavior": "No confirmation card appeared in Full Access.",
    "Screenshot or transcript reference": "manual screenshot row 1",
    "Row #1 visible feedback fix result": "Button now visibly depresses on click.",
  };

  const result = reviewDisposableLoopManualJson({ row: "1", manualValues, requireComplete: true });

  assert.equal(result.status, "complete");
  assert.equal(result.pass, true);
  assert.deepEqual(result.issues, []);
});

test("catches prompt mismatch and placeholders", () => {
  const manualValues = {
    ...createDisposableLoopManualTemplate({ row: "2" }),
    "Forge prompt": "wrong prompt",
    "Forge final answer": "TODO",
    "Confirmation behavior": "done",
    "Screenshot or transcript reference": "row 2 screenshot",
    "Row #2 style-only polish result": "changed styles",
  };

  const result = reviewDisposableLoopManualJson({ row: "2", manualValues, requireComplete: true });

  assert.equal(result.status, "incomplete");
  assert.equal(result.pass, false);
  assert.ok(result.issues.some((issue) => issue.code === "prompt_mismatch"));
  assert.ok(result.issues.some((issue) => issue.code === "field_placeholder" && issue.field === "Forge final answer"));
});

test("warns on unexpected fields", () => {
  const manualValues = {
    ...createDisposableLoopManualTemplate({ row: "3" }),
    "Forge final answer": "Build passed.",
    "Confirmation behavior": "No confirmation card.",
    "Screenshot or transcript reference": "row 3 transcript",
    "Row #3 command-only check result": "No files changed.",
    "Row #1 visible feedback fix result": "wrong row",
  };

  const result = reviewDisposableLoopManualJson({ row: "3", manualValues, requireComplete: true });

  assert.equal(result.pass, true);
  assert.equal(result.warnings.length, 1);
  assert.equal(result.warnings[0].code, "unexpected_field");
});

test("cli require-complete exits nonzero for an empty template", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--json", "--row", "1", "--require-complete"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 1);
  const parsed = JSON.parse(result.stdout);
  assert.equal(parsed.pass, false);
  assert.ok(parsed.issues.some((issue) => issue.code === "field_empty"));
});

test("cli accepts a filled manual json file", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "forge-manual-review-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const manualPath = join(dir, "manual.json");
  writeFileSync(
    manualPath,
    `${JSON.stringify(
      {
        ...createDisposableLoopManualTemplate({ row: "1" }),
        "Forge final answer": "Added click feedback and build passed.",
        "Confirmation behavior": "No confirmation card appeared.",
        "Screenshot or transcript reference": "row 1 screenshot",
        "Row #1 visible feedback fix result": "Button feedback is visible.",
      },
      null,
      2,
    )}\n`,
  );

  const output = execFileSync(process.execPath, [scriptPath, "--json", "--row", "1", "--manual-json", manualPath, "--require-complete"], {
    cwd: root,
    encoding: "utf8",
  });
  const parsed = JSON.parse(output);

  assert.equal(parsed.status, "complete");
  assert.equal(parsed.pass, true);
});
