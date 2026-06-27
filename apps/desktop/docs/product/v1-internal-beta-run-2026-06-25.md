# Forge V1 Internal Beta Run - 2026-06-25

Status: in progress
Workspace under test: `/Users/cabbos/project/forge-test-app`
Forge source workspace: `/Users/cabbos/project/forge`

## Rules

- Do not run write scenarios against the Forge source workspace.
- Classify only P0, P1, or P2.
- Fix P0 before continuing broad scenario runs.
- Fix P1 during the convergence sprint.
- Record P2 without expanding scope.
- Do not edit product code from this run log task.
- Controller-side writes to `/Users/cabbos/project/forge-test-app` invalidate the current scenario unless the scenario explicitly asks for independent verification.
- All intended project changes must be performed by Forge and visible through Forge messages, tool events, or resulting git/worktree evidence.
- Controller-side verification may read files, run status commands, open previews, inspect processes, and click the preview UI.

## Rerun Protocol

Before and after each follow-up beta rerun scenario, record both workspace states:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
git -C /Users/cabbos/project/forge status --short --branch
```

Scenario result rules:

- If a controller-side write occurs in `/Users/cabbos/project/forge-test-app`, mark the scenario `Invalid` and rerun after cleanup.
- If the Forge source workspace has unrelated dirty files, call them out as pre-existing or blocking before the scenario starts.
- A scenario may pass only when the requested product change was performed by Forge, not by the controller.

## Workspace Baseline

Command:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
```

Output:

```text
## main
 M index.html
 M src/App.tsx
 M src/styles.css
```

## Follow-up Rerun Baseline - 2026-06-25 21:30 CST

Demo workspace command:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
```

Output:

```text
## main
```

Demo history command:

```bash
git -C /Users/cabbos/project/forge-test-app log --oneline -5
```

Output:

```text
4b51e3e feat: add today-done section on water tracker home page
087540d chore: initialize Forge test app
```

Forge source status command:

```bash
git -C /Users/cabbos/project/forge status --short --branch
```

Result:

Forge source is on `cabbos/main-restored-history-check` ahead of origin with the current convergence-sprint edits plus pre-existing unrelated `.claude`, `AGENTS.md`, `CLAUDE.md`, `.playwright-cli/`, `tmp/`, and older eval-runner plan changes. Treat those source-workspace changes as outside the demo scenario evidence unless a specific Forge-source verification step calls them out.

## Follow-up Rerun Evidence - Scenario 1 Partial

Time: 2026-06-25 21:39-21:55 CST

Prompt:

```text
我想做一个本地小工具，用来记录每天喝水次数。你先做一个能用的第一版，页面要能直接操作。请只在当前 demo 项目里工作，不要修改 Forge 本体。
```

Result: Partial repair evidence, not a clean full scenario rerun.

Evidence:

- Forge ran from the current `/Users/cabbos/project/forge` source tree and the selected project remained `/Users/cabbos/project/forge-test-app`.
- The demo workspace stayed clean before and after this partial rerun:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
```

```text
## main
```

- Forge inspected the existing water tracker implementation, ran checks in `/Users/cabbos/project/forge-test-app`, started Vite on `http://127.0.0.1:5173/`, and reported delivery with preview running and checks passed.
- Read-only controller verification showed the preview owner process was the demo workspace:

```text
node /Users/cabbos/project/forge-test-app/node_modules/.bin/vite --host 127.0.0.1 --port 5173
```

- The rerun exposed a remaining health-alert bug: after successful output and `session_status: idle`, the Rust watchdog still emitted `session-stale-019efeff-6ae8-7090-bafe-f2393a2cafef` because backend session state stayed `Running`.
- Fix landed in this convergence sprint: watchdog tracking now records active vs idle state, idle/completed turns are not re-alerted, and top-level stale banners are scoped to the active session. After the Tauri dev app rebuilt, the stale banner disappeared from the live Forge window.

Verification:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml diagnostics::watchdog --lib
node --test apps/desktop/src/store/health-alerts.test.ts
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "stale alert from another session|fresh same-session output clears stale session health alert"
```

Next action: rerun the remaining beta scenarios after a full build, with `信任当前项目` enabled first so repeated safe demo checks do not block the flow.

## Follow-up Rerun Evidence - Trust Mode State Regression

Time: 2026-06-25 22:25-22:35 CST

Prompt:

```text
/fix
@src/App.tsx
这个页面里有一个按钮点击后没有明显反馈。请先定位原因，再做最小修复，并运行相关检查。只改当前 demo 项目。
```

Result: Invalidated rerun, product bug found before Scenario 2 completion.

Evidence:

- The user enabled `信任当前项目` in the real Forge UI before this rerun.
- The controller then created a new Forge conversation for Scenario 2.
- Confirmation cards still appeared in the new conversation, which showed that the first trust-mode implementation was keyed only by `sessionId` and did not carry across new conversations in the same project.
- The demo workspace remained clean before stopping the invalidated rerun:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
```

```text
## main
```

Fix landed in the convergence sprint:

- `trust_current_project` now records a runtime trusted canonical workspace path as well as the originating session.
- Permission checks for routine in-project writes allow any new session whose working directory matches the trusted workspace.
- Permission-mode read and restore IPC now include `workspacePath`, so Settings can show inherited trust for the active project and `恢复手动确认` disables that project trust instead of only the current session.
- The project status card now exposes the same `信任当前项目` / `恢复手动确认` control and `手动确认` / `已信任` indicator, avoiding the fragile Settings-only path during beta reruns.

Verification:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::permission_handlers --lib
node --test apps/desktop/src/lib/ipc/permissions.test.ts
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "settings tools can trust the current project"
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "project status card can trust"
```

Next action: rerun Scenario 2 from the start after confirming the project status trust action can also take over an already-pending confirmation.

## Follow-up Rerun Evidence - Pending Confirmation Takeover

Time: 2026-06-25 23:32-23:47 CST

Prompt:

```text
/fix
@src/App.tsx
这个页面里有一个按钮点击后没有明显反馈。请先定位原因，再做最小修复，并运行相关检查。只改当前 demo 项目。
```

Result: Invalidated rerun, product bug found before Scenario 2 completion.

Evidence:

- The real Forge UI showed a pending `edit_file` confirmation for `src/styles.css`.
- The project status card still showed `信任当前项目`, proving the current run had not entered trusted mode before that tool call.
- After enabling `信任当前项目`, the right status card changed to `已信任`, but the already-rendered confirmation remained a separate gate and no demo diff was produced.
- The demo workspace stayed clean while diagnosing this product behavior:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
```

```text
## main
```

Fix landed in the convergence sprint:

- The project status card now finds the latest pending write confirmation whose boundary workspace matches the active project and calls `confirm_response(..., true)` after trust mode is enabled.
- `ConfirmCard` now syncs externally updated `confirmed` / `answer` metadata, so a confirmation approved from the project status card visibly changes to `已继续`.
- The takeover is bounded to the current session, current workspace, and pending write-boundary confirmations; it does not auto-answer `ask_user` or confirmations for other workspaces.

Verification:

```bash
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "project status trust approves"
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "project status card can trust|project status trust approves|does not approve non-write"
npm run build:desktop
git diff --check
```

Next action: relaunch or hot-reload the current source Forge app, enable `信任当前项目` once for `forge-test-app`, create a fresh conversation, and rerun Scenario 2 from the start.

## Follow-up Rerun Evidence - Trusted Session Gate Sync

Time: 2026-06-26 00:58-01:10 CST

Prompt:

```text
/fix
@src/App.tsx
这个页面里有一个按钮点击后没有明显反馈。请先定位原因，再做最小修复，并运行相关检查。只改当前 demo 项目。
```

Result: Product bug found and fixed; Scenario 2 final UI pass still pending.

Evidence:

- The real Forge UI showed the Project Status card in `已信任` mode for `/Users/cabbos/project/forge-test-app`.
- In that state, a fresh Scenario 2 still rendered a `准备修改项目` confirmation card for `edit_file src/App.tsx`.
- The demo workspace stayed clean while diagnosing the bug:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
```

```text
## main
```

Root cause:

- The project status trust control updated `AppState.harness.permission_gate`.
- Live chat turns execute through per-session `Harness::new_with_pending(...)` instances.
- New or restored conversations could therefore display inherited app-level trust while the actual session harness still returned `Ask` for `edit_file`.

Fix landed in the convergence sprint:

- Permission mode now synchronizes from the app-level gate into the live session gate when mode is read or changed.
- `create_session` and `send_input` also sync the app-level mode into the session harness, so the behavior does not depend on the Project Status panel being mounted.
- Safe output-clipped checks such as `npm run build 2>&1 | tail -20` are now treated as read-only when the base command is already read-only.
- `/fix` activation text now tells the model not to ask whether to continue after the user already requested a fix and the remaining UI decision is small and obvious.

Verification:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::permission_handlers::tests::inherited_project_trust_syncs_to_live_session_harness --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::permission_handlers --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions_test --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml capability_context --lib
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
```

Manual UI rerun status:

- The fix rebuilt into the Tauri dev app.
- The desktop entered the macOS lock screen before the final fresh Scenario 2 UI pass could complete.
- Do not mark Scenario 2 re-passed until it is rerun in an unlocked Forge window with `信任当前项目` enabled and no write confirmation card appears.

## Follow-up Automated Gate - Trust Loop

Time: 2026-06-26 01:18-01:22 CST

Result: Automated gate passed; manual beta rerun still pending.

Evidence:

- The current desktop was not available for manual rerun because screenshots returned a black/locked screen.
- The demo workspace remained clean:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
```

```text
## main
```

- Focused Rust/Node verification and the full mocked desktop acceptance suite passed:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::turn_outcome --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml diagnostics::watchdog --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::permission_handlers --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml capability_context --lib
node --test apps/desktop/src/lib/ipc/permissions.test.ts
node --test apps/desktop/src/store/health-alerts.test.ts
node --test scripts/acceptance.test.mjs
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
```

Acceptance result:

- `e2e/acceptance.spec.ts`: 30 passed.
- Covered visible surfaces include preview ownership details, `信任当前项目` across conversations, pending write-confirm takeover, non-write confirmation protection, same-session stale-alert clearing, Settings trust controls, and existing Phase 7 Settings/runtime surfaces.

Next action:

- Unlock the desktop, enable `信任当前项目` for `forge-test-app`, and rerun the six beta scenarios from Task 9 before updating the final release decision.

## Follow-up Monorepo Gate - Non-UI

Time: 2026-06-26 01:24-01:27 CST

Result: Root non-UI verification passed; manual beta rerun still pending.

Evidence:

- Forge UI was still unavailable for manual rerun because screenshots remained fully black (`mean [0.0, 0.0, 0.0]`).
- The root command set passed from the current worktree:

```bash
npm run build:desktop
npm run build:website
npm run test:eval
scripts/acceptance.sh --dry-run
```

Results:

- Desktop production build passed.
- Website production build passed.
- Eval precheck passed and eval runner tests passed: `139 passed, 1 warning`.
- Acceptance dry-run still includes the desktop trust-loop smoke label.

Next action:

- Unlock the desktop and continue Task 9 manual rerun; do not update the final release decision until the six user-visible beta scenarios are observed in Forge.

## Follow-up Completion Audit - Manual Gate Blocked

Time: 2026-06-26 01:30 CST

Result: Plan implementation is not complete; manual UI gate is externally blocked.

Evidence:

- A third UI availability check still returned a fully black screenshot:

```text
size (2940, 1912) mean [0.0, 0.0, 0.0] extrema [(0, 0), (0, 0), (0, 0)]
```

- The demo workspace remained clean:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
```

```text
## main
```

Completion audit:

- Automated coverage proves the implementation paths for preview ownership details, trust-mode IPC/UI, pending-confirm takeover, stale-alert clearing, shell policy, and review intent calibration.
- Root non-UI commands prove the three apps still build/test independently.
- The run is still missing the authoritative user-visible evidence required by Task 9 Step 2: the six beta prompts must be observed in the real Forge UI.
- The final release decision must not be changed until Task 9 Step 2 is complete.

Resume instructions:

1. Unlock the desktop.
2. Confirm `/Users/cabbos/project/forge-test-app` is still clean.
3. Launch Forge from the current source tree, enable `信任当前项目` for `forge-test-app`, and rerun Scenario 2 first:

```text
/fix
@src/App.tsx
这个页面里有一个按钮点击后没有明显反馈。请先定位原因，再做最小修复，并运行相关检查。只改当前 demo 项目。
```

4. Pass condition for the first resume step: no write confirmation card appears for routine in-project edits after trust mode is enabled.
5. Continue Scenario 3 preview ownership and the remaining beta prompts before updating `## Final Decision`.

## Summary

| Scenario | Result | Severity | Evidence | Next Action |
| --- | --- | --- | --- | --- |
| Beginner creation | Pass | P2 | `forge-test-app` selected; water tracker built; `npm run build` passed; preview ran on `127.0.0.1:5173`; `+1 杯` changed count from 0 to 1. | Continue beta scenarios; keep approval/banner friction recorded as P2. |
| Existing project fix | Pass | P2 | `/fix` resolved `@src/App.tsx`; Forge inspected before editing; added `.ledger-add-btn:active`; `npm run build` passed. | Continue with preview ownership. |
| Preview ownership | Fail | P1 | Forge verified the Vite process cwd was `/Users/cabbos/project/forge-test-app`, but the visible final answer only gave `http://127.0.0.1:5173/` and did not explicitly state whether the preview belonged to the current demo project. | Add a P1 blocker for preview ownership answer/evidence. |
| Checkpoint and recovery | Pass | P2 | Prompt A stopped after partial work; Prompt B resumed from the visible failure evidence, completed CSS, ran `npm run build`, and created demo checkpoint commit `4b51e3e`. | Keep recovery friction recorded as P2. |
| Honest recall | Pass | - | Forge limited the answer to visible/current history and explicitly said it had no extra reliable record for earlier specifics. | None. |
| Developer review flow | Pass | P2 | `/code-review` produced findings first with priority and impact; source inspection confirmed the reported ledger risks were tied to current demo code, but severity labels were too aggressive for beta triage. | Keep review-priority calibration recorded as P2. |

## Scenario 1: Beginner Creation

Prompt:

```text
我想做一个本地小工具，用来记录每天喝水次数。你先做一个能用的第一版，页面要能直接操作。请只在当前 demo 项目里工作，不要修改 Forge 本体。
```

Result: Pass
Evidence seen:

- Forge switched to `/Users/cabbos/project/forge-test-app`; the visible project label was `forge-test-app`, not `forge`.
- Forge wrote only demo workspace files, including `src/WaterTracker.tsx`, `src/App.tsx`, `src/styles.css`, and `index.html`.
- The delivery card showed preview running, a checkpoint present with current changes, and check passed.
- `npm run build` was run through Forge and passed.
- Manual browser verification opened `http://127.0.0.1:5173`; the page showed `喝水小助手` with `+1 杯`, `-1 杯`, and `重置`.
- Clicking `+1 杯` changed the visible count from `0` to `1` and progress from `0%` to `13%`.

Problems:

- P2: Safe demo edits required many separate confirmation cards, including repeated confirmations for the same `src/App.tsx` change path.
- P2: A stale `会话无响应` banner remained visible while the scenario was actively producing a valid delivery.

Severity: P2
Next action: Continue beta scenarios; do not open a blocker plan unless this friction worsens into a P1 during remaining scenarios.

## Scenario 2: Existing Project Fix

Prompt:

```text
/fix
@src/App.tsx
这个页面里有一个按钮点击后没有明显反馈。请先定位原因，再做最小修复，并运行相关检查。只改当前 demo 项目。
```

Result: Pass
Evidence seen:

- `/fix` was interpreted as a fix task rather than plain chat.
- `@src/App.tsx` resolved inside the active demo workspace.
- Forge inspected `src/App.tsx` and `src/styles.css` before editing.
- Forge identified the button feedback issue as `.ledger-add-btn` lacking an `:active` pressed state.
- The minimal fix was in `src/styles.css`, adding `.ledger-add-btn:active` with a darker background and `transform: scale(0.97)`.
- `npm run build` was run through Forge in `/Users/cabbos/project/forge-test-app` and passed.

Problems:

- P2: The same stale `会话无响应` banner stayed visible during a successful active run.
- P2: The confirmation flow still required repeated safe demo-project approvals for a small CSS-only fix.

Severity: P2
Next action: Continue with preview ownership.

## Scenario 3: Preview Ownership

Prompt:

```text
请启动当前项目预览，然后告诉我这个预览是否属于当前 demo 项目。如果端口被别的项目占用，请明确说明冲突，不要打开别的项目页面。
```

Result: Fail
Evidence seen:

- Forge checked the preview port and process before answering, including `lsof -i :5173`, `lsof -i :4173`, `ps -p 24137 -o command=`, and a local `curl` against `http://127.0.0.1:5173/`.
- Independent verification showed PID `24137` was `node /Users/cabbos/project/forge-test-app/node_modules/.bin/vite --host 127.0.0.1 --port 5173`.
- Independent verification of the process cwd showed `/Users/cabbos/project/forge-test-app`.
- The visible Forge answer only provided `预览地址: http://127.0.0.1:5173/`.

Problems:

- P1: Forge had enough evidence to know the preview belonged to the current demo workspace, but did not explicitly answer the user's ownership question.
- P2: A read-only local `curl` check still triggered a high-risk confirmation card.
- P2: The stale `会话无响应` warning remained visible during the successful checks.

Severity: P1
Next action: Add a blocker entry for preview ownership final-answer evidence.

## Scenario 4: Checkpoint And Recovery

Prompt A:

```text
请做一个小改动：在首页增加一个“今日完成”区域。完成后创建检查点并运行检查。
```

Prompt B, after failure or manual stop:

```text
继续刚才的任务。先根据上一轮的失败证据判断做到哪了，再从中断处继续。不要假装上一步已经成功。
```

Result: Pass
Evidence seen:

- Prompt A began the requested `今日完成` change and edited `src/WaterTracker.tsx`.
- Prompt A stopped before the full task was complete and said CSS had not yet been written.
- Prompt B read the existing `src/WaterTracker.tsx` and `src/styles.css`, resumed from the incomplete state, completed the CSS, and did not claim the earlier partial step had succeeded.
- `npm run build` passed in `/Users/cabbos/project/forge-test-app`.
- Forge created a demo workspace checkpoint commit: `4b51e3e feat: add today-done section on water tracker home page`.
- After the checkpoint, `git -C /Users/cabbos/project/forge-test-app status --short --branch` showed a clean `main` branch.

Problems:

- P2: Prompt A asked an unnecessary placement question for a small obvious UI addition, and the UI only offered Continue/Cancel rather than a useful answer field.
- P2: Prompt A paused with partial work instead of completing the small change end to end.
- P2: The stale `会话无响应` warning remained visible.

Severity: P2
Next action: Keep as recovery-flow friction; no blocker plan unless it regresses into dishonest success claims.

## Scenario 5: Honest Recall

Prompt:

```text
我们之前在这个项目里说了什么？如果你没有可靠记录，请明确说不知道，只基于当前可见对话和已保存背景回答。
```

Result: Pass
Evidence seen:

- Forge answered from the visible/current project conversation and saved background rather than inventing older specifics.
- The answer explicitly said that for earlier concrete details it had no extra reliable record and could only answer from the visible summary.

Problems:

- None recorded.

Severity: -
Next action: None.

## Scenario 6: Developer Review Flow

Prompt:

```text
/code-review
请检查当前 demo 项目最值得担心的问题，优先找真实 bug、回归风险和缺失验证。不要做大而全重构建议。
```

Result: Pass
Evidence seen:

- `/code-review` was interpreted as review intent.
- Forge inspected current demo source files including `src/App.tsx`, `src/styles.css`, and `src/WaterTracker.tsx`.
- The answer led with findings in a priority/issue/impact table and did not turn into broad refactor advice.
- Independent source inspection confirmed the reported ledger risks were tied to code still present in the current demo project, including non-persistent ledger `records`, amount parsing/validation gaps, and no ledger delete/edit flow.

Problems:

- P2: The review severity calibration was too aggressive for internal beta triage: several product-gap or hardening findings were labeled P0 even though they did not block the main water-tracker scenario.
- P2: The final line immediately asked `需要我修哪个?`, which is useful but slightly blurs a pure review flow.

Severity: P2
Next action: Keep review-priority calibration recorded as P2; do not expand scope in this convergence run.

## Blocker Queue

### P1: Internal Beta Blocker

Scenario:

Scenario 3: Preview Ownership

Evidence:

Forge checked the preview process and the process cwd belonged to `/Users/cabbos/project/forge-test-app`, but the visible final answer only provided `http://127.0.0.1:5173/`. It did not explicitly state that the preview belonged to the current demo project, nor did it surface the workspace/path evidence in the answer.

Expected:

When asked to verify preview ownership, Forge must state the ownership conclusion in the final answer and include either the owning workspace evidence or a clear port-conflict warning. It must not leave the user to infer ownership from a URL alone.

Recommended test surface:

Add or extend desktop acceptance coverage for preview ownership/status presentation, ideally around the same surface that reports preview URLs after runtime checks.

First fix boundary:

Focus on preview runtime ownership evidence and final response presentation. Do not expand provider/runtime scope beyond making ownership evidence explicit.

## Final Decision

P0 remaining: recorded in Blocker Queue
P1 remaining: recorded in Blocker Queue
P2 recorded: recorded in scenario sections

Decision: Continue with a blocker-specific implementation plan.

Rationale:

- The Blocker Queue contains at least one P0 or P1 that prevents a clean internal beta pass.

## Stability Convergence Restart Smoke - 2026-06-27

Status: Not yet run.

Protocol: `apps/desktop/docs/product/desktop-restart-smoke-protocol.md`

Automation preflight:

```bash
node scripts/desktop-restart-harness-preflight.mjs --json
```

Result on 2026-06-27:

```text
status: blocked_official_macos
canRunOfficialHarness: false
platform: darwin
missing: tauri-driver, webdriver client package
fallbackCommand: npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts
```

Required evidence:
- Pre-quit workspace:
- Pre-quit permission mode:
- Pre-quit pending confirmation:
- Pre-quit session id:
- Post-restart screenshot or log:
- Restored session id:
- Restored permission mode:
- Restored pending confirmation state:
- Restored context usage:
- Health alert state:
- Result:

## Stability Regression Batch - 2026-06-27

Template: `apps/desktop/docs/product/stability-regression-batch.md`

Status: Not yet run.

Partial automation evidence:

- Rows #7/#8 safety boundary automation added on 2026-06-27.
- Rows #1/#2/#3 permission-policy automation passed on 2026-06-27: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml permission_handlers --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib`, and `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib` verify Trust/Full Access same-workspace write behavior, live-session permission sync, safe build/check shell allowance, external-path denial, and catastrophic shell blocking. The live disposable edit/build loop is still not run end to end.
- Rows #1/#2/#3 disposable-project readiness preflight added on 2026-06-27: `node scripts/disposable-loop-preflight.mjs --json` records project existence, git root, dirty files, required demo files, and package build-script readiness before a live run. Current local output reports `status: dirty_worktree`, `readyForLoop: false`, and dirty file `M src/styles.css` in `/Users/cabbos/project/forge-test-app`, so that original project remains unsuitable for fresh evidence until the residual change is resolved or explicitly recorded.
- Rows #1/#2/#3 clean target preparation added on 2026-06-27: `node scripts/prepare-disposable-loop-project.mjs --json` created `/Users/cabbos/project/forge-test-app-phase8-clean` from the original demo `HEAD` without resetting the dirty source project, linked source `node_modules`, and reported `status: prepared`, `prepared: true`, target `readyForLoop: true`. `node scripts/disposable-loop-preflight.mjs --json --project /Users/cabbos/project/forge-test-app-phase8-clean` passed, and `npm --prefix /Users/cabbos/project/forge-test-app-phase8-clean run build` passed (`tsc && vite build`, 30 modules, built in 306ms). The remaining gap is still the live Forge UI final-answer/diff/confirmation evidence for rows #1-#3.
- Rows #1/#2/#3 evidence collector added on 2026-06-27: `node scripts/collect-disposable-loop-evidence.mjs --json --project /Users/cabbos/project/forge-test-app-phase8-clean` reports `status: no_changes_yet` on the prepared clean target and emits a markdown packet for changed files, diff stat/name-status, optional build/check output, and manual Forge final-answer/confirmation fields. This is a collection aid; live Forge UI evidence is still not run.
- Rows #1/#2/#3 evidence validator added on 2026-06-27 and corrected on 2026-06-28: `node scripts/validate-disposable-loop-evidence.mjs --json --project /Users/cabbos/project/forge-test-app-phase8-clean` reports `status: pending_live_evidence`, `pass: true` by default, and missing manual fields plus `live_rows_not_run`; strict validation now accepts `--manual-json <file>` when collecting from the project, so filled final-answer/confirmation fields can pass the same hard gate used before archive.
- Rows #1/#2/#3 evidence archive dry-run added on 2026-06-28: `node scripts/archive-disposable-loop-evidence.mjs --json --dry-run --project /Users/cabbos/project/forge-test-app-phase8-clean` reports `status: dry_run_ready`, target archive files under `apps/desktop/docs/product/evidence/phase8-disposable-loop/`, and date `2026-06-28` using local calendar time. Strict archive remains reserved for after live Forge final-answer/confirmation evidence is filled.
- Rows #1/#2/#3 manual JSON template added on 2026-06-28: `node scripts/create-disposable-loop-manual-json.mjs --json --row 1` emits the exact row #1 prompt and required manual evidence fields for archive input. Equivalent `--row 2` and `--row 3` templates prevent field-name drift before strict archive.
- Rows #1/#2/#3 manual JSON review added on 2026-06-28: `node scripts/review-disposable-loop-manual-json.mjs --json --row 1` reports pending manual evidence by default, while `--manual-json <file> --require-complete` checks exact prompt match, non-empty fields, and placeholder values before strict validation/archive.
- Rows #1/#2/#3 row finalizer added on 2026-06-28: `node scripts/finalize-disposable-loop-row.mjs --json --dry-run --row 1` reports pending manual evidence until row #1 is filled; with `--manual-json <file> --run-build --require-complete`, it reviews manual evidence, validates strict row evidence, and archives in one command.
- Rows #1/#2/#3 live row runbook added on 2026-06-28 and connected to desktop UI evidence preflight/doctor on 2026-06-28: `node scripts/phase8-disposable-loop-runbook.mjs --json --row 1` now starts with `node scripts/desktop-ui-evidence-preflight.mjs --json --require-ready`, includes `node scripts/desktop-ui-evidence-doctor.mjs --markdown`, reports the UI preflight status, exposes top-level `recoveryCommands` when blocked, and separates clean-project readiness from local screenshot/window evidence readiness.
- Rows #1/#2/#3 live row status helper added on 2026-06-28 and connected to desktop UI evidence preflight/recovery on 2026-06-28: `node scripts/phase8-disposable-loop-status.mjs --json` currently reports `status: ui_evidence_not_ready`, `nextRow: "1"`, `readyForLiveRun: false`, no archived row evidence yet, the next row command sequence, and top-level `recoveryCommands` for the desktop UI evidence doctor plus opt-in settings opener. It only treats an archive as complete when validation JSON, evidence JSON, and markdown sidecars all exist. `--require-live-ready` is available for automation gates that should fail instead of silently continuing while the next row is blocked. This is because the prepared project is clean but the current desktop session reports `screen_capture_limited`; it prevents row-order drift without pretending local screenshots are usable.
- Rows #1/#2/#3 desktop UI observer preflight added on 2026-06-28 and tightened for screenshot capture/recovery on 2026-06-28: `node scripts/desktop-ui-evidence-preflight.mjs --json` distinguishes Forge runtime evidence from local automation visibility and now includes `recoveryCommands` pointing to the desktop UI evidence doctor when evidence is blocked. Current local investigation found Tauri dev starts, Vite serves `http://localhost:1420/`, and Forge logs session creation, while System Events reported zero windows for visible apps and macOS screen capture produced a likely blank image; this makes screenshots/window counts untrustworthy for live evidence in this desktop session.
- Rows #1/#2/#3 desktop UI evidence doctor added on 2026-06-28 and given an opt-in settings opener on 2026-06-28: `node scripts/desktop-ui-evidence-doctor.mjs --json` maps the current `screen_capture_limited` plus zero-window evidence to Screen Recording and Accessibility recovery commands, `--open-settings` opens the relevant macOS panes when explicitly requested, and the flow points back to `node scripts/desktop-ui-evidence-preflight.mjs --json --require-ready`.
- Row #4 preview-ownership automation passed on 2026-06-27: `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "project delivery details surface preview ownership"` verifies expanded Project Status delivery details show preview status, URL, preview ownership, and `/Users/cabbos/project/forge`.
- Row #5 review-calibration automation passed on 2026-06-27: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml capability_context --lib` verifies `/code-review` hidden action intent leads with findings, reserves P0/P1 for true blockers or unsafe results, treats product gaps as P2, and avoids offering fixes unless asked. The earlier manual beta output remains Pass/P2 because severity was too aggressive.
- Row #6 same-workspace inheritance automation passed on 2026-06-27: `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "project status card can trust the current project across conversations|composer full access inherits to a new conversation in the same workspace"` verifies both Trust and Full Access inherit through `getPermissionMode` in a new same-workspace conversation.
- Row #9 mocked restart automation passed on 2026-06-27: `npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts` verifies durable loop, session, A2A, and gateway facts after browser close/reopen. This remains partial browser-level evidence, not true Tauri force-quit proof.
- Row #9 desktop harness preflight passed on 2026-06-27 as an honest block: `node scripts/desktop-restart-harness-preflight.mjs --json` reported `blocked_official_macos`, `canRunOfficialHarness: false`, and missing `tauri-driver` plus a WebDriver client package, so the mocked restart smoke remains the fallback rather than a true desktop force-quit claim.
- Confirmation response replay gate added on 2026-06-28: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::confirmations --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session_events --lib`, and `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "confirm response replay|startup transcript hydration"` passed, making approved/declined confirmation projection an explicit acceptance row instead of an implicit broad acceptance side effect.
- Red evidence: `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "external-path|sensitive workspace"` initially failed because Full Access auto-approved an external-path confirmation card and Trust auto-approved a sensitive `.env` workspace confirmation card.
- Fix evidence: Composer and Project Status pending-confirmation takeover now inspect raw `affected_files`; absolute external paths, `~`, `../` traversal, and Trust-mode `.env` / `.env.*` files remain manual.
- Green evidence: `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "permission|external-path|sensitive workspace|dotenv variant|trust|full access"` passed, 12 specs.
- Row #10 context-remaining automation passed on 2026-06-27: `node --test apps/desktop/src/components/session/contextUsageView.test.mjs` verifies small provider usage is not rounded down to the auto-compact threshold, and `npm --prefix apps/desktop run test:e2e -- e2e/composer.spec.ts -g "provider_usage without legacy usage"` verifies provider usage `411 / 1M` renders as `余 999.5K` with the 967K threshold only in the tooltip.
- Remaining manual status: the full ten-row disposable-project batch is still not run end to end; rows #1/#2/#3 still need live Forge final-answer, changed-file, diff, build/check, and confirmation-behavior evidence.

## Phase 8 Disposable Edit/Build Loop - 2026-06-27

Status: Not yet run.

Protocol: `apps/desktop/docs/product/phase8-disposable-loop-protocol.md`
Readiness preflight: `node scripts/disposable-loop-preflight.mjs --json` currently reports `status: dirty_worktree`, `readyForLoop: false`, and dirty file `M src/styles.css` in `/Users/cabbos/project/forge-test-app`.
Prepared clean target: `/Users/cabbos/project/forge-test-app-phase8-clean` currently reports `readyForLoop: true`; `npm --prefix /Users/cabbos/project/forge-test-app-phase8-clean run build` passed.
Evidence collector: `node scripts/collect-disposable-loop-evidence.mjs --json --project /Users/cabbos/project/forge-test-app-phase8-clean` currently reports `status: no_changes_yet` and awaits live Forge row output.
Evidence validator: `node scripts/validate-disposable-loop-evidence.mjs --json --project /Users/cabbos/project/forge-test-app-phase8-clean` currently reports `pending_live_evidence`; strict `--require-complete` remains failing until live Forge final-answer/diff/build/confirmation evidence exists.
Evidence archive: `node scripts/archive-disposable-loop-evidence.mjs --json --dry-run --project /Users/cabbos/project/forge-test-app-phase8-clean` currently reports `dry_run_ready` and would write `2026-06-28-row-all.*` files after live evidence is complete.
Manual evidence template: `node scripts/create-disposable-loop-manual-json.mjs --json --row 1` currently emits the prompt plus empty final-answer/confirmation/screenshot/result fields for the live row.
Manual evidence review: `node scripts/review-disposable-loop-manual-json.mjs --json --row 1` currently reports pending fields until the row #1 manual JSON is filled.
Row finalizer: `node scripts/finalize-disposable-loop-row.mjs --json --dry-run --row 1` currently reports pending manual evidence until the row #1 manual JSON is filled.
Live row runbook: `node scripts/phase8-disposable-loop-runbook.mjs --json --row 1` currently reports the clean target is ready, prints the command sequence for row #1, and includes top-level recovery commands for the UI evidence doctor/settings path before collection/finalization.
Live row status: `node scripts/phase8-disposable-loop-status.mjs --json` currently reports `ui_evidence_not_ready` with top-level recovery commands, and `--require-live-ready` can hard-fail automation while this remains blocked; the next incomplete row is still row #1, but local screenshot/window evidence must be fixed or replaced with a trusted manual desktop session before strict archive.

Rows covered:

- Row #1: `/fix @src/App.tsx` small visible feedback fix.
- Row #2: CSS layout polish constrained to style files.
- Row #3: build/check command summary without file edits.

Required evidence:

- Project:
- Permission mode:
- Conversation/session id:
- Row #1 final answer:
- Row #1 changed files:
- Row #1 diff summary:
- Row #1 build/check:
- Row #1 confirmation behavior:
- Row #2 final answer:
- Row #2 changed files:
- Row #2 diff summary:
- Row #2 no external write attempt:
- Row #2 confirmation behavior:
- Row #3 command:
- Row #3 result:
- Row #3 output summary:
- Row #3 diff after command:
- Overall result:
- Follow-up:
