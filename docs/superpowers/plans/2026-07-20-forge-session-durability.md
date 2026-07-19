# Forge Session Durability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make ordinary Forge sessions crash-safe and reconstructable from an append-only canonical mutation journal while preserving existing snapshot compatibility.

**Architecture:** First make the existing snapshot cache atomic. Then add a per-session JSONL mutation journal in shadow mode, centralize conversation mutations behind small `AgentSession` helpers, build a deterministic replay projection, and promote journal fallback only after parity and corruption tests pass. `StreamEvent` remains the UI transport; the new journal is backend-only.

**Tech Stack:** Rust, serde/serde_json, file locking and atomic rename, Tauri desktop runtime, existing Forge snapshot/diagnostics/acceptance infrastructure.

---

## Scope and file map

**Create:**

- `apps/desktop/src-tauri/src/agent/session_journal.rs` — event schema, JSONL append/load, corruption classification.
- `apps/desktop/src-tauri/src/agent/session_projection.rs` — deterministic replay into snapshot-compatible state.
- `apps/desktop/src-tauri/src/agent/session_mutation.rs` — mutation helper API used by `AgentSession`.

**Modify:**

- `apps/desktop/src-tauri/src/agent/mod.rs` — register the three modules.
- `apps/desktop/src-tauri/src/agent/snapshot.rs` — atomic writes, sequence metadata, backup fallback.
- `apps/desktop/src-tauri/src/agent/session/mod.rs` — journal handle and controlled state mutation methods.
- `apps/desktop/src-tauri/src/agent/session/loop.rs` — append user/assistant/continuation mutations.
- `apps/desktop/src-tauri/src/agent/session/tools.rs` — append ordered tool-result message mutation.
- `apps/desktop/src-tauri/src/agent/session/compact.rs` — append conversation replacement mutation.
- `apps/desktop/src-tauri/src/ipc/session_builder.rs` — initialize journal in shadow mode.
- `apps/desktop/src-tauri/src/ipc/session_lifecycle.rs` — restore selection and replay fallback.
- `apps/desktop/src-tauri/src/diagnostics/mod.rs` — session journal parity/health check.
- `apps/desktop/e2e/acceptance.spec.ts` and `apps/desktop/e2e/fixtures/app.ts` — corruption/fallback product smoke.
- `README.md`, `apps/desktop/README.md`, `CHANGELOG.md` — user-visible recovery behavior.

## Required pre-change impact checks

Before the first edit in each task, run GitNexus impact on the exact symbol. The planning-time baseline found:

- `save_session_snapshot_at`: **CRITICAL**, 6 direct dependents, 28 total, 3 affected processes.
- `AgentSession` is a shared state hub and must be treated as **CRITICAL** even if a stale graph undercounts a specific method.
- Planning-time `execute_tools` reported LOW/zero upstream, but source inspection proves `execute_single_round` calls it; treat that graph result as incomplete.

If GitNexus is unavailable, record the fallback report required by `apps/desktop/AGENTS.md` before editing.

### Task 1: Make snapshot replacement atomic

**Files:**

- Modify: `apps/desktop/src-tauri/src/agent/snapshot.rs`
- Test: `apps/desktop/src-tauri/src/agent/snapshot.rs`

- [ ] **Step 1: Run impact analysis**

Run:

```text
impact({target: "save_session_snapshot_at", file_path: "apps/desktop/src-tauri/src/agent/snapshot.rs", direction: "upstream", includeTests: true})
```

Expected: CRITICAL warning acknowledged in the implementation log; direct callers and affected processes recorded before editing.

- [ ] **Step 2: Write atomic replacement tests**

Add focused tests that establish these facts:

```rust
#[test]
fn atomic_write_json_replaces_existing_snapshot_and_removes_tmp() {
    let root = temp_root("atomic-replace");
    let path = root.join("sessions").join("session-1.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, br#"{"summary":"first"}"#).unwrap();

    atomic_write_json(&path, br#"{"summary":"second"}"#).unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), r#"{"summary":"second"}"#);
    assert!(!path.with_extension("tmp").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn snapshot_listing_ignores_tmp_and_backup_files() {
    // Create session-1.json, session-1.tmp, and session-1.bak.
    // Assert only session-1 is returned.
}
```

- [ ] **Step 3: Run the tests and verify the first new assertion fails**

Run:

```bash
cd apps/desktop/src-tauri
cargo test agent::snapshot::tests::atomic_write_json_replaces_existing_snapshot_and_removes_tmp -- --exact
```

Expected: compilation FAIL because `atomic_write_json` does not exist yet.

- [ ] **Step 4: Add one local atomic JSON writer**

Keep the helper private to `snapshot.rs` in this task; do not start a repository-wide persistence refactor.

```rust
fn atomic_write_json(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    let mut file = fs::File::create(&tmp)
        .map_err(|e| format!("create snapshot tmp '{}': {e}", tmp.display()))?;
    use std::io::Write;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("flush snapshot tmp '{}': {e}", tmp.display()))?;
    fs::rename(&tmp, path)
        .map_err(|e| format!("replace session snapshot '{}': {e}", path.display()))
}
```

Change only the final write inside `save_session_snapshot_at` to call this helper. Preserve `created_at_ms` behavior and A2A ledger synchronization.

- [ ] **Step 5: Run snapshot tests**

Run:

```bash
cd apps/desktop/src-tauri
cargo test agent::snapshot::tests
```

Expected: all snapshot tests pass.

- [ ] **Step 6: Commit the atomic snapshot slice**

```bash
git add apps/desktop/src-tauri/src/agent/snapshot.rs
git commit -m "fix(desktop): replace session snapshots atomically"
```

### Task 2: Define the session mutation journal and corruption contract

**Files:**

- Create: `apps/desktop/src-tauri/src/agent/session_journal.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`
- Test: `apps/desktop/src-tauri/src/agent/session_journal.rs`

- [ ] **Step 1: Write schema and append/replay tests**

The test matrix must include monotonic sequence assignment, concurrent append serialization, torn final line, corrupt interior line, unknown schema version, and unsafe session id.

```rust
#[test]
fn torn_final_line_is_reported_but_valid_prefix_replays() {
    let store = test_store("torn-final");
    store.append(test_initialized()).unwrap();
    append_raw(store.path(), br#"{"schema_version":1"#);

    let loaded = store.load().unwrap();
    assert_eq!(loaded.events.len(), 1);
    assert_eq!(loaded.damage, Some(JournalDamage::TornFinalLine { line: 2 }));
}

#[test]
fn corrupt_interior_line_blocks_authoritative_replay() {
    // valid line, malformed line, valid line
    // Assert JournalLoadError::CorruptInteriorLine { line: 2 }.
}
```

- [ ] **Step 2: Run the new module test target**

Run:

```bash
cd apps/desktop/src-tauri
cargo test agent::session_journal
```

Expected: compilation fails because the module does not exist.

- [ ] **Step 3: Implement the backend-only event schema**

Use this initial shape:

```rust
pub(crate) const SESSION_JOURNAL_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SessionMutationEnvelope {
    pub schema_version: u32,
    pub event_id: String,
    pub session_id: String,
    pub sequence: u64,
    pub created_at_ms: u64,
    pub mutation: SessionMutation,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum SessionMutation {
    SessionInitialized { provider: String, model: String, working_dir: String },
    MessageAppended { message: ChatMessage },
    ConversationReplaced {
        checkpoint_id: String,
        messages: Vec<ChatMessage>,
        summary: Option<String>,
    },
    RuntimeStateUpdated { state: SessionRuntimeState },
}
```

`SessionRuntimeState` mirrors serializable snapshot runtime fields and uses `#[serde(default)]` on additive fields.

- [ ] **Step 4: Implement append and load with explicit damage classification**

Requirements:

- one shared process lock per normalized path, following `loop_runtime/journal.rs`;
- one JSON object and newline per append;
- `sync_data()` before success;
- validate session id and per-session monotonic sequence;
- ignore only a malformed non-newline-terminated final record;
- reject malformed interior records and sequence gaps.

- [ ] **Step 5: Run formatter, lint, and module tests**

```bash
cd apps/desktop/src-tauri
cargo fmt --check
cargo clippy --lib -- -D warnings
cargo test agent::session_journal
```

Expected: all pass.

- [ ] **Step 6: Commit the isolated journal module**

```bash
git add apps/desktop/src-tauri/src/agent/mod.rs apps/desktop/src-tauri/src/agent/session_journal.rs
git commit -m "feat(desktop): add canonical session mutation journal"
```

### Task 3: Build deterministic replay into a snapshot-compatible projection

**Files:**

- Create: `apps/desktop/src-tauri/src/agent/session_projection.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`
- Test: `apps/desktop/src-tauri/src/agent/session_projection.rs`

- [ ] **Step 1: Write projection tests before implementation**

Cover initialized session, append order, conversation replacement, runtime-state replacement, missing initialization, and duplicate initialization.

```rust
#[test]
fn conversation_replacement_discards_pre_checkpoint_messages() {
    let events = vec![
        initialized(1),
        appended(2, ChatMessage::user("old")),
        replaced(3, "checkpoint-1", vec![ChatMessage::user("retained")], Some("summary")),
        appended(4, ChatMessage::assistant(json!("new"))),
    ];

    let projection = SessionProjection::from_events(&events).unwrap();
    assert_eq!(projection.messages.len(), 2);
    assert_eq!(projection.summary.as_deref(), Some("summary"));
    assert_eq!(projection.last_sequence, 4);
}
```

- [ ] **Step 2: Run the projection tests and observe failure**

```bash
cd apps/desktop/src-tauri
cargo test agent::session_projection
```

Expected: FAIL before implementation.

- [ ] **Step 3: Implement a pure replay reducer**

```rust
impl SessionProjection {
    pub(crate) fn from_events(events: &[SessionMutationEnvelope]) -> Result<Self, String> {
        let mut projection = None;
        for event in events {
            validate_next_sequence(projection.as_ref(), event)?;
            apply_event(&mut projection, event)?;
        }
        projection.ok_or_else(|| "session journal has no initialization event".to_string())
    }
}
```

Keep file IO out of this module. It accepts an event slice and returns deterministic state.

- [ ] **Step 4: Add snapshot parity conversion**

Implement `SessionProjection::to_snapshot()` and a comparator that ignores only `created_at_ms`/`updated_at_ms` skew explicitly documented by the test.

- [ ] **Step 5: Run projection and snapshot tests**

```bash
cd apps/desktop/src-tauri
cargo test agent::session_projection
cargo test agent::snapshot::tests
```

Expected: all pass.

- [ ] **Step 6: Commit the pure projection**

```bash
git add apps/desktop/src-tauri/src/agent/mod.rs apps/desktop/src-tauri/src/agent/session_projection.rs
git commit -m "feat(desktop): replay session mutations into projections"
```

### Task 4: Centralize conversation mutations and run the journal in shadow mode

**Files:**

- Create: `apps/desktop/src-tauri/src/agent/session_mutation.rs`
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/mod.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/loop.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/tools.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session/compact.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/session_builder.rs`
- Test: `apps/desktop/src-tauri/src/agent/session/loop_test.rs`
- Test: `apps/desktop/src-tauri/src/agent/session/tools_test.rs`

- [ ] **Step 1: Run impact checks on every mutation symbol**

At minimum check `AgentSession`, `execute_single_round`, `execute_tools`, `apply_compaction_emitter`, and the session builder function selected by source inspection. Warn before proceeding for HIGH or CRITICAL results.

- [ ] **Step 2: Write mutation-order tests**

Add tests proving:

- user append is journaled before the provider call;
- assistant message is journaled before tool dispatch;
- ordered tool result message is one journal mutation;
- continuation prompts are journaled;
- compaction uses `ConversationReplaced`, never a series of delete events;
- a journal append failure stops the corresponding in-memory mutation in shadow mode only when `FORGE_SESSION_JOURNAL_STRICT=1`; default shadow mode logs diagnostics and preserves current behavior.

- [ ] **Step 3: Introduce a narrow mutation helper**

```rust
impl AgentSession {
    pub(crate) fn append_conversation_message(
        &self,
        message: ChatMessage,
        source: SessionMutationSource,
    ) -> Result<(), String> {
        self.session_journal.append_message(message.clone(), source)?;
        lock_unpoisoned(&self.messages).push(message);
        Ok(())
    }

    pub(crate) fn replace_conversation(
        &self,
        checkpoint_id: String,
        messages: Vec<ChatMessage>,
        summary: Option<String>,
    ) -> Result<(), String> {
        self.session_journal.append_replacement(checkpoint_id, messages.clone(), summary.clone())?;
        *lock_unpoisoned(&self.messages) = messages;
        *lock_unpoisoned(&self.summary) = summary;
        Ok(())
    }
}
```

Do not expose raw journal mutation to provider or UI layers.

- [ ] **Step 4: Replace direct conversation writes one site at a time**

Change and test in this order:

1. initial user message in `loop.rs`;
2. assistant message after sampling;
3. auto-continuation message;
4. final-summary assistant message;
5. ordered tool-result message in `tools.rs`;
6. compaction replacement in `compact.rs`.

Run the relevant targeted test after each site; do not batch all edits before testing.

- [ ] **Step 5: Add journal initialization in the session builder**

Default to shadow mode. Existing restored snapshots without a journal initialize a new journal generation from one `SessionInitialized` event plus one `ConversationReplaced` baseline event. Record the imported snapshot schema and sequence in diagnostics metadata.

- [ ] **Step 6: Run the desktop backend gate**

```bash
cd apps/desktop
npm run check:backend
```

Expected: format, clippy, and Rust tests pass.

- [ ] **Step 7: Commit the shadow integration**

```bash
git add apps/desktop/src-tauri/src/agent apps/desktop/src-tauri/src/ipc/session_builder.rs
git commit -m "feat(desktop): shadow ordinary session mutations"
```

### Task 5: Add parity diagnostics and journal-backed restore fallback

**Files:**

- Modify: `apps/desktop/src-tauri/src/agent/snapshot.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/session_lifecycle.rs`
- Modify: `apps/desktop/src-tauri/src/diagnostics/mod.rs`
- Test: `apps/desktop/src-tauri/src/ipc/session_lifecycle_tests.rs`
- Test: `apps/desktop/src-tauri/src/diagnostics/mod.rs`

- [ ] **Step 1: Write restore-selection tests**

Test this exact matrix:

| Snapshot | Journal | Expected restore |
|---|---|---|
| valid and same sequence | valid | snapshot, parity healthy |
| valid but behind | valid | journal projection |
| missing | valid | journal projection |
| corrupt | valid | journal projection + recovery notice |
| valid | corrupt interior | snapshot + journal quarantine notice |
| corrupt | corrupt interior | fresh session + recovery notice |
| valid | torn final | journal valid prefix if newer, with warning |

- [ ] **Step 2: Add explicit sequence metadata to snapshots**

```rust
#[serde(default)]
pub journal_generation: Option<String>,
#[serde(default)]
pub journal_sequence: u64,
```

Legacy snapshots deserialize with sequence zero.

- [ ] **Step 3: Implement restore selection as a pure function**

```rust
fn choose_session_restore_source(
    snapshot: Result<Option<AgentSessionSnapshot>, SnapshotLoadFailure>,
    journal: Result<Option<SessionJournalLoad>, JournalLoadError>,
) -> SessionRestoreDecision
```

Keep file reads and UI event emission outside the selector so the full matrix is unit-testable.

- [ ] **Step 4: Add diagnostics summary**

Expose counts for healthy parity, snapshot-behind, torn final line, corrupt interior, quarantined, and journal-only sessions. Never include conversation body text.

- [ ] **Step 5: Run focused restore and diagnostics tests**

```bash
cd apps/desktop/src-tauri
cargo test ipc::session_lifecycle_tests
cargo test diagnostics
```

Expected: all pass.

- [ ] **Step 6: Commit fallback restore**

```bash
git add apps/desktop/src-tauri/src/agent/snapshot.rs apps/desktop/src-tauri/src/ipc/session_lifecycle.rs apps/desktop/src-tauri/src/ipc/session_lifecycle_tests.rs apps/desktop/src-tauri/src/diagnostics/mod.rs
git commit -m "feat(desktop): recover sessions from mutation journals"
```

### Task 6: Add product acceptance and documentation

**Files:**

- Modify: `apps/desktop/e2e/fixtures/app.ts`
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
- Modify: `scripts/acceptance.sh` only if the advertised spec list changes
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add mocked journal-recovery acceptance state**

Extend the existing contract-shaped fixture with a recovery notice and diagnostics payload representing a corrupt snapshot recovered from a healthy journal. Do not expose a test-only IPC shape.

- [ ] **Step 2: Add user-visible acceptance assertions**

The test must verify:

- recovered session appears in History;
- opening it renders the expected visible conversation blocks;
- a recovery notice explains that the snapshot was replaced from durable history;
- Diagnostics reports journal parity as healthy after repair.

- [ ] **Step 3: Run acceptance and repository gates**

```bash
cd apps/desktop
npm run test:e2e -- e2e/acceptance.spec.ts
cd ../..
npm run build:desktop
npm run test:eval
scripts/acceptance.sh --dry-run
```

Expected: all pass.

- [ ] **Step 4: Update documentation with exact authority wording**

Document that session snapshots are restore caches, mutation journals provide recovery, malformed interior events fail closed, and no provider stream/tool process survives restart.

- [ ] **Step 5: Commit acceptance and docs**

```bash
git add apps/desktop/e2e README.md apps/desktop/README.md CHANGELOG.md scripts/acceptance.sh
git commit -m "test(desktop): accept journal-backed session recovery"
```

### Task 7: Final change-impact and promotion audit

**Files:** none expected beyond fixes discovered by the audit.

- [ ] **Step 1: Compare the completed implementation against main**

Run:

```text
detect_changes({scope: "compare", base_ref: "main", repo: "forge"})
```

Expected: affected flows are limited to session mutation, snapshot save/restore, diagnostics, and acceptance surfaces. Investigate any provider, credential, scheduler, or unrelated gateway flow.

- [ ] **Step 2: Prove shadow replay parity on fixtures**

Run the parity test corpus for:

- plain chat;
- assistant plus multiple tool calls;
- permission denial;
- compaction;
- pending confirmation snapshot;
- interrupted tool snapshot;
- A2A state;
- legacy snapshot import.

Expected: journal replay equals the saved snapshot modulo explicitly normalized timestamps.

- [ ] **Step 3: Run the full prescribed verification**

```bash
npm run build:desktop
npm run build:website
npm run test:eval
scripts/acceptance.sh --dry-run
```

- [ ] **Step 4: Keep journal authority conservative**

Ship with journal fallback enabled only for missing/corrupt/stale snapshots. Do not delete snapshots, remove legacy loaders, or compact journal generations in this plan.

- [ ] **Step 5: Commit audit fixes if required**

Stage each concrete file named by `git diff --name-only` individually, inspect the staged diff, then run:

```bash
git commit -m "fix(desktop): close session durability audit gaps"
```
