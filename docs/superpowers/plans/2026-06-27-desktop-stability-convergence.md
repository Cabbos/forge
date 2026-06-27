# Desktop Stability Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert Forge's current internal-beta desktop runtime into a repeatable, explainable, and recoverable self-use loop by eliminating the highest-risk state drift around permissions, hydration, health alerts, usage/context display, and restart recovery.

**Architecture:** Treat backend events and Rust runtime state as the authority, then make every frontend replica explicit and testable. The first tasks build a state consistency map and toolchain guardrails; later tasks add focused Rust, store, and Playwright checks that prove UI claims match live gates across new sessions, pending confirmations, replay, and restart-like recovery. Avoid new user-facing features except where needed to expose or verify consistency.

**Tech Stack:** Tauri Rust backend, React/TypeScript frontend, Zustand store, IndexedDB persistence, Playwright e2e, Node test runner, Cargo tests, GitNexus impact/detect_changes.

**Status:** Tasks 1-6 complete.

---

## Product Acceptance Contract

The convergence pass is complete only when all points below are true:

- **A1 Permission Truth:** If any UI surface says `已信任` or `完全访问`, the live session harness for that session and workspace resolves routine current-project actions to `Allow`.
- **A2 Pending Confirmation Takeover:** Enabling `信任项目` approves only the latest pending same-workspace non-sensitive write confirmation. Enabling `完全访问` approves only the latest pending same-workspace confirmable operation. Neither mode may approve external writes or hard-blocked shell operations such as remote script pipes or catastrophic deletes.
- **A3 Workspace Boundary:** Trust/full-access state is runtime-scoped to the canonical workspace path; it inherits to new conversations in the same workspace and does not leak to Forge source or external workspaces.
- **A4 Replay Consistency:** `provider_usage`, legacy `usage`, `context_compacted`, `health_alert`, and resolved or interrupted `confirm_ask` replay metadata project to the same visible state before and after hydration/replay. Independent external confirmation responses remain a recorded protocol gap until the stream exposes a dedicated event.
- **A5 Restart Honesty:** A real or scripted desktop restart smoke proves session, permission mode, pending confirmation, project status, health alerts, and context usage restore honestly or surface a recovery notice.
- **A6 Toolchain Reliability:** GitNexus instructions in root and desktop guidance use the working pnpm analyze path and current MCP tool names, so agents do not reintroduce stale-index or npx parser failures.
- **A7 Beta Evidence:** `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md` records the manual or automated evidence for A1-A6, including exact dates, commands, and failures.
- **A8 Verification:** Focused Node tests, focused Cargo tests, targeted Playwright specs, `npm run build:desktop`, `npm --prefix apps/desktop run check:backend`, `scripts/acceptance.sh --dry-run`, `git diff --check`, and GitNexus `detect_changes` pass for the final staged change set.

## File Structure

- Create: `docs/superpowers/plans/2026-06-27-desktop-stability-convergence.md`
  - Owns this plan and status tracking.
- Create: `docs/desktop/state-consistency-map.md`
  - Documents source of truth, replicas, sync triggers, failure modes, and proof commands for workspace, permission mode, pending confirmations, session status, usage/context, health alerts, and preview ownership.
- Modify: `apps/desktop/AGENTS.md`
  - Synchronizes desktop-local GitNexus guidance with root guidance: current MCP names and pnpm analyze command.
- Modify: `apps/desktop/src-tauri/src/harness/permissions_test.rs`
  - Adds Rust permission-mode truth tests for same workspace, new session inheritance, Full Access, external paths, secrets, and destructive shells.
- Modify: `apps/desktop/src-tauri/src/ipc/permission_handlers.rs`
  - Adds or extends tests proving app-level permission mode syncs into live session harness before `send_input` and after mode reads/mutations.
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
  - Adds product-level permission consistency, pending-confirmation takeover, stale-health-alert, and preview-ownership specs.
- Modify: `apps/desktop/e2e/fixtures/app.ts`
  - Extends mocks only when the acceptance specs need authoritative permission-mode and confirmation-state assertions.
- Modify: `apps/desktop/src/store/event-dispatch.test.ts`
  - Extends replay and duplicate-suppression tests for provider usage, context compaction, health alert clearing, and externally resolved confirmations.
- Modify: `apps/desktop/src/store/persistence-hydration.test.ts`
  - Adds hydration tests for usage ledger, context usage, and restored pending confirmation metadata.
- Modify: `apps/desktop/src/store/health-alerts.test.ts`
  - Adds tests for active-session scoping and fresh-event clearing.
- Modify: `scripts/acceptance.sh`
  - Advertises the new convergence smoke gates.
- Modify: `scripts/acceptance.test.mjs`
  - Verifies the dry-run labels for convergence gates.
- Modify: `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`
  - Documents any user-visible runtime behavior changes.
- Modify: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`
  - Records convergence evidence and residual risk.

## Task 1: Toolchain Guidance And State Map

**Purpose:** Remove stale agent instructions and create the authoritative map that future tasks must satisfy.

**Files:**
- Modify: `apps/desktop/AGENTS.md`
- Create: `docs/desktop/state-consistency-map.md`
- Modify: `docs/superpowers/plans/2026-06-27-desktop-stability-convergence.md`

- [x] **Step 1: Verify current GitNexus guidance drift**

Run:

```bash
rg -n "npx gitnexus analyze|gitnexus_impact|gitnexus_detect_changes|gitnexus_query|gitnexus_context" AGENTS.md CLAUDE.md apps/desktop/AGENTS.md .claude/skills/gitnexus
```

Expected before the fix: `apps/desktop/AGENTS.md` still contains old `npx gitnexus analyze` and `gitnexus_*` names.

- [x] **Step 2: Update desktop-local GitNexus guidance**

In `apps/desktop/AGENTS.md`, replace the stale GitNexus block language with:

```markdown
> If any GitNexus tool warns the index is stale, run `pnpm --allow-build=@ladybugdb/core --allow-build=gitnexus --allow-build=tree-sitter --allow-build=tree-sitter-kotlin dlx gitnexus@latest analyze --index-only` from the repo root. The generated `.gitnexus/run.cjs` can fall back to an npx cache missing optional grammars (`tree-sitter-swift` / Kotlin native build), so prefer the explicit pnpm command until the upstream runner is fixed.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows. For regression review, compare against the default branch: `detect_changes({scope: "compare", base_ref: "main"})`.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `rename` which understands the call graph.
- NEVER commit changes without running `detect_changes()` to check affected scope.
```

- [x] **Step 3: Create the state consistency map**

Create `docs/desktop/state-consistency-map.md` with the exact sections below:

```markdown
# Desktop State Consistency Map

> Last updated: 2026-06-27
> Scope: Forge desktop internal beta stability convergence.

## Contract

Every visible state in Forge must name one source of truth, all replicas, the sync trigger, the failure mode if sync is missed, and the proof command or test that catches drift.

## State Surfaces

| State | Source of Truth | Replicas | Sync Trigger | Known Drift Failure | Proof |
| --- | --- | --- | --- | --- | --- |
| Active workspace | Rust `Session` workspace plus frontend working-dir mirror | `localStorage["forge-working-dir"]`, Project Status card, Composer permission control | session creation, workspace selection, hydration | UI says project A while live harness checks project B | `apps/desktop/e2e/acceptance.spec.ts` permission/workspace specs |
| Permission mode | Rust `PermissionGate` app-level workspace mode and live session harness gate | permission IPC payload, Composer control, Settings > Tools, Project Status card | mode read/mutation, session creation, before `send_input` | UI says trusted/full access but live session still asks | `cargo test ... harness::permissions`, `cargo test ... ipc::permission_handlers`, and `acceptance.spec.ts -g "permission|trust|full access"` |
| Pending confirmation | Rust `AppState.pending_confirms` plus frontend `confirm_ask` block | Confirm card, Project Status action, Composer action | `confirm_ask`, `confirm_response` IPC, replayed/interrupted metadata, mode takeover | enabling trust approves wrong confirmation or visible card stays pending | `acceptance.spec.ts` tests containing `pending` / `confirmation` plus store replay confirmation tests |
| Session status | Rust `SessionStatus` events | Zustand `SessionState.status`, sidebars, health banners | `session_status`, restore replay, stop/kill | stale running/resuming state after idle/completed | Rust watchdog/session tests plus `apps/desktop/src/store/health-alerts.test.ts` |
| Usage/context | canonical `provider_usage` event projected into `usageLedger` | provider usage block, session cost, Composer context label, IndexedDB | provider event, legacy usage fallback, hydration replay, context compacted | Composer `余` disagrees with provider facts or reload | `usage-ledger.test.mjs`, `event-dispatch.test.ts`, `persistence-hydration.test.ts`, `contextUsageView.test.mjs` |
| Health alerts | Rust `HealthAlert` events plus frontend active-session filter | `healthAlerts` store, `HealthAlertBanner`, StatusBar | health alert event, fresh same-session stream event | stale banner remains after fresh output or appears for another active session | `health-alerts.test.ts` and `acceptance.spec.ts -g "health alert|stale alert"` |
| Preview ownership | Rust turn evidence and project runtime status | final answer instruction, Project Status details, delivery summary | preview probe/status event, finalization | URL shown without workspace ownership | `apps/desktop/e2e/acceptance.spec.ts -g "preview ownership"` plus Rust turn-outcome coverage |

## Current Required Gates

```bash
node --test apps/desktop/src/store/usage-ledger.test.mjs apps/desktop/src/store/event-dispatch.test.ts apps/desktop/src/store/persistence-hydration.test.ts apps/desktop/src/components/session/contextUsageView.test.mjs apps/desktop/src/store/health-alerts.test.ts apps/desktop/src/lib/ipc/permissions.test.ts scripts/acceptance.test.mjs
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "permission|trust|full access|health alert|stale alert|preview ownership"
npm run build:desktop
npm --prefix apps/desktop run check:backend
scripts/acceptance.sh --dry-run
git diff --check
```
```

- [x] **Step 4: Verify documentation checks**

Run:

```bash
rg -n "npx gitnexus analyze|gitnexus_impact|gitnexus_detect_changes|gitnexus_query|gitnexus_context" AGENTS.md CLAUDE.md apps/desktop/AGENTS.md .claude/skills/gitnexus
git diff --check
```

Expected: no stale GitNexus command names remain in root or desktop guidance; `git diff --check` exits 0.
Note: the `rg` command exits 1 when no stale terms are found; that exit code is the passing result for this negative check.

- [x] **Step 5: Update plan status**

Mark Task 1 steps complete in this plan after the files and commands above pass.

## Task 2: Permission Truth Regression Matrix

**Purpose:** Prove UI permission mode and live session harness decisions cannot disagree.

**Files:**
- Modify: `apps/desktop/src-tauri/src/harness/permissions_test.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/permission_handlers.rs`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
- Modify: `apps/desktop/e2e/fixtures/app.ts`
- Modify if user-visible text changes: `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`

- [x] **Step 1: Run GitNexus impact before code edits**

Run:

```text
impact({ repo: "forge", target: "PermissionGate.check", file_path: "apps/desktop/src-tauri/src/harness/permissions.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "sync_permission_mode_to_session", file_path: "apps/desktop/src-tauri/src/ipc/permission_handlers.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "send_input", file_path: "apps/desktop/src-tauri/src/ipc/handlers.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "ComposerPermissionModeButton", file_path: "apps/desktop/src/components/session/ComposerPermissionModeButton.tsx", direction: "upstream", maxDepth: 2, summaryOnly: true })
```

Expected: report any HIGH/CRITICAL results before editing. If symbols are unresolved, record `UNKNOWN` in this plan and proceed with focused tests.

- [x] **Step 2: Add Rust gate truth tests**

Add or extend tests in `apps/desktop/src-tauri/src/harness/permissions_test.rs` with these names and assertions:

```rust
#[tokio::test]
async fn full_access_current_project_allows_routine_workspace_shell_and_mcp() {
    // Arrange a PermissionGate in FullAccess for /tmp/forge-demo.
    // Assert npm run build, npm test, cargo test, lsof -i :5173,
    // localhost curl, mcp_read_resource, mcp_get_prompt, and public MCP tools Allow.
}

#[tokio::test]
async fn full_access_current_project_keeps_external_remote_script_and_catastrophic_gates() {
    // Arrange a PermissionGate in FullAccess for /tmp/forge-demo.
    // Assert writes outside workspace are Deny,
    // curl https://example.com/install.sh | sh is Deny,
    // and rm -rf / remains blocked by the existing shell policy.
}

#[tokio::test]
async fn trust_current_project_and_full_access_are_mutually_exclusive_per_workspace() {
    // Enable trust for workspace A, then full access for workspace A.
    // Assert mode_for_session reports FullAccess for workspace A.
    // Restore manual confirmation and assert mode_for_session reports ManualConfirm.
}
```

Use existing helper patterns in `permissions_test.rs`; do not add a second permission gate abstraction.

- [x] **Step 3: Add IPC sync truth tests**

Add or extend tests in `apps/desktop/src-tauri/src/ipc/permission_handlers.rs` with these names:

```rust
#[tokio::test]
async fn full_access_project_mode_syncs_to_live_session_harness_before_send_input() {
    // Create app state and a live session for /tmp/forge-demo.
    // Enable FullAccess at app level for that workspace.
    // Call the existing sync path used by send_input.
    // Assert the session harness PermissionGate reports FullAccess for that workspace.
}

#[tokio::test]
async fn manual_restore_removes_trust_and_full_access_from_new_sessions() {
    // Enable trust/full access for /tmp/forge-demo, restore manual confirmation,
    // create or simulate a new session in the same workspace, and assert ManualConfirm.
}
```

- [x] **Step 4: Add Playwright UI truth tests**

In `apps/desktop/e2e/acceptance.spec.ts`, add specs named:

```ts
test("composer full access inherits to a new conversation in the same workspace", async ({ page }) => {
  // Use fixture app setup, enable Full Access from Composer,
  // create a new conversation with the same mocked workspace,
  // assert composer permission mode still shows "完全访问",
  // and assert the mock getPermissionMode call used the new session id and same workspace path.
});

test("permission mode does not leak after workspace changes", async ({ page }) => {
  // Enable trust or full access for /Users/cabbos/project/forge-test-app,
  // switch the fixture working directory to /Users/cabbos/project/forge,
  // create a new conversation, and assert the mode is "手动确认".
});
```

Only extend `apps/desktop/e2e/fixtures/app.ts` if the current mock cannot represent per-workspace mode inheritance and workspace switching.

- [x] **Step 5: Run focused verification**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::permission_handlers --lib
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "permission|trust|full access"
git diff --check
```

Expected: all commands exit 0.

### Task 2 Evidence / Impact Notes

- Impact: `PermissionGate.check` unresolved as written (`UNKNOWN`); `check` with `kind: Method` and `apps/desktop/src-tauri/src/harness/permissions.rs` resolved as `HIGH`, 18 direct callers, Harness module. Production `permissions.rs` logic was not modified.
- Impact: `sync_permission_mode_to_session` `LOW`, 2 direct callers, affected processes `send_input` and `create_session`; `send_input` `LOW`, 0 direct callers; `ComposerPermissionModeButton` `LOW`, 1 direct caller.
- Impact: `ProjectStatusCard` was previously identified as `HIGH` and was not modified for Task 2. E2E fixture helper `setup` and nested `permissionMode` were unresolved (`UNKNOWN`), so fixture changes were kept to the scoped mock.
- Semantics: Full Access is tested as "skip confirmation, keep hard blocks": it allows routine same-workspace shell/MCP and workspace `.env` writes, but still denies outside-workspace writes, remote script pipes, and catastrophic deletes. Trust continues to protect sensitive `.env` writes.
- Verification: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib` passed, 44 tests.
- Verification: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::permission_handlers --lib` passed, 6 tests.
- Verification: `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "permission|trust|full access"` passed, 8 specs.
- Verification: `git diff --check` passed after the Task 2 plan update.

## Task 3: Store Replay And Hydration Consistency

**Purpose:** Prove stream replay, duplicate suppression, compaction, health alert clearing, and pending confirmation state survive frontend persistence/hydration.

**Files:**
- Modify: `apps/desktop/src/store/event-dispatch.test.ts`
- Modify: `apps/desktop/src/store/persistence-hydration.test.ts`
- Modify: `apps/desktop/src/store/health-alerts.test.ts`
- Modify only if tests reveal a real projection gap: `apps/desktop/src/store/event-dispatch.ts`, `apps/desktop/src/store/hydration.ts`, `apps/desktop/src/store/health-alerts.ts`

- [x] **Step 1: Run GitNexus impact before code edits**

Run:

```text
impact({ repo: "forge", target: "createOutputEventDispatcher", file_path: "apps/desktop/src/store/event-dispatch.ts", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "createHydrateAction", file_path: "apps/desktop/src/store/hydration.ts", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "clearStaleSessionHealthAlerts", file_path: "apps/desktop/src/store/health-alerts.ts", direction: "upstream", maxDepth: 2, summaryOnly: true })
```

- [x] **Step 2: Add frontend replay matrix tests**

Add tests with these names:

```ts
it("keeps compacted local estimate after provider usage replay and legacy companion", () => {
  // Dispatch provider_usage, context_compacted, then matching legacy usage.
  // Assert contextUsage.source remains "local_estimate" and usageLedger remains provider-backed.
});

it("restored interrupted pending confirmation replaces the existing confirm block after hydration", () => {
  // Hydrate a session with a confirm_ask block.
  // Dispatch the existing replayed_interrupted confirm_ask metadata path.
  // Assert the existing block is marked restored/interrupted and no duplicate confirm block exists.
});
```

If no event currently represents external approval after hydration, record that gap in this plan before adding production code.

- [x] **Step 3: Add hydration consistency tests**

In `apps/desktop/src/store/persistence-hydration.test.ts`, add:

```ts
it("hydrates usage ledger and composer context label from provider usage blocks after reload", async () => {
  // Persist a session with provider_usage blocks and no legacy usage.
  // Hydrate.
  // Assert usageLedger.inputTokens, contextUsage.usedTokens, costUsd, and context label inputs match.
});
```

- [x] **Step 4: Add health-alert active-session tests**

In `apps/desktop/src/store/health-alerts.test.ts`, add:

```ts
it("does not show stale alerts for an inactive session when another session is active", () => {
  // Build two stale session alerts and one global alert.
  // Assert visibleHealthAlertsForSession(alerts, activeSessionId) returns only the active stale alert plus global.
});
```

- [x] **Step 5: Run focused verification**

Run:

```bash
node --test apps/desktop/src/store/event-dispatch.test.ts apps/desktop/src/store/persistence-hydration.test.ts apps/desktop/src/store/health-alerts.test.ts
git diff --check
```

Expected: all tests pass.

### Task 3 Evidence / Impact Notes

- Impact: `createOutputEventDispatcher` was provided by the Task 3 handoff as `CRITICAL`, 34 impacted symbols, 18 affected processes, and broad Ui/Layout/Messages/Session/Context/History blast radius including `AppShell`, `Sidebar`, `CommandPalette`, `HistoryView`, and `ProjectCockpit`.
- Impact: `createHydrateAction` was provided by the Task 3 handoff as `CRITICAL`, 34 impacted symbols, 18 affected processes, and the same broad module/process blast radius.
- Impact: `clearStaleSessionHealthAlerts` was provided by the Task 3 handoff as `LOW`, 0 impacted symbols.
- Scope: because dispatcher/hydration risk is CRITICAL, Task 3 modified only store test files and this plan. Production store logic was not changed.
- Replay evidence: `event-dispatch.test.ts` now proves `provider_usage -> context_compacted -> replayed provider_usage -> matching legacy usage` keeps `contextUsage.source` as `local_estimate`, keeps compacted `usedTokens`, keeps the ledger provider-backed, marks `legacyDuplicateIgnored`, and avoids duplicate `provider_usage` blocks.
- Confirmation evidence/gap: the current stream protocol has no independent `confirm_response` event, so Task 3 does not claim external approval replay coverage. The confirmation test uses the existing replayed `confirm_ask` metadata path and proves an existing hydrated `confirm_ask` block is replaced with restored/interrupted metadata instead of duplicated.
- Hydration evidence: `persistence-hydration.test.ts` now proves persisted provider usage blocks restore usage ledger fields, accumulated cost, context usage, and the input consumed by the Composer context label after reload without a UI render.
- Health alert evidence: `health-alerts.test.ts` now proves inactive-session stale alerts are hidden while the active-session stale alert and a non-stale global alert remain visible.
- Verification: `node --test apps/desktop/src/store/event-dispatch.test.ts apps/desktop/src/store/persistence-hydration.test.ts apps/desktop/src/store/health-alerts.test.ts` passed, 31 tests.
- Verification: `git diff --check` passed.

## Task 4: Restart Smoke Protocol

**Purpose:** Add a realistic desktop restart smoke protocol, even if the first version is semi-automated, so restart behavior stops being asserted only through browser-level mocks.

**Files:**
- Create: `apps/desktop/docs/product/desktop-restart-smoke-protocol.md`
- Modify: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

- [x] **Step 1: Document the restart smoke protocol**

Create `apps/desktop/docs/product/desktop-restart-smoke-protocol.md` with:

```markdown
# Desktop Restart Smoke Protocol

## Scope

This is a manual or semi-automated macOS smoke until Forge has a dedicated Tauri WebDriver harness.

## Required Evidence

1. Current project path before quit.
2. Permission mode before quit.
3. Whether a pending confirmation exists before quit.
4. Session id before quit.
5. Screenshot or log after restart showing restored session.
6. Whether Composer permission mode, Project Status mode, pending confirmation card, health alerts, and context usage agree after restart.

## Steps

1. Start Forge with `npm --prefix apps/desktop run tauri -- dev`.
2. Select `/Users/cabbos/project/forge-test-app` or a disposable test project.
3. Start a new conversation and enable `信任项目` or `完全访问`.
4. Send a prompt that causes a current-project write confirmation.
5. Quit the app with an active session or pending confirmation.
6. Restart Forge.
7. Record restored session status, permission mode, pending confirmation card, project status, and context usage.

## Pass Criteria

- Restored UI never claims a broader permission mode than the live session gate can honor.
- Pending confirmation is replayed as interrupted, resolved, or pending with a clear explanation.
- Stale health alerts from the old run are cleared or scoped to the restored active session.
- Context usage is either restored from provider usage or explicitly unknown.
```

- [x] **Step 2: Add dry-run acceptance labels**

Add a dry-run label to `scripts/acceptance.sh`:

```bash
"manual desktop restart smoke protocol"
```

Update `scripts/acceptance.test.mjs` so it asserts that label appears.

- [x] **Step 3: Record beta log placeholder section**

Append this section to `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`:

```markdown
## Stability Convergence Restart Smoke - 2026-06-27

Status: Not yet run.

Protocol: `apps/desktop/docs/product/desktop-restart-smoke-protocol.md`

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
```

- [x] **Step 4: Verify docs and dry-run**

Run:

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
git diff --check
```

Expected: commands pass and the dry-run output advertises the restart protocol.

### Task 4 Evidence

- Protocol: `apps/desktop/docs/product/desktop-restart-smoke-protocol.md` now defines scope, required evidence, steps, and pass criteria for the real desktop restart smoke.
- Beta log: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md` now has `Stability Convergence Restart Smoke - 2026-06-27` with `Not yet run` status and required evidence fields.
- Acceptance dry-run: `scripts/acceptance.sh --dry-run` advertises the manual restart protocol gate after the mocked restart smoke, and the lightweight command checks both the protocol doc and beta log section.
- Review follow-up: the beta log evidence fields now mirror the protocol's pre-quit session id and post-restart screenshot/log requirements.
- Verification: `node --test scripts/acceptance.test.mjs`, `scripts/acceptance.sh --dry-run`, and `git diff --check` passed for the Task 4 change set.
- Impact note: no runtime product logic was changed. GitNexus `impact` for `parseDryRunEntries` in `scripts/acceptance.test.mjs` reported LOW risk, 0 direct callers, and 0 affected processes.

## Task 5: Real Task Regression Batch Template

**Purpose:** Make future beta runs repeatable and comparable instead of anecdotal.

**Files:**
- Create: `apps/desktop/docs/product/stability-regression-batch.md`
- Modify: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

- [x] **Step 1: Create the regression batch template**

Create `apps/desktop/docs/product/stability-regression-batch.md` with:

```markdown
# Stability Regression Batch

Run these tasks against a disposable current project. Controller-side manual writes invalidate the task.

| # | Task | Expected Permission State | Required Evidence | Result |
| --- | --- | --- | --- | --- |
| 1 | `/fix @src/App.tsx` for a small visible button feedback issue | Trust or Full Access should avoid repeated routine write prompts | final answer, diff, build/check result | |
| 2 | CSS layout polish in current project | Routine write allowed only inside current workspace | changed files, no external write | |
| 3 | Build/check command | Safe shell allowed under Full Access | command output summary | |
| 4 | Preview ownership question | final answer states URL and workspace path | final answer + Project Status details | |
| 5 | `/code-review` | findings-first, calibrated severity | review output | |
| 6 | New conversation same workspace | runtime trust/full access inherited | Composer mode + getPermissionMode args | |
| 7 | External path write attempt | blocked or confirmed | confirm/deny evidence | |
| 8 | Secret-like path write attempt | blocked or confirmed | confirm/deny evidence | |
| 9 | Restart with active task | honest restore or recovery notice | restart smoke evidence | |
| 10 | Context usage after provider event | `余` means true remaining context | Composer label + provider usage row | |
```

- [x] **Step 2: Reference the batch from the beta log**

Append to `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`:

```markdown
## Stability Regression Batch - 2026-06-27

Template: `apps/desktop/docs/product/stability-regression-batch.md`

Status: Not yet run.
```

- [x] **Step 3: Advertise the batch in dry-run acceptance**

Add a dry-run label to `scripts/acceptance.sh`:

```bash
"manual stability regression batch"
```

Update `scripts/acceptance.test.mjs` accordingly.

- [x] **Step 4: Verify docs and dry-run**

Run:

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
bash -n scripts/acceptance.sh
git diff --check
```

Expected: commands pass and dry-run advertises the regression batch.

### Task 5 Evidence

- Template: `apps/desktop/docs/product/stability-regression-batch.md` now defines a ten-task disposable-project regression batch covering routine edits, build/check, preview ownership, review calibration, workspace inheritance, external/secret writes, restart recovery, and provider context usage.
- Beta log: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md` now references the batch template under `Stability Regression Batch - 2026-06-27` with `Not yet run` status.
- Acceptance dry-run: `scripts/acceptance.sh --dry-run` advertises `manual stability regression batch` immediately after the manual desktop restart smoke protocol, and the lightweight command checks both the template file and beta log section.
- Impact note: no runtime product logic was changed. GitNexus `impact` for `parseDryRunEntries` in `scripts/acceptance.test.mjs` reported LOW risk, 1 direct caller, and 0 affected processes; the shell script file anchor was unresolved (`UNKNOWN`).
- Verification: `node --test scripts/acceptance.test.mjs`, `scripts/acceptance.sh --dry-run`, `bash -n scripts/acceptance.sh`, and `git diff --check` passed for the Task 5 change set.

## Task 6: Final Convergence Verification

**Purpose:** Prove the whole convergence pass is coherent before committing or handing off.

**Files:**
- Modify: `docs/superpowers/plans/2026-06-27-desktop-stability-convergence.md`

- [x] **Step 1: Run full focused test set**

Run:

```bash
node --test apps/desktop/src/store/usage-ledger.test.mjs apps/desktop/src/store/event-dispatch.test.ts apps/desktop/src/store/persistence-hydration.test.ts apps/desktop/src/components/session/contextUsageView.test.mjs apps/desktop/src/store/health-alerts.test.ts apps/desktop/src/lib/ipc/permissions.test.ts scripts/acceptance.test.mjs
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::permission_handlers --lib
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "trust|full access|health alert|stale alert|preview ownership|permission"
npm run build:desktop
npm --prefix apps/desktop run check:backend
scripts/acceptance.sh --dry-run
git diff --check
```

- [x] **Step 2: Run GitNexus staged diff audit**

Run:

```text
detect_changes({ repo: "forge", scope: "staged" })
```

Expected: record risk level and affected processes in this plan. If risk is HIGH or CRITICAL, list the exact affected processes and tests that cover them before committing.

- [x] **Step 3: Update the plan with final evidence**

Add a `## Final Evidence` section to this file with:

```markdown
## Final Evidence

- Node/store tests:
- Cargo permission tests:
- Playwright acceptance:
- Desktop build:
- Backend check:
- Acceptance dry-run:
- GitNexus detect_changes:
- Residual risk:
```

- [x] **Step 4: Commit**

Run:

```bash
git status --short
git add docs/superpowers/plans/2026-06-27-desktop-stability-convergence.md docs/desktop/state-consistency-map.md apps/desktop/AGENTS.md apps/desktop/src-tauri/src/harness/permissions_test.rs apps/desktop/src-tauri/src/ipc/permission_handlers.rs apps/desktop/e2e/acceptance.spec.ts apps/desktop/e2e/fixtures/app.ts apps/desktop/src/store/event-dispatch.test.ts apps/desktop/src/store/persistence-hydration.test.ts apps/desktop/src/store/health-alerts.test.ts apps/desktop/docs/product/desktop-restart-smoke-protocol.md apps/desktop/docs/product/stability-regression-batch.md apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md scripts/acceptance.sh scripts/acceptance.test.mjs README.md apps/desktop/README.md CHANGELOG.md
git commit -m "test: add desktop stability convergence gates"
```

Expected: commit succeeds. Leave generated screenshots, logs, and temporary files untracked or deleted.

## Final Evidence

- Node/store tests: `node --test apps/desktop/src/store/usage-ledger.test.mjs apps/desktop/src/store/event-dispatch.test.ts apps/desktop/src/store/persistence-hydration.test.ts apps/desktop/src/components/session/contextUsageView.test.mjs apps/desktop/src/store/health-alerts.test.ts apps/desktop/src/lib/ipc/permissions.test.ts scripts/acceptance.test.mjs` passed, 53 tests.
- Cargo permission tests: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib` passed, 44 tests; `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::permission_handlers --lib` passed, 6 tests.
- Playwright acceptance: `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "trust|full access|health alert|stale alert|preview ownership|permission"` passed, 11 specs.
- Desktop build: `npm run build:desktop` passed.
- Backend check: `npm --prefix apps/desktop run check:backend` passed after running `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml` to satisfy `cargo fmt --check`.
- Acceptance dry-run: `scripts/acceptance.sh --dry-run`, `bash -n scripts/acceptance.sh`, and `git diff --check` passed.
- GitNexus detect_changes: staged audit reported `low` risk, 25 changed symbols across 15 changed files, and 0 affected processes.
- Residual risk: real desktop restart smoke and the ten-task stability regression batch are documented and advertised but still marked `Not yet run` in the beta log; the stream still has no independent external `confirm_response` replay event, so replay coverage remains limited to existing interrupted `confirm_ask` metadata.

## Execution Notes

- Use fresh subagents per task.
- Do not run multiple implementation subagents against overlapping files.
- Review sequence per task: implementation, spec compliance review, code quality review.
- If a task discovers a source-of-truth mismatch that requires production changes outside its file list, stop that task, record the mismatch in this plan, and spawn a follow-up task with a disjoint write set.
