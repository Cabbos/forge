# Forge Backend Next Stage Priority Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for implementation slices. Use GitNexus impact analysis before editing symbols, and run `detect_changes` before committing.

**Goal:** Stabilize Forge's backend around one reliable agent runtime after memory Phase 1. Merge confusing memory surfaces at the API/action layer first, then fix context accounting, permission/confirmation state, runtime durability, code-intelligence reliability, and leave gateway ownership for last.

**Decision:** Yes, the memory system should be merged, but in stages. Phase 1 unified the read and recall path without moving storage. Phase 2 should merge actions, audit, and status semantics behind one backend API. Physical storage migration should wait until the unified API has production evidence.

**Non-Goals For This Stage:**

- Do not migrate `wiki_memories.json`, `memory.json`, or `.forge/continuity.db` into one database yet.
- Do not make gateway the owner of the main loop until local runtime state, permission state, and context accounting are stable.
- Do not remove source-specific management panels until the unified action API covers their important operations.

---

## Priority Order

### P0: Memory Phase 2, Unified Actions And Audit

**Problem:** The user-facing memory story is still split. Phase 1 unified listing and send-input recall, but accept/archive/forget/pin/edit still live in source-specific flows.

**Implementation Scope:**

- Add a backend `UnifiedMemoryAction` layer that routes action requests to wiki memory, user facts, or continuity experiences by `source` and `source_id`.
- Add one audit payload for every injected memory: selected id, source, kind, score, reason, project/profile match, and whether it was injected or only shown.
- Add source adapters instead of copying action logic into the IPC command.
- Keep Settings as the detailed user-fact editor for now, but make Project Archive the unified overview and status surface.
- Add a small evidence fixture where one prompt recalls one wiki memory, one global/profile fact, and one continuity experience without duplicate hidden context.

**Primary Files:**

- `apps/desktop/src-tauri/src/memory/unified.rs`
- `apps/desktop/src-tauri/src/ipc/unified_memory.rs`
- `apps/desktop/src-tauri/src/ipc/send_input_context.rs`
- `apps/desktop/src/components/context/UnifiedMemorySection.tsx`
- `apps/desktop/e2e/acceptance.spec.ts`

**验收点:**

- Project Archive can show source, status, provenance, and recall reason for all three memory sources from one endpoint.
- One backend action endpoint can archive or forget a unified memory record and preserves source-specific storage rules.
- Send-input hidden memory context is formatted once under `## Work Memory`; continuity is not injected a second time.
- Tests cover accepted, pinned, archived, forgotten, wrong-project, and wrong-profile cases.

### P1: Context Budget And Usage Ledger Correctness

**Problem:** The composer/context number has already looked wrong. Backend-selected hidden context, UI-visible input, project records, memory records, and provider usage need one accounting contract.

**Implementation Scope:**

- Introduce one backend `ContextUsageEstimate` payload for visible input, hidden instructions, selected memory, project records, file attachments, compacted transcript, and reserved output.
- Emit that payload before provider dispatch and hydrate it into the composer instead of letting the UI infer partial numbers.
- Make provider-reported usage the post-run truth, but keep pre-run estimate conservative and explainable.
- Add duplicate-suppression tests for memory and continuity context after Phase 1 unification.

**Primary Files To Inspect:**

- `apps/desktop/src-tauri/src/ipc/send_input_context.rs`
- `apps/desktop/src-tauri/src/usage.rs`
- `apps/desktop/src/store/event-dispatch.ts`
- `apps/desktop/src/components/composer/*`
- `apps/desktop/e2e/composer.spec.ts`

**验收点:**

- Composer displays the same input/context estimate emitted by backend events.
- Hidden memory, continuity, and project records are counted exactly once.
- Provider usage updates replace or reconcile estimates without showing duplicate legacy usage.
- Acceptance includes a fixture with unified memory enabled and a fixture with no memory selected.

### P2: Permission And Confirmation State Hardening

**Problem:** Trust/full-access state and confirmation cards have been the most visible reliability pain. The backend must be the only authority, and the UI should only reflect replayable backend state.

**Implementation Scope:**

- Persist permission mode and trust source at the workspace/session boundary.
- Add a confirmation auto-decision ledger entry with decision, reason, affected files, risk tier, and policy source.
- Make takeover/confirmation cards derive eligibility from backend policy snapshots, not optimistic UI state.
- Keep external-path and sensitive-file safeguards explicit even in full-access mode.

**Primary Files To Inspect:**

- `apps/desktop/src-tauri/src/permission_handlers.rs`
- `apps/desktop/src-tauri/src/ipc/confirmations.rs`
- `apps/desktop/src-tauri/src/harness/permissions.rs`
- `apps/desktop/src/components/messages/ConfirmCard.tsx`
- `apps/desktop/e2e/acceptance.spec.ts`

**验收点:**

- Restarting or opening a new session preserves intended workspace trust/full-access state.
- A confirmation card shows only when backend policy says manual approval is required.
- Auto-approved steps have replayable evidence in transcript/history.
- External-path and secret-like operations still require manual confirmation.

### P3: Runtime Durability And Task Ledger

**Problem:** The loop runtime has many good pieces, but product confidence depends on restart recovery, task ownership, and background status being boringly consistent.

**Implementation Scope:**

- Add a backend runtime health snapshot that reports active runs, pending confirmations, task ledger state, queue depth, and last journal replay result.
- Add orphaned-run detection and a clear recovery action.
- Tighten background task status so UI cards, History, and Project Archive read the same run state.
- Keep disposable-loop evidence scripts aligned with acceptance gates.

**Primary Files To Inspect:**

- `apps/desktop/src-tauri/src/loop_runtime/*`
- `apps/desktop/src-tauri/src/agent/session/*`
- `apps/desktop/src-tauri/src/ipc/session_lifecycle.rs`
- `apps/desktop/src/store/*`
- `scripts/acceptance.sh`

**验收点:**

- Restart smoke can distinguish recovered, interrupted, and orphaned runs.
- Project status and transcript show the same pending confirmation state.
- Acceptance dry-run advertises every runtime recovery check that CI or manual evidence expects.

### P4: Code Intelligence Reliability

**Problem:** GitNexus has been useful but can hang or become stale. Backend development needs a predictable fallback path before it becomes a daily blocker.

**Implementation Scope:**

- Add a documented wrapper/runbook for GitNexus with timeout, stale-index detection, and a fallback to `rg`/static search.
- Evaluate CodeGraph only for the stuck paths, not as a wholesale replacement.
- Record when impact analysis is unavailable and what fallback evidence was used.
- Keep AGENTS requirements honest: if GitNexus cannot answer, the worker must say so and use fallback analysis before editing.

**验收点:**

- A stuck GitNexus command times out with a clear recovery message.
- Index freshness can be checked without blocking normal work.
- A fallback analysis report includes searched symbols, callers found, and residual risk.

### P5: Gateway Runtime Ownership, Last

**Problem:** Gateway will help once local runtime contracts are firm. If moved earlier, it will amplify the current ambiguity around memory, context, permissions, and recovery.

**Implementation Scope:**

- Define the gateway/local boundary only after P0-P3 pass.
- Move provider dispatch and long-running loop ownership behind a capability flag.
- Keep local fallback runnable for desktop self-use.
- Add gateway health, rollback, and parity tests before default enablement.

**验收点:**

- Gateway path and local path produce equivalent transcript, usage, memory-audit, permission, and recovery events for the same fixture.
- Disabling gateway returns to the current local path without data migration.
- Gateway failures degrade to visible, actionable status instead of silent stuck runs.

---

## Recommended Execution Sequence

1. **Task 1:** P0 unified memory actions and audit payload.
2. **Task 2:** P1 context usage estimate emitted by backend and consumed by composer.
3. **Task 3:** P2 permission/confirmation state replay and policy-source evidence.
4. **Task 4:** P3 runtime health snapshot and orphan recovery.
5. **Task 5:** P4 GitNexus reliability wrapper/runbook.
6. **Task 6:** P5 gateway ownership design gate after the above are green.

## Stage Definition Of Done

- `npm run build:desktop` passes.
- Focused Rust unit tests pass for memory, context usage, permissions, and runtime health changes.
- Product-level Playwright smoke covers the changed desktop surface.
- `scripts/acceptance.sh --dry-run` names the relevant checks.
- `README.md`, `apps/desktop/README.md`, and `CHANGELOG.md` are updated when user-visible runtime behavior changes.
- GitNexus `detect_changes` or a documented fallback impact report confirms the affected scope before commit.
