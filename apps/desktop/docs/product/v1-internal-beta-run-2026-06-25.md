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
| Preview ownership | Not run | - | - | - |
| Checkpoint and recovery | Not run | - | - | - |
| Honest recall | Not run | - | - | - |
| Developer review flow | Not run | - | - | - |

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

Result: Not run
Evidence seen:

- Not run.

Problems:

- None recorded.

Severity: -
Next action: -

## Scenario 4: Checkpoint And Recovery

Prompt A:

```text
请做一个小改动：在首页增加一个“今日完成”区域。完成后创建检查点并运行检查。
```

Prompt B, after failure or manual stop:

```text
继续刚才的任务。先根据上一轮的失败证据判断做到哪了，再从中断处继续。不要假装上一步已经成功。
```

Result: Not run
Evidence seen:

- Not run.

Problems:

- None recorded.

Severity: -
Next action: -

## Scenario 5: Honest Recall

Prompt:

```text
我们之前在这个项目里说了什么？如果你没有可靠记录，请明确说不知道，只基于当前可见对话和已保存背景回答。
```

Result: Not run
Evidence seen:

- Not run.

Problems:

- None recorded.

Severity: -
Next action: -

## Scenario 6: Developer Review Flow

Prompt:

```text
/code-review
请检查当前 demo 项目最值得担心的问题，优先找真实 bug、回归风险和缺失验证。不要做大而全重构建议。
```

Result: Not run
Evidence seen:

- Not run.

Problems:

- None recorded.

Severity: -
Next action: -

## Blocker Queue

No P0 or P1 blockers recorded yet.

## Final Decision

Not evaluated yet.
