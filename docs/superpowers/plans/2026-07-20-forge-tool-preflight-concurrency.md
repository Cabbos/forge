# Forge Typed Tool Preflight and Conflict-Aware Concurrency Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate tool approval from execution and safely allow non-conflicting file writes to run concurrently while preserving Forge's permission evidence, cancellation, result ordering, and worktree boundaries.

**Architecture:** Extract the current capability-hook-permission path into a typed preflight result, derive a conservative conflict scope after hook input transformation, and dispatch prepared calls through a scheduler. The first release only parallelizes allowlisted direct file writes with disjoint normalized paths; all shell, git mutation, unknown MCP mutation, and pathless writes remain workspace-exclusive.

**Tech Stack:** Rust/Tokio, existing `Harness`, `PermissionGate`, hook engine, tool executor, A2A runtime, turn metrics, and acceptance suite.

---

## Dependencies

This workstream may begin after atomic snapshot hardening. It does not require journal authority or subagent resume. It must land before enabling high-volume durable child resume so child tools share the same safety contract.

## Scope and file map

**Create:**

- `apps/desktop/src-tauri/src/harness/tool_preflight.rs` — typed preflight contract and execution token.
- `apps/desktop/src-tauri/src/agent/tool_conflicts.rs` — conservative scope derivation and overlap rules.
- `apps/desktop/src-tauri/src/agent/tool_scheduler.rs` — prepared-call scheduler and deterministic result collection.

**Modify:**

- `apps/desktop/src-tauri/src/harness/mod.rs`
- `apps/desktop/src-tauri/src/harness/permissions.rs`
- `apps/desktop/src-tauri/src/agent/mod.rs`
- `apps/desktop/src-tauri/src/agent/session/tools.rs`
- `apps/desktop/src-tauri/src/agent/session/tools_test.rs`
- `apps/desktop/src-tauri/src/agent/tool_results.rs`
- `apps/desktop/src-tauri/src/agent/turn_state.rs`
- `apps/desktop/src-tauri/src/protocol/events.rs` only if queue/preflight facts need a user-visible event
- `apps/desktop/src/lib/protocol.ts` in lockstep if the protocol changes
- `apps/desktop/e2e/acceptance.spec.ts`
- `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`

## Safety invariants

1. Hook transformation happens before conflict-scope derivation.
2. Permission approval is bound to the exact transformed input and write boundary.
3. No denied, invalid, unavailable, or cancelled call reaches dispatch.
4. Same-path writes never overlap.
5. Unknown mutation defaults to workspace-exclusive.
6. Result messages preserve original model call order.
7. Cancellation prevents not-yet-started calls and cooperatively cancels running calls through existing tokens.
8. Delegate/A2A work cannot bypass its existing execution-mode and review-gate rules.

### Task 1: Extract typed preflight without changing execution order

**Files:**

- Create: `apps/desktop/src-tauri/src/harness/tool_preflight.rs`
- Modify: `apps/desktop/src-tauri/src/harness/mod.rs`
- Test: `apps/desktop/src-tauri/src/harness/tool_preflight.rs`
- Test: `apps/desktop/src-tauri/src/harness/permissions_test.rs`

- [ ] **Step 1: Run impact analysis**

Run impact for `Harness::execute_tool_with_emitter`, `PermissionGate::check_with_evidence`, pre/post hook methods, and MCP availability checks. Record all direct callers and affected processes before editing.

- [ ] **Step 2: Write preflight outcome tests**

Test capability disabled, hook block, hook input modification, MCP unavailable, permission allow, permission policy deny, explicit user deny, cancellation while awaiting confirmation, and shell approval binding failure.

```rust
#[tokio::test]
async fn conflict_input_is_the_hook_transformed_input() {
    let harness = harness_with_hook(json!({"path": "src/after.rs"}));
    let prepared = harness
        .preflight_tool(test_request("edit_file", json!({"path": "src/before.rs"})))
        .await
        .unwrap();

    assert_eq!(prepared.input["path"], "src/after.rs");
    assert!(matches!(prepared.decision, ToolPreflightDecision::Executable { .. }));
}
```

- [ ] **Step 3: Define a typed contract that carries evidence**

```rust
pub(crate) struct ToolPreflightRequest {
    pub session_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub cancel: Option<Arc<Notify>>,
}

pub(crate) struct PreparedHarnessCall {
    pub request: ToolPreflightRequest,
    pub input: serde_json::Value,
    pub permission_evidence: Option<PermissionLedgerEvent>,
    pub shell_approval: Option<ShellApproval>,
    pub decision: ToolPreflightDecision,
}

pub(crate) enum ToolPreflightDecision {
    Executable,
    PolicyDenied { result: String },
    UserDenied { result: String },
    Cancelled { result: String },
    Invalid { result: String },
    Unavailable { result: String },
}
```

Do not put `AppHandle`, raw oneshot senders, or mutable registry guards into the prepared value.

- [ ] **Step 4: Split `execute_tool_with_emitter` into preflight and prepared dispatch**

```rust
pub(crate) async fn preflight_tool_with_emitter(...) -> PreparedHarnessCall;
pub(crate) async fn execute_prepared_tool_with_emitter(
    &self,
    prepared: PreparedHarnessCall,
    emitter: Arc<dyn EventEmitter>,
) -> String;
```

Keep `execute_tool_with_emitter` as a compatibility wrapper that calls the two methods sequentially. Existing callers must behave identically in this task.

- [ ] **Step 5: Preserve post-tool semantics**

Blocked outcomes must emit the same blocked tool result and post-tool event behavior as before. Executable outcomes run the existing executor/MCP dispatch and then post-tool hooks exactly once.

- [ ] **Step 6: Run focused harness tests**

```bash
cd apps/desktop/src-tauri
cargo test harness::tool_preflight
cargo test harness::permissions
cargo test harness::permissions_test
```

Expected: all pass and no execution-order behavior changes.

- [ ] **Step 7: Commit extraction only**

```bash
git add apps/desktop/src-tauri/src/harness/mod.rs apps/desktop/src-tauri/src/harness/tool_preflight.rs apps/desktop/src-tauri/src/harness/permissions_test.rs
git commit -m "refactor(desktop): type tool preflight outcomes"
```

### Task 2: Define conservative conflict scopes

**Files:**

- Create: `apps/desktop/src-tauri/src/agent/tool_conflicts.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`
- Test: `apps/desktop/src-tauri/src/agent/tool_conflicts.rs`

- [ ] **Step 1: Write the full classification table as tests**

Required cases:

| Tool | Input | Scope |
|---|---|---|
| `read_file` | workspace path | `ReadOnly` |
| `grep` / `search_files` | any | `ReadOnly` |
| `write_to_file` | normalized in-workspace path | `Paths` |
| `edit_file` | normalized in-workspace path | `Paths` |
| `write_to_file` | missing/non-string path | `WorkspaceExclusive` |
| `run_shell` | any | `WorkspaceExclusive` |
| mutating git tool | any | `WorkspaceExclusive` |
| unknown MCP tool | any | `WorkspaceExclusive` |
| known read-only MCP resource read | any | `ReadOnly` |
| `delegate_task` | any | `ExternalExclusive("a2a:<parent>")` or dedicated delegate lane |

- [ ] **Step 2: Implement normalized scope types**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ToolConflictScope {
    ReadOnly,
    Paths(BTreeSet<PathBuf>),
    WorkspaceExclusive,
    ExternalExclusive(String),
}

pub(crate) fn scopes_conflict(left: &ToolConflictScope, right: &ToolConflictScope) -> bool;
```

Use the existing workspace boundary/canonicalization rules. Never use untrusted raw paths as lock keys before validation.

- [ ] **Step 3: Implement allowlisted derivation after preflight**

```rust
pub(crate) fn conflict_scope_for_prepared_call(
    tool_name: &str,
    transformed_input: &serde_json::Value,
    working_dir: &Path,
) -> ToolConflictScope
```

Only direct file-edit tools return `Paths` in the first release. Unknown tools fail closed to `WorkspaceExclusive`.

- [ ] **Step 4: Add property-style overlap tests**

Prove symmetry, reflexivity for exclusive/path scopes, no conflict between read-only scopes, and conflict between parent/child path aliases that normalize to the same path.

- [ ] **Step 5: Run tests and commit**

```bash
cd apps/desktop/src-tauri
cargo test agent::tool_conflicts
git add src/agent/mod.rs src/agent/tool_conflicts.rs
git commit -m "feat(desktop): classify conservative tool conflict scopes"
```

### Task 3: Build a scheduler with deterministic result reconstruction

**Files:**

- Create: `apps/desktop/src-tauri/src/agent/tool_scheduler.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`
- Test: `apps/desktop/src-tauri/src/agent/tool_scheduler.rs`

- [ ] **Step 1: Write scheduler concurrency tests using barriers**

Avoid timing-only assertions. Use barriers/channels to prove:

- two read-only calls overlap;
- disjoint path writes overlap;
- same-path writes preserve model order and never overlap;
- workspace-exclusive waits for all earlier work and blocks later mutation work;
- denied calls never enter the executor;
- cancellation marks queued calls cancelled;
- results return in original index order despite reverse completion order.

```rust
#[tokio::test]
async fn same_path_writes_do_not_overlap() {
    let probe = ConcurrencyProbe::new();
    let calls = vec![prepared_path_write(0, "src/lib.rs"), prepared_path_write(1, "src/lib.rs")];
    let results = schedule_prepared_calls(calls, probe.executor()).await;
    assert_eq!(probe.max_concurrency_for("src/lib.rs"), 1);
    assert_eq!(results.iter().map(|r| r.index).collect::<Vec<_>>(), vec![0, 1]);
}
```

- [ ] **Step 2: Define scheduler inputs and outputs**

```rust
pub(crate) struct PreparedToolCall {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub prepared: PreparedHarnessCall,
    pub conflict_scope: ToolConflictScope,
    pub queued_at_ms: u64,
}

pub(crate) struct ScheduledToolResult {
    pub index: usize,
    pub id: String,
    pub result: String,
    pub queue_duration_ms: u64,
    pub execution_duration_ms: u64,
}
```

- [ ] **Step 3: Implement scheduling without holding locks across await**

Use owned per-resource Tokio mutexes or a deterministic wave planner. Never hold the central resource-map mutex while awaiting execution. Remove idle per-path locks after the batch completes.

- [ ] **Step 4: Keep a legacy mode**

Support `ToolSchedulerMode::LegacyWritesSequential` and `ToolSchedulerMode::ConflictAware`. Tests must prove legacy mode matches current read-parallel/write-sequential ordering.

- [ ] **Step 5: Run scheduler tests under Tokio's multi-thread flavor**

```bash
cd apps/desktop/src-tauri
cargo test agent::tool_scheduler -- --nocapture
```

Expected: deterministic pass across repeated runs.

- [ ] **Step 6: Commit the unused scheduler module**

```bash
git add apps/desktop/src-tauri/src/agent/mod.rs apps/desktop/src-tauri/src/agent/tool_scheduler.rs
git commit -m "feat(desktop): schedule prepared tools by conflict scope"
```

### Task 4: Integrate preflight and scheduler into `AgentSession::execute_tools`

**Files:**

- Modify: `apps/desktop/src-tauri/src/agent/session/tools.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/tools_test.rs`
- Modify: `apps/desktop/src-tauri/src/agent/tool_results.rs`

- [ ] **Step 1: Re-run impact and record source fallback**

Run impact on `execute_tools`. If the graph still reports zero callers, record the direct source caller `execute_single_round` in `agent/session/loop.rs`, the ordered result builder, affected A2A dispatch paths, selected tests, and residual risk before editing.

- [ ] **Step 2: Write integration tests before changing orchestration**

Test a mixed batch containing:

- read;
- two disjoint edits;
- one same-path edit;
- permission denial;
- workspace-exclusive shell;
- delegate task;
- reverse completion order.

Assert exactly one model-visible result per original call id and original order in the final tool-result message.

- [ ] **Step 3: Extract delegate preparation from execution**

Represent delegate calls as prepared calls in a dedicated lane. Preserve A2A task assignment, execution-mode selection, usage folding, artifact creation, worktree review, and failure recording. Do not route delegate tasks through generic filesystem conflict logic.

- [ ] **Step 4: Preflight regular calls sequentially in model order**

For each call:

1. record running trace;
2. await typed preflight and any confirmation;
3. derive scope from transformed input;
4. collect executable or terminal preflight result.

No regular tool execution begins until the batch preflight pass finishes. If turn cancellation fires, stop preflighting and mark remaining calls cancelled.

- [ ] **Step 5: Dispatch through legacy scheduler first**

Land the integration with `LegacyWritesSequential` as default. Run all tests and compare permission/tool events against existing fixtures before enabling conflict-aware mode.

- [ ] **Step 6: Enable conflict-aware mode only for allowlisted direct writes**

Feature flag/config default:

```text
FORGE_TOOL_CONFLICT_SCHEDULER=allowlisted
```

Allowed path concurrency: `write_to_file`, `edit_file`. All other mutation calls remain workspace-exclusive.

- [ ] **Step 7: Run focused and backend tests**

```bash
cd apps/desktop/src-tauri
cargo test agent::session::tools_test
cargo test harness::tool_preflight
cargo test agent::tool_scheduler
cd ..
npm run check:backend
```

Expected: all pass.

- [ ] **Step 8: Commit integration**

```bash
git add apps/desktop/src-tauri/src/agent/session/tools.rs apps/desktop/src-tauri/src/agent/session/tools_test.rs apps/desktop/src-tauri/src/agent/tool_results.rs
git commit -m "feat(desktop): preflight and schedule tool batches"
```

### Task 5: Add queue/conflict evidence without protocol churn unless needed

**Files:**

- Modify: `apps/desktop/src-tauri/src/agent/turn_state.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/tools.rs`
- Modify: `apps/desktop/src-tauri/src/protocol/events.rs` only if existing turn projection cannot carry the facts
- Modify: `apps/desktop/src/lib/protocol.ts` only with the Rust protocol
- Test: corresponding Rust and TypeScript protocol tests

- [ ] **Step 1: Write metrics aggregation tests**

Record batch preflight duration, permission-wait duration, maximum concurrent calls, conflict wait duration, workspace-exclusive count, denied count, and cancelled-before-start count.

- [ ] **Step 2: Extend existing turn/tool trace first**

Prefer additive optional fields on existing `AgentToolTrace`/turn projection over a new stream event. Keep queue timing as evidence, not chat content.

- [ ] **Step 3: If protocol changes, update both sides in one commit**

Run:

```bash
cd apps/desktop
npm run check:protocol
```

Expected: field names and optionality match.

- [ ] **Step 4: Run metrics tests and commit**

```bash
cd apps/desktop/src-tauri
cargo test agent::turn_state
cargo test agent::session::tools_test
git add src/agent/turn_state.rs src/agent/session/tools.rs src/protocol/events.rs ../src/lib/protocol.ts
git commit -m "feat(desktop): record tool preflight and conflict evidence"
```

### Task 6: Add race, cancellation, permission, and product acceptance coverage

**Files:**

- Modify: `apps/desktop/e2e/fixtures/app.ts`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add backend regression corpus**

Include:

- symlink/path alias conflict;
- two edits targeting the same nonexistent future path;
- write plus shell conflict;
- post-hook path change;
- confirmation timeout;
- cancellation during permission wait;
- cancellation while one call runs and another is queued;
- MCP read and unknown MCP mutation;
- result order with missing executor result fallback.

- [ ] **Step 2: Add acceptance assertions around permission truth**

Use existing mocked IPC contracts to verify that a mixed tool batch shows permission evidence and ordered results. The UI must not imply a denied call executed, and running/queued counts must settle after cancellation.

- [ ] **Step 3: Run acceptance and full desktop gates**

```bash
cd apps/desktop
npm run check:protocol
npm run test:e2e -- e2e/acceptance.spec.ts
npm run build
npm run check:backend
cd ../..
scripts/acceptance.sh --dry-run
```

Expected: all pass.

- [ ] **Step 4: Run GitNexus change detection**

```text
detect_changes({scope: "compare", base_ref: "main", repo: "forge"})
```

Expected affected domains: Harness, permissions, Agent/Session tools, A2A delegate integration, protocol only if changed, acceptance, and docs.

- [ ] **Step 5: Document conservative defaults and rollback**

Document that only direct disjoint file edits parallelize. Shell, git mutation, generators, formatters, and unknown MCP mutation remain exclusive. Explain the environment/config switch that returns all writes to legacy serialization.

- [ ] **Step 6: Commit acceptance and docs**

```bash
git add apps/desktop/e2e README.md apps/desktop/README.md CHANGELOG.md
git commit -m "test(desktop): accept conflict-aware tool execution"
```

### Task 7: Promotion audit

**Files:** none expected beyond audit fixes.

- [ ] **Step 1: Run the scheduler stress test repeatedly**

```bash
cd apps/desktop/src-tauri
for run in 1 2 3 4 5 6 7 8 9 10; do cargo test agent::tool_scheduler --quiet || exit 1; done
```

Expected: ten passes with no hangs or nondeterministic assertions.

- [ ] **Step 2: Compare permission ledgers across scheduler modes**

Replay the same mixed batch under legacy and conflict-aware modes. Assert identical preflight decisions, evidence boundaries, and ordered model-visible results; only queue/execution timing may differ.

- [ ] **Step 3: Keep the allowlist narrow**

Do not add shell, formatter, generator, git mutation, or generic MCP path concurrency in response to passing unit tests. Expansion requires real runtime evidence and a separate review.

- [ ] **Step 4: Commit only concrete audit fixes**

Stage each concrete file named by `git diff --name-only` individually, inspect the staged diff, and commit with:

```bash
git commit -m "fix(desktop): close tool scheduler audit gaps"
```
