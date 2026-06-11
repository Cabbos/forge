import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

test("worktree smoke require-key accepts key from forge config", () => {
  const tmpHome = mkdtempSync(join(tmpdir(), "forge-wt-smoke-home-"));
  const forgeDir = join(tmpHome, ".forge");
  mkdirSync(forgeDir, { recursive: true });
  writeFileSync(
    join(forgeDir, "config.json"),
    JSON.stringify({ api_keys: { deepseek: "sk-test" } }),
  );

  try {
    const scriptPath = resolve("scripts/smoke-worktree-worker.mjs");
    const result = spawnSync(
      process.execPath,
      [scriptPath, "--require-key", "--dry-run"],
      {
        cwd: resolve("."),
        encoding: "utf8",
        env: {
          PATH: process.env.PATH,
          HOME: tmpHome,
        },
      },
    );

    assert.equal(
      result.status,
      0,
      `expected smoke dry-run to accept config key\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
    );
    assert.match(result.stdout, /Using API key from: config:deepseek/);
    assert.match(result.stdout, /DRY RUN/);
  } finally {
    rmSync(tmpHome, { recursive: true, force: true });
  }
});
