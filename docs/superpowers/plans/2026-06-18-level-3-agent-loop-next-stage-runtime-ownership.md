# Level 3 Agent Loop Next Stage Runtime Ownership Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the next Level 3 hardening stage by proving Forge can own loop runtime state across restarts, emit honest usage and file-effect evidence, preserve A2A lineage, gate autonomous gateway ownership, and keep commit as a human decision.

**Architecture:** Build on the committed Level 3 MVP instead of replacing it. The gateway-owned loop ledger, runner lease state, runtime event protocol, A2A bus, ToolExecutor `file_io` stream, desktop runtime projections, and acceptance script remain the substrate; this stage adds restart harness evidence, normalized usage telemetry, bounded shell delta evidence, gated headless ownership policy, durable parent-side lineage, and review-to-commit eligibility. Autonomous resume is only enabled behind explicit policy and human approval, and the default path continues to stop at review.

**Tech Stack:** Rust/Tauri backend, gateway JSON-RPC and loop runner, `AgentSession`, `ToolExecutor`, `StreamEvent`, existing A2A bus/worktree worker, React/TypeScript protocol/store/UI, Playwright mocked acceptance, future platform-gated Tauri/WebDriver acceptance on Windows/Linux, `scripts/acceptance.sh`, GitNexus impact analysis.

---

## Current State And Evidence

This plan starts after Phase 4-K was committed as `b959091b feat(runtime): stream executor file io events`.
Task 6 is now represented by code commit `33bee230 feat(runtime): harden review to commit eligibility` and docs evidence commit `4ccdf702 docs(runtime): record task 6 evidence`.

The committed Level 3 MVP evidence is:

- Tasks 1-3: durable loop task ledger, gateway create/list/get/cancel RPC, policy/human gate foundations, and completion contract evaluator.
- Task 3.5: runtime contract reconciliation for event envelope metadata, statuses, lease/attempt/causation fields, policy decisions, budget snapshots, completion events, and projection replay.
- Task 4: runtime event protocol and frontend projection ingress for `subagent_runtime_event` and `loop_runtime_updated`.
- Task 5: `LoopUsageLedger` and boundary file IO facts with explicit unknown token/cost handling.
- Task 6: gateway-owned loop runner leases, stale lease interruption, runtime status/dashboard queue stats, and the explicit "no headless AgentSession" waiting boundary.
- Task 7: desktop and static dashboard runtime fact consumption through `LoopTaskPanel`, `TaskManager`, `StatusBar`, and A2A runtime fact rows.
- Task 8: acceptance/docs product proof for journal, projection replay, policy, budget, gates, completion, runner status, subagent runtime projection, mocked desktop completion contract, Phase 7 smoke, and rich previews.
- Phase 4-I: real Rust worktree worker lifecycle harness acceptance for `ChildAgentRuntime::run_worktree_worker`.
- Phase 4-J: A2A child runtime bridge for child file-ish facts with parent `AgentSession.id`, real A2A `task_id`, and no invented loop task id.
- Phase 4-K: direct `ToolExecutor` file-ish operations emit live `file_io` stream facts after successful read/write/edit/git diff/list/search operations, and frontend attaches those facts to existing tool/shell block metadata.

The current truth is strong but bounded:

- Direct ToolExecutor file IO is live for file-ish tools.
- A2A child file-ish facts are live only when a task-aware child runtime context exists.
- Runner leases are durable, but the runner stops at `waiting_for_input` instead of continuing a complete coding loop.
- The loop ledger is durable and stricter than projection-style gateway stores.
- Completion contracts and review/control gates exist, but commit remains a human gate.
- Runtime facts exist, but shell-internal tracing, full provider usage coverage, gateway autonomous resume, and auto commit/merge/push are still unclaimed.

## Non-Goals And Hard Boundaries

- No auto commit/merge/push. A task can become eligible for human review, but Forge must not run `git commit`, merge a worktree, or push without an explicit human action outside the autonomous loop.
- Do not claim shell-internal tracing until it is implemented and tested. Post-shell worktree delta evidence can prove file effects after a command completes; it is not true live shell-internal tracing.
- Provider token/cost can remain unknown until the adapter supplies usage. Unknown values must be first-class runtime facts, not silently converted to zero.
- Gateway autonomous resume and headless `AgentSession` ownership are future work until this plan's gated policy path lands. The default runner path continues to require an existing desktop session or human approval.
- Do not extract shared packages across apps in this stage. The repo rule still holds: keep `apps/desktop`, `apps/eval-runner`, and `apps/website` independently runnable until code is actually shared by at least two apps.
- Do not widen the runtime claim by updating docs without matching tests and acceptance evidence.

## File Structure For This Stage

- `apps/desktop/src-tauri/src/loop_runtime/runner.rs` owns loop runner lease transitions, waiting reasons, stale interruption, and the future gated headless ownership branch.
- `apps/desktop/src-tauri/src/loop_runtime/types.rs`, `journal.rs`, `projection.rs`, `budget.rs`, `completion.rs`, `gates.rs`, and `policy.rs` own durable event, projection, budget, completion, gate, and policy facts.
- `apps/desktop/src-tauri/src/gateway/server.rs`, `protocol.rs`, `dashboard.rs`, and `apps/desktop/src-tauri/src/bin/gateway.rs` expose loop runtime state and start the runner.
- `apps/desktop/src-tauri/src/protocol/events.rs` and `apps/desktop/src/lib/protocol.ts` are the Rust/TypeScript stream protocol contract and must stay in sync.
- `apps/desktop/src/store/event-dispatch.ts`, `apps/desktop/src/store/blocks.ts`, `apps/desktop/src/store/runtime-projections.ts`, and their tests own frontend event ingestion.
- `apps/desktop/src-tauri/src/executor/mod.rs`, `executor/shell.rs`, and `executor/executor_test.rs` own direct tool/shell execution evidence.
- `apps/desktop/src-tauri/src/adapters/base.rs`, `anthropic.rs`, `openai_compatible.rs`, `codex.rs`, `claude.rs`, and `hermes.rs` own provider usage availability.
- `apps/desktop/src-tauri/src/agent/session/mod.rs`, `agent/session/loop.rs`, `agent/session/tools.rs`, `agent/session/a2a.rs`, `agent/session_tests.rs`, and `agent/snapshot.rs` own `AgentSession` execution, snapshot, and A2A session coupling.
- `apps/desktop/src-tauri/src/agent/a2a/bus.rs`, `types.rs`, `projection.rs`, `supervisor.rs`, `child.rs`, `ledger.rs`, and `review_gate.rs` own A2A lineage, worker lifecycle, worktree evidence, and review gates.
- `apps/desktop/src/components/loop/LoopTaskPanel.tsx`, `components/messages/AgentA2ATimeline.tsx`, `components/tasks/TaskManager.tsx`, `src/lib/loopRuntime.ts`, `src/lib/backgroundTaskStatus.ts`, and related tests own runtime UI presentation.
- `apps/desktop/e2e/acceptance.spec.ts`, `apps/desktop/e2e/a2a-confirm-runtime.spec.ts`, `apps/desktop/e2e/resume.spec.ts`, the new restart harness spec, `scripts/acceptance.sh`, and `scripts/acceptance.test.mjs` own product acceptance evidence.
- `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`, this plan, and the Obsidian mirror own the public evidence narrative.

## GitNexus Rules For Every Code Task

Before editing any function, class, method, or enum variant, run upstream impact analysis for the exact symbol and report the blast radius. If GitNexus returns HIGH or CRITICAL risk, pause and give the controller the direct callers, affected processes, and intended containment before editing.

Run `gitnexus_detect_changes(repo: "forge", scope: "all")` before any commit. If the detected affected processes do not match the task, stop and reconcile before staging.

Use these likely high-risk symbols with `summaryOnly: true` first when they are in scope:

```text
gitnexus_impact(repo: "forge", target: "AgentSession", file_path: "apps/desktop/src-tauri/src/agent/session/mod.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "dispatch", file_path: "apps/desktop/src-tauri/src/gateway/server.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "ToolExecutor", file_path: "apps/desktop/src-tauri/src/executor/mod.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "StreamEvent", file_path: "apps/desktop/src-tauri/src/protocol/events.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "createOutputEventDispatcher", file_path: "apps/desktop/src/store/event-dispatch.ts", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "applyTranscriptEventToBlocks", file_path: "apps/desktop/src/store/blocks.ts", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "AgentA2ABus", file_path: "apps/desktop/src-tauri/src/agent/a2a/bus.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "run_worktree_worker", file_path: "apps/desktop/src-tauri/src/agent/a2a/child.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "classify_tool_category", file_path: "apps/desktop/src-tauri/src/agent/turn_state.rs", direction: "upstream", summaryOnly: true)
```

## Suggested Execution Order

1. Current macOS next step: Task 2 provider usage telemetry. Task 1 official Tauri/WebDriver binary force-quit/reopen proof is `BLOCKED_OFFICIAL_MACOS` and deferred to a future Windows/Linux platform gate; keep `18f6ce9a` as partial mocked evidence only.
2. Task 2 provider usage telemetry, because it is additive and already has partial Anthropic support plus unknown handling.
3. Task 3 post-shell file-effect evidence, because it strengthens observability without claiming shell-internal tracing.
4. Task 4 gateway autonomous resume/headless ownership, because it is the highest-risk behavioral step and must stay disabled by default behind policy and human approval.
5. Task 5 A2A lineage completion, because parent ownership should be durable before autonomous gateway resume chooses work.
6. Task 6 review-to-commit hardening, because autonomous ownership must never bypass review gates.
7. Task 7 acceptance matrix and evidence docs after each runtime slice has its proof.

## Risk Map

- **CRITICAL:** `AgentSession`, `agent/session/loop.rs`, `agent/session/tools.rs`, and snapshot restore. Mistakes can duplicate model calls, lose pending confirmations, or resume side effects after restart.
- **CRITICAL:** Gateway dispatch/server and loop runner. Mistakes can mutate durable ledger state incorrectly or create unbounded autonomous work.
- **HIGH:** `ToolExecutor`, shell execution, event emission, and permission gates. Mistakes can over-report side effects or bypass confirmation.
- **HIGH:** `StreamEvent` and TypeScript protocol/store mirrors. Mistakes can break frontend ingestion or session-scoped runtime facts.
- **HIGH:** A2A bus/session paths, parent lineage, worktree worker lifecycle, and review gates. Mistakes can orphan child tasks or make review decisions non-durable.
- **HIGH:** Autosave/process activity classification in `agent/turn_state.rs` and frontend block/activity summaries. Mistakes can make runtime evidence appear stronger than the emitted facts.
- **MEDIUM:** Static gateway dashboard and background task UI. Mistakes can mislead users but should be contained by projection tests and mocked e2e.
- **MEDIUM:** Docs and acceptance labels. Mistakes can overclaim capability even when runtime code is correct.

---

### Task 1: Tauri/WebDriver Force-Quit/Reopen Runtime Harness

**Goal:** Add a product-level harness that proves durable loop/session/A2A runtime state survives a real desktop app quit/reopen sequence without claiming autonomous continuation.

**Current state (2026-06-18):** commit `18f6ce9a test(runtime): add mocked restart runtime smoke` added honest partial product evidence in `apps/desktop/e2e/level3-runtime-restart.spec.ts` and advertises it as `mocked desktop restart runtime smoke`. It closes and reopens a Playwright page through the existing Vite + mocked Tauri IPC harness, replays durable runtime facts from IndexedDB, and verifies no autonomous continuation. This is not full Task 1 completion: true Tauri/WebDriver binary force-quit/reopen remains open because the current e2e contract is Playwright + Vite + mocked Tauri IPC and no `tauri-driver`/WebDriver launcher is present.

**Official blocker:** fresh environment discovery on macOS/Darwin found no `tauri-driver`, `WebKitWebDriver`, `msedgedriver`, WebdriverIO, or Selenium harness dependencies. Official Tauri v2 desktop WebDriver docs describe desktop WebDriver support for Windows/Linux only because macOS lacks a WKWebView driver tool: https://v2.tauri.app/develop/tests/webdriver/. The official WebdriverIO path also expects a separate WDIO harness, a debug Tauri binary, and `~/.cargo/bin/tauri-driver`: https://v2.tauri.app/develop/tests/webdriver/example/webdriverio/.

**Next path:** keep the mocked smoke as partial evidence on macOS. A future official proof should be a Windows/Linux CI or platform-gated slice that installs `tauri-driver` with `cargo install tauri-driver --locked`, uses the native driver available on that platform (`WebKitWebDriver` or `msedgedriver`), launches the debug Tauri binary through the official WebDriver harness, then force-quits and reopens the desktop app. Do not introduce a third-party `tauri-plugin-webdriver` path without an explicit architecture decision.

**Files:**
- Create: `apps/desktop/e2e/level3-runtime-restart.spec.ts`
- Modify: `apps/desktop/e2e/fixtures/app.ts`
- Modify: `apps/desktop/e2e/resume.spec.ts` only if existing helpers should be shared.
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`
- Read before editing: `apps/desktop/package.json`, `apps/desktop/e2e/resume.spec.ts`, `apps/desktop/e2e/fixtures/app.ts`, `apps/desktop/src-tauri/src/loop_runtime/replay_tests.rs`, `apps/desktop/src-tauri/src/loop_runtime/runner.rs`

**GitNexus impact requirements:**

```text
gitnexus_impact(repo: "forge", target: "createHydrateAction", file_path: "apps/desktop/src/store/hydration.ts", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "createOutputEventDispatcher", file_path: "apps/desktop/src/store/event-dispatch.ts", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "serve_loop_runner", file_path: "apps/desktop/src-tauri/src/loop_runtime/runner.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "AgentSessionSnapshot", file_path: "apps/desktop/src-tauri/src/agent/snapshot.rs", direction: "upstream", summaryOnly: true)
```

**Implementation steps:**

- [ ] **Step 1.1: Discover the current e2e launch and restart contract**

  Run:

  ```bash
  rg -n "resume|restart|close|reload|launch|tauri|electron|webkit|mockIpc|setup" apps/desktop/e2e apps/desktop/package.json
  ```

  Expected: on current macOS, record `BLOCKED_OFFICIAL_MACOS` for true Tauri/WebDriver desktop force-quit/reopen and keep the mocked Playwright/Vite smoke as partial evidence. On a Windows/Linux runner, identify the official `tauri-driver` plus native-driver launch path and record the platform-gated command in the task handoff.

- [ ] **Step 1.2: Write the failing restart smoke**

  Add `apps/desktop/e2e/level3-runtime-restart.spec.ts` with one test that:

  - starts a session with persisted session snapshot state,
  - injects or creates a loop task in `waiting_for_input` with a runner lease history,
  - injects A2A projection state with a retained worktree worker, runtime file facts, and usage facts with unknown cost,
  - force-quits the app process through the official Tauri/WebDriver harness on a supported Windows/Linux platform,
  - reopens the app,
  - asserts History/session restore, TaskManager/LoopTaskPanel, A2A workbench, and gateway runtime status still show the durable facts,
  - asserts the loop is waiting for human input and did not continue side effects.

  Run:

  ```bash
  npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts
  ```

  Expected first result: FAIL because the spec or restart helper does not exist.

- [ ] **Step 1.3: Add the smallest restart helper**

  Extend `apps/desktop/e2e/fixtures/app.ts` only where the existing harness owns app setup. The helper must expose explicit `quitApp()` and `reopenApp()` style operations or equivalent names discovered in Step 1.1. Keep mocked IPC state contract-shaped so the UI path is the real user path.

- [ ] **Step 1.4: Assert no autonomous continuation after reopen**

  The spec must check for the existing waiting copy from `loop_runtime/runner.rs`:

  ```text
  autonomous agent resume is disabled
  ```

  or the no-headless owner copy:

  ```text
  no headless agent session was created
  ```

  If product text changes, assert a stable test id or raw loop status instead of localized prose.

- [ ] **Step 1.5: Add acceptance script coverage**

  Add a label such as `mocked desktop restart runtime smoke` to `scripts/acceptance.sh` until a true Tauri/WebDriver harness exists on Windows/Linux, and add the exact command:

  ```bash
  npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts
  ```

  Extend `scripts/acceptance.test.mjs` to assert the label and command appear in `--dry-run`.

- [ ] **Step 1.6: Run focused verification**

  Run:

  ```bash
  npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts
  node scripts/acceptance.test.mjs
  scripts/acceptance.sh --dry-run
  git diff --check
  ```

  Expected: all pass.

**Acceptance command:** `npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts` currently proves the mocked desktop restart runtime smoke only. The future official Tauri/WebDriver force-quit/reopen command must be platform-gated to Windows/Linux.

**Expected commit message:** `test(runtime): add level 3 restart harness`

**Not claimed by this task:**

- No gateway autonomous resume after crash.
- No default headless `AgentSession`.
- No shell-internal tracing.
- No auto commit/merge/push.
- No official macOS Tauri/WebDriver desktop proof while macOS lacks a WKWebView driver tool.
- No third-party WebDriver plugin path without an explicit architecture decision.

---

### Task 2: Provider Token/Cost Telemetry Stream With Explicit Unknown Handling

**Goal:** Normalize provider usage telemetry so known adapter usage becomes runtime facts, and unknown tokens/cost remain explicit unknowns across direct sessions, loop tasks, and A2A child runs.

**Status (2026-06-18): Implemented for the active Anthropic and OpenAI-compatible adapter paths.** Added an additive `provider_usage` stream event with nullable `input_tokens`, `output_tokens`, `estimated_cost_micros`, adapter `source`, model, and reason (`provider_reported`, `provider_omitted`, `pricing_unknown`). Legacy numeric `usage` remains compatible and is emitted only when token usage and known pricing are available; omitted provider usage and unknown pricing are not converted to zero or default Sonnet pricing. `UsageEvent` now carries source/reason metadata, `UsageCaptureEmitter` prefers normalized provider usage records, and task-aware subagents emit live `usage_recorded` runtime facts from the same records used by `LoopUsageLedger`. Direct-session `provider_usage` events now render as completed transcript blocks without double-counting legacy `usage` cost/context state, including provider-omitted and pricing-unknown facts. Frontend runtime usage facts expose structured model/source/reason, nullable token/cost values, and explicit unknown flags while preserving the existing label/detail rows that render `provider omitted` and `pricing unknown` distinctly.

**Evidence/tests (fresh during implementation):**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::budget --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml unknown_pricing --lib
node --test apps/desktop/src/lib/loopRuntime.test.ts
node --test apps/desktop/src/store/blocks.test.ts
node --test apps/desktop/src/store/event-dispatch.test.ts
node scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
```

**Not claimed:** billing-grade usage accounting, exact cost when provider usage or model pricing is unknown, shell-internal file tracing, gateway autonomous resume, or auto commit/merge/push. `codex.rs`, `claude.rs`, and `hermes.rs` are still stubs in this repo and were not given new model-call behavior.

**Files:**
- Modify: `apps/desktop/src-tauri/src/adapters/base.rs`
- Modify: `apps/desktop/src-tauri/src/adapters/anthropic.rs`
- Modify: `apps/desktop/src-tauri/src/adapters/openai_compatible.rs`
- Modify: `apps/desktop/src-tauri/src/adapters/codex.rs`
- Modify: `apps/desktop/src-tauri/src/adapters/claude.rs`
- Modify: `apps/desktop/src-tauri/src/adapters/hermes.rs`
- Modify: `apps/desktop/src-tauri/src/protocol/events.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/budget.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/projection.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/loop.rs`
- Modify: `apps/desktop/src/lib/protocol.ts`
- Modify: `apps/desktop/src/store/runtime-projections.ts`
- Modify: `apps/desktop/src/components/messages/AgentA2ATimeline.tsx`
- Modify: `apps/desktop/src/lib/loopRuntime.ts`
- Modify tests near existing adapter usage tests, `apps/desktop/src/lib/loopRuntime.test.ts`, `apps/desktop/src/store/blocks.test.ts`, `apps/desktop/src/store/event-dispatch.test.ts`, and `apps/desktop/e2e/acceptance.spec.ts`

**GitNexus impact requirements:**

```text
gitnexus_impact(repo: "forge", target: "AiAdapter", file_path: "apps/desktop/src-tauri/src/adapters/base.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "stream_message_with_emitter", file_path: "apps/desktop/src-tauri/src/adapters/base.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "UsageEvent", file_path: "apps/desktop/src-tauri/src/loop_runtime/budget.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "LoopUsageLedger", file_path: "apps/desktop/src-tauri/src/loop_runtime/budget.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "StreamEvent", file_path: "apps/desktop/src-tauri/src/protocol/events.rs", direction: "upstream", summaryOnly: true)
```

**Implementation steps:**

- [x] **Step 2.1: Inventory current usage emission**

  Run:

  ```bash
  rg -n "usage_emitter|StreamEvent::Usage|estimated_cost|input_tokens|output_tokens|LoopUsageLedger|UsageEvent|unknown" apps/desktop/src-tauri/src/adapters apps/desktop/src-tauri/src/agent apps/desktop/src-tauri/src/loop_runtime apps/desktop/src-tauri/src/protocol
  ```

  Expected: Anthropic has partial known usage emission; `LoopUsageLedger` already preserves unknown input/output/cost; other adapters may need explicit unknown events.

- [x] **Step 2.2: Write Rust tests for known and unknown adapter usage**

  Add focused tests that prove:

  - an adapter response with usage emits known input/output tokens and estimated cost when pricing is available,
  - an adapter response without usage emits a usage fact with `has_unknown_input_tokens`, `has_unknown_output_tokens`, and `has_unknown_cost`,
  - unknown cost is not serialized as `0`,
  - loop budget projection preserves known and unknown usage separately.

  Run:

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::budget --lib
  ```

  Expected first result: FAIL for missing cross-adapter unknown usage behavior.

- [x] **Step 2.3: Normalize the usage event shape**

  Add or refine a shared usage telemetry structure in `adapters/base.rs` or `loop_runtime/budget.rs` so adapters can produce:

  ```text
  model
  input_tokens: known or unknown
  output_tokens: known or unknown
  estimated_cost_micros: known or unknown
  source: adapter id
  reason: provider_reported | provider_omitted | pricing_unknown
  ```

  If the existing top-level `usage` stream event remains direct-session-only, add runtime usage facts through `subagent_runtime_event` or loop projection without breaking the older event. Keep TypeScript mirrors exact.

- [x] **Step 2.4: Wire active adapters through the normalized path**

  For adapters that cannot supply usage, emit explicit unknown usage at model-call completion. For adapters that can supply usage, emit known token counts and unknown cost only when pricing is not available.

- [x] **Step 2.5: Extend frontend projection and UI tests**

  Update `runtime-projections.ts`, `loopRuntime.ts`, and `AgentA2ATimeline.tsx` so known values and unknown flags render distinctly. Add tests proving:

  - known token/cost rows show numeric facts,
  - unknown output/cost rows say unknown rather than zero,
  - direct-session `provider_usage` appends a completed block for known, provider-omitted, and pricing-unknown payloads,
  - additive `provider_usage` does not double-count legacy `usage` cost/context state,
  - runtime usage facts expose structured model/source/reason and explicit unknown flags,
  - runtime facts remain session-scoped.

  Run:

  ```bash
  node --test apps/desktop/src/lib/loopRuntime.test.ts
  node --test apps/desktop/src/store/blocks.test.ts
  node --test apps/desktop/src/store/event-dispatch.test.ts
  npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
  ```

- [ ] **Step 2.6: Run focused verification**

  Run:

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::budget --lib
  node --test apps/desktop/src/lib/loopRuntime.test.ts
  node --test apps/desktop/src/store/blocks.test.ts
  node --test apps/desktop/src/store/event-dispatch.test.ts
  npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
  node scripts/acceptance.test.mjs
  scripts/acceptance.sh --dry-run
  git diff --check
  ```

  Expected: all pass.

**Acceptance command:** `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib`

**Expected commit message:** `feat(runtime): normalize provider usage telemetry`

**Not claimed by this task:**

- No exact cost when the adapter omits usage or pricing is unknown.
- No billing-grade accounting.
- No gateway autonomous resume.
- No auto commit/merge/push.

---

### Task 3: Shell/Internal File-Effect Evidence Strategy

**Goal:** Add bounded post-shell file-effect evidence after `run_shell` completes, while keeping the current no-claim boundary for shell-internal tracing.

**Status (2026-06-18): Implemented.** Forge now records bounded post-shell file-effect evidence for shell commands after `run_shell` completes. The runtime captures git-aware before/after workspace deltas for shell execution, emits those observations as `source: "post_shell_delta"` facts attached to the originating shell block, and preserves the existing no-claim boundary that shell writes are not live shell-internal `file_io` traces. Git snapshot commands are time-bounded; if the snapshot is unavailable or times out, Forge records an `unknown_boundary` sentinel (`[git_snapshot_unavailable]`) instead of blocking or overclaiming exact paths. Non-git workspaces keep an explicit unknown-boundary behavior instead of attempting full workspace enumeration.

**Files:**
- Modify: `apps/desktop/src-tauri/src/executor/mod.rs`
- Modify: `apps/desktop/src-tauri/src/executor/shell.rs`
- Modify: `apps/desktop/src-tauri/src/executor/executor_test.rs`
- Modify: `apps/desktop/src-tauri/src/protocol/events.rs`
- Modify: `apps/desktop/src/lib/protocol.ts`
- Modify: `apps/desktop/src/store/blocks.ts`
- Modify: `apps/desktop/src/store/blocks.test.ts`
- Modify: `apps/desktop/src/components/messages/ShellCard.tsx` or the shell detail presentation files only if UI text is required.
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

**Implemented summary:**

- Preserved the shell-internal no-claim test boundary: shell writes do not emit live `source: "shell_internal"` or executor direct file IO facts.
- Added post-shell delta capture around shell execution using git-aware workspace evidence where available:

  ```bash
  git status --porcelain=v1 -z
  git diff --name-only -z
  git ls-files --others --exclude-standard -z
  ```

- Time-bounded the git snapshot commands so unavailable or timed-out snapshots emit an `unknown_boundary` sentinel (`[git_snapshot_unavailable]`) instead of blocking the loop or claiming exact changed paths.
- Attached `source: "post_shell_delta"` facts to the matching shell block metadata in the frontend projection without creating standalone transcript blocks.
- Added acceptance coverage for the post-shell file-effect evidence smoke path and kept the dry-run script aligned with the advertised checks.

**Evidence/tests:**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml shell_file_effect --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml process_runner --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml executor_file_io_stream --lib
node --test apps/desktop/src/store/blocks.test.ts
node scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
```

**Acceptance command:** `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml shell_file_effect --lib`

**Expected commit message:** `feat(runtime): record post-shell file effect evidence`

**Not claimed by this task:**

- No shell-internal tracing.
- No per-process syscall/file descriptor tracing.
- No guarantee that non-git workspace effects are fully enumerated.
- No full non-git workspace enumeration.
- No gateway autonomous resume.
- No auto commit/merge/push.

---

### Task 4: Gateway Resume Split: Approval Contract, Eligibility Dry Run, Real Owner

**Goal:** Keep gateway autonomous continuation disabled while splitting the oversized headless resume work into a durable approval contract, a derived-only readiness/status dry run, and a separately designed real headless owner.

**Task 4A: Durable Approval Contract (landed 2026-06-18).**

- **Status:** implemented only the disabled-by-default approval intent contract.
- **What changed:** added `loop_runtime::headless` contract types (`HeadlessResumeMode`, `HeadlessResumeApproval`, `HeadlessAgentLease`), durable `headless_resume_approval_recorded`, projection-visible approval state with serde defaults for old task records, and gateway `request_headless_resume`.
- **Why it matters:** Forge can persist and replay human approval intent for future headless ownership without pretending the gateway can resume work today. Missing approval appends no event. Explicit approval is durable and idempotent, but responses still report `gateway_can_resume: false`.
- **Runner behavior:** pending tasks still stop at `waiting_for_input`; no headless `AgentSession` is created with or without approval.
- **Evidence/tests:**

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::runner --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::projection --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::types --lib
  ```

**Task 4B: Headless Resume Eligibility And Lease-Pending Dry Run (landed 2026-06-19).**

- **Status:** implemented and committed in `aa9fd74e feat(runtime): surface headless resume readiness`.
- **What changed:** added a derived `HeadlessResumeReadiness` helper, made runner waiting reasons distinguish desktop owner required, approval required, approval recorded lease pending, and expired approval, surfaced a frontend derived readiness row, and added e2e coverage.
- **Runner boundary:** the runner still records and serves `waiting_for_input`; readiness is display/preflight context only.
- **Gateway boundary:** `gateway_can_resume` remains false and no automatic gateway resume was introduced.
- **Evidence/tests run by controller:**

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::runner --lib # passed 6/6
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_resume --lib # passed 12/12
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib # passed 146/146
  node --test apps/desktop/src/lib/loopRuntime.test.ts # passed 16/16
  node --test apps/desktop/src/lib/backgroundTaskStatus.test.ts # passed 6/6
  npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts # passed 8/8
  scripts/acceptance.sh --dry-run # passed
  git diff --check # passed
  ```

- **GitNexus staged detect:** LOW risk, affected_processes 0.

**Not claimed by Task 4B:**

- No automatic gateway resume.
- No `gateway_can_resume: true`.
- No real headless `AgentSession`.
- No eval_headless execution, model call, or file side effect.
- No durable readiness event.
- No auto commit/merge/push.

**Task 4C: Design Gate / Real Headless Owner Roadmap (next, not implemented).**

Task 4C is a design gate before any real headless owner work. It must not be described as implemented until a later code slice proves the contract, idempotency, orchestration, and adapter behavior with focused tests and acceptance evidence.

**4C.0 status (2026-06-19): DESIGN ONLY / NOT IMPLEMENTED.** The 4C.0 design gate has been drafted in `docs/superpowers/plans/2026-06-19-level-3-headless-owner-design-gate.md`. It captures the fresh GitNexus CRITICAL/HIGH risk scan, proposes the `HeadlessOwnerRun` ownership contract, records stop lines, and still blocks runtime implementation until explicit user confirmation for HIGH/CRITICAL code. Task 4C is not implemented by this documentation update.

**Risk scan to carry into 4C design:**

- GitNexus impact for `AgentSession` in `apps/desktop/src-tauri/src/agent/session/mod.rs` is **CRITICAL**: impactedCount 48. Affected processes include `send_input`, `create_session`, and `run_request`; affected modules include IPC, Agent, Eval_headless, A2A, and Session.
- GitNexus impact for gateway `dispatch` in `apps/desktop/src-tauri/src/gateway/server.rs` is **CRITICAL**: impactedCount 45, direct 39. Affected processes include `dispatch_dashboard_snapshot_returns_dashboard_operational_summary`, `evaluate_loop_task_completion_uses_projected_evidence`, and `evaluate_loop_task_completion_returns_typed_result`.
- GitNexus impact for `handle_request_headless_resume` in `apps/desktop/src-tauri/src/gateway/server.rs` is **HIGH**: impactedCount 42, direct 1. Affected processes include the same gateway completion/dashboard flows.
- GitNexus impact for `eval_headless::run_request` in `apps/desktop/src-tauri/src/eval_headless/mod.rs` is LOW direct 2, but that path creates a new `AgentSession`, snapshots the workspace, sends model turns, and validates/repairs output. Using it for 4C still requires an explicit production policy and side-effect boundary.

**4C.0 Design Gate And Stop Lines**

- Before any code edit, rerun GitNexus impact on the exact target symbols and report direct callers, affected processes, and risk level to the user.
- Any **CRITICAL** or **HIGH** result blocks implementation until the user explicitly confirms the narrowed scope.
- The first 4C artifact is a written ownership design, not runtime code. The design must state which ledgers, snapshots, leases, policy gates, cancellation paths, and UI surfaces are allowed to observe or mutate ownership state.
- Stop if the proposal depends on making `gateway_can_resume: true` by default, bypassing human approval, or treating Task 4B readiness as execution authorization.

**4C.1 Ownership Contract Before Execution**

- Define the durable contract before execution logic: `HeadlessOwnerRun`, lease id, attempt id, `snapshot_source`, `human_gate_id`, `budget_snapshot_id`, `policy_decision_id`, and `idempotency_key`.
- Add contract tests first in the eventual implementation slice. Tests must prove serialization/backcompat defaults, projection visibility, and rejected/missing approval behavior before any runner or gateway dispatch changes.
- Approval/readiness is not execution authorization. The contract must bind a specific owner attempt to a specific human gate, policy decision, snapshot source, and budget snapshot.

**4C.2 Ledger Projection And Idempotency**

- Add events, projection, and replay tests before orchestration. Duplicate `request_headless_resume` calls with the same `idempotency_key` must not create duplicate owner runs, leases, model calls, tool calls, or file side effects.
- Replayed ledgers must reconstruct the same owner state after restart, including waiting, interrupted, cancelled, expired lease, and denied policy states.
- Commit remains a human gate and must not be satisfied or attempted by headless ownership events.

**4C.3 Coordinator Dry Run**

- Implement only acquisition plus immediate `waiting_for_input` or `interrupted` states. This slice must not call a model provider, mutate files, invoke tools, auto-accept confirmations, or create a production `AgentSession`.
- The dry run should prove lease acquisition/release, heartbeat/expiry projection, cancellation, budget preflight denial, and dashboard/status visibility.
- The default remains disabled. A disabled or denied policy path must be the ordinary outcome until explicit test fixtures opt in.

**4C.4 Fake Executor Acceptance**

- Use a fake executor to verify resume orchestration without connecting to a real provider or `eval_headless`.
- Acceptance should prove the coordinator can move through acquired, running, waiting, interrupted, cancelled, expired, and completed fake states while preserving idempotency and lease ownership.
- The fake executor must surface pending confirmations/tool calls as blockers; it must not auto-accept them.

**4C.5 Real AgentSession Adapter Behind Policy**

- Only after the contract, projection, dry run, and fake executor are proven may 4C consider a real `AgentSession` adapter behind explicit policy.
- Required adapter gates: pending confirmations/tool calls, cancellation, snapshot restore, lease heartbeat/expiry, budget preflight, provider/model/profile resolution, failure evidence, and no auto commit.
- `eval_headless` is not a drop-in production runtime owner. It may inform implementation, but direct reuse must first pass the explicit policy and side-effect boundary described above.

**4C.6 Product Evidence And Obsidian Narrative**

- Every 4C milestone must update the repo plan and Obsidian mirror in the same change set, including commit hash, focused tests, acceptance command, GitNexus risk summary, and "not claimed" bullets.
- Do not claim shell-internal tracing, billing-grade cost, automatic resume, or true runtime ownership until the relevant acceptance evidence exists.
- Docs evidence commit `1d5df5d4` records the Task 4B evidence baseline; 4C evidence starts after that baseline and must not rewrite 4A/4B claims.

**MVP boundaries for Task 4C:**

- Default remains disabled.
- Approval/readiness is not execution authorization.
- Commit is always a human gate.
- Pending confirmations/tool calls are never auto-accepted.
- `eval_headless` is not treated as a directly reusable production runtime owner.
- Shell-internal tracing is not claimed.
- Billing-grade cost is not claimed.
- No automatic gateway resume, model call, file side effect, or real headless `AgentSession` exists until a later implemented slice proves it.

**Chinese product explanation:**

下一阶段不是让 agent 自动乱跑，而是先做一个可审计的接管协议。Forge 要先证明它知道谁授权、从哪个快照恢复、拿了哪个租约、预算是否允许、失败/取消怎么记录；这些都可回放、可去重、可被人审计之后，才考虑让真正的 `AgentSession` 在明确策略后接管执行。

---

### Task 5: A2A Lineage Completion

**Goal:** Complete durable A2A lineage by adding parent-session structs, persisted parent-side child arrays, and automatic parent selection for ordinary delegates when a real parent context exists.

**Status (2026-06-18): Implemented.** Before this task, child lineage was durable only on the child record (`parent_task_id`) and parent `child_task_ids` were rebuilt in projection by scanning children. The A2A bus now stores parent-side `child_task_ids` directly on `AgentTaskRecord`, writes both sides during `assign_child_task`, and keeps projection compatible with old ledgers by appending legacy-derived child ids when a parent lacks a persisted array. `AgentParentSessionContext` is stored in A2A durable state with `parent_session_id`, `active_parent_task_id`, `root_task_id`, `selection_reason`, and `updated_at_ms`, so snapshots and sidecar ledgers can carry active parent-selection context.

**What changed:** `assign_child_task` is now an all-or-nothing mutation: missing parents leave the bus unchanged, successful child assignment sets the child's `parent_task_id`, and the parent records the child id idempotently. Projection prefers persisted child order, deduplicates ids, and appends legacy child records that still only have `parent_task_id`. `delegate_task` without an explicit `parent_task_id` now chooses the active parent only when the current `AgentSession.id` matches the stored parent-session context, the active parent exists in the current bus, and the delegate input is not marked as a root planning task. Root-planning delegates can opt out explicitly with the schema-visible optional boolean `root_planning_task: true`; when no valid context exists, no-parent delegates remain root A2A tasks.

**Why it matters:** A parent task can now survive serialization, sidecar replay, and session snapshot restore with its own durable child array instead of relying on the UI/read model to reconstruct lineage. That makes parent ownership auditable before later gateway resume/review flows depend on parent-child task boundaries.

**Evidence/tests (fresh during implementation):**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::assign_child_task_persists_parent_child_task_ids --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_task_id_survives_bus_serialization_roundtrip --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_child_task_ids_survive_bus_serialization_roundtrip --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::ledger::tests::ledger_roundtrips_parent_child_task_ids --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::a2a::tests::snapshot_restore_preserves_a2a_parent_child_task_ids --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml delegate_task_schema --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::a2a --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session --lib
node --test apps/desktop/src/store/workbenchSummary.test.ts
npm --prefix apps/desktop run test:e2e -- e2e/a2a-confirm-runtime.spec.ts
git diff --check
```

**Not claimed:** automatic creation of parent-session context, fuzzy title/prompt-based root planning detection, autonomous gateway resume, default headless `AgentSession`, auto commit/merge/push, or any UI redesign. TypeScript protocol, workbench summary defaults, and A2A e2e mocks already carried `parent_task_id` / `child_task_ids`, so no frontend contract change was required; the model-facing Anthropic `delegate_task` schema now advertises `root_planning_task` as an optional boolean.

**Files:**
- Modify: `apps/desktop/src-tauri/src/agent/a2a/types.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/bus.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/projection.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/supervisor.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/ledger.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/mod.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/tools.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/a2a.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session_tests.rs`
- Modify: `apps/desktop/src-tauri/src/agent/snapshot.rs`
- Modify: `apps/desktop/src/lib/protocol.ts`
- Modify: `apps/desktop/src/lib/workbenchSummary.ts`
- Modify: `apps/desktop/src/components/messages/AgentA2ATimeline.tsx`
- Modify: `apps/desktop/e2e/a2a-confirm-runtime.spec.ts`

**GitNexus impact requirements:**

```text
gitnexus_impact(repo: "forge", target: "AgentA2ABus", file_path: "apps/desktop/src-tauri/src/agent/a2a/bus.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "AgentTaskRecord", file_path: "apps/desktop/src-tauri/src/agent/a2a/types.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "assign_child_task", file_path: "apps/desktop/src-tauri/src/agent/a2a/bus.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "execute_tools", file_path: "apps/desktop/src-tauri/src/agent/session/tools.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "delegate_parent_task_id_from_input", file_path: "apps/desktop/src-tauri/src/agent/session/tools.rs", direction: "upstream", summaryOnly: true)
```

**Implementation steps:**

- [x] **Step 5.1: Add persistence tests for parent-side child arrays**

  Tests must prove:

  - assigning a child updates `parent_task_id` on the child and a durable `child_task_ids` array on the parent,
  - child arrays survive bus serialization, A2A sidecar load, and `AgentSession` snapshot restore,
  - legacy ledgers without `child_task_ids` deserialize with an empty array,
  - duplicate child ids are not recorded twice.

  Run:

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::assign_child_task_persists_parent_child_task_ids --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_task_id_survives_bus_serialization_roundtrip --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_child_task_ids_survive_bus_serialization_roundtrip --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::ledger::tests::ledger_roundtrips_parent_child_task_ids --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::a2a::tests::snapshot_restore_preserves_a2a_parent_child_task_ids --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::a2a --lib
  ```

  Expected first result: FAIL because parent-side child arrays are currently projection-derived, not persisted.

- [x] **Step 5.2: Add parent-session structs**

  Add a compact parent session structure that records:

  ```text
  parent_session_id
  active_parent_task_id
  root_task_id
  selection_reason
  updated_at_ms
  ```

  Persist it through `AgentSession` snapshot and the A2A sidecar if it is part of A2A durable state.

- [x] **Step 5.3: Implement durable parent-side arrays**

  Extend `AgentTaskRecord` with `child_task_ids` using serde defaults for old records. Update `assign_child_task` to mutate both sides atomically within the bus update path. Projection should read the persisted array and may still rebuild missing legacy arrays for compatibility when a child has `parent_task_id`.

- [x] **Step 5.4: Implement automatic parent selection for ordinary delegates**

  In `agent/session/tools.rs`, when `delegate_task` has no explicit `parent_task_id`, choose the active parent task only if:

  - the current `AgentSession` has a parent-session context,
  - the active parent id exists in the current bus,
  - the selected parent belongs to the same session,
  - the delegate is not itself a root planning task.

  If no valid active parent exists, keep the current root assignment behavior.

- [x] **Step 5.5: Update UI and acceptance mocks**

  TypeScript protocol, workbench summary normalization, and the A2A mocked e2e already carried `parent_task_id` / `child_task_ids`, so this implementation kept visible UI behavior unchanged and verified those existing mocks/tests instead of adding frontend churn.

- [x] **Step 5.6: Run focused verification**

  Run:

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session --lib
  node --test apps/desktop/src/store/workbenchSummary.test.ts
  npm --prefix apps/desktop run test:e2e -- e2e/a2a-confirm-runtime.spec.ts
  git diff --check
  ```

  Expected: all pass.

**Acceptance command:** `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib`

**Expected commit message:** `feat(agent): complete a2a parent lineage ownership`

**Not claimed by this task:**

- No cross-session or cross-repo global agent graph.
- No automatic parent selection when there is no real active parent context.
- No autonomous gateway resume.
- No auto commit/merge/push.

---

### Task 6: Completion And Review-To-Commit Workflow Hardening

**Goal:** Make completion, review, and commit eligibility auditable without letting the runtime commit, merge, or push.

**Status (2026-06-18): Committed in `33bee230` (`feat(runtime): harden review to commit eligibility`) with docs evidence captured in `4ccdf702` (`docs(runtime): record task 6 evidence`).** Completion evaluation now emits explicit `review_status`, `commit_eligible`, `commit_blockers`, `human_gate_id`, and `last_review_decision` facts with serde defaults for older records. A satisfied build/test/docs contract can become `ready_for_review` when review is required; no-review contracts report `not_required` instead of inventing a human-review blocker. Missing or rejected human review keeps `commit_eligible` false when review is required and records the blocker/rejection reason. Commit evidence is only accepted as contract evidence when it references an approved human gate, and runtime policy keeps commit intent human-gated even if eligibility facts are true.

**Quality follow-up (2026-06-18):** `LoopEventJournal::new(path)` now shares a process-wide mutex per normalized path so separate journal instances serialize load/prepare/append on the same JSONL file. A2A review bridge idempotency keys and review evidence ids now use stable semantic keys based on loop task, gate, decision, and reason instead of timestamps, so exact retries coalesce. The loop-evidence bridge is explicit best-effort: if loop journal/projection writes fail after the A2A review state is saved, Forge logs a warning and still returns the A2A review state. Completion facts now require no remaining open gates for `commit_eligible`, and `commit_blockers` includes open-gate blockers plus required-commit blockers such as `missing_commit`, `commit_missing_human_gate`, and `commit_without_approved_human_gate:*`.

**What changed:** A2A review IPC now accepts an optional `loop_task_id` / `loopTaskId`. The frontend only sends it when the selected reviewed A2A task(s) have the same non-null runtime `loop_task_id`; the Rust bridge records loop review gate/evidence only when that loop task exists and its `session_id` matches the reviewed A2A session. Mismatched or missing loop ids do not write broad loop evidence. `LoopTaskPanel`, Agent Workbench review calls, TypeScript protocol/summary helpers, and the acceptance mock now surface `ready for human review`, `blocked by review`, `commit eligible after human review`, and `commit remains human-gated` without adding commit/merge/push controls. The acceptance mock uses a completed loop task with an approved review and commit eligibility, while ordinary terminal loop tasks remain hidden from background work.

**Why it matters:** Forge can now explain the handoff from completion evidence to human review and commit readiness without pretending the runtime is allowed to commit. Review approval satisfies a configured gate; it is not a correctness guarantee and it is not an automatic Git operation.

**Review and change detection:** The spec reviewer and quality reviewer approved after the follow-up fix loops. GitNexus `detect_changes` returned **CRITICAL** because Task 6 intentionally touched a broad runtime/IPC/UI surface; the staged changed files were expected for the Task 6 scope and excluded unrelated metadata and roadmap files.

**Evidence/tests:**

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::journal --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::gates --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::policy --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml ipc::a2a_handlers --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib
node --test apps/desktop/src/lib/loopRuntime.test.ts
node --test apps/desktop/src/lib/backgroundTaskStatus.test.ts
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
git diff --check
```

**Not claimed:** No auto commit/merge/push, no worktree merge automation, no gateway autonomous resume, no headless `AgentSession` ownership change, and no correctness guarantee from review approval.

**Interview-ready explanation:** "Forge does not let the runtime commit. It computes auditable eligibility: which checks passed, whether docs evidence exists, whether a human review gate was approved or rejected, and whether a commit record is tied to that human gate. A2A review decisions only enter the loop ledger when they carry a matching loop task id for the same session, so a completed worker cannot accidentally imply approval for a parent loop."

**Files:**
- Modify: `apps/desktop/src-tauri/src/loop_runtime/completion.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/gates.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/policy.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/types.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/projection.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/protocol.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/server.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/review_gate.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/a2a_handlers.rs`
- Modify: `apps/desktop/src/lib/loopRuntime.ts`
- Modify: `apps/desktop/src/components/loop/LoopTaskPanel.tsx`
- Modify: `apps/desktop/src/components/messages/AgentA2ATimeline.tsx`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`

**GitNexus impact requirements:**

```text
gitnexus_impact(repo: "forge", target: "evaluate_completion", file_path: "apps/desktop/src-tauri/src/loop_runtime/completion.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "HumanGateDecision", file_path: "apps/desktop/src-tauri/src/loop_runtime/gates.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "review_agent_a2a_tasks", file_path: "apps/desktop/src-tauri/src/ipc/a2a_handlers.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "LoopTaskPanel", file_path: "apps/desktop/src/components/loop/LoopTaskPanel.tsx", direction: "upstream", summaryOnly: true)
```

**Implementation steps:**

- [x] **Step 6.1: Write completion blocker tests**

  Tests must prove:

  - a satisfied build/test/docs contract can make a task `ready_for_review`,
  - missing human review keeps `commit_eligible` false,
  - rejected review keeps `commit_eligible` false and records the rejection reason,
  - commit evidence cannot satisfy the contract unless it references a closed human gate,
  - no runtime function shells out to `git commit`, `git merge`, or `git push`.

  Run:

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::gates --lib
  ```

  Expected first result: FAIL for missing commit eligibility shape.

- [x] **Step 6.2: Add explicit commit eligibility facts**

  Add projection fields or evidence records for:

  ```text
  review_status
  commit_eligible
  commit_blockers
  human_gate_id
  last_review_decision
  ```

  These facts describe readiness only. They must not contain a command executor for commits.

- [x] **Step 6.3: Bridge A2A review decisions into loop completion**

  When an A2A review decision changes, record loop evidence or gate decisions only for the matching loop task/session. Do not infer approval from a completed worker without an explicit review decision.

- [x] **Step 6.4: Update UI copy and controls**

  `LoopTaskPanel` and A2A review UI should show:

  - ready for human review,
  - blocked by review,
  - commit eligible after human review,
  - commit remains human-gated.

  Do not add a button that runs commit/merge/push.

- [x] **Step 6.5: Add product smoke coverage**

  Extend `apps/desktop/e2e/acceptance.spec.ts` to assert that a completed loop shows review/commit eligibility but not an automatic commit action.

- [x] **Step 6.6: Run focused verification**

  Run:

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::gates --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib
  node --test apps/desktop/src/lib/loopRuntime.test.ts
  npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
  git diff --check
  ```

  Expected: all pass.

**Acceptance command:** `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib`

**Expected commit message:** `feat(runtime): harden review to commit eligibility`

**Not claimed by this task:**

- No auto commit/merge/push.
- No worktree merge automation.
- No guarantee that review approval means the code is correct; it only satisfies the configured gate.
- No gateway autonomous resume.
- No headless `AgentSession` ownership change.

---

### Task 7: Acceptance Matrix And Obsidian Evidence Requirements

**Goal:** Keep acceptance, repo docs, and the Obsidian narrative aligned with the new runtime ownership claims.

**Pre-commit review evidence captured during Task 7 implementation (2026-06-19):** `scripts/acceptance.sh` dry-run now groups the ownership evidence after the existing MVP/runtime gates and before desktop smoke. The new/renamed gates are `mocked desktop restart runtime smoke (partial macOS evidence)`, `provider usage known/unknown telemetry`, `post-shell file-effect evidence smoke (bounded, not shell-internal)`, `persisted A2A lineage tests`, `typed completion evidence and review-to-commit eligibility tests`, and `gated headless ownership policy tests`.

**What changed:** `scripts/acceptance.test.mjs` first failed against the old matrix, then was updated to parse dry-run output into ordered `[label, command]` entries and assert exact labels, commands, order, and ownership command uniqueness. The script now advertises concrete commands for each claim: restart mocked smoke, provider usage/unknown pricing, post-shell deltas, focused A2A parent lineage filters, completion/review-to-commit, and Task 4A headless approval policy.

**Why it matters:** The dry-run is the interview/backing table for runtime ownership. It prevents docs from saying "owned" when the acceptance script only proves older MVP gates, and it keeps the headless claim limited to policy/approval evidence instead of real autonomous execution.

**Task 7 commit:** `0d9f5670 test(runtime): extend level 3 ownership acceptance gates`.

**Verification captured during Task 7 implementation:**

```bash
node scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
rg -n "auto commit|shell-internal|Phase 4-K|b959091b|review-to-commit|mocked desktop restart|provider usage|post-shell|persisted A2A lineage|headless" docs/superpowers/plans/2026-06-18-level-3-agent-loop-next-stage-runtime-ownership.md "/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Level 3 Agent Loop Next Stage Runtime Ownership Plan.md"
git diff --check
```

**Not claimed:** Task 7 does not create runtime behavior; docs and labels do not prove a claim unless their commands exist and pass; commit remains human-gated; shell-internal tracing is not claimed; unknown provider token/cost remains unknown when adapters omit usage; gateway autonomous resume requires explicit policy and human approval; Task 4A still records approval intent and does not create a real headless `AgentSession`.

**Interview-ready explanation:** Task 7 is the evidence alignment pass. Forge's runtime ownership claim now points to the exact acceptance gates that prove each bounded fact, while the narrative stays honest about the parts that are still human-controlled or future work.

**Files:**
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
- Modify: `apps/desktop/e2e/a2a-confirm-runtime.spec.ts`
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/superpowers/plans/2026-06-16-level-3-agent-loop-runtime.md`
- Modify: `docs/superpowers/plans/2026-06-18-level-3-agent-loop-next-stage-runtime-ownership.md`
- Modify: `/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Level 3 Agent Loop Next Stage Runtime Ownership Plan.md`

**GitNexus impact requirements:**

Docs-only edits do not require symbol impact. For Playwright fixture/helper-only changes, use `rg` and direct file review; GitNexus may not index e2e fixtures. If acceptance scripts or frontend runtime helpers change product code, run impact for the touched helper symbols before editing them.

```text
gitnexus_impact(repo: "forge", target: "summarizeLoopTask", file_path: "apps/desktop/src/lib/loopRuntime.ts", direction: "upstream", summaryOnly: true)
```

**Implementation steps:**

- [ ] **Step 7.1: Extend the acceptance matrix**

  Update `scripts/acceptance.sh` so the dry-run matrix includes, in order:

  - existing Level 3 MVP gates,
  - mocked desktop restart runtime smoke as partial evidence on macOS until true Tauri/WebDriver restart exists in Windows/Linux CI or another supported platform gate,
  - provider usage known/unknown telemetry,
  - post-shell file-effect evidence,
  - persisted A2A lineage,
  - typed completion evidence and review-to-commit eligibility,
  - gated headless ownership policy if Task 4 landed.

- [ ] **Step 7.2: Add acceptance script contract tests**

  Extend `scripts/acceptance.test.mjs` to assert every new label and command appears in `--dry-run`.

  Run:

  ```bash
  node scripts/acceptance.test.mjs
  scripts/acceptance.sh --dry-run
  ```

  Expected: both pass and list the new commands.

- [ ] **Step 7.3: Update product docs**

  Update `README.md`, `apps/desktop/README.md`, and `CHANGELOG.md` only for user-visible runtime surfaces that landed. Each doc update must include the boundary language:

  ```text
  commit remains human-gated
  shell-internal tracing is not claimed
  unknown provider token/cost remains unknown when adapters omit usage
  gateway autonomous resume requires explicit policy and human approval
  ```

- [ ] **Step 7.4: Update repo and Obsidian evidence**

  After each task commit, run:

  ```bash
  git rev-parse --short HEAD
  ```

  Add the resulting commit hash, focused tests, acceptance command, and not-claimed bullets to both the repo plan and Obsidian mirror. If a task is not implemented, describe it as pending future work with the exact acceptance command that will prove it; do not present it as current capability.

- [ ] **Step 7.5: Run final docs verification**

  Run:

  ```bash
  rg -n "auto commit|shell-internal|Phase 4-K|b959091b|review-to-commit|mocked desktop restart|provider usage|post-shell|persisted A2A lineage|headless" docs/superpowers/plans/2026-06-18-level-3-agent-loop-next-stage-runtime-ownership.md "/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Level 3 Agent Loop Next Stage Runtime Ownership Plan.md"
  git diff --check
  ```

  Expected: the search returns the important boundary/evidence lines, a separate placeholder red-flag scan returns no matches, and `git diff --check` passes.

**Acceptance command:** `scripts/acceptance.sh --dry-run`

**Expected commit message:** `test(runtime): extend level 3 ownership acceptance gates`

**Not claimed by this task:**

- Docs do not create runtime capability.
- Acceptance labels do not prove capabilities unless the listed commands exist and pass.
- No auto commit/merge/push.

---

## Acceptance Matrix

| Capability | Minimum proof | Command |
| --- | --- | --- |
| Durable ledger/replay remains intact | Existing journal and projection tests | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::journal --lib` and `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::replay_tests --lib` |
| Runner lease/waiting boundary remains intact | Runner/gateway tests | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::runner --lib` |
| Mocked desktop restart runtime ownership | Partial mocked desktop restart smoke on macOS; official Tauri/WebDriver force-quit/reopen is `BLOCKED_OFFICIAL_MACOS` and moves to future Windows/Linux platform-gated proof | `npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts` |
| Known/unknown provider usage | Adapter and budget tests, including provider omission and unknown pricing | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib` and `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml unknown_pricing --lib` |
| Direct ToolExecutor file IO | Existing Phase 4-K executor stream tests | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml executor_file_io_stream --lib` |
| Post-shell delta evidence | Shell file-effect tests | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml shell_file_effect --lib` |
| A2A child file-ish facts | Existing child runtime bridge tests | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::child --lib` |
| A2A persisted lineage | A2A/session lineage tests focused by parent/child state | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::assign_child_task_persists_parent_child_task_ids --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_task_id_survives_bus_serialization_roundtrip --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::bus::tests::parent_child_task_ids_survive_bus_serialization_roundtrip --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::ledger::tests::ledger_roundtrips_parent_child_task_ids --lib && cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::a2a::tests::snapshot_restore_preserves_a2a_parent_child_task_ids --lib` |
| Completion/review/commit eligibility | Completion/gate tests and e2e smoke | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib` |
| Gated headless ownership policy | Task 4A approval/status contract plus Task 4B derived readiness display; no real headless `AgentSession` execution | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_resume --lib` |
| Product visibility | Acceptance specs | `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts e2e/a2a-confirm-runtime.spec.ts` |
| Full advertised suite | Acceptance script | `scripts/acceptance.sh` |

## Obsidian Evidence Requirements

The Obsidian mirror must stay narrative, but every implemented slice needs these concrete facts:

- current state before the slice,
- commit hash from `git rev-parse --short HEAD`,
- focused tests run,
- acceptance command added or preserved,
- what the slice proves,
- what the slice does not claim,
- one interview-ready paragraph explaining why the boundary matters.

Do not describe unimplemented autonomous resume, shell-internal tracing, or precise provider cost as current capability.

## Interview And Backing Narrative

When this next stage is complete, Forge can be described as a Level 3 runtime with durable ownership rather than just a desktop agent cockpit. The gateway has a replayable ledger, lease state, policy and budget facts, review gates, completion decisions, and controlled ownership of loop tasks. The desktop and dashboard consume runtime facts rather than transcript guesses. A reviewer can inspect what ran, what changed, what usage was known or unknown, which A2A child belonged to which parent, which evidence satisfied the completion contract, and why the runtime stopped.

The important backing claim is not "Forge lets agents run wild." The claim is that Forge can prove what it allowed, what it observed, where it stopped, and which human gate is still required. Phase 4-K `b959091b` closed direct ToolExecutor file IO visibility; this next stage closes restart proof, provider usage honesty, post-shell evidence, lineage ownership, and review-to-commit control without pretending shell-internal tracing or auto commit exists.

## Stop Conditions

Stop and ask the controller before continuing if any of these happen:

- GitNexus impact returns HIGH or CRITICAL for `AgentSession`, gateway dispatch/server, `ToolExecutor`, `StreamEvent`, autosave classification, `AgentA2ABus`, or A2A session paths and the change cannot be narrowed.
- A task requires runtime code outside `apps/desktop` or shared package extraction.
- A proposed change would run `git commit`, merge worktrees, push branches, or hide commit behind an automatic control path.
- A provider cannot supply usage and the implementation would convert unknown tokens/cost to zero.
- Shell file evidence requires claiming shell-internal tracing before tests prove it.
- Restart harness behavior is flaky twice after isolation; split the test into a deterministic mocked product smoke plus a separate true process harness rather than weakening the claim.
- Acceptance script labels a capability that has no command.
- Any docs change says a future capability is current.

## Final Gate Commands

Run these before marking the stage complete:

```bash
git status --short
npm run build:desktop
npm run build:website
npm run test:eval
scripts/acceptance.sh --dry-run
scripts/acceptance.sh
git diff --check
```

Then run:

```text
gitnexus_detect_changes(repo: "forge", scope: "all")
```

Expected final state: only planned files changed, acceptance gates pass, GitNexus affected flows match the implemented tasks, docs and Obsidian mirror contain the same capability boundaries, and no stage performs auto commit/merge/push.
