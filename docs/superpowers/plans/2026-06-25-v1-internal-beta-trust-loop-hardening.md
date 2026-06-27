# V1 Internal Beta Trust Loop Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for single-threaded implementation, or superpowers:subagent-driven-development when splitting independent test, Rust, and React slices. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the 2026-06-25 internal beta findings into a small set of trust-loop fixes that make Forge reliable for real self-use: the user can tell which workspace is active, which preview belongs to that workspace, why a confirmation is needed, whether a warning is current, and what was actually verified.

**Architecture:** Land this as ordered hardening slices. Start with a visible "Trust Current Project" permission mode because it removes the biggest self-use friction without weakening Forge source or machine safety. Then close the existing P1 preview-ownership blocker and address the remaining P2 friction with focused tests and product-level acceptance coverage. Keep each slice bounded to the desktop app surfaces that already own the behavior: Rust agent/session/runtime evidence, Rust permission and watchdog policy, React message/status presentation, Playwright acceptance specs, and product run logs.

**Tech Stack:** Tauri Rust backend, React/TypeScript desktop frontend, Zustand store, Playwright desktop e2e, Node test runner for TS helpers, Cargo tests for Rust policy/session code, GitNexus impact analysis.

---

## Source Evidence

- Internal beta run log: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`
- P1 blocker plan: `docs/superpowers/plans/2026-06-25-v1-internal-beta-first-blocker.md`
- Existing acceptance harness: `apps/desktop/e2e/acceptance.spec.ts`
- Desktop AGENTS guidance: `apps/desktop/AGENTS.md`

## Product Direction

Forge should now optimize for "real internal use by us" before adding breadth. The next milestone is not more agent features; it is making the first complete work loop trustworthy:

1. User selects or creates a project.
2. Forge changes only that project.
3. Forge can run and identify the correct preview.
4. Forge lets the user trust the current project for routine edits and checks, while still blocking or confirming dangerous work.
5. Forge resumes honestly from partial work.
6. Forge reports review findings with calibrated severity.
7. The operator can verify the whole path without manually changing the demo project.

## Acceptance Contract

The convergence sprint is complete only when all of these points are true:

- **A1 Preview Ownership:** When asked whether a preview belongs to the current demo project, Forge's visible final answer states the conclusion, the preview URL, and the owning workspace path. If the port belongs to another workspace, the answer states the conflict instead of presenting the URL as usable.
- **A2 Workspace Boundary:** During beta scenarios, Forge source workspace edits and demo workspace edits are never mixed. Any manual controller write to the demo project invalidates that scenario and requires a rerun after cleanup.
- **A3 Current Health Alerts:** A `会话无响应` banner is not visible for a session after fresh stream events from the same session have arrived, unless the session has again exceeded the stale threshold.
- **A4 Trust Current Project Mode:** The user can enable a visible "Trust Current Project" mode that automatically allows routine actions inside the active workspace and carries across new conversations in that workspace for the current app run, while Forge source edits, workspace-external paths, secrets, destructive commands, dependency installation, remote scripts, and publication commands still require confirmation or are blocked.
- **A5 Read-Only Preview Probe:** Local read-only preview checks such as `lsof` and safe localhost `curl` probes do not appear as high-risk destructive operations.
- **A6 Recovery Discipline:** For small obvious UI continuation work, Forge resumes from visible evidence and finishes the task without asking a placement question unless the requested layout is genuinely ambiguous.
- **A7 Review Calibration:** `/code-review` leads with findings and real risk, but P0/P1 labels are reserved for issues that block the user's stated task, corrupt data, create security exposure, or make verification impossible.
- **A8 Acceptance Visibility:** `scripts/acceptance.sh --dry-run` advertises the trust-loop coverage it expects, and the run log records the exact manual beta evidence that maps to A1 through A7.

## Scope

In scope:

- Preview ownership final-answer evidence and expanded project status details.
- Stale session health alert clearing or freshness logic.
- "Trust Current Project" permission mode for routine demo writes, verification commands, and read-only preview probes.
- `ask_user` prompt discipline and UI affordance for answerable questions.
- `/code-review` severity instruction calibration.
- Product run protocol and acceptance documentation.

Out of scope:

- New provider integrations.
- Headless owner execution beyond already planned runtime work.
- Shared packages across apps.
- Website and eval-runner behavior except the root acceptance script labels.
- Rebuilding the demo application by hand from the controller side.

## File Structure

- Modify: `docs/superpowers/plans/2026-06-25-v1-internal-beta-first-blocker.md`
  - Responsibility: remain the implementation source for the P1 preview ownership blocker.
- Modify: `apps/desktop/src-tauri/src/agent/turn_outcome.rs`
  - Responsibility: final-answer guidance for preview ownership evidence.
- Modify: `apps/desktop/src-tauri/src/agent/session/loop.rs`
  - Responsibility: pass latest turn evidence into finalization.
- Modify: `apps/desktop/src/components/layout/ProjectStatusDetails.tsx`
  - Responsibility: visible preview workspace ownership details.
- Modify: `apps/desktop/src-tauri/src/diagnostics/watchdog.rs`
  - Responsibility: stale-session detection and alert emission.
- Modify: `apps/desktop/src/store/health-alerts.ts`
  - Responsibility: replace, clear, or suppress stale alerts when fresh same-session evidence arrives.
- Modify: `apps/desktop/src/store/event-dispatch.ts`
  - Responsibility: clear stale health alerts on fresh events from the same session.
- Modify: `apps/desktop/src-tauri/src/harness/permissions.rs`
  - Responsibility: trust-mode policy for current-workspace writes and shell commands.
- Modify: `apps/desktop/src-tauri/src/harness/shell_policy.rs`
  - Responsibility: classify read-only preview probes accurately.
- Modify: `apps/desktop/src-tauri/src/ipc/permission_handlers.rs`
  - Responsibility: expose trust-mode read and mutation IPC commands.
- Modify: `apps/desktop/src/lib/ipc/permissions.ts`
  - Responsibility: TypeScript wrapper for trust-mode IPC.
- Modify: `apps/desktop/src/lib/ipc/types.ts`
  - Responsibility: TypeScript shape for permission mode state.
- Modify: `apps/desktop/src/components/settings/PermissionsPanel.tsx`
  - Responsibility: settings-surface control for trusting or untrusting the current project.
- Modify: `apps/desktop/src/components/layout/ProjectStatusView.tsx`
  - Responsibility: visible project-level permission mode indicator or action.
- Modify: `apps/desktop/src/components/messages/ConfirmCard.tsx`
  - Responsibility: render confirmation state and response handling.
- Modify: `apps/desktop/src/components/messages/ConfirmViews.tsx`
  - Responsibility: confirmation and ask-user presentation.
- Modify: `apps/desktop/src/components/messages/confirmPresentation.ts`
  - Responsibility: confirmation labels, helper text, and ask-user wording.
- Modify: `apps/desktop/src-tauri/src/agent/capability_context.rs`
  - Responsibility: slash-command intent for `/code-review`.
- Modify: `apps/desktop/src-tauri/src/workflow/router.rs`
  - Responsibility: `/code-review` workflow classification if copy or routing evidence needs alignment.
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
  - Responsibility: product-level smoke coverage for trust-loop behavior.
- Modify: `scripts/acceptance.sh`
  - Responsibility: advertise and run the trust-loop acceptance slice.
- Modify: `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`
  - Responsibility: document user-visible runtime and confirmation behavior changes.
- Modify: `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md`
  - Responsibility: record rerun evidence and final triage.

## Task 1: Add Trust Current Project Permission Mode

**Purpose:** Give Forge a Codex-like smooth path for internal self-use without creating an unbounded "skip all safety" mode. The first version is explicit, visible, reversible, and scoped to the current workspace.

- [x] **Step 1: Define the first-version product contract**

Implement one new mode:

```text
manual_confirm: existing behavior
trust_current_project: automatically allow routine actions inside the active workspace
```

The first version is runtime-scoped and workspace-scoped. It is active when the current workspace canonical path matches a trusted workspace path, including new conversations created in the same project during the current app run. It does not persist across app restart unless a later task explicitly adds persistence.

Acceptance points:

- The UI label is `信任当前项目`.
- The UI also offers `恢复手动确认`.
- The active mode is visible while enabled.
- New conversations in the same workspace inherit the active mode during the current app run.
- The mode names do not expose internal permission gate details.

- [x] **Step 2: Run impact analysis**

Before editing symbols, run:

```text
impact({ repo: "forge", target: "check", file_path: "apps/desktop/src-tauri/src/harness/permissions.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "is_allowed", file_path: "apps/desktop/src-tauri/src/harness/permissions.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "classify_shell_command", file_path: "apps/desktop/src-tauri/src/harness/shell_policy.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "PermissionsPanel", file_path: "apps/desktop/src/components/settings/PermissionsPanel.tsx", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "ProjectStatusView", file_path: "apps/desktop/src/components/layout/ProjectStatusView.tsx", direction: "upstream", maxDepth: 2, summaryOnly: true })
```

If any risk is HIGH or CRITICAL, report the blast radius before editing.

- [x] **Step 3: Add trust-mode state and IPC**

Modify:

```text
apps/desktop/src-tauri/src/harness/permissions.rs
apps/desktop/src-tauri/src/ipc/permission_handlers.rs
apps/desktop/src/lib/ipc/permissions.ts
apps/desktop/src/lib/ipc/types.ts
```

Expected shape:

```text
PermissionModeState {
  mode: "manual_confirm" | "trust_current_project"
  workspace_path: string | null
  session_scoped: boolean
}
```

Acceptance points:

- `list_permission_rules` behavior remains unchanged.
- A new read IPC returns the current permission mode state.
- A new mutation IPC can enable trust mode for the active workspace path.
- A new mutation IPC can restore manual confirmation.
- Trust mode carries across new conversations for the same trusted workspace during the current app run, and restores manual confirmation for that workspace when the user turns it off.

- [x] **Step 4: Allow routine current-project actions**

When `trust_current_project` is active for the matching workspace, allow:

```text
write_to_file and edit_file inside the trusted workspace
npm run build
npm test
cargo test
cargo check
git status
git diff
lsof -i :PORT
ps -p PID -o command=
pwd -P
curl or curl -I to http://127.0.0.1:PORT or http://localhost:PORT
```

Acceptance points:

- Editing `src/App.tsx` inside `/Users/cabbos/project/forge-test-app` does not show repeated confirmation after the mode is enabled.
- The same edit in `/Users/cabbos/project/forge` still confirms or blocks according to existing Forge source safety.
- Workspace-external paths still confirm or block.
- `.env`, `.env.*`, `.git/config`, key files, and credential-like paths still confirm or block.
- Remote `curl https://...` remains confirmation-gated.
- `curl ... | sh`, `git push`, `git reset`, destructive filesystem commands, dependency installation, and publish commands remain blocked or confirmed by existing safety policy.

- [x] **Step 5: Add visible controls**

Modify:

```text
apps/desktop/src/components/settings/PermissionsPanel.tsx
apps/desktop/src/components/layout/ProjectStatusView.tsx
apps/desktop/e2e/acceptance.spec.ts
```

Acceptance points:

- Settings > Tools shows a `信任当前项目` control when a workspace is active.
- The project status card shows an active trust-mode indicator when enabled.
- The user can restore manual confirmation from the same surface.
- If no workspace is active, the control is disabled with a short explanation.

- [x] **Step 6: Verify the trust-mode slice**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::permission_handlers --lib
node --test apps/desktop/src/lib/ipc/permissions.test.ts
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "trust current project"
```

Acceptance points:

- The focused tests prove allowed current-project actions are smooth.
- The focused tests prove blocked or confirmation-gated actions stay protected.
- Manual beta Scenario 1 or 2 can complete without repeated identical safe-write prompts after trust mode is enabled.
- If a current-workspace write confirmation is already pending, enabling `信任当前项目` from the project status card approves that pending confirmation once and updates the card to `已继续`.

**2026-06-25 implementation evidence:** Task 1 landed a runtime workspace-scoped `trust_current_project` mode in the Rust permission gate, permission-mode IPC, TypeScript IPC wrappers, Settings > Tools controls, Project Status controls, and acceptance fixture coverage. A live Scenario 2 rerun exposed the first implementation's session-only gap: after enabling `信任当前项目`, creating a new conversation produced confirmation cards because the trust state was keyed only by `sessionId`. The follow-up fix records trusted canonical workspace paths, lets new sessions in the same workspace inherit routine in-project write approval, and sends the workspace path through permission-mode read/restore calls so Settings and the project status card do not appear to lose state. A second live rerun then exposed that clicking `信任当前项目` after a confirmation was already pending changed the project mode but did not continue the current gate. The project status trust action now approves the latest pending write confirmation for the same workspace, and `ConfirmCard` syncs externally resolved confirmation metadata so the visible card changes to `已继续`. GitNexus symbol and file impact could not resolve after index refresh failed with the known missing `tree-sitter-swift` package; the latest `trustCurrentProject`, `ConfirmCard`, and fixture `setup` impact calls returned `Target not found` / `UNKNOWN`, with no HIGH or CRITICAL blast radius reported. RED failed first on missing `PermissionMode`, trust-mode methods, IPC functions, and TS exports, then on the live new-session inheritance gap, missing Project Status entry point, and missing pending-confirm approval. GREEN verification passed:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::permission_handlers --lib
node --test apps/desktop/src/lib/ipc/permissions.test.ts
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "trust the current project"
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "project status card can trust"
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "project status trust approves"
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "project status card can trust|project status trust approves|does not approve non-write"
npm --prefix apps/desktop run build
git diff --check
```

**2026-06-26 follow-up evidence:** A live trusted Scenario 2 rerun still showed a `准备修改项目` confirmation for `edit_file src/App.tsx` while the Project Status card displayed `已信任`. Root cause: chat turns execute through per-session `Harness::new_with_pending(...)` instances, while the trust control originally mutated only `AppState.harness.permission_gate`. The UI could therefore report inherited app-level trust while the live session gate still returned `Ask`. The fix now synchronizes app-level permission mode into the live session harness when permission mode is read or mutated, when a session is created, and immediately before `send_input` reserves a turn. The same follow-up also found two adjacent Scenario 2 blockers: safe check commands such as `npm run build 2>&1 | tail -20` were misclassified as confirmation-worthy because of output clipping, and `/fix` still asked whether to continue after already locating a small obvious UI fix. Both were tightened in the same Task 1 hardening pass. GitNexus impact calls for `set_permission_mode_for_state`, `PermissionGate`, `build_agent_session`, `AppState`, `create_session`, and `send_input` returned `Target not found` / `UNKNOWN`; no HIGH or CRITICAL blast radius was reported. RED/GREEN verification passed:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::permission_handlers::tests::inherited_project_trust_syncs_to_live_session_harness --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::permission_handlers --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions_test --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml capability_context --lib
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
```

Manual UI rerun status: the fix was rebuilt into the Tauri dev app, but the desktop entered the macOS lock screen before the final fresh Scenario 2 UI pass could be completed. Do not count Scenario 2 as re-passed until the prompt is rerun in an unlocked Forge window with `信任当前项目` enabled and no write confirmation card appears.

## Task 2: Freeze The Operator Boundary

**Purpose:** Prevent the tester or controller from accidentally becoming part of the product behavior. This directly addresses the confusion caused by manually acting on a prompt meant for Forge.

- [x] **Step 1: Add an operator-boundary section to the run log**

Modify `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md` under `## Rules`:

```markdown
- Controller-side writes to `/Users/cabbos/project/forge-test-app` invalidate the current scenario unless the scenario explicitly asks for independent verification.
- All intended project changes must be performed by Forge and visible through Forge messages, tool events, or resulting git/worktree evidence.
- Controller-side verification may read files, run status commands, open previews, inspect processes, and click the preview UI.
```

Acceptance points:

- The run log distinguishes Forge actions from controller verification.
- A future reader can tell that a scenario pass means Forge performed the change.
- The controller may still verify preview ownership and git status independently.

- [x] **Step 2: Add scenario preflight and postflight commands**

For every rerun scenario, record:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
git -C /Users/cabbos/project/forge status --short --branch
```

Acceptance points:

- Demo repo and Forge source repo status are recorded before and after the scenario.
- Unexpected files in the Forge source repo are called out as unrelated or blocking before the scenario starts.
- If a controller-side write occurs, the scenario result is marked invalid rather than pass or fail.

**2026-06-25 implementation evidence:** Task 2 updated `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md` with explicit controller/operator boundary rules and a rerun protocol that records both demo and Forge source workspace states before and after follow-up scenarios. Current verification:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
git -C /Users/cabbos/project/forge status --short --branch
rg -n "Controller-side writes|Rerun Protocol|Invalid" apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md
```

## Task 3: Close P1 Preview Ownership First

**Purpose:** The current P1 blocks a clean internal beta pass because Forge had enough evidence but did not answer the ownership question.

- [ ] **Step 1: Execute the existing blocker plan**

Follow `docs/superpowers/plans/2026-06-25-v1-internal-beta-first-blocker.md` task-by-task.

Required GitNexus preflight before editing symbols:

```text
impact({ repo: "forge", target: "final_answer_instruction", file_path: "apps/desktop/src-tauri/src/agent/turn_outcome.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "finalize_turn", file_path: "apps/desktop/src-tauri/src/agent/session/loop.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "ProjectStatusDetails", file_path: "apps/desktop/src/components/layout/ProjectStatusDetails.tsx", direction: "upstream", maxDepth: 2, summaryOnly: true })
```

Acceptance points:

- Backend tests prove final-answer instructions include preview ownership evidence when present.
- UI tests prove expanded project status shows preview URL and workspace path.
- Product e2e proves the visible answer does not leave ownership implicit.
- Manual rerun of Scenario 3 passes A1.

- [x] **Step 2: Keep this slice narrow**

Do not change provider routing, process spawning, checkpoint policy, or permission policy in this task.

Acceptance points:

- `git diff --stat` for this task stays limited to the files listed in the P1 blocker plan plus docs.
- New acceptance coverage fails before the preview ownership fix and passes after it.

**2026-06-25 implementation evidence:** The P1 preview ownership implementation is in place, with automatic verification passing. `final_answer_instruction` now receives latest turn evidence and appends preview ownership evidence when present; `finalize_turn` passes the latest turn snapshot without holding the turn lock across the model call; expanded project status details now show `预览归属`. The focused acceptance test failed first on the missing `预览归属` field and passed after the UI change. Verification passed:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::turn_outcome --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::loop_test --lib
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "project delivery details surface preview ownership"
npm run build:desktop
scripts/acceptance.sh --dry-run
```

Remaining before marking Task 3 fully closed: run staged `detect_changes`, commit the intended files, and manually rerun Scenario 3 in Forge to prove the visible final answer states preview ownership.

## Task 4: Make Health Alerts Current

**Purpose:** The beta repeatedly showed stale `会话无响应` while the session was actively producing useful output. This weakens trust because the user cannot tell whether the warning is current.

- [x] **Step 1: Run impact analysis**

Before editing symbols, run:

```text
impact({ repo: "forge", target: "record_session_event", file_path: "apps/desktop/src-tauri/src/diagnostics/watchdog.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "spawn_session_watchdog", file_path: "apps/desktop/src-tauri/src/diagnostics/watchdog.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "upsertHealthAlert", file_path: "apps/desktop/src/store/health-alerts.ts", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "createOutputEventDispatcher", file_path: "apps/desktop/src/store/event-dispatch.ts", direction: "upstream", maxDepth: 2, summaryOnly: true })
```

If any risk is HIGH or CRITICAL, report the blast radius before editing.

- [x] **Step 2: Add failing store tests for stale alert clearing**

Extend `apps/desktop/src/store/health-alerts.test.ts` and the dispatcher tests used by `apps/desktop/src/store/event-dispatch.ts`.

Expected behavior:

```text
Given healthAlerts contains alert_id "session-stale:s1" for session "s1"
When a fresh non-health stream event arrives for session "s1"
Then the stale alert for "s1" is removed
And alerts for other sessions remain visible
```

Acceptance points:

- Fresh same-session events clear only that session's stale alert.
- Missing API key and other non-stale alerts are not cleared by unrelated stream events.
- Alert dedupe by `alert_id` still works.

- [x] **Step 3: Add or adjust Rust watchdog tests**

Extend `apps/desktop/src-tauri/src/diagnostics/watchdog.rs`.

Expected behavior:

```text
record_session_event(SessionStarted { session_id: "s1", ... }) marks "s1" fresh
record_session_event(TextChunk { session_id: "s1", ... }) refreshes the same session
no stale alert is emitted while the tracked last-event timestamp is below DEFAULT_STALE_THRESHOLD_SECS
```

Acceptance points:

- The watchdog does not emit `会话无响应` for a fresh active session.
- The watchdog still emits stale warnings after the threshold for a truly inactive running session.
- Cooldown behavior remains intact.

- [x] **Step 4: Add product e2e coverage**

Extend `apps/desktop/e2e/acceptance.spec.ts` with a scenario that dispatches a stale health alert, then dispatches fresh output for the same session.

Acceptance points:

- The stale banner appears after the stale alert event.
- The stale banner disappears after fresh same-session output.
- A different session's alert remains visible until dismissed or refreshed.

Verification commands:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml diagnostics::watchdog --lib
node --test apps/desktop/src/store/health-alerts.test.ts
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "health alert"
```

**2026-06-25 implementation evidence:** Stale `session-stale*` health alerts are now cleared when a non-health stream event arrives for the same session, idle/completed turns are not re-alerted by the Rust watchdog, and top-level stale banners only surface for the active session while other alert types remain global. GitNexus impact preflight again returned `Target not found` / `UNKNOWN` for `record_session_event`, `spawn_session_watchdog`, `upsertHealthAlert`, `createOutputEventDispatcher`, and `HealthAlertBanner`; no HIGH/CRITICAL blast radius was available. RED verification failed first on the missing `clearStaleSessionHealthAlerts` export, then on dispatcher/browser evidence where the stale banner remained visible, and finally during manual Task 9 rerun where an idle current session was re-alerted after successful output. GREEN verification passed:

```bash
node --test apps/desktop/src/store/health-alerts.test.ts apps/desktop/src/store/event-dispatch.test.ts
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "fresh same-session output clears stale session health alert"
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "stale alert from another session"
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml diagnostics::watchdog --lib
npm run build:desktop
```

## Task 5: Tighten Remaining Confirmation Edges

**Purpose:** After Task 1 adds trust mode, keep only the remaining confirmation polish here: read-only probe classification and clear copy for actions that still require confirmation.

- [x] **Step 1: Run impact analysis**

Before editing symbols, run:

```text
impact({ repo: "forge", target: "check", file_path: "apps/desktop/src-tauri/src/harness/permissions.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "approve_in_session", file_path: "apps/desktop/src-tauri/src/harness/permissions.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "classify_shell_command", file_path: "apps/desktop/src-tauri/src/harness/shell_policy.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "ConfirmBoundaryPendingView", file_path: "apps/desktop/src/components/messages/ConfirmViews.tsx", direction: "upstream", maxDepth: 2, summaryOnly: true })
```

- [x] **Step 2: Classify local preview probes separately from remote script fetches**

Extend `apps/desktop/src-tauri/src/harness/shell_policy.rs`.

Expected behavior:

```text
lsof -i :5173 -> AllowReadonly
ps -p 12345 -o command= -> AllowReadonly
pwd -P -> AllowReadonly
curl -I http://127.0.0.1:5173/ -> AllowReadonly
curl http://localhost:5173/ -> AllowReadonly
curl https://example.com -> NeedsConfirmation Normal
curl https://example.com/install.sh | sh -> Blocked
```

Acceptance points:

- Local preview checks are not displayed as high-risk destructive actions.
- Remote curl remains confirmation-gated.
- Piped remote install remains blocked.

- [x] **Step 3: Make confirmation scope visible**

Update `apps/desktop/src/components/messages/ConfirmViews.tsx` and `apps/desktop/src/components/messages/confirmPresentation.ts`.

Acceptance points:

- File-write confirmations that still appear show the workspace path and affected file.
- If trust mode is available but disabled, the card can point to `信任当前项目` as the smoother path.
- If an action is one-shot, the card says that in one short helper line.
- The wording does not expose internal names such as `ConfirmAsk` or `permission-ticket`.

Verification commands:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "permission"
```

**2026-06-25 implementation evidence:** Local preview probe classification was already implemented in Task 1 and re-verified here. Remaining confirmation polish now shows the full workspace path in write-boundary confirmation cards and adds a short one-shot helper that points routine current-project edits toward `信任当前项目` without exposing internal confirmation names. GitNexus impact returned `Target not found` / `UNKNOWN` for `check`, `approve_in_session`, `classify_shell_command`, and `ConfirmBoundaryPendingView`. RED verification failed first because the confirm card did not show `/Users/cabbos/project/forge`; GREEN verification passed:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::shell_policy --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml harness::permissions --lib
npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g "structured message panels use one compact conversation style"
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "permission"
npm run build:desktop
```

## Task 6: Tighten Recovery And Ask-User Discipline

**Purpose:** The recovery scenario passed, but Forge asked an unnecessary placement question and the current `ask_user` card only offers Continue or Cancel, which is awkward when the model asks a real question.

- [x] **Step 1: Run impact analysis**

Before editing symbols, run:

```text
impact({ repo: "forge", target: "deriveConfirmPromptView", file_path: "apps/desktop/src/components/messages/confirmPresentation.ts", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "ConfirmPromptView", file_path: "apps/desktop/src/components/messages/ConfirmViews.tsx", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "ConfirmCard", file_path: "apps/desktop/src/components/messages/ConfirmCard.tsx", direction: "upstream", maxDepth: 2, summaryOnly: true })
```

- [x] **Step 2: Split approval questions from answerable questions**

Keep boolean confirmation for permission operations. For `ask_user`, add a presentation state that makes the limitation explicit until a text-answer IPC path exists:

```text
If the backend only accepts boolean response:
  show "继续" and "取消"
  helper text: "这一步只能确认是否继续；如需补充具体偏好，请直接发一条新消息。"
```

Acceptance points:

- The UI no longer pretends a real free-form question can be answered through Continue/Cancel.
- Existing interrupted-confirm recovery still displays correctly.
- No backend API change is required for this narrow slice.

- [x] **Step 3: Add model instruction for obvious small continuations**

Extend the recovery or capability-context instruction that accompanies `/fix` and continuation flows:

```text
For small obvious UI placement decisions, use the most conventional placement and continue. Ask the user only when multiple choices would materially change the result.
```

Acceptance points:

- A rerun of the `今日完成` continuation completes without asking where to put a simple status area.
- If the user asks for a design choice with meaningful alternatives, Forge may still ask one concise question.
- Forge still reports partial work honestly if interrupted.

Verification commands:

```bash
node --test apps/desktop/src/components/messages/confirmPresentation.test.ts
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "ask user"
```

**2026-06-25 implementation evidence:** `ask_user` confirmation cards now state that the current UI only supports continue/cancel and that concrete preferences should be sent as a new message. `/fix` capability intent now tells the model to make conventional small UI placement decisions and continue unless alternatives materially change the result. GitNexus impact returned `Target not found` / `UNKNOWN` for `deriveConfirmPromptView`, `ConfirmPromptView`, and `ConfirmCard`. RED verification failed first on the missing boolean-only helper and missing `/fix` small-decision wording. GREEN verification passed:

```bash
npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g "ask_user confirmation explains boolean-only response limits"
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::capability_context --lib
npm run build:desktop
```

## Task 7: Calibrate Developer Review Severity

**Purpose:** `/code-review` behaved like a review, but it over-labeled product gaps as P0. The review flow should be useful without making every improvement sound existential.

- [x] **Step 1: Run impact analysis**

Before editing symbols, run:

```text
impact({ repo: "forge", target: "slash_command_intent", file_path: "apps/desktop/src-tauri/src/agent/capability_context.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
impact({ repo: "forge", target: "classify_workflow_with_command", file_path: "apps/desktop/src-tauri/src/workflow/router.rs", direction: "upstream", maxDepth: 2, summaryOnly: true })
```

- [x] **Step 2: Update `/code-review` intent copy**

Update the slash-command intent for `/code-review` in `apps/desktop/src-tauri/src/agent/capability_context.rs` so the model receives this severity contract:

```text
Lead with findings. Prioritize real bugs, regressions, security or data-loss risk, and missing verification. Use P0 only for critical blockers. Use P1 for issues that block the user's stated goal or make the result unsafe to ship. Use P2 for product gaps, polish, hardening, or follow-up work. Do not offer to fix unless the user asks.
```

Acceptance points:

- `/code-review` still routes to verification.
- Findings remain first.
- Product gaps in the demo are P2 unless they block the user's stated flow.
- The answer does not end by pushing a fix action unless the user requested it.

- [x] **Step 3: Add focused tests**

Extend `apps/desktop/src-tauri/src/agent/capability_context_tests.rs` and relevant Playwright coverage.

Acceptance points:

- The capability context for `/code-review` contains the P0/P1/P2 contract.
- The visible composer command description remains concise.
- The flow still hides internal skill names in normal user messages.

Verification commands:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml capability_context --lib
npm --prefix apps/desktop run test:e2e -- e2e/composer.spec.ts -g "code-review"
```

**2026-06-25 implementation evidence:** `/code-review` hidden capability intent now carries the findings-first P0/P1/P2 severity contract while preserving the existing verification route and concise composer command surface. GitNexus impact returned `Target not found` / `UNKNOWN` for `slash_command_intent` and `classify_workflow_with_command`. RED verification failed first because the capability context lacked `Lead with findings` and P0/P1/P2 guidance. GREEN verification passed:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::capability_context --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml workflow::router --lib
npm --prefix apps/desktop run test:e2e -- e2e/composer.spec.ts -g "composer command surface stays compact"
```

## Task 8: Align Acceptance Script And Docs

**Purpose:** Once the trust-loop behavior changes are user-visible, the docs and dry-run acceptance list must say what is being protected.

- [x] **Step 1: Extend desktop acceptance coverage**

Add grouped tests to `apps/desktop/e2e/acceptance.spec.ts`:

```text
preview ownership answer and status details
fresh session output clears stale health alert
trust current project permission mode
read-only preview probes are not high-risk
code-review severity contract remains in the hidden capability context
```

Acceptance points:

- Each new test has a visible user-level assertion.
- Shared e2e IPC fixtures remain contract-shaped and do not mirror implementation internals.
- The tests can be run independently with a `-g` filter.

- [x] **Step 2: Update `scripts/acceptance.sh`**

Add or rename one command label for the desktop trust-loop smoke:

```text
desktop trust-loop trust mode, preview ownership, health alert, confirmation, and review calibration smoke specs
```

Acceptance points:

- `scripts/acceptance.sh --dry-run` prints the new label.
- `scripts/acceptance.test.mjs` passes after the label update.
- The command still runs `apps/desktop/e2e/acceptance.spec.ts` rather than a private implementation test only.

- [x] **Step 3: Update runtime docs**

Update:

```text
README.md
apps/desktop/README.md
CHANGELOG.md
apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md
```

Acceptance points:

- Docs mention preview ownership evidence as a user-visible runtime behavior.
- Docs mention that stale health alerts clear when the same session resumes producing output.
- Changelog records trust mode, confirmation, and review calibration at user-visible level.
- The beta run log records rerun evidence for A1 through A7.

Verification commands:

```bash
scripts/acceptance.sh --dry-run
node --test scripts/acceptance.test.mjs
```

**2026-06-25 implementation evidence:** Trust-loop product smoke coverage is now visible in the desktop acceptance slice: `acceptance.spec.ts` covers preview ownership details, same-session stale health alert clearing, and the `信任当前项目` mode; focused message/composer/Rust tests cover confirmation helper text, `ask_user` limits, read-only preview probe policy, and the hidden `/code-review` severity contract without leaking implementation-only context into browser assertions. The root acceptance dry-run label now advertises the trust-loop smoke instead of the older Phase 7-only wording, and `scripts/acceptance.test.mjs` asserts that label while keeping the command pointed at `apps/desktop/e2e/acceptance.spec.ts`. Runtime docs and changelog were updated in Tasks 3 through 7 for preview ownership, current health alerts, trust mode, confirmation, and review calibration.

## Task 9: Manual Internal Beta Rerun

**Purpose:** Automated tests should protect contracts, but this direction is about feel and trust. Finish with the exact human path that exposed the problems.

- [x] **Step 1: Establish the demo workspace baseline**

Record:

```bash
git -C /Users/cabbos/project/forge-test-app status --short --branch
git -C /Users/cabbos/project/forge-test-app log --oneline -5
```

Acceptance points:

- The demo baseline is written into the run log.
- Existing Forge-created checkpoint commits are preserved unless the user explicitly asks to reset them.
- No controller-side file writes are made to the demo project during the rerun.

**2026-06-25 implementation evidence:** Follow-up rerun baseline was recorded in `apps/desktop/docs/product/v1-internal-beta-run-2026-06-25.md` using read-only commands only. `/Users/cabbos/project/forge-test-app` is clean on `main`, with recent commits `4b51e3e feat: add today-done section on water tracker home page` and `087540d chore: initialize Forge test app`. The Forge source workspace status was also summarized as dirty with current convergence-sprint edits plus pre-existing unrelated files, so those source changes are not demo scenario evidence.

- [ ] **Step 2: Rerun the six beta scenarios**

Use the same prompts from the run log:

```text
Beginner creation
Existing project fix
Preview ownership
Checkpoint and recovery
Honest recall
Developer review flow
```

Acceptance points:

- Scenario 3 changes from Fail P1 to Pass.
- Stale warning no longer remains visible during active successful output.
- After enabling `信任当前项目`, repeated same-target demo writes no longer show repeated safe-write prompts.
- `/code-review` remains findings-first and no longer marks non-blocking product gaps as P0.

**2026-06-25 partial rerun evidence:** Scenario 1 was rerun against the preserved demo workspace without controller-side demo writes. The demo workspace stayed clean, Forge started the demo preview on `127.0.0.1:5173`, and read-only process inspection confirmed the Vite owner was `/Users/cabbos/project/forge-test-app`. This partial rerun exposed the remaining health-alert bug where a successful idle current session was re-alerted by the watchdog after five minutes; the watchdog active/idle fix and banner scoping fix were landed before continuing. The remaining beta scenarios still need a clean rerun after enabling `信任当前项目`.

- [ ] **Step 3: Make the release decision**

Update the run log final decision:

```text
P0 remaining: none
P1 remaining: none
P2 remaining: listed follow-ups
Decision: ready for the next internal self-use loop
```

Acceptance points:

- Remaining P2 items are explicitly listed and do not block the next loop.
- Any P1 regression gets a new blocker-specific plan before more broad testing.
- The final log includes commands, user-visible evidence, and screenshots or preview notes where useful.

## Full Verification Gate

Run the focused checks for changed surfaces:

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
scripts/acceptance.sh --dry-run
```

Before any commit, run:

```text
detect_changes({ repo: "forge", scope: "staged" })
```

Acceptance points:

- All focused checks pass.
- `detect_changes` reports only expected docs, desktop runtime, trust-mode permission, confirmation, health-alert, review, and acceptance surfaces.
- If `detect_changes` reports unexpected execution flows, stop and inspect before committing.

**2026-06-26 automated gate evidence:** After the trusted-session gate sync fix, the focused full verification gate was rerun from the current worktree. The desktop was still unavailable for manual beta rerun because screenshots returned a black/locked screen, but the automated trust-loop gate passed:

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

Result highlights: `acceptance.spec.ts` passed all 30 tests, including preview ownership details, `信任当前项目` across conversations, pending-confirm takeover, same-session stale-alert clearing, and Settings trust controls. Remaining manual gate: rerun the six beta scenarios in an unlocked Forge window before updating the final release decision.

**2026-06-26 monorepo command evidence:** The desktop was still unavailable for manual beta rerun because screenshots remained fully black, so the remaining progress this pass focused on root-level non-UI verification. The commands advertised by the root `AGENTS.md` passed from the current worktree:

```bash
npm run build:desktop
npm run build:website
npm run test:eval
scripts/acceptance.sh --dry-run
```

Result highlights: website production build passed, eval runner precheck passed, eval pytest suite passed with `139 passed, 1 warning`, desktop production build passed, and the acceptance dry-run still advertises the trust-loop smoke command. This does not replace Task 9's manual rerun requirement.

**2026-06-26 completion audit / external blocker:** The remaining incomplete work is now limited to Task 9 Step 2 and Step 3: rerun the six beta scenarios in the real Forge UI and update the final release decision from that user-visible evidence. The authoritative checks for the implemented code paths are green, but they are not sufficient to prove the plan complete because the acceptance contract explicitly requires manual beta evidence for preview ownership, current health alerts, trust-mode write smoothness, and review calibration. Three consecutive UI availability checks returned fully black screenshots (`mean [0.0, 0.0, 0.0]`), so Forge cannot currently be operated for the manual rerun from this thread. Resume when the desktop is unlocked, then start with Scenario 2 (`/fix @src/App.tsx`) under `信任当前项目` to prove no write confirmation card appears, continue through Scenario 3 for preview ownership, then finish the remaining beta prompts before changing the release decision.

## Stop Conditions

Stop and report before continuing if any of these occur:

- GitNexus reports HIGH or CRITICAL risk for a planned symbol edit.
- A fix requires editing outside `apps/desktop`, root docs, root acceptance script, or the beta run log.
- A P2 slice starts changing provider routing, process management, or headless owner behavior.
- Manual beta rerun requires controller-side writes to the demo project.
- New tests require weakening the existing Forge source workspace safety boundary.
