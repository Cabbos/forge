# Forge H1 Credential Preflight Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent a DeepSeek baseline attempt from creating a paid pending record until the exact Forge headless command has proven that its configured credential is available without starting a provider request.

**Architecture:** Add a typed `credential_preflight` mode to the Forge headless stdin/stdout protocol. `ForgeAgentRunner` executes that mode in the same isolated runtime environment as a real task, validates provider/model/no-call evidence, and the baseline CLI gates `run_baseline_campaign` on the result. The existing immutable blocked campaign remains evidence; a new campaign is created only after the preflight passes.

**Tech Stack:** Rust/Tokio/Serde, Python/Pydantic/subprocess control, pytest, Cargo tests, ForgeBench baseline ledger.

---

### Task 1: Add a no-call Forge headless credential mode

**Files:**
- Modify: `apps/desktop/src-tauri/src/eval_headless/types.rs`
- Modify: `apps/desktop/src-tauri/src/eval_headless/mod.rs`
- Modify: `apps/desktop/src-tauri/src/settings.rs`

- [x] **Step 1: Write the failing Rust tests**

Add tests which deserialize `mode: "credential_preflight"` and prove:

```rust
assert_eq!(request.mode, EvalHeadlessMode::CredentialPreflight);
assert!(request.use_provider_credential_store);
```

Add an async environment-key test that invokes `run_request`, then asserts:

```rust
assert_eq!(payload["credential_preflight"]["ok"], true);
assert_eq!(payload["credential_preflight"]["provider"], "deepseek");
assert_eq!(payload["credential_preflight"]["model"], "deepseek-v4-flash");
assert_eq!(payload["credential_preflight"]["provider_call_started"], false);
assert!(payload.get("forge_run_evidence").is_none());
```

- [x] **Step 2: Run the focused Rust tests and observe RED**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml eval_headless::tests::credential_preflight --lib
```

Expected: compilation/test failure because the mode and payload do not exist.

- [x] **Step 3: Implement the typed mode and provider-store opt-in**

Add:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalHeadlessMode {
    #[default]
    Run,
    CredentialPreflight,
}
```

Extend `EvalHeadlessRequest` with `mode` and `use_provider_credential_store`.
Add a `CredentialResolver` method which resolves the canonical
`CredentialRef::provider(provider)` only when explicitly requested. In
`run_request`, return the bounded `credential_preflight` payload immediately
after credential/provider/model resolution and before adapter/session creation.
Never serialize the credential value.

- [x] **Step 4: Re-run focused Rust tests**

Run the command from Step 2.

Expected: PASS with no provider/network call.

### Task 2: Gate the baseline transaction in Python

**Files:**
- Modify: `apps/eval-runner/app/runner.py`
- Modify: `apps/eval-runner/app/baseline_cli.py`
- Modify: `apps/eval-runner/tests/test_runner.py`
- Modify: `apps/eval-runner/tests/test_baseline_cli.py`

- [x] **Step 1: Write the failing Python runner test**

Use a fake command that accepts only the preflight mode and returns:

```python
{
    "credential_preflight": {
        "ok": True,
        "provider": "deepseek",
        "model": "deepseek-v4-flash",
        "provider_call_started": False,
        "error": None,
    }
}
```

Assert `ForgeAgentRunner.credential_preflight()` returns the typed identity and
does not include a task prompt.

- [x] **Step 2: Write the failing baseline CLI transaction test**

Initialize a campaign, run it against a fake preflight returning `ok: false`,
and assert:

```python
assert completed.stderr == "error: baseline_run_preflight_failed\n"
assert (campaign / "budget.jsonl").read_bytes() == b""
assert list((campaign / "pending").iterdir()) == []
assert list((campaign / "attempts").iterdir()) == []
```

- [x] **Step 3: Run the focused Python tests and observe RED**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_runner.py tests/test_baseline_cli.py \
  -k "credential_preflight or offline_attempt" -q
```

Expected: failure because the runner method and CLI gate do not exist.

- [x] **Step 4: Implement the minimal typed runner and CLI gate**

Add a Pydantic `ForgeCredentialPreflight` result and a method that uses the
same command, isolated HOME, runtime state, provider/model, timeout, and
provider-store opt-in as real task execution. Validate exact identity and
`provider_call_started is False`. Call it in `baseline_cli._run` before
`run_baseline_campaign`; a failed probe returns the stable error without
touching pending, attempts, report, or budget files.

- [x] **Step 5: Re-run focused Python tests**

Run the command from Step 3.

Expected: PASS.

### Task 3: Verify, commit, and restart the paid smoke safely

**Files:**
- Modify: `docs/forgebench-h1-baseline.md`
- Modify: `apps/eval-runner/README.md`
- Modify: `CHANGELOG.md`

- [x] **Step 1: Document the preflight boundary**

State that `baseline run` performs an exact-command credential probe before
publishing `provider_call_pending`, that the probe sets
`provider_call_started=false`, and that probe failures consume neither a
campaign attempt nor ledger budget.

- [x] **Step 2: Run all offline gates**

Run:

```bash
npm run test:eval
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml eval_headless --lib
cd apps/eval-runner
uv run ruff check app tests
uv run ruff format --check .
uv run mypy app
cd ../..
scripts/acceptance.sh --only "DeepSeek H1 paid-baseline contract"
npm run build:desktop
```

Expected: all PASS.

- [x] **Step 3: Detect changes and commit**

Run GitNexus `detect_changes(scope="staged")` after staging only the files in
this plan, then commit:

```bash
git commit -m "fix(eval): preflight paid baseline credentials"
```

- [ ] **Step 4: Rebuild from a clean detached worktree**

Create a detached worktree at the remediation commit, build
`forge_eval_agent`, initialize a new `h1-anthropic-control-v2` campaign, and
run `baseline verify`. Preserve the v1 blocked campaign unchanged.

- [ ] **Step 5: Execute one paid smoke only after preflight succeeds**

Run:

```bash
uv run python -m app.cli baseline run \
  --campaign artifacts/h1-baseline/anthropic-control-v2 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json \
  --max-attempts 1
```

Expected: the credential preflight succeeds without a provider call, then
exactly one paid attempt is recorded with known DeepSeek usage and cost no
greater than 1 CNY. Stop on any blocker and never retry automatically.

---

## Self-review

- Spec coverage: the plan covers the observed missing-credential root cause,
  the no-provider-call proof, the transaction boundary, stable errors,
  no-secret output, immutable failed evidence, clean rebuild, and one-call
  restart.
- Type consistency: `EvalHeadlessMode`, `ForgeCredentialPreflight`,
  `provider_call_started`, and `use_provider_credential_store` use the same
  spelling across Rust, Python, tests, and documentation.
- Placeholder scan: the plan contains no deferred implementation or
  unspecified error-handling step.
