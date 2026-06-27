# Phase 8 Disposable Edit/Build Loop Protocol

## Scope

This protocol covers stability regression batch rows #1, #2, and #3 against a disposable current project. It is designed to prove that Forge can complete a small edit, a small style polish, and a build/check loop without controller-side manual writes.

Controller-side manual writes invalidate the run. Read-only evidence commands, screenshots, and build/check commands are allowed.

## Target Project

Recommended disposable project:

```bash
/Users/cabbos/project/forge-test-app
```

## Readiness Preflight

Before starting a fresh live loop, run:

```bash
node scripts/disposable-loop-preflight.mjs --json --project /Users/cabbos/project/forge-test-app
```

This preflight verifies that the disposable project exists, is a git worktree rooted at the selected project path, has no existing git changes, includes the expected demo files, and exposes a package `build` script for row #3.

`readyForLoop: true` means the project is ready to collect fresh live Forge evidence. `readyForLoop: false` is not a product failure by itself, but the issue must be resolved or explicitly recorded before treating a live run as fresh evidence. Use `--require-ready` when a local script should fail fast on a non-ready project.

If the default disposable project has residual changes that should not be reset, prepare a clean non-destructive worktree from its current `HEAD`:

```bash
node scripts/prepare-disposable-loop-project.mjs --json --source /Users/cabbos/project/forge-test-app --target /Users/cabbos/project/forge-test-app-phase8-clean
node scripts/disposable-loop-preflight.mjs --json --project /Users/cabbos/project/forge-test-app-phase8-clean
npm --prefix /Users/cabbos/project/forge-test-app-phase8-clean run build
```

Open the prepared target project in Forge for rows #1-#3. The helper does not reset, stash, or edit the original source project; when possible it symlinks the source `node_modules` into the clean worktree so build/check evidence can run without reinstalling dependencies.

Before collecting screen- or Computer Use-based evidence, check whether this desktop session can observe app windows and capture nonblank screenshots:

```bash
node scripts/desktop-ui-evidence-preflight.mjs --json
```

If this reports `observer_limited`, `window_snapshot_failed`, `screen_capture_failed`, or `screen_capture_limited`, do not claim live UI evidence from screenshots, window counts, or Computer Use output. The JSON includes `recoveryCommands` pointing to the desktop UI evidence doctor plus `permissionScope` explaining that Forge Trust/Full Access does not grant macOS Screen Recording or Accessibility. Run the Forge row manually in a trusted desktop session, then paste the final answer, confirmation behavior, and screenshot/transcript reference into the manual JSON fields.

For a concrete local recovery checklist, run:

```bash
node scripts/desktop-ui-evidence-doctor.mjs --markdown
```

The doctor maps screenshot and window-observation failures to Screen Recording and Accessibility recovery commands, explicitly notes that Forge Trust/Full Access does not grant those macOS privacy permissions, then points back to the strict preflight and the `node scripts/phase8-disposable-loop-status.mjs --json --require-live-ready` hard gate. To open the relevant macOS settings panes directly, run `node scripts/desktop-ui-evidence-doctor.mjs --markdown --open-settings`; this is intentionally opt-in and is not used by the acceptance dry-run.

After each live row, collect a consistent evidence packet:

```bash
node scripts/phase8-disposable-loop-status.mjs --markdown
node scripts/phase8-disposable-loop-runbook.mjs --markdown --row 1
node scripts/create-disposable-loop-manual-json.mjs --row 1 --out /tmp/phase8-row-1-manual.json
node scripts/collect-disposable-loop-evidence.mjs --markdown --project /Users/cabbos/project/forge-test-app-phase8-clean --row 1 --run-build
node scripts/finalize-disposable-loop-row.mjs --json --project /Users/cabbos/project/forge-test-app-phase8-clean --row 1 --manual-json /tmp/phase8-row-1-manual.json --run-build --require-complete
```

Use `--row 2` or `--row 3` for the later prompts. The status command scans archived row validation files, checks local desktop UI evidence readiness, and prints the next incomplete row plus the matching runbook commands. A row is treated as `archived_complete` only when the validation JSON passes and the matching `.evidence.json` plus `.md` sidecars both exist; a lone validation file stays incomplete and is reported as missing sidecars. If the project is clean but local screenshots/window automation are not trustworthy, it reports `ui_evidence_not_ready` instead of `ready_for_live_row`, and its top-level JSON/Markdown includes `recoveryCommands` for the same doctor and opt-in settings opener exposed by the preflight. Status and runbook JSON both expose `liveReadyGate.pass`, `liveReadyGate.reason`, and the `--require-live-ready` command, while Markdown mirrors pass/reason as a `Live-ready gate` line so automation and humans can explain whether the hard gate passed, was skipped by unchecked UI evidence, or is blocked by local evidence readiness. Even when UI preflight is skipped and the nested status is `not_checked`, status/runbook JSON keeps `permissionScope` so the macOS privacy boundary is not lost. Add `--require-live-ready` when using the status command as an automation gate; it exits nonzero unless all rows are complete or the next row is ready after a checked UI preflight, so `--skip-ui-preflight --require-live-ready` intentionally fails. Full `scripts/acceptance.sh` now runs this hard gate after the report-only status summary. The runbook command prints the current project readiness, UI evidence preflight status, live-ready gate reason, exact prompt, manual JSON path, UI evidence doctor command, collect/finalize commands, top-level recovery commands, and target archive paths for the selected row. The manual JSON generator pre-fills the exact prompt and correct evidence field labels; after Forge finishes, fill in the final answer, confirmation behavior, screenshot/transcript reference, and row result. The collector captures git changed files, diff stat/name-status, optional build/check output, and markdown placeholders for the Forge final answer, confirmation behavior, and screenshot/transcript reference. The finalizer runs manual JSON review, strict row validation, and archive in order: row #1 needs an in-`src/` fix plus build/check evidence, row #2 must stay style-file-only, and row #3 must leave no file diff while reporting a successful command. Without `--require-complete`, helper commands can report pending evidence without failing; with `--require-complete`, missing final-answer/confirmation/build/diff evidence fails the check. The archive helper writes `.evidence.json`, `.md`, and `.validation.json` into `apps/desktop/docs/product/evidence/phase8-disposable-loop/` only after strict validation passes. These helpers do not replace the live Forge UI evidence; paste the missing manual fields into the generated packet or pass them with `--manual-json`.

Before starting, record:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
git -C /Users/cabbos/project/forge-test-app rev-parse --show-toplevel
```

## Required Setup

1. Open Forge desktop with the disposable project selected.
2. Start a new conversation in that workspace.
3. Enable `信任项目` or `完全访问`.
4. Do not edit project files outside Forge.
5. Keep screenshots or final-answer text for each prompt.

## Prompt Sequence

### Row #1: Small `/fix`

```text
/fix @src/App.tsx
这个 demo 页面里有一个按钮点击后没有明显反馈。请先定位原因，再做最小修复，并运行相关检查。只改当前 demo 项目。
```

Required evidence:

- final answer;
- changed file list;
- diff summary;
- build/check command and result;
- whether any confirmation card appeared after Trust or Full Access was enabled.

### Row #2: CSS Polish

```text
在当前 demo 项目里做一个很小的 CSS layout polish，只改样式文件。目标是让主要按钮点击反馈更明显，但不要重构组件，不要改业务逻辑。完成后说明改了哪些文件。
```

Required evidence:

- final answer;
- changed file list proving only style files changed;
- diff summary;
- no external write attempt.

### Row #3: Build/Check Command

```text
请在当前 demo 项目运行合适的 build/check 命令，并总结命令、结果和任何失败原因。不要修改文件。
```

Required evidence:

- exact command;
- pass/fail result;
- output summary;
- no file diff after the command-only prompt.

## Pass Criteria

- Forge completes each row without controller-side manual writes.
- Trust or Full Access avoids repeated routine current-project confirmations.
- Any external, secret-like, or unexpected write remains blocked or manual.
- Changed files stay under the disposable workspace.
- The final answer names the command result and changed files.

## Failure Recording

For each failed row, record:

```markdown
| Row | Prompt | Status | Failure | Evidence | Follow-up |
| --- | --- | --- | --- | --- | --- |
| 1 | Small `/fix` | Failed |  |  |  |
```

Severity guidance:

- P0: external write, secret leak, data loss, or false success after failed command.
- P1: repeated confirmation despite enabled permission mode, wrong workspace, missing build/check after claiming success.
- P2: weak final answer, unclear diff summary, unnecessary clarification, or minor workflow friction.

## Completion Evidence Template

```markdown
## Phase 8 Disposable Edit/Build Loop - 2026-06-27

Status: Not yet run.

Project:
Permission mode:
Conversation/session id:

Row #1 result:
- Final answer:
- Changed files:
- Diff summary:
- Build/check:
- Confirmation behavior:

Row #2 result:
- Final answer:
- Changed files:
- Diff summary:
- No external write attempt:
- Confirmation behavior:

Row #3 result:
- Command:
- Result:
- Output summary:
- Diff after command:

Overall result:
Follow-up:
```
