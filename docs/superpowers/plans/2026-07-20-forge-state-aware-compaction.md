# Forge State-Aware Compaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every Forge compaction path preserve active runtime continuity and recover safely from interruption without repeating the summary model call.

**Architecture:** Add a bounded structured `CompactionCapsule`, persist a prepared checkpoint before replacing conversation state, append the replacement through the session mutation journal, and reconcile incomplete checkpoints during restore. Keep current one-pass summary as the default; add two-pass prefire only after checkpoint correctness and metrics are proven.

**Tech Stack:** Rust, serde, existing `AgentSession` compaction pipeline, session mutation journal from the durability plan, existing `StreamEvent` protocol and acceptance harness.

---

## Dependencies

This plan starts after Tasks 1–4 of `2026-07-20-forge-session-durability.md` are complete. It requires atomic snapshot writes, `SessionMutation::ConversationReplaced`, and centralized conversation replacement. Journal authority promotion may happen after this plan.

## Scope and file map

**Create:**

- `apps/desktop/src-tauri/src/agent/compaction_capsule.rs` — structured bounded continuity state and rendering.
- `apps/desktop/src-tauri/src/agent/compaction_checkpoint.rs` — checkpoint schema, atomic store, restore reconciliation.
- `apps/desktop/src-tauri/src/agent/compaction_prefire.rs` — optional two-pass cache and fingerprint validation.

**Modify:**

- `apps/desktop/src-tauri/src/agent/mod.rs`
- `apps/desktop/src-tauri/src/agent/auto_compact.rs`
- `apps/desktop/src-tauri/src/agent/compact_summary.rs`
- `apps/desktop/src-tauri/src/agent/context_builder.rs`
- `apps/desktop/src-tauri/src/agent/session/compact.rs`
- `apps/desktop/src-tauri/src/agent/session/loop.rs`
- `apps/desktop/src-tauri/src/agent/snapshot.rs`
- `apps/desktop/src-tauri/src/ipc/session_lifecycle.rs`
- `apps/desktop/src-tauri/src/protocol/events.rs`
- `apps/desktop/src/lib/protocol.ts`
- `apps/desktop/e2e/acceptance.spec.ts`
- `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`

### Task 1: Define and bound the compaction capsule

**Files:**

- Create: `apps/desktop/src-tauri/src/agent/compaction_capsule.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`
- Test: `apps/desktop/src-tauri/src/agent/compaction_capsule.rs`

- [ ] **Step 1: Run impact analysis on state sources**

Check `AgentSession`, `GoalLedger`, `AgentA2ABus::projection`, and the Harness accessors used for pending confirmations and active tools. Record HIGH/CRITICAL warnings before editing.

- [ ] **Step 2: Write capsule derivation tests**

Required cases:

- only pending/in-progress goal tasks are included;
- completed A2A tasks without pending review are omitted;
- review-pending and running A2A tasks remain;
- pending confirmations and active tool descriptors preserve ids;
- edited paths are normalized, deduplicated, sorted, and capped;
- no prompt body, tool result body, secret, API key, or environment value enters the capsule;
- serialized capsule stays under a fixed byte limit.

```rust
#[test]
fn capsule_is_deterministic_bounded_and_body_free() {
    let input = capsule_input_with_duplicates_and_long_bodies();
    let capsule = CompactionCapsule::derive(input);
    let json = serde_json::to_string(&capsule).unwrap();

    assert!(json.len() <= MAX_COMPACTION_CAPSULE_BYTES);
    assert_eq!(capsule.edited_paths, vec!["src/a.rs", "src/b.rs"]);
    assert!(!json.contains("secret-result-body"));
    assert!(!json.contains("sk-test-key"));
}
```

- [ ] **Step 3: Run tests and observe the missing module failure**

```bash
cd apps/desktop/src-tauri
cargo test agent::compaction_capsule
```

Expected: FAIL before implementation.

- [ ] **Step 4: Implement structured types and a single derivation input**

```rust
pub(crate) const MAX_COMPACTION_PATHS: usize = 64;
pub(crate) const MAX_COMPACTION_TASKS: usize = 32;
pub(crate) const MAX_COMPACTION_CAPSULE_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CompactionCapsule {
    pub goal: Option<CompactionGoalState>,
    pub active_a2a_tasks: Vec<CompactionA2ATask>,
    pub pending_confirms: Vec<PendingConfirmDescriptor>,
    pub active_tool_calls: Vec<ActiveToolCallDescriptor>,
    pub edited_paths: Vec<String>,
    pub connected_mcp_servers: Vec<String>,
    pub next_action: Option<String>,
    pub truncated: bool,
}
```

Use ids, statuses, concise labels, and paths only. Truncate user/model text by character count before serialization.

- [ ] **Step 5: Add a deterministic hidden-context renderer**

```rust
impl CompactionCapsule {
    pub(crate) fn render_hidden_context(&self) -> String {
        // Stable headings and sorted entries; no prose inference.
    }
}
```

The renderer must clearly label the block as runtime state, not a user message or model-generated summary.

- [ ] **Step 6: Run module tests and commit**

```bash
cd apps/desktop/src-tauri
cargo fmt --check
cargo test agent::compaction_capsule
git add src/agent/mod.rs src/agent/compaction_capsule.rs
git commit -m "feat(desktop): derive bounded compaction continuity capsules"
```

### Task 2: Persist checkpoint-before-replace

**Files:**

- Create: `apps/desktop/src-tauri/src/agent/compaction_checkpoint.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`
- Test: `apps/desktop/src-tauri/src/agent/compaction_checkpoint.rs`

- [ ] **Step 1: Write checkpoint store tests**

Cover atomic save, prepared-to-committed transition, fingerprint mismatch, invalid session id, old schema loading, and cleanup retention.

```rust
#[test]
fn prepared_checkpoint_roundtrips_without_summary_regeneration() {
    let store = test_store("prepared");
    let checkpoint = checkpoint(CompactionCheckpointState::Prepared);
    store.save(&checkpoint).unwrap();

    let restored = store.load_latest("session-1").unwrap().unwrap();
    assert_eq!(restored.checkpoint_id, checkpoint.checkpoint_id);
    assert_eq!(restored.summary, checkpoint.summary);
    assert_eq!(restored.state, CompactionCheckpointState::Prepared);
}
```

- [ ] **Step 2: Implement checkpoint schema**

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CompactionCheckpoint {
    pub schema_version: u32,
    pub checkpoint_id: String,
    pub session_id: String,
    pub journal_generation: String,
    pub base_journal_sequence: u64,
    pub original_messages_fingerprint: String,
    pub retained_messages: Vec<ChatMessage>,
    pub summary: Option<String>,
    pub capsule: CompactionCapsule,
    pub reason: CompactionReason,
    pub estimated_tokens_before: u32,
    pub estimated_tokens_after: u32,
    pub state: CompactionCheckpointState,
    pub created_at_ms: u64,
}
```

- [ ] **Step 3: Implement atomic per-session storage**

Store under `~/.forge/sessions/<session-id>/compaction/<checkpoint-id>.json`. Use temp + flush + rename. Keep the latest committed checkpoint and any single prepared checkpoint; prune older committed checkpoints only after snapshot save succeeds.

- [ ] **Step 4: Run tests and commit**

```bash
cd apps/desktop/src-tauri
cargo test agent::compaction_checkpoint
git add src/agent/mod.rs src/agent/compaction_checkpoint.rs
git commit -m "feat(desktop): persist compaction checkpoints"
```

### Task 3: Feed the capsule into summary and next-turn context

**Files:**

- Modify: `apps/desktop/src-tauri/src/agent/compact_summary.rs`
- Modify: `apps/desktop/src-tauri/src/agent/context_builder.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/compact.rs`
- Test: `apps/desktop/src-tauri/src/agent/compact_summary.rs`
- Test: `apps/desktop/src-tauri/src/agent/context_builder.rs`

- [ ] **Step 1: Run impact analysis**

Check `compact_summary_prompt_messages`, `ContextBuilder::build`, `compact_plan_with_summary`, and `apply_compaction_emitter` before editing. Planning-time impact for `apply_compaction_emitter` was LOW with one direct caller, but re-run against the implementation branch.

- [ ] **Step 2: Write prompt and context tests**

Prove that:

- capsule facts appear in the summary request under an explicit structured block;
- the summary prompt says capsule state is authoritative and must not be rewritten as new ids/statuses;
- the next normal model call receives capsule hidden context separately from the summary;
- empty capsules add no empty block;
- omitted/truncated capsule entries are recorded in context-source evidence.

- [ ] **Step 3: Extend the plan object rather than reading locks inside prompt code**

```rust
pub(crate) struct CompactPlan {
    // existing fields
    pub(crate) capsule: CompactionCapsule,
}
```

Build the capsule once in the async session layer and pass it down. Do not make `compact_summary.rs` reach into `AgentSession` or `Harness`.

- [ ] **Step 4: Add a dedicated hidden context kind**

Extend `ContextSourceKind` with `CompactionCapsule`. Preserve source accounting and estimated tokens. Do not expose the capsule as visible conversation history.

- [ ] **Step 5: Run targeted tests**

```bash
cd apps/desktop/src-tauri
cargo test agent::compact_summary
cargo test agent::context_builder
cargo test agent::auto_compact
```

Expected: all pass.

- [ ] **Step 6: Commit capsule integration**

```bash
git add apps/desktop/src-tauri/src/agent/compact_summary.rs apps/desktop/src-tauri/src/agent/context_builder.rs apps/desktop/src-tauri/src/agent/auto_compact.rs apps/desktop/src-tauri/src/agent/session/compact.rs
git commit -m "feat(desktop): preserve runtime state across compaction"
```

### Task 4: Make all compaction paths transactional

**Files:**

- Modify: `apps/desktop/src-tauri/src/agent/session/compact.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/loop.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/loop_test.rs`
- Test: `apps/desktop/src-tauri/src/agent/session/loop_test.rs`

- [ ] **Step 1: Write crash-point state-machine tests**

Inject a test-only persistence trait and simulate failure after each boundary:

1. summary generated, before checkpoint;
2. prepared checkpoint saved, before journal append;
3. journal replacement appended, before memory replace;
4. memory replaced, before snapshot save;
5. snapshot saved, before checkpoint commit.

For every point, assert the recoverable source and that no second summary model call is required.

- [ ] **Step 2: Introduce one async transaction method**

```rust
pub(crate) async fn commit_compaction(
    &self,
    compacted: CompactResult,
    stats: CompactStats,
    capsule: CompactionCapsule,
    reason: CompactionReason,
    emitter: &dyn EventEmitter,
) -> Result<(), String>
```

This method owns checkpoint prepare, session journal append, in-memory replace, snapshot trigger, checkpoint commit, metrics, and event emission in that order.

- [ ] **Step 3: Route manual, proactive, and overflow compaction through the transaction**

Remove direct calls that mutate `messages`/`summary` outside `commit_compaction`. Keep the existing model-error behavior:

- proactive model-summary failure may skip;
- overflow retry may use heuristic fallback;
- cancellation never applies a partial compact result.

- [ ] **Step 4: Verify tool adjacency after every transaction path**

Call the existing repair/validation helper before checkpoint persistence and test assistant tool calls with zero, one, and multiple results.

- [ ] **Step 5: Run session and backend tests**

```bash
cd apps/desktop/src-tauri
cargo test agent::session::loop_test
cargo test agent::session_tests
cd ..
npm run check:backend
```

Expected: all pass.

- [ ] **Step 6: Commit transactional compaction**

```bash
git add apps/desktop/src-tauri/src/agent/session/compact.rs apps/desktop/src-tauri/src/agent/session/loop.rs apps/desktop/src-tauri/src/agent/session/loop_test.rs
git commit -m "feat(desktop): commit compaction transactionally"
```

### Task 5: Reconcile incomplete checkpoints during restore

**Files:**

- Modify: `apps/desktop/src-tauri/src/agent/compaction_checkpoint.rs`
- Modify: `apps/desktop/src-tauri/src/agent/snapshot.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/session_lifecycle.rs`
- Test: `apps/desktop/src-tauri/src/ipc/session_lifecycle_tests.rs`

- [ ] **Step 1: Write reconciliation table tests**

| Checkpoint | Journal replacement | Snapshot fingerprint | Decision |
|---|---|---|---|
| prepared | absent | old | discard prepared checkpoint |
| prepared | present | old | replay journal replacement, save snapshot, commit checkpoint |
| prepared | present | new | mark committed |
| committed | present | old | replay journal replacement |
| committed | absent | any | quarantine mismatch and use last valid authority |

- [ ] **Step 2: Implement a pure reconciliation decision**

```rust
pub(crate) fn reconcile_compaction(
    checkpoint: &CompactionCheckpoint,
    journal: &SessionProjection,
    snapshot: Option<&AgentSessionSnapshot>,
) -> Result<CompactionRecoveryDecision, String>
```

This function must not perform model calls or file writes.

- [ ] **Step 3: Apply the decision in session restore**

Emit an existing or additive recovery notice when reconciliation changes the selected snapshot. Do not render internal capsule bodies in the notice.

- [ ] **Step 4: Run restore tests and commit**

```bash
cd apps/desktop/src-tauri
cargo test ipc::session_lifecycle_tests
git add src/agent/compaction_checkpoint.rs src/agent/snapshot.rs src/ipc/session_lifecycle.rs src/ipc/session_lifecycle_tests.rs
git commit -m "feat(desktop): recover interrupted compaction checkpoints"
```

### Task 6: Add two-pass prefire behind a feature flag

**Files:**

- Create: `apps/desktop/src-tauri/src/agent/compaction_prefire.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/loop.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/compact.rs`
- Test: `apps/desktop/src-tauri/src/agent/compaction_prefire.rs`

- [ ] **Step 1: Write fingerprint and stale-cache tests**

Test exact reuse only when prefix fingerprint, model id, prompt schema, and journal generation match. Any mismatch must fall back to normal one-pass compaction.

- [ ] **Step 2: Implement a bounded cache**

```rust
pub(crate) struct CompactionPrefireCache {
    pub prefix_fingerprint: String,
    pub model_id: String,
    pub prompt_schema_version: u32,
    pub journal_generation: String,
    pub note: String,
    pub created_at_ms: u64,
}
```

Store at most one cache per live session. It is an optimization, not durable authority.

- [ ] **Step 3: Start prefire only under explicit gates**

Gate on the environment/config feature flag, context percentage, no in-flight prefire, provider availability, and uncancelled turn. Do not hold `AgentSession` locks across the provider call.

- [ ] **Step 4: Merge prefire note into final summary request**

The final request summarizes only the tail plus the validated prefire note. If validation fails, use the original full compact plan.

- [ ] **Step 5: Add timing and reuse metrics**

Record started, reused, stale, cancelled, failed, and saved-latency facts in the existing turn metrics/trace structure. No new UI is required in this task.

- [ ] **Step 6: Run tests with the flag off and on**

```bash
cd apps/desktop/src-tauri
cargo test agent::compaction_prefire
cargo test agent::session::loop_test
FORGE_COMPACTION_PREFIRE=1 cargo test agent::session::loop_test
```

Expected: both modes pass; flag-off behavior matches the pre-change compaction path.

- [ ] **Step 7: Commit the optional optimization**

```bash
git add apps/desktop/src-tauri/src/agent/compaction_prefire.rs apps/desktop/src-tauri/src/agent/mod.rs apps/desktop/src-tauri/src/agent/session/loop.rs apps/desktop/src-tauri/src/agent/session/compact.rs
git commit -m "feat(desktop): prefire compaction summaries safely"
```

### Task 7: Product acceptance, docs, and final audit

**Files:**

- Modify: `apps/desktop/src-tauri/src/protocol/events.rs` only if additive recovery/metric fields are needed
- Modify: `apps/desktop/src/lib/protocol.ts` in lockstep with Rust protocol
- Modify: `apps/desktop/e2e/fixtures/app.ts`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add acceptance fixtures for continuity and recovery**

Cover a compacted session with pending goal work, a running A2A task, a pending confirmation, edited paths, and a prepared checkpoint recovered after restart.

- [ ] **Step 2: Assert user-visible truth**

The UI must show the restored conversation, pending confirmation, A2A task/review state, and a recovery notice. It must not claim that an old model stream or tool process is still running.

- [ ] **Step 3: Run protocol and acceptance gates**

```bash
cd apps/desktop
npm run check:protocol
npm run test:e2e -- e2e/acceptance.spec.ts
npm run build
cd ../..
scripts/acceptance.sh --dry-run
```

Expected: all pass.

- [ ] **Step 4: Run GitNexus change detection**

```text
detect_changes({scope: "compare", base_ref: "main", repo: "forge"})
```

Expected domains: Agent, Session, IPC restore, protocol only if changed, acceptance, and docs. Investigate unrelated provider, credential, scheduler, or gateway impacts.

- [ ] **Step 5: Document feature-flag status and rollback**

State that checkpoint/capsule behavior is authoritative, while two-pass prefire remains off by default until production metrics show benefit without continuity regressions.

- [ ] **Step 6: Commit acceptance and documentation**

```bash
git add apps/desktop/src-tauri/src/protocol/events.rs apps/desktop/src/lib/protocol.ts apps/desktop/e2e README.md apps/desktop/README.md CHANGELOG.md
git commit -m "test(desktop): accept state-aware compaction recovery"
```
