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
  assert.match(output, /loop event journal contract tests/);
  assert.match(output, /projection rebuild\/replay tests/);
  assert.match(output, /policy preflight tests/);
  assert.match(output, /budget preflight tests/);
  assert.match(output, /durable human gate tests/);
  assert.match(output, /typed completion evidence tests/);
  assert.match(output, /gateway loop runner status smoke/);
  assert.match(output, /subagent runtime event projection smoke/);
  assert.match(output, /completion contract mocked desktop smoke/);
  assert.match(output, /resume\.spec\.ts/);
  assert.match(output, /workbench\.spec\.ts/);
  assert.match(output, /a2a-confirm-runtime\.spec\.ts/);
  assert.match(output, /acceptance\.spec\.ts/);
  assert.match(output, /messages\.spec\.ts/);
  assert.match(output, /write_file tool details show\|diff cards show\|image diff cards show/);
});
