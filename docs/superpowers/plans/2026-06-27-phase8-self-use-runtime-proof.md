# Phase 8 Self-Use Runtime Proof Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the stability regression batch into concrete self-use proof by first hardening the highest-risk permission boundary, then proving a disposable-project edit/build loop, and finally closing the confirmation replay protocol gap.

**Architecture:** Keep backend permission gates as the authority and use frontend/Playwright tests only to prove the UI never claims a broader permission state than the backend can honor. Add small regression evidence before production changes; only change production logic when a focused failing test proves drift. Record manual-only restart evidence separately instead of pretending mocked browser restarts are full desktop restarts.

**Tech Stack:** Tauri Rust backend, React/TypeScript frontend, Zustand store, IndexedDB persistence, Playwright e2e, Node test runner, Cargo tests, GitNexus impact/detect_changes.

**Status:** Task 1 complete; Tasks 2-4 pending.

---

## Acceptance Contract

- **P8-A1 Safety Boundary Honesty:** Trust and Full Access must not auto-approve visible confirmation cards whose affected files are outside the current workspace, even when the confirmation's workspace label points at the current project.
- **P8-A2 Secret Boundary Honesty:** Trust must not auto-approve sensitive workspace writes such as `.env`; Full Access may skip the prompt only when the backend classifies the operation as same-workspace and confirmable, never for external paths.
- **P8-A3 Disposable Loop Proof:** A disposable-project edit/build loop records exact changed files, build/check result, and final-answer evidence without manual controller writes.
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
- Create if useful: `apps/desktop/docs/product/phase8-disposable-loop-protocol.md`

- [ ] **Step 1: Define disposable-project protocol**
- [ ] **Step 2: Record exact command and diff evidence format**
- [ ] **Step 3: Run or mark manual-only evidence**
- [ ] **Step 4: Verify docs and acceptance dry-run**

## Task 3: Confirmation Response Replay Protocol

**Purpose:** Replace the recorded protocol gap with explicit approved/declined confirmation replay evidence.

**Files:**
- Modify: `apps/desktop/src-tauri/src/protocol/events.rs`
- Modify: `apps/desktop/src/lib/protocol.ts`
- Modify: `apps/desktop/src-tauri/src/ipc/confirmations.rs`
- Modify: `apps/desktop/src/store/event-dispatch.ts`
- Modify: `apps/desktop/src/store/blocks.ts`
- Modify/add focused Rust and TS tests.

- [ ] **Step 1: Run GitNexus impact on protocol/store symbols**
- [ ] **Step 2: Add failing Rust/TS tests for resolved confirmation replay**
- [ ] **Step 3: Add minimal `confirm_response` stream event or persisted transcript marker**
- [ ] **Step 4: Verify restart/history projection**
- [ ] **Step 5: Run focused Node, Cargo, Playwright checks**

## Task 4: Final Phase 8 Verification And Commit

**Purpose:** Prove Phase 8 changes are coherent before committing.

**Files:**
- Modify: `docs/superpowers/plans/2026-06-27-phase8-self-use-runtime-proof.md`

- [ ] **Step 1: Run focused verification from completed tasks**
- [ ] **Step 2: Run `scripts/acceptance.sh --dry-run`**
- [ ] **Step 3: Run `git diff --check`**
- [ ] **Step 4: Run GitNexus `detect_changes({ repo: "forge", scope: "staged" })`**
- [ ] **Step 5: Commit the completed Phase 8 slice**

## Notes From Initial Subagent Audit

- The ten-row stability regression batch has not been run end-to-end; the beta log still marks it `Not yet run`.
- Rows #7/#8 are the highest-risk first slice because external path and secret-like writes are safety boundaries.
- Rows #1/#2/#3 are the highest product-value second slice because they prove the edit/build trust loop users feel.
- Restart remains manual-only until a real desktop restart harness exists.
