# Forge Feishu Upgrade Sync Hook Design

Date: 2026-06-30
Status: pending user review
Scope: Local Forge repository hook that writes valuable upgrade summaries to Feishu

## Goal

Create a local hook that automatically writes useful Forge upgrade notes to the remote Feishu knowledge base after meaningful commits.

The hook should reduce manual documentation drift without turning Feishu into a noisy commit log.

The desired behavior is:

```text
git commit
  -> post-commit hook
  -> classify whether the commit is worth syncing
  -> generate a concise Chinese upgrade summary
  -> append the summary to a local sync log
  -> append the same summary to Feishu
```

The hook must be non-blocking. A sync failure should not fail or undo a commit.

## Non-Goals

This design does not introduce a full documentation publishing platform.

It will not:

- sync every commit blindly
- upload diffs or patches to Feishu
- expose local absolute paths such as `/Users/...`
- rewrite the existing curated Forge Feishu knowledge base on every commit
- create a cloud cron job or server-side automation
- depend on an LLM API call to decide whether a commit matters
- block commits when Feishu auth, network, or CLI state is unavailable

## Trigger

Use a Git `post-commit` hook.

Rationale:

- The commit hash and final commit message exist.
- The hook can compare `HEAD^..HEAD`.
- It cannot block commit creation if the sync fails.
- It matches the user intent: write upgrade notes after real development steps.

The hook will be installed through the repository hook path, not by directly editing `.git/hooks` as the source of truth.

The implementation should create or repair a versioned `.githooks` directory and make `apps/desktop/scripts/install-git-hooks.mjs` install `post-commit` alongside any existing hooks.

## Hook Shape

Versioned hook:

```text
.githooks/post-commit
```

Expected body:

```sh
#!/bin/sh
node scripts/sync-forge-feishu.mjs --hook post-commit --commit HEAD || true
```

The hook may print a concise warning if sync fails, but it must exit `0`.

## Script

Add:

```text
scripts/sync-forge-feishu.mjs
scripts/sync-forge-feishu.test.mjs
```

The script should support:

```bash
node scripts/sync-forge-feishu.mjs --commit HEAD
node scripts/sync-forge-feishu.mjs --since HEAD~3 --dry-run
node scripts/sync-forge-feishu.mjs --hook post-commit --commit HEAD
```

`--dry-run` prints the classification and generated Markdown without writing locally or remotely.

`--hook post-commit` enables non-blocking behavior and shorter terminal output.

## Value Classification

The script should classify commits into `sync` or `skip`.

Sync by default when either the commit message or touched files indicate user-visible or architecture-visible value.

### Commit Message Signals

Sync:

- `feat:`
- `fix:`
- `test:`
- `docs:`
- `refactor:` only when touching core runtime paths

Usually skip:

- `chore(deps):`
- lockfile-only commits
- dependency-only commits
- pure formatting commits

### File Path Signals

High-value paths:

- `apps/desktop/src-tauri/src/agent/`
- `apps/desktop/src-tauri/src/harness/`
- `apps/desktop/src-tauri/src/executor/`
- `apps/desktop/src-tauri/src/protocol/`
- `apps/desktop/src-tauri/src/loop_runtime/`
- `apps/desktop/src-tauri/src/gateway/`
- `apps/desktop/src-tauri/src/diagnostics/`
- `apps/desktop/src/store/`
- `apps/desktop/src/components/`
- `apps/eval-runner/app/`
- `apps/eval-runner/eval_cases/`
- `scripts/acceptance.sh`
- `scripts/acceptance.test.mjs`
- `README.md`
- `apps/desktop/README.md`
- `CHANGELOG.md`
- `docs/superpowers/plans/`
- `docs/superpowers/specs/`

Skip-only paths:

- generated artifacts
- `.forge/`
- `dist/`
- `target/`
- `artifacts/`
- `playwright-report/`
- `test-results/`
- lockfile-only dependency bumps unless paired with security or acceptance notes

### Minimum Signal Rule

The first implementation should be deterministic and conservative:

- A commit syncs if it has a sync message prefix and at least one high-value path.
- A commit syncs if it touches `README.md`, `CHANGELOG.md`, `scripts/acceptance.sh`, `docs/superpowers/plans/`, or `docs/superpowers/specs/`.
- A commit skips if every changed file is skip-only.
- A `chore(deps)` commit skips unless it also touches docs/acceptance code outside lockfiles.

## Summary Content

Generate Chinese Markdown.

Template:

```markdown
## YYYY-MM-DD · <short sha> · <commit subject>

### 升级摘要
<one or two concise sentences>

### 影响范围
- <area>

### 关键改动
- <repo-relative path group and change summary>

### 验证证据
- 自动同步脚本未推断出验证命令；请按需要补充。

### 边界
- 本条同步由本地 hook 生成，只记录高层升级，不包含完整 diff。
```

The first version may derive summaries from commit subject and changed file groups. It should not invent verification commands.

## Path Hygiene

All paths in generated output must be repo-relative.

Forbidden output:

```text
/Users/
/private/
file://
```

Allowed output:

```text
apps/desktop/src-tauri/src/agent/session/loop.rs
scripts/acceptance.sh
docs/superpowers/plans/...
```

## Local Log

Append successful or pending entries to:

```text
docs/forge-sync/feishu-upgrade-log.md
```

The local log is the durable audit trail.

If Feishu upload fails, write the entry locally with a pending marker:

```markdown
<!-- feishu-sync: pending reason="auth_unavailable" -->
```

If Feishu upload succeeds, include:

```markdown
<!-- feishu-sync: uploaded url="..." -->
```

## Feishu Target

Use the existing Forge Feishu knowledge base.

Current root:

```text
https://www.feishu.cn/wiki/U7OrwxDUCiwTbak7xyhcD03FnPc
```

Create one child page:

```text
Forge 升级同步
```

Then append future entries to that page.

The target URL/token should not be hardcoded only inside the script. Store it in a small config file:

```text
docs/forge-sync/feishu-sync.config.json
```

Example:

```json
{
  "rootWikiUrl": "https://www.feishu.cn/wiki/U7OrwxDUCiwTbak7xyhcD03FnPc",
  "upgradeLogUrl": ""
}
```

If `upgradeLogUrl` is empty, the script should ask the user to run an explicit setup command rather than creating pages silently from a hook.

Recommended setup command:

```bash
node scripts/sync-forge-feishu.mjs --setup-feishu
```

`--setup-feishu` may create the `Forge 升级同步` page and write its URL to config.

## Feishu Failure Modes

The script must handle:

- `lark-cli` missing
- user not authenticated
- expired or insufficient Feishu scopes
- network failure
- Feishu update failure
- missing `upgradeLogUrl`

In hook mode:

- write a local pending entry when possible
- print a short warning
- exit `0`

In manual mode:

- exit non-zero for upload failures unless `--dry-run`
- print the remediation command

## Tests

Use Node's built-in test runner.

Tests should cover:

- classifies user-visible runtime commits as sync
- skips `chore(deps)` lockfile-only commits
- syncs docs/acceptance changes
- strips local absolute path prefixes from generated Markdown
- generates deterministic Chinese Markdown from commit metadata
- writes pending local log when Feishu is unavailable
- parses CLI flags without touching Feishu in `--dry-run`

The tests should not require network or Feishu auth.

## Integration Points

Update:

- `apps/desktop/scripts/install-git-hooks.mjs`
- `apps/desktop/scripts/pre-commit-check.test.mjs` only if hook install behavior needs shared tests
- `package.json` if a root script is useful
- `README.md` or `apps/desktop/README.md` only if the user-facing workflow should mention this automation

Do not update the acceptance matrix for the first version unless the hook becomes part of release gating. This hook is documentation automation, not product runtime behavior.

## Acceptance

Minimum acceptance:

```bash
node --test scripts/sync-forge-feishu.test.mjs
npm --prefix apps/desktop run hooks:install
node scripts/sync-forge-feishu.mjs --commit HEAD --dry-run
```

Optional manual acceptance after Feishu setup:

```bash
node scripts/sync-forge-feishu.mjs --setup-feishu
node scripts/sync-forge-feishu.mjs --commit HEAD
```

Success means:

- the hook can be installed from versioned `.githooks`
- meaningful commits produce concise Chinese summaries
- low-value commits are skipped
- generated output has no local absolute paths
- hook mode never blocks the commit
- Feishu failures degrade to local pending log entries

## Open Decisions

Resolved by user approval:

- Trigger: `post-commit`
- Behavior: non-blocking
- Sync style: valuable upgrades only, not every commit
- Remote target: Feishu

Implementation should still avoid creating the Feishu child page from an unattended hook. Setup must be explicit.

