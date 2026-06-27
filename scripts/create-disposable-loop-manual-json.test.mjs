import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { createDisposableLoopManualTemplate } from "./create-disposable-loop-manual-json.mjs";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "create-disposable-loop-manual-json.mjs");

test("row #1 template contains exact prompt and required fields", () => {
  const template = createDisposableLoopManualTemplate({ row: "1" });

  assert.match(template["Forge prompt"], /^\/fix @src\/App\.tsx/);
  assert.equal(template["Forge final answer"], "");
  assert.equal(template["Confirmation behavior"], "");
  assert.equal(template["Screenshot or transcript reference"], "");
  assert.equal(template["Row #1 visible feedback fix result"], "");
  assert.equal(Object.hasOwn(template, "Row #2 style-only polish result"), false);
});

test("row #2 template can leave prompt empty", () => {
  const template = createDisposableLoopManualTemplate({ row: "2", includePrompt: false });

  assert.equal(template["Forge prompt"], "");
  assert.equal(Object.hasOwn(template, "Row #2 style-only polish result"), true);
});

test("all template includes all row result fields", () => {
  const template = createDisposableLoopManualTemplate({ row: "all" });

  assert.match(template["Forge prompt"], /Row #1:/);
  assert.match(template["Forge prompt"], /Row #2:/);
  assert.match(template["Forge prompt"], /Row #3:/);
  assert.equal(Object.hasOwn(template, "Row #1 visible feedback fix result"), true);
  assert.equal(Object.hasOwn(template, "Row #2 style-only polish result"), true);
  assert.equal(Object.hasOwn(template, "Row #3 command-only check result"), true);
});

test("cli writes template to file", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "forge-manual-template-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const outPath = join(dir, "manual-row-1.json");

  execFileSync(process.execPath, [scriptPath, "--row", "1", "--out", outPath], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(existsSync(outPath), true);
  const parsed = JSON.parse(readFileSync(outPath, "utf8"));
  assert.match(parsed["Forge prompt"], /^\/fix @src\/App\.tsx/);
});

test("cli rejects invalid row", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--row", "4"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.equal(result.status, 2);
  assert.match(result.stderr, /--row must be one of/);
});
