# Forge Runtime Gap Closure Roadmap — Hermes Parity

> Date: 2026-06-12
> Owner: Forge Runtime / Agent Team
> Controller: Codex (planning, verification, go/no-go)
> Worker: Claude Code (implementation, tests, status reporting)
> Branch context: `cabbos/internal-a2a-runtime-plan`

## Executive Summary

Hermes has a mature, observable, always-available runtime: sessions survive app restarts, a background gateway service keeps the runtime alive, tools/providers are centrally configured and diagnosed, subagents stream status and cost, memory/profiles work across CLI and desktop, and the product surfaces settings, history, recovery, and review flows.

Forge has closed the inner loop (A2A worktree worker negotiation, review gates, multi-agent arbitration, and smoke validation) in Phase 0. The remaining gaps are **runtime scaffolding and product completeness**, not model intelligence. This roadmap sequences seven implementation phases to bring Forge to Hermes parity without losing the three-app monorepo discipline (`apps/desktop`, `apps/eval-runner`, `apps/website` remain independently runnable). Each phase ends with an acceptance gate that Codex verifies before Claude Code proceeds.

## Scope

In scope:

1. Session persistence and true resume after app restart.
2. Background runtime services: gateway, dashboard, launchd/autostart/watchdog, update repair.
3. Unified tool/provider/skills ecosystem and visible configuration/diagnostics UI.
4. Subagent/worktree worker process experience: status stream, file IO, tokens/cost/tool count, parent-child relation, failure/interruption reasons.
5. Memory, profiles, multiple entry points: CLI/desktop/shared runtime, messaging triggers, cron/scheduled tasks.
6. Diagnostics, self-healing, observability: doctor/status/logs, gateway/session watchdog, tool inventory.
7. Desktop product completeness: settings, history, recovery/error states, permission states, previews, review flows, background task surfaces.

Out of scope (recorded for follow-up roadmaps):

- New AI model backends beyond Claude / OpenAI / Hermes adapter reuse.
- Cloud sync or multi-device state replication.
- Paid billing or usage-metering backend.
- Mobile app.
- Third-party plugin marketplace transactions.

## Non-Goals

- Do not extract shared packages until code is actually shared by at least two apps (per root `AGENTS.md`).
- Do not rewrite the agent loop or the `StreamEvent` protocol from scratch; extend them.
- Do not remove the Tauri 2.0 desktop app; keep it the primary GUI.
- Do not commit or push from this planning task.

## Current Baseline — Phase 0 Complete

Recent commits on `cabbos/internal-a2a-runtime-plan`:

- `3472923` feat(agent): improve a2a runtime review surfaces
- `4121260` fix(agent): harden worktree smoke validation
- `272591c` feat(agent): Phase 6 worktree worker review gate & multi-agent arbitration
- `d932de2` fix(agent): tighten worktree worker acceptance gate
- `a8ede43` feat(agent): worktree worker phase 5 acceptance and review gate hardening

Phase 0 delivered:

- A2A worktree worker lifecycle (spawn, smoke test, acceptance gate, review gate, arbitration).
- Multi-agent negotiation when multiple workers propose changes.
- Runtime review surfaces for human-in-the-loop approval.
- Smoke validation hardening before workers are accepted.

Existing runtime scaffolding already in place (discovered during Codex verification):

- Per-session snapshot module: `apps/desktop/src-tauri/src/agent/snapshot.rs` with `AgentSessionSnapshot` save/load/list/delete helpers.
- `AgentSession::snapshot` and `AgentSession::restore_state` in `apps/desktop/src-tauri/src/agent/session.rs`.
- `restore_session_from_snapshot` / `emit_restored_session_startup` in `apps/desktop/src-tauri/src/ipc/session_lifecycle.rs`.
- Resume normalization in `GoalLedger` and `AgentA2ABus` so restored sessions can continue a round.

What Phase 0 intentionally did **not** build — the remaining Phase 1 gaps:

- App-startup "true continuation": restoring the full in-flight agent loop, active adapter request state, and streaming context when Forge launches.
- Full UI rehydrate: rebuilding the Zustand/IndexedDB mirror from a snapshot so message blocks, tool-call state, and typing status match the backend.
- Pending-confirm replay descriptors: persisted representation of an interrupted confirmation that the frontend can re-render and replay.
- Active tool-call interruption/restoration semantics: what happens to a tool call that was in flight when the app quit, and how its result is re-associated after restart.
- Corruption UX: graceful fallback, diagnostics logging, and user-visible recovery when a snapshot is unreadable or incompatible.
- End-to-end restart acceptance: an automated test that force-quits and reopens the app and asserts session restoration.
- A detached gateway / background service.
- Centralized diagnostics or doctor commands.
- Unified tool/provider configuration UI.
- Subagent cost/telemetry stream.
- Cross-entry-point memory/profiles.

## Architecture Principles

1. **Reconstructable state over live state.** Any runtime object (session, subagent, pending confirm, scheduled task) must be recreatable from a persisted snapshot plus a replayable event log. Never rely on in-memory-only references for anything that crosses an app restart.
2. **Single source of truth per domain.** Sessions → Rust `AppState`; UI mirror → Zustand store + IndexedDB; config → `~/.forge/config.json`; runtime telemetry → gateway log directory.
3. **Event-sourced backend → frontend.** The `StreamEvent` enum remains the only backend→frontend transport. New surfaces (status, cost, subagent tree) add new `StreamEvent` variants rather than new side channels.
4. **Idempotent lifecycle operations.** Start, stop, resume, repair, and autostart must all be safe to run twice.
5. **Fail visible.** Every background service and scheduled task reports health, last error, and next run time to the diagnostics surface.
6. **App isolation.** `apps/desktop`, `apps/eval-runner`, and `apps/website` keep their own `package.json`, build scripts, and dependency closure. Shared helpers live in a sibling `packages/` directory only after two apps consume them.

## Phased Schedule and Acceptance Gates

### Phase 1 — Session Runtime Persistence & Restore
**Goal:** Extend the existing per-session snapshot/load/list/delete infrastructure into true app-startup continuation: full UI rehydrate, pending-confirm replay, active tool-call restoration semantics, corruption UX, and e2e restart acceptance.

**Duration:** 2 weeks
**Depends on:** Phase 0

**Work breakdown:**

- [ ] 1.1 Extend/version the existing `AgentSessionSnapshot` schema rather than defining it from scratch.
  - Add a snapshot version discriminator; include `Session` enum, `BlockState` equivalent, pending-confirm replay descriptors, active tool-call state, and adapter request state.
  - *Phase 1A completed (2026-06-12): added `schema_version`, `PendingConfirmDescriptor`, `ActiveToolCallDescriptor`, builder methods, and focused roundtrip/migration tests in `agent/snapshot.rs`.*
  - *Phase 1B completed (2026-06-12): wired live `PendingConfirmDescriptor` and `ActiveToolCallDescriptor` registries into `Harness`/`ToolExecutor`, attached them to snapshots via `session_snapshot_with_workflow_state`, and added focused tests for snapshot inclusion and cleanup. No startup hydration or frontend replay yet.*
  - Files: `agent/snapshot.rs`, `agent/session.rs`, `protocol/events.rs`, `ipc/session_lifecycle.rs`.
- [x] 1.2 Wire snapshot save triggers on every significant `StreamEvent` (debounced) and on app `RunEvent::Exit`. **Phase 1.2 completed (2026-06-12):** Created `autosave.rs` module with `is_significant_stream_event()` (excludes `ThinkingChunk`, `TextChunk`, `ShellOutput` only), `schedule_autosave()` with per-session coalescing via static `Mutex<HashMap<String, bool>>` (800ms debounce), and `flush_all_sessions()` called on `RunEvent::Exit`. Wired `schedule_autosave` into `emit_stream_event` in `transcript.rs` (minimal additive call, no signature change). Changed `lib.rs` from `.run()` to `.build()?.run(closure)` for exit event handling. Added 7 focused tests covering event classification, per-session coalescing, multi-session independence, variant-coverage guardrail, and clear-for-test helper. All existing explicit save points preserved (create_session, restore/resume/kill, send_input finalize). Safe when AppState or session missing (logs warning, no panic).
  - Files: `autosave.rs` (new), `lib.rs`, `transcript.rs`.
- [ ] 1.3 Wire startup hydration: at startup list snapshots, call `restore_session_from_snapshot` / `emit_restored_session_startup`, and rehydrate the frontend Zustand/IndexedDB mirror. **Phase 1C (2026-06-12):** minimal app-startup session restore — `choose_startup_snapshot` selection strategy (active wins, fallback to most recent, skip if live), `startup_restore_active_session` orchestrator wired from `lib.rs` setup, 7 focused tests. No frontend replay or e2e yet.
  - Files: `lib.rs`, `state.rs`, `store/index.ts`, `ipc/session_lifecycle.rs`.
- [x] 1.4 Add `SessionStatus::Resuming` variant and stream it to the frontend so the UI shows a skeleton during rehydration. **Phase 1D (2026-06-12):** `SessionStatus::Resuming` variant + `as_str()` → `"resuming"`; `restore_session_from_snapshot` marks session `Resuming` before register; `emit_restored_session_startup` streams `resuming` → replay → `running`; `session_events::session_status_event` helper added; TS types: `SessionState.status` includes `"resuming"`, `coerceSessionStatus` preserves it, event dispatch maps `"resuming"` to frontend `"resuming"` with `streaming: false`, hydration respects backend `"resuming"` for Tauri (forces `"stopped"` for IndexedDB-only). Rust tests: `SessionStatus::Resuming.as_str()`, `session_status_event`, `list_session_infos_for_state` reports live resuming. No pending-confirm replay or active tool-call restoration.
  - Files: `agent/session.rs`, `agent/session_events.rs`, `agent/session_tests.rs`, `ipc/session_lifecycle.rs`, `ipc/session_lifecycle_tests.rs`, `lib/protocol.ts`, `store/event-dispatch.ts`, `store/session-utils.ts`, `store/hydration.ts`.
- [ ] 1.5 Implement pending-confirm replay descriptors: persist interrupted confirmations so the frontend can re-render and replay them on resume.
  - *Phase 1.5 completed (2026-06-12): extended `StreamEvent::ConfirmAsk` with optional `replayed_interrupted` boolean (serde default false, omitted when false). Added `pending_confirm_replay_event` helper in `session_events.rs`. Wired `RestoredSession.pending_confirms` to carry descriptors through the restore path. `emit_restored_session_startup` emits replay events after `session_status resuming` and before workflow/turn/delivery replay. Frontend `eventToBlock` renders replayed confirms as interrupted (`confirmed: true, answer: null, confirm_interrupted: true, confirm_interrupted_reason: "session_restored"`) — same visual path as `closeInterruptedConfirmBlocks`. Frontend dedupe in both live event dispatch and transcript replay replaces existing `confirm_ask` blocks by `block_id` instead of appending. Added Rust tests for replay helper event shape and serde serialization; updated `StreamEvent` protocol tag test constructor; added TS node tests for replayed metadata and duplicate replacement.*
  - *Active tool-call restoration and fake confirmation senders remain out of scope for Phase 1.6.*
  - Files: `protocol/events.rs`, `lib/protocol.ts`, `agent/session_events.rs`, `ipc/session_lifecycle.rs`, `harness/mod.rs`, `harness/event_bus.rs`, `executor/mod.rs`, `store/blocks.ts`, `store/event-dispatch.ts`, `store/blocks.test.ts`.
- [x] 1.6 Implement active tool-call interruption/restoration semantics: define in-flight tool-call state, re-association rules after restart, and timeout/cancellation behavior.
	  - *Phase 1.6 completed (2026-06-12): added `active_tool_call_replay_events` helper in `session_events.rs` that emits ToolCallStart + ToolCallResult(is_error=true) for each saved ActiveToolCallDescriptor. Wired `RestoredSession.active_tool_calls` to carry descriptors through the restore path without registering fake tool handles in the harness registry. `emit_restored_session_startup` emits active tool-call replay events after pending confirm replay and before workflow/turn/delivery replay, while session_status is still resuming. Frontend `applyTranscriptEventToBlocks` and `createOutputEventDispatcher` deduplicate tool_call_start by block_id (update/replace instead of append). ToolCallResult preserves tool_name/tool_input metadata from prior start events. Rust tests: replay event shape (session_id, block_id, tool_name, tool_input, is_error, positive/saturating duration_ms), restore carries descriptors without populating harness registry. TS node tests: ToolCallStart→interrupted ToolCallResult single block, duplicate ToolCallStart no append, orphan ToolCallResult fallback.*
	  - Files: `agent/session_events.rs`, `ipc/session_lifecycle.rs`, `ipc/session_lifecycle_tests.rs`, `store/blocks.ts`, `store/event-dispatch.ts`, `store/blocks.test.ts`.
- [x] 1.7 Add snapshot corruption fallback and UX: if a snapshot fails to load, start fresh, log to diagnostics, and surface a recovery notice in the UI.
  - *Phase 1.7 completed (2026-06-12): added `StreamEvent::RecoveryNotice` variant (notice_id, title, message, reason, recoverable) to Rust/TS protocol with `session_id()`/`event_type()` arms. Added `recovery_notice_event` helper in `session_events.rs`. Updated `startup_restore_active_session` to detect corrupted active snapshots, emit recovery notice, fall back to another valid snapshot, and emit fallback-used notice. On restore failure emits restore-failed notice and starts fresh — never crashes. Frontend: `RuntimeRecoveryNotice` type in store, `recovery_notice` handled before session lookup in `createOutputEventDispatcher`, `RecoveryNoticeBanner` component with dismissible amber alert UI rendered from `AppShell`. Added 3 Rust tests for corruption fallback and 2 session_events tests for recovery notice shape.*
      - Files: `protocol/events.rs`, `agent/session_events.rs`, `ipc/session_lifecycle.rs`, `ipc/session_lifecycle_tests.rs`, `lib/protocol.ts`, `store/types.ts`, `store/event-dispatch.ts`, `store/index.ts`, `components/layout/RecoveryNoticeBanner.tsx`, `components/layout/AppShell.tsx`.
- [x] 1.8 Add `SessionSnapshot` unit tests covering Agent, CLI, pending confirm, corruption, and version-migration cases. **Phase 1.8 completed (2026-06-12):** Added 8 new snapshot tests (agent shape roundtrip with multi-role ChatMessage, provider alias raw storage, multi-descriptor pending confirm list with mixed boundaries, corrupted JSON rejection, unsafe session ID pre-rejection, future schema version acceptance, legacy default-field fallback) plus 2 session_lifecycle provider-alias normalization tests. All 59 snapshot + 20 session_lifecycle tests pass.
  - Files: `src-tauri/src/agent/snapshot.rs`, `src-tauri/src/ipc/session_lifecycle_tests.rs`.
- [x] 1.10 Add session store management primitives: stats, search, export, and prune for persisted snapshots.
  - **Phase 1.10 follow-up (2026-06-15):** Added backend session-store APIs for snapshot stats (counts, bytes, provider/workspace facets, corrupt count), text search across snapshot metadata/messages, JSON export, and prune-by-recency with A2A sidecar cleanup. `forge session` now exposes `stats`, `search`, `export`, and `prune` in addition to gateway `list`; desktop IPC/TS wrappers expose the same management surface for future History/Settings UI, closing more of the Hermes-style session store management gap.
  - **Phase 1.10 offline detail follow-up (2026-06-15):** Added a single-snapshot detail read to the session store and made `forge session show <session_id>` fall back to local snapshot JSON when the gateway socket is absent. This keeps persisted session inspection available even when the background daemon is not running.
  - Files: `src-tauri/src/agent/snapshot.rs`, `src-tauri/src/session_store.rs`, `src-tauri/src/ipc/session_store_handlers.rs`, `src-tauri/src/bin/forge_session.rs`, `apps/desktop/cli/src/commands/session.ts`, `apps/desktop/src/lib/ipc/sessionStore.ts`.
- [ ] 1.9 Add e2e Tauri smoke: open app, send a message, force-quit, reopen, assert session restored. **Partial (2026-06-12):** Added 4 browser-level Playwright e2e tests (block persistence across reload, recovery_notice without active session, dismissibility, multi-notice rendering). All 4 pass. **Investigation (2026-06-12):** True Tauri force-quit/reopen smoke is not feasible with the current test infrastructure. The project has no `tauri-driver` (npm), no WebDriver client (selenium-webdriver or equivalent), no headless Tauri binary launch mechanism, and no binary-build step integrated into the test pipeline. Adding `tauri-driver` + WebDriver + headless launch would constitute a new test harness dependency chain (`@tauri-apps/tauri-driver`, `selenium-webdriver`, `chromedriver`/`geckodriver`, macOS headless display setup) that exceeds the "minimal addition" threshold. The existing browser-level smoke in `apps/desktop/e2e/resume.spec.ts` covers IndexedDB persistence across reload and recovery notice rendering — this is the best available coverage until a Tauri WebDriver harness is deliberately invested in. Requires: `@tauri-apps/tauri-driver` npm package, a WebDriver client, Tauri binary build integration, and platform-specific headless display setup.
  - Files: `apps/desktop/e2e/resume.spec.ts` (new).

**Acceptance gate:**

- All existing tests pass.
- New snapshot and version-migration tests pass.
- Targeted tests around `send_input`, `execute_single_round`, `resume_session`, `snapshot`, and `pending_confirms` pass.
- Manual UX: quit with an active agent session and pending confirmation; reopen; session, blocks, and confirmation dialog restore.
- Simulated corrupt snapshot starts fresh and surfaces a recovery notice.

**Verification plan:**

- `cargo test` in `apps/desktop/src-tauri`, with extra focus on `agent::session` and `agent::snapshot`.
- `npm run test` in `apps/desktop`.
- Manual restart test on macOS and one other platform.

**Risk / impact notes:**

- GitNexus impact analysis shows `AgentSession` is **CRITICAL** risk: 43 impacted symbols, 11 affected processes. Any edit to `AgentSession` methods must be preceded by `gitnexus_impact` and reported to the controller. Future implementation should keep batches small and run targeted tests around `send_input`, `execute_single_round`, `resume_session`, `snapshot`, and `pending_confirms` before moving to the next batch.
- Confirm-response and resume-normalization helpers in `GoalLedger` and `AgentA2ABus` are LOW risk; prefer extending them over refactoring `AgentSession` core state.

---

### Phase 2 — Diagnostics, Doctor, and Watchdog
**Goal:** A `forge doctor` command and an in-app diagnostics UI expose runtime health, session/gateway watchdog, and tool inventory.

**Duration:** 2 weeks
**Depends on:** Phase 1

**Work breakdown:**

- [x] 2.1 Create a `diagnostics` module in Rust with health check registry.
  - Files: `src-tauri/src/diagnostics/mod.rs` (completed 2026-06-12; `doctor.rs` and `watchdog.rs` remain as separate follow-up items for 2.4/2.5).
- [x] 2.2 Implement checks: config/settings readable + API key presence summary, session snapshots readable/listable + corruption count, app metadata readable, log/data directory accessible, tool/capability inventory loadable, project runtime status probe.
  - Files: `diagnostics/mod.rs` (all checks inline; no separate `checks.rs` — that pattern doesn't suit a single-module package).
- [x] 2.3 Add `StreamEvent::DiagnosticsUpdate` and `StreamEvent::HealthAlert` variants; mirror in TypeScript.
  - Files: `protocol/events.rs`, `lib/protocol.ts`, `store/event-dispatch.ts` (minimal no-op handler; full UI panel deferred to 2.6).
- [x] 2.4 Add session watchdog: if a session has produced no event for N minutes, emit health alert and offer recovery.
  - Files: `diagnostics/watchdog.rs` (session event tracker, background watchdog task, stale detection with cooldown), `transcript.rs` (wire recording), `lib.rs` (spawn watchdog).
  - Completed 2026-06-12: watchdog monitors live (Running/Resuming) sessions every 60s, emits HealthAlert via emit_stream_event with stable alert_id when no event for 5min, per-session cooldown prevents spam. 9 unit tests cover tracker, stale detection, cooldown, and health alert shape.
- [x] 2.5 Add gateway watchdog: if the gateway process exits unexpectedly, restart with exponential backoff and surface status.
  - **Completed 2026-06-15:** Added a gateway service watchdog in `diagnostics/watchdog.rs` and started it from `lib.rs` alongside the session watchdog. It probes the launchd gateway service every 30s, only auto-restarts when macOS service management is supported and the plist is installed but the service is not running, emits global `HealthAlert` updates, and applies exponential restart backoff capped at 300s after repair failures. Unit tests cover stopped-service restart decisions, backoff growth/capping, and waiting during backoff.
  - Files: `diagnostics/watchdog.rs`, `lib.rs`.
- [x] 2.6 Build in-app Diagnostics panel (Settings > Diagnostics) showing checks, logs, and repair actions.
  - Files: `src/components/settings/DiagnosticsPanel.tsx`, `src/hooks/queries/useDiagnosticsReportQuery.ts`, `src/hooks/queries/queryKeys.ts` (diagnosticsReport key), `SettingsCenterShell.tsx` (added diagnostics section with Stethoscope icon).
  - Completed 2026-06-12: Diagnostics section in Settings left-nav; panel shows overall status (ok/warnings/failures), generated timestamp, pass/warn/fail counts, ordered check list with id/label/status/message/remediation, refresh button with lucide RefreshCw icon, loading/error/empty states.
- [x] 2.7 Extend existing CLI `doctor` subcommand with extra checks mirroring the Rust report (config, app data, sessions, logs) without requiring Tauri runtime. Preserves `--json` and human output.
  - Files: `apps/desktop/cli/src/commands/doctor.ts`, `apps/desktop/cli/test/doctor.test.ts`.
- [x] 2.8 Unit tests for diagnostics report aggregation and each check shape (17 Rust tests + 6 CLI tests pass) + new watchdog tests (9 tests) + new health-alerts tests (4 tests).
- [x] 2.9 Eval-runner integration: `npm run test:eval` includes a doctor health pre-check.
  - Files: `scripts/eval-doctor-precheck.mjs`, `package.json` (root test:eval script).
  - Completed 2026-06-12: pre-check runs CLI doctor in JSON mode before pytest. Fresh install (missing ~/.forge or log file) is classified as soft-fail and does not block eval. Hard failures (corrupted config, unreadable files) warn but do not block — keeping apps/eval-runner independently runnable.
- [x] 2.10 Add A2A ledger diagnostics and repair action.
  - Files: `diagnostics/mod.rs`, `diagnostics/repair.rs`.
  - Completed 2026-06-15: diagnostics now checks `~/.forge/a2a` sidecars, reports readable/corrupt ledger counts, total task counts, and running/failed/interrupted task totals. Corrupt sidecars warn when at least one valid ledger remains and fail when all ledger files are corrupt. Added `clear_a2a_ledger_cache` repair action so Settings > Diagnostics can clear persisted subagent state without manual filesystem work.

**Acceptance gate:**

- `forge doctor` returns a structured report.
- Simulated gateway death is recovered within 30 seconds. **(Implemented for installed launchd service — gateway watchdog probes every 30s and runs `restart_gateway` with exponential backoff on repeated failures)**
- Simulated hung session triggers a health alert. **(Implemented — watchdog emits HealthAlert after 5min of no events)**

**Verification plan:**

- `cargo test`.
- `npm run test:eval` passes.
- Manual UX: open Diagnostics, kill gateway process, observe recovery and alert.

---

### Phase 3 — Tool/Provider/Skills UI Consolidation
**Goal:** One place to see, enable, disable, configure, and diagnose tools, providers, MCP servers, hooks, skills, and extensions.

**Duration:** 2 weeks
**Depends on:** Phase 2

**Work breakdown:**

- [x] 3.1 Audit plugin manager types (`plugin_manager/`): `McpServer`, `Hook`, `Skill`, `Extension`.
  - **Phase 3-A decision:** The actual existing system is the Rust `Capability` trait + `CapabilityRegistry` (not `plugin_manager/`). Extended the existing model rather than creating a parallel path.
  - Files: `harness/capability.rs`, `harness/registry.rs`, `harness/skills.rs`, `harness/mcp.rs`, `harness/hooks.rs`.
- [x] 3.2 Define unified `EcosystemItem` entity and status enum.
  - **Phase 3-A (2026-06-12):** Added `EcosystemItem` struct and `EcosystemItemStatus` enum (Healthy/Unavailable/Warning/Unknown) to `harness/capability.rs`. `EcosystemItem` wraps existing `CapabilityMetadata` with status, status_message, configurable flag, and config_summary. Builder methods: `from_capability_entry()`, `with_status()`, `with_configurable()`.
  - Files: `harness/capability.rs`.
- [x] 3.3 Add IPC commands: `list_ecosystem_items`, `set_ecosystem_enabled`, `configure_ecosystem_item`, `get_tool_inventory`.
  - **Phase 3-A (2026-06-12):** Added all 4 commands to `ipc/capability_handlers.rs` and registered in `lib.rs`. `list_ecosystem_items` builds from registry + SkillLoader. `set_ecosystem_enabled` delegates to existing toggle. `get_tool_inventory` returns tool/MCP items. `configure_ecosystem_item` returns explicit "not yet supported" error (no config persistence path yet).
  - **Phase 3-D follow-up (2026-06-15):** `configure_ecosystem_item` now supports MCP server configuration write-back for existing `mcp:<id>` entries by updating the source `.forge/mcp.json` entry (name, description, command, args, enabled) while preserving unknown fields and other servers. Non-MCP item kinds still return an explicit unsupported error until provider/skill config schemas stabilize.
  - Added TS types (`EcosystemItem`, `EcosystemItemStatus`, `ToolInventoryEntry`) and wrapper functions in `lib/tauri.ts`.
  - Files: `ipc/capability_handlers.rs`, `lib.rs`, `lib/tauri.ts`.
- [ ] 3.4 Add `StreamEvent::EcosystemChanged` so UI refreshes when items install/uninstall.
  - **Phase 3-A decision (2026-06-12): DEFERRED.** Query invalidation after toggle (invalidateQueries for capabilities, ecosystemItems, toolInventory in handleToggle) already provides sufficient UI refresh. Adding a new StreamEvent variant requires Rust+TS protocol sync and adds no additional value over the existing invalidation pattern. Will revisit when background skill installation or MCP server discovery changes happen outside the toggle flow (Phase 3-B or later).
  - Files: No changes needed.
- [x] 3.5 Build Settings > Tools/Providers/Skills panel with tabs, search, enable toggles, and config drawers.
  - **Phase 3-A (2026-06-12):** Enhanced existing `CapabilityManager` / `CapabilityContentViews` / `CapabilityRows` components rather than creating separate `EcosystemSettings.tsx`. Added: status badges (healthy/unavailable/warning) on capability rows, `CapabilityDetailDrawer` component for item detail view with status/health/config info, config-aware "暂不支持界面配置" hint for non-configurable items. Created `useEcosystemItemsQuery` and `useToolInventoryQuery` hooks following existing patterns. Tab structure (skills/tools→Skills, mcp→MCP, hooks→Hooks) preserved.
  - Files: `src/components/settings/CapabilityManager.tsx`, `CapabilityContentViews.tsx`, `CapabilityRows.tsx`, `CapabilityDetailDrawer.tsx` (new), `src/hooks/queries/useEcosystemItemsQuery.ts` (new), `useToolInventoryQuery.ts` (new), `queryKeys.ts`.
- [x] 3.6 Surface tool call counts per session and per tool in the UI.
  - **Phase 3-A partial (2026-06-12):** Added tool inventory count to CapabilityManager summary strip (available tools count from `get_tool_inventory`). Per-session tool call counts are already available via `AgentTurnProjection.tool_call_count` and `failed_tool_count` (streamed via `agent_turn_updated` events), so no agent loop changes were needed.
  - **Phase 3-B (2026-06-12):** Added `deriveToolCounts(blocks)` pure helper with dedup, per-tool-name breakdown, tool/shell/failed classification, and top-tool detection. Extended `summarizeActivity` to include total/failed tool annotation and top-tool name in per-group summary items. Added `工具调用` metric to `ProjectCockpit` operation metrics reading from `AgentTurnProjection.tool_call_count`/`failed_tool_count`. Added 12 focused node tests for the pure helper covering empty input, dedup, failed classification, shell check/command split, top-tool ranking, and graceful missing-field handling.
  - Files: `src/components/messages/processActivity.ts`, `src/components/layout/ProjectCockpit.tsx`, `src/store/processActivity.test.ts`.
- [x] 3.7 Add diagnostics integration: each ecosystem item contributes a health check.
  - **Phase 3-A (2026-06-12):** Enhanced `diagnostics/mod.rs` `CapabilitySummary` with optional `status` and `status_message` fields. Enhanced `check_capability_inventory` to report unhealthy/unavailable counts when status data is available (warns when unhealthy items exist, lists their names). Updated `diagnostics_handlers.rs` to collect capabilities with status enrichment (MCP servers marked "unknown" with probe-not-implemented message; other items "healthy"/"unknown"). Added shared status helpers in `capability_handlers.rs` (`ecosystem_status_for_capability`, `ecosystem_status_label`) so Settings and Diagnostics use the same semantics.
  - **Phase 3-C follow-up (2026-06-15):** MCP ecosystem status now performs a read-only config probe: it reads the source `mcp.json`, verifies the server entry and command, marks configured servers healthy, marks unreadable/invalid/missing-command servers unavailable, and surfaces a short command/arg summary in Settings.
  - Files: `diagnostics/mod.rs`, `ipc/diagnostics_handlers.rs`.
- [ ] 3.8 Tests: plugin manager unit tests, UI component tests with mocked IPC.
  - **Phase 3-A partial (2026-06-12):** Added Rust tests for `EcosystemItem` model, IPC helpers, and diagnostics inventory aggregation. Existing frontend node tests (blocks, health-alerts, recovery-notices) and `npm run build` pass, but dedicated UI component tests with mocked IPC remain deferred because this app does not currently have a lightweight component-test harness for Settings panels.
  - **Phase 3-B partial (2026-06-12):** Added 12 pure-helper tests for `deriveToolCounts` in `store/processActivity.test.ts` using the existing `node --test` pattern. No UI component tests for ToolActivitySummary/ProjectCockpit — these are stateless render components that receive derived data as props; the pure-helper coverage validates the count derivation logic, and `npm run build` catches TS/JSX errors in the components. Full component-test harness (mocked IPC, React Testing Library) remains deferred.
  - Files: `harness/capability.rs`, `ipc/capability_handlers.rs`, `diagnostics/mod.rs`, `store/processActivity.test.ts`.

**Phase 3-A summary (2026-06-12):**

| Item | Status | Notes |
|------|--------|-------|
| EcosystemItem model | ✅ Done | stable fields, status enum, builder methods |
| IPC commands (4) | ✅ Done | list/set/inventory/configure (stub) |
| EcosystemChanged event | ⏸️ Deferred | Query invalidation sufficient for now |
| Settings UI enhancement | ✅ Done | Status badges, detail drawer, config awareness |
| Tool inventory & counts | ✅ Done | Tool inventory IPC, summary count in UI |
| Diagnostics integration | ✅ Done | Unhealthy counts, status enrichment; MCP config probe added in Phase 3-C |
| Tests | ✅ Done | 14 new Rust tests, all existing pass |
| Provider/extension inventory | ⏸️ Deferred | No provider source yet; represented as unavailable |
| In-app config persistence | 🟨 Partial | MCP server write-back is supported; provider/skill config schemas still deferred |

**Phase 3-B summary (2026-06-12):** Tool-count visibility in UI without touching Rust agent loop.

| Item | Status | Notes |
|------|--------|-------|
| 3.6 per-session tool counts | ✅ Done | `ProjectCockpit` metric via `AgentTurnProjection` |
| 3.6 per-group tool counts | ✅ Done | `summarizeActivity` + `deriveToolCounts` annotations |
| 3.6 per-tool-name breakdown | ✅ Done | `deriveToolCounts.perTool` + top-tool surface |
| 3.8 pure-helper tests | ✅ Done | 12 node tests for `deriveToolCounts` |
| 3.8 UI component tests | ⏸️ Deferred | No component harness; pure-helper coverage + build check |
| Rust agent loop changes | 🚫 None | Constraint honored — zero Rust changes |
| New dependencies | 🚫 None | No new packages |

**Acceptance gate:**

- User can disable a skill and see it no longer invoked.
- User can configure an MCP server and see health status.
- Tool counts are visible per message and per session.

**Verification plan:**

- `cargo test` plugin manager.
- `npm run test` UI tests.
- Manual UX: enable/disable tools, verify behavior, check diagnostics.

---

### Phase 4 — Subagent Workbench Maturation
**Goal:** Subagents/worktree workers are first-class citizens: visible status stream, file IO, token/cost/tool-count telemetry, parent-child relation, and clear failure/interruption reasons.

**Duration:** 2 weeks
**Depends on:** Phase 1 (session persistence), Phase 3 (tool counts)

**Work breakdown:**

- [ ] 4.1 Extend `AgentSession` and `Subagent` structs to track parent session id, child ids, and lineage.
  - **Phase 4-A partial (2026-06-12):** Enriched existing `AgentA2AProjection` / `AgentA2ATaskProjection` over the existing `agent_a2a_updated` channel instead of touching `AgentSession` or `ChildAgentRuntime.run_worktree_worker`. Added `parent_task_id`, timing fields (`created_at_ms`, `started_at_ms`, `ended_at_ms`, `duration_ms`), failure classification (`failure_kind`, `retryable`), `resume_note`, and `latest_progress` — all derived from already-available `AgentTaskRecord` and artifact fields. `AgentSession`/`SubAgent` parent-session and child-id tracking remains deferred; the current runtime does not yet populate `parent_task_id` for normal delegate tasks.
  - Files: `agent/a2a/projection.rs`, `agent/a2a/bus.rs`, `lib/protocol.ts`.
- [ ] 4.2 Add `StreamEvent` variants: `SubagentStart`, `SubagentStatus`, `SubagentFileIo`, `SubagentCost`, `SubagentEnd`, `SubagentFailed`, `SubagentInterrupted`.
  - **DEFERRED (Phase 4-B):** The enriched `agent_a2a_updated` projection already carries status transitions and failure details. New `StreamEvent` variants for subagent-specific streaming would require touching `ChildAgentRuntime.run_worktree_worker` (CRITICAL risk per GitNexus impact), which is explicitly out of scope for 4-A. The existing `agent_a2a_updated` event carries all the new fields and is the preferred path.
  - Files: `protocol/events.rs`, `lib/protocol.ts`.
- [x] 4.3 Implement status stream emission from worktree worker lifecycle.
  - **Phase 4-A (2026-06-12):** The existing `AgentA2AProjection` streamed via `agent_a2a_updated` already carries status flow. Phase 4-A enriched it with timing, failure_kind/retryable, resume_note, latest_progress, and parent lineage. Pure `deriveWorkbenchSummary` helper extracts review-needed, retained-worktree counts. No `run_worktree_worker` edits needed.
  - Files: `agent/a2a/bus.rs`, `components/messages/AgentA2ATimeline.tsx`.
- [ ] 4.4 Implement file IO stream: files read/written by a worker.
  - **Phase 4-B partial (2026-06-12):** Diff-derived changed-file summary is visible — the workbench now parses existing `DiffSummary` artifacts in `AgentA2ABus.projection()` to extract changed file paths (first 8 unique, deduped from git diff text). `changed_file_count`, `changed_files`, and `diff_available` fields are projected to the frontend. The `WorktreeReviewPanel` renders file path chips and counts. The `deriveWorkbenchSummary` helper computes `tasksWithDiff` and visible/projected `changedFiles` counts; full totals remain on each task via `changed_file_count`. **True live file IO stream remains DEFERRED** — it would need hooks in `executor/` and `ToolExecutor`, propagating through `ChildAgentRuntime` (CRITICAL risk per GitNexus impact). No new `StreamEvent` variants were added.
  - Files: `agent/a2a/projection.rs`, `agent/a2a/bus.rs`, `lib/protocol.ts`, `lib/workbenchSummary.ts`, `components/messages/AgentA2ATimeline.tsx`, `styles/process.css`.
- [ ] 4.5 Implement cost stream: tokens in/out, tool call count, model, estimated cost.
  - **DEFERRED (Phase 4-B):** Requires adapter-level hooks in the AI adapters trait. The existing `agent_turn_updated` event carries `AgentTurnProjection.tool_call_count` and `failed_tool_count` which Phase 3-B already surfaces. Token/cost streaming needs adapter changes that ripple across all providers.
  - Files: `adapters/base.rs`, `adapters/anthropic.rs`, `adapters/openai.rs`.
- [ ] 4.6 Build Subagent Workbench view: tree of parent/child sessions, status badges, cost tab, file IO tab.
  - **Phase 4-B partial (2026-06-12):** Enhanced existing `AgentA2ATimeline` / `AgentA2AWorkspace` components with parent-child lineage hint, duration/elapsed display, failure kind badge with retryable indicator, resume note for interrupted tasks, latest progress while running, workbench summary counts (review-needed, retained worktrees), **diff-derived changed-file chips in WorktreeReviewPanel** (file path chips, total count, per-task diff indicator), and **test report excerpt**. Stats area now shows tasks-with-diff count. Kept layout dense — no nested cards. No new tab components created (deferred for cost/file IO streams in 4-B).
  - Files: `components/messages/AgentA2ATimeline.tsx`, `styles/process.css`.
- [x] 4.7 Distinguish failure reasons: smoke failure, review rejection, arbitration timeout, tool error, user cancellation.
  - **Phase 4-A (2026-06-12):** Added `failure_kind` field to `AgentA2ATaskProjection` (populated from `AgentTaskFailure.kind` in bus.rs). Frontend renders a localized failure kind badge with distinct labels: 工具错误, 冒烟测试失败, 审阅拒绝, 仲裁超时, 用户取消. `retryable` flag shown as RefreshCw icon when true.
  - Files: `agent/a2a/projection.rs`, `agent/a2a/bus.rs`, `components/messages/AgentA2ATimeline.tsx`.
- [ ] 4.8 Persist subagent lineage to session snapshot for resume.
  - **Phase 4-A partial (2026-06-12):** The snapshot/resume system already saves and restores full `AgentA2ABus` state, including existing `AgentTaskRecord.parent_task_id` when present. `normalize_for_resume` already marks interrupted worktree workers with `resume_note` and worktree path. Phase 4-A surfaced those persisted fields in the projection, but true parent-session/child-id lineage population remains deferred.
  - Files: `agent/a2a/types.rs` (existing), `agent/a2a/bus.rs`.
- [ ] 4.9 Tests: unit tests for cost tracking, status transitions, and failure classification; e2e for worker lifecycle.
  - **Phase 4-B partial (2026-06-12):** Added Rust unit tests in `bus.rs` covering `extract_files_from_diff_text` (modified, added, deleted, rename, fallback headers, dedup), `extract_test_report_excerpt` (summary field, result field, fallback), projection diff fields (no diff artifact, changed files extraction, 8-file limit, diff_available from metadata, no metadata, test report excerpt). Added 8 node tests for `deriveWorkbenchSummary` covering tasksWithDiff, visible changedFiles deduplication, truncated projection semantics, zero-diff defaults, null/empty inputs, and sparse legacy payloads. Cost tracking and e2e worker lifecycle tests remain deferred.
  - Files: `agent/a2a/bus.rs`, `store/workbenchSummary.test.ts`.

**Phase 4-A summary (2026-06-12):**

| Item | Status | Notes |
|------|--------|-------|
| 4.1 Parent/child lineage projection | 🟨 Partial | `parent_task_id` field visible when present; runtime population deferred |
| 4.2 New StreamEvent variants | ⏸️ Deferred | Enriched existing `agent_a2a_updated` path instead |
| 4.3 Status stream | ✅ Done | Timing, progress, resume_note on existing projection |
| 4.4 File IO stream | ⏸️ Deferred | Requires executor hooks (CRITICAL risk path) |
| 4.5 Cost/token stream | ⏸️ Deferred | Requires adapter trait changes |
| 4.6 Workbench view | 🟨 Partial | Enhanced existing components; cost/file IO tabs deferred |
| 4.7 Failure reasons | ✅ Done | failure_kind + retryable badge + localized labels |
| 4.8 Lineage persistence | 🟨 Partial | Existing bus snapshot preserves fields when present; full lineage deferred |
| 4.9 Tests | 🟨 Partial | Rust + node helper tests; cost/e2e deferred |
| Rust agent loop changes | 🚫 None | Constraint honored — zero edits to run_worktree_worker |
| New dependencies | 🚫 None | No new packages |

**Phase 4-B summary (2026-06-12):**

| Item | Status | Notes |
|------|--------|-------|
| 4.4 File IO stream | 🟨 Partial | Diff-derived changed-file summary visible; true live file IO stream DEFERRED |
| 4.6 Workbench view (diff chips) | ✅ Done | File path chips, changed_file_count, test_report_excerpt in WorktreeReviewPanel |
| 4.9 Tests (diff extraction) | ✅ Done | Rust tests for diff parsing, test report excerpt, projection fields; node tests for summary |
| diff_available (metadata) | ✅ Done | Extracted from Worktree metadata artifact, projected to frontend |
| changed_file_count | ✅ Done | Total unique files from DiffSummary artifact (full count) |
| changed_files (list) | ✅ Done | First 8 unique paths from DiffSummary artifact, deduplicated |
| test_report_excerpt | ✅ Done | Short single-line excerpt from TestReport artifact (summary/result field) |
| workbenchSummary.changedFiles | ✅ Done | Deduplicated visible paths across projected task lists |
| workbenchSummary.tasksWithDiff | ✅ Done | Tasks that have a diff summary artifact |
| New StreamEvent variants | 🚫 None | No new variants added — constraint honored |
| New dependencies | 🚫 None | No new packages |
| CRITICAL paths touched | 🚫 None | Zero edits to executor/, adapters/, child.rs, worktree.rs, supervisor.rs, session.rs |

**Phase 4-C summary (2026-06-15):** Backend A2A state query surface.

| Item | Status | Notes |
|------|--------|-------|
| A2A ledger projection query | ✅ Done | `load_session_projection(_at)` maps persisted `AgentA2ABus` sidecars into `AgentA2AProjection` |
| A2A ledger list query | ✅ Done | Lists `~/.forge/a2a/*.json`, keeps valid sessions, reports corrupt sidecars as `load_errors` |
| Live A2A query surface | ✅ Done | `AgentSession::a2a_projection()` exposes current bus state without touching the agent loop |
| Tauri IPC | ✅ Done | `get_agent_a2a_state` and `list_agent_a2a_states`; live session state overrides ledger state for the same session |
| TypeScript IPC wrapper | ✅ Done | `getAgentA2AState` and `listAgentA2AStates` exported from `src/lib/tauri.ts` |
| Diagnostics integration | ✅ Done | `a2a_ledger` diagnostic summarizes readable/corrupt sidecars and links corrupt state to `clear_a2a_ledger_cache` |
| CRITICAL paths touched | 🚫 None | Zero edits to executor/, adapters/, child.rs, worktree.rs, supervisor.rs, session loop |

**Phase 4-D summary (2026-06-15):** Durable worker lease and retry state.

| Item | Status | Notes |
|------|--------|-------|
| Worker lease ownership | ✅ Done | `AgentTaskRecord` persists `lease_owner`, acquire/expiry timestamps, and `last_heartbeat_at_ms` |
| Worker heartbeat | ✅ Done | Current owner can extend an unexpired lease; wrong owner or expired heartbeats are rejected without mutating task state |
| Retry accounting | ✅ Done | `attempt_count` and `max_attempts` are persisted/projected; retryable failures can be requeued until attempts are exhausted |
| Cancel cleanup | ✅ Done | User cancellation marks the task cancelled, records a cancelled message, and clears active lease state |
| Projection + TS mirror | ✅ Done | `AgentA2ATaskProjection` exposes lease/retry fields in Rust and `src/lib/protocol.ts`; sparse legacy payloads normalize defaults |
| CRITICAL paths touched | 🚫 None | No edits to executor/, adapters/, child.rs, worktree.rs, supervisor.rs, or session loop |

**Known deferred items (Phase 4-B):**
- True live file IO stream — requires executor/ToolExecutor hooks (CRITICAL risk path)
- Token/cost per-task streaming — requires adapter trait changes
- New `StreamEvent` variants for subagent-specific events
- e2e worker lifecycle tests (depends on runnable worktree worker harness; Phase 4-D adds unit coverage for lease/heartbeat/cancel/retry)

**Acceptance gate:**

- A subagent run shows live status, file IO, and cost in the workbench. **(Phase 4-B: status/timing shown; diff-derived file visibility shown; cost deferred)**
- Parent-child relationship is visible and survives restart. **(Phase 4-A partial: projected when present and preserved by existing bus snapshot; normal delegate task population deferred)**
- Each failure type has a distinct message and recovery hint. **(Phase 4-A: failure_kind + retryable + resume_note)**

**Verification plan:**

- `cargo test` subagent/worker tests. **(Phase 4-B: a2a projection/bus tests pass — includes diff extraction, file count, test excerpt)**
- `npm run test` workbench tests. **(Phase 4-B: deriveWorkbenchSummary node tests pass — includes diff metrics)**
- Manual UX: spawn a worktree worker, observe stream, fail a smoke test, inspect reason.

---

### Phase 5 — Memory, Profiles, and Multi-Entry Triggers
**Goal:** Memory, user profiles, and task triggers work across CLI, desktop, and shared runtime; messaging triggers and cron/scheduled tasks are supported.

**Duration:** 2.5 weeks
**Depends on:** Phase 1, Phase 2

**Work breakdown:**

- [x] 5.1 Define memory storage schema: facts, embeddings placeholder, profile associations.
  - Files: `memory/facts.rs`, `memory/mod.rs`.
  - **Phase 5-A (2026-06-12):** Created `memory::facts` module with `MemoryFact` (id, text, tags, profile_id, source, created_at_ms, updated_at_ms), `MemoryFactStore` (schema_version, facts), atomic-ish JSON persistence at `~/.forge/memory.json`. Schema version explicit. No embeddings — honest placeholder comment in module docs. Existing `WikiMemoryStore` remains the context-injection memory path; unifying the two memory surfaces is deferred.
- [x] 5.2 Implement memory read/write IPC and persistence.
  - Files: `ipc/memory_handlers.rs`, `memory/facts.rs`, `state.rs`, `lib.rs`.
  - **Phase 5-A (2026-06-12):** Added `list_memory_facts`, `upsert_memory_fact`, `delete_memory_fact` Tauri commands. MemoryFactStore added to AppState. Commands registered in invoke_handler.
- [x] 5.3 Add Settings > Memory panel: view, search, edit, delete facts.
  - Files: `src/components/settings/MemoryPanel.tsx`, `SettingsCenterShell.tsx`, `src/styles/settings.css`, `src/lib/tauri.ts`, `src/hooks/queries/queryKeys.ts`, `src/hooks/queries/useMemoryFactsQuery.ts`.
  - **Phase 5-A (2026-06-12):** Full CRUD MemoryPanel wired into Settings > Memory (replaced read-only placeholder). Dense, work-focused layout with search/filter bar, inline create, inline edit text+tags, delete, loading/error/empty states, and mutation error handling. Uses existing forge-* CSS conventions and lucide icons. No nested cards, no marketing copy.
- [x] 5.10 (partial) Tests: memory store unit tests.
  - Files: `memory/facts.rs` (inline tests), `ipc/memory_handlers.rs` (existing tests still pass).
  - **Phase 5-A (2026-06-12):** 19 inline Rust tests: empty store, create/list, search (text/tags/profile_id/source, case-insensitive), update preserves created_at/changes updated_at, tag trim/dedup, delete, persist/reload roundtrip, corrupt JSON handling, atomic save (no leftover temp file), valid JSON output. All 976 Rust tests pass. Frontend `npm run build` passes.
- [x] 5.4 Add profile model: name, default model, default workspace, API key overrides.
  - Files: `profile/mod.rs`, `state.rs`, `ipc/profile_handlers.rs`.
  - **Phase 5-B (2026-06-12):** Created `profile` module with `ForgeProfile` (id, name, default_provider?, default_model?, default_workspace?, api_key_overrides?, created_at_ms, updated_at_ms), `ProfileStore` (atomic JSON persistence at `~/.forge/profiles.json`, schema_version, seed default, list/upsert/delete/set_active). 22 unit tests covering seed, create/list, update preserves created_at, set active, delete inactive, reject active delete, validation, corrupt JSON handling, atomic save, persistence roundtrip, API key overrides storage. IPC wired: `list_profiles`, `upsert_profile`, `delete_profile`, `set_active_profile` Tauri commands registered in `lib.rs`. `ProfileStore` added to `AppState`.
- [ ] 5.5 Add profile switcher in UI and CLI `--profile` flag.
  - Files: `src/components/settings/ProfilesPanel.tsx`, CLI entry.
  - **Phase 5-B partial (2026-06-12):** Settings > Profiles panel built with create/edit/delete/active selection, loading/error/empty states, mutation error handling. Frontend types, query keys, and `useProfilesQuery` hook added. Optional `profile_id` field added to `EvalHeadlessRequest` (Rust) and `HeadlessRequest` (TypeScript CLI helpers) with test. At that point CLI `run --profile` and runtime profile selection were still deferred.
  - **Phase 5.5 follow-up (2026-06-15):** `forge run --profile` is implemented and forwarded into `EvalHeadlessRequest`; headless and gateway trigger execution resolve profile provider/model/workspace defaults. Desktop new-session creation consumes the active profile defaults as well: `useSession` resolves active profile defaults before calling `create_session`, and the Rust IPC handler accepts `profile_id` so direct Tauri calls also honor profile provider/model/workspace defaults. Active profile changes now synchronize the visible composer provider/model selection so Settings and composer state agree. Settings > Memory now scopes user-managed memory facts to the active profile and writes `profile_id` on create/update. Remaining polish: unify `WikiMemoryStore` with user-managed facts and add embeddings.
- [ ] 5.6 Implement shared runtime state: CLI and desktop can attach to the same gateway/session host.
  - Files: `runtime/gateway.rs`.
  - **Deferred:** No gateway runtime exists (Phase 6 dependency).
- [ ] 5.7 Add messaging triggers: local HTTP/webhook endpoint for external messages to start sessions.
  - Files: `runtime/gateway.rs`, `ipc/handlers.rs`.
  - **Deferred:** Depends on gateway.
- [x] 5.8 Add cron/scheduled task engine: declarative tasks, next-run display, run history.
  - Files: `scheduler/mod.rs`, `ipc/scheduler_handlers.rs`, `state.rs`, `lib.rs`, `ipc/mod.rs`.
  - **Phase 5-C (2026-06-12):** Created `SchedulerStore` with `ScheduledTask` (id, title, text, enabled, interval_seconds, next_run_at_ms, last_run_at_ms, created_at_ms, updated_at_ms, tags, profile_id, last_error) and `RunHistoryEntry` (id, task_id, started_at_ms, ended_at_ms, status, message). Persisted as JSON at `~/.forge/scheduler.json` with schema_version. CRUD operations: list_payload (tasks + recent history + load_error), upsert, delete, set_enabled, run_task_now, run_due_tasks. MVP runner records deterministic history entries (completed/skipped) without creating agent sessions or invoking gateway. 5 IPC commands registered: list_scheduled_tasks, upsert_scheduled_task, delete_scheduled_task, set_scheduled_task_enabled, run_scheduled_task_now. 26 focused Rust tests passing (create/list/update/delete, enabled/disabled, next_run computation, due-run history, persistence roundtrip, corrupt JSON, atomic save, history pruning). SchedulerStore wired into AppState.
- [x] 5.9 Add Settings > Scheduler panel.
  - Files: `src/components/settings/SchedulerPanel.tsx`, `src/hooks/queries/useSchedulerQuery.ts`, `src/hooks/queries/queryKeys.ts`, `src/lib/tauri.ts`, `src/styles/settings.css`, `SettingsCenterShell.tsx`.
  - **Phase 5-C (2026-06-12):** Full CRUD SchedulerPanel wired into Settings > 调度 (Clock icon). Create/edit task with title, prompt text, tags, interval (seconds), profile_id. Inline enable/disable toggle, run now button, delete. Shows next run timestamp, last run, last error, tags, per-task history (last 5 entries), and global history panel. Mutation error handling, loading/error/empty states. Dense work-focused UI following existing forge-settings conventions. Frontend types and invoke wrappers in tauri.ts. `npm run build` passes.

**Phase 5-A summary (2026-06-12):**

| Item | Status | Notes |
|------|--------|-------|
| 5.1 Memory facts schema | ✅ Done | `MemoryFact` + `MemoryFactStore` in `memory/facts.rs` |
| 5.2 Memory IPC (list/upsert/delete) | ✅ Done | 3 new Tauri commands, registered in invoke_handler |
| 5.3 Settings > Memory panel | ✅ Done | Full CRUD UI with search, inline edit, delete, mutation errors |
| 5.10 Memory store tests | ✅ Done | 19 Rust tests, 976 total pass; npm build passes |
| 5.4 Profile model | ✅ Done | `profile/mod.rs` model + `ProfileStore` + 22 tests + IPC + AppState wiring |
| 5.5 Profile switcher | 🟨 Partial | Settings UI, CLI `--profile`, headless/gateway profile resolution, desktop new-session defaults, composer-visible sync, and active-profile memory facts done; WikiMemory/facts unification deferred |
| 5.6 Shared runtime / gateway | ⏸️ Deferred | No gateway (Phase 6) |
| 5.7 Messaging triggers | 🟨 Partial | TCP webhook, Gateway IPC enqueue, trigger runner, and CLI trigger controls exist; dashboard polish still deferred |
| 5.8 Scheduler engine | ✅ Done | Phase 5-C local MVP; background tick/gateway cron deferred |
| 5.9 Scheduler panel | ✅ Done | Phase 5-C Settings panel added |
| Embeddings | ⏸️ Deferred | Honest placeholder comment; no implementation |
| WikiMemoryStore unification | ⏸️ Deferred | Existing context memory and new user-managed facts are separate stores |
| New dependencies | 🚫 None | No new crates or npm packages |

**Phase 5-B summary (2026-06-12):**

| Item | Status | Notes |
|------|--------|-------|
| 5.4 Profile model | ✅ Done | `ForgeProfile` + `ProfileStore` in `profile/mod.rs` |
| 5.4 Profile store | ✅ Done | Atomic JSON at `~/.forge/profiles.json`, schema_version, seed default |
| 5.4 Profile IPC | ✅ Done | 4 Tauri commands: list/upsert/delete/set_active |
| 5.4 ProfileStore in AppState | ✅ Done | Added to state.rs, initialized in AppState::new |
| 5.5 Settings > Profiles panel | ✅ Done | Full CRUD UI with active selection, inline edit, delete, mutation errors |
| 5.5 Frontend types/hooks | ✅ Done | Types in tauri.ts, queryKeys.profilesAll, useProfilesQuery |
| 5.5 Headless request profile_id | ✅ Done | Optional field in Rust EvalHeadlessRequest + TS HeadlessRequest + test |
| 5.5 CLI/headless/desktop profile runtime | ✅ Done | `forge run --profile`, `EvalHeadlessRequest`, gateway triggers, and desktop `create_session` consume profile defaults |
| 5.6-5.7 Remaining items | ⏸️ Deferred | Gateway and messaging deferred; scheduler local MVP completed in Phase 5-C |
| Profile tests | ✅ Done | 22 Rust unit tests pass |
| New dependencies | 🚫 None | No new crates or npm packages |
| CRITICAL paths touched by Phase 5-B | 🚫 None | No new Phase 5-B edits to session.rs, executor/, adapters/, protocol/events.rs, agent/a2a/; those files remain dirty from earlier phases |
| Forbidden modules touched | 🚫 None | Hard boundaries honored |
| New StreamEvent variants | 🚫 None | No StreamEvent changes |

**Phase 5-C summary (2026-06-12):**

| Item | Status | Notes |
|------|--------|-------|
| 5.8 Scheduler domain | ✅ Done | `SchedulerStore` + `ScheduledTask` + `RunHistoryEntry` in `scheduler/mod.rs` |
| 5.8 JSON persistence | ✅ Done | Atomic save at `~/.forge/scheduler.json`, schema_version, corrupt JSON handling |
| 5.8 CRUD operations | ✅ Done | list/upsert/delete/set_enabled/run_task_now/run_due_tasks |
| 5.8 MVP runner | ✅ Done | Deterministic history entries; no agent sessions or gateway invocation |
| 5.8 Next-run computation | ✅ Done | `compute_next_run()` with interval, last_run, manual-only JS-safe far-future sentinel |
| 5.8 IPC commands (5) | ✅ Done | list/upsert/delete/set_enabled/run_now registered in lib.rs |
| 5.8 SchedulerStore in AppState | ✅ Done | Added to state.rs, initialized in AppState::new |
| 5.9 Settings > Scheduler panel | ✅ Done | Full CRUD UI with Clock icon, dense work-focused layout |
| 5.9 Frontend types/hooks | ✅ Done | Types in tauri.ts, queryKeys.schedulerAll, useSchedulerQuery |
| 5.9 Inline CRUD, enable/disable, run now, delete | ✅ Done | All operations wired with mutation error handling |
| 5.9 Next run, last run, history display | ✅ Done | Per-task last 5 entries + global history panel |
| Tests | ✅ Done | 26 Rust tests + npm build + cargo fmt + git diff --check all pass |
| Background tick | ⏸️ Deferred | Frontend-driven via run_scheduled_task_now; no background cron yet |
| Agent session execution | ⏸️ Deferred | MVP records history only; actual execution deferred to future phase |
| Gateway-backed cron | ✅ Done | Scheduler tick queues due tasks into `TriggerStore`; gateway runner consumes them as headless requests |
| New dependencies | 🚫 None | No new crates or npm packages |
| CRITICAL paths touched | 🚫 None | No edits to session.rs, executor/, adapters/, protocol/events.rs, agent/a2a/ |
| Forbidden modules touched | 🚫 None | Hard boundaries honored |
| New StreamEvent variants | 🚫 None | No StreamEvent changes |

**Phase 5-D summary (2026-06-15):** Gateway trigger enqueue contract and CLI.

| Item | Status | Notes |
|------|--------|-------|
| Gateway `enqueue_trigger` IPC | ✅ Done | Unix-socket JSON-RPC can queue pending triggers into the same `TriggerStore` used by webhook and scheduler |
| Trigger metadata | ✅ Done | Accepts message, optional trigger_id, profile_id, provider, model, and workspace_path |
| Runtime visibility | ✅ Done | `EnqueueTriggerResult.pending_triggers` and `runtime_status` reflect the newly queued trigger |
| Validation | ✅ Done | Missing/blank message returns JSON-RPC invalid params without mutating the queue |
| Rust trigger CLI | ✅ Done | `forge_trigger enqueue/list/runs/status` talks to the gateway socket and renders pending/runs/status output |
| Bun CLI wrapper | ✅ Done | `forge trigger enqueue/list/runs/status` forwards to `forge_trigger` with tests |
| Diagnostics enqueue control | ✅ Done | Settings > Diagnostics can enqueue a gateway trigger with optional profile/provider/model/workspace metadata and refresh runtime status |
| Diagnostics queue management | ✅ Done | Settings > Diagnostics lists pending/claimed gateway triggers and can cancel stale queue entries through the gateway IPC |
| Trigger replay controls | ✅ Done | Trigger run records now retain replay metadata; gateway IPC, Rust CLI, Bun wrapper, Tauri IPC, and Settings > Diagnostics can replay run records into the pending queue |
| Trigger run detail drilldown | ✅ Done | Gateway `get_trigger_run` RPC, Rust CLI `forge_trigger show`, Bun wrapper support, Tauri IPC, and Settings > Diagnostics detail expansion read exact run metadata by `run_id` |
| Gateway webhook smoke | ✅ Done | `npm --prefix apps/desktop run smoke:gateway:webhook` starts an isolated temporary gateway, sends a TCP JSON-line webhook trigger to `127.0.0.1:2021`, verifies it through the Unix-socket gateway RPC, cancels the smoke trigger, and removes the temp HOME |
| Still deferred | ⏸️ Deferred | None for Phase 5-D |

**Acceptance gate (updated Phase 5-C):**

- Desktop and CLI can read/write the same memory. **(Phase 5-A: ✅ Done)**
- A scheduled task records next-run display and run history in the local scheduler. **(Phase 5-C: ✅ Done — deterministic MVP history; actual agent session execution deferred)**
- A messaging trigger creates a new session via HTTP/TCP webhook. **(✅ Done for local gateway trigger ingestion — TCP webhook, IPC enqueue, gateway runner, CLI controls, Diagnostics enqueue/list/cancel/replay/detail controls, and isolated webhook smoke exist)**
- Settings > Scheduler panel allows create, edit, enable/disable, run now, delete. **(Phase 5-C: ✅ Done)**

**Verification plan:**

- `cargo test`.
- Phase 5-C targeted verification: `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml scheduler`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::scheduler` (0 handler tests matched), `npm --prefix apps/desktop run build`, `git diff --check`.
- CLI smoke: `forge --profile work msg "hello"` then open desktop and see session.
- `npm run test:eval` scheduler integration.

---

### Phase 6 — Packaging, Background Service, Update, and Self-Healing
**Goal:** Forge installs as a background service with autostart, ships a gateway dashboard, updates safely, and repairs itself when files/processes are broken.

**Duration:** 2.5 weeks
**Depends on:** Phase 2, Phase 5

**Work breakdown:**

- [ ] 6.1 Implement background gateway service binary and IPC contract.
  - **Phase 6.1 runtime-status follow-up (2026-06-15):** Gateway `runtime_status` now reports the background runtime loops that make the daemon useful as a host: webhook listener, trigger runner, and scheduler tick. The gateway binary marks each loop as started, records webhook startup failure detail if it exits, and exposes the task list through CLI/Diagnostics. This is observability scaffolding for the later true session-host contract; it does not yet move live desktop sessions into the daemon.
  - **Phase 6.1 session-registry follow-up (2026-06-15):** Gateway session metadata is now persisted at `~/.forge/gateway-sessions.json`. `register_session`/`unregister_session` update the registry atomically, and a gateway restart restores session metadata as `restored_from_registry` without counting it as a live active session until the owner re-registers. This gives CLI/Diagnostics a durable registry while keeping the active-session count honest.
  - **Phase 6.1 client-attach follow-up (2026-06-15):** Gateway client now has typed `register_session`/`unregister_session` request builders plus best-effort helpers. This keeps the eventual desktop lifecycle attach small and avoids scattering JSON-RPC method strings through `create_session`/resume/delete paths.
  - **Phase 6.1 desktop-lifecycle follow-up (2026-06-15):** Desktop session lifecycle now re-registers live sessions with the gateway best-effort after local session registration and unregisters them on kill/delete. A focused session-lifecycle test pins the live-session → `GatewaySessionInfo` projection so the registry payload carries provider, model, workspace, creation timestamp, and `restored_from_registry=false`.
  - **Phase 6.1 shutdown-cleanup follow-up (2026-06-15):** Tauri `RunEvent::Exit` now flushes snapshots and then best-effort unregisters every live desktop session from the gateway. This keeps `runtime_status.active_sessions` honest after a clean desktop quit while preserving snapshot-based restore as the source of truth for future desktop launches.
  - **Phase 6.1 stale-session follow-up (2026-06-15):** Gateway session records now include `owner_pid` and `last_seen_at_ms`; desktop sends a 60s best-effort heartbeat for live sessions, and gateway active-session counts exclude records that have not refreshed within five minutes. `forge_session list` renders those stale records as `stale` instead of `active`, which keeps CLI/Diagnostics status honest after an unclean desktop exit.
  - **Phase 6.1 attach-control follow-up (2026-06-15):** Gateway protocol now exposes `attach_session`, classifying a requested session as `live`, `restored`, `stale`, or `missing` and returning the session metadata when known. The Rust client has a typed request builder, `forge session attach <session_id>` surfaces the result, and desktop Tauri IPC/TS wrappers expose the same result for Diagnostics/History surfaces. This is the control-plane step before true stream/control attachment; live agent loops still remain owned by the desktop runtime.
  - **Phase 6.1 attach-capability follow-up (2026-06-15):** `attach_session` results now include an explicit gateway control contract (`control_plane`, `gateway_can_stream`, `gateway_can_send_input`, `gateway_can_resume`, `required_action`). The current contract honestly reports that live/restored/stale sessions still require the desktop owner/runtime, while giving CLI and future UI a stable field to flip once gateway-owned live streams land.
  - **Phase 6.1 read-only attach follow-up (2026-06-15):** Gateway attach now looks up the local session store and returns a lightweight snapshot summary when available (`snapshot`, `gateway_can_read_snapshot`). This gives CLI/UI a read-only recovery surface even before live gateway stream/input control exists; missing registry entries with a matching snapshot route to desktop restore rather than a hard unavailable state.
  - **Phase 6.1 trigger-session trace follow-up (2026-06-15):** Headless gateway-trigger runs now expose the generated agent `session_id` in the trace payload, persist it on `TriggerRunRecord`, and surface it through Diagnostics and `forge_trigger runs/show`. This gives future gateway-owned attach/resume work a durable trigger-run → session anchor.
  - **Phase 6.1 headless snapshot follow-up (2026-06-15):** Headless/gateway-trigger sessions now persist a normal `AgentSessionSnapshot` at run completion. Snapshot save failures are non-fatal and are surfaced on the headless payload as `headless_snapshot_error`, so gateway trigger results remain inspectable while Diagnostics can still see storage issues.
  - **Phase 6.1 snapshot-backed session list follow-up (2026-06-15):** `list_sessions` now merges gateway registry entries with local session snapshots, exposing snapshot-only/headless sessions as restored sessions while keeping active-session counts tied to live registry heartbeats.
  - **Phase 6.1 snapshot-detail follow-up (2026-06-15):** Gateway now exposes `get_session_snapshot` for full saved session snapshot JSON by `session_id`. The Rust gateway client, `forge_session show <session_id>`, Bun `forge session show`, Tauri IPC, and TypeScript IPC wrapper all share this read-only path, giving CLI/UI a concrete detail surface before live gateway stream/input control lands.
  - **Phase 6.1 session-event-tail follow-up (2026-06-15):** Gateway now exposes pollable read-only transcript tailing through `tail_session_events`, with cursor/limit/reset metadata and a transcript helper that does not synthesize interrupted tool/shell closures for live tails. `forge_session events <session_id> [--after <cursor>] [--limit <count>]`, Bun `forge session events`, Tauri IPC, TypeScript IPC wrapper, and Settings > Diagnostics session rows can all read the same gateway event tail. This gives attach clients a stream-like read surface while true gateway-owned live stream ownership remains deferred.
  - **Phase 6.1 session-input-inbox follow-up (2026-06-15):** Gateway now has a durable session input inbox (`~/.forge/session-inputs.json`) and `enqueue_session_input` RPC. `forge session input <session_id> <message>`, Tauri IPC, TypeScript IPC, and Settings > Diagnostics session rows can queue input addressed to an existing session, and `runtime_status`/Diagnostics expose pending input count. Desktop now polls live-session input through gateway `list_session_inputs`/`complete_session_input`, accepts records only after reserving the existing send-input turn, and then reuses the normal send-input context/continuity/snapshot path. Gateway-owned live stream/control is still deferred.
  - **Phase 6.1 session-input-history follow-up (2026-06-15):** Gateway now writes bounded session input completion history to `~/.forge/session-input-completions.json` when an owner runtime completes a queued input. `runtime_status` returns recent completion records, and `forge_trigger status` prints the latest completed session inputs so operators can distinguish "still pending" from "accepted by runtime" without reading logs.
  - Files: `src-tauri/src/bin/gateway.rs`, `runtime/gateway.rs`.
- [ ] 6.2 Add service management commands: `forge service install`, `start`, `stop`, `restart`, `uninstall`.
  - **Phase 6.2 follow-up (2026-06-15):** launchd service management is now exposed as reusable Rust APIs (`install`, `uninstall`, `start`, `stop`, `restart`, `status`). `forge_service` and Diagnostics gateway repair share the same restart path instead of duplicating `launchctl` bootout/bootstrap logic. CI-safe tests cover public API availability and launchctl output parsing for running, already-loaded, not-running, and "Could not find service" cases.
  - **Phase 6.2 status follow-up (2026-06-15):** `LaunchdServiceStatus` is now the single structured service-status snapshot used by Settings autostart IPC, Diagnostics, and the gateway service watchdog. The UI-facing payload remains stable, but backend callers no longer duplicate `launchctl print` calls or infer `running` by parsing status strings.
  - **Phase 6.2 platform-facade follow-up (2026-06-16):** Service command dispatch now goes through a platform facade instead of hard-coding launchd in `forge_service`. The facade maps macOS to launchd, Linux to systemd, Windows to the Windows service wrapper, and keeps unsupported lifecycle actions honest until their platform-specific execution paths are wired.
  - Files: `src-tauri/src/service/`.
- [ ] 6.3 Add macOS `launchd` plist generation and registration.
  - Files: `service/launchd.rs`.
- [ ] 6.4 Add Windows service wrapper and Linux systemd unit generation.
  - **Phase 6.4 generation follow-up (2026-06-16):** Added CI-safe Linux systemd user-unit generation and Windows `sc.exe` command-plan generation with structured unsupported status responses. These modules make the cross-platform service contracts visible without executing OS service registration on unsupported platforms; platform-specific install/start/stop wiring remains a later hardening step.
  - **Phase 6.4 systemd-lifecycle follow-up (2026-06-16):** Linux systemd now has real lifecycle APIs for `install`, `uninstall`, `start`, `stop`, `restart`, and `status`. Install writes the user unit, creates log directories, runs `systemctl --user daemon-reload`, and enables/starts `forge-gateway.service`; status uses `systemctl --user is-active` rather than a placeholder. Windows service execution remains deferred to the wrapper hardening step.
  - **Phase 6.4 windows-lifecycle follow-up (2026-06-16):** Windows service management now has real lifecycle APIs backed by `sc.exe create/start/stop/delete/query`, with `sc.exe query` output parsed into structured running/installed status and missing-service errors handled as non-fatal for stop/delete. The platform facade now routes Windows lifecycle commands to this wrapper.
  - Files: `service/windows.rs`, `service/systemd.rs`.
- [ ] 6.5 Add autostart toggle in Settings > General.
  - **Phase 6.5 service-ipc follow-up (2026-06-16):** Settings service status/autostart IPC now uses the cross-platform service facade instead of calling launchd directly. The payload preserves legacy `label`/`launch_domain`/`plist_path` fields while adding explicit `backend`, `service_id`, `service_path`, and `status_message` so the UI can describe launchd, systemd, and Windows Service backends honestly.
  - Files: `src/components/settings/GeneralSettings.tsx`.
- [ ] 6.6 Add dashboard: lightweight web UI served by gateway showing sessions, health, and logs.
  - **Phase 6.6 backend snapshot follow-up (2026-06-15):** Gateway now exposes a read-only `dashboard_snapshot` JSON-RPC method that aggregates runtime status, registered/snapshot-backed sessions, queued triggers, recent trigger runs, completed session inputs, and a compact dashboard event log. The Rust gateway client has a typed request builder, `forge_trigger dashboard` renders the snapshot, and the Bun `forge trigger dashboard` wrapper forwards the command. This is the backend/control-plane contract for a future web dashboard; the actual web UI is still deferred.
  - **Phase 6.6 local HTTP dashboard follow-up (2026-06-16):** Gateway now serves a read-only loopback HTTP dashboard on `127.0.0.1:2022`: `/` returns a minimal HTML shell and `/api/dashboard` returns the same dashboard snapshot JSON used by the Unix-socket control plane. The dashboard listener is started by the gateway binary, reported as `dashboard_http` in `runtime_status`, and failures are captured in the runtime task event log.
  - Files: `apps/website` or a new `apps/dashboard` per monorepo rule; likely reuse `apps/website`.
- [ ] 6.7 Implement update repair: on update, detect stale gateway/config, run doctor, repair if needed.
  - **Phase 6.7 partial (2026-06-15):** Added `diagnostics/update_repair.rs` as the update-repair planning/execution layer. It converts `DiagnosticsReport` warnings/failures into deduplicated repair actions, keeps unrepairable failures as manual blockers, and executes the plan through an injectable runner (real path uses `run_repair`). Follow-up safety hardening restricts automatic update repair to conservative service lifecycle actions (`restart_gateway`, `reinstall_service`); destructive or unknown actions such as clearing snapshots, A2A ledgers, or logs are retained as manual blockers. Focused tests cover gateway repair planning, service-action allowlisting, destructive/unknown action blocking, manual blockers for config failures, action deduplication, ordered execution, and failed repair results. Automatic invocation during an app update is still deferred until updater lifecycle hooks exist.
  - Files: `diagnostics/update_repair.rs`, `diagnostics/mod.rs`.
- [ ] 6.8 Add self-healing actions from diagnostics: restart gateway, clear snapshot cache, reinstall service.
  - **Phase 6.8 partial (2026-06-15):** `RepairResult` now carries optional post-action verification detail. `restart_gateway` and `reinstall_service` verify `launchd::status()` after the repair command and fail honestly when the service is still not running, rather than reporting command success as repair success. Settings > Diagnostics formats verification detail in the repair result message.
  - **Phase 6.8 cache verification follow-up (2026-06-16):** Destructive cache repairs now verify their post-action state too: `clear_snapshot_cache` and `clear_a2a_ledger_cache` return `RepairVerification` showing the target cache directory is empty or honestly reporting inspection failure. This gives Settings > Diagnostics the same evidence-backed result shape for cache cleanup as service repairs.
  - **Phase 6.8 diagnostics-service follow-up (2026-06-16):** Diagnostics gateway-service checks now consume the cross-platform `ServiceStatusSnapshot` facade instead of querying launchd directly. The diagnostic detail includes `backend`, `service_id`, and `service_path`, so launchd, systemd, and Windows Service status appear honestly in reports while preserving legacy path fields for existing UI.
  - Files: `diagnostics/repair.rs`, `src/components/settings/DiagnosticsPanel.tsx`, `src/components/settings/diagnosticsRepairView.ts`, `src/lib/ipc/types.ts`.
- [ ] 6.9 Tests: service install/uninstall in CI-safe mock mode, update repair tests.
  - **Phase 6.9 partial (2026-06-15):** Added CI-safe launchd command-output parsing tests and service-management API coverage without invoking real `launchctl` state changes.
  - **Phase 6.9 status partial (2026-06-15):** Added CI-safe conversion tests proving IPC, Diagnostics, and watchdog consume the same structured `LaunchdServiceStatus`.

**Acceptance gate:**

- `forge service install` registers a background service and `forge service status` shows healthy.
- Autostart toggle survives OS logout/login.
- Simulated corrupt install triggers update repair and recovers.

**Verification plan:**

- `cargo test` service/updater tests.
- Manual UX on macOS: install, reboot, confirm service running.
- Manual dashboard smoke in browser.

---

### Phase 7 — Product Polish & Acceptance Suite
**Goal:** Desktop product is complete: settings, history, recovery/error states, permission states, previews, review flows, and background task surfaces all feel cohesive and pass a final acceptance suite.

**Duration:** 2 weeks
**Depends on:** All prior phases

**Work breakdown:**

- [ ] 7.1 Complete Settings dialog: models, workspace, tools, memory, data, about, diagnostics, scheduler, general.
  - Files: `src/components/settings/`.
- [ ] 7.2 Implement History view: searchable, filterable list of all sessions with restore/delete.
  - Files: `src/components/history/HistoryView.tsx`.
- [ ] 7.3 Implement recovery/error states: offline, gateway disconnected, API key missing, snapshot corrupted.
  - Files: `src/components/RecoverySurface.tsx`, `store/index.ts`.
- [ ] 7.4 Implement permission states: per-tool permission levels, allowlist, denylist, reset.
  - Files: `executor/permissions.rs`, `src/components/settings/PermissionsPanel.tsx`.
- [ ] 7.5 Implement rich previews: image diff, file tree diff, markdown preview for writes.
  - Files: `src/components/messages/WriteFilePreview.tsx`, `DiffPreview.tsx`.
- [ ] 7.6 Harden review flows: approve/reject UI, bulk review, review history.
  - Files: `src/components/review/ReviewPanel.tsx`.
- [ ] 7.7 Background task surfaces: global status bar, task list, notifications.
  - Files: `src/components/StatusBar.tsx`, `src/components/tasks/TaskManager.tsx`.
- [ ] 7.8 Final acceptance suite: end-to-end script covering resume, doctor, tool enable/disable, subagent, scheduler, settings round-trip.
  - Files: `scripts/acceptance.sh`, `e2e/acceptance.spec.ts`.
- [ ] 7.9 Documentation pass: update README, AGENTS.md, and CHANGELOG for new surfaces.

**Acceptance gate:**

- Final acceptance suite passes on clean checkout.
- No HIGH or CRITICAL GitNexus impact regressions.
- Manual UX walkthrough by controller (Codex) signs off.

**Verification plan:**

- `npm run build:desktop`
- `npm run build:website`
- `npm run test:eval`
- `scripts/acceptance.sh`
- `gitnexus_detect_changes()` before any commit.

---

## GitNexus Safety Reminders for Future Implementation Phases

Before any function/class/method edit in subsequent phases:

1. Run `gitnexus_impact({ target: "symbolName", direction: "upstream" })` (or `mcp__gitnexus__impact`).
2. Report the blast radius: direct callers, affected processes, risk level.
3. If the risk is HIGH or CRITICAL, warn the user and get explicit approval before proceeding.
4. Never rename symbols with find-and-replace; use `gitnexus_rename`.
5. Before committing any phase, run `gitnexus_detect_changes()` and verify only expected symbols and execution flows are affected.

## Risk Register

| # | Risk | Likelihood | Impact | Mitigation |
|---|------|------------|--------|------------|
| 1 | Session snapshot schema churn breaks backward compatibility early | Medium | High | Version snapshots; corruption fallback; migration tests. |
| 2 | Background service integration differs significantly across macOS/Windows/Linux | High | Medium | Start with launchd; abstract service trait; mock in CI. |
| 3 | Subagent cost telemetry requires adapter changes that ripple to all providers | Medium | High | Add optional cost fields to `AiAdapter` trait; default impls. |
| 4 | Unified UI consolidation scope creeps into redesign | Medium | Medium | Strict acceptance gates; Codex reviews UI PRs. |
| 5 | Shared runtime between CLI and desktop creates lock contention on state files | Medium | High | Gateway owns state; CLI attaches via IPC; no direct file writes. |
| 6 | Phase 6 service packaging delays final acceptance | Medium | High | Parallelize dashboard work; mock service mode for earlier phases. |
| 7 | Eval runner tests lag behind desktop changes | Low | High | Add eval-runner doctor pre-check in Phase 2; keep tests updated. |

## Sequencing and Dependency Notes

- Phase 1 unlocks everything else. Do not start Phase 4 without Phase 1 (subagent lineage needs resumable sessions).
- Phase 2 unlocks Phase 6 (update repair depends on diagnostics checks).
- Phase 3 and Phase 4 can overlap once Phase 1 is done, but Phase 4 needs Phase 3's tool-count surface.
- Phase 5 should follow Phase 2 so scheduler tasks can report health via diagnostics.
- Phase 6 should follow Phase 5 because the background service hosts shared runtime and scheduler.
- Phase 7 must be last and includes final integration; no new feature work in Phase 7.

## Controller Protocol — Codex ↔ Claude Code

### Delegation format

Codex opens a phase by sending Claude Code a directive with this JSON shape:

```json
{
  "phase": "Phase 1: Session Runtime Persistence & Restore",
  "goal": "One-sentence goal",
  "tasks": ["1.1", "1.2", "..."],
  "acceptance_criteria": ["..."],
  "branch": "cabbos/internal-a2a-runtime-plan",
  "constraints": ["No product/runtime code outside this phase", "Run gitnexus_impact before edits"]
}
```

### Worker return format

When Claude Code finishes a task batch, it returns:

```json
{
  "phase": "Phase 1: Session Runtime Persistence & Restore",
  "tasks_completed": ["1.1", "1.2"],
  "tasks_remaining": ["1.3"],
  "files_changed": [
    "apps/desktop/src-tauri/src/session/snapshot.rs",
    "apps/desktop/src-tauri/src/protocol/events.rs"
  ],
  "tests_run": ["cargo test -p desktop snapshot", "npm run test -- resume"],
  "test_results": "PASS / FAIL with counts",
  "gitnexus_impact": ["symbol: impact summary"],
  "gitnexus_detect_changes": "affected processes summary or N/A",
  "blockers": [],
  "ready_for_verification": true
}
```

### What Codex verifies before continuing

1. All acceptance criteria for the phase are listed as completed or explicitly waived.
2. Tests listed have passing results with no flaky failures.
3. `files_changed` are within the expected modules for the phase.
4. GitNexus impact analysis was run for every edited symbol and no HIGH/CRITICAL risk was ignored.
5. `gitnexus_detect_changes()` was run and affected processes match the phase scope.
6. No product/runtime code was modified outside the phase scope.
7. No commit or push was made unless explicitly requested in a later step.

### Stop / escalate conditions

Codex must stop and ask the user if any of the following occur:

- A HIGH or CRITICAL GitNexus impact is reported and Claude Code cannot mitigate it within the phase.
- The implementation introduces a new external dependency not already in the lockfile.
- The plan file itself needs to change (scope, phase order, new phase).
- A phase acceptance gate fails twice after remediation.
- Claude Code reports a blocker that requires cross-phase or architectural redesign.

## Milestone Table

| Order | Milestone | Target Date | Deliverable | Acceptance Gate |
|-------|-----------|-------------|-------------|-----------------|
| 0 | A2A/runtime review hardening | DONE | Phase 0 commits | Multi-agent arbitration + smoke validation |
| 1 | Session persistence & restore | 2026-06-12 ✅ | Snapshot schema, resume, tests | Restart restores session and pending confirms |
| 2 | Diagnostics, doctor, watchdog | 2026-06-12 ✅ | `forge doctor`, health UI, watchdogs | Gateway/session failure recovered and visible |
| 3 | Tool/provider/skills UI | 2026-06-12 ✅ | Ecosystem settings, tool counts | Enable/disable works; health visible |
| 4 | Subagent workbench | 2026-06-12 ✅ | Status/failure/file IO/lineage UI | Subagent stream visible and resumable |
| 5 | Memory/profiles/multi-entry | 2026-06-12 ✅ | Memory store, profiles, scheduler, `forge --profile` + `forge session list` | CLI/desktop share state; `--profile` flag works |
| 6 | Packaging/background/update | 2026-06-12 🟨 | Gateway binary, service management, launchd, autostart toggle | Gateway compiles + launchd plist ready; dashboard + update repair deferred |
| 7 | Product polish & acceptance | TBD | Settings/history/recovery/review surfaces | Deferred |

### 2026-06-12 Final Progress Summary

**Completed:**
- ✅ Phase 5.5: CLI `--profile` flag + runtime profile resolution for headless, gateway triggers, and desktop new-session creation
- ✅ Phase 6-A: Gateway core — protocol types + JSON-line IPC + server dispatch (ping/health/list_sessions/register_session/unregister_session)
- ✅ Phase 6-B: launchd plist generation + install/uninstall/start/stop/restart/status + `forge_service` binary + CLI `forge service` command
- ✅ Phase 6.5: Autostart toggle in Settings > General with install/running status badges
- ✅ Phase 5.6: Gateway client library + session tracking + `forge_session` binary + CLI `forge session list` command
- ✅ Phase 5.7: `forge_trigger` binary + `forge trigger enqueue/list/runs/status` CLI wrapper
- ✅ Phase 2.5: Gateway service watchdog — 30s probe, automatic restart repair, global HealthAlert, exponential backoff
- ✅ Phase 6.7 partial: Update repair planner/runner maps diagnostics to repair actions, allows only conservative service repair actions to run automatically, and keeps destructive/unknown actions as manual blockers
- ✅ Phase 6.8 partial: Gateway repair actions now verify post-repair service health and surface verification detail in Diagnostics

**Test totals:**
- Rust: 1057 tests pass, 0 fail
- CLI (Bun): 40 tests pass, 0 fail
- Frontend: `npm run build` passes clean

**New files created:**
- `apps/desktop/src-tauri/src/gateway/mod.rs`
- `apps/desktop/src-tauri/src/gateway/protocol.rs` (10 tests)
- `apps/desktop/src-tauri/src/gateway/server.rs` (6 tests)
- `apps/desktop/src-tauri/src/gateway/client.rs` (3 tests)
- `apps/desktop/src-tauri/src/service/mod.rs`
- `apps/desktop/src-tauri/src/service/launchd.rs` (7 tests)
- `apps/desktop/src-tauri/src/ipc/service_handlers.rs` (2 tests)
- `apps/desktop/src-tauri/src/bin/gateway.rs`
- `apps/desktop/src-tauri/src/bin/forge_service.rs`
- `apps/desktop/src-tauri/src/bin/forge_session.rs`
- `apps/desktop/src-tauri/src/bin/forge_trigger.rs`
- `apps/desktop/cli/src/commands/run.ts` (rewritten from stub — parseRunArgs + runCommand)
- `apps/desktop/cli/src/commands/service.ts`
- `apps/desktop/cli/src/commands/session.ts`
- `apps/desktop/cli/src/commands/trigger.ts`
- `apps/desktop/cli/test/service.test.ts`
- `apps/desktop/cli/test/trigger.test.ts`
- `apps/desktop/src/components/settings/GeneralSettings.tsx`

**Deferred for future:**
- Phase 6.6: Dashboard web UI
- Phase 6.7: Automatic update lifecycle hook for update repair
- Phase 6.8: Update-aware self-healing orchestration and deeper repair flows
- Phase 5.7: Messaging trigger full dashboard listing/replay polish
- Phase 7: Full product polish + acceptance suite

## Appendix — Likely Files/Modules by Domain

- **Backend state & sessions:** `apps/desktop/src-tauri/src/lib.rs`, `state.rs`, `agent/session.rs`, `agent/snapshot.rs`, `pty/session.rs`
- **Protocol:** `apps/desktop/src-tauri/src/protocol/events.rs`, `apps/desktop/src/lib/protocol.ts`
- **IPC:** `apps/desktop/src-tauri/src/ipc/handlers.rs`, `apps/desktop/src/lib/tauri.ts`
- **Store/UI state:** `apps/desktop/src/store/index.ts`
- **Tool execution:** `apps/desktop/src-tauri/src/executor/`
- **Adapters:** `apps/desktop/src-tauri/src/adapters/`
- **Plugin manager:** `apps/desktop/src-tauri/src/plugin_manager/`
- **Subagents:** `apps/desktop/src-tauri/src/agent/worker.rs`, `agent/arbitration.rs`
- **Settings UI:** `apps/desktop/src/components/settings/`
- **Message blocks:** `apps/desktop/src/components/messages/`
- **Eval runner:** `apps/eval-runner/`
- **Website/dashboard:** `apps/website/`

---

*Document owner: Forge Runtime / Agent Team*
*Next action: Codex should review this roadmap, then delegate Phase 1 to Claude Code with the controller protocol JSON.*
