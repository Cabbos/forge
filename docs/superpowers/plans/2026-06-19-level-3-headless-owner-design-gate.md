# Level 3 Headless Owner Design Gate Implementation Plan

> **Status:** 4C.0 DESIGN GATE RECORDED. 4C.1 CONTRACT-ONLY SLICE LANDED IN `5ececb56 feat(runtime): add headless owner contract`. PROJECTION / REPLAY / GATEWAY / RUNNER / EXECUTION REMAIN FUTURE. NO USER APPROVAL FOR HIGH-CRITICAL RUNTIME CODE YET.
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. This document began as a 4C.0 design gate and now records the 4C.1 contract-only evidence. It does not authorize runtime owner execution edits. Any later implementation touching `AgentSession`, gateway dispatch, `handle_request_headless_resume`, or `eval_headless` must rerun GitNexus impact and receive explicit user confirmation for HIGH or CRITICAL code.

**Goal:** Define the ownership contract, rollout slices, evidence gates, and stop lines required before Forge can consider a real headless owner for Level 3 loop tasks. As of 4C.1, only the contract type surface has landed.

**Architecture:** Keep 4A/4B behavior conservative. Durable approval and derived readiness can describe whether a task could be eligible for future ownership, but they do not authorize execution. A future owner must bind one human approval, policy decision, budget snapshot, snapshot source, lease, attempt, idempotency key, and causation chain before any real `AgentSession` adapter is considered.

**Tech Stack:** Rust/Tauri desktop runtime, loop runtime ledger/projection/replay, gateway JSON-RPC, `AgentSession`, existing `eval_headless` module as a reference only, TypeScript runtime projections/UI, Playwright acceptance, GitNexus impact analysis, Obsidian roadmap mirror.

---

## 1. Goal / Non-goal

**Goal:** 4C.0 produces an engineering design gate for real headless ownership. The design must be readable without prior context and must answer who authorizes a headless owner, which snapshot it resumes from, which lease and attempt own work, which policy and budget facts allow or deny it, how idempotency prevents duplicate side effects, and how failure, cancellation, expiry, and replay remain auditable.

**Non-goal:** 4C.0 does not implement a runtime owner. It does not set `gateway_can_resume=true`, does not create a headless `AgentSession`, does not call a model, does not run tools, does not accept pending confirmations, does not mutate files, and does not add auto commit, merge, or push behavior.

**Source of truth:** This repo design doc is the engineering source of truth. The Obsidian note is a narrative mirror for planning and review.

## 2. Current State From 4A/4B

Task 4A landed a durable headless approval contract. It records approval intent, still keeps `gateway_can_resume=false`, and creates no `AgentSession`.

Task 4B landed in `aa9fd74e feat(runtime): surface headless resume readiness`. It added derived readiness and lease-pending UI only. It did not add automatic resume, did not create a real headless `AgentSession`, did not execute `eval_headless`, did not make a model call, did not create file side effects, and did not introduce auto commit.

Task 4C.1 landed in `5ececb56 feat(runtime): add headless owner contract`. It added the contract-only Rust type surface for future owner runs: `HeadlessOwnerRun`, `HeadlessOwnerRunState`, `HeadlessOwnerSnapshotSource`, and `HeadlessOwnerExecutorKind`; re-exported the new contract types; and added contract-only tests. This did not add durable owner run events, projection, replay, gateway behavior, runner lease allocation, execution, model/tool/file side effects, a real `AgentSession` adapter, or `gateway_can_resume=true`.

Current code facts to preserve:

- `loop_runtime/headless.rs` has `HeadlessResumeMode`, `HeadlessResumeApproval`, `HeadlessAgentLease`, `HeadlessResumeReadiness`, and `derive_headless_resume_readiness`.
- `LoopEventEnvelope` already has `lease_id`, `attempt`, `correlation_id`, `causation_id`, and `idempotency_key`.
- `LoopRuntimeEvent` already has `TaskStarted`, `TaskWaitingForInput`, `TaskInterrupted`, `HeadlessResumeApprovalRecorded`, `PolicyDecisionRecorded`, `BudgetSnapshotRecorded`, and `CompletionEvaluated`.
- `LoopTaskRunner` currently claims pending tasks, writes `TaskStarted`, immediately writes `TaskWaitingForInput`, evaluates completion, and does not create `AgentSession`.
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

- no new durable owner run event/projection/replay yet; no durable owner run event is implemented.
- no gateway API change.
- no runner lease allocation for owner contract.
- no model/tool/file side effects.
- no real AgentSession adapter.
- no gateway_can_resume=true.
- no auto commit/merge/push.
- no user approval for HIGH/CRITICAL runtime code yet.

## 4. Ownership Contract

The contract type names are now implemented by 4C.1 as a contract-only surface. The durable event/projection/replay/gateway/runner semantics below remain the design contract for future slices, not implemented runtime behavior:

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
- No real headless `AgentSession` until contract, projection, dry run, fake executor, and explicit HIGH/CRITICAL user approval are complete.
- No auto pending confirmation acceptance, no auto pending tool call acceptance, and no hidden bypass for permission gates.
- No auto commit/merge/push. Commit remains a human-controlled action.
- `eval_headless` is not a production owner shortcut. Its `run_request` path creates `AgentSession`, snapshots workspace, sends model turns, and validates/repairs output, so it requires a separate production policy and side-effect boundary.
- No shell-internal tracing claim.
- No billing-grade cost claim. Unknown provider usage or pricing remains unknown.
- No docs or Obsidian note may claim runtime ownership before tests and acceptance evidence exist.

## 6. Safe Rollout Sequence

**4C.1 Contract tests (landed in `5ececb56`):** Added only contract types, re-exports, serialization/shape/default coverage, and enum coverage for lifecycle/source/executor values. No runner, gateway dispatch, model call, tool call, file side effect, durable owner run event, projection, replay, or `gateway_can_resume=true`.

**4C.2 Projection/replay/idempotency:** Add ledger events, projection state, replay tests, duplicate request handling, expired/cancelled/interrupted states, and idempotent owner run reconstruction. Still no execution.

**4C.3 Coordinator dry run:** Add lease acquisition plus immediate waiting/interrupted outcomes. It may update durable owner state but must not create `AgentSession`, call `eval_headless`, call a model, run a tool, or touch files.

**4C.4 Fake executor:** Add a fake executor behind explicit test fixtures to prove orchestration and state transitions without provider access or file side effects. Pending confirmations and pending tool calls must be blockers.

**4C.5 Real `AgentSession` adapter:** Only after explicit user approval for HIGH/CRITICAL symbols, consider a real adapter. It must handle snapshot restore, provider/profile resolution, pending confirmations, cancellation, lease heartbeat/expiry, budget preflight, policy denial, and no auto commit.

**4C.6 Evidence sync:** Update repo docs and Obsidian mirror with commit hash, tests, acceptance command, GitNexus risk summary, and not-claimed bullets for each implemented slice.

## 7. Test / Evidence Gates

These commands are slice gates. 4C.1 has run the contract and headless-resume gates listed below; later projection/replay/gateway/runner/e2e gates remain future:

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
- 4C.2: projection and replay tests prove owner state survives restart and duplicate idempotency keys do not duplicate owner runs.
- 4C.3: runner/coordinator tests prove default disabled, policy denied, approval missing, lease acquired, lease expired, cancelled, interrupted, and waiting states without execution.
- 4C.4: fake executor acceptance proves orchestration states and blockers, including pending confirmations and pending tool calls.
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
- Decision required before implementation: Can fake executor completion ever satisfy a completion contract, or must it remain orchestration-only evidence?
- Decision required before implementation: Which UI surface is allowed to show owner readiness without implying execution authorization?

## 9. 中文产品说明

这不是让 agent 自动乱跑，而是先证明一套可审计的接管协议：谁授权、从哪个快照恢复、拿哪个租约、预算和策略是否允许、失败和取消怎么记录、重放时会不会重复执行。4C.0 只写设计和停止线；在用户明确批准 HIGH/CRITICAL 代码之前，Forge 仍然不会默认自动恢复、不会创建真正的 headless `AgentSession`、不会自动接受确认或工具调用、不会自动提交代码。

## 10. Interview-ready Explanation

Forge is not jumping from "approval recorded" to "agent runs by itself." The next step is an auditable ownership protocol. A headless owner must prove who authorized it, which snapshot it resumed from, which lease and attempt it owns, which policy and budget facts allowed it, how idempotency prevents duplicate side effects, and how cancellation, expiry, waiting, and failure replay. Only after that evidence exists, and only after explicit approval for the HIGH/CRITICAL runtime symbols, can a real `AgentSession` adapter be considered.
