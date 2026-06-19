# Level 3 Headless Owner Design Gate Implementation Plan

> **Status:** 4C.0 DESIGN GATE RECORDED. 4C.1 CONTRACT-ONLY SLICE LANDED IN `5ececb56 feat(runtime): add headless owner contract`. 4C.2 LEDGER / PROJECTION / REPLAY / IDEMPOTENCY SLICE LANDED IN `28da5966 feat(runtime): project headless owner runs`. 4C.3 COORDINATOR DRY RUN LANDED IN `932dffcb feat(runtime): add headless owner dry run`. 4C.4 TEST-ONLY FAKE EXECUTOR FIXTURE LANDED IN `cb469b27 feat(runtime): add fake headless owner executor fixture`. REAL RUNTIME EXECUTION REMAINS FUTURE. NO USER APPROVAL FOR HIGH-CRITICAL REAL RUNTIME EXECUTION CODE YET.
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. This document began as a 4C.0 design gate and now records the 4C.1 contract-only evidence, 4C.2 additive ledger/projection evidence, 4C.3 coordinator dry-run evidence, and 4C.4 test-only fake executor fixture evidence. It does not authorize real runtime owner execution edits. Any later implementation touching `AgentSession`, gateway dispatch, `handle_request_headless_resume`, `eval_headless`, model/tool/file execution, or production executor acceptance must rerun GitNexus impact and receive explicit user confirmation for HIGH or CRITICAL code.

**Goal:** Define the ownership contract, rollout slices, evidence gates, and stop lines required before Forge can consider a real headless owner for Level 3 loop tasks. As of 4C.4, Forge can persist and replay owner-run intent/state, perform a coordinator dry run after approval/policy/budget facts, and exercise runner-only fake executor orchestration states in tests, but still cannot execute a real headless owner.

**Architecture:** Keep 4A/4B behavior conservative. Durable approval and derived readiness can describe whether a task could be eligible for future ownership, but they do not authorize execution. A future owner must bind one human approval, policy decision, budget snapshot, snapshot source, lease, attempt, idempotency key, and causation chain before any real `AgentSession` adapter is considered.

**Tech Stack:** Rust/Tauri desktop runtime, loop runtime ledger/projection/replay, gateway JSON-RPC, `AgentSession`, existing `eval_headless` module as a reference only, TypeScript runtime projections/UI, Playwright acceptance, GitNexus impact analysis, Obsidian roadmap mirror.

---

## 1. Goal / Non-goal

**Goal:** 4C.0 produces an engineering design gate for real headless ownership. The design must be readable without prior context and must answer who authorizes a headless owner, which snapshot it resumes from, which lease and attempt own work, which policy and budget facts allow or deny it, how idempotency prevents duplicate side effects, and how failure, cancellation, expiry, and replay remain auditable.

**Non-goal:** 4C.0 does not implement a runtime owner. It does not set `gateway_can_resume=true`, does not create a headless `AgentSession`, does not call a model, does not run tools, does not accept pending confirmations, does not mutate files, and does not add auto commit, merge, or push behavior.

**Source of truth:** This repo design doc is the engineering source of truth. The Obsidian note is a narrative mirror for planning and review.

## 2. Current State From 4A/4B/4C

Task 4A landed a durable headless approval contract. It records approval intent, still keeps `gateway_can_resume=false`, and creates no `AgentSession`.

Task 4B landed in `aa9fd74e feat(runtime): surface headless resume readiness`. It added derived readiness and lease-pending UI only. It did not add automatic resume, did not create a real headless `AgentSession`, did not execute `eval_headless`, did not make a model call, did not create file side effects, and did not introduce auto commit.

Task 4C.1 landed in `5ececb56 feat(runtime): add headless owner contract`. It added the contract-only Rust type surface for future owner runs: `HeadlessOwnerRun`, `HeadlessOwnerRunState`, `HeadlessOwnerSnapshotSource`, and `HeadlessOwnerExecutorKind`; re-exported the new contract types; and added contract-only tests. This did not add durable owner run events, projection, replay, gateway behavior, runner lease allocation, execution, model/tool/file side effects, a real `AgentSession` adapter, or `gateway_can_resume=true`.

Task 4C.2 landed in `28da5966 feat(runtime): project headless owner runs`. It added durable owner-run request/state events, projection state, replay/idempotency semantics, journal retry fingerprinting, and the TypeScript protocol mirror. It did not add a gateway API change, did not allocate a runner lease, did not run a coordinator dry run, did not add a fake executor, did not create a real `AgentSession` adapter, did not call a model, did not run tools, did not mutate files, did not set `gateway_can_resume=true`, and did not add auto commit, merge, or push behavior.

Task 4C.3 landed in `932dffcb feat(runtime): add headless owner dry run`. It records coordinator dry-run facts only after unexpired headless resume approval, records the chain `PolicyDecisionRecorded` -> `BudgetSnapshotRecorded` -> `HeadlessOwnerRunRequested` -> `Denied` OR `LeaseAcquired` -> `WaitingForInput`, and surfaces status through runner stats, gateway runtime status/dashboard snapshots, CLI runtime lines, the TypeScript IPC type, and diagnostics summary. It did not add a fake executor, real `AgentSession`, `eval_headless` production shortcut, model/provider call, tool call, file IO, pending confirmation/tool auto-acceptance, `gateway_can_resume=true`, or auto commit/merge/push.

Task 4C.4 landed in `cb469b27 feat(runtime): add fake headless owner executor fixture`. It adds runner-only `#[cfg(test)]` fake executor fixture support in `apps/desktop/src-tauri/src/loop_runtime/runner.rs` and records fake owner-run state chains through the same journal/projection/envelope path. It covers completed, pending confirmation blocker, pending tool-call blocker, interrupted, cancelled, expired, and stale pending-view idempotency paths. Fake `Completed` is orchestration-only evidence: the task stays `WaitingForInput`, completion result reasons and commit blockers are `task_waiting_for_input`, and `commit_eligible == false`. It did not change gateway, TypeScript, or status shapes; did not create a real `AgentSession`; did not call `eval_headless`, a model/provider, `ToolExecutor`, or file IO; did not set `gateway_can_resume=true`; did not auto-accept pending confirmations/tools; and did not automate commit/merge/push.

Current code facts to preserve:

- `loop_runtime/headless.rs` has `HeadlessResumeMode`, `HeadlessResumeApproval`, `HeadlessAgentLease`, `HeadlessResumeReadiness`, and `derive_headless_resume_readiness`.
- `LoopEventEnvelope` already has `lease_id`, `attempt`, `correlation_id`, `causation_id`, and `idempotency_key`.
- `LoopRuntimeEvent` already has `TaskStarted`, `TaskWaitingForInput`, `TaskInterrupted`, `HeadlessResumeApprovalRecorded`, `PolicyDecisionRecorded`, `BudgetSnapshotRecorded`, and `CompletionEvaluated`.
- 4C.2 adds `HeadlessOwnerRunRequested`, `HeadlessOwnerRunStateRecorded`, and `LoopTaskRecord.headless_owner_runs` for durable owner-run projection/replay.
- `LoopTaskRunner` can now record a headless owner dry run after approval/policy/budget facts; in tests only, `with_fake_executor_fixture` can swap the owner executor kind to `FakeExecutor` and record fake running/outcome states without creating `AgentSession`.
- Gateway `request_headless_resume` currently records approval only, and responses keep `gateway_can_resume=false`.

## 3. Fresh GitNexus Risk Scan

This risk scan is the required evidence baseline for 4C.0. These values must be carried forward before any later implementation discussion:

- Struct `AgentSession`, file `apps/desktop/src-tauri/src/agent/session/mod.rs`: **CRITICAL**, impactedCount 48, direct 1, processes_affected 3, modules_affected 5. Affected processes: `send_input`, `create_session`, `run_request`. Modules: IPC, Agent, Eval_headless, A2A, Session.
- Function `dispatch`, file `apps/desktop/src-tauri/src/gateway/server.rs`: **CRITICAL**, impactedCount 45, direct 39, processes_affected 3, modules_affected 3. Affected processes: `dispatch_dashboard_snapshot_returns_dashboard_operational_summary`, `evaluate_loop_task_completion_uses_projected_evidence`, `evaluate_loop_task_completion_returns_typed_result`. Modules: Gateway, Loop_runtime, IPC.
- Function `handle_request_headless_resume`, file `apps/desktop/src-tauri/src/gateway/server.rs`: **HIGH**, impactedCount 42, direct 1, processes_affected 3, modules_affected 3. Affected processes: `dispatch_dashboard_snapshot_returns_dashboard_operational_summary`, `evaluate_loop_task_completion_uses_projected_evidence`, `evaluate_loop_task_completion_returns_typed_result`. Modules: Gateway, Loop_runtime, IPC.
- Function `run_request`, file `apps/desktop/src-tauri/src/eval_headless/mod.rs`: **LOW**, impactedCount 2, direct 2, processes_affected 0, module Eval_headless. Code facts still matter: `eval_headless::run_request` creates an `AgentSession`, snapshots the workspace, sends model turns, and validates/repairs output; design must treat it as reference material, not a production owner shortcut.

Risk conclusion: 4C implementation is blocked until a later user explicitly confirms the narrowed HIGH/CRITICAL runtime scope. This document is design only and does not grant that approval.

## 3.1 4C.1 Contract-Only Evidence

4C.1 is implemented as a contract-only slice in `5ececb56 feat(runtime): add headless owner contract`.

What changed:

- Added `HeadlessOwnerRun`.
- Added `HeadlessOwnerRunState`.
- Added `HeadlessOwnerSnapshotSource`.
- Added `HeadlessOwnerExecutorKind`.
- Re-exported the new contract types from the loop runtime surface.
- Added contract-only tests for serialization/shape/default coverage.

TDD and verification evidence recorded by the controller/subagents:

- RED: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_owner_contract --lib` failed first with unresolved/missing contract types.
- RED: after adding enum coverage, the same command failed again because required enum variants were still missing. This proves the TDD gate covered lifecycle/source/executor breadth.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_owner_contract --lib` passed 5/5.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_resume --lib` passed 12/12.
- `git diff --check` passed.
- GitNexus staged detect before commit reported LOW risk, changed_files 2, affected_processes 0. It noted `HeadlessResumeApproval` fields as touched due line movement; the shape guard test proves approval serde shape unchanged.

Review evidence:

- Spec review initially returned NEEDS_FIX because lifecycle/source/executor enums were too narrow.
- Implementer fixed the contract by adding the full design enum values.
- Spec re-review APPROVED.
- Quality review APPROVED.

4C.1 not claimed:

- 4C.1 itself did not implement durable owner run event/projection/replay.
- no gateway API change.
- no runner lease allocation for owner contract.
- no model/tool/file side effects.
- no real AgentSession adapter.
- no gateway_can_resume=true.
- no auto commit/merge/push.
- no user approval for HIGH/CRITICAL runtime code yet.

## 3.2 4C.2 Ledger / Projection / Idempotency Evidence

4C.2 is implemented as an additive ledger/projection slice in `28da5966 feat(runtime): project headless owner runs`.

What changed:

- Added durable owner-run runtime events `HeadlessOwnerRunRequested` and `HeadlessOwnerRunStateRecorded`.
- Added snake_case `kind()` strings for the new event variants.
- Added a request envelope helper that fills `lease_id`, `attempt`, `correlation_id`, `causation_id`, and `idempotency_key` from `HeadlessOwnerRun`.
- Added projection state `LoopTaskRecord.headless_owner_runs: Vec<HeadlessOwnerRun>` with serde default and skip-empty behavior.
- Updated projection/replay semantics so owner-run requests validate the auth bundle and envelope lease/attempt/idempotency metadata.
- Made duplicate requests idempotent by same `owner_run_id` or same task plus `idempotency_key`; duplicates do not duplicate owner runs.
- Made regenerated retry requests return/replay the first event when `owner_run_id`, `requested_at_ms`, and `expires_at_ms` are regenerated under the same task plus idempotency key, preserving the original timestamps.
- Kept conflicting request identity fields as errors.
- Made state events update one existing owner run in place for lease/waiting/interrupted/expired/cancelled-style states.
- Kept state-before-request as an error.
- Updated journal idempotency fingerprinting for owner-run request retries.
- Updated the TypeScript protocol mirror with optional `HeadlessOwnerRun` types and `headless_owner_runs?: HeadlessOwnerRun[]` only. No UI behavior was added.

TDD and verification evidence recorded by the controller/subagents:

- RED: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::projection --lib` failed first with E0609 because `LoopTaskRecord` had no `headless_owner_runs` field.
- Follow-up RED: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::journal --lib` failed on regenerated timestamp retry with `idempotency conflict for key: owner-idem-1`.
- Follow-up RED: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::projection --lib` failed with `duplicate headless owner run requested: owner-run-regenerated`.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::journal --lib` passed 20/20, recorded as 20 passed.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::projection --lib` passed 23/23, recorded as 23 passed.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::replay_tests --lib` passed 3/3.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_owner_contract --lib` passed 5/5.
- GREEN: `node --test apps/desktop/src/lib/loopRuntime.test.ts` passed 16/16.
- `git diff --check` passed.

Review evidence:

- Spec reviewer APPROVED, then re-review APPROVED after the regenerated timestamp idempotency fix.
- Quality reviewer APPROVED, then re-review APPROVED after the regenerated timestamp idempotency fix.

GitNexus evidence:

- Before-edit impact for `LoopTaskProjection::from_events`: HIGH.
- Before-edit impact for `LoopEventJournal::append_idempotent`: HIGH.
- Before-edit impact for `event_payload_fingerprint`: HIGH.
- Before-edit impact for TypeScript `LoopTaskRecord`: CRITICAL.
- Staged detect before commit reported HIGH risk, changed_files 5, affected_processes 7. That HIGH/CRITICAL risk was confined to additive shared runtime/projection/journal/protocol contract work with focused RED/GREEN tests and spec/quality review.

4C.2 not claimed:

- no gateway API change.
- no runner lease allocation.
- no coordinator dry run.
- no fake executor.
- no real AgentSession adapter.
- no model call/tool call/file side effect.
- no gateway_can_resume=true.
- no auto commit/merge/push.
- no user approval for HIGH/CRITICAL runtime execution code.
- 4C.2 proves ledger/projection/replay/idempotency facts only.

## 3.3 4C.3 Coordinator Dry Run Evidence

4C.3 is implemented as a safe coordinator dry-run slice in `932dffcb feat(runtime): add headless owner dry run`.

What changed:

- Runner records coordinator dry-run facts only after unexpired headless resume approval.
- The durable chain is `PolicyDecisionRecorded` -> `BudgetSnapshotRecorded` -> `HeadlessOwnerRunRequested` -> `Denied` OR `LeaseAcquired` -> `WaitingForInput`.
- Default policy denial is observable and safe: `Denied`, with no `LeaseAcquired`.
- Policy allowed plus budget allowed stops at `WaitingForInput` with the `DryRun` executor.
- Budget denial records `Denied` with the budget reason.
- No owner run is recorded without approval.
- Stale running lease expiry expires the associated nonterminal owner run.
- `TaskStarted` idempotency uses stable task/attempt identity so regenerated leases reuse the original persisted event while different task/attempt conflicts remain conflicts.
- Status visibility appears through runner stats, gateway runtime status/dashboard snapshot, CLI runtime lines, TypeScript IPC type, and diagnostics summary.

Verification evidence recorded by the controller/subagents:

- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::runner --lib` passed 12/12.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::journal --lib` passed 23/23.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib` passed 146/146.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml diagnostics_handlers --lib` passed 18/18.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::runner` passed across lib/bin/integration filtered targets.
- GREEN: `node --test apps/desktop/src/components/settings/diagnosticsRuntimeView.test.ts` passed 17/17.
- `git diff --check` passed.
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check` was not used as a pass gate because it reports pre-existing formatting diffs in `apps/desktop/src-tauri/src/loop_runtime/headless.rs`.

GitNexus evidence:

- Targeted impacts were mostly LOW for runner/status symbols.
- HIGH warnings were surfaced for `event_payload_fingerprint` and shared TypeScript `GatewayRuntimeStatus`; changes were kept narrow and reviewed.
- `detect_changes({scope: staged})` reported HIGH, expected because shared runtime/status/journal contracts changed; affected flows centered on dashboard snapshot and runner wait/idempotency flows.

4C.3 not claimed:

- no real headless `AgentSession`.
- no `eval_headless` production shortcut.
- no model/provider call.
- no tool call.
- no project file IO or executor-level live file tracing claim.
- no gateway API automatic resume and no `gateway_can_resume=true`.
- 4C.3 itself did not add a fake executor; the later 4C.4 fixture is test-only.
- no pending confirmation/tool auto-acceptance.
- no auto commit/merge/push; commit remains human-gated.
- no user approval for HIGH/CRITICAL real runtime execution code yet.

## 3.4 4C.4 Fake Executor Fixture Evidence

4C.4 is implemented as a runner-only fake executor fixture slice in `cb469b27 feat(runtime): add fake headless owner executor fixture`.

What changed:

- Added `#[cfg(test)]` fake executor fixture support in `apps/desktop/src-tauri/src/loop_runtime/runner.rs`.
- Added fake executor outcomes for completed, pending confirmation, pending tool call, interrupted, cancelled, and expired.
- Recorded the fake executor chain through existing durable owner-run request/state events and the same journal/projection/envelope metadata.
- Preserved lease id, attempt, causation, correlation, and idempotency metadata across fake `LeaseAcquired` -> `FakeRunning` -> outcome state chains.
- Kept fake `Completed` as orchestration-only evidence: the loop task remains `WaitingForInput`, completion reasons and commit blockers stay `task_waiting_for_input`, and `commit_eligible == false`.
- Proved stale pending-view retry idempotency for the fake executor path.

Verification evidence recorded by the controller/subagents:

- RED: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::runner --lib` failed first with missing `with_fake_executor_fixture` / `FakeOwnerExecutorFixture`.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::runner --lib` passed 17/17.
- Additional hardening RED: a focused runner test initially failed when expecting `task_not_completed`; the actual blocker was `task_waiting_for_input`.
- Additional hardening GREEN: the focused runner test passed after asserting `commit_eligible == false` and `commit_blockers == ["task_waiting_for_input"]`.
- `rustfmt --edition 2024 --check apps/desktop/src-tauri/src/loop_runtime/runner.rs` passed.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::runner --lib` passed 17/17.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::journal --lib` passed 23/23.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::projection --lib` passed 23/23.
- GREEN: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::replay_tests --lib` passed 3/3.
- `git diff --check` passed.

Review evidence:

- Spec review PASS.
- Quality review PASS after formatting fix.

GitNexus evidence:

- Before-edit impact for `process_pending_task`: LOW.
- Before-edit impact for `queue_stats`: LOW.
- Before-edit impact for `runner.rs`: LOW.
- Staged detect after implementation: MEDIUM, with affected flows limited to runner wait/lease/idempotency flows.

4C.4 not claimed:

- no gateway/TypeScript/status shape change.
- no real `AgentSession`.
- no `eval_headless` production shortcut.
- no model/provider call.
- no `ToolExecutor` call.
- no project file IO.
- no `gateway_can_resume=true`.
- no pending confirmation/tool auto-acceptance.
- no auto commit/merge/push; commit remains human-gated.
- no user approval for HIGH/CRITICAL real runtime execution code yet.

## 4. Ownership Contract

The contract type names are implemented by 4C.1, durable request/state event plus projection/replay/idempotency semantics are implemented by 4C.2, the coordinator dry run is implemented by 4C.3, and runner-only fake executor fixture evidence is implemented by 4C.4. Real adapter behavior, model/tool/file execution, `gateway_can_resume=true`, pending confirmation/tool auto-acceptance, and auto commit/merge/push remain future and not claimed:

```text
HeadlessOwnerRun {
  owner_run_id
  task_id
  session_id
  lease_id
  attempt
  state
  snapshot_source
  snapshot_ref
  human_gate_id
  policy_decision_id
  budget_snapshot_id
  idempotency_key
  correlation_id
  causation_id
  requested_by
  requested_at_ms
  heartbeat_at_ms
  expires_at_ms
  cancellation_reason
  waiting_reason
  executor_kind
  evidence_refs
}
```

**Required fields and meaning:**

- `owner_run_id`: durable identity for one proposed owner run.
- `task_id`: loop task being considered for ownership.
- `session_id`: session boundary the task belongs to, if available.
- `lease_id`: durable lease identity. It must match envelope `lease_id` when owner events are emitted.
- `attempt`: monotonic attempt number for the task/owner pair. Retries create new attempts; idempotent replays do not.
- `state`: lifecycle state listed below.
- `snapshot_source`: explicit source such as `current_desktop_session`, `persisted_session_snapshot`, `workspace_snapshot`, or `restored_headless_snapshot`.
- `snapshot_ref`: stable reference to the selected snapshot or the reason no snapshot can be selected.
- `human_gate_id`: approval gate that authorized this exact attempt. Approval/readiness without this binding is not authorization.
- `policy_decision_id`: policy fact that allowed or denied this attempt.
- `budget_snapshot_id`: budget fact used before execution.
- `idempotency_key`: request key that prevents duplicate owner runs, duplicate leases, duplicate model calls, duplicate tool calls, and duplicate file side effects.
- `correlation_id`: groups the request, policy, budget, lease, and owner run.
- `causation_id`: points to the event that caused the next event in the chain.
- `requested_by`: human or gateway actor that requested the run.
- `requested_at_ms`, `heartbeat_at_ms`, `expires_at_ms`: timing facts for replay, stale lease detection, and cancellation.
- `cancellation_reason` and `waiting_reason`: explicit stop reasons, not inferred from missing events.
- `executor_kind`: `none`, `dry_run`, `fake_executor`, or future `agent_session_adapter`.
- `evidence_refs`: ids for completion, policy, budget, review, usage, and file-effect evidence that belong to this attempt.

**Lifecycle states:**

- `requested`: a request was recorded but no authorization bundle has been validated.
- `denied`: policy, budget, approval, or snapshot selection denied the run.
- `ready`: all required authorization facts exist, but no lease has been acquired.
- `lease_acquired`: a lease is held for a specific attempt.
- `dry_run_waiting`: coordinator acquired ownership and intentionally stopped without execution.
- `fake_running`: fake executor is exercising orchestration only.
- `running`: reserved for a future real adapter after explicit approval.
- `waiting_for_input`: pending confirmations, pending tool calls, missing profile/provider, or human input blocked progress.
- `interrupted`: runner or owner stopped before completion, with cause recorded.
- `cancelled`: user or policy cancelled this attempt.
- `expired`: lease TTL/heartbeat expired.
- `completed`: owner attempt finished its allowed scope.
- `failed`: owner attempt failed with recorded error evidence.

**Lease and attempt semantics:**

- A lease grants temporary ownership of one task attempt; it is not execution authorization by itself.
- `attempt` increments when a new non-idempotent owner attempt is started.
- Replaying the same `idempotency_key` must return the existing owner run state, not create another attempt.
- Lease heartbeat and TTL must be replayable from ledger events.
- Expired leases must not permit a later owner event unless a new authorized attempt is created.

**Causation chain:**

The expected chain is approval request -> approval recorded -> policy decision -> budget snapshot -> owner run requested -> lease acquired -> dry run / fake executor / future adapter event -> waiting, cancelled, expired, completed, failed, or interrupted. Each event must carry envelope `correlation_id`, `causation_id`, `idempotency_key`, `lease_id` where applicable, and `attempt` where applicable.

## 5. Stop Lines

- Default disabled. A future feature flag or policy must default to no real headless owner.
- Approval/readiness is not authorization. `HeadlessResumeReadiness` can explain eligibility, but it cannot execute.
- No `gateway_can_resume=true` by default.
- No automatic gateway resume from 4A/4B state.
- No real headless `AgentSession` until contract, projection, dry run, test-only fake executor evidence, and explicit HIGH/CRITICAL user approval are complete.
- No auto pending confirmation acceptance, no auto pending tool call acceptance, and no hidden bypass for permission gates.
- No auto commit/merge/push. Commit remains a human-controlled action.
- `eval_headless` is not a production owner shortcut. Its `run_request` path creates `AgentSession`, snapshots workspace, sends model turns, and validates/repairs output, so it requires a separate production policy and side-effect boundary.
- No shell-internal tracing claim.
- No billing-grade cost claim. Unknown provider usage or pricing remains unknown.
- No docs or Obsidian note may claim runtime ownership before tests and acceptance evidence exist.

## 6. Safe Rollout Sequence

**4C.1 Contract tests (landed in `5ececb56`):** Added only contract types, re-exports, serialization/shape/default coverage, and enum coverage for lifecycle/source/executor values. No runner, gateway dispatch, model call, tool call, file side effect, durable owner run event, projection, replay, or `gateway_can_resume=true`.

**4C.2 Projection/replay/idempotency (landed in `28da5966`):** Added ledger events, projection state, replay tests, duplicate request handling, regenerated retry handling, state-update semantics, journal idempotency fingerprinting, and TypeScript protocol mirror types. Still no gateway API change, runner lease allocation, coordinator dry run, fake executor, real `AgentSession` adapter, model/tool/file side effect, `gateway_can_resume=true`, or auto commit/merge/push.

**4C.3 Coordinator dry run (landed in `932dffcb`):** Records coordinator dry-run facts after unexpired approval, policy, and budget facts. Default/policy/budget denial is safe and observable; the allowed path acquires a lease and then stops at `WaitingForInput` with `DryRun`. It does not create `AgentSession`, call `eval_headless`, call a model, run a tool, touch files, set `gateway_can_resume=true`, or auto commit/merge/push.

**4C.4 Fake executor fixture (landed in `cb469b27`):** Added a fake executor behind explicit runner test fixtures to prove orchestration and state transitions without provider access, `eval_headless`, `ToolExecutor`, or file side effects. Pending confirmations and pending tool calls are blockers and are not auto-accepted. Fake `Completed` is orchestration-only evidence and does not make commit eligible.

**4C.5 Real `AgentSession` adapter:** Only after explicit user approval for HIGH/CRITICAL symbols, consider a real adapter. It must handle snapshot restore, provider/profile resolution, pending confirmations, cancellation, lease heartbeat/expiry, budget preflight, policy denial, and no auto commit.

**4C.6 Evidence sync:** Update repo docs and Obsidian mirror with commit hash, focused tests, GitNexus risk summary, and not-claimed bullets for each implemented slice. Include an acceptance command only when that slice actually changes or is covered by the acceptance matrix; for 4C.4, the evidence is focused runner/journal/projection/replay tests.

## 7. Test / Evidence Gates

These commands are slice gates. 4C.1 has run the contract and headless-resume gates listed below; 4C.2 has run the journal/projection/replay/protocol mirror gates listed below; 4C.3 has run the runner/journal/gateway/diagnostics/status gates listed below; 4C.4 has run the focused runner/journal/projection/replay fake executor fixture gates listed below. Real adapter/e2e execution gates remain future:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_owner_contract --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::projection --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::replay_tests --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime::runner --lib
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
scripts/acceptance.sh --dry-run
```

Evidence by slice:

- 4C.1 landed in `5ececb56`: `headless_owner_contract --lib` passed 5/5, `headless_resume --lib` passed 12/12, `git diff --check` passed, and GitNexus staged detect reported LOW risk with affected_processes 0. The slice proves the contract type surface and enum coverage only.
- 4C.2 landed in `28da5966`: `loop_runtime::journal --lib` passed 20/20, recorded as 20 passed; `loop_runtime::projection --lib` passed 23/23, recorded as 23 passed; `loop_runtime::replay_tests --lib` passed 3/3; `headless_owner_contract --lib` passed 5/5; `node --test apps/desktop/src/lib/loopRuntime.test.ts` passed 16/16; `git diff --check` passed. The slice proves owner-run request/state events, `headless_owner_runs` projection/replay, regenerated idempotency retry behavior, and TypeScript protocol shape only.
- 4C.3 landed in `932dffcb`: `loop_runtime::runner --lib` passed 12/12; `loop_runtime::journal --lib` passed 23/23; `gateway --lib` passed 146/146; `diagnostics_handlers --lib` passed 18/18; `loop_runtime::runner` passed across lib/bin/integration filtered targets; `node --test apps/desktop/src/components/settings/diagnosticsRuntimeView.test.ts` passed 17/17; `git diff --check` passed. The slice proves approval-gated coordinator dry-run facts, default/policy/budget denial, lease-acquired-to-waiting behavior, stale lease expiry, regenerated lease idempotency, and status visibility without execution.
- 4C.4 landed in `cb469b27`: `loop_runtime::runner --lib` failed RED first with missing fixture API, then passed 17/17; a follow-up RED clarified fake completion remains blocked by `task_waiting_for_input`; `rustfmt --edition 2024 --check apps/desktop/src-tauri/src/loop_runtime/runner.rs` passed; `loop_runtime::journal --lib` passed 23/23; `loop_runtime::projection --lib` passed 23/23; `loop_runtime::replay_tests --lib` passed 3/3; `git diff --check` passed. The slice proves runner-only fake executor orchestration states and blockers, including pending confirmations and pending tool calls, without production execution.
- 4C.5: real adapter tests prove snapshot selection, profile/provider resolution, cancellation, heartbeat, budget, policy, pending confirmations, no auto commit, and no file side effect without explicit tool evidence.
- 4C.6: evidence sync proves repo plan, Obsidian mirror, acceptance dry-run labels, and not-claimed bullets match the implemented behavior.

## 8. Open Questions For User / Architecture Review

- Decision required before implementation: Where does the authoritative snapshot come from for a headless owner: live desktop session, persisted `AgentSession` snapshot, workspace snapshot, or a new restored headless snapshot?
- Decision required before implementation: Who owns live session continuation versus restored snapshot execution when a desktop session still exists?
- Decision required before implementation: What lease TTL and heartbeat interval should be used, and which component is allowed to expire stale leases?
- Decision required before implementation: What budget cap is sufficient for a headless attempt, and is budget denial hard failure or waiting-for-input?
- Decision required before implementation: How should pending confirmations be represented: terminal blocker, resumable blocker, or human gate?
- Decision required before implementation: How should cancellation propagate into a future `AgentSession` adapter and any in-flight provider stream?
- Decision required before implementation: Which provider/model/profile resolution rules are allowed for headless ownership, and what happens when the profile is missing or stale?
- Decision recorded by 4C.4: fake executor completion remains orchestration-only evidence for now. It does not satisfy a completion contract or make commit eligible.
- Decision required before implementation: Which UI surface is allowed to show owner readiness without implying execution authorization?

## 9. 中文产品说明

这一步证明 Forge 已经有可审计的接管演练和 test-only fake executor 状态链：有授权、有策略/预算事实、有租约、有 fake running/completed/waiting/interrupted/cancelled/expired 证据，但仍不执行模型、工具或文件写入。Fake `Completed` 只是 orchestration evidence，不满足 commit；在用户明确批准 HIGH/CRITICAL 真实执行代码之前，Forge 仍然不会默认自动恢复、不会创建真正的 headless `AgentSession`、不会自动接受确认或工具调用、不会自动提交代码。

## 10. Interview-ready Explanation

Forge is not jumping from "approval recorded" to "agent runs by itself." 4C.4 proves the next rehearsal layer: after approval, policy, budget, owner-run request, and lease acquisition are durable, a runner-only fake executor can drive fake running, completed, waiting, interrupted, cancelled, expired, and idempotent retry states through the same ledger. That still is not a real `AgentSession`, model call, tool call, file write, gateway resume, or commit path.
