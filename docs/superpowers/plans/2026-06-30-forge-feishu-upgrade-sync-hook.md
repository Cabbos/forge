# Forge Feishu Upgrade Sync Hook Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a non-blocking `post-commit` hook that writes valuable Forge upgrade summaries to a local log and Feishu.

**Architecture:** A root-level Node script owns classification, Markdown generation, local log writes, Feishu setup, and Feishu append. A versioned `.githooks/post-commit` invokes the script in non-blocking hook mode. The existing desktop hook installer is extended to install `post-commit` from `.githooks`.

**Tech Stack:** Node.js ESM, Node built-in test runner, Git CLI, `lark-cli`, Markdown files, Git hooks.

---

## Files

- Create: `scripts/sync-forge-feishu.mjs` — CLI and exported pure functions for classification, summary generation, local logging, setup, and Feishu upload.
- Create: `scripts/sync-forge-feishu.test.mjs` — Node tests for deterministic classification, sanitization, Markdown generation, dry-run parsing, and local pending behavior.
- Create: `.githooks/post-commit` — versioned non-blocking hook entry.
- Create: `.githooks/pre-commit` — versioned hook entry for the existing desktop pre-commit check.
- Create: `docs/forge-sync/feishu-sync.config.json` — Feishu sync target config with existing Forge root and empty upgrade log URL until setup.
- Create: `docs/forge-sync/feishu-upgrade-log.md` — local audit log.
- Modify: `apps/desktop/scripts/install-git-hooks.mjs` — install `pre-commit`, `pre-push`, and `post-commit` from `.githooks`, keeping missing hooks optional.
- Modify: `apps/desktop/scripts/pre-commit-check.test.mjs` or create focused installer tests only if needed after inspecting installer boundaries.
- Modify: `package.json` — add root scripts for dry-run/setup if useful.

## Task 1: Classification, Sanitization, and Markdown Tests

**Files:**
- Create: `scripts/sync-forge-feishu.test.mjs`
- Create after RED: `scripts/sync-forge-feishu.mjs`

- [ ] **Step 1: Write failing tests for pure behavior**

Add tests that import these named exports from `scripts/sync-forge-feishu.mjs`:

```js
import assert from "node:assert/strict";
import test from "node:test";

import {
  classifyCommit,
  generateUpgradeMarkdown,
  sanitizeRepoText,
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

  assert.match(markdown, /## 2026-06-30 · abcdef1 · feat\\(runtime\\): add durable loop evidence/);
  assert.match(markdown, /### 升级摘要/);
  assert.match(markdown, /Loop Runtime/);
  assert.match(markdown, /apps\\/desktop\\/src-tauri\\/src\\/loop_runtime\\/journal\\.rs/);
  assert.doesNotMatch(markdown, /\\/Users\\/cabbos/);
});
```

- [ ] **Step 2: Run RED**

Run:

```bash
node --test scripts/sync-forge-feishu.test.mjs
```

Expected: FAIL with module not found for `scripts/sync-forge-feishu.mjs`.

- [ ] **Step 3: Implement minimal pure functions**

Create `scripts/sync-forge-feishu.mjs` with:

- constants for high-value paths and area labels
- `sanitizeRepoText(text)`
- `classifyCommit({ subject, files })`
- `generateUpgradeMarkdown({ sha, date, subject, files, classification })`
- no Feishu calls yet

- [ ] **Step 4: Run GREEN**

Run:

```bash
node --test scripts/sync-forge-feishu.test.mjs
```

Expected: PASS.

## Task 2: CLI Parsing, Git Metadata, Dry Run, and Local Pending Log

**Files:**
- Modify: `scripts/sync-forge-feishu.test.mjs`
- Modify: `scripts/sync-forge-feishu.mjs`
- Create: `docs/forge-sync/feishu-upgrade-log.md`
- Create: `docs/forge-sync/feishu-sync.config.json`

- [ ] **Step 1: Add failing tests for CLI-safe helpers**

Add tests for:

- `parseArgs(["--commit", "HEAD", "--dry-run"])`
- `appendLocalLog({ logPath, markdown, status: "pending", reason: "missing_upgrade_log_url" })`
- `loadConfig(configPath)` returning empty `upgradeLogUrl` when config exists

- [ ] **Step 2: Run RED**

Run:

```bash
node --test scripts/sync-forge-feishu.test.mjs
```

Expected: FAIL because helpers do not exist.

- [ ] **Step 3: Implement helpers**

Add:

- `parseArgs(argv)`
- `loadConfig(configPath)`
- `appendLocalLog({ logPath, markdown, status, reason, url })`
- `readCommitMetadata({ commit, execFileSync })`
- CLI `runCli()` that supports `--dry-run`

The CLI should not write local log or call Feishu in `--dry-run`.

- [ ] **Step 4: Add config and initial local log**

Create `docs/forge-sync/feishu-sync.config.json`:

```json
{
  "rootWikiUrl": "https://www.feishu.cn/wiki/U7OrwxDUCiwTbak7xyhcD03FnPc",
  "upgradeLogUrl": ""
}
```

Create `docs/forge-sync/feishu-upgrade-log.md`:

```markdown
# Forge Feishu Upgrade Log

This file is the local audit trail for valuable Forge upgrade summaries synced to Feishu.

```

- [ ] **Step 5: Run GREEN**

Run:

```bash
node --test scripts/sync-forge-feishu.test.mjs
node scripts/sync-forge-feishu.mjs --commit HEAD --dry-run
```

Expected: tests pass; dry run prints classification/Markdown and does not write local log.

## Task 3: Feishu Setup and Upload Failure Handling

**Files:**
- Modify: `scripts/sync-forge-feishu.test.mjs`
- Modify: `scripts/sync-forge-feishu.mjs`
- Modify: `docs/forge-sync/feishu-sync.config.json` only during manual setup, not during unit tests.

- [ ] **Step 1: Add failing tests with fake command runner**

Add tests for:

- `syncToFeishu` returns `{ ok: false, reason: "missing_upgrade_log_url" }` when config lacks `upgradeLogUrl`
- hook mode writes a pending local entry and returns success status when upload cannot run
- `setupFeishu` parses a fake `lark-cli docs +create` JSON response and writes `upgradeLogUrl`

- [ ] **Step 2: Run RED**

Run:

```bash
node --test scripts/sync-forge-feishu.test.mjs
```

Expected: FAIL because Feishu helper functions do not exist.

- [ ] **Step 3: Implement Feishu helpers**

Add:

- `extractJsonObject(output)`
- `setupFeishu({ configPath, rootWikiUrl, execFileSync })`
- `syncToFeishu({ config, markdown, execFileSync })`
- hook-mode fallback that calls `appendLocalLog(... pending ...)`

Implementation must use `lark-cli docs +create` only for explicit `--setup-feishu`.

Implementation must use `lark-cli docs +update --mode append` for upload.

- [ ] **Step 4: Run GREEN**

Run:

```bash
node --test scripts/sync-forge-feishu.test.mjs
```

Expected: PASS without network or Feishu auth.

## Task 4: Versioned Git Hooks and Installer

**Files:**
- Create: `.githooks/post-commit`
- Create: `.githooks/pre-commit`
- Modify: `apps/desktop/scripts/install-git-hooks.mjs`
- Modify or create tests for installer behavior.

- [ ] **Step 1: Run GitNexus impact before editing installer**

Run:

```text
impact({ target: "run", repo: "forge", file_path: "apps/desktop/scripts/install-git-hooks.mjs", direction: "upstream" })
```

If HIGH or CRITICAL, stop and warn the user before editing.

- [ ] **Step 2: Add failing installer test**

If installer test coverage is practical, add a test that proves the installer includes `post-commit` in the hook names it chmods.

If existing installer structure makes this impractical without a large refactor, skip installer tests and verify through `npm --prefix apps/desktop run hooks:install` plus filesystem inspection.

- [ ] **Step 3: Run RED if a test was added**

Run:

```bash
node --test apps/desktop/scripts/pre-commit-check.test.mjs
```

Expected: FAIL only if new installer behavior is test-covered.

- [ ] **Step 4: Add versioned hooks**

Create `.githooks/pre-commit`:

```sh
#!/bin/sh
cd "$(git rev-parse --show-toplevel)/apps/desktop" || exit 1
npm run check:precommit
```

Create `.githooks/post-commit`:

```sh
#!/bin/sh
cd "$(git rev-parse --show-toplevel)" || exit 0
node scripts/sync-forge-feishu.mjs --hook post-commit --commit HEAD || true
```

- [ ] **Step 5: Update installer**

Change `apps/desktop/scripts/install-git-hooks.mjs` so the list includes `post-commit`.

- [ ] **Step 6: Run GREEN and install**

Run:

```bash
node --test scripts/sync-forge-feishu.test.mjs
npm --prefix apps/desktop run hooks:install
git config --get core.hooksPath
test -x .githooks/post-commit
```

Expected:

- tests pass
- hooks install reports `.githooks`
- `core.hooksPath` is `.githooks`
- post-commit is executable

## Task 5: Manual Feishu Setup and End-to-End Dry Run

**Files:**
- Modify: `docs/forge-sync/feishu-sync.config.json` after successful setup.
- Modify: `docs/forge-sync/feishu-upgrade-log.md` only if manual non-dry upload is run.

- [ ] **Step 1: Run dry run**

Run:

```bash
node scripts/sync-forge-feishu.mjs --commit HEAD --dry-run
```

Expected:

- prints skip/sync classification
- prints Markdown if syncable
- no local absolute path appears

- [ ] **Step 2: Run explicit setup**

Run:

```bash
node scripts/sync-forge-feishu.mjs --setup-feishu
```

Expected:

- creates or reports the `Forge 升级同步` Feishu page
- writes `upgradeLogUrl` into `docs/forge-sync/feishu-sync.config.json`

- [ ] **Step 3: Run manual sync**

Run:

```bash
node scripts/sync-forge-feishu.mjs --commit HEAD
```

Expected:

- if HEAD is syncable, appends to Feishu and local log
- if HEAD is low-value, prints skip and does not upload

## Task 6: Final Verification and Commit

**Files:**
- All implementation files above.

- [ ] **Step 1: Run full focused verification**

Run:

```bash
node --test scripts/sync-forge-feishu.test.mjs
npm --prefix apps/desktop run hooks:install
node scripts/sync-forge-feishu.mjs --commit HEAD --dry-run
```

Expected:

- tests pass
- install succeeds
- dry-run succeeds
- generated sync output and local log entries do not contain local absolute path prefixes.

- [ ] **Step 2: Run GitNexus detect changes**

Run:

```text
detect_changes({ scope: "all", repo: "forge" })
```

Expected: review affected scope; warn user if risk is HIGH or CRITICAL.

- [ ] **Step 3: Commit implementation**

Run:

```bash
git status --short
git add .githooks apps/desktop/scripts/install-git-hooks.mjs scripts/sync-forge-feishu.mjs scripts/sync-forge-feishu.test.mjs docs/forge-sync docs/superpowers/plans/2026-06-30-forge-feishu-upgrade-sync-hook.md
git commit -m "feat: sync forge upgrades to feishu"
```

Do not stage unrelated untracked files such as `docs/superpowers/plans/2026-06-30-backend-runtime-reliability-convergence.md`.
