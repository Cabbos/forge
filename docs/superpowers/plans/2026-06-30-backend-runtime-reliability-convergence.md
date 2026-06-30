# Backend Runtime Reliability Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring Forge's backend/runtime from internal-beta capable to restart-tolerant, CI-observable, and safe for longer self-use loops without claiming unattended autonomous delivery.

**Architecture:** Treat the Rust/Tauri backend and gateway runtime as the source of truth, then prove every long-running path through CI, persistent run records, and restart smoke tests. This phase tightens existing `eval_headless`, `gateway`, `loop_runtime`, permission, and acceptance surfaces rather than introducing a new runtime abstraction.

**Tech Stack:** Tauri Rust backend, Tokio, JSON-line gateway IPC, Node CI/eval helper scripts, GitHub Actions, Playwright acceptance smoke, Cargo tests, Node test runner.

---

## Current Baseline

- `main` is at `1b0ebe6d`.
- Open PR count is `0`.
- Latest `Desktop backend` CI job passes on `main`.
- Latest full scheduled/workflow-dispatch CI fails in `Nightly eval`, specifically at `npm run eval:forge:mock`.
- `Nightly eval` currently runs from `apps/desktop`, calls `apps/desktop/scripts/run-forge-backtest.mjs`, and that script spawns `uv` in the sibling `apps/eval-runner` directory.
- The scheduled `Nightly eval` job currently sets up Node and `npm ci`, but does not explicitly install Python or `uv`, while the normal `Eval runner` job does.
- `eval_headless::run_request` can create a real `AgentSession` for eval flows, but gateway autonomous resume remains disabled by policy and is not a default product claim.
- Existing documentation explicitly does not claim: default autonomous gateway continuation, auto commit/merge/push, official Tauri/WebDriver force-quit proof, shell-internal tracing, syscall/file-descriptor tracing, full non-git workspace enumeration, or billing-grade cost accounting.

## File Structure

- `.github/workflows/ci.yml`
  - Add the runtime dependencies required by the scheduled `Nightly eval` job.
- `apps/desktop/scripts/run-forge-backtest.mjs`
  - Add explicit spawn-error diagnostics and keep artifact paths discoverable when the runner cannot start.
- `apps/desktop/scripts/run-forge-backtest.test.mjs`
  - Cover runner spawn failures and dry-run/runtime command planning.
- `apps/desktop/src-tauri/src/gateway/runner.rs`
  - Make gateway-trigger execution records more explicit about executor kind, failure category, lease timing, and retry/dead-letter status.
- `apps/desktop/src-tauri/src/gateway/protocol.rs`
  - Expose any new gateway status fields through stable JSON protocol types.
- `apps/desktop/src-tauri/src/gateway/server.rs`
  - Route gateway status/list calls to the richer run projection.
- `apps/desktop/src-tauri/src/loop_runtime/headless.rs`
  - Keep approval/readiness facts durable and explicit for future gateway-owned runs.
- `apps/desktop/src-tauri/src/eval_headless/mod.rs`
  - Add narrow evidence fields only if needed to distinguish eval-owned headless runs from gateway-owned runs.
- `apps/desktop/scripts/smoke-gateway-restart.mjs`
  - New backend restart smoke: isolated HOME, start gateway, enqueue/list, stop, restart, assert persisted status.
- `apps/desktop/scripts/smoke-gateway-restart.test.mjs`
  - Contract tests for the restart smoke script's plan, isolated paths, and failure output.
- `scripts/acceptance.sh`
  - Add a dry-run gate for the backend restart smoke and gateway session-host status.
- `scripts/acceptance.test.mjs`
  - Keep acceptance label/command matrix synchronized.
- `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`
  - Update only when runtime behavior or user-visible diagnostics change.
- `docs/desktop/state-consistency-map.md`
  - Add gateway run state and restart persistence as tracked state surfaces.

## Non-Goals

- Do not enable default unattended gateway continuation.
- Do not add auto commit, auto merge, auto push, or automatic PR creation.
- Do not claim official macOS Tauri/WebDriver force-quit coverage until there is a real driver-backed harness.
- Do not broaden headless confirmation auto-approval beyond the currently documented safe boundaries.
- Do not scan non-git workspaces recursively to infer shell side effects.

---

### Task 1: Make Nightly Eval Green And Observable

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `apps/desktop/scripts/run-forge-backtest.mjs`
- Test: `apps/desktop/scripts/run-forge-backtest.test.mjs`

- [ ] **Step 1: Write a failing spawn-error test**

Add this export to the import list in `apps/desktop/scripts/run-forge-backtest.test.mjs`:

```js
import {
  buildBacktestPlan,
  checkApiKey,
  createSuiteCaseFile,
  runBacktestProcess,
  selectCaseFiles,
} from "./run-forge-backtest.mjs";
```

Add this test near the existing CLI/process tests:

```js
test("runBacktestProcess reports runner spawn errors", () => {
  const messages = [];
  const originalError = console.error;
  console.error = (message) => messages.push(String(message));
  try {
    const status = runBacktestProcess({
      command: "definitely-not-a-forge-command",
      args: ["--version"],
      cwd: resolve("."),
      env: process.env,
    });

    assert.equal(status, 1);
    assert.match(messages.join("\n"), /failed to start definitely-not-a-forge-command/);
  } finally {
    console.error = originalError;
  }
});
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
npm --prefix apps/desktop run eval:forge:test -- --test-name-pattern "spawn errors"
```

Expected before implementation: fail because `runBacktestProcess` is not exported.

- [ ] **Step 3: Implement explicit spawn diagnostics**

In `apps/desktop/scripts/run-forge-backtest.mjs`, add this exported helper above `runCli`:

```js
export function runBacktestProcess(plan) {
  const result = spawnSync(plan.command, plan.args, {
    cwd: plan.cwd,
    env: plan.env,
    stdio: "inherit",
  });

  if (result.error) {
    console.error(
      `[forge-backtest] ERROR: failed to start ${plan.command}: ${result.error.message}`,
    );
    console.error(`[forge-backtest] cwd: ${plan.cwd}`);
    console.error(`[forge-backtest] command: ${[plan.command, ...plan.args].join(" ")}`);
    return 1;
  }

  return result.status ?? 1;
}
```

Replace the inline `spawnSync` block at the end of `runCli` with:

```js
return runBacktestProcess(plan);
```

- [ ] **Step 4: Add Python and uv setup to Nightly eval**

In `.github/workflows/ci.yml`, update the `nightly-eval` steps so they mirror the eval runner's runtime setup before `npm ci`:

```yaml
      - uses: actions/setup-python@v5
        with:
          python-version: "3.11"
      - uses: astral-sh/setup-uv@v5
        with:
          enable-cache: true
      - run: npm ci
```

Keep this inside the existing `nightly-eval` job and leave the ordinary `eval-runner` job unchanged.

- [ ] **Step 5: Verify locally**

Run:

```bash
npm --prefix apps/desktop run eval:forge:test
npm --prefix apps/desktop run eval:forge:mock
```

Expected: tests pass; mock eval writes an artifact under `apps/desktop/artifacts/eval-runs/`.

- [ ] **Step 6: Verify CI contract**

Run:

```bash
npm run check:ci
```

Expected: workflow contract tests pass.

- [ ] **Step 7: Commit**

```bash
git add .github/workflows/ci.yml apps/desktop/scripts/run-forge-backtest.mjs apps/desktop/scripts/run-forge-backtest.test.mjs
git commit -m "fix(ci): make nightly eval runner observable"
```

**Acceptance Points:**
- Scheduled or workflow-dispatch CI has a green `Nightly eval` job.
- If the eval runner executable is missing, logs name the missing command and cwd.
- `Desktop backend` remains green.

---

### Task 2: Stabilize Gateway Session-Host Run Records

**Files:**
- Modify: `apps/desktop/src-tauri/src/gateway/runner.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/protocol.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/server.rs`
- Test: `apps/desktop/src-tauri/src/gateway/runner.rs`

- [ ] **Step 1: Write run-record contract tests**

Add tests in the existing `#[cfg(test)] mod tests` in `apps/desktop/src-tauri/src/gateway/runner.rs`:

```rust
#[tokio::test]
async fn trigger_run_record_preserves_executor_kind_and_failure_category() {
    let workspace = tempfile::tempdir().expect("workspace");
    let store = TriggerStore::new();
    let run_store = super::TriggerRunStore::new();
    store.push(PendingTrigger {
        id: "trigger-observable".into(),
        message: "run observable task".into(),
        profile_id: None,
        provider: None,
        model: None,
        workspace_path: Some(workspace.path().to_string_lossy().to_string()),
        attempt_count: 0,
        claimed_at_ms: None,
        received_at_ms: 20,
    });

    let records = super::run_pending_triggers_once(
        &store,
        &run_store,
        workspace.path(),
        |_request| async {
            Ok(serde_json::json!({
                "error": "missing_api_key",
                "failure_category": "runner_error",
                "failure_reason": "headless setup failed"
            }))
        },
    )
    .await;

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].status, "retrying");
    assert_eq!(records[0].executor_kind.as_deref(), Some("eval_headless"));
    assert_eq!(records[0].failure_category.as_deref(), Some("runner_error"));
}
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml trigger_run_record_preserves_executor_kind_and_failure_category --lib
```

Expected before implementation: fail because the new fields do not exist.

- [ ] **Step 3: Add explicit run-record fields**

Extend `TriggerRunRecord` in `apps/desktop/src-tauri/src/gateway/runner.rs`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub executor_kind: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub failure_category: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub lease_expires_at_ms: Option<u64>,
```

Update `TriggerRunRecord::from_trigger` to initialize:

```rust
executor_kind: Some("eval_headless".to_string()),
failure_category: None,
lease_expires_at_ms: trigger.claimed_at_ms.map(|claimed| claimed + TRIGGER_LEASE_TIMEOUT_MS),
```

When recording failures from an eval payload, set:

```rust
record.failure_category = failure_category_from_payload(payload);
```

Add this helper:

```rust
fn failure_category_from_payload(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("failure_category")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
```

- [ ] **Step 4: Expose fields through gateway status**

If `gateway/protocol.rs` has mirrored run DTOs, add the same optional fields there using `#[serde(default, skip_serializing_if = "Option::is_none")]`. If the protocol already serializes `TriggerRunRecord` directly, no protocol type split is needed.

- [ ] **Step 5: Verify Rust gateway tests**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway::runner --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway --lib
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/gateway/runner.rs apps/desktop/src-tauri/src/gateway/protocol.rs apps/desktop/src-tauri/src/gateway/server.rs
git commit -m "feat(gateway): expose session host run evidence"
```

**Acceptance Points:**
- Every gateway-trigger run record says which executor owned it.
- Retry/dead-letter records preserve a failure category when the headless payload provides one.
- Gateway status can distinguish `completed`, `retrying`, `dead_letter`, and setup failures without reading logs.

---

### Task 3: Add Backend Gateway Restart Smoke

**Files:**
- Create: `apps/desktop/scripts/smoke-gateway-restart.mjs`
- Create: `apps/desktop/scripts/smoke-gateway-restart.test.mjs`
- Modify: `apps/desktop/package.json`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`

- [ ] **Step 1: Add smoke script contract tests**

Create `apps/desktop/scripts/smoke-gateway-restart.test.mjs`:

```js
import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { buildGatewayRestartPlan } from "./smoke-gateway-restart.mjs";

test("buildGatewayRestartPlan uses isolated HOME and stable store paths", () => {
  const root = mkdtempSync(join(tmpdir(), "forge-gateway-restart-test-"));
  try {
    const plan = buildGatewayRestartPlan({ root });

    assert.equal(plan.home, join(root, "home"));
    assert.match(plan.gatewayCommand.join(" "), /cargo run .* --bin gateway/);
    assert.equal(plan.triggerStorePath, join(root, "home", ".forge", "triggers.json"));
    assert.equal(plan.runStorePath, join(root, "home", ".forge", "trigger-runs.json"));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
```

- [ ] **Step 2: Implement dry-run capable restart smoke**

Create `apps/desktop/scripts/smoke-gateway-restart.mjs` with:

```js
import { mkdirSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

export function buildGatewayRestartPlan({ root = mkdtempSync(join(tmpdir(), "forge-gateway-restart-")) } = {}) {
  const home = join(root, "home");
  const repoRoot = resolve("..", "..");
  return {
    root,
    home,
    triggerStorePath: join(home, ".forge", "triggers.json"),
    runStorePath: join(home, ".forge", "trigger-runs.json"),
    gatewayCommand: [
      "cargo",
      "run",
      "--manifest-path",
      join(repoRoot, "apps", "desktop", "src-tauri", "Cargo.toml"),
      "--bin",
      "gateway",
      "--quiet",
    ],
  };
}

function main(argv) {
  const dryRun = argv.includes("--dry-run");
  const json = argv.includes("--json");
  const plan = buildGatewayRestartPlan();
  mkdirSync(plan.home, { recursive: true });

  if (dryRun || json) {
    console.log(JSON.stringify({ ok: true, dryRun, plan }, null, 2));
    return 0;
  }

  console.error("[smoke-gateway-restart] live restart execution is intentionally added after the dry-run contract passes.");
  return 2;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  process.exitCode = main(process.argv.slice(2));
}
```

- [ ] **Step 3: Run script tests**

Run:

```bash
node --test apps/desktop/scripts/smoke-gateway-restart.test.mjs
node apps/desktop/scripts/smoke-gateway-restart.mjs --json --dry-run
```

Expected: pass and print isolated paths.

- [ ] **Step 4: Add npm script**

In `apps/desktop/package.json`, add:

```json
"smoke:gateway:restart": "node scripts/smoke-gateway-restart.mjs"
```

- [ ] **Step 5: Add acceptance dry-run gate**

In `scripts/acceptance.sh`, add a gate label:

```bash
add_gate "backend gateway restart smoke dry-run" "npm --prefix apps/desktop run smoke:gateway:restart -- --json --dry-run"
```

In `scripts/acceptance.test.mjs`, assert the label exists:

```js
assert.match(output, /backend gateway restart smoke dry-run/);
```

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/scripts/smoke-gateway-restart.mjs apps/desktop/scripts/smoke-gateway-restart.test.mjs apps/desktop/package.json scripts/acceptance.sh scripts/acceptance.test.mjs
git commit -m "test(gateway): add restart smoke harness shell"
```

**Acceptance Points:**
- The restart smoke has a deterministic isolated HOME.
- Acceptance advertises the restart smoke before any live restart claim is made.
- The live restart path is not claimed until the dry-run contract is stable.

---

### Task 4: Promote Headless Ownership From Status To Guarded Runtime Contract

**Files:**
- Modify: `apps/desktop/src-tauri/src/loop_runtime/headless.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/runner.rs`
- Modify: `apps/desktop/src-tauri/src/gateway/server.rs`
- Test: `apps/desktop/src-tauri/src/loop_runtime/headless.rs`
- Test: `apps/desktop/src-tauri/src/gateway/runner.rs`

- [ ] **Step 1: Add approval/readiness tests for gateway-owned runs**

In `loop_runtime/headless.rs`, add:

```rust
#[test]
fn headless_owner_run_requires_approval_bundle_before_agent_session_adapter() {
    let run = HeadlessOwnerRun {
        owner_run_id: "owner-1".into(),
        task_id: "task-1".into(),
        session_id: Some("session-1".into()),
        lease_id: "lease-1".into(),
        attempt: 1,
        state: HeadlessOwnerRunState::Ready,
        snapshot_source: HeadlessOwnerSnapshotSource::PersistedSessionSnapshot,
        snapshot_ref: Some("snapshot-1".into()),
        human_gate_id: "gate-1".into(),
        policy_decision_id: "policy-1".into(),
        budget_snapshot_id: "budget-1".into(),
        idempotency_key: "task-1:attempt-1".into(),
        correlation_id: "correlation-1".into(),
        causation_id: None,
        requested_by: "gateway".into(),
        requested_at_ms: 100,
        heartbeat_at_ms: None,
        expires_at_ms: 200,
        cancellation_reason: None,
        waiting_reason: None,
        executor_kind: HeadlessOwnerExecutorKind::AgentSessionAdapter,
        evidence_refs: vec!["approval:gate-1".into(), "policy:policy-1".into()],
    };

    assert!(run.validate_authorization_bundle().is_ok());
}
```

- [ ] **Step 2: Run headless contract tests**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_owner --lib
```

Expected: pass after adding the test if the existing contract is sufficient; otherwise fix validation without broadening permissions.

- [ ] **Step 3: Gate gateway execution on explicit approval facts**

In `gateway/runner.rs`, before calling `run_headless_request`, derive and record a denied run when gateway execution lacks explicit approval. The denied record must use:

```rust
status: "approval_required"
message: "Gateway headless execution requires explicit human approval for this task."
executor_kind: Some("none".to_string())
failure_category: Some("approval_required".to_string())
```

Keep existing eval-trigger behavior behind current tests until the approval source is wired. Do not silently auto-approve.

- [ ] **Step 4: Add gateway tests for approval-required status**

Add a `run_pending_triggers_once_requires_approval_for_gateway_owned_execution` test in `gateway/runner.rs` that pushes a trigger, runs the executor path without approval, and asserts:

```rust
assert_eq!(records[0].status, "approval_required");
assert_eq!(records[0].failure_category.as_deref(), Some("approval_required"));
assert!(!store.list().is_empty(), "approval-required trigger should remain inspectable");
```

- [ ] **Step 5: Verify contract**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_resume --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway::runner --lib
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/loop_runtime/headless.rs apps/desktop/src-tauri/src/gateway/runner.rs apps/desktop/src-tauri/src/gateway/server.rs
git commit -m "feat(runtime): gate gateway headless ownership"
```

**Acceptance Points:**
- Gateway-owned headless work has a visible approval-required state.
- No gateway path creates a headless `AgentSession` without explicit approval facts.
- The UI/status layer can explain why a run is waiting instead of silently doing nothing.

---

### Task 5: Extend Acceptance, Docs, And Release Boundary Language

**Files:**
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`
- Modify: `docs/desktop/state-consistency-map.md`
- Modify: `apps/desktop/README.md`
- Modify: `README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add gateway run state to the consistency map**

In `docs/desktop/state-consistency-map.md`, add a state surface row:

```markdown
| Gateway run state | Rust `TriggerRunStore` plus gateway JSON-line status | Settings gateway runtime, CLI trigger status, acceptance restart smoke | trigger enqueue/claim/complete/retry/dead-letter, gateway restart | trigger appears lost or running after restart | `npm --prefix apps/desktop run smoke:gateway:restart -- --json --dry-run` and `cargo test ... gateway::runner --lib` |
```

- [ ] **Step 2: Add acceptance labels for the new backend gates**

In `scripts/acceptance.sh`, include:

```bash
add_gate "gateway session-host run evidence" "cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway::runner --lib"
add_gate "backend gateway restart smoke dry-run" "npm --prefix apps/desktop run smoke:gateway:restart -- --json --dry-run"
```

In `scripts/acceptance.test.mjs`, assert both labels appear in `--dry-run` output.

- [ ] **Step 3: Update README boundary language**

In `apps/desktop/README.md`, replace the backend reliability paragraph with wording that states:

```markdown
Gateway trigger runs now keep explicit executor, retry/dead-letter, failure-category, and restart-smoke evidence. This proves backend-visible ownership and persistence, but it still does not claim unattended autonomous continuation, auto commit/merge/push, or official Tauri/WebDriver force-quit recovery.
```

- [ ] **Step 4: Update changelog**

Add under `CHANGELOG.md` unreleased section:

```markdown
- Hardened backend runtime reliability with observable nightly eval failures, gateway session-host run evidence, and a backend restart-smoke dry-run gate.
```

- [ ] **Step 5: Run full relevant gates**

Run:

```bash
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
npm --prefix apps/desktop run eval:forge:test
npm --prefix apps/desktop run smoke:gateway:restart -- --json --dry-run
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway::runner --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_resume --lib
git diff --check
```

Expected: all pass.

- [ ] **Step 6: Run GitNexus change detection before commit**

Run the project-required GitNexus change detector:

```bash
gitnexus detect_changes --scope compare --base-ref main
```

If the local GitNexus CLI uses JSON-style arguments instead, run the equivalent:

```bash
gitnexus detect_changes '{"scope":"compare","base_ref":"main"}'
```

Expected: affected symbols are limited to gateway runner/protocol, headless contract, eval helper scripts, acceptance scripts, and docs.

- [ ] **Step 7: Commit**

```bash
git add scripts/acceptance.sh scripts/acceptance.test.mjs docs/desktop/state-consistency-map.md apps/desktop/README.md README.md CHANGELOG.md
git commit -m "docs(runtime): define backend reliability gates"
```

**Acceptance Points:**
- The acceptance matrix advertises every new backend reliability gate.
- Docs say exactly what is proven and what remains unclaimed.
- GitNexus reports the expected blast radius before final merge.

---

## Final Phase Acceptance

Run these before opening the phase PR:

```bash
npm run check:ci
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
npm --prefix apps/desktop run eval:forge:test
npm --prefix apps/desktop run eval:forge:mock
npm --prefix apps/desktop run smoke:gateway:restart -- --json --dry-run
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml gateway::runner --lib
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml headless_resume --lib
npm --prefix apps/desktop run check:backend
git diff --check
```

Remote acceptance:

- `Nightly eval` is green on workflow dispatch or schedule.
- `Desktop backend` is green on `main`.
- No open PR remains blocked by this phase.

## Risk Notes

- `Nightly eval` may expose additional eval-case failures after `uv` is installed. Treat those as real eval failures, not CI setup failures.
- Gateway restart live smoke should start as dry-run plus contract tests. Only claim live restart once process start/stop is stable on CI and local macOS.
- Headless approval records must stay task-scoped and expiry-bounded.
- Commit/merge/push remains human-gated throughout this phase.

## Self-Review

- Spec coverage: covers Nightly eval failure, gateway session-host observability, backend restart proof, guarded headless ownership, and acceptance/docs alignment.
- Placeholder scan: no task uses TBD/TODO/fill-in language; implementation steps include concrete files, commands, and expected results.
- Type consistency: field names introduced in `TriggerRunRecord` are reused consistently as `executor_kind`, `failure_category`, and `lease_expires_at_ms`.
