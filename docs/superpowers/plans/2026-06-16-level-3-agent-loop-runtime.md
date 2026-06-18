# Level 3 Agent Loop Runtime Implementation Plan v2

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Forge from a desktop-hosted agent cockpit to a Level 3 agent loop runtime where the gateway can own durable loop tasks, enforce policy and budgets, emit live subagent telemetry, recover after crashes, and stop only when a structured completion contract is satisfied.

**Architecture:** Keep the current Phase 7 product surfaces and make the runtime underneath stronger. Add a durable loop task ledger owned by the gateway, review/control gates, a completion contract evaluator, and runtime facts before expanding the UI. The MVP makes desktop a client and reviewer of loop state, but it does not promise autonomous gateway recovery and continuation of a full coding loop yet.

**Tech Stack:** Rust/Tauri backend, gateway JSON-RPC and loopback dashboard, existing A2A bus/worktree worker, TypeScript protocol/store/UI, Playwright acceptance, GitNexus impact checks, existing `scripts/acceptance.sh`.

---

## Baseline

As of 2026-06-16:

- Phase 7 product polish is complete: Settings, History, recovery/error state, permissions, rich previews, review actions, background status/notifications, docs, and final acceptance all have coverage.
- The gateway already has registry, attach, snapshot detail, transcript tail, session input inbox/history, scheduler/trigger surfaces, runtime status, service management, dashboard, diagnostics, and repair.
- Code read on 2026-06-16 confirms the runtime split:
  - `gateway/server.rs` is a JSON-RPC dispatcher and runtime status/dashboard aggregator, with `GatewayState` holding session registry, trigger store, trigger run store, session input store, and runtime task state.
  - `gateway/webhook.rs`, `gateway/runner.rs`, and `gateway/session_input.rs` already provide durable queues and histories, but they persist mutable JSON projections and warn/return empty on corrupt JSON. Level 3 loop state must be stricter.
  - `AgentSession` is already a mature per-turn execution loop: context prep, model calls, tool rounds, loop guard, auto-continuation, verification, final summary, cancel, and snapshot restore all live there. Level 3 should wrap this loop with durable ownership instead of rewriting it first.
  - `AgentSessionSnapshot` remains the authority for session recovery, transcript history, pending confirmations, active tool descriptors, `GoalLedger`, and `AgentA2ABus` state.
  - `AgentA2ABus` already has child roles, execution modes, permissions, leases, attempts, artifacts, worktree review metadata, and projection fields. It is the subagent-domain ledger, not the gateway-level workflow owner.
  - `TaskManager` / `StatusBar` already consume A2A, scheduler, and health alert projections for background task status. Loop tasks can plug into this surface later without inventing a new dashboard first.
- The remaining unchecked roadmap work is concentrated in true runtime parity:
  - Tauri force-quit/reopen WebDriver smoke.
  - Subagent parent/child lineage.
  - Dedicated subagent stream events.
  - Live file IO stream.
  - Token/cost telemetry.
  - Workbench views backed by live file/cost streams.
  - Subagent lineage persistence and worker lifecycle coverage.

## Level 3 Definition

Forge reaches Level 3 when:

1. A loop task can be created, resumed, canceled, and inspected through the gateway without relying on a single desktop UI process as the only owner.
2. Each loop task has a durable ledger: goal, plan, policy, budget, execution lease, events, verification attempts, review decisions, and final outcome.
3. Subagent work emits first-class runtime events for lifecycle, file IO, usage/cost, review, interruption, and failure.
4. The loop has a structured completion contract and stops because the contract passed, failed, was canceled, or needs human review.
5. Policy gates cover intent-level permissions such as editing runtime code, running tests, installing dependencies, committing, pushing, and destructive filesystem actions.
6. UI surfaces consume runtime facts instead of reconstructing state from artifacts or transcript heuristics.

## Level 3 MVP Boundary

The Level 3 MVP claim is deliberately narrow:

- **MVP includes:** durable loop ledger, review/control surfaces, completion contract evaluation, and runtime facts that later runner/protocol/UI work can consume.
- **MVP does not include:** gateway automatically recovering and continuing a complete agent coding loop after a crash.
- `resume_loop_task` first-version semantics: mark the task `interrupted` or `waiting_for_input` and require an explicit user/runtime decision before execution continues. It must not silently resume side effects.
- Gateway-created loop tasks first bind to an existing desktop session/session owner. They must not default to spawning a headless `AgentSession`.
- File IO facts now include reliable worktree/diff boundaries, A2A child file-ish facts, and direct ToolExecutor file-ish `file_io` events. They still must not infer Shell-internal file effects.
- Token/cost fields are required in the contract, but unknown values are valid as `null` plus explicit unknown flags. Do not infer precise cost when an adapter did not provide it.
- Commit is always a human gate. A satisfied completion contract can make a task eligible for review, but it must not auto-commit.

## Runtime Invariants

These invariants make the work defensible as agent engineering rather than a larger task tracker:

1. **Events are authoritative.** `LoopEventJournal` is the source of truth for loop state. `LoopTaskRecord` is a projection/cache that must be rebuildable from events.
2. **Replay restores state, not side effects.** Replaying a loop journal recreates task/gate/budget/evidence state, but never re-runs model calls, shell commands, file writes, or service actions.
3. **Every side effect has an idempotency key.** Model calls, tool calls, shell commands, file writes, review decisions, and commit/push attempts must be recorded with a stable idempotency key before they can be retried or resumed.
4. **Every decision is attributable.** Policy, budget, human gate, completion, and runner decisions record actor, lease/attempt, correlation id, timestamp, and rationale.
5. **Policy precedes execution.** A runner cannot start a side-effecting action until policy and budget preflight have emitted an auditable decision.
6. **Human gates are durable.** Any required approval/input/review/budget override survives restart as a first-class record, not as a transient UI state.
7. **Observability is not optional.** Runtime events must connect loop task, session, turn, A2A child task, worktree, runner attempt, and trace/span identifiers when available.
8. **Repo plan is canonical.** This file is the canonical engineering plan. Obsidian mirrors strategy, handoff state, and decisions for product memory.

## Source of Truth and Obsidian Deliverable

Engineering truth lives in this repo plan, code, and tests. The Obsidian mirror is not a source of truth for implementation details, but every milestone must sync a narrative update there before the slice is considered fully closed.

Each Obsidian milestone note must be interview/backing ready:

- **Current state:** what Forge can do now.
- **What changed:** the runtime contract or product boundary that moved.
- **Why it matters:** the Level 3 engineering claim it supports.
- **Evidence/tests:** exact tests, acceptance gates, or commits that back the claim.
- **Not claimed:** explicit limits, especially around autonomous resume, headless agents, shell-internal file IO, precise cost, and commit automation.
- **Interview-ready explanation:** a short explanation a human can use to describe the architecture without overstating it.

## Non-Goals

- Do not introduce cloud execution, teams, or multi-user collaboration.
- Do not auto-merge worktree worker changes into the main workspace.
- Do not add a new frontend redesign before the runtime event/ledger contracts exist.
- Do not add external dependencies for WebDriver, usage metering, or service supervision without an explicit implementation slice.
- Do not make memory embeddings or full WikiMemoryStore migration a prerequisite for Level 3 runtime ownership.

## File Map

Likely files to create:

- `apps/desktop/src-tauri/src/loop_runtime/mod.rs` — module boundary for loop task ledger, completion contracts, policy, budget, runner state, and event helpers.
- `apps/desktop/src-tauri/src/loop_runtime/types.rs` — durable serde types for loop tasks and event records.
- `apps/desktop/src-tauri/src/loop_runtime/journal.rs` — append-only JSONL authority under `~/.forge/loop-events.jsonl`.
- `apps/desktop/src-tauri/src/loop_runtime/projection.rs` — rebuildable task projection and cache under `~/.forge/loop-tasks.json`.
- `apps/desktop/src-tauri/src/loop_runtime/store.rs` — shared persistence helpers for projection cache and typed store errors.
- `apps/desktop/src-tauri/src/loop_runtime/completion.rs` — completion contract evaluator.
- `apps/desktop/src-tauri/src/loop_runtime/policy.rs` — intent-level loop policy and action decisions.
- `apps/desktop/src-tauri/src/loop_runtime/budget.rs` — usage, elapsed-time, model, and tool-call budget accounting.
- `apps/desktop/src-tauri/src/loop_runtime/runner.rs` — gateway-owned loop runner MVP and lease transitions.
- `apps/desktop/src-tauri/src/loop_runtime/events.rs` — helpers for emitting loop/subagent runtime events.
- `apps/desktop/src-tauri/src/ipc/loop_runtime_handlers.rs` — desktop IPC wrappers for loop tasks.
- `apps/desktop/src/components/loop/LoopTaskPanel.tsx` — compact runtime task inspector once contracts exist.
- `apps/desktop/src/lib/loopRuntime.ts` — frontend query/mutation wrappers.
- `apps/desktop/src/lib/loopRuntime.test.ts` — pure helper coverage for projections and completion labels.

Likely files to modify:

- `apps/desktop/src-tauri/src/gateway/protocol.rs` — JSON-RPC request/response types for loop tasks.
- `apps/desktop/src-tauri/src/gateway/server.rs` — dispatch for loop task methods.
- `apps/desktop/src-tauri/src/gateway/dashboard.rs` — include loop task snapshot in dashboard after ledger/projection tests are green.
- `apps/desktop/src-tauri/src/bin/gateway.rs` — start runner loop after store initialization.
- `apps/desktop/src-tauri/src/protocol/events.rs` — add first-class loop/subagent runtime stream events.
- `apps/desktop/src/lib/protocol.ts` — mirror protocol event types.
- `apps/desktop/src/store/index.ts` — store loop runtime projections.
- `apps/desktop/src-tauri/src/agent/a2a/types.rs` — add parent/child lineage and runtime event ids.
- `apps/desktop/src-tauri/src/agent/a2a/bus.rs` — persist lineage and append runtime telemetry.
- `apps/desktop/src-tauri/src/agent/a2a/child.rs` — emit live worktree worker lifecycle, file IO, and usage records.
- `apps/desktop/src-tauri/src/agent/a2a/supervisor.rs` — map worker records into loop task events.
- `apps/desktop/src-tauri/src/agent/sub.rs` — propagate usage/tool stats from child runs where available.
- `apps/desktop/src/components/messages/AgentA2ATimeline.tsx` — read live event-backed file/cost sections.
- `apps/desktop/src/components/tasks/TaskManager.tsx` — show loop tasks as background runtime tasks.
- `apps/desktop/e2e/acceptance.spec.ts` — extend product smoke once each runtime surface exists.
- `scripts/acceptance.sh` and `scripts/acceptance.test.mjs` — advertise the new Level 3 runtime gates.
- `README.md`, `apps/desktop/README.md`, and `CHANGELOG.md` — document visible runtime changes.

## Source-of-Truth Matrix

Do not let Level 3 create another ambiguous state pile. Each current Forge subsystem keeps its own local facts, while loop runtime records cross-domain orchestration facts:

| Domain | Source of Truth | Loop Runtime Relationship |
|---|---|---|
| Loop workflow state | `LoopEventJournal` | Authority for workflow status, gates, runner leases, policy decisions, budget decisions, and completion decisions |
| Loop task list/dashboard | `LoopTaskProjection` / `LoopTaskRecord` | Rebuildable projection from the event journal; never the authority |
| Transcript and live session state | `AgentSessionSnapshot` + transcript events | Referenced by `session_id`; loop runtime does not rewrite transcript truth or replay transcript side effects |
| Session-local plan progress | `GoalLedger` | May be linked as evidence or progress signal; not the loop workflow authority |
| Subagent tasks/artifacts/review | `AgentA2ABus` / `~/.forge/a2a/<session>.json` | Referenced by `a2a_task_id`; subagent runtime events bridge into loop events without replacing the A2A ledger |
| Scheduler/webhook/headless runs | `PendingTrigger` + `TriggerRunStore` | Triggers can create or correlate loop tasks; trigger history remains trigger-domain truth |
| Gateway session input | `SessionInputStore` + completion history | Input queue remains a delivery mechanism to session owners; loop runtime records why input was enqueued |
| Settings/memory/profile | Existing profile/memory stores | Referenced as inputs; loop runtime records which profile/context was used |

Conflict rule: the loop journal wins for workflow orchestration state; domain stores win for their own artifacts. If a projection disagrees with its source, rebuild the projection.

Important migration rule: existing gateway stores are projection-style JSON files that may warn and continue on corrupt input. `LoopEventJournal` intentionally differs: corrupt journal lines block mutation until repaired or quarantined because Level 3 cannot silently drop workflow truth.

## Data Contracts

Start with additive versioned types. Use optional fields for provider-specific telemetry, but make event envelopes strict.

### Event Envelope

Every durable event is wrapped in an envelope:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopEventEnvelope {
    pub schema_version: u32,
    pub event_id: String,
    pub task_id: String,
    pub sequence: u64,
    pub created_at_ms: u64,
    pub actor: LoopActor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub event: LoopRuntimeEvent,
}
```

`event_id` is unique. `sequence` is monotonic per task. `idempotency_key` prevents duplicate command/evidence/gate effects. `causation_id` points to the event that caused this event; `correlation_id` groups one user-visible workflow, session input, trigger run, or review cycle.

### Task Projection

`LoopTaskRecord` is a projection. It can be cached in `~/.forge/loop-tasks.json`, but it must be rebuildable from `~/.forge/loop-events.jsonl`.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoopTaskStatus {
    Pending,
    Running,
    WaitingForReview,
    WaitingForInput,
    Completed,
    Failed,
    Canceled,
    Interrupted,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopTaskRecord {
    pub id: String,
    pub goal: String,
    pub session_id: Option<String>,
    pub profile_id: Option<String>,
    pub workspace_path: Option<String>,
    pub status: LoopTaskStatus,
    pub owner: LoopTaskOwner,
    pub policy: LoopPolicy,
    pub budget: LoopBudget,
    pub completion_contract: LoopCompletionContract,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub lease: Option<LoopTaskLease>,
    pub latest_event_id: Option<String>,
    pub outcome: Option<LoopTaskOutcome>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_gates: Vec<HumanGateRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_decisions: Vec<PolicyDecisionRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_budget_snapshot: Option<BudgetSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_result: Option<LoopCompletionResult>,
}
```

### Completion Contract

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopCompletionContract {
    pub required_checks: Vec<String>,
    pub max_gitnexus_risk: Option<String>,
    pub require_docs: bool,
    pub require_commit: bool,
    pub require_review_decision: bool,
    pub stop_on_budget_exceeded: bool,
}
```

`required_checks` names human-readable gates. The evaluator must require typed evidence records for those names; string names are not evidence.

### Event Payloads

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoopRuntimeEvent {
    TaskCreated { task_id: String },
    TaskStarted { task_id: String, lease: LoopTaskLease },
    PlanUpdated { task_id: String, summary: String },
    SubagentStarted { task_id: String, child_task_id: String, role: String },
    SubagentFileIo { task_id: String, child_task_id: String, path: String, operation: String },
    UsageRecorded { task_id: String, model: Option<String>, input_tokens: Option<u64>, output_tokens: Option<u64>, estimated_cost_micros: Option<u64> },
    VerificationStarted { task_id: String, command: String },
    VerificationFinished { task_id: String, command: String, success: bool },
    EvidenceRecorded { task_id: String, evidence: EvidenceRecord },
    HumanGateRequested { task_id: String, gate: HumanGateRecord },
    HumanGateDecided { task_id: String, gate_id: String, decision: HumanGateDecision },
    PolicyDecisionRecorded { task_id: String, decision: PolicyDecisionRecord },
    BudgetSnapshotRecorded { task_id: String, snapshot: BudgetSnapshot },
    CompletionEvaluated { task_id: String, result: LoopCompletionResult },
    ReviewRequested { task_id: String, reason: String },
    TaskCompleted { task_id: String },
    TaskFailed { task_id: String, reason: String },
    TaskCanceled { task_id: String, reason: Option<String> },
    TaskInterrupted { task_id: String, reason: String },
    TaskTransitionRejected { task_id: String, from: String, to: String, reason: String },
}
```

`TaskStarted` must carry the full `LoopTaskLease`, not only a `lease_id`, so replay can rebuild the projected lease without consulting another store. The envelope-level `lease_id` remains attribution metadata for events emitted while a lease is active.

### Evidence, Gates, Policy, and Budget

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EvidenceRecord {
    Command {
        evidence_id: String,
        check_name: String,
        command: String,
        exit_code: i32,
        success: bool,
        artifact_hash: Option<String>,
    },
    GitNexus {
        evidence_id: String,
        risk: String,
        changed_symbols: u32,
        affected_processes: u32,
        report_hash: Option<String>,
    },
    Commit {
        evidence_id: String,
        commit_sha: String,
        summary: String,
    },
    Docs {
        evidence_id: String,
        paths: Vec<String>,
    },
    Review {
        evidence_id: String,
        gate_id: String,
        decision: HumanGateDecision,
    },
}
```

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanGateRecord {
    pub gate_id: String,
    pub task_id: String,
    pub gate_type: HumanGateType,
    pub requested_action: String,
    pub requested_by: LoopActor,
    pub status: HumanGateStatus,
    pub allowed_decisions: Vec<HumanGateDecisionKind>,
    pub created_at_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub decision: Option<HumanGateDecision>,
}
```

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyDecisionRecord {
    pub decision_id: String,
    pub intent: LoopActionIntent,
    pub allowed: bool,
    pub reason: String,
    pub actor: LoopActor,
    pub created_at_ms: u64,
}
```

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetSnapshot {
    pub model_rounds: u32,
    pub tool_calls: u32,
    pub elapsed_ms: u64,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub estimated_cost_micros: Option<u64>,
    pub has_unknown_token_usage: bool,
    pub has_unknown_cost: bool,
}
```

Supporting types must be defined before Task 2 so later tests do not invent names:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoopActor {
    Gateway,
    Desktop,
    Runner { runner_id: String },
    User { source: String },
    Subagent { a2a_task_id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoopTaskOwner {
    Gateway,
    Session { session_id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopTaskLease {
    pub lease_id: String,
    pub owner_pid: Option<u32>,
    pub acquired_at_ms: u64,
    pub expires_at_ms: u64,
    pub heartbeat_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoopTaskOutcome {
    Completed { summary: String },
    Failed { reason: String },
    Canceled { reason: Option<String> },
    Interrupted { reason: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopPolicy {
    pub mode: String,
    pub allow_workspace_reads: bool,
    pub allow_test_and_doc_edits: bool,
    pub allow_runtime_edits: bool,
    pub allow_dependency_install: bool,
    pub allow_commit: bool,
    pub allow_push: bool,
    pub allow_destructive_filesystem: bool,
    pub allow_service_lifecycle: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopBudget {
    pub max_model_rounds: u32,
    pub max_tool_calls: u32,
    pub max_elapsed_ms: u64,
    pub max_estimated_cost_micros: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoopActionIntent {
    ReadWorkspace { path: String },
    EditDocs { path: String },
    EditTests { path: String },
    EditRuntimeCode { path: String },
    InstallDependency { package: String },
    RunCommand { command: String },
    CommitChanges,
    PushBranch,
    DestructiveFilesystem { path: String, operation: String },
    ServiceLifecycle { action: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopPolicyDecision {
    pub allowed: bool,
    pub reason: String,
    pub required_gate_type: Option<HumanGateType>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HumanGateType {
    PolicyOverride,
    BudgetOverride,
    UserInput,
    ReviewDecision,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HumanGateStatus {
    Open,
    Decided,
    Expired,
    Canceled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HumanGateDecisionKind {
    Approve,
    Reject,
    ProvideInput,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanGateDecision {
    pub kind: HumanGateDecisionKind,
    pub decided_by: LoopActor,
    pub message: Option<String>,
    pub decided_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoopCompletionStatus {
    Complete,
    Blocked,
    WaitingForReview,
    FailedBudget,
    FailedRisk,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopCompletionResult {
    pub status: LoopCompletionStatus,
    pub reasons: Vec<String>,
}
```

## Milestone Overview

| Milestone | Purpose | Main Result |
|---|---|---|
| 0 | Contract discovery and guardrails | Frozen Level 3 contracts and impact map |
| 0.5 | Source-of-truth and state machine freeze | Event envelope, state machine, evidence/gate/policy primitives |
| 1A | Append-only loop event journal | Durable event history, idempotency, projection rebuild |
| 1B | Projection and gateway RPC | Gateway can create/list/get/cancel tasks through event-producing RPC |
| 2 | Policy, budget, and human gates | Preflight decisions and durable gates exist before runner side effects |
| 3 | Completion evaluator and typed evidence | Loop stop reasons are based on auditable evidence |
| 3.5 | Runtime contract reconciliation | Current code is reconciled with the frozen durable contract before protocol/UI work |
| 4 | Subagent runtime event protocol | A2A lifecycle/file/usage events become first-class |
| 5 | Live IO and usage ledger | Workbench no longer depends only on diff-derived summaries |
| 6 | Gateway-owned runner MVP | Gateway owns queued loop task leases, stale interruption, and runner status |
| 7 | UI/dashboard consumption | Desktop and dashboard read runtime facts |
| 8 | Acceptance, crash/replay, and docs | `scripts/acceptance.sh` proves Level 3 runtime behavior end-to-end |

## Refinement Strategy

The first execution pass should not try to make the gateway run autonomous coding loops. It should establish the durable runtime substrate that later runner work can trust. Split implementation into independently reviewable commits:

1. **Source-of-truth freeze:** define which existing store owns which fact and which facts only project into loop runtime.
2. **Event journal:** add `LoopEventJournal` as append-only authority with envelope, sequence, idempotency, and corruption behavior. The gateway is the only writer in the first slice.
3. **Projection rebuild:** derive `LoopTaskRecord` from events; treat `loop-tasks.json` as rebuildable cache.
4. **Gateway RPC:** create/list/get/cancel loop tasks by appending events, not mutating task state directly.
5. **Policy/gate/evidence before runner:** implement preflight and durable human gates before the gateway claims side-effecting work.
6. **Status and docs:** expose counts in runtime/dashboard snapshots only after the event journal and projection are green. Do not add UI event types just to display an incomplete projection.

This keeps the first slice small enough to review, while leaving a real runtime artifact for later completion, policy, runner, and UI work.

## Implementation Status as of 2026-06-18

Tasks 1-8 have landed in the current implementation line:

| Task | Commit | Status |
|---|---|---|
| Task 1: durable loop task ledger | `3a1f3bf feat(runtime): add durable loop task ledger` | Implemented/committed |
| Task 2: loop policy and human gates | `2b8e2c6 feat(runtime): add loop policy and human gates` | Implemented/committed |
| Task 3: completion contracts | `f9c67d4 feat(runtime): evaluate loop completion contracts` | Implemented/committed |
| Task 3.5: runtime contract reconciliation | `ea693c6 feat(runtime): reconcile loop runtime contract` | Implemented/committed |
| Task 4: subagent runtime event protocol | `844ccf6 feat(runtime): add subagent runtime event protocol` | Implemented/committed |
| Task 5: live IO and usage ledger | `a3a1965 feat(runtime): record subagent file and usage telemetry` | Implemented/committed |
| Task 6: gateway-owned runner MVP | `f8ce269 feat(runtime): add gateway loop runner leases` | Implemented/committed |
| Task 7: UI/dashboard consumption | `47bf27a feat(desktop): surface loop runtime tasks` | Implemented/committed |
| Task 8: acceptance, crash/replay, and docs | `d493421 feat(runtime): verify level 3 loop runtime` | Implemented/committed |

The TDD and commit steps below are now historical execution records. They remain
in the plan as implementation evidence, but there is no active Task 3.5-8
instruction left to restart.

2026-06-18 Phase 4-I follow-up: the final acceptance plan now includes the
real Rust worktree worker lifecycle harness as `live worktree worker lifecycle
harness`, running `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
agent::a2a::child::tests::run_worktree_worker --lib`. This promotes existing
`ChildAgentRuntime::run_worktree_worker` success and already-in-use human-review
coverage into reproducible acceptance evidence without changing the Level 3 MVP
boundary.

2026-06-18 Phase 4-J follow-up: A2A child runtime calls now pass a task-aware
runtime context from `AgentSession::execute_tools` into read-only,
patch-proposal, and worktree-worker children. When that context is present,
`SubAgent::run_with_mode` emits `subagent_runtime_event` `started`, successful
file-ish tool `file_io`, `ended`, and failure facts using the parent
`AgentSession.id` plus the assigned A2A task id. This is a narrow child-runtime
bridge only; it does not trace shell-internal file effects or claim broad
executor-level live IO.

Known Level 3 MVP boundaries after Task 8:

- Gateway automatic recovery and continuation of a full agent coding loop remains future work.
- Gateway-created loop tasks still bind to existing desktop session/session owner; no default headless `AgentSession` is claimed.
- File IO now includes worktree/diff boundary telemetry, A2A child runtime facts for successful file-ish tool calls, and direct ToolExecutor file-ish `file_io` stream facts after successful read/write/edit/git diff/list/search operations.
- Shell-internal file effects and deeper executor instrumentation beyond those direct ToolExecutor file-ish operations remain unclaimed.
- The worktree worker lifecycle has a real Rust harness acceptance gate, but no Tauri/WebDriver force-quit harness is claimed.
- Token/cost can be unknown/null with explicit unknown flags.
- Commit remains human-gated and is never automatic after contract satisfaction.

---

## Task 0: Contract Discovery and Risk Map

**Files:**
- Modify: `docs/superpowers/plans/2026-06-16-level-3-agent-loop-runtime.md`
- Read: `apps/desktop/src-tauri/src/gateway/protocol.rs`
- Read: `apps/desktop/src-tauri/src/gateway/server.rs`
- Read: `apps/desktop/src-tauri/src/agent/a2a/types.rs`
- Read: `apps/desktop/src-tauri/src/agent/a2a/bus.rs`
- Read: `apps/desktop/src-tauri/src/agent/a2a/child.rs`
- Read: `apps/desktop/src-tauri/src/protocol/events.rs`
- Read: `apps/desktop/src/lib/protocol.ts`

- [x] **Step 0.1: Query existing runtime flows with GitNexus**

Run:

```bash
npx gitnexus analyze
```

Only run this if any GitNexus tool warns the index is stale.

Then run MCP/GitNexus queries for:

```text
gateway session input runtime status attach_session task runner dashboard_snapshot
agent a2a worktree worker AgentA2ABus AgentTaskRecord review_agent_a2a_tasks
StreamEvent DiagnosticsUpdate HealthAlert agent_a2a_updated
```

Expected: list of relevant symbols and processes, especially gateway server dispatch, A2A bus, child worktree worker, protocol events, and frontend protocol mirror.

- [x] **Step 0.2: Run impact checks before any symbol edit**

Before editing each symbol, run the corresponding upstream impact:

```text
gitnexus_impact(repo: "forge", target: "dispatch", file_path: "apps/desktop/src-tauri/src/gateway/server.rs", direction: "upstream")
gitnexus_impact(repo: "forge", target: "AgentA2ABus", file_path: "apps/desktop/src-tauri/src/agent/a2a/bus.rs", direction: "upstream")
gitnexus_impact(repo: "forge", target: "run_worktree_worker", file_path: "apps/desktop/src-tauri/src/agent/a2a/child.rs", direction: "upstream")
gitnexus_impact(repo: "forge", target: "StreamEvent", file_path: "apps/desktop/src-tauri/src/protocol/events.rs", direction: "upstream")
```

Expected: HIGH risk is likely for protocol/session symbols. If HIGH or CRITICAL appears, report it before edits and widen tests.

- [x] **Step 0.3: Freeze public contract names**

Confirm these public method names before implementation:

```text
Gateway JSON-RPC:
- create_loop_task
- list_loop_tasks
- get_loop_task
- cancel_loop_task
```

Defer these names until the matching milestone:

```text
Internal helper only in Task 1:
- LoopEventJournal::append_idempotent

Gateway JSON-RPC in Task 3:
- evaluate_loop_task_completion

Stream events in Task 4/7:
- loop_runtime_updated
- subagent_runtime_event
```

`append_loop_task_event` is not public in Phase 1. External callers must use typed RPCs so they cannot append arbitrary event payloads.

Expected: no product code changed in Task 0 unless this plan is updated.

---

## Task 0.5: Source-of-Truth and State Machine Freeze

**Files:**
- Modify: `docs/superpowers/plans/2026-06-16-level-3-agent-loop-runtime.md`
- Modify: `/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Level 3 Agent Loop Runtime Plan.md`
- Read: `apps/desktop/src-tauri/src/agent/snapshot.rs`
- Read: `apps/desktop/src-tauri/src/agent/a2a/bus.rs`
- Read: `apps/desktop/src-tauri/src/agent/a2a/ledger.rs`
- Read: `apps/desktop/src-tauri/src/gateway/runner.rs`
- Read: `apps/desktop/src-tauri/src/gateway/session_input.rs`
- Read: `apps/desktop/src-tauri/src/gateway/server.rs`

- [x] **Step 0.5.1: Write the source-of-truth matrix into this plan**

Confirm the matrix in `Source-of-Truth Matrix` matches code reality:

```text
LoopEventJournal: workflow orchestration facts.
LoopTaskProjection: rebuildable workflow view.
AgentSessionSnapshot: transcript/session recovery facts.
GoalLedger: session-local plan facts.
AgentA2ABus: subagent task/artifact/review facts.
TriggerRunStore: scheduler/webhook/headless run facts.
Profile/memory stores: context/profile input facts.
```

Expected: no runtime code changes.

- [x] **Step 0.5.2: Freeze task state transitions**

Document this state machine before implementing `LoopEventJournal`:

```text
pending -> running
pending -> canceled
running -> waiting_for_input
running -> waiting_for_review
running -> completed
running -> failed
running -> interrupted
waiting_for_input -> running
waiting_for_input -> canceled
waiting_for_review -> running
waiting_for_review -> canceled
waiting_for_review -> completed
interrupted -> pending
interrupted -> canceled
```

Invalid transitions must produce an auditable `TaskTransitionRejected` event or gateway error; they must not mutate projection directly.

- [x] **Step 0.5.3: Freeze event envelope and idempotency rules**

Record:

```text
event_id: unique v7 uuid
task_id: stable loop task id
sequence: monotonic per task
idempotency_key: required for create/cancel/gate decision/evidence/side effects
causation_id: event that caused this event
correlation_id: user turn, trigger run, session input, or review cycle
```

Expected: subsequent tasks use these names exactly.

- [x] **Step 0.5.4: Decide canonical docs**

Write this rule into both repo plan and Obsidian note:

```text
Canonical implementation plan: docs/superpowers/plans/2026-06-16-level-3-agent-loop-runtime.md
Obsidian role: strategy, decision log, and handoff summary
```

Expected: Obsidian remains useful for product memory without drifting into an alternate engineering source of truth.

- [x] **Step 0.5.5: Freeze Phase 1 implementation boundary**

Record these boundaries before code:

```text
Gateway owns LoopEventJournal and LoopTaskProjection.
Gateway is the only writer to loop-events.jsonl in Phase 1.
AgentSession remains the per-turn execution loop.
AgentSessionSnapshot remains session recovery authority.
AgentA2ABus remains subagent task/artifact/review authority.
PendingTrigger/TriggerRunStore remain trigger-domain authority.
SessionInputStore remains delivery queue authority.
TaskManager/StatusBar are not modified until loop projection is stable.
```

Expected: implementation cannot drift into runner, UI, or protocol event work during Task 1A.

---

## Task 1A: Append-Only Loop Event Journal and Projection

> **Historical status:** Implemented/committed as part of `3a1f3bf feat(runtime): add durable loop task ledger`. The TDD steps, expected failures, and commit command in this section are archival context only. Do not execute this task again.

**Files:**
- Create: `apps/desktop/src-tauri/src/loop_runtime/mod.rs`
- Create: `apps/desktop/src-tauri/src/loop_runtime/types.rs`
- Create: `apps/desktop/src-tauri/src/loop_runtime/store.rs`
- Create: `apps/desktop/src-tauri/src/loop_runtime/journal.rs`
- Create: `apps/desktop/src-tauri/src/loop_runtime/projection.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/protocol.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/server.rs`
- Test: `apps/desktop/src-tauri/src/loop_runtime/journal.rs`
- Test: `apps/desktop/src-tauri/src/loop_runtime/projection.rs`

### Phase 1 Detailed Contract

**Authority file:** `~/.forge/loop-events.jsonl`

Each line is one `LoopEventEnvelope`. This file is append-only and authoritative.

**Projection cache:** `~/.forge/loop-tasks.json`

Use a versioned wrapper for the projection cache. It is rebuildable from the event journal and may be deleted if corrupt:

```json
{
  "schema_version": 1,
  "tasks": [
    {
      "id": "loop-018f0a...",
      "goal": "Cover profile settings acceptance",
      "session_id": null,
      "profile_id": null,
      "workspace_path": "/Users/cabbos/project/forge",
      "status": "pending",
      "owner": {
        "kind": "gateway"
      },
      "policy": {
        "mode": "background_task",
        "allow_workspace_reads": true,
        "allow_test_and_doc_edits": true,
        "allow_runtime_edits": false,
        "allow_dependency_install": false,
        "allow_commit": false,
        "allow_push": false,
        "allow_destructive_filesystem": false,
        "allow_service_lifecycle": false
      },
      "budget": {
        "max_model_rounds": 40,
        "max_tool_calls": 120,
        "max_elapsed_ms": 7200000,
        "max_estimated_cost_micros": null
      },
      "completion_contract": {
        "required_checks": [],
        "max_gitnexus_risk": null,
        "require_docs": false,
        "require_commit": false,
        "require_review_decision": false,
        "stop_on_budget_exceeded": true
      },
      "created_at_ms": 1781600000000,
      "updated_at_ms": 1781600000000,
      "lease": null,
      "latest_event_id": null,
      "outcome": null
    }
  ]
}
```

**ID convention:** create task ids as `loop-{uuid::Uuid::now_v7().simple()}`. Use the existing `uuid` dependency; do not add an id package.

**Store semantics:**

- Missing journal means an empty runtime.
- Corrupt journal line is a hard error with line number; gateway mutation RPCs return an error until the journal is repaired or quarantined.
- Corrupt projection cache is non-fatal; rebuild it from the journal.
- Append events with `OpenOptions::append(true).create(true)` and one newline-delimited JSON envelope per write.
- Projection writes use `path.with_extension("tmp")` then `rename`, matching existing gateway/session-input/A2A ledger style.
- Phase 1 assumes a single gateway writer. Desktop, CLI, dashboard, and tests call typed RPC/helpers; they do not write `loop-events.jsonl` directly.
- RPC-originated events use `GatewayRequest.id` as `correlation_id` when no stronger correlation id exists.
- `CreateLoopTaskRequest` accepts an optional `idempotency_key`; when absent, derive one from `rpc:<GatewayRequest.id>`. Duplicate create keys return the original `TaskCreated` effect and task id.
- `create_loop_task` appends `TaskCreated`; it does not directly mutate the projection.
- `cancel_loop_task` appends `TaskCanceled` or returns the existing terminal projection with `changed: false`.
- Duplicate `idempotency_key` for the same task/action returns the existing event effect rather than appending a duplicate event.

**Default task values:**

```rust
LoopTaskStatus::Pending
LoopTaskOwner::Gateway
LoopPolicy::default_for_background_task()
LoopBudget::default_for_background_task()
LoopCompletionContract::default_for_background_task()
lease: None
latest_event_id: None
outcome: None
```

**Gateway JSON-RPC contracts:**

`create_loop_task`

```json
{
  "id": "req-1",
  "method": "create_loop_task",
  "params": {
    "goal": "Cover profile settings acceptance",
    "idempotency_key": "create:profile-settings-acceptance",
    "session_id": null,
    "profile_id": null,
    "workspace_path": "/Users/cabbos/project/forge",
    "policy": null,
    "budget": null,
    "completion_contract": {
      "required_checks": ["build:desktop"],
      "max_gitnexus_risk": "medium",
      "require_docs": true,
      "require_commit": true,
      "require_review_decision": false,
      "stop_on_budget_exceeded": true
    }
  }
}
```

Response:

```json
{
  "id": "req-1",
  "result": {
    "ok": true,
    "task": {
      "id": "loop-018f0a...",
      "status": "pending"
    }
  }
}
```

`list_loop_tasks`

```json
{
  "id": "req-2",
  "method": "list_loop_tasks",
  "params": {
    "statuses": ["pending", "running", "waiting_for_review"],
    "limit": 20
  }
}
```

Response:

```json
{
  "id": "req-2",
  "result": {
    "ok": true,
    "tasks": [],
    "total": 0
  }
}
```

`get_loop_task`

```json
{
  "id": "req-3",
  "method": "get_loop_task",
  "params": {
    "task_id": "loop-018f0a..."
  }
}
```

If the task is missing, return gateway error `-32602` with `loop task not found: <id>`.

`cancel_loop_task`

```json
{
  "id": "req-4",
  "method": "cancel_loop_task",
  "params": {
    "task_id": "loop-018f0a...",
    "reason": "user canceled from dashboard"
  }
}
```

Response:

```json
{
  "id": "req-4",
  "result": {
    "ok": true,
    "changed": true,
    "task": {
      "id": "loop-018f0a...",
      "status": "canceled"
    }
  }
}
```

- [x] **Step 1.1: Write failing event journal and projection tests**

Add tests in `journal.rs` and `projection.rs` for:

```rust
#[test]
fn loop_event_journal_appends_and_replays_created_task() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("loop-events.jsonl");
    let journal = LoopEventJournal::new(path.clone());
    let event = LoopEventEnvelope::task_created_for_test("loop-1", "ship Level 3 runtime");

    journal.append(event.clone()).unwrap();

    let loaded = LoopEventJournal::new(path).load_all().unwrap();
    assert_eq!(loaded, vec![event.clone()]);

    let projection = LoopTaskProjection::from_events(&loaded).unwrap();
    assert_eq!(projection.tasks[0].id, "loop-1");
    assert_eq!(projection.tasks[0].status, LoopTaskStatus::Pending);
}

#[test]
fn corrupt_projection_rebuilds_from_journal() {
    let temp = tempfile::tempdir().unwrap();
    let journal_path = temp.path().join("loop-events.jsonl");
    let projection_path = temp.path().join("loop-tasks.json");
    let journal = LoopEventJournal::new(journal_path);
    journal
        .append(LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime"))
        .unwrap();
    std::fs::write(&projection_path, "{not json").unwrap();

    let projection = LoopTaskProjectionStore::new(projection_path)
        .load_or_rebuild(&journal)
        .unwrap();

    assert_eq!(projection.tasks.len(), 1);
}

#[test]
fn duplicate_idempotency_key_does_not_append_twice() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("loop-events.jsonl");
    let journal = LoopEventJournal::new(path);
    let event = LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime")
        .with_idempotency_key("create:profile-settings-acceptance");

    let first = journal.append_idempotent(event.clone()).unwrap();
    let second = journal.append_idempotent(event).unwrap();

    assert!(first.appended);
    assert!(!second.appended);
    assert_eq!(journal.load_all().unwrap().len(), 1);
}

#[test]
fn journal_assigns_monotonic_sequence_per_task() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("loop-events.jsonl");
    let journal = LoopEventJournal::new(path);

    journal
        .append_idempotent(
            LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime")
                .with_idempotency_key("create:loop-1"),
        )
        .unwrap();
    journal
        .append_idempotent(
            LoopEventEnvelope::plan_updated_for_test("loop-1", "first plan")
                .with_idempotency_key("plan:loop-1:1"),
        )
        .unwrap();

    let loaded = journal.load_all().unwrap();
    assert_eq!(loaded[0].sequence, 1);
    assert_eq!(loaded[1].sequence, 2);
}

#[test]
fn conflicting_idempotency_key_returns_error() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("loop-events.jsonl");
    let journal = LoopEventJournal::new(path);

    journal
        .append_idempotent(
            LoopEventEnvelope::task_created_for_test("loop-1", "first")
                .with_idempotency_key("create:same-key"),
        )
        .unwrap();
    let error = journal
        .append_idempotent(
            LoopEventEnvelope::task_created_for_test("loop-2", "second")
                .with_idempotency_key("create:same-key"),
        )
        .unwrap_err();

    assert!(error.to_string().contains("idempotency conflict"));
    assert_eq!(journal.load_all().unwrap().len(), 1);
}
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime --lib
```

Expected: FAIL because `LoopEventJournal`, `LoopEventEnvelope`, `LoopTaskProjection`, and `LoopTaskStatus` do not exist.

- [x] **Step 1.2: Implement versioned event, task projection, and journal types**

Create the module with:

```rust
pub mod journal;
pub mod projection;
pub mod store;
pub mod types;

pub use journal::LoopEventJournal;
pub use projection::{LoopTaskProjection, LoopTaskProjectionStore};
pub use types::{
    BudgetSnapshot, EvidenceRecord, HumanGateRecord, LoopActor, LoopBudget,
    LoopCompletionContract, LoopEventEnvelope, LoopPolicy, LoopRuntimeEvent,
    LoopTaskLease, LoopTaskOwner, LoopTaskOutcome, LoopTaskRecord, LoopTaskStatus,
    PolicyDecisionRecord,
};
```

Implement:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopTaskProjectionFile {
    pub schema_version: u32,
    pub tasks: Vec<LoopTaskRecord>,
}
```

Implement `LoopEventEnvelope::task_created_for_test`, `LoopEventEnvelope::plan_updated_for_test`, `LoopTaskRecord::new_for_test`, `new_loop_task_id()`, `new_loop_event_id()`, `now_millis()`, append-only journal writes, per-task sequence assignment, idempotency replay/conflict detection, projection rebuild, atomic projection save through write-temp-then-rename, missing-journal-as-empty semantics, corrupt-journal error reporting, and corrupt-projection rebuild.

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime --lib
```

Expected: PASS.

- [x] **Step 1.3: Attach `LoopEventJournal` and projection store to `GatewayState`**

Modify `GatewayState` to hold:

```rust
pub loop_event_journal: Arc<LoopEventJournal>,
pub loop_task_projection_store: Arc<LoopTaskProjectionStore>,
```

Initialize it in `GatewayState::new_with_session_registry_path_and_snapshot_listing` with:

```rust
loop_event_journal: Arc::new(LoopEventJournal::persistent_default()),
loop_task_projection_store: Arc::new(LoopTaskProjectionStore::persistent_default()),
```

Add a test constructor only if needed:

```rust
pub fn new_with_loop_runtime_stores(
    loop_event_journal: Arc<LoopEventJournal>,
    loop_task_projection_store: Arc<LoopTaskProjectionStore>,
) -> Self
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib
```

Expected: PASS with no behavior change for existing gateway methods.

- [x] **Step 1.4: Add gateway JSON-RPC create/list/get/cancel methods**

Add protocol request/response types:

```rust
pub struct CreateLoopTaskRequest {
    pub goal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub profile_id: Option<String>,
    pub workspace_path: Option<String>,
    pub policy: Option<LoopPolicy>,
    pub budget: Option<LoopBudget>,
    pub completion_contract: Option<LoopCompletionContract>,
}

pub struct LoopTaskResponse {
    pub ok: bool,
    pub task: LoopTaskRecord,
}

pub struct ListLoopTasksParams {
    #[serde(default)]
    pub statuses: Vec<LoopTaskStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

pub struct ListLoopTasksResult {
    pub ok: bool,
    pub tasks: Vec<LoopTaskRecord>,
    pub total: usize,
}

pub struct GetLoopTaskParams {
    pub task_id: String,
}

pub struct CancelLoopTaskParams {
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub struct CancelLoopTaskResult {
    pub ok: bool,
    pub changed: bool,
    pub task: LoopTaskRecord,
}
```

Add dispatch for `create_loop_task`, `list_loop_tasks`, `get_loop_task`, and `cancel_loop_task`.

Behavior rules:

- `create_loop_task` appends a `TaskCreated` envelope with caller-provided idempotency key or `rpc:<GatewayRequest.id>`, then rebuilds/saves projection.
- `list_loop_tasks` reads projection through `load_or_rebuild`.
- `get_loop_task` reads projection through `load_or_rebuild`.
- `cancel_loop_task` appends `TaskCanceled` with idempotency key `cancel:<task_id>:<reason_hash>` unless the projection is already terminal.

Validation rules:

- `goal` must not be empty.
- `task_id` must not be empty.
- `limit` defaults to 50 and clamps to 1..200.
- `workspace_path`, `profile_id`, `session_id`, optional idempotency key, and optional reason are trimmed; empty strings become `None`.
- Missing task returns `invalid_params(request.id, format!("loop task not found: {task_id}"))`.

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib
```

Expected: gateway protocol/server tests pass with task methods.

- [x] **Step 1.5: Add gateway protocol/server tests**

Add tests near the existing gateway dispatch tests:

```rust
#[test]
fn create_loop_task_dispatch_persists_task() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Arc::new(LoopEventJournal::persistent_at(dir.path().join("loop-events.jsonl")));
    let projection = Arc::new(LoopTaskProjectionStore::persistent_at(dir.path().join("loop-tasks.json")));
    let state = GatewayState::new_with_loop_runtime_stores(journal.clone(), projection.clone());
    let request = GatewayRequest {
        id: "req-1".to_string(),
        method: "create_loop_task".to_string(),
        params: Some(serde_json::json!({
            "goal": "Ship Level 3 runtime",
            "workspace_path": "/Users/cabbos/project/forge"
        })),
    };

    let GatewayReply::Ok(response) = dispatch(&state, request) else {
        panic!("expected ok response");
    };
    let result: LoopTaskResponse = serde_json::from_value(response.result).unwrap();
    assert!(result.ok);
    assert_eq!(result.task.goal, "Ship Level 3 runtime");
    assert_eq!(journal.load_all().unwrap().len(), 1);
    assert_eq!(projection.load_or_rebuild(&journal).unwrap().tasks.len(), 1);
}
```

Also cover:

```text
create_loop_task rejects empty goal
list_loop_tasks filters by status
get_loop_task rejects missing task
cancel_loop_task is idempotent through dispatch
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway::server --lib
```

Expected: PASS.

- [x] **Step 1.6: Commit Task 1**

Stage the intended changes first:

```bash
git add apps/desktop/src-tauri/src/loop_runtime apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/gateway/protocol.rs apps/desktop/src-tauri/src/gateway/server.rs
```

Before commit, verify the staged scope:

```text
gitnexus_detect_changes(repo: "forge", scope: "staged")
```

Then:

```bash
git commit -m "feat(runtime): add durable loop task ledger"
```

---

## Task 2: Policy, Budget, and Durable Human Gates

> **Historical status:** Implemented/committed as `2b8e2c6 feat(runtime): add loop policy and human gates`. The TDD steps, expected failures, and commit command in this section are archival context only. Later tasks add attribution records without reimplementing policy enforcement.

**Files:**
- Create: `apps/desktop/src-tauri/src/loop_runtime/policy.rs`
- Create: `apps/desktop/src-tauri/src/loop_runtime/budget.rs`
- Create: `apps/desktop/src-tauri/src/loop_runtime/gates.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/mod.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/types.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/journal.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/projection.rs`
- Test: `apps/desktop/src-tauri/src/loop_runtime/policy.rs`
- Test: `apps/desktop/src-tauri/src/loop_runtime/gates.rs`

- [x] **Step 2.1: Write policy preflight tests**

Policy decisions are intent-level and must happen before side effects:

```rust
#[test]
fn loop_policy_blocks_push_without_human_gate() {
    let policy = LoopPolicy::default_for_background_task();
    let decision = policy.decide(LoopActionIntent::PushBranch, &BudgetSnapshot::empty_for_test());

    assert!(!decision.allowed);
    assert_eq!(decision.reason, "push_requires_human_approval");
    assert_eq!(decision.required_gate_type, Some(HumanGateType::PolicyOverride));
}

#[test]
fn loop_policy_allows_docs_edit_inside_workspace() {
    let policy = LoopPolicy::default_for_background_task();
    let decision = policy.decide(
        LoopActionIntent::EditDocs {
            path: "docs/superpowers/plans/plan.md".to_string(),
        },
        &BudgetSnapshot::empty_for_test(),
    );

    assert!(decision.allowed);
    assert_eq!(decision.reason, "allowed_by_background_task_policy");
}
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_policy --lib
```

Expected: FAIL before implementation.

- [x] **Step 2.2: Write durable human gate tests**

Human gates are records emitted through events:

```rust
#[test]
fn human_gate_survives_projection_rebuild() {
    let events = vec![
        LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime"),
        LoopEventEnvelope::human_gate_requested_for_test(
            "loop-1",
            "gate-1",
            HumanGateType::PolicyOverride,
            "Approve dependency install",
        ),
    ];

    let projection = LoopTaskProjection::from_events(&events).unwrap();

    assert_eq!(projection.tasks[0].status, LoopTaskStatus::WaitingForReview);
    assert_eq!(projection.tasks[0].open_gates.len(), 1);
    assert_eq!(projection.tasks[0].open_gates[0].gate_id, "gate-1");
}
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml human_gate --lib
```

Expected: FAIL before implementation.

- [x] **Step 2.3: Implement conservative policy and budget defaults**

Defaults:

```text
read workspace: allowed inside workspace
edit tests/docs: allowed when task contract permits
edit runtime code: requires impact-analysis evidence or human policy override
install dependency: requires human gate
commit: requires completion contract, passing evidence, and a durable human gate
push: requires human gate
destructive filesystem action: requires human gate
service lifecycle action: requires human gate unless update-repair allowlist applies
```

Budget behavior:

```text
If a model call has already started, wait for it to finish.
If a tool call has not started, block it and request a budget gate.
If a long-running shell/tool supports cancellation, allow interrupt.
If budget is exceeded, task becomes waiting_for_review with HumanGateType::BudgetOverride.
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime --lib
```

Expected: PASS.

- [x] **Step 2.4: Commit Task 2**

```bash
git add apps/desktop/src-tauri/src/loop_runtime
```

```text
gitnexus_detect_changes(repo: "forge", scope: "staged")
```

```bash
git commit -m "feat(runtime): add loop policy and human gates"
```

---

## Task 3: Completion Contract Evaluator

> **Historical status:** Implemented/committed as `f9c67d4 feat(runtime): evaluate loop completion contracts`. The TDD steps, expected failures, and commit command in this section are archival context only. Later tasks reconcile completion event recording and non-complete statuses before protocol/UI consumption.

**Files:**
- Create: `apps/desktop/src-tauri/src/loop_runtime/completion.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/mod.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/types.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/protocol.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/server.rs`
- Test: `apps/desktop/src-tauri/src/loop_runtime/completion.rs`

- [x] **Step 3.1: Write evaluator tests**

Cover exact stop reasons:

```rust
#[test]
fn completion_waits_for_required_check() {
    let task = LoopTaskRecord::new_for_test("task-1", "finish runtime")
        .with_completion_contract(LoopCompletionContract {
            required_checks: vec!["build:desktop".to_string()],
            max_gitnexus_risk: Some("medium".to_string()),
            require_docs: true,
            require_commit: true,
            require_review_decision: false,
            stop_on_budget_exceeded: true,
        });

    let result = evaluate_completion(&task, &[]);

    assert_eq!(result.status, LoopCompletionStatus::Blocked);
    assert_eq!(result.reasons, vec!["missing_required_check:build:desktop"]);
}
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml completion --lib
```

Expected: FAIL before implementation.

- [x] **Step 3.2: Implement typed evidence evaluator**

The evaluator must classify:

```text
complete
blocked
waiting_for_review
failed_budget
failed_risk
```

It must not shell out to GitNexus or run tests. It only evaluates `EvidenceRecord` values already recorded in the loop event journal. A string in `required_checks` is only a requirement name; it is not proof.

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime --lib
```

Expected: PASS.

- [x] **Step 3.3: Add gateway evaluation method**

Add `evaluate_loop_task_completion` so desktop, CLI, and dashboard can show why a loop is not done.

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib
```

Expected: PASS.

- [x] **Step 3.4: Commit Task 3**

```bash
git add apps/desktop/src-tauri/src/loop_runtime apps/desktop/src-tauri/src/gateway/protocol.rs apps/desktop/src-tauri/src/gateway/server.rs
```

```text
gitnexus_detect_changes(repo: "forge", scope: "staged")
```

```bash
git commit -m "feat(runtime): evaluate loop completion contracts"
```

---

## Task 3.5: Runtime Contract Reconciliation

**Status:** Implemented in working tree on 2026-06-17; pending review and commit gate. This task was required because Tasks 1-3 landed, but the durable runtime contract drifted from the frozen Level 3 contract.

**Goal:** Reconcile the existing loop runtime ledger/projection/completion code with the minimum durable contract needed by later runner, A2A protocol, and UI work. Do not implement runner ownership, frontend protocol events, A2A stream events, or UI in this task.

**Staging guard:** `docs/superpowers/plans/2026-06-16-eval-runner-optimization-roadmap.md` is unrelated untracked work. Do not stage, commit, edit, rename, or clean that file for Task 3.5.

**Files:**
- Modify: `apps/desktop/src-tauri/src/loop_runtime/types.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/projection.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/journal.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/completion.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/mod.rs`
- Tiny compile fallout: `apps/desktop/src-tauri/src/loop_runtime/gates.rs` test helper only, to fill additive `LoopEventEnvelope` metadata fields in an existing direct struct literal.
- Modify: `docs/superpowers/plans/2026-06-16-level-3-agent-loop-runtime.md`
- Modify: `/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Level 3 Agent Loop Runtime Plan.md`

Do not modify `dispatch`, frontend protocol/store/UI, A2A streaming, or runner files unless a compiler error proves a tiny gateway type export is necessary. If that happens, document it in the implementation report.

- [x] **Step 3.5.1: Run impact checks before editing symbols**

Run upstream impact before changing each existing symbol:

```text
gitnexus_impact(repo: "forge", target: "LoopEventEnvelope", file_path: "apps/desktop/src-tauri/src/loop_runtime/types.rs", direction: "upstream")
gitnexus_impact(repo: "forge", target: "LoopRuntimeEvent", file_path: "apps/desktop/src-tauri/src/loop_runtime/types.rs", direction: "upstream")
gitnexus_impact(repo: "forge", target: "LoopTaskStatus", file_path: "apps/desktop/src-tauri/src/loop_runtime/types.rs", direction: "upstream")
gitnexus_impact(repo: "forge", target: "LoopTaskRecord", file_path: "apps/desktop/src-tauri/src/loop_runtime/types.rs", direction: "upstream")
gitnexus_impact(repo: "forge", target: "from_events", file_path: "apps/desktop/src-tauri/src/loop_runtime/projection.rs", direction: "upstream")
gitnexus_impact(repo: "forge", target: "event_payload_fingerprint", file_path: "apps/desktop/src-tauri/src/loop_runtime/journal.rs", direction: "upstream")
gitnexus_impact(repo: "forge", target: "evaluate_completion", file_path: "apps/desktop/src-tauri/src/loop_runtime/completion.rs", direction: "upstream")
```

Expected: report any HIGH/CRITICAL risk before editing and keep the patch limited to runtime contract reconciliation.

- [x] **Step 3.5.2: Write failing compatibility and projection tests**

Add focused tests before implementation for:

```text
old minimal LoopEventEnvelope JSON without lease_id/attempt/causation_id still deserializes
TaskStarted projects an existing non-terminal task to running and stores the full lease payload
waiting_for_input status projects/replays and completion reports not complete
TaskInterrupted projects to interrupted, clears lease, records outcome, and completion reports not complete
PolicyDecisionRecorded appends decisions, survives projection rebuild, ignores identical duplicate decision_id, and errors on conflicting duplicate decision_id
BudgetSnapshotRecorded updates latest_budget_snapshot
CompletionEvaluated updates completion_result
journal idempotency reuses a semantically identical new event payload and conflicts on a changed payload
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml completion --lib
```

Expected: FAIL for missing fields, variants, projection behavior, and completion behavior.

- [x] **Step 3.5.3: Extend the durable envelope and task projection types**

Reconcile the additive serde contract:

```text
LoopEventEnvelope:
- lease_id: Option<String>
- attempt: Option<u32>
- causation_id: Option<String>

LoopTaskStatus:
- waiting_for_input
- interrupted

LoopTaskRecord projection fields:
- policy_decisions: Vec<PolicyDecisionRecord>
- latest_budget_snapshot: Option<BudgetSnapshot>
```

The new envelope fields must use serde defaults so old JSONL records deserialize when fields are missing. `waiting_for_input` and `interrupted` must not be treated as terminal by default.

- [x] **Step 3.5.4: Extend runtime events without starting Task 4**

Add the minimum durable loop runtime events:

```text
task_started
task_interrupted
policy_decision_recorded
budget_snapshot_recorded
completion_evaluated
```

`task_started` must be represented as `TaskStarted { task_id: String, lease: LoopTaskLease }`. Do not use a `lease_id`-only payload for this event; the projection must store the full lease exactly as recorded in the event payload.

If projection needs a distinct durable event to enter `waiting_for_input`, add the smallest `task_waiting_for_input` event in this same slice and keep it limited to loop runtime replay. Do not add frontend stream events, A2A events, or UI handling here.

Add `PolicyDecisionRecord` with:

```text
decision_id: String
intent: LoopActionIntent
allowed: bool
reason: String
actor: LoopActor
created_at_ms: u64
```

Reuse the existing `LoopActionIntent` from `policy.rs`; do not create a duplicate intent enum.

- [x] **Step 3.5.5: Reconcile projection replay and idempotency**

Projection behavior:

```text
TaskStarted: requires existing non-terminal task, sets running, stores the full LoopTaskLease payload exactly, updates updated_at/latest_event_id.
TaskWaitingForInput, if added: requires existing non-terminal task, sets waiting_for_input, clears lease if appropriate, records local outcome/message if the current pattern needs it, updates updated_at/latest_event_id.
TaskInterrupted: requires existing non-terminal task, sets interrupted, clears lease, records interrupted outcome/message/time, updates updated_at/latest_event_id.
PolicyDecisionRecorded: appends idempotently by decision_id; identical duplicate is ignored; conflicting duplicate errors.
BudgetSnapshotRecorded: updates latest_budget_snapshot.
CompletionEvaluated: updates completion_result.
```

Existing terminal-task ignore behavior must remain for events after terminal states. Journal idempotency fingerprints must cover every new event with stable semantic payloads and must not collapse distinct policy decisions accidentally.

- [x] **Step 3.5.6: Reconcile completion behavior**

Completion must not report complete for new non-terminal statuses:

```text
waiting_for_input -> blocked with reason task_waiting_for_input
interrupted -> blocked with reason task_interrupted
```

Prefer existing `LoopCompletionStatus::Blocked`; do not add a new public completion status unless the API cannot remain coherent.

- [x] **Step 3.5.7: Run narrow verification**

Run:

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml completion --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib
git diff --check
```

Then run:

```text
gitnexus_detect_changes(repo: "forge", scope: "all")
```

Expected: tests pass, diff check is clean, and GitNexus changed-symbol summary is limited to the runtime contract reconciliation slice plus plan docs.

- [x] **Step 3.5.8: Sync Obsidian narrative**

Update `/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Level 3 Agent Loop Runtime Plan.md` with a 2026-06-17 note covering:

```text
current state: Tasks 1-3 implemented at 3a1f3bf, 2b8e2c6, f9c67d4
what changed: Task 3.5 inserted before protocol/UI work to reconcile durable contract drift
why it matters: later runner/A2A/UI work can rely on lease/attempt/causation/status/event/projection facts
evidence/tests: list the Task 3.5 tests and commands run
not claimed: no autonomous gateway resume, no default headless AgentSession, no Shell-internal file effect tracing, no precise token/cost claim, no auto-commit
interview-ready explanation: concise architecture narrative for Level 3 runtime ownership
remaining gaps: runner ownership, A2A correlation, policy enforcement, acceptance gates
```

Expected: repo plan/code/tests remain engineering source of truth; Obsidian is synced as required narrative deliverable.

**Implementation evidence (2026-06-17):**

- Initial red run failed as expected on missing `PolicyDecisionRecord`, envelope metadata fields, `waiting_for_input` / `interrupted` statuses, new event variants, and projection fields.
- Added additive envelope metadata with serde defaults, new non-terminal statuses, `PolicyDecisionRecord`, task policy/budget/completion projection fields, and the minimum durable runtime events.
- Added `task_waiting_for_input` as the smallest replay-only event needed to project `waiting_for_input`.
- Reconciled projection replay for started, waiting-for-input, interrupted, policy decision, budget snapshot, and completion evaluated events.
- Reconciled idempotency fingerprints for every new event without using retry timestamps for policy decision identity.
- Reconciled completion so `waiting_for_input` and `interrupted` return `LoopCompletionStatus::Blocked`.
- Review fix: expanded `BudgetSnapshot` with nullable `input_tokens`, `output_tokens`, existing nullable `estimated_cost_micros`, and explicit `has_unknown_token_usage` / `has_unknown_cost` flags. Old snapshot JSON defaults these flags to unknown.
- Added tests proving old budget snapshot JSON defaults unknown token/cost flags and `BudgetSnapshotRecorded` preserves unknown token/cost facts through projection.
- Verification commands run: `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml completion --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib`, `git diff --check`, and `gitnexus_detect_changes(repo: "forge", scope: "all")`.

---

## Task 4: Subagent Runtime Event Protocol

**Files:**
- Modify: `apps/desktop/src-tauri/src/protocol/events.rs`
- Modify: `apps/desktop/src/lib/protocol.ts`
- Modify: `apps/desktop/src/store/index.ts`
- Modify: `apps/desktop/src/store/types.ts`
- Modify: `apps/desktop/src/store/event-dispatch.ts`
- Create: `apps/desktop/src/store/runtime-projections.ts`
- Modify as needed: `apps/desktop/src-tauri/src/agent/a2a/types.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/bus.rs`
- Test: `apps/desktop/src-tauri/src/agent/a2a/bus.rs`
- Test: `apps/desktop/src/store/blocks.test.ts` or new focused store test

- [x] **Step 4.1: Write Rust protocol serialization tests**

First verify the existing A2A contract:

```text
AgentA2ABus already owns task/artifact/review facts.
AgentA2AProjection already exposes running/completed/failed/interrupted counts, lease data, review data, changed files, and test report excerpts.
agent_a2a_updated remains the current UI projection event.
```

Add tests that prove event names serialize as stable snake_case:

```rust
#[test]
fn subagent_runtime_event_serializes_snake_case() {
    let event = StreamEvent::SubagentRuntimeEvent {
        session_id: "s1".to_string(),
        task_id: "t1".to_string(),
        event: SubagentRuntimePayload::Started { role: "implementer".to_string() },
    };

    let json = serde_json::to_value(event).unwrap();
    assert_eq!(json["event_type"], "subagent_runtime_event");
    assert_eq!(json["event"]["type"], "started");
}
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml subagent_runtime_event --lib
```

Expected: FAIL before implementation.

- [x] **Step 4.2: Implement additive protocol events**

Add events without removing `agent_a2a_updated`:

```rust
SubagentRuntimeEvent {
    session_id: String,
    loop_task_id: Option<String>,
    task_id: String,
    event: SubagentRuntimePayload,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubagentRuntimePayload {
    Started { role: String },
    Status { status: String, message: Option<String> },
    FileIo { path: String, operation: String },
    UsageRecorded { model: Option<String>, input_tokens: Option<u64>, output_tokens: Option<u64>, estimated_cost_micros: Option<u64> },
    Ended { status: String },
    Failed { reason: String },
    Interrupted { reason: String },
}

LoopRuntimeUpdated {
    session_id: String,
    loop_task_id: String,
    task: LoopTaskRecord,
}
```

`StreamEvent` is currently session-scoped. For loop tasks without an attached session, use the stable sentinel `session_id: "gateway"` and handle `loop_runtime_updated` before frontend session lookup, just like global recovery/health style events are handled before transcript block rendering.

Add TypeScript mirror types in `src/lib/protocol.ts`.

Run:

```bash
npm --prefix apps/desktop run build
```

Expected: PASS.

- [x] **Step 4.3: Store runtime events separately from transcript blocks**

Add a runtime projection map keyed by session/task id. Do not render raw events into the transcript.

Run:

```bash
node --test apps/desktop/src/store/blocks.test.ts
npm --prefix apps/desktop run build
```

Expected: PASS.

- [x] **Step 4.4: Commit Task 4**

```bash
git add apps/desktop/src-tauri/src/protocol/events.rs apps/desktop/src/lib/protocol.ts apps/desktop/src/store/index.ts apps/desktop/src-tauri/src/agent/a2a
```

```text
gitnexus_detect_changes(repo: "forge", scope: "staged")
```

```bash
git commit -m "feat(runtime): add subagent runtime event protocol"
```

### 2026-06-17 Task 4 implementation evidence

Task 3.5 has landed at `ea693c6 feat(runtime): reconcile loop runtime contract`. Task 4 is implemented in the working tree as an additive protocol/store slice only; commit remains delegated to the separate commit-gate subagent.

What changed:

- Added Rust `StreamEvent::SubagentRuntimeEvent`, `StreamEvent::LoopRuntimeUpdated`, and `SubagentRuntimePayload` with stable snake_case serde tags.
- Mirrored the new runtime protocol in `apps/desktop/src/lib/protocol.ts`.
- Added frontend runtime projection maps for subagent events and loop task updates, keyed by `session_id:task_id` / `session_id:loop_task_id`.
- Runtime projection updates preserve prior meaningful status/message/reason across telemetry-only `file_io` and `usage_recorded` events; `latest_event` carries the newest telemetry payload.
- The TypeScript `LoopTaskRecord` mirror keeps Rust-required fields required (`policy`, `budget`, `completion_contract`, `created_at_ms`).
- Handled `subagent_runtime_event` and `loop_runtime_updated` in `createOutputEventDispatcher` before transcript block conversion; `loop_runtime_updated` supports the `session_id: "gateway"` sentinel before session lookup.
- Kept `agent_a2a_updated` intact and added an A2A projection contract guard covering counts, lease/attempt state, review metadata, changed files, and test report excerpts.

Evidence/tests:

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml subagent_runtime_event --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib
node --test apps/desktop/src/store/blocks.test.ts
node --test apps/desktop/src/store/event-dispatch.test.mjs
npm --prefix apps/desktop run build
git diff --check
```

Not claimed:

- No actual live file IO emitters existed in this Task 4 protocol-only slice; the later Phase 4-J follow-up adds narrow A2A child file-ish tool facts only.
- No usage ledger aggregation or precise cost metering.
- No runner ownership, gateway autonomous resume, default headless `AgentSession`, UI rendering, acceptance-script updates, or auto-commit behavior.

Historical note: at Task 4 handoff the remaining gaps were Task 5 telemetry
emission, Task 6 runner ownership, Task 7 UI consumption, and Task 8
acceptance/docs. Those slices have since landed in later commits listed in the
implementation status table above.

---

## Task 5: Live File IO and Usage Ledger

**Files:**
- Modify: `apps/desktop/src-tauri/src/loop_runtime/budget.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/types.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/child.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/bus.rs`
- Modify: `apps/desktop/src-tauri/src/agent/sub.rs`
- Test: `apps/desktop/src-tauri/src/agent/a2a/bus.rs`
- Test: `apps/desktop/src-tauri/src/loop_runtime/budget.rs`

- [x] **Step 5.1: Write usage aggregation tests**

Use optional telemetry so adapters can adopt it gradually:

```rust
#[test]
fn usage_ledger_sums_known_tokens_and_preserves_unknown_cost() {
    let usage = LoopUsageLedger::from_events(vec![
        UsageEvent { model: Some("claude".into()), input_tokens: Some(100), output_tokens: Some(50), estimated_cost_micros: None },
        UsageEvent { model: Some("claude".into()), input_tokens: Some(25), output_tokens: None, estimated_cost_micros: Some(10) },
    ]);

    assert_eq!(usage.input_tokens, Some(125));
    assert_eq!(usage.output_tokens, Some(50));
    assert_eq!(usage.estimated_cost_micros, Some(10));
    assert!(usage.has_unknown_output_tokens);
}
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml budget --lib
```

Expected: FAIL before implementation.

- [x] **Step 5.2: Emit file IO events from worktree worker boundaries**

At first, emit events from known boundaries rather than every low-level read:

```text
worktree created
file changed path observed in diff
test report artifact read
worktree preserved or cleaned
```

Expected: honest live-adjacent telemetry. Do not claim low-level executor read/write hooks until executor hooks exist.

- [x] **Step 5.3: Record model/tool usage where already available**

Record:

```text
model
turn count
tool call count
elapsed time
known input/output tokens when adapter exposes them
estimated cost when available
unknown fields as null with explicit has_unknown_* flags
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib
npm --prefix apps/desktop run build
```

Expected: PASS.

- [x] **Step 5.4: Commit Task 5**

```bash
git add apps/desktop/src-tauri/src/loop_runtime apps/desktop/src-tauri/src/agent/a2a apps/desktop/src-tauri/src/agent/sub.rs
```

```text
gitnexus_detect_changes(repo: "forge", scope: "staged")
```

```bash
git commit -m "feat(runtime): record subagent file and usage telemetry"
```

**2026-06-17 Task 5 implementation evidence:** Task 5 adds `LoopUsageLedger` / `UsageEvent` aggregation with explicit unknown input/output/cost flags, records subagent model turn count, tool call count, elapsed time, and unknown token/cost facts in subagent result JSON, and projects worktree boundary file IO from A2A metadata (`worktree_created`, `diff_observed`, `test_report_observed`, `worktree_preserved`, `worktree_cleaned`). This is boundary-level telemetry only: it does not claim executor-level live read/write tracing, precise cost when provider data is absent, runner ownership, autonomous gateway resume, UI rendering, or auto-commit behavior. Evidence: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml budget --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib`, focused adapter usage tests, `npm --prefix apps/desktop run build`, `git diff --check`, and commit `a3a1965 feat(runtime): record subagent file and usage telemetry`.

**2026-06-17 spec review fixes:** Task 5 telemetry now serializes unknown `input_tokens`, `output_tokens`, and `estimated_cost_micros` as explicit `null` values alongside `has_unknown_*` flags; subagents capture known provider usage already emitted via existing adapter `StreamEvent::Usage` paths without changing `StreamResult`; and `diff_observed` file IO now trusts only diff file markers instead of arbitrary indented context lines. Evidence: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml budget --lib` passes with null-serialization coverage, and `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib` passes with known-usage and diff-parser regression coverage.

**2026-06-17 spec re-review fixes:** Subagent usage capture now calls the non-streaming `call_with_emitter` path, preserving Anthropic's subagent-only tool contract while still emitting known provider usage from both Anthropic and OpenAI-compatible adapters. Worktree boundary parsing also restores `## Untracked files:` section support without accepting normal indented unified-diff context lines. Evidence: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib` passes with provider usage and untracked-file parser coverage, and focused adapter tests confirm non-streaming usage emission without broadening the subagent tool surface.

---

## Task 6: Gateway-Owned Runner MVP

**Files:**
- Create: `apps/desktop/src-tauri/src/loop_runtime/runner.rs`
- Modify: `apps/desktop/src-tauri/src/loop_runtime/mod.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/server.rs`
- Modify: `apps/desktop/src-tauri/src/bin/gateway.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/dashboard.rs`
- Test: `apps/desktop/src-tauri/src/loop_runtime/runner.rs`

- [x] **Step 6.1: Write lease transition tests**

Pin the runner state machine:

```rust
#[test]
fn runner_claims_pending_task_with_lease() {
    let mut task = LoopTaskRecord::new_for_test("task-1", "run acceptance");

    let lease = LoopTaskRunner::claim_for_test(&mut task, 1234).unwrap();

    assert_eq!(task.status, LoopTaskStatus::Running);
    assert_eq!(task.lease.as_ref().unwrap().lease_id, lease.lease_id);
    assert_eq!(task.lease.as_ref().unwrap().owner_pid, 1234);
}
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml runner --lib
```

Expected: FAIL before implementation.

- [x] **Step 6.2: Implement queue ownership without full autonomous coding**

Use the existing trigger runner as the pattern, not as the loop runner itself. `gateway/runner.rs` already claims pending triggers with a lease, records attempts, retries, and dead-letters through `TriggerRunStore`; the loop runner should reuse that operational shape while writing loop events instead of mutating task projection directly.

The MVP runner may execute only safe built-in loop actions:

```text
evaluate completion
record waiting_for_input
record waiting_for_review
enqueue session input to an existing owner runtime
mark stale leases interrupted
```

It must not spawn an unconstrained coding agent yet. Gateway-created loop tasks bind to an existing desktop session/session owner in the first version; they do not default to a headless `AgentSession`.

`resume_loop_task` first-version semantics:

```text
record interrupted or waiting_for_input state
surface the recovery reason to the user/runtime
wait for an explicit decision before any side effect continues
```

It must not auto-resume model calls, shell commands, file writes, commits, pushes, or worktree merges. Full autonomous recovery comes after ledger, policy, review gates, and acceptance coverage are stable.

- [x] **Step 6.3: Surface runner status in dashboard/runtime_status**

Add:

```text
loop_runner: started | stopped | failed
pending_loop_tasks
running_loop_tasks
stale_loop_task_leases
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib
```

Expected: PASS.

**2026-06-17 Task 6 implementation evidence:** Task 6 adds `loop_runtime::runner` with a gateway-owned lease state machine that claims pending tasks, records `TaskStarted`, records `TaskWaitingForInput`, evaluates completion after the waiting state is projected, and interrupts stale running leases as `TaskInterrupted`. The gateway daemon now starts the loop runner and reports `loop_runner`, `pending_loop_tasks`, `running_loop_tasks`, and `stale_loop_task_leases` through `runtime_status`, the static dashboard, diagnostics fallback, and `forge_trigger` status rendering. This remains an MVP runner: it does not spawn a headless `AgentSession`, does not auto-resume model calls, does not run shell/file side effects, does not commit/push, and does not merge worktrees. Evidence: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml runner --lib` first failed on missing `LoopTaskRunner`, then passed; `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib` first failed on missing status fields, then passed.

- [x] **Step 6.4: Commit Task 6**

```bash
git add apps/desktop/src-tauri/src/loop_runtime apps/desktop/src-tauri/src/gateway apps/desktop/src-tauri/src/bin/gateway.rs
```

```text
gitnexus_detect_changes(repo: "forge", scope: "staged")
```

```bash
git commit -m "feat(runtime): add gateway loop runner leases"
```

---

## Task 7: Runtime UI and Dashboard Consumption

**Files:**
- Create: `apps/desktop/src/lib/loopRuntime.ts`
- Create: `apps/desktop/src/lib/loopRuntime.test.ts`
- Create: `apps/desktop/src/components/loop/LoopTaskPanel.tsx`
- Modify: `apps/desktop/src/components/tasks/TaskManager.tsx`
- Modify: `apps/desktop/src/components/messages/AgentA2ATimeline.tsx`
- Modify: `apps/desktop/src-tauri/src/gateway/dashboard.rs`
- Test: `apps/desktop/e2e/acceptance.spec.ts`

- [x] **Step 7.1: Write projection helper tests**

Test status labels, blocked reasons, and budget warnings:

```ts
import test from "node:test";
import assert from "node:assert/strict";
import { summarizeLoopTask } from "./loopRuntime";

test("summarizeLoopTask labels missing required checks", () => {
  const summary = summarizeLoopTask({
    status: "waiting_for_review",
    completion_result: {
      status: "blocked",
      reasons: ["missing_required_check:build:desktop"],
    },
  });

  assert.equal(summary.label, "等待验证");
  assert.match(summary.detail, /build:desktop/);
});
```

Run:

```bash
node --test apps/desktop/src/lib/loopRuntime.test.ts
```

Expected: FAIL before helper exists.

- [x] **Step 7.2: Implement compact UI surfaces**

Add UI only after event/ledger contracts exist:

```text
TaskManager/StatusBar: extend the existing background task model with running/waiting loop tasks.
LoopTaskPanel: status, latest event, budget, completion blockers, review requirement.
AgentA2ATimeline: file IO and usage sections backed by runtime events when they exist.
Dashboard: loop task table in static gateway dashboard.
```

Run:

```bash
node --test apps/desktop/src/lib/loopRuntime.test.ts
npm --prefix apps/desktop run build
```

Expected: PASS.

- [x] **Step 7.3: Add mocked acceptance coverage**

Extend `e2e/acceptance.spec.ts` to prove:

```text
loop task appears in background task drawer
completion blockers are visible
review-required state is visible
usage/file event rows render from mocked IPC/protocol data
```

Run:

```bash
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
```

Expected: PASS.

- [x] **Step 7.4: Commit Task 7**

```bash
git add apps/desktop/src/lib/loopRuntime.ts apps/desktop/src/lib/loopRuntime.test.ts apps/desktop/src/components/loop apps/desktop/src/components/tasks/TaskManager.tsx apps/desktop/src/components/messages/AgentA2ATimeline.tsx apps/desktop/src-tauri/src/gateway/dashboard.rs apps/desktop/e2e/acceptance.spec.ts
```

```text
gitnexus_detect_changes(repo: "forge", scope: "staged")
```

```bash
git commit -m "feat(desktop): surface loop runtime tasks"
```

**2026-06-17 Task 7 implementation evidence:** Task 7 is implemented and committed in `47bf27a feat(desktop): surface loop runtime tasks`. The desktop now has `loopRuntime.ts` projection helpers/tests, a compact `LoopTaskPanel`, StatusBar/TaskManager consumption of active-session loop tasks, A2A file IO / usage fact rows backed by `subagent_runtime_event`, and a static gateway dashboard table backed by `loop_tasks` in the dashboard snapshot. The UI keeps the MVP boundary explicit: loop tasks can wait for input/review, completion blockers stay visible, token/cost can be unknown, and commit remains human-gated. Evidence to preserve for the gate: `node --test apps/desktop/src/lib/loopRuntime.test.ts` first failed because `runtimeFactsForSubagentTask` was missing and then passed after the helper was added. Review fixes then made the dashboard reuse one loaded loop projection for both runtime status stats and loop task rows, made compact A2A runtime facts session-scoped, and replaced localized-label background filtering with raw loop task status filtering. Verification passed: `node --test apps/desktop/src/lib/loopRuntime.test.ts`, `node --test apps/desktop/src/lib/backgroundTaskStatus.test.ts`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib`, `npm --prefix apps/desktop run build`, `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts`, `git diff --check`, and staged GitNexus change detection.

At Task 7 time, the UI slice did not claim gateway automatic
recovery/continuation, default headless `AgentSession`, executor-level live
read/write tracing, precise cost when unknown, or auto-commit. Phase 4-K later
adds direct ToolExecutor file-ish `file_io` stream facts only.

---

## Task 8: Acceptance, Crash/Replay, and Documentation

**Files:**
- Create: `apps/desktop/src-tauri/src/loop_runtime/replay_tests.rs`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/superpowers/plans/2026-06-12-forge-hermes-runtime-gap-roadmap.md`
- Modify: `docs/superpowers/plans/2026-06-16-level-3-agent-loop-runtime.md`
- Modify: `/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Level 3 Agent Loop Runtime Plan.md`

- [x] **Step 8.1: Add crash/replay regression tests**

Add tests that make the Level 3 claim credible:

```rust
#[test]
fn projection_rebuilds_after_projection_file_corruption() {
    let temp = tempfile::tempdir().unwrap();
    let journal = LoopEventJournal::persistent_at(temp.path().join("loop-events.jsonl"));
    let projection = LoopTaskProjectionStore::persistent_at(temp.path().join("loop-tasks.json"));

    journal
        .append(LoopEventEnvelope::task_created_for_test("loop-1", "prove replay"))
        .unwrap();
    std::fs::write(temp.path().join("loop-tasks.json"), "{broken").unwrap();

    let rebuilt = projection.load_or_rebuild(&journal).unwrap();

    assert_eq!(rebuilt.tasks[0].id, "loop-1");
}

#[test]
fn waiting_human_gate_survives_replay() {
    let events = vec![
        LoopEventEnvelope::task_created_for_test("loop-1", "install dependency"),
        LoopEventEnvelope::human_gate_requested_for_test(
            "loop-1",
            "gate-1",
            HumanGateType::PolicyOverride,
            "Approve dependency install",
        ),
    ];

    let projection = LoopTaskProjection::from_events(&events).unwrap();

    assert_eq!(projection.tasks[0].status, LoopTaskStatus::WaitingForReview);
    assert_eq!(projection.tasks[0].open_gates[0].gate_id, "gate-1");
}
```

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime --lib
```

Expected: FAIL before implementation.

- [x] **Step 8.2: Extend final acceptance**

Add dry-run and real gates:

```text
loop event journal contract tests
projection rebuild/replay tests
policy and budget preflight tests
durable human gate tests
typed completion evidence tests
gateway loop runner status smoke
subagent runtime event projection smoke
completion contract mocked desktop smoke
```

Run:

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
```

Expected: PASS and the dry-run output names Level 3 runtime gates.

- [x] **Step 8.3: Produce a backing engineering packet**

Add a short section to README/desktop README/changelog and the Obsidian plan that can back the product claim:

```text
Forge Level 3 Runtime backs long-running agent work with:
- append-only loop event journal
- rebuildable projections
- durable human gates
- policy and budget preflight
- typed completion evidence
- crash/replay regression coverage
- gateway runner leases
```

Run:

```bash
git diff --check
```

Expected: PASS.

- [x] **Step 8.4: Full signoff**

Run:

```bash
npm run build:desktop
npm run build:website
npm run test:eval
scripts/acceptance.sh
```

Expected: PASS.

- [x] **Step 8.5: Final commit**

Stage the intended changes:

```bash
git add apps/desktop/src-tauri/src/loop_runtime scripts/acceptance.sh scripts/acceptance.test.mjs README.md apps/desktop/README.md CHANGELOG.md docs/superpowers/plans/2026-06-12-forge-hermes-runtime-gap-roadmap.md docs/superpowers/plans/2026-06-16-level-3-agent-loop-runtime.md
```

Before committing, verify the staged scope:

```text
gitnexus_detect_changes(repo: "forge", scope: "staged")
```

Then:

```bash
git commit -m "feat(runtime): verify level 3 loop runtime"
```

**2026-06-17 Task 8 implementation evidence:** Crash/replay coverage now includes
`loop_runtime::replay_tests`, proving projection corruption rebuilds from the
append-only journal and waiting human gates survive replay. The final acceptance
script advertises and runs the Level 3 runtime gates before desktop smoke:
loop event journal contracts, projection replay, policy/budget preflight,
durable human gates, typed completion evidence, gateway loop runner status,
subagent runtime projection, and completion-contract desktop smoke. The docs
packet in root README, desktop README, CHANGELOG, Hermes roadmap, and the
Obsidian Level 3 plan backs the product claim with explicit evidence and keeps
the MVP boundary honest: no automatic gateway continuation after crash, no
default headless `AgentSession`, no Shell-internal file effect tracing, no
precise unknown cost metering, and no automatic commits.

**2026-06-18 Phase 4-I acceptance evidence:** The acceptance script now gates
the existing real worktree worker lifecycle harness with
`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
agent::a2a::child::tests::run_worktree_worker --lib`. That focused Rust gate covers
`ChildAgentRuntime::run_worktree_worker` creating a real temporary git
repo/worktree, collecting diff/usage/summary behavior through the mock
adapter/harness, and the already-in-use path that requires human review. This is
evidence for the child runtime harness only; at Phase 4-I time it did not add a
Tauri/WebDriver force-quit harness, executor-level live IO tracing, provider
token/cost streams, new `StreamEvent` variants, or auto commit/merge/push
behavior. Phase 4-K later adds direct ToolExecutor file-ish `file_io` stream
facts only.

**2026-06-18 Phase 4-J A2A child runtime file-IO bridge:** A narrow runtime
bridge now carries parent-session/A2A-task context from
`AgentSession::execute_tools` into child read-only, patch-proposal, and
worktree-worker execution. When present, child execution emits
`subagent_runtime_event` lifecycle facts and successful file-ish tool facts
(`read`, `write`, `edit`, `diff`, `list`, `search`) with the parent
`session_id`, real A2A `task_id`, and no invented `loop_task_id`. This preserves
the existing tool transcript/diff/shell event paths and does not broaden
read-only or patch-proposal permissions. It also does not claim shell-internal
file effects from `run_shell`, provider token/cost streaming, gateway
autonomous resume, parent-session structs, automatic parent selection, or auto
commit/merge/push behavior.

**2026-06-18 Phase 4-K executor-level live file-IO stream:** Direct
ToolExecutor file-ish calls now emit a general `file_io` stream event after
successful operations only. The event carries `session_id`, `block_id`, `path`,
`operation`, and `source: "executor"`. Covered direct operations are read,
write, edit, git diff, list, glob/search-files, and grep/search-content. Write
keeps the existing `diff_view` emission. The TypeScript protocol mirrors the
event, and `applyTranscriptEventToBlocks` attaches file IO facts to existing
matching tool/shell blocks as `metadata.file_io_events` without editing
`eventToBlock` or creating standalone transcript blocks. Acceptance now includes
`executor file IO stream smoke`, running `cargo test --manifest-path
apps/desktop/src-tauri/Cargo.toml executor_file_io_stream --lib`.

Evidence/tests:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml executor_file_io_stream --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml file_io_event_serializes_executor_source --lib
node --test apps/desktop/src/store/blocks.test.ts
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
```

Not claimed: shell-internal file effects from `run_shell`, provider token/cost
streaming, gateway autonomous resume, automatic parent selection,
parent-session structs, auto commit/merge/push, or Tauri/WebDriver force-quit
coverage.

**2026-06-17 full signoff evidence:** A sandboxed `scripts/acceptance.sh` run
first passed desktop and website builds, then stopped at `npm run test:eval`
because the sandbox could not write `/Users/cabbos/.forge`. The controller reran
the same full suite with normal home access and it passed completely: desktop
production build, website production build, eval runner tests (139 passed, 1
warning), loop event journal contracts (13 passed), projection rebuild/replay (2
passed), policy preflight (3 passed), budget preflight (7 passed), durable human
gate tests (5 passed), typed completion evidence (14 passed), gateway loop
runner status smoke (1 passed), subagent runtime projection smoke (22 passed),
completion contract desktop helper smoke (6 passed), completion contract mocked
desktop smoke (8 passed), desktop Phase 7 smoke specs (32 passed), and rich
preview smoke specs (4 passed).

---

## Credible Agent Engineering Bar

This work should produce engineering evidence that can back product claims. A Level 3 runtime slice is not done because a UI says it is done. It is done when the repo contains proof for these claims:

| Claim | Required Evidence |
|---|---|
| Long-running tasks survive process failure | Replay tests rebuild projection from event journal |
| Human oversight is durable | Human gate records survive replay and block side effects |
| Agent actions are governed | Policy decisions are recorded before side-effecting actions |
| Completion is auditable | Typed evidence records include command/risk/review/commit details |
| Multi-agent work is inspectable | Subagent events correlate loop task, A2A task, session, worktree, and attempt |
| Product state is honest | UI/dashboard consume runtime facts, not transcript heuristics |
| Claims are reproducible | Acceptance script advertises and runs Level 3 runtime gates |

Each implementation slice should update this table when it adds or changes the evidence behind a claim.

## Update docs and roadmap

Update docs with the user-visible runtime changes and add a Level 3 follow-up section to the Hermes roadmap.

Run:

```bash
git diff --check
npm run build:desktop
scripts/acceptance.sh --dry-run
```

Expected: PASS.

---

## Verification Matrix

Run the narrowest checks per task, then widen near integration:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml loop_runtime --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib
node --test apps/desktop/src/lib/loopRuntime.test.ts
npm --prefix apps/desktop run build
npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
git diff --check
```

Runtime-specific proof points:

```text
event journal append and idempotency
projection rebuild from journal
corrupt projection rebuild
corrupt journal hard error
human gate replay
budget override gate replay
policy preflight before side effects
typed evidence satisfies completion
stale lease interruption
duplicate RPC idempotency
worktree cancel does not merge
preserved worktree artifact survives cancel/replay
```

Full signoff:

```bash
npm run build:desktop
npm run build:website
npm run test:eval
scripts/acceptance.sh
```

2026-06-17 evidence: the full `scripts/acceptance.sh` suite passed after a
sandbox-only `/Users/cabbos/.forge` writability failure was rerun with normal
home access. The passing run covered desktop build, website build, eval runner
tests (139 passed), all Level 3 runtime gates, 32 desktop smoke specs, and 4
rich-preview smoke specs.

## Stop Conditions

Stop and ask before continuing if:

- GitNexus reports HIGH/CRITICAL risk for a symbol and the implementation needs to touch more than one runtime boundary in the same slice.
- Gateway runner work requires spawning autonomous coding agents before ledger, policy, and review contracts are implemented.
- A new dependency is needed for WebDriver, browser automation, usage metering, or service supervision.
- A migration would break old session snapshots, A2A state, gateway registry files, or memory/profile stores.
- Any acceptance gate fails twice after remediation.

## Obsidian Sync

Mirror the strategic version of this plan in:

```text
/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Level 3 Agent Loop Runtime Plan.md
```

Keep `Forge - Home.md` and `00 LLM Wiki/Forge Current Handoff.md` linked to that note so future planning sessions start from the Level 3 runtime direction, not from the completed Phase 7 product polish work.
