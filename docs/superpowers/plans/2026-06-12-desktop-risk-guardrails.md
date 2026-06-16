# Desktop Risk Guardrails Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add low-cost, high-value guardrails around Forge desktop's three biggest risks: StreamEvent protocol drift, oversized `agent/session.rs`, and missing tests in core modules.

**Architecture:** Keep each fix self-contained and additive: (1) a TypeScript/Rust cross-check test that fails when a backend event type is not dispatched on the frontend; (2) an internal refactor that splits `agent/session.rs` into focused coordinator modules without changing behavior; (3) backfill unit tests for `harness`, `executor`, and `memory` using test doubles and existing fixtures.

**Tech Stack:** Rust (Tauri), TypeScript (React/Zustand), Vitest for TS unit tests, `cargo test` for Rust, Node.js scripts for static cross-checks.

---

## Slice 0: Baseline & Repo State

**Files:**
- Read: `apps/desktop/src-tauri/src/protocol/events.rs`
- Read: `apps/desktop/src/lib/protocol.ts`
- Read: `apps/desktop/src/store/event-dispatch.ts`
- Read: `apps/desktop/src/store/blocks.ts`
- Read: `apps/desktop/src-tauri/src/agent/session.rs`
- Read: `apps/desktop/src-tauri/src/harness/mod.rs`
- Read: `apps/desktop/src-tauri/src/executor/mod.rs`
- Read: `apps/desktop/src-tauri/src/memory/mod.rs`

- [ ] **Step 0.1: Verify current build/test commands work**

Run:
```bash
cd /Users/cabbos/project/forge/apps/desktop
npm run build          # TypeScript + Vite build
npm test               # TS unit tests
cargo test             # Rust tests
```

Expected: At least `cargo test` passes; note any pre-existing failures.

- [ ] **Step 0.2: Create feature branch**

```bash
cd /Users/cabbos/project/forge
git checkout -b cabbos/desktop-risk-guardrails
```

---

## Slice 1: Protocol Cross-Check Guardrail

**Files:**
- Create: `apps/desktop/scripts/check-protocol-sync.mjs`
- Modify: `apps/desktop/package.json` (add `scripts` entry)
- Modify: `apps/desktop/src/store/event-dispatch.ts` (minor, only if needed to expose dispatch coverage)

### Task 1.1: Extract event type lists

- [ ] **Step 1.1.1: Write the cross-check script skeleton**

Create `apps/desktop/scripts/check-protocol-sync.mjs`:

```javascript
#!/usr/bin/env node
/**
 * Cross-checks that every `event_type` emitted by the Rust StreamEvent enum
 * has a handler branch in the frontend event dispatcher.
 *
 * Exit 0 when in sync; exit 1 with a detailed diff when Rust emits an event
 * that the frontend ignores.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "../..");
const desktopRoot = path.resolve(repoRoot, "apps/desktop");

const rustEventsPath = path.join(
  desktopRoot,
  "src-tauri/src/protocol/events.rs",
);
const dispatchPath = path.join(
  desktopRoot,
  "src/store/event-dispatch.ts",
);

function parseRustEventTypes(source) {
  const types = new Set();
  const renameRe = /#\[serde\(rename\s*=\s*"([^"]+)"\)\]/g;
  for (const [, name] of source.matchAll(renameRe)) {
    types.add(name);
  }
  return types;
}

function parseHandledEventTypes(source) {
  const types = new Set();

  // explicit string comparisons: event_type === "foo"
  const eqRe = /event_type\s*===?\s*"([^"]+)"/g;
  for (const [, name] of source.matchAll(eqRe)) {
    types.add(name);
  }

  // array membership: ["a", "b"]
  const arrayRe = /\[\s*(["'][^\]]+["'](?:\s*,\s*["'][^\]]+["'])?)\s*\]/g;
  for (const [, body] of source.matchAll(arrayRe)) {
    for (const m of body.matchAll(/["']([^"']+)["']/g)) {
      types.add(m[1]);
    }
  }

  return types;
}

function main() {
  const rustSource = fs.readFileSync(rustEventsPath, "utf8");
  const dispatchSource = fs.readFileSync(dispatchPath, "utf8");

  const rustTypes = parseRustEventTypes(rustSource);
  const handledTypes = parseHandledEventTypes(dispatchSource);

  const missing = [...rustTypes].filter((t) => !handledTypes.has(t)).sort();

  if (missing.length === 0) {
    console.log(`OK: all ${rustTypes.size} Rust StreamEvent types are handled in event-dispatch.ts`);
    process.exit(0);
  }

  console.error("FAIL: Rust emits event types that the frontend does not handle:");
  for (const m of missing) {
    console.error(`  - ${m}`);
  }
  console.error("\nAdd a branch to src/store/event-dispatch.ts or update the parser whitelist.");
  process.exit(1);
}

main();
```

- [ ] **Step 1.1.2: Run the script and expect it to fail**

```bash
cd /Users/cabbos/project/forge/apps/desktop
node scripts/check-protocol-sync.mjs
```

Expected: FAIL listing unhandled Rust-only event types (e.g. `agent_a2a_updated`, `delivery_summary`, etc. if the regex misses arrays).

### Task 1.2: Refine parsing to match real dispatcher shape

- [ ] **Step 1.2.1: Improve array parsing for `CHUNK_TYPES` and `END_TYPES`**

Replace the generic array regex in `check-protocol-sync.mjs` with a targeted scan that reads any string literal appearing inside `CHUNK_TYPES` / `END_TYPES` arrays, plus explicit `event_type ===` comparisons.

Updated parsing:

```javascript
function parseHandledEventTypes(source) {
  const types = new Set();

  // explicit comparisons
  const eqRe = /event_type\s*===?\s*"([^"]+)"/g;
  for (const [, name] of source.matchAll(eqRe)) {
    types.add(name);
  }

  // membership arrays declared as const FOO = ["a", "b"]
  const arrayConstRe = /const\s+\w+\s*=\s*\[([^\]]+)\]/g;
  for (const [, body] of source.matchAll(arrayConstRe)) {
    for (const m of body.matchAll(/"([^"]+)"/g)) {
      types.add(m[1]);
    }
  }

  return types;
}
```

- [ ] **Step 1.2.2: Run again and document intentionally unhandled types**

Run the script. If `diagnostics_update` is intentionally a no-op, add an explicit `event_type === "diagnostics_update"` early return in `event-dispatch.ts` (already present) so the script sees it as handled.

Expected: All 37 Rust event types are either explicitly handled in `event-dispatch.ts` or listed in a project-allowed ignore list.

### Task 1.3: Add allow-list for project-level intentional no-ops

- [ ] **Step 1.3.1: Add ALLOWED_UNHANDLED list**

Add near the top of `check-protocol-sync.mjs`:

```javascript
// Event types the backend may emit but that the UI intentionally drops.
// Each entry must have a one-line justification comment.
const ALLOWED_UNHANDLED = new Set([
  // "diagnostics_update", // handled: remove if you add a diagnostics panel
]);
```

Update `main()` to subtract this set before reporting failures.

- [ ] **Step 1.3.2: Wire into CI/pre-commit**

Modify `apps/desktop/package.json` scripts:

```json
{
  "scripts": {
    "check:protocol": "node scripts/check-protocol-sync.mjs"
  }
}
```

Modify `apps/desktop/scripts/pre-commit-check.test.mjs` (if it exists) to run:

```javascript
import { execSync } from "node:child_process";

test("protocol sync check passes", () => {
  execSync("npm run check:protocol", { cwd: process.cwd(), stdio: "pipe" });
});
```

- [ ] **Step 1.3.3: Commit**

```bash
git add apps/desktop/scripts/check-protocol-sync.mjs apps/desktop/package.json apps/desktop/scripts/pre-commit-check.test.mjs
git commit -m "feat(desktop): add StreamEvent protocol cross-check guardrail

Adds a Node script that parses Rust StreamEvent event_type tags and the
frontend dispatcher, failing the build when a backend event has no frontend
handler. Wired into npm run check:protocol and the pre-commit test suite.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Slice 2: Split `agent/session.rs`

**Files:**
- Create: `apps/desktop/src-tauri/src/agent/session/loop.rs`
- Create: `apps/desktop/src-tauri/src/agent/session/compact.rs`
- Create: `apps/desktop/src-tauri/src/agent/session/lifecycle.rs`
- Create: `apps/desktop/src-tauri/src/agent/session/a2a.rs`
- Modify: `apps/desktop/src-tauri/src/agent/session.rs` (shrink to public API and data model)
- Modify: `apps/desktop/src-tauri/src/agent/mod.rs` (if needed to re-export)

### Task 2.1: Establish new module layout

- [ ] **Step 2.1.1: Create `agent/session/mod.rs` or directory module**

Rename current `src/agent/session.rs` to `src/agent/session/mod.rs` **without changing contents yet**, and create placeholder submodules:

```bash
cd /Users/cabbos/project/forge/apps/desktop/src-tauri/src/agent
mkdir session
mv session.rs session/mod.rs
touch session/loop.rs
touch session/compact.rs
touch session/lifecycle.rs
touch session/a2a.rs
```

- [ ] **Step 2.1.2: Verify cargo check still passes**

```bash
cargo check
```

Expected: Pass (module path unchanged).

### Task 2.2: Extract lifecycle helpers

- [ ] **Step 2.2.1: Move session start/stop/pause helpers**

In `session/mod.rs`, find public methods:
- `pub(crate) fn start(...)`
- `pub(crate) fn stop(...)`
- `pub(crate) fn pause(...)`
- `pub(crate) fn resume(...)`
- `pub(crate) fn set_goal_ledger(...)`
- `pub(crate) fn clear_goal_ledger(...)`

Move their bodies into `session/lifecycle.rs` as free functions taking `&AgentSession` where appropriate, leaving thin wrapper methods in `mod.rs`.

Example extraction:

```rust
// session/lifecycle.rs
use std::sync::Arc;
use crate::agent::session::{AgentSession, SessionStatus};
use crate::agent::session_guards::lock_unpoisoned;

pub(crate) fn stop(session: &AgentSession, reason: impl Into<String>) {
    let reason = reason.into();
    session.running.store(false, std::sync::atomic::Ordering::SeqCst);
    *lock_unpoisoned(&session.status) = SessionStatus::Stopped;
    // ... existing body continues
}
```

Update `mod.rs`:

```rust
pub(crate) mod lifecycle;

impl AgentSession {
    pub(crate) fn stop(&self, reason: impl Into<String>) {
        lifecycle::stop(self, reason);
    }
}
```

- [ ] **Step 2.2.2: Run cargo test for lifecycle tests**

```bash
cargo test agent::session
```

Expected: All existing `agent::session_tests` still pass.

### Task 2.3: Extract compaction logic

- [ ] **Step 2.3.1: Move compact methods**

Move methods related to context compaction from `session/mod.rs` into `session/compact.rs`:

- `prepare_compaction`
- `apply_compaction`
- `finalize_compaction`
- `compact_now`
- `maybe_compact`

Keep the same locking order and `AutoCompactGuard` semantics.

- [ ] **Step 2.3.2: Run cargo test for compact tests**

```bash
cargo test compact
```

Expected: Pass.

### Task 2.4: Extract A2A coordination

- [ ] **Step 2.4.1: Move A2A helper methods**

Move methods that interact with `self.a2a_bus` into `session/a2a.rs`:

- `assign_a2a_task`
- `start_a2a_task`
- `record_a2a_progress`
- `complete_a2a_task`
- `fail_a2a_task`

These are pure helpers that lock `a2a_bus` and emit events.

- [ ] **Step 2.4.2: Run cargo test for A2A tests**

```bash
cargo test a2a
```

Expected: Pass.

### Task 2.5: Extract main turn loop

- [ ] **Step 2.5.1: Move `run_one_turn` / `run_turn_loop` into `session/loop.rs`**

The core loop function (likely the largest block in `session.rs`) moves to `session/loop.rs`. It should remain an `impl AgentSession` method or a free function taking `&Arc<AgentSession>`.

```rust
// session/loop.rs
use std::sync::Arc;
use crate::agent::session::AgentSession;

pub(crate) async fn run_turn_loop(session: Arc<AgentSession>) {
    // existing loop body
}
```

Update callers in `mod.rs` to call `loop::run_turn_loop(self.clone()).await`.

- [ ] **Step 2.5.2: Run full cargo test**

```bash
cargo test
```

Expected: Pass.

### Task 2.6: Clean up `session/mod.rs` and commit

- [ ] **Step 2.6.1: Reduce `mod.rs` to data model + thin API**

Goal: `mod.rs` under 600 lines. It contains:
- `AgentSession` struct + impl block with constructors
- `SessionStatus` enum
- Re-exports
- Thin wrapper methods dispatching to `lifecycle`, `compact`, `a2a`, `loop`

- [ ] **Step 2.6.2: Verify line counts**

```bash
wc -l apps/desktop/src-tauri/src/agent/session/*.rs
```

Expected: `mod.rs` < 600 lines, no single submodule > 800 lines.

- [ ] **Step 2.6.3: Commit**

```bash
git add apps/desktop/src-tauri/src/agent/session/
git rm apps/desktop/src-tauri/src/agent/session.rs
git commit -m "refactor(agent): split session.rs into focused modules

Splits the 2126-line session.rs into lifecycle, compact, a2a, and loop
submodules. Behavior unchanged; all existing tests pass.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Slice 3: Backfill Tests for Core Modules

**Files:**
- Create: `apps/desktop/src-tauri/src/harness/permissions_test.rs`
- Create: `apps/desktop/src-tauri/src/harness/capability_test.rs`
- Create: `apps/desktop/src-tauri/src/executor/files_test.rs`
- Create: `apps/desktop/src-tauri/src/executor/shell_test.rs`
- Create: `apps/desktop/src-tauri/src/memory/scoring_test.rs`
- Modify: existing `mod.rs` files to include `#[cfg(test)] mod *_test;`

### Task 3.1: Harness permission gate tests

- [ ] **Step 3.1.1: Inspect `permissions.rs` public API**

Read `apps/desktop/src-tauri/src/harness/permissions.rs` and identify:
- `PermissionGate`
- `PermissionDecision`
- Method signatures for `request_permission` / `decide`

- [ ] **Step 3.1.2: Write tests**

Create `apps/desktop/src-tauri/src/harness/permissions_test.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::super::permissions::{PermissionDecision, PermissionGate, PermissionRequest};

    #[test]
    fn default_gate_denies_dangerous_write() {
        let gate = PermissionGate::default();
        let req = PermissionRequest::WriteFile {
            path: "/etc/passwd".into(),
        };
        assert_eq!(gate.decide(&req), PermissionDecision::Deny);
    }

    #[test]
    fn allowlisted_path_permits_write() {
        let mut gate = PermissionGate::default();
        gate.allow_prefix("/tmp/forge-test");
        let req = PermissionRequest::WriteFile {
            path: "/tmp/forge-test/foo.txt".into(),
        };
        assert_eq!(gate.decide(&req), PermissionDecision::Allow);
    }
}
```

Adjust struct/variant names to match the real `permissions.rs` API.

- [ ] **Step 3.1.3: Run harness tests**

```bash
cargo test harness::permissions
```

Expected: Pass (or fail with API mismatch to fix in next step).

### Task 3.2: Executor file write tests

- [ ] **Step 3.2.1: Inspect `executor/files.rs` public API**

Read `apps/desktop/src-tauri/src/executor/files.rs`.

- [ ] **Step 3.2.2: Write tests with temp dir**

Create `apps/desktop/src-tauri/src/executor/files_test.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::TempDir;
    use super::super::files::FileExecutor;

    #[test]
    fn write_file_creates_content() {
        let tmp = TempDir::new().unwrap();
        let exec = FileExecutor::new(tmp.path().to_path_buf());
        exec.write_file("hello.txt", "world").unwrap();
        let content = fs::read_to_string(tmp.path().join("hello.txt")).unwrap();
        assert_eq!(content, "world");
    }

    #[test]
    fn read_file_returns_content() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), "data").unwrap();
        let exec = FileExecutor::new(tmp.path().to_path_buf());
        let content = exec.read_file("a.txt").unwrap();
        assert_eq!(content, "data");
    }
}
```

Add `tempfile` to `Cargo.toml` dev-dependencies if not present.

- [ ] **Step 3.2.3: Run executor tests**

```bash
cargo test executor::files
```

Expected: Pass.

### Task 3.3: Memory scoring tests

- [ ] **Step 3.3.1: Inspect `memory/scoring.rs` public API**

Read `apps/desktop/src-tauri/src/memory/scoring.rs`.

- [ ] **Step 3.3.2: Write scoring tests**

Create `apps/desktop/src-tauri/src/memory/scoring_test.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::super::scoring::score_memory_relevance;
    use super::super::model::{MemoryCategory, MemoryScope, WikiMemory};

    fn make_memory(title: &str, body: &str) -> WikiMemory {
        WikiMemory {
            id: "m1".into(),
            category: MemoryCategory::ProjectFact,
            scope: MemoryScope::Project,
            status: crate::memory::MemoryStatus::Accepted,
            title: title.into(),
            body: body.into(),
            project_path: None,
            source_session_id: None,
            source_message_ids: vec![],
            confidence: 1.0,
            created_at: "0".into(),
            updated_at: "0".into(),
            last_used_at: None,
            use_count: 0,
            tags: vec![],
        }
    }

    #[test]
    fn exact_title_match_scores_high() {
        let m = make_memory("auth flow", "user login details");
        let scores = score_memory_relevance(&[m], "auth flow design", 3);
        assert_eq!(scores.len(), 1);
        assert!(scores[0].score > 0.5);
    }
}
```

- [ ] **Step 3.3.3: Run memory tests**

```bash
cargo test memory::scoring
```

Expected: Pass.

### Task 3.4: Shell policy and executor tests

- [ ] **Step 3.4.1: Add shell command validation tests**

Create `apps/desktop/src-tauri/src/executor/shell_test.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::super::shell::ShellExecutor;
    use tempfile::TempDir;

    #[test]
    fn run_echo_command() {
        let tmp = TempDir::new().unwrap();
        let exec = ShellExecutor::new(tmp.path().to_path_buf());
        let out = exec.run_captured("echo hello").unwrap();
        assert!(out.stdout.contains("hello"));
    }

    #[test]
    fn rejects_command_with_shell_metacharacters() {
        let tmp = TempDir::new().unwrap();
        let exec = ShellExecutor::new(tmp.path().to_path_buf());
        assert!(exec.run_captured("echo foo; rm -rf /").is_err());
    }
}
```

- [ ] **Step 3.4.2: Run executor tests**

```bash
cargo test executor::shell
```

Expected: Pass.

### Task 3.5: Wire tests and commit

- [ ] **Step 3.5.1: Add test module declarations**

In each `mod.rs`, add:

```rust
#[cfg(test)]
mod permissions_test;
```

(replace with appropriate test module names for each subsystem.)

- [ ] **Step 3.5.2: Run full cargo test**

```bash
cargo test
```

Expected: All new + existing tests pass.

- [ ] **Step 3.5.3: Commit**

```bash
git add apps/desktop/src-tauri/src/harness/ apps/desktop/src-tauri/src/executor/ apps/desktop/src-tauri/src/memory/
git commit -m "test(desktop): backfill unit tests for harness, executor, memory

Adds focused tests for permission gating, file I/O, shell execution, and
memory scoring. No production behavior changes.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Slice 4: Final Verification

- [ ] **Step 4.1: Run all desktop checks**

```bash
cd /Users/cabbos/project/forge/apps/desktop
npm run build
npm test
cargo test
npm run check:protocol
```

Expected: All green.

- [ ] **Step 4.2: Run gitnexus detect_changes**

```bash
cd /Users/cabbos/project/forge
npx gitnexus detect-changes
```

Expected: Only expected symbols/processes affected.

- [ ] **Step 4.3: Push branch**

```bash
git push -u origin cabbos/desktop-risk-guardrails
```

---

## Self-Review Checklist

- [ ] Spec coverage: protocol check, session split, and core tests each have dedicated tasks.
- [ ] Placeholder scan: no TBD/TODO; all code blocks contain concrete content.
- [ ] Type consistency: `StreamEvent`, `AgentSession`, `PermissionGate`, `WikiMemory`, `FileExecutor`, `ShellExecutor` names match the codebase.
- [ ] Each slice leaves the project buildable and testable.
- [ ] Commits are atomic and rollback-friendly.

---

## Execution Options

**Plan complete and saved to `docs/superpowers/plans/2026-06-12-desktop-risk-guardrails.md`.**

Two execution options:

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per slice, review between slices, fast iteration. Use `superpowers:subagent-driven-development`.

2. **Inline Execution** — Execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints.

Which approach would you like?
