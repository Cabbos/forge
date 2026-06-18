# Level 3 Agent Loop Next Stage Runtime Ownership Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the next Level 3 hardening stage by proving Forge can own loop runtime state across restarts, emit honest usage and file-effect evidence, preserve A2A lineage, gate autonomous gateway ownership, and keep commit as a human decision.

**Architecture:** Build on the committed Level 3 MVP instead of replacing it. The gateway-owned loop ledger, runner lease state, runtime event protocol, A2A bus, ToolExecutor `file_io` stream, desktop runtime projections, and acceptance script remain the substrate; this stage adds restart harness evidence, normalized usage telemetry, bounded shell delta evidence, gated headless ownership policy, durable parent-side lineage, and review-to-commit eligibility. Autonomous resume is only enabled behind explicit policy and human approval, and the default path continues to stop at review.

**Tech Stack:** Rust/Tauri backend, gateway JSON-RPC and loop runner, `AgentSession`, `ToolExecutor`, `StreamEvent`, existing A2A bus/worktree worker, React/TypeScript protocol/store/UI, Playwright/WebDriver acceptance, `scripts/acceptance.sh`, GitNexus impact analysis.

---

## Current State And Evidence

This plan starts after Phase 4-K was committed as `b959091b feat(runtime): stream executor file io events`.

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
- Runtime facts exist, but shell-internal tracing, full provider usage coverage, automatic parent selection, parent-session structs, persisted parent-side child arrays, and gateway autonomous resume are still unclaimed.

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

1. Task 1 restart harness first, because it defines the external proof for runtime ownership.
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

  Expected: identify whether the existing Playwright suite can force a Tauri/WebDriver app restart directly or whether the first committed slice must wrap the existing launcher with an app-process restart helper. Record the chosen path in the task handoff.

- [ ] **Step 1.2: Write the failing restart smoke**

  Add `apps/desktop/e2e/level3-runtime-restart.spec.ts` with one test that:

  - starts a session with persisted session snapshot state,
  - injects or creates a loop task in `waiting_for_input` with a runner lease history,
  - injects A2A projection state with a retained worktree worker, runtime file facts, and usage facts with unknown cost,
  - force-quits the app process through the available Tauri/WebDriver harness,
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

  Add a label such as `Tauri/WebDriver restart runtime harness` to `scripts/acceptance.sh` and add the exact command:

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

**Acceptance command:** `npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts`

**Expected commit message:** `test(runtime): add level 3 restart harness`

**Not claimed by this task:**

- No gateway autonomous resume after crash.
- No default headless `AgentSession`.
- No shell-internal tracing.
- No auto commit/merge/push.

---

### Task 2: Provider Token/Cost Telemetry Stream With Explicit Unknown Handling

**Goal:** Normalize provider usage telemetry so known adapter usage becomes runtime facts, and unknown tokens/cost remain explicit unknowns across direct sessions, loop tasks, and A2A child runs.

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
- Modify tests near existing adapter usage tests, `apps/desktop/src/lib/loopRuntime.test.ts`, and `apps/desktop/e2e/acceptance.spec.ts`

**GitNexus impact requirements:**

```text
gitnexus_impact(repo: "forge", target: "AiAdapter", file_path: "apps/desktop/src-tauri/src/adapters/base.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "stream_message_with_emitter", file_path: "apps/desktop/src-tauri/src/adapters/base.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "UsageEvent", file_path: "apps/desktop/src-tauri/src/loop_runtime/budget.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "LoopUsageLedger", file_path: "apps/desktop/src-tauri/src/loop_runtime/budget.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "StreamEvent", file_path: "apps/desktop/src-tauri/src/protocol/events.rs", direction: "upstream", summaryOnly: true)
```

**Implementation steps:**

- [ ] **Step 2.1: Inventory current usage emission**

  Run:

  ```bash
  rg -n "usage_emitter|StreamEvent::Usage|estimated_cost|input_tokens|output_tokens|LoopUsageLedger|UsageEvent|unknown" apps/desktop/src-tauri/src/adapters apps/desktop/src-tauri/src/agent apps/desktop/src-tauri/src/loop_runtime apps/desktop/src-tauri/src/protocol
  ```

  Expected: Anthropic has partial known usage emission; `LoopUsageLedger` already preserves unknown input/output/cost; other adapters may need explicit unknown events.

- [ ] **Step 2.2: Write Rust tests for known and unknown adapter usage**

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

- [ ] **Step 2.3: Normalize the usage event shape**

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

- [ ] **Step 2.4: Wire all adapters through the normalized path**

  For adapters that cannot supply usage, emit explicit unknown usage at model-call completion. For adapters that can supply usage, emit known token counts and unknown cost only when pricing is not available.

- [ ] **Step 2.5: Extend frontend projection and UI tests**

  Update `runtime-projections.ts`, `loopRuntime.ts`, and `AgentA2ATimeline.tsx` so known values and unknown flags render distinctly. Add tests proving:

  - known token/cost rows show numeric facts,
  - unknown output/cost rows say unknown rather than zero,
  - runtime facts remain session-scoped.

  Run:

  ```bash
  node --test apps/desktop/src/lib/loopRuntime.test.ts
  npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
  ```

- [ ] **Step 2.6: Run focused verification**

  Run:

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::budget --lib
  node --test apps/desktop/src/lib/loopRuntime.test.ts
  npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
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

**GitNexus impact requirements:**

```text
gitnexus_impact(repo: "forge", target: "ToolExecutor", file_path: "apps/desktop/src-tauri/src/executor/mod.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "execute_with_emitter", file_path: "apps/desktop/src-tauri/src/executor/mod.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "execute_streaming", file_path: "apps/desktop/src-tauri/src/executor/shell.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "StreamEvent", file_path: "apps/desktop/src-tauri/src/protocol/events.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "applyTranscriptEventToBlocks", file_path: "apps/desktop/src/store/blocks.ts", direction: "upstream", summaryOnly: true)
```

**Implementation steps:**

- [ ] **Step 3.1: Preserve the existing shell-internal no-claim test**

  The existing `executor_file_io_stream_run_shell_file_writes_emit_no_file_io` test proves `run_shell` does not emit direct live `file_io` facts. Replace it only with a stricter pair:

  - shell writes do not emit `source: "shell_internal"` or `source: "executor"` direct file IO,
  - shell writes may emit a separate `source: "post_shell_delta"` fact after the command finishes.

- [ ] **Step 3.2: Write failing post-shell delta tests**

  Add tests in `executor_test.rs` proving:

  - `printf 'hello' > shell.txt` emits a post-shell delta fact for `shell.txt` after shell completion,
  - a read-only shell command emits no post-shell delta facts,
  - failed shell commands emit no success delta facts unless an explicit failed-delta record is added with `success: false`,
  - the event carries the shell block id and cannot be attached to another block.

  Run:

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml shell_file_effect --lib
  ```

  Expected first result: FAIL because no post-shell delta capture exists.

- [ ] **Step 3.3: Implement bounded delta capture**

  Capture a before/after worktree snapshot around `run_shell` using git-aware commands when inside a git worktree:

  ```bash
  git status --porcelain=v1 -z
  git diff --name-only -z
  git ls-files --others --exclude-standard -z
  ```

  For non-git workspaces, record a single unknown-boundary fact rather than scanning the whole tree. The event source must be `post_shell_delta`, and docs/UI must describe it as post-shell evidence, not shell-internal tracing.

- [ ] **Step 3.4: Attach post-shell delta facts to shell blocks**

  Update the TypeScript protocol and block projection so post-shell delta facts attach to the matching shell block metadata. Do not create standalone transcript blocks.

- [ ] **Step 3.5: Add acceptance coverage**

  Add an acceptance label such as `post-shell file effect evidence smoke` to `scripts/acceptance.sh` and assert it in `scripts/acceptance.test.mjs`.

- [ ] **Step 3.6: Run focused verification**

  Run:

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml shell_file_effect --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml executor_file_io_stream --lib
  node --test apps/desktop/src/store/blocks.test.ts
  node scripts/acceptance.test.mjs
  scripts/acceptance.sh --dry-run
  git diff --check
  ```

  Expected: all pass.

**Acceptance command:** `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml shell_file_effect --lib`

**Expected commit message:** `feat(runtime): record post-shell file effect evidence`

**Not claimed by this task:**

- No shell-internal tracing.
- No per-process syscall/file descriptor tracing.
- No guarantee that non-git workspace effects are fully enumerated.
- No auto commit/merge/push.

---

### Task 4: Gateway Autonomous Resume And Headless AgentSession Ownership

**Goal:** Introduce a gated ownership path for gateway autonomous resume/headless `AgentSession` without enabling it by default.

**Files:**
- Create: `apps/desktop/src-tauri/src/loop_runtime/headless.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/mod.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/runner.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/policy.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/gates.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/types.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/protocol.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/server.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/session_input.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/mod.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/lifecycle.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/loop.rs`
- Modify: `apps/desktop/src-tauri/src/agent/snapshot.rs`
- Modify: `apps/desktop/src/lib/loopRuntime.ts`
- Modify: `apps/desktop/src/components/loop/LoopTaskPanel.tsx`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`

**GitNexus impact requirements:**

```text
gitnexus_impact(repo: "forge", target: "AgentSession", file_path: "apps/desktop/src-tauri/src/agent/session/mod.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "run_agent_turn", file_path: "apps/desktop/src-tauri/src/agent/session/loop.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "send_message_with_emitter", file_path: "apps/desktop/src-tauri/src/agent/session/loop.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "send_message_with_shared_emitter", file_path: "apps/desktop/src-tauri/src/agent/session/loop.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "LoopTaskRunner", file_path: "apps/desktop/src-tauri/src/loop_runtime/runner.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "dispatch", file_path: "apps/desktop/src-tauri/src/gateway/server.rs", direction: "upstream", summaryOnly: true)
gitnexus_impact(repo: "forge", target: "LoopPolicy", file_path: "apps/desktop/src-tauri/src/loop_runtime/types.rs", direction: "upstream", summaryOnly: true)
```

**Implementation steps:**

- [ ] **Step 4.1: Write disabled-by-default tests first**

  Add runner tests proving:

  - a pending task without an existing desktop session remains `waiting_for_input`,
  - a pending task with no approved headless policy does not create an `AgentSession`,
  - restart replay never resumes side effects unless an approved durable gate exists.

  Run:

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::runner --lib
  ```

  Expected first result: PASS for current waiting behavior, then FAIL only for new explicit policy structs not yet present.

- [ ] **Step 4.2: Add headless ownership policy types**

  In `loop_runtime/headless.rs`, define a small policy surface:

  ```text
  HeadlessResumeMode: Disabled | RequireHumanApproval | ApprovedForTask
  HeadlessResumeApproval: task_id, approved_by, approved_at_ms, scope, expires_at_ms
  HeadlessAgentLease: task_id, session_id, lease_id, owner_pid, expires_at_ms
  ```

  Store approvals as loop runtime events, not as mutable-only projection fields.

- [ ] **Step 4.3: Add gateway RPC/control for approval**

  Add JSON-RPC methods that request and record headless resume approval. The default policy returns a clear gateway error explaining that autonomous resume is disabled until approval is recorded.

- [ ] **Step 4.4: Connect runner to headless creation only behind approval**

  The runner may create or resume a headless `AgentSession` only when:

  - the task policy allows headless resume,
  - a durable human approval gate is closed for this task,
  - budget and policy preflight pass,
  - no stale running lease is active,
  - session snapshot/replay has succeeded.

  If any condition fails, record a waiting/interrupted event rather than starting model work.

- [ ] **Step 4.5: Add UI copy that exposes policy state**

  `LoopTaskPanel` must distinguish:

  - waiting for desktop session,
  - waiting for headless approval,
  - headless approved but lease pending,
  - headless running.

  The UI must not present "continue automatically" as the default action.

- [ ] **Step 4.6: Run focused verification**

  Run:

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::runner --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session --lib
  node --test apps/desktop/src/lib/loopRuntime.test.ts
  npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
  git diff --check
  ```

  Expected: all pass.

**Acceptance command:** `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::runner --lib`

**Expected commit message:** `feat(runtime): gate gateway headless resume ownership`

**Not claimed by this task:**

- No default autonomous gateway continuation.
- No unattended production coding loop without explicit policy and human approval.
- No auto commit/merge/push.
- No guarantee that every provider/session type can run headless until adapter/session tests prove it.

---

### Task 5: A2A Lineage Completion

**Goal:** Complete durable A2A lineage by adding parent-session structs, persisted parent-side child arrays, and automatic parent selection for ordinary delegates when a real parent context exists.

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

- [ ] **Step 5.1: Add persistence tests for parent-side child arrays**

  Tests must prove:

  - assigning a child updates `parent_task_id` on the child and a durable `child_task_ids` array on the parent,
  - child arrays survive bus serialization, A2A sidecar load, and `AgentSession` snapshot restore,
  - legacy ledgers without `child_task_ids` deserialize with an empty array,
  - duplicate child ids are not recorded twice.

  Run:

  ```bash
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml parent --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::session::a2a --lib
  ```

  Expected first result: FAIL because parent-side child arrays are currently projection-derived, not persisted.

- [ ] **Step 5.2: Add parent-session structs**

  Add a compact parent session structure that records:

  ```text
  parent_session_id
  active_parent_task_id
  root_task_id
  selection_reason
  updated_at_ms
  ```

  Persist it through `AgentSession` snapshot and the A2A sidecar if it is part of A2A durable state.

- [ ] **Step 5.3: Implement durable parent-side arrays**

  Extend `AgentTaskRecord` with `child_task_ids` using serde defaults for old records. Update `assign_child_task` to mutate both sides atomically within the bus update path. Projection should read the persisted array and may still rebuild missing legacy arrays for compatibility when a child has `parent_task_id`.

- [ ] **Step 5.4: Implement automatic parent selection for ordinary delegates**

  In `agent/session/tools.rs`, when `delegate_task` has no explicit `parent_task_id`, choose the active parent task only if:

  - the current `AgentSession` has a parent-session context,
  - the active parent id exists in the current bus,
  - the selected parent belongs to the same session,
  - the delegate is not itself a root planning task.

  If no valid active parent exists, keep the current root assignment behavior.

- [ ] **Step 5.5: Update UI and acceptance mocks**

  Update TypeScript protocol and A2A workbench mocks so parent tasks display persisted child arrays after replay. Keep the visible parent/child indicators unchanged unless the persisted data enables a clearer label.

- [ ] **Step 5.6: Run focused verification**

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

- [ ] **Step 6.1: Write completion blocker tests**

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

- [ ] **Step 6.2: Add explicit commit eligibility facts**

  Add projection fields or evidence records for:

  ```text
  review_status
  commit_eligible
  commit_blockers
  human_gate_id
  last_review_decision
  ```

  These facts describe readiness only. They must not contain a command executor for commits.

- [ ] **Step 6.3: Bridge A2A review decisions into loop completion**

  When an A2A review decision changes, record loop evidence or gate decisions only for the matching loop task/session. Do not infer approval from a completed worker without an explicit review decision.

- [ ] **Step 6.4: Update UI copy and controls**

  `LoopTaskPanel` and A2A review UI should show:

  - ready for human review,
  - blocked by review,
  - commit eligible after human review,
  - commit remains human-gated.

  Do not add a button that runs commit/merge/push.

- [ ] **Step 6.5: Add product smoke coverage**

  Extend `apps/desktop/e2e/acceptance.spec.ts` to assert that a completed loop shows review/commit eligibility but not an automatic commit action.

- [ ] **Step 6.6: Run focused verification**

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

---

### Task 7: Acceptance Matrix And Obsidian Evidence Requirements

**Goal:** Keep acceptance, repo docs, and the Obsidian narrative aligned with the new runtime ownership claims.

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
  - Tauri/WebDriver restart harness,
  - provider usage known/unknown telemetry,
  - post-shell file-effect evidence,
  - persisted A2A lineage,
  - review-to-commit eligibility,
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
  rg -n "auto commit|shell-internal|Phase 4-K|b959091b" docs/superpowers/plans/2026-06-18-level-3-agent-loop-next-stage-runtime-ownership.md "/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Level 3 Agent Loop Next Stage Runtime Ownership Plan.md"
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
| Restart runtime ownership | Tauri/WebDriver force-quit/reopen spec | `npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts` |
| Known/unknown provider usage | Adapter and budget tests | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib` |
| Direct ToolExecutor file IO | Existing Phase 4-K executor stream tests | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml executor_file_io_stream --lib` |
| Post-shell delta evidence | Shell file-effect tests | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml shell_file_effect --lib` |
| A2A child file-ish facts | Existing child runtime bridge tests | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a::child --lib` |
| A2A persisted lineage | A2A/session lineage tests | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib` |
| Completion/review/commit eligibility | Completion/gate tests and e2e smoke | `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::completion --lib` |
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
