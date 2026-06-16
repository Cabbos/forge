import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

const root = new URL("..", import.meta.url).pathname;
const scriptPath = join(root, "scripts", "acceptance.sh");

test("acceptance script dry-run lists the final product gates", () => {
  assert.equal(existsSync(scriptPath), true, "scripts/acceptance.sh should exist");

  const output = execFileSync(scriptPath, ["--dry-run"], {
    cwd: root,
    encoding: "utf8",
  });

  assert.match(output, /npm run build:desktop/);
  assert.match(output, /npm run build:website/);
  assert.match(output, /npm run test:eval/);
  assert.match(output, /resume\.spec\.ts/);
  assert.match(output, /workbench\.spec\.ts/);
  assert.match(output, /a2a-confirm-runtime\.spec\.ts/);
  assert.match(output, /acceptance\.spec\.ts/);
  assert.match(output, /messages\.spec\.ts/);
  assert.match(output, /write_file tool details show\|diff cards show\|image diff cards show/);
});
