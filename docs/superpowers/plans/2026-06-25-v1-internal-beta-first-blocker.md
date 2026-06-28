# Preview Ownership Answer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Forge explicitly state preview ownership when it reports a preview URL, so the user can tell whether the preview belongs to the current project.

**Architecture:** Keep the fix narrow. The backend already records preview evidence with `project_path` and `url`; the agent final-answer instruction should surface that evidence when asking the model for the final text-only answer. The desktop delivery/status details should also expose the owning workspace path so the same fact is visible in the UI.

**Tech Stack:** Rust agent loop and turn-state evidence, React/TypeScript desktop status views, Playwright acceptance tests, GitNexus impact checks.

---

## Source Evidence

- Run log: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`
- Blocker: Scenario 3, Preview Ownership
- Observed failure: Forge checked the preview process and the process cwd belonged to `/Users/cabbos/project/forge-test-app`, but the visible final answer only provided `http://127.0.0.1:5173/`. It did not explicitly say the preview belonged to the current demo project.

## Scope Check

This plan fixes one P1 only: preview ownership answer/evidence.

Do not change provider routing, runtime process management, permission policy, checkpoint creation, or broad model behavior. The desired product behavior is: when Forge has preview ownership evidence and mentions the preview URL, it must say whether that URL belongs to the current project path.

## Current Investigation Notes

- `apps/desktop/src-tauri/src/agent/turn_state.rs` already records preview evidence through `record_preview_status(...)`; `preview_evidence_summary(...)` includes `project_path=...` and `url=...`.
- `apps/desktop/src-tauri/src/agent/turn_outcome.rs` currently builds the final text-only instruction with `final_answer_instruction(verification)`. It only adds extra guidance for failed verification.
- `apps/desktop/src-tauri/src/agent/session/loop.rs` calls `final_answer_instruction(...)` during `finalize_turn(...)` after tools and verification.
- `apps/desktop/src/components/layout/ProjectStatusDetails.tsx` shows preview status, preview URL, and command, but not an explicit ownership/workspace line in the expanded details.
- GitNexus did not resolve the relevant function symbols in the current index during planning; file nodes were present, but function-level `impact(...)` returned `Target not found`. Treat this as a preflight item for the implementation task.

## File Structure

- Modify: `apps/desktop/src-tauri/src/agent/turn_outcome.rs`
  - Responsibility: build the final text-only answer instruction and include preview ownership evidence when present.
- Modify: `apps/desktop/src-tauri/src/agent/session/loop.rs`
  - Responsibility: pass the latest turn state into `final_answer_instruction(...)` during finalization.
- Modify: `apps/desktop/src/components/layout/ProjectStatusDetails.tsx`
  - Responsibility: display explicit preview ownership workspace in the expanded project status card.
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
  - Responsibility: product-level smoke coverage for visible preview ownership evidence.
- Modify if the visible wording changes: `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`
  - Responsibility: keep user-visible runtime surface documentation in sync.
- Modify only if the acceptance command list changes: `scripts/acceptance.sh`
  - Responsibility: keep the dry-run acceptance plan aligned with advertised specs.

## Task 1: Preflight And Impact Analysis

**Files:**
- Read: `AGENTS.md`
- Read: `apps/desktop/AGENTS.md`
- Read: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`

- [x] **Step 1: Confirm the blocker evidence**

Read:

```bash
sed -n '/## Scenario 3: Preview Ownership/,/## Scenario 4:/p' apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md
sed -n '/## Blocker Queue/,/## Final Decision/p' apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md
```

Expected: the output identifies Scenario 3 as `Fail`, `P1`, and names the missing ownership conclusion.

- [x] **Step 2: Refresh GitNexus if symbol impact cannot resolve**

Run these GitNexus MCP calls first:

```text
impact({ repo: "forge", target: "final_answer_instruction", file_path: "apps/desktop/src-tauri/src/agent/turn_outcome.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "finalize_turn", file_path: "apps/desktop/src-tauri/src/agent/session/loop.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "ProjectStatusDetails", file_path: "apps/desktop/src/components/layout/ProjectStatusDetails.tsx", direction: "upstream", maxDepth: 2, summaryOnly: true })
```

If any returns `Target not found`, refresh the index:

```bash
node .gitnexus/run.cjs analyze
```

Then rerun the same GitNexus `impact(...)` calls.

Expected: risk is LOW or MEDIUM. If any risk is HIGH or CRITICAL, stop and report the blast radius before editing.

- [x] **Step 3: Record the implementation boundary**

Confirm the planned file list stays within:

```text
apps/desktop/src-tauri/src/agent/turn_outcome.rs
apps/desktop/src-tauri/src/agent/session/loop.rs
apps/desktop/src/components/layout/ProjectStatusDetails.tsx
apps/desktop/e2e/acceptance.spec.ts
README.md
apps/desktop/README.md
CHANGELOG.md
```

Expected: no provider adapter, permission policy, or runtime process manager file is needed.

## Task 2: Add Failing Backend Tests For Final-Answer Ownership Guidance

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/turn_outcome.rs`

- [x] **Step 1: Add the failing tests**

In the existing `#[cfg(test)] mod tests` in `apps/desktop/src-tauri/src/agent/turn_outcome.rs`, extend the imports:

```rust
use super::{
    final_answer_instruction, final_turn_status_for_current_turn, final_turn_status_for_run,
    final_turn_transition_reason_for_current_turn, final_turn_transition_reason_for_run,
    verification_has_failed,
};
use crate::agent::turn_state::{
    AgentTurnState, AgentTurnStatus, AgentVerificationStatus, AgentVerificationTrace,
};
```

Then add these tests and helper inside the same test module:

```rust
fn turn_with_running_preview() -> AgentTurnState {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/workspace/demo".to_string(),
        "deepseek".to_string(),
        "deepseek-v4-flash".to_string(),
        "agent-core".to_string(),
        "delivery".to_string(),
        "检查预览归属".to_string(),
    );

    turn.record_preview_status(
        Some("/workspace/demo"),
        true,
        false,
        true,
        "预览运行中",
        Some("http://localhost:5173"),
    );

    turn
}

#[test]
fn final_answer_instruction_includes_preview_ownership_when_preview_evidence_exists() {
    let turn = turn_with_running_preview();

    let instruction = final_answer_instruction(None, Some(&turn));

    assert!(instruction.contains("Preview ownership evidence"));
    assert!(instruction.contains("project_path=/workspace/demo"));
    assert!(instruction.contains("url=http://localhost:5173"));
    assert!(instruction.contains("explicitly say whether it belongs to the current project"));
}

#[test]
fn final_answer_instruction_keeps_failed_verification_and_preview_ownership_guidance() {
    let turn = turn_with_running_preview();
    let trace = verification(AgentVerificationStatus::Failed);

    let instruction = final_answer_instruction(Some(&trace), Some(&turn));

    assert!(instruction.contains("Verification did not pass"));
    assert!(instruction.contains("Verification command: npm run build"));
    assert!(instruction.contains("Preview ownership evidence"));
    assert!(instruction.contains("project_path=/workspace/demo"));
}

#[test]
fn final_answer_instruction_omits_preview_ownership_when_no_preview_evidence_exists() {
    let instruction = final_answer_instruction(None, None);

    assert!(!instruction.contains("Preview ownership evidence"));
    assert!(instruction.contains("provide your final answer as plain text"));
}
```

- [x] **Step 2: Run the focused Rust test and verify it fails**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::turn_outcome --lib
```

Expected: compile failure or test failure because `final_answer_instruction` does not yet accept the latest turn state and does not emit preview ownership guidance.

## Task 3: Implement Backend Preview Ownership Guidance

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/turn_outcome.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/loop.rs`

- [x] **Step 1: Change the final-answer instruction signature**

In `apps/desktop/src-tauri/src/agent/turn_outcome.rs`, change the imports and signature:

```rust
use crate::agent::turn_state::{
    AgentEvidenceKind, AgentToolStatus, AgentTurnState, AgentTurnStatus,
    AgentVerificationStatus, AgentVerificationTrace,
};

pub(crate) fn final_answer_instruction(
    verification: Option<&AgentVerificationTrace>,
    latest_turn: Option<&AgentTurnState>,
) -> String {
    let mut detail = match verification.filter(|trace| verification_has_failed(trace)) {
        Some(trace) => failed_verification_final_answer_instruction(trace),
        None => {
            "Based on the above, provide your final answer as plain text. Do not use tools."
                .to_string()
        }
    };

    if let Some(preview_instruction) = preview_ownership_final_answer_instruction(latest_turn) {
        detail.push('\n');
        detail.push_str(&preview_instruction);
    }

    detail
}
```

Move the existing failed-verification string assembly into this helper:

```rust
fn failed_verification_final_answer_instruction(trace: &AgentVerificationTrace) -> String {
    let mut detail = String::from(
        "Based on the above, provide your final answer as plain text. Do not use tools. Verification did not pass, so clearly tell the user what failed and avoid claiming the task is fully complete.",
    );
    if let Some(command) = trace.command.as_deref() {
        detail.push_str(&format!("\nVerification command: {command}"));
    }
    if let Some(exit_code) = trace.exit_code {
        detail.push_str(&format!("\nExit code: {exit_code}"));
    }
    if let Some(stderr) = trace.stderr_preview.as_deref() {
        detail.push_str(&format!("\nError output: {stderr}"));
    }
    if let Some(stdout) = trace.stdout_preview.as_deref() {
        detail.push_str(&format!("\nOutput: {stdout}"));
    }
    detail
}
```

- [x] **Step 2: Add the preview ownership helper**

Add this helper in `apps/desktop/src-tauri/src/agent/turn_outcome.rs`:

```rust
fn preview_ownership_final_answer_instruction(latest_turn: Option<&AgentTurnState>) -> Option<String> {
    let evidence = latest_turn?
        .evidence
        .iter()
        .rev()
        .find(|evidence| {
            evidence.kind == AgentEvidenceKind::Preview
                && evidence.status == AgentToolStatus::Completed
                && evidence.summary.as_deref().is_some_and(|summary| {
                    summary.contains("project_path=") || summary.contains("url=")
                })
        })?;
    let summary = evidence.summary.as_deref()?.trim();
    if summary.is_empty() {
        return None;
    }

    Some(format!(
        "Preview ownership evidence: {summary}. If you mention a preview URL, explicitly say whether it belongs to the current project path shown in this evidence. If ownership is unclear or the evidence shows a port conflict, say that clearly instead of only returning the URL."
    ))
}
```

- [x] **Step 3: Update existing final-answer tests**

In `apps/desktop/src-tauri/src/agent/turn_outcome.rs`, update existing calls:

```rust
final_answer_instruction(Some(&trace), None)
final_answer_instruction(None, None)
```

Expected: no existing test still calls `final_answer_instruction(...)` with one argument.

- [x] **Step 4: Pass latest turn state from the session loop**

In `apps/desktop/src-tauri/src/agent/session/loop.rs`, inside `finalize_turn(...)`, before building the final summary prompt, clone the latest turn once:

```rust
let latest_turn_for_final_answer = lock_unpoisoned(&self.latest_turn).clone();
```

Then update the final instruction call:

```rust
msgs.push(ChatMessage::user(&final_answer_instruction(
    verification_trace.as_ref(),
    latest_turn_for_final_answer.as_ref(),
)));
```

Expected: the final-answer prompt now receives the latest preview evidence without holding the `latest_turn` lock during model calls.

- [x] **Step 5: Run the focused Rust test and verify it passes**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::turn_outcome --lib
```

Expected: all `agent::turn_outcome` tests pass.

- [x] **Step 6: Run the focused session loop tests**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::loop_test --lib
```

Expected: session loop tests pass after the signature change.

## Task 4: Surface Preview Ownership In Desktop Status Details

**Files:**
- Modify: `apps/desktop/src/components/layout/ProjectStatusDetails.tsx`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`

- [x] **Step 1: Add the visible ownership line**

In `apps/desktop/src/components/layout/ProjectStatusDetails.tsx`, add one detail line between `预览地址` and `运行命令`:

```tsx
<DetailLine label="预览归属" value={runtime?.working_dir || "暂无"} />
```

The detail block should read:

```tsx
<DetailLine label="预览状态" value={runtime?.message || "暂无"} />
<DetailLine label="预览地址" value={runtime?.url || "暂无"} />
<DetailLine label="预览归属" value={runtime?.working_dir || "暂无"} />
<DetailLine label="运行命令" value={runtime?.command || "未检测到"} />
```

- [x] **Step 2: Add the acceptance test**

In `apps/desktop/e2e/acceptance.spec.ts`, add this test inside the existing `test.describe("Phase 7 acceptance surfaces", () => { ... })` block:

```ts
test("project status details expose preview ownership workspace", async ({ page }) => {
  const card = page.getByTestId("project-status-card");

  await card.getByRole("button", { name: "展开详情" }).click();

  await expect(card).toContainText("预览状态");
  await expect(card).toContainText("预览地址");
  await expect(card).toContainText("预览归属");
  await expect(card).toContainText("/Users/cabbos/project/forge");
  await expect(card).toContainText("http://localhost:1420");
});
```

- [x] **Step 3: Run the focused acceptance test and verify it passes**

Run:

```bash
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "project status details expose preview ownership workspace"
```

Expected: the focused Playwright test passes.

## Task 5: Update User-Visible Documentation

**Files:**
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`
- Modify only if needed: `scripts/acceptance.sh`

- [x] **Step 1: Update root runtime surface docs**

In `README.md`, in `## Desktop Runtime Surfaces`, update the `Acceptance:` bullet to mention preview ownership evidence:

```markdown
- Acceptance: browser smoke coverage for resume, diagnostics, provider probes/model catalog refresh, static fallback catalogs, selection/default saving, compact provider metadata rendering, provider evidence start readiness, custom provider profile templates/add/edit/delete, permissions, scheduler, A2A review, background task UI, and preview ownership details, plus runtime ownership gates for mocked restart evidence, provider usage, post-shell file effects, persisted A2A lineage, review-to-commit eligibility, gated headless policy/approval checks, and the real Rust `run_worktree_worker` harness.
```

- [x] **Step 2: Update desktop README**

In `apps/desktop/README.md`, in the `## 核心承诺` table, replace the `结果可判断` row with:

```markdown
| 结果可判断 | 每轮任务围绕当前任务、项目档案和交付状态组织；预览地址会配套显示归属项目，帮助用户决定继续、验证、修复或停止。 |
```

In the `## 能做什么` list, replace:

```markdown
- 记录任务状态、上下文来源、工具证据、检查点、验证结果和恢复状态。
```

with:

```markdown
- 记录任务状态、上下文来源、工具证据、预览归属、检查点、验证结果和恢复状态。
```

In the acceptance coverage paragraph, add `预览归属详情` after `Provider evidence start readiness`.

- [x] **Step 3: Update changelog**

Add this bullet at the top of `CHANGELOG.md` under `## Unreleased`:

```markdown
- Added explicit preview ownership evidence to delivery/status surfaces. Final answers that report a preview URL now receive the owning project path as final-answer guidance, and expanded project status details show the preview workspace so users do not have to infer ownership from the URL alone.
```

- [x] **Step 4: Check acceptance script dry-run alignment**

Run:

```bash
scripts/acceptance.sh --dry-run
```

Expected: the output still includes `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts`. If a new spec file was created instead of extending `acceptance.spec.ts`, update `scripts/acceptance.sh`; otherwise leave it unchanged.

## Task 6: Verification And Commit

**Files:**
- Modified files from Tasks 3-5.

- [x] **Step 1: Run focused Rust tests**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::turn_outcome --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::loop_test --lib
```

Expected: both commands pass.

- [x] **Step 2: Run focused desktop e2e**

Run:

```bash
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "project status details expose preview ownership workspace"
```

Expected: the focused acceptance test passes.

- [x] **Step 3: Run desktop build**

Run:

```bash
npm run build:desktop
```

Expected: command exits 0.

- [x] **Step 4: Run acceptance dry-run**

Run:

```bash
scripts/acceptance.sh --dry-run
```

Expected: command exits 0 and lists the desktop acceptance command.

- [ ] **Step 5: Run GitNexus staged change detection**

Stage only the intended files:

```bash
git add apps/desktop/src-tauri/src/agent/turn_outcome.rs \
  apps/desktop/src-tauri/src/agent/session/loop.rs \
  apps/desktop/src/components/layout/ProjectStatusDetails.tsx \
  apps/desktop/e2e/acceptance.spec.ts \
  README.md \
  apps/desktop/README.md \
  CHANGELOG.md
```

Then run:

```text
detect_changes({ repo: "forge", scope: "staged" })
```

Expected: changed symbols and affected flows match preview ownership/final-answer/status surfaces. If unrelated files are staged, unstage them before committing.

- [ ] **Step 6: Commit**

Run:

```bash
git commit -m "fix(desktop): surface preview ownership evidence"
```

Expected: commit succeeds with only the intended files staged.

**2026-06-25 implementation evidence:** Preview ownership evidence now flows into the final text-only answer prompt and the expanded project status details. GitNexus impact preflight returned `Target not found` / `UNKNOWN` for `final_answer_instruction`, `finalize_turn`, and `ProjectStatusDetails`; the earlier index refresh attempt was blocked by the known missing `tree-sitter-swift` package, so this slice proceeded with file-level review and limited-confidence impact analysis. RED verification failed first on the new `final_answer_instruction(None, Some(&turn))` signature and then on the acceptance card missing `预览归属`. GREEN verification passed:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::turn_outcome --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::loop_test --lib
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "project delivery details surface preview ownership"
npm run build:desktop
scripts/acceptance.sh --dry-run
```

Pending before closing the blocker: staged `detect_changes`, commit, and the manual Scenario 3 beta recheck in Forge.

## Task 7: Manual Beta Recheck

**Files:**
- Modify: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`

- [ ] **Step 1: Re-run Scenario 3 only**

In Forge, on `/Users/cabbos/project/forge-test-app`, paste:

```text
请启动当前项目预览，然后告诉我这个预览是否属于当前 demo 项目。如果端口被别的项目占用，请明确说明冲突，不要打开别的项目页面。
```

Expected pass signals:

- The final answer explicitly says the preview belongs to `/Users/cabbos/project/forge-test-app`, or explicitly says ownership cannot be confirmed.
- If there is a port conflict, the final answer says which ownership/conflict evidence is known.
- The answer does not only return `http://127.0.0.1:5173/`.

- [ ] **Step 2: Update the run log blocker status**

Append this note under the Scenario 3 section in `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`:

```markdown
Follow-up recheck:

- Pending: re-run after `fix(desktop): surface preview ownership evidence`.
```

Replace `Pending` with `Pass` only after the manual beta recheck has actually passed.

- [ ] **Step 3: Commit the recheck note**

Run GitNexus staged detection before committing:

```bash
git add apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md
```

```text
detect_changes({ repo: "forge", scope: "staged" })
```

Then commit:

```bash
git commit -m "docs(product): record preview ownership recheck"
```

Expected: commit succeeds with only the run log staged.
