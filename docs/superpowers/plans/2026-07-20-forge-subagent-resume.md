# Forge Durable Subagent Identity and Resume Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist subagent identity, transcript, usage, worktree facts, and interruption state so an explicitly resumed child can continue safely after restart without pretending an old process is alive.

**Architecture:** Keep Forge's current lightweight `SubAgent` loop and A2A review gate. Add an atomic per-child `ChildRunRecord`, write one round record after each completed model/tool round, convert in-flight runs to interrupted on restore, and resume by starting a new attempt after strict parent/task/role/mode/model/workspace/worktree validation. Worktree changes always continue through existing diff, test, artifact, and human-review gates.

**Tech Stack:** Rust/Tokio, existing A2A bus/ledger/worktree worker, session journal and snapshots, Tauri IPC/StreamEvent protocol, React Workbench acceptance fixtures.

---

## Dependencies

Required before enabling resume:

- atomic snapshot and child-record writes from the session durability plan;
- state-aware compaction capsule able to carry active/interrupted child ids;
- typed tool preflight for resumed child tool calls.

The record/inspection tasks may land earlier, but the resume command remains disabled until all three dependencies pass their acceptance gates.

## Scope and file map

**Create:**

- `apps/desktop/src-tauri/src/agent/a2a/child_run.rs` — durable schema, atomic store, status transitions.
- `apps/desktop/src-tauri/src/agent/a2a/child_resume.rs` — pure validation and resume plan.

**Modify:**

- `apps/desktop/src-tauri/src/agent/a2a/mod.rs`
- `apps/desktop/src-tauri/src/agent/a2a/child.rs`
- `apps/desktop/src-tauri/src/agent/a2a/ledger.rs`
- `apps/desktop/src-tauri/src/agent/a2a/projection.rs`
- `apps/desktop/src-tauri/src/agent/a2a/supervisor.rs`
- `apps/desktop/src-tauri/src/agent/a2a/worktree.rs`
- `apps/desktop/src-tauri/src/agent/sub.rs`
- `apps/desktop/src-tauri/src/agent/session/tools.rs`
- `apps/desktop/src-tauri/src/agent/snapshot.rs`
- `apps/desktop/src-tauri/src/ipc/a2a_handlers.rs`
- `apps/desktop/src-tauri/src/ipc/session_lifecycle.rs`
- `apps/desktop/src-tauri/src/protocol/events.rs`
- `apps/desktop/src/lib/protocol.ts`
- `apps/desktop/src/components/messages/AgentA2ATimeline.tsx`
- `apps/desktop/e2e/acceptance.spec.ts`
- `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`

## Safety invariants

1. Restart never restores a child as actively running.
2. Resume starts a new attempt; it never reconnects to an old provider stream, shell, tool future, or Tokio task.
3. Parent session id, A2A task id, role, execution mode, provider/model, workspace, and worktree identity are validated before a model call.
4. Read-only children cannot resume as patch or worktree workers.
5. A changed/missing worktree fails closed and remains inspectable.
6. Worktree output still requires existing diff/test/review gates and cannot auto-merge.
7. Child records never store credentials, environment secrets, raw cancellation handles, or unbounded tool output.

### Task 1: Define the durable child-run schema and atomic store

**Files:**

- Create: `apps/desktop/src-tauri/src/agent/a2a/child_run.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/mod.rs`
- Test: `apps/desktop/src-tauri/src/agent/a2a/child_run.rs`

- [ ] **Step 1: Run impact analysis**

Check `SubAgent::run_with_mode`, `ChildAgentRuntime::run_worktree_worker`, `AgentA2ABus`, and A2A ledger save/load. Planning-time `SubAgent::run_with_mode` was HIGH: 3 direct dependents, 8 total, with `execute_tools` affected. Warn before implementation.

- [ ] **Step 2: Write schema round-trip and safety tests**

Cover every status, attempt increment, bounded transcript/tool output, missing optional worktree, unsafe ids, atomic replacement, legacy schema defaults, and secret/body redaction.

```rust
#[test]
fn in_flight_record_restores_as_interrupted() {
    let mut record = child_run(ChildRunStatus::Running);
    record.active_round = Some(active_round());
    store().save(&record).unwrap();

    let restored = store().load(&record.parent_session_id, &record.child_run_id).unwrap();
    let interrupted = restored.into_restart_state(1_700_000_000_000);
    assert_eq!(interrupted.status, ChildRunStatus::Interrupted);
    assert_eq!(interrupted.interruption_reason.as_deref(), Some("app_restarted"));
    assert!(interrupted.active_round.is_none());
}
```

- [ ] **Step 3: Implement explicit identity and status types**

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ChildRunRecord {
    pub schema_version: u32,
    pub child_run_id: String,
    pub parent_session_id: String,
    pub task_id: String,
    pub parent_task_id: Option<String>,
    pub role: AgentRole,
    pub execution_mode: AgentExecutionMode,
    pub provider: String,
    pub model: String,
    pub working_dir: String,
    pub worktree: Option<ChildWorktreeIdentity>,
    pub status: ChildRunStatus,
    pub attempt: u32,
    pub rounds: Vec<ChildRoundRecord>,
    pub active_round: Option<ActiveChildRound>,
    pub usage: LoopUsageLedger,
    pub interruption_reason: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}
```

- [ ] **Step 4: Bound persisted content**

Persist assistant text and tool results as truncated evidence using named constants. Preserve tool name, normalized input summary, result status, and digest. Do not persist full shell streams or arbitrary binary/file bodies.

- [ ] **Step 5: Implement atomic storage layout**

Use:

```text
~/.forge/sessions/<parent-session-id>/subagents/<child-run-id>/run.json
```

Write temp + sync + rename. Listing skips temp/backup files and returns per-record load errors without hiding healthy siblings.

- [ ] **Step 6: Run tests and commit**

```bash
cd apps/desktop/src-tauri
cargo test agent::a2a::child_run
git add src/agent/a2a/mod.rs src/agent/a2a/child_run.rs
git commit -m "feat(desktop): persist durable child run records"
```

### Task 2: Persist child lifecycle and round checkpoints without enabling resume

**Files:**

- Modify: `apps/desktop/src-tauri/src/agent/sub.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/child.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/supervisor.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/tools.rs`
- Test: `apps/desktop/src-tauri/src/agent/sub.rs`

- [ ] **Step 1: Re-run impact checks for each execution entry point**

Check `run_with_mode`, `run_read_only`, `run_patch_proposal`, `run_worktree_worker`, and `execute_tools`. Record direct callers and A2A/session effects.

- [ ] **Step 2: Write lifecycle transition tests**

Required transitions:

- `Created -> Running -> Completed`;
- `Created -> Running -> Failed`;
- `Running -> Cancelled`;
- `Running -> Interrupted` on simulated process restart;
- `Running -> ReviewPending` for preserved worktree result;
- persistence failure is surfaced as A2A failure evidence and never silently reported as resumable.

- [ ] **Step 3: Create the child record before spawning**

The parent allocates `child_run_id`, writes `Created`, and links it to the A2A task before `tokio::spawn`. If creation fails, do not spawn the child; return one ordered tool result explaining durable child initialization failed.

- [ ] **Step 4: Persist active round before provider sampling**

Write a bounded `ActiveChildRound` with round number and input digest. After the model/tool round completes, append a `ChildRoundRecord`, update usage, clear `active_round`, and save atomically.

- [ ] **Step 5: Persist terminal state before returning to parent**

Completed/failed/cancelled/review-pending state must reach disk before `execute_tools` adds the child result to the model-visible result map. If terminal persistence fails, return a failure result and preserve the worktree for diagnosis.

- [ ] **Step 6: Run focused tests and commit**

```bash
cd apps/desktop/src-tauri
cargo test agent::sub
cargo test agent::a2a::child
cargo test agent::session::tools_test
git add src/agent/sub.rs src/agent/a2a/child.rs src/agent/a2a/supervisor.rs src/agent/session/tools.rs
git commit -m "feat(desktop): checkpoint subagent lifecycle and rounds"
```

### Task 3: Project child-run identity into A2A and snapshots

**Files:**

- Modify: `apps/desktop/src-tauri/src/agent/a2a/ledger.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/projection.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/bus.rs`
- Modify: `apps/desktop/src-tauri/src/agent/snapshot.rs`
- Test: corresponding Rust module tests

- [ ] **Step 1: Write projection tests**

Add optional `child_run_id`, `child_attempt`, `child_resumable`, and `child_interruption_reason` to task projection. Test omitted serialization for root/non-child tasks and stable round-trip for child tasks.

- [ ] **Step 2: Link A2A task to child record by id only**

Do not embed the complete child transcript into `AgentA2ABus` or `AgentSessionSnapshot`. Store the child-run id and small status projection; the child record remains the detailed authority.

- [ ] **Step 3: Add snapshot-compatible child descriptors**

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub child_runs: Vec<ChildRunDescriptor>,
```

Descriptors contain identity/status/attempt/path only, allowing History and Diagnostics to find detailed records.

- [ ] **Step 4: Convert in-flight descriptors during restore**

When loading a snapshot or ledger, any running/starting child becomes interrupted before projections are emitted. Save the corrected descriptor and child record; never emit a running badge from stale state.

- [ ] **Step 5: Run snapshot/A2A tests and commit**

```bash
cd apps/desktop/src-tauri
cargo test agent::a2a
cargo test agent::snapshot::tests
git add src/agent/a2a src/agent/snapshot.rs
git commit -m "feat(desktop): restore child runs as durable A2A projections"
```

### Task 4: Implement pure resume validation

**Files:**

- Create: `apps/desktop/src-tauri/src/agent/a2a/child_resume.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/mod.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/worktree.rs`
- Test: `apps/desktop/src-tauri/src/agent/a2a/child_resume.rs`

- [ ] **Step 1: Write the validation matrix**

Test rejection for:

- parent session mismatch;
- unknown A2A task;
- role mismatch;
- execution-mode change;
- provider/model unavailable;
- workspace mismatch or missing directory;
- worktree path outside repository;
- missing branch;
- changed repository root;
- HEAD mismatch;
- completed/failed/cancelled/review-pending status;
- resume-attempt limit exceeded.

Test acceptance only for an interrupted record whose identity matches and whose workspace/worktree evidence is intact.

- [ ] **Step 2: Define validation output**

```rust
pub(crate) struct ChildResumePlan {
    pub child_run_id: String,
    pub next_attempt: u32,
    pub task_id: String,
    pub role: AgentRole,
    pub execution_mode: AgentExecutionMode,
    pub provider: String,
    pub model: String,
    pub working_dir: PathBuf,
    pub prior_context: Vec<ChatMessage>,
    pub worktree: Option<ValidatedChildWorktree>,
}

pub(crate) enum ChildResumeRejection {
    StatusNotResumable,
    IdentityMismatch { field: &'static str },
    ProviderUnavailable,
    WorkspaceUnavailable,
    WorktreeMismatch { reason: String },
    AttemptLimitReached,
}
```

- [ ] **Step 3: Validate worktree identity read-only**

Capture repository root, worktree path, branch, and HEAD when the child starts. Resume validation may run read-only git commands but must not recreate, clean, reset, delete, or switch the worktree.

- [ ] **Step 4: Rebuild prior context conservatively**

Use task text plus bounded completed round records. A previously active round is recorded as interrupted evidence, not replayed as if it completed. Repair tool adjacency before returning context.

- [ ] **Step 5: Run tests and commit**

```bash
cd apps/desktop/src-tauri
cargo test agent::a2a::child_resume
git add src/agent/a2a/mod.rs src/agent/a2a/child_resume.rs src/agent/a2a/worktree.rs
git commit -m "feat(desktop): validate interrupted child resume plans"
```

### Task 5: Add explicit resume IPC and new-attempt execution

**Files:**

- Modify: `apps/desktop/src-tauri/src/ipc/a2a_handlers.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/child.rs`
- Modify: `apps/desktop/src-tauri/src/agent/sub.rs`
- Modify: `apps/desktop/src-tauri/src/agent/a2a/supervisor.rs`
- Test: `apps/desktop/src-tauri/src/ipc/a2a_handlers.rs`

- [ ] **Step 1: Run impact analysis on IPC and execution symbols**

Check the selected A2A handler, `run_with_mode`, child runtime entry points, and handler registration. Warn for HIGH/CRITICAL results.

- [ ] **Step 2: Write handler tests before registration**

Test missing session, missing child, non-interrupted status, validation rejection payload, accepted plan, duplicate concurrent resume, cancellation, and successful new attempt.

- [ ] **Step 3: Define an explicit command**

```rust
#[tauri::command]
pub(crate) async fn resume_agent_child_run(
    state: tauri::State<'_, AppState>,
    session_id: String,
    child_run_id: String,
) -> Result<AgentA2AProjection, String>
```

No automatic resume occurs during startup. The command is initiated by a user action or a separately human-approved parent action.

- [ ] **Step 4: Acquire an attempt lease**

Persist `Resuming { attempt }` atomically before spawn. A second request for the same child/attempt returns the existing projection and does not spawn duplicate work.

- [ ] **Step 5: Execute through the existing mode-specific runtime**

Pass `ChildResumePlan.prior_context` and the validated worktree into the current read-only, patch-proposal, or worktree-worker path. Do not create a new role/mode based on request input.

- [ ] **Step 6: Preserve review-gate behavior**

Resumed worktree output goes through current diff extraction, structured test report, review-gate decision, artifact persistence, and user review. A failed validation or execution preserves the worktree when evidence may be needed.

- [ ] **Step 7: Run handler and backend tests**

```bash
cd apps/desktop/src-tauri
cargo test ipc::a2a_handlers
cargo test agent::a2a
cargo test agent::sub
cd ..
npm run check:backend
```

Expected: all pass.

- [ ] **Step 8: Commit resume execution**

```bash
git add apps/desktop/src-tauri/src/ipc/a2a_handlers.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/agent/a2a/child.rs apps/desktop/src-tauri/src/agent/a2a/supervisor.rs apps/desktop/src-tauri/src/agent/sub.rs
git commit -m "feat(desktop): resume interrupted child runs explicitly"
```

### Task 6: Expose resume truth in protocol and Workbench

**Files:**

- Modify: `apps/desktop/src-tauri/src/protocol/events.rs`
- Modify: `apps/desktop/src/lib/protocol.ts`
- Modify: `apps/desktop/src/lib/ipc/a2a.ts`
- Modify: `apps/desktop/src/components/messages/AgentA2ATimeline.tsx`
- Modify: `apps/desktop/src/lib/workbenchSummary.ts`
- Test: corresponding Node tests and Playwright acceptance

- [ ] **Step 1: Add optional projection fields in Rust and TypeScript**

Keep `agent_a2a_updated` as the transport. Add fields rather than a second subagent event channel unless lifecycle evidence cannot be represented.

- [ ] **Step 2: Add a narrowly gated resume action**

Show “继续子任务” only when `child_resumable === true`. Disable it while the resume IPC is pending. Display the validation rejection reason inline without changing the record back to running.

- [ ] **Step 3: Render attempts and interruption honestly**

Show attempt count, interrupted reason, model, mode, worktree preservation, and latest completed round. Never label a restored interrupted child as live.

- [ ] **Step 4: Run frontend contracts**

```bash
cd apps/desktop
npm run check:protocol
node --test src/store/workbenchSummary.test.ts
npm run build
```

Expected: all pass.

- [ ] **Step 5: Commit protocol and Workbench**

```bash
git add apps/desktop/src-tauri/src/protocol/events.rs apps/desktop/src/lib/protocol.ts apps/desktop/src/lib/ipc apps/desktop/src/components/messages/AgentA2ATimeline.tsx apps/desktop/src/lib/workbenchSummary.ts
git commit -m "feat(desktop): surface resumable child attempts"
```

### Task 7: Add restart/worktree/review acceptance

**Files:**

- Modify: `apps/desktop/e2e/fixtures/app.ts`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
- Modify: `apps/desktop/e2e/a2a-confirm-runtime.spec.ts`
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add mocked restart fixture**

Represent a child that was running before restart and is now interrupted/resumable. The fixture must expose the same A2A projection and resume IPC contracts as production.

- [ ] **Step 2: Add Workbench acceptance**

Assert:

- restored child is interrupted, not running;
- resume action is visible only for the resumable child;
- clicking resume increments attempt and transitions through resuming/running;
- completion produces a review-pending worktree artifact;
- approve/reject remains the existing explicit review action.

- [ ] **Step 3: Add backend worktree integration tests**

Using a temporary git repository, cover intact HEAD acceptance, changed HEAD rejection, missing worktree rejection, preserved diff after execution failure, and no automatic merge.

- [ ] **Step 4: Run all acceptance gates**

```bash
cd apps/desktop
npm run test:e2e -- e2e/acceptance.spec.ts e2e/a2a-confirm-runtime.spec.ts
npm run check:backend
npm run build
cd ../..
npm run test:eval
scripts/acceptance.sh --dry-run
```

Expected: all pass.

- [ ] **Step 5: Update documentation**

Document explicit resume, identity validation, new-attempt semantics, worktree failure behavior, and the fact that live processes do not survive restart.

- [ ] **Step 6: Commit acceptance and docs**

```bash
git add apps/desktop/e2e README.md apps/desktop/README.md CHANGELOG.md
git commit -m "test(desktop): accept durable child resume and review"
```

### Task 8: Final impact, data-safety, and rollout audit

**Files:** none expected beyond concrete audit fixes.

- [ ] **Step 1: Run GitNexus change detection**

```text
detect_changes({scope: "compare", base_ref: "main", repo: "forge"})
```

Expected affected domains: Agent/A2A, Session tool delegation, snapshot/restore descriptors, IPC, protocol/Workbench, acceptance, and docs. Investigate unrelated provider, credential, scheduler, gateway trigger, or work-panel flows.

- [ ] **Step 2: Scan persisted records for forbidden data**

Generate records containing fake API keys, environment values, long shell output, binary-like content, and tool bodies. Assert the persisted JSON contains ids/digests/status but none of the forbidden strings.

- [ ] **Step 3: Prove no automatic resume**

Restore sessions with interrupted children and wait through normal startup. Assert zero provider calls, zero tool calls, and zero worktree mutations until `resume_agent_child_run` receives explicit user action.

- [ ] **Step 4: Prove review authority remains unchanged**

Run a resumed worktree child through completion and assert no main-worktree write or merge occurs before the existing A2A approval action.

- [ ] **Step 5: Ship inspection before resume by default if any gate is weak**

If restart, identity, worktree, secret-redaction, or review tests are incomplete, keep durable records and Workbench inspection enabled but leave the resume command hidden/disabled. Do not redefine partial inspection as completed resume support.

- [ ] **Step 6: Commit only verified audit fixes**

Stage each concrete audit fix individually, inspect the staged diff, and commit with:

```bash
git commit -m "fix(desktop): close durable child resume audit gaps"
```
