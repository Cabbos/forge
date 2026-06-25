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
