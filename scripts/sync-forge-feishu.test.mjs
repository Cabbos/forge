import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  appendLocalLog,
  classifyCommit,
  extractJsonObject,
  generateUpgradeMarkdown,
  loadConfig,
  parseArgs,
  runCli,
  sanitizeRepoText,
  setupFeishu,
  syncToFeishu,
} from "./sync-forge-feishu.mjs";

test("classifies user-visible runtime commits as sync", () => {
  const result = classifyCommit({
    subject: "feat(runtime): add durable loop evidence",
    files: ["apps/desktop/src-tauri/src/loop_runtime/journal.rs"],
  });

  assert.equal(result.action, "sync");
  assert.match(result.reason, /high-value/);
  assert.deepEqual(result.areas, ["Loop Runtime"]);
});

test("skips dependency-only chore commits", () => {
  const result = classifyCommit({
    subject: "chore(deps): bump esbuild",
    files: ["package-lock.json"],
  });

  assert.equal(result.action, "skip");
  assert.match(result.reason, /dependency-only/);
});

test("syncs docs and acceptance changes", () => {
  const result = classifyCommit({
    subject: "docs: update runtime proof",
    files: ["CHANGELOG.md", "scripts/acceptance.sh"],
  });

  assert.equal(result.action, "sync");
  assert.deepEqual(result.areas, ["Acceptance", "Docs"]);
});

test("strips local absolute path prefixes from generated text", () => {
  const text = sanitizeRepoText(
    "/Users/example/project/forge/apps/desktop/src-tauri/src/agent/session/loop.rs",
  );

  assert.equal(text, "apps/desktop/src-tauri/src/agent/session/loop.rs");
});

test("generates deterministic Chinese markdown without local paths", () => {
  const markdown = generateUpgradeMarkdown({
    sha: "abcdef123456",
    date: "2026-06-30",
    subject: "feat(runtime): add durable loop evidence",
    files: ["/Users/example/project/forge/apps/desktop/src-tauri/src/loop_runtime/journal.rs"],
    classification: {
      action: "sync",
      reason: "sync prefix with high-value paths",
      areas: ["Loop Runtime"],
    },
  });

  assert.match(
    markdown,
    /## 2026-06-30 · abcdef1 · feat\(runtime\): add durable loop evidence/,
  );
  assert.match(markdown, /### 升级摘要/);
  assert.match(markdown, /Loop Runtime/);
  assert.match(markdown, /apps\/desktop\/src-tauri\/src\/loop_runtime\/journal\.rs/);
  assert.doesNotMatch(markdown, /\/Users\/cabbos/);
});

test("does not call skipped changes high-value in generated markdown", () => {
  const markdown = generateUpgradeMarkdown({
    sha: "abcdef123456",
    date: "2026-06-30",
    subject: "chore: format files",
    files: ["package-lock.json"],
    classification: {
      action: "skip",
      reason: "skip-only generated or lockfile paths",
      areas: [],
    },
  });

  assert.match(markdown, /自动识别为暂不需要远程同步的变更/);
  assert.doesNotMatch(markdown, /高价值变更/);
});

test("parses commit dry-run arguments", () => {
  const args = parseArgs(["--commit", "HEAD", "--dry-run"]);

  assert.deepEqual(args, {
    commit: "HEAD",
    dryRun: true,
    hook: "",
    setupFeishu: false,
    since: "",
  });
});

test("loads Feishu sync config with empty upgrade log URL", () => {
  const dir = mkdtempSync(join(tmpdir(), "forge-feishu-config-"));
  const configPath = join(dir, "feishu-sync.config.json");
  writeFileSync(
    configPath,
    JSON.stringify({
      rootWikiUrl: "https://www.feishu.cn/wiki/U7OrwxDUCiwTbak7xyhcD03FnPc",
      upgradeLogUrl: "",
    }),
  );

  assert.deepEqual(loadConfig(configPath), {
    rootWikiUrl: "https://www.feishu.cn/wiki/U7OrwxDUCiwTbak7xyhcD03FnPc",
    upgradeLogUrl: "",
  });
});

test("appends sanitized pending local log entries", () => {
  const dir = mkdtempSync(join(tmpdir(), "forge-feishu-log-"));
  const logPath = join(dir, "feishu-upgrade-log.md");

  appendLocalLog({
    logPath,
    markdown:
      "## 2026-06-30 · abcdef1 · feat\n\n- /Users/example/project/forge/apps/desktop/src/store/session.ts\n",
    status: "pending",
    reason: "missing_upgrade_log_url",
  });

  const output = readFileSync(logPath, "utf8");
  assert.match(output, /<!-- feishu-sync: pending reason="missing_upgrade_log_url" -->/);
  assert.match(output, /apps\/desktop\/src\/store\/session\.ts/);
  assert.doesNotMatch(output, /\/Users\/cabbos/);
});

test("Feishu sync reports missing upgrade log URL without shelling out", () => {
  let called = false;
  const result = syncToFeishu({
    config: {
      rootWikiUrl: "https://www.feishu.cn/wiki/U7OrwxDUCiwTbak7xyhcD03FnPc",
      upgradeLogUrl: "",
    },
    markdown: "## Upgrade",
    execFileSync: () => {
      called = true;
    },
  });

  assert.equal(called, false);
  assert.deepEqual(result, {
    ok: false,
    reason: "missing_upgrade_log_url",
  });
});

test("hook mode writes pending local log and returns success when upload cannot run", async () => {
  const dir = mkdtempSync(join(tmpdir(), "forge-feishu-hook-"));
  const configPath = join(dir, "feishu-sync.config.json");
  const logPath = join(dir, "feishu-upgrade-log.md");
  writeFileSync(
    configPath,
    JSON.stringify({
      rootWikiUrl: "https://www.feishu.cn/wiki/U7OrwxDUCiwTbak7xyhcD03FnPc",
      upgradeLogUrl: "",
    }),
  );
  const fakeExecFileSync = (command, args) => {
    assert.equal(command, "git");
    if (args[0] === "show") {
      return "abcdef123456\nfeat(runtime): add durable loop evidence\n2026-06-30\n";
    }
    if (args[0] === "diff-tree") {
      return "apps/desktop/src-tauri/src/loop_runtime/journal.rs\n";
    }
    throw new Error(`unexpected command: ${command} ${args.join(" ")}`);
  };

  const result = await runCli(["--hook", "post-commit", "--commit", "HEAD"], {
    configPath,
    execFileSync: fakeExecFileSync,
    logPath,
    repoRoot: dir,
    stderr: { write() {} },
    stdout: { write() {} },
  });

  assert.equal(result.ok, true);
  assert.equal(result.pending, true);
  const output = readFileSync(logPath, "utf8");
  assert.match(output, /<!-- feishu-sync: pending reason="missing_upgrade_log_url" -->/);
  assert.match(output, /feat\(runtime\): add durable loop evidence/);
});

test("extracts JSON object from lark-cli output", () => {
  assert.deepEqual(
    extractJsonObject('created\n{"url":"https://www.feishu.cn/wiki/child","token":"child"}\n'),
    {
      token: "child",
      url: "https://www.feishu.cn/wiki/child",
    },
  );
});

test("setupFeishu creates a child doc and writes upgrade log URL", () => {
  const dir = mkdtempSync(join(tmpdir(), "forge-feishu-setup-"));
  const configPath = join(dir, "feishu-sync.config.json");
  writeFileSync(
    configPath,
    JSON.stringify({
      rootWikiUrl: "https://www.feishu.cn/wiki/U7OrwxDUCiwTbak7xyhcD03FnPc",
      upgradeLogUrl: "",
    }),
  );
  const calls = [];
  const fakeExecFileSync = (command, args) => {
    calls.push([command, args]);
    assert.equal(command, "lark-cli");
    assert.deepEqual(args.slice(0, 3), ["docs", "+create", "--wiki-node"]);
    return 'ok\n{"url":"https://www.feishu.cn/wiki/ForgeSyncChild","token":"ForgeSyncChild"}\n';
  };

  const result = setupFeishu({
    configPath,
    execFileSync: fakeExecFileSync,
    rootWikiUrl: "https://www.feishu.cn/wiki/U7OrwxDUCiwTbak7xyhcD03FnPc",
  });

  assert.equal(calls.length, 1);
  assert.deepEqual(result, {
    ok: true,
    upgradeLogUrl: "https://www.feishu.cn/wiki/ForgeSyncChild",
  });
  assert.deepEqual(JSON.parse(readFileSync(configPath, "utf8")), {
    rootWikiUrl: "https://www.feishu.cn/wiki/U7OrwxDUCiwTbak7xyhcD03FnPc",
    upgradeLogUrl: "https://www.feishu.cn/wiki/ForgeSyncChild",
  });
});
