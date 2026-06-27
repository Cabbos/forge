# Phase 8 Self-Use Runtime Proof Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the stability regression batch into concrete self-use proof by first hardening the highest-risk permission boundary, then proving a disposable-project edit/build loop, and finally closing the confirmation replay protocol gap.

**Architecture:** Keep backend permission gates as the authority and use frontend/Playwright tests only to prove the UI never claims a broader permission state than the backend can honor. Add small regression evidence before production changes; only change production logic when a focused failing test proves drift. Record manual-only restart evidence separately instead of pretending mocked browser restarts are full desktop restarts.

**Tech Stack:** Tauri Rust backend, React/TypeScript frontend, Zustand store, IndexedDB persistence, Playwright e2e, Node test runner, Cargo tests, GitNexus impact/detect_changes.

**Status:** Tasks 1-4 complete for the implemented Phase 8 slice. Follow-up acceptance gates now cover permission-policy/live-session sync and `/code-review` calibration, a disposable-project readiness preflight now guards the remaining live edit/build loop, a clean non-destructive target worktree has been prepared for rows #1-#3, an evidence collector now standardizes changed-file/diff/build packet capture, an evidence validator provides a strict completion gate after live row evidence exists, an archive helper writes validated evidence into product docs, a manual JSON template generator prevents final-answer/confirmation field drift, a manual JSON reviewer catches prompt/placeholder/field drift before archive, a row finalizer combines manual review, strict validation, and archive after live evidence is filled, a row runbook helper prints the exact live-run command sequence, a row status helper reports the next incomplete row from archived validation files, and a desktop UI observer preflight prevents local automation visibility failures from being mistaken for Forge runtime proof. The live Forge UI run and true Tauri force-quit restart smoke remain manual/not-run evidence items.

---

## Acceptance Contract

- **P8-A1 Safety Boundary Honesty:** Trust and Full Access must not auto-approve visible confirmation cards whose affected files are outside the current workspace, even when the confirmation's workspace label points at the current project.
- **P8-A2 Secret Boundary Honesty:** Trust must not auto-approve sensitive workspace writes such as `.env`; Full Access may skip the prompt only when the backend classifies the operation as same-workspace and confirmable, never for external paths.
- **P8-A3 Disposable Loop Proof:** A disposable-project edit/build loop records exact changed files, build/check result, and final-answer evidence without manual controller writes. Current state: backend policy and acceptance-gate evidence cover rows #1/#2/#3, `scripts/disposable-loop-preflight.mjs` records whether a project is ready for fresh evidence, `scripts/prepare-disposable-loop-project.mjs` can create a clean target without resetting a dirty source project, `scripts/desktop-ui-evidence-preflight.mjs` records whether local desktop automation can be trusted for live UI evidence, `scripts/phase8-disposable-loop-status.mjs` reports archived row coverage and the next incomplete row, `scripts/phase8-disposable-loop-runbook.mjs` prints the row prompt and command sequence, `scripts/create-disposable-loop-manual-json.mjs` emits row-specific manual evidence templates, `scripts/review-disposable-loop-manual-json.mjs` reviews filled manual evidence before strict archive, `scripts/collect-disposable-loop-evidence.mjs` captures changed-file/diff/build packets after live rows, `scripts/finalize-disposable-loop-row.mjs` combines manual review, strict validation, and archive after live rows, `scripts/validate-disposable-loop-evidence.mjs` fails strict validation until row-specific evidence is complete, and `scripts/archive-disposable-loop-evidence.mjs` archives validated packets, but the live Forge UI loop remains not run end to end.
- **P8-A4 Confirmation Replay Protocol:** Resolved confirmations have an explicit replayable event or persisted transcript marker so restart/history can show approved/declined state without inferring from interrupted `confirm_ask` metadata.
- **P8-A5 Evidence Trail:** `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md` records which regression-batch rows are automated, manual-only, passed, failed, or still not run.
- **P8-A6 Verification:** Focused Playwright, Node, Cargo, acceptance dry-run, `git diff --check`, and GitNexus `detect_changes` pass before any commit.

## File Structure

- Create: `docs/superpowers/plans/2026-06-27-phase8-self-use-runtime-proof.md`
  - Owns Phase 8 task tracking and final evidence.
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
  - Adds safety-boundary product specs for external-path and secret-like confirmations.
- Modify only if tests fail: `apps/desktop/src/components/session/ComposerPermissionModeButton.tsx`
  - Tightens composer-side pending confirmation takeover eligibility.
- Modify only if tests fail: `apps/desktop/src/components/layout/ProjectStatusCard.tsx`
  - Tightens project-status pending confirmation takeover eligibility.
- Modify only if needed: `apps/desktop/src/lib/write-boundary.ts`
  - Preserves raw affected-file paths or exposes boundary classification needed for safe takeover checks.
- Modify: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`
  - Records Phase 8 regression-batch evidence and residual risk.
- Modify later: `apps/desktop/src-tauri/src/protocol/events.rs`, `apps/desktop/src/lib/protocol.ts`, `apps/desktop/src-tauri/src/ipc/confirmations.rs`, `apps/desktop/src/store/event-dispatch.ts`, and related tests
  - Adds explicit confirmation response replay support if Task 3 proceeds.

## Task 1: Safety Boundary UI Regression

**Purpose:** Prove Trust/Full Access never make an external-path or sensitive confirmation card look approved unless the backend-safe boundary really permits it.

**Files:**
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
- Modify if failing: `apps/desktop/src/components/session/ComposerPermissionModeButton.tsx`
- Modify if failing: `apps/desktop/src/components/layout/ProjectStatusCard.tsx`
- Modify if needed: `apps/desktop/src/lib/write-boundary.ts`

- [x] **Step 1: Run GitNexus impact before production edits**

Run before editing any production symbol:

```text
impact({ repo: "forge", target: "findLatestPendingWorkspaceConfirm", file_path: "apps/desktop/src/components/session/ComposerPermissionModeButton.tsx", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "findLatestPendingWorkspaceConfirm", file_path: "apps/desktop/src/components/layout/ProjectStatusCard.tsx", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "parseWriteBoundary", file_path: "apps/desktop/src/lib/write-boundary.ts", direction: "upstream", maxDepth: 2, summaryOnly: true })
```

Expected: record any HIGH/CRITICAL risk before production edits. If only e2e tests are edited, record that no production symbol changed.

- [x] **Step 2: Add failing external-path Playwright spec**

Add this spec to `apps/desktop/e2e/acceptance.spec.ts` near the permission-mode specs:

```ts
test("composer full access does not approve an external-path confirmation card", async ({ page }) => {
  const sessionId = "composer-full-access-external-boundary";
  await page.evaluate((id) => {
    // @ts-expect-error acceptance mock
    window.__mockSessionId = id;
  }, sessionId);

  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await expect(page.getByTestId("composer-lane")).toBeVisible();
  await page.waitForFunction(() => {
    // @ts-expect-error Tauri listener registry installed by setup()
    return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
  });

  await simulateStream(page, sessionId, [
    {
      event_type: "confirm_ask",
      session_id: sessionId,
      block_id: "full-access-external-write",
      question: "Allow external write?",
      kind: "file_write",
      boundary: {
        workspace_name: "forge",
        workspace_path: "/Users/cabbos/project/forge",
        operation: "write_file",
        affected_files: ["/Users/cabbos/.ssh/config"],
        risk_level: "high",
        checkpoint_status: "ready",
        warning: "项目外写入不会被完全访问自动放行。",
      },
    },
  ], 1);

  const confirmPanel = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
  await expect(confirmPanel).toContainText(".ssh");
  await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();

  await page.getByTestId("composer-permission-mode").click();
  await page.getByTestId("composer-permission-full-access").click();

  await expect(page.getByTestId("composer-permission-mode")).toContainText("完全访问");
  await page.waitForTimeout(100);
  const confirmArgs = await page.evaluate(() => {
    // @ts-expect-error acceptance mock
    return window.__lastConfirmResponseArgs ?? null;
  });
  expect(confirmArgs).toBeNull();
  await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();
});
```

Expected before fix, if drift exists: the test fails because Full Access auto-approves the external-path card.

- [x] **Step 3: Add trust-sensitive-path Playwright spec**

Add this spec:

```ts
test("project trust does not approve a sensitive workspace confirmation card", async ({ page }) => {
  const sessionId = "project-trust-sensitive-boundary";
  await page.evaluate((id) => {
    // @ts-expect-error acceptance mock
    window.__mockSessionId = id;
  }, sessionId);

  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await expect(page.getByTestId("composer-lane")).toBeVisible();
  await page.waitForFunction(() => {
    // @ts-expect-error Tauri listener registry installed by setup()
    return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
  });

  await simulateStream(page, sessionId, [
    {
      event_type: "confirm_ask",
      session_id: sessionId,
      block_id: "trust-sensitive-write",
      question: "Allow .env write?",
      kind: "file_write",
      boundary: {
        workspace_name: "forge",
        workspace_path: "/Users/cabbos/project/forge",
        operation: "write_file",
        affected_files: ["/Users/cabbos/project/forge/.env"],
        risk_level: "high",
        checkpoint_status: "ready",
        warning: "敏感文件仍需手动确认。",
      },
    },
  ], 1);

  const confirmPanel = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
  await expect(confirmPanel).toContainText(".env");
  await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();

  await page.getByRole("button", { name: "打开项目档案" }).click();
  const card = page.getByTestId("project-archive-panel").getByTestId("project-status-card");
  await card.getByRole("button", { name: "信任当前项目" }).click();

  await expect(card.getByTestId("project-status-permission-mode")).toContainText("已信任");
  await page.waitForTimeout(100);
  const confirmArgs = await page.evaluate(() => {
    // @ts-expect-error acceptance mock
    return window.__lastConfirmResponseArgs ?? null;
  });
  expect(confirmArgs).toBeNull();
  await expect(confirmPanel.getByTestId("confirm-approve")).toBeVisible();
});
```

Expected before fix, if drift exists: the test fails because Trust auto-approves `.env`.

- [x] **Step 4: Implement minimal production fix if either spec fails**

The tests failed as expected: Full Access auto-approved `full-access-external-write`, and Trust auto-approved `trust-sensitive-write`.

The fix keeps normal Full Access command approvals intact by using the raw `affected_files` metadata instead of blocking every high-risk confirmation. Composer and Project Status now require:

- absolute affected paths to be exactly the current workspace or inside it;
- `~` paths, `../` traversal, and absolute external paths to remain manual;
- Trust mode to keep `.env` and `.env.*` workspace files manual;
- Full Access to continue allowing same-workspace sensitive files when the backend classified them as confirmable.

```ts
function isAutoApprovableBoundary(
  boundary: unknown,
  workingDir: string,
  allowSensitiveWorkspaceFiles: boolean,
): boolean {
  if (!boundary || typeof boundary !== "object" || Array.isArray(boundary)) return false;
  const rawFiles = (boundary as { affected_files?: unknown }).affected_files;
  if (!Array.isArray(rawFiles)) return true;
  const normalizedWorkingDir = normalizeProjectPath(workingDir);
  return rawFiles.every((file) => {
    if (typeof file !== "string") return false;
    const normalizedFile = normalizeProjectPath(file);
    const projectRelativeFile = normalizedFile.startsWith(`${normalizedWorkingDir}/`)
      ? normalizedFile.slice(normalizedWorkingDir.length + 1)
      : normalizedFile;
    if (normalizedFile.startsWith("~")) return false;
    if (normalizedFile.startsWith("/") && normalizedFile !== normalizedWorkingDir && !normalizedFile.startsWith(`${normalizedWorkingDir}/`)) return false;
    if (projectRelativeFile === ".." || projectRelativeFile.startsWith("../") || projectRelativeFile.includes("/../")) return false;
    if (!allowSensitiveWorkspaceFiles && isSensitiveProjectPath(projectRelativeFile)) return false;
    return true;
  });
}
```

This mirrors the backend semantics at the UI takeover layer without changing backend permission decisions.

- [x] **Step 5: Run focused verification**

Run:

```bash
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "external-path|sensitive workspace|trust|full access"
git diff --check
```

Expected: the new safety specs pass and the existing trust/full-access specs remain green.

### Task 1 Evidence

- Impact: `findLatestPendingWorkspaceConfirm` in `ComposerPermissionModeButton.tsx` reported LOW risk, 1 direct affected process (`ComposerPermissionModeButton`).
- Impact: `findLatestPendingWorkspaceConfirm` in `ProjectStatusCard.tsx` reported LOW risk, 2 direct affected processes (`fullAccessCurrentProject`, `trustCurrentProject`).
- Impact: `parseWriteBoundary` reported HIGH risk, so it was not modified.
- Red test: `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "external-path|sensitive workspace"` initially failed because the UI sent `confirm_response approved=true` for both `full-access-external-write` and `trust-sensitive-write`.
- Spec review follow-up: added explicit Trust external-path coverage and `.env.local` coverage after review found P8-A1/P8-A2 evidence gaps.
- Green test: `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "permission|external-path|sensitive workspace|dotenv variant|trust|full access"` passed, 12 specs.
- Build/dry-run: `npm --prefix apps/desktop run build`, `scripts/acceptance.sh --dry-run`, and `git diff --check` passed after the Task 1 fix.
- GitNexus all-change audit: `detect_changes({ repo: "forge", scope: "all" })` reported HIGH risk across 6 affected Trust/FullAccess takeover processes, which are the exact flows covered by the 12-spec Playwright run above.
- GitNexus staged audit: `detect_changes({ repo: "forge", scope: "staged" })` reported HIGH risk, 3 changed symbols, 6 affected Trust/FullAccess takeover processes, and no unrelated affected process families.

## Task 2: Disposable Edit/Build Loop Proof

**Purpose:** Turn regression-batch rows #1/#2/#3 into a repeatable disposable-project loop with exact diff and build evidence.

**Files:**
- Modify: `apps/desktop/docs/product/stability-regression-batch.md`
- Modify: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`
- Create: `apps/desktop/docs/product/phase8-disposable-loop-protocol.md`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

- [x] **Step 1: Define disposable-project protocol**
- [x] **Step 2: Record exact command and diff evidence format**
- [x] **Step 3: Run or mark manual-only evidence**
- [x] **Step 4: Verify docs and acceptance dry-run**

### Task 2 Evidence

- Protocol: `apps/desktop/docs/product/phase8-disposable-loop-protocol.md` now defines the no-controller-writes setup, prompt sequence, required evidence, pass criteria, failure taxonomy, and completion evidence template for rows #1/#2/#3.
- Batch table: rows #1/#2/#3 now reference the protocol, carry automated permission-policy evidence, and remain honest that live final-answer/diff/build evidence still follows the protocol and has not been run end to end.
- Beta log: `Phase 8 Disposable Edit/Build Loop - 2026-06-27` records required evidence fields and `Status: Not yet run`.
- Acceptance dry-run: `scripts/acceptance.sh --dry-run` now advertises `manual disposable edit/build loop protocol` and checks both the protocol file and beta log section.
- Impact: `parseDryRunEntries` in `scripts/acceptance.test.mjs` reported LOW risk, 0 direct callers, and 0 affected processes before the acceptance matrix update.
- Spec review follow-up: beta log evidence fields were aligned with the protocol template by adding Row #1 diff summary, Row #2 final answer, and Row #3 output summary.
- Code quality review follow-up: protocol and beta log now include Row #2 no-external-write evidence so the style polish row carries its full required evidence into the completion template.
- Verification: `node --test scripts/acceptance.test.mjs`, `scripts/acceptance.sh --dry-run`, `scripts/acceptance.sh --help`, `bash -n scripts/acceptance.sh`, and `git diff --check` passed after the review follow-ups.
- GitNexus staged audit: `detect_changes({ repo: "forge", scope: "staged" })` reported LOW risk, 0 changed symbols, and 0 affected processes.
- Follow-up acceptance gate: `test: add permission policy acceptance gate` added `permission_handlers`, `harness::permissions`, and `harness::shell_policy` to `scripts/acceptance.sh`, proving permission mode inheritance, live-session harness sync, routine workspace write/shell allowance, and catastrophic/external blocking remain covered by the final gate.

## Task 3: Confirmation Response Replay Protocol

**Purpose:** Replace the recorded protocol gap with explicit approved/declined confirmation replay evidence.

**Files:**
- Modify: `apps/desktop/src-tauri/src/protocol/events.rs`
- Modify: `apps/desktop/src/lib/protocol.ts`
- Modify: `apps/desktop/src-tauri/src/ipc/confirmations.rs`
- Modify: `apps/desktop/src/store/event-dispatch.ts`
- Modify: `apps/desktop/src/store/blocks.ts`
- Modify/add focused Rust and TS tests.

- [x] **Step 1: Run GitNexus impact on protocol/store symbols**
- [x] **Step 2: Add failing Rust/TS tests for resolved confirmation replay**
- [x] **Step 3: Add minimal `confirm_response` stream event or persisted transcript marker**
- [x] **Step 4: Verify restart/history projection**
- [x] **Step 5: Run focused Node, Cargo, Playwright checks**

### Task 3 Evidence

- Impact: `StreamEvent` in `apps/desktop/src-tauri/src/protocol/events.rs` reported LOW risk, 0 direct callers, 0 affected processes.
- Impact: `confirm_response_for_state` reported LOW risk, 3 direct test/IPC callers, 0 affected processes.
- Impact: `confirm_response` IPC entry reported LOW risk, 0 indexed direct callers, 0 affected processes.
- Impact: `emit_restored_session_startup` reported LOW risk, 2 direct callers, 1 affected startup process (`run`).
- Impact warning: `eventToBlock` and `createOutputEventDispatcher` both reported CRITICAL risk across 18 UI processes, so the frontend change was kept to a narrow `confirm_response` projection branch and focused tests.
- Protocol: Rust and TypeScript now share a `confirm_response` stream event with `block_id`, optional `question/kind/boundary`, `approved` (`true`/`false`/`null`), `responded_at_ms`, optional `reason`, and replay metadata.
- Runtime marker: live `confirm_response` IPC resolves the pending sender and emits a transcript-backed response event when a pending descriptor is available; restored pending confirmations emit the existing interrupted `confirm_ask` plus an explicit `confirm_response` marker with `approved: null`.
- Projection: transcript/history and live dispatch both resolve the existing `confirm_ask` block in place; orphan response events create a completed non-interactive audit block.
- Spec review: no blocking gaps found; remaining restart proof is still bounded to transcript projection tests plus restore-marker Rust tests, not a full real-desktop restart harness.
- Code quality follow-up: `updateBlock` now merges metadata with the current block so optimistic confirmation writes cannot erase `confirm_response` replay metadata emitted by the backend.
- Code quality follow-up: when a live response has a pending sender but no descriptor, `confirm_response_for_state` now emits a minimal marker for the single-live-session case instead of silently producing no transcript marker.
- Tests: `node --test apps/desktop/src/store/blocks.test.ts apps/desktop/src/store/event-dispatch.test.ts apps/desktop/src/store/persistence-hydration.test.ts` passed, 59 tests.
- Rust: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml protocol::events --lib`, `agent::session_events --lib`, `ipc::confirmations --lib`, `autosave --lib`, and `ipc::session_lifecycle --lib` passed. After quality follow-ups, `ipc::confirmations --lib` passed with 4 tests and `agent::session_events --lib` passed with 10 tests.
- Playwright: `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "confirm response replay"` passed, 1 spec.
- Build: `npm run build:desktop` and `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` passed.
- Acceptance dry-run and formatting: `scripts/acceptance.sh --dry-run` and `git diff --check` passed.

## Task 4: Final Phase 8 Verification And Commit

**Purpose:** Prove Phase 8 changes are coherent before committing.

**Files:**
- Modify: `docs/superpowers/plans/2026-06-27-phase8-self-use-runtime-proof.md`

- [x] **Step 1: Run focused verification from completed tasks**
- [x] **Step 2: Run `scripts/acceptance.sh --dry-run`**
- [x] **Step 3: Run `git diff --check`**
- [x] **Step 4: Run GitNexus `detect_changes({ repo: "forge", scope: "staged" })`**
- [x] **Step 5: Commit the completed Phase 8 slice**

### Task 4 Evidence

- Focused checks rerun after review follow-ups: Node store tests passed (59 tests), Rust `ipc::confirmations` passed (4 tests), Rust `agent::session_events` passed (10 tests), Playwright `confirm response replay` passed (1 spec), `npm run build:desktop` passed, and `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` passed.
- Final dry-run/formatting: `scripts/acceptance.sh --dry-run`, `git diff --check`, and `git diff --cached --check` passed.
- GitNexus staged audit: `detect_changes({ repo: "forge", scope: "staged" })` reported MEDIUM risk, 54 changed symbols, 17 changed files, and 4 affected `createOutputEventDispatcher` runtime projection flows; the touched dispatcher path is covered by the focused store tests and Playwright smoke above.
- Commit: Phase 8 Task 3/4 slice committed as `feat: add confirmation response replay events`.
- Follow-up commits:
  - `test: add code review calibration acceptance gate` added `capability_context` to the final acceptance matrix so `/code-review` findings-first and severity-calibration behavior stays executable evidence.
  - `test: add permission policy acceptance gate` added permission-mode/live-session sync and shell-policy contract tests to the final acceptance matrix, closing the evidence drift between mocked UI trust controls and backend session gates.
  - `test: add confirmation replay acceptance gate` adds a focused acceptance row for Rust `confirm_response` replay markers plus Playwright replay/hydration specs, preventing older restart plans from regressing to the pre-`confirm_response` protocol gap.

## Notes From Initial Subagent Audit

- The ten-row stability regression batch has not been run end-to-end; the beta log still marks it `Not yet run`.
- Rows #7/#8 have product-level mocked UI safety evidence plus backend policy evidence.
- Rows #1/#2/#3 now have backend policy and acceptance-gate evidence, but still need the live disposable-project final-answer/diff/build proof that users feel.
- Row #5 now has executable hidden-intent contract evidence through `capability_context`, while the older beta run remains Pass/P2 because its model output was too aggressive.
- Restart remains manual-only until a real desktop restart harness exists. The acceptance matrix now runs `node scripts/desktop-restart-harness-preflight.mjs --json` so the current `blocked_official_macos` state is explicit instead of implied.
