# Forge V1 Internal Beta Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run Forge's V1 internal beta playbook in a non-Forge workspace, capture evidence, and produce a ranked blocker queue for focused follow-up fixes.

**Architecture:** The beta run is the product evidence source for this sprint. This plan creates a durable run log, executes the six approved beta scenarios, classifies failures as P0/P1/P2, and stops before code changes when a blocker needs a separate focused implementation plan.

**Tech Stack:** Forge desktop Tauri app, the demo workspace `/Users/cabbos/project/forge-test-app`, existing repo docs, `scripts/acceptance.sh --dry-run`, Git, and GitNexus for any later blocker-specific code work.

---

## Scope Check

This plan covers one convergence loop: run the V1 internal beta scenarios and produce an evidence-backed blocker queue.

It does not implement product code changes. If a P0/P1 blocker is found, create a separate blocker-specific plan after this run. That later plan must include GitNexus impact analysis before symbol edits, focused failing tests, implementation steps, verification, and `detect_changes()` before commit.

## Source Documents

- Design spec: `docs/superpowers/specs/2026-06-25-v1-internal-beta-convergence-design.md`
- Existing playbook: `apps/desktop/docs/product/forge-v1-internal-beta-playbook.md`
- Acceptance map: `scripts/acceptance.sh`

## File Structure

- Create: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`
  - Responsibility: durable run log for the six beta scenarios, severity decisions, evidence, and next actions.
- Create later only if needed: a blocker-specific plan under `docs/superpowers/plans/`
  - Responsibility: focused fix plan for one P0 or P1.

Do not modify product code, runtime files, provider files, acceptance scripts, README files, or CHANGELOG in this plan.

## Task 1: Create The Beta Run Log

**Files:**
- Create: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`

- [ ] **Step 1: Create the run log from the approved template**

Create `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md` with this exact content:

````markdown
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

Not recorded yet.

## Summary

| Scenario | Result | Severity | Evidence | Next Action |
| --- | --- | --- | --- | --- |
| Beginner creation | Not run | - | - | - |
| Existing project fix | Not run | - | - | - |
| Preview ownership | Not run | - | - | - |
| Checkpoint and recovery | Not run | - | - | - |
| Honest recall | Not run | - | - | - |
| Developer review flow | Not run | - | - | - |

## Scenario 1: Beginner Creation

Prompt:

```text
我想做一个本地小工具，用来记录每天喝水次数。你先做一个能用的第一版，页面要能直接操作。请只在当前 demo 项目里工作，不要修改 Forge 本体。
```

Result: Not run
Evidence seen:

- Not run.

Problems:

- None recorded.

Severity: -
Next action: -

## Scenario 2: Existing Project Fix

Prompt:

```text
/fix
@src/App.tsx
这个页面里有一个按钮点击后没有明显反馈。请先定位原因，再做最小修复，并运行相关检查。只改当前 demo 项目。
```

Result: Not run
Evidence seen:

- Not run.

Problems:

- None recorded.

Severity: -
Next action: -

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
````

- [ ] **Step 2: Verify the run log exists**

Run:

```bash
test -f apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md
```

Expected: command exits 0.

- [ ] **Step 3: Commit the run log**

Run:

```bash
git add apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md
git commit -m "docs(product): start v1 internal beta run log"
```

Expected: commit succeeds with only the run log file staged.

## Task 2: Record Workspace Baseline

**Files:**
- Modify: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`

- [ ] **Step 1: Confirm the demo workspace exists**

Run:

```bash
test -d /Users/cabbos/project/forge-test-app
```

Expected: command exits 0.

- [ ] **Step 2: Capture demo workspace Git status**

Run:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
```

Expected: command prints the branch and any local changes for `/Users/cabbos/project/forge-test-app`.

- [ ] **Step 3: Update the run log baseline**

Replace the `## Workspace Baseline` section in `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md` with:

````markdown
## Workspace Baseline

Command:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
```

Output:

```text
Paste the exact output from the command here.
```
````

Then replace the sentence `Paste the exact output from the command here.` with the actual output from Step 2.

- [ ] **Step 4: Commit the workspace baseline**

Run:

```bash
git add apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md
git commit -m "docs(product): record beta workspace baseline"
```

Expected: commit succeeds with only the run log file staged.

## Task 3: Run Beginner Creation And Existing Project Fix

**Files:**
- Modify: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`

- [ ] **Step 1: Start Forge for manual beta testing**

Run:

```bash
npm run dev:desktop
```

Expected: Forge desktop starts. If it does not start, record Scenario 1 as `Fail`, severity `P1`, with the terminal error as evidence.

- [ ] **Step 2: Switch Forge to the demo workspace**

In the Forge UI, set the active workspace to:

```text
/Users/cabbos/project/forge-test-app
```

Expected: the visible project label refers to `forge-test-app`, not `forge`.

- [ ] **Step 3: Run Scenario 1 manually**

Paste this prompt into Forge:

```text
我想做一个本地小工具，用来记录每天喝水次数。你先做一个能用的第一版，页面要能直接操作。请只在当前 demo 项目里工作，不要修改 Forge 本体。
```

Expected pass signals:

- Forge asks at most one necessary question or proceeds.
- A visible local interface is produced.
- One core interaction works.
- Preview, checkpoint, or verification evidence is visible when available.

- [ ] **Step 4: Record Scenario 1 result**

Update `Scenario 1: Beginner Creation` and the Summary table in the run log.

Use `Pass` when all expected pass signals are present. Use `Fail` when any expected pass signal is missing. Use the highest severity observed.

- [ ] **Step 5: Run Scenario 2 manually**

Paste this prompt into Forge:

```text
/fix
@src/App.tsx
这个页面里有一个按钮点击后没有明显反馈。请先定位原因，再做最小修复，并运行相关检查。只改当前 demo 项目。
```

Expected pass signals:

- `/fix` is interpreted as action intent.
- `@src/App.tsx` resolves inside the demo workspace.
- Forge inspects before editing.
- The fix is minimal.
- Verification is run or clearly marked unavailable.

- [ ] **Step 6: Record Scenario 2 result**

Update `Scenario 2: Existing Project Fix` and the Summary table in the run log.

- [ ] **Step 7: Stop broad scenario execution if a P0 was found**

If Scenario 1 or Scenario 2 produced a P0, add a blocker entry under `## Blocker Queue` with these exact field names:

```markdown
### P0: Unsafe Or Dishonest Beta Failure

Scenario:

Evidence:

Expected:

Recommended next plan:

- Write a blocker-specific implementation plan before running more broad scenarios.
```

Fill in the blank field bodies with the observed scenario name, evidence, and expected behavior.

- [ ] **Step 8: Commit the first two scenario results**

Run:

```bash
git add apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md
git commit -m "docs(product): record first beta scenario results"
```

Expected: commit succeeds with only the run log file staged.

## Task 4: Run Preview, Recovery, Recall, And Review Scenarios

**Files:**
- Modify: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`

- [ ] **Step 1: Run Scenario 3 manually**

Paste this prompt into Forge:

```text
请启动当前项目预览，然后告诉我这个预览是否属于当前 demo 项目。如果端口被别的项目占用，请明确说明冲突，不要打开别的项目页面。
```

Expected pass signals:

- Runtime status is tied to `/Users/cabbos/project/forge-test-app`.
- Any port conflict is reported as a conflict.
- Forge does not open or validate a page from another workspace.

- [ ] **Step 2: Record Scenario 3 result**

Update `Scenario 3: Preview Ownership` and the Summary table in the run log.

- [ ] **Step 3: Run Scenario 4 manually**

Paste Prompt A:

```text
请做一个小改动：在首页增加一个“今日完成”区域。完成后创建检查点并运行检查。
```

If the turn fails or is manually stopped, paste Prompt B:

```text
继续刚才的任务。先根据上一轮的失败证据判断做到哪了，再从中断处继续。不要假装上一步已经成功。
```

Expected pass signals:

- Forge uses visible or persisted failure evidence.
- It does not claim failed work succeeded.
- It can continue or clearly says what blocks continuation.

- [ ] **Step 4: Record Scenario 4 result**

Update `Scenario 4: Checkpoint And Recovery` and the Summary table in the run log.

- [ ] **Step 5: Run Scenario 5 manually**

Paste this prompt into Forge:

```text
我们之前在这个项目里说了什么？如果你没有可靠记录，请明确说不知道，只基于当前可见对话和已保存背景回答。
```

Expected pass signals:

- Forge distinguishes visible history from saved background.
- It says when reliable history is unavailable.
- It does not fabricate old decisions.

- [ ] **Step 6: Record Scenario 5 result**

Update `Scenario 5: Honest Recall` and the Summary table in the run log.

- [ ] **Step 7: Run Scenario 6 manually**

Paste this prompt into Forge:

```text
/code-review
请检查当前 demo 项目最值得担心的问题，优先找真实 bug、回归风险和缺失验证。不要做大而全重构建议。
```

Expected pass signals:

- Forge uses code-review stance.
- Findings lead the answer.
- It avoids broad refactor advice unless tied to a real risk.

- [ ] **Step 8: Record Scenario 6 result**

Update `Scenario 6: Developer Review Flow` and the Summary table in the run log.

- [ ] **Step 9: Commit the remaining scenario results**

Run:

```bash
git add apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md
git commit -m "docs(product): record full beta scenario run"
```

Expected: commit succeeds with only the run log file staged.

## Task 5: Triage The Blocker Queue

**Files:**
- Modify: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`

- [ ] **Step 1: Verify every scenario is classified**

Read the Summary table.

Expected: every scenario row has `Pass` or `Fail`, and every failed row has `P0`, `P1`, or `P2`.

- [ ] **Step 2: Add P0/P1 blockers**

For every P0 or P1, add one entry under `## Blocker Queue` with these exact field names:

```markdown
### P1: Internal Beta Blocker

Scenario:

Evidence:

Expected:

Recommended test surface:

First fix boundary:
```

Use `### P0: Internal Beta Blocker` for P0 issues. Fill in each blank field body from the observed run evidence.

- [ ] **Step 3: Leave P2 out of the blocker queue**

Record P2 issues in the relevant scenario section only.

Expected: `## Blocker Queue` contains only P0 and P1.

- [ ] **Step 4: Commit the blocker queue**

Run:

```bash
git add apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md
git commit -m "docs(product): triage internal beta blockers"
```

Expected: commit succeeds with only the run log file staged.

## Task 6: Decide Whether A Focused Fix Plan Is Needed

**Files:**
- Modify: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`

- [ ] **Step 1: Evaluate the stop condition**

The run is ready for another internal beta pass when:

- All six scenarios have been run.
- No P0 remains.
- At most two P1 issues remain.
- Every remaining P1 has evidence and a recommended next plan.
- P2 issues are recorded without expanding scope.

Expected: the run log makes this decision possible from recorded evidence.

- [ ] **Step 2: Update the final decision section**

Replace `## Final Decision` with one of these two exact sections.

Use this section if no focused fix plan is needed before another beta pass:

```markdown
## Final Decision

P0 remaining: 0
P1 remaining: 0
P2 recorded: recorded in scenario sections

Decision: Ready for next internal beta pass.

Rationale:

- All six scenarios completed without P0 or P1 blockers.
```

Use this section if a focused fix plan is needed:

```markdown
## Final Decision

P0 remaining: recorded in Blocker Queue
P1 remaining: recorded in Blocker Queue
P2 recorded: recorded in scenario sections

Decision: Continue with a blocker-specific implementation plan.

Rationale:

- The Blocker Queue contains at least one P0 or P1 that prevents a clean internal beta pass.
```

- [ ] **Step 3: Commit the final decision**

Run:

```bash
git add apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md
git commit -m "docs(product): record beta convergence decision"
```

Expected: commit succeeds with only the run log file staged.

## Task 7: Create The First Blocker Plan When Needed

**Files:**
- Create when Final Decision says to continue: `docs/superpowers/plans/2026-06-25-v1-internal-beta-first-blocker.md`

- [ ] **Step 1: Stop before editing product code**

If the Final Decision says `Continue with a blocker-specific implementation plan`, do not edit product code in this plan.

Expected: product code remains untouched by the convergence-run plan.

- [ ] **Step 2: Create a new blocker-specific plan**

Create `docs/superpowers/plans/2026-06-25-v1-internal-beta-first-blocker.md` using the writing-plans skill again.

Expected: the new plan is based on exactly one P0 or P1 from the run log and includes:

- the selected blocker evidence,
- GitNexus query or impact steps before edits,
- the failing test to write first,
- the minimal implementation boundary,
- focused verification commands,
- documentation updates if the visible runtime surface changes,
- `detect_changes({ repo: "forge", scope: "staged" })` before commit.

- [ ] **Step 3: Commit the blocker-specific plan**

Run:

```bash
git add docs/superpowers/plans/2026-06-25-v1-internal-beta-first-blocker.md
git commit -m "docs(product): plan first beta blocker fix"
```

Expected: commit succeeds with only the blocker-specific plan file staged.
