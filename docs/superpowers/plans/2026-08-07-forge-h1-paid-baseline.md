# Forge H1 Paid Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pin and execute a resumable, budget-safe, fully attributable 10-case × 3-trial DeepSeek V4 Flash baseline over the frozen ForgeBench development split.

**Architecture:** Keep the existing `AgentTrace` contract unchanged. A strict harness profile controls the existing Anthropic-compatible Forge runner, while a separate baseline campaign stores the full profile, source identity, attempt identity, trusted trace, provider-reported usage, CNY accounting, and hash-chained budget entry for every attempt. The controller writes a pending record before any paid call, refuses silent retries, stops on unknown usage or trust, and regenerates the aggregate report only from verified attempt artifacts.

**Tech Stack:** Python 3.11, Pydantic 2, Decimal CNY accounting, SHA-256 canonical JSON, existing Forge headless Rust binary, pytest, Rust unit tests, Git.

---

## File structure

- `apps/eval-runner/app/harness_profiles.py`: strict profile and price-table models, canonical loading, hashing, and H1 identity validation.
- `apps/eval-runner/app/provider_usage.py`: provider-usage normalization and exact CNY calculation; legacy usage is evidence only and is never double-counted.
- `apps/eval-runner/app/baseline_models.py`: strict campaign, pending-attempt, completed-attempt, ledger, and aggregate-report schemas.
- `apps/eval-runner/app/baseline.py`: initialization, paid-attempt transaction boundaries, resume/recovery, budget hard stops, verification, and report generation.
- `apps/eval-runner/app/baseline_cli.py`: `baseline init|run|status|recover|report|verify` command surface.
- `apps/eval-runner/forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json`: frozen H1 control profile and official CNY price snapshot.
- `apps/eval-runner/tests/test_harness_profiles.py`: profile validation and fingerprint tests.
- `apps/eval-runner/tests/test_provider_usage.py`: CNY accounting and unknown-usage tests.
- `apps/eval-runner/tests/test_baseline.py`: transaction, budget, resume, trust, and report tests.
- `apps/eval-runner/tests/test_baseline_cli.py`: offline CLI end-to-end tests through a deterministic fake Forge command.
- `docs/forgebench-h1-baseline.md`: operator workflow, budget contract, results, and claim boundary.

Existing files receive only narrow changes:

- `apps/eval-runner/app/runner.py`: allow the measured runner to send the profile-selected real provider/model and retain the headless-reported agent identity as a normalized raw event.
- `apps/eval-runner/app/cli.py`: dispatch the isolated `baseline` command group.
- `scripts/acceptance.sh`, `scripts/acceptance.test.mjs`, `README.md`, `apps/eval-runner/README.md`, and `CHANGELOG.md`: publish and lock the offline H1 contract.

`AgentTrace` is intentionally not modified: its GitNexus impact is CRITICAL. Preflight impact is MEDIUM for `ForgeAgentRunner` (5 direct/12 total) and `app.cli.main` (5 direct/6 total), and LOW for `ForgeAgentRunner.run_task` and `_trace_from_payload`. No HIGH or CRITICAL existing symbol is edited in H1.

---

### Task 1: Freeze the strict H1 control profile and official CNY price table

**Files:**

- Create: `apps/eval-runner/app/harness_profiles.py`
- Create: `apps/eval-runner/tests/test_harness_profiles.py`
- Create: `apps/eval-runner/forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json`

- [ ] **Step 1: Write failing profile tests**

Create tests that require:

```python
def test_h1_profile_is_strict_canonical_and_development_only() -> None:
    profile = load_harness_profile(PROFILE)
    assert profile.schema_version == 1
    assert profile.profile_id == "deepseek-v4-flash-anthropic-baseline-v1"
    assert profile.provider == "deepseek"
    assert profile.model == "deepseek-v4-flash"
    assert profile.transport == "anthropic_messages"
    assert profile.dataset_split == DatasetSplit.DEVELOPMENT
    assert profile.repetitions == 3
    assert profile.max_attempts == 30
    assert profile.max_cost_cny_per_task == Decimal("1")
    assert profile.campaign_budget_cny == Decimal("8")
    assert profile.global_budget_cny == Decimal("100")
    assert len(harness_profile_fingerprint(profile)) == 64


def test_h1_price_table_matches_official_cny_snapshot() -> None:
    profile = load_harness_profile(PROFILE)
    assert profile.price_table.input_cache_hit_cny_per_million == Decimal("0.02")
    assert profile.price_table.input_cache_miss_cny_per_million == Decimal("1")
    assert profile.price_table.output_cny_per_million == Decimal("2")
    assert profile.price_table.source_url == (
        "https://api-docs.deepseek.com/zh-cn/quick_start/pricing/"
    )
    assert profile.price_table.retrieved_at == date(2026, 8, 7)
```

Also reject extra fields, non-canonical identity, holdout paths/splits, nonpositive limits, inconsistent `repetitions * development_case_count != max_attempts`, price-table currency other than CNY, non-HTTPS sources, secret-like keys/values, NaN/Infinity, and a mutable unversioned profile ID.

- [ ] **Step 2: Run the tests and confirm the red state**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_harness_profiles.py -q
```

Expected: FAIL because `app.harness_profiles` and the profile do not exist.

- [ ] **Step 3: Implement strict models and canonical hashing**

Use these public types and functions:

```python
class PriceTableV1(EvalModel):
    currency: Literal["CNY"]
    model: Literal["deepseek-v4-flash"]
    input_cache_hit_cny_per_million: Decimal = Field(gt=0, allow_inf_nan=False)
    input_cache_miss_cny_per_million: Decimal = Field(gt=0, allow_inf_nan=False)
    output_cny_per_million: Decimal = Field(gt=0, allow_inf_nan=False)
    source_url: AnyHttpUrl
    retrieved_at: date


class HarnessProfileV1(EvalModel):
    schema_version: Literal[1] = 1
    profile_id: Literal["deepseek-v4-flash-anthropic-baseline-v1"]
    provider: Literal["deepseek"]
    model: Literal["deepseek-v4-flash"]
    transport: Literal["anthropic_messages"]
    endpoint_family: Literal["anthropic"]
    reasoning_effort: Literal["provider_default"]
    thinking_mode: Literal["enabled_default"]
    prompt_mode: Literal["current_product_v1"]
    context_policy: Literal["current_product_v1"]
    tool_result_policy: Literal["current_product_v1"]
    parallel_read_tools: Literal[True]
    max_model_rounds: int = Field(ge=1, le=80)
    stagnation_policy: Literal["current_loop_guard_v1"]
    verification_policy: Literal["required_targeted_then_full_v1"]
    repair_attempt_limit: int = Field(ge=0, le=1)
    dataset_split: Literal[DatasetSplit.DEVELOPMENT]
    development_case_count: Literal[10]
    repetitions: Literal[3]
    max_attempts: Literal[30]
    max_cost_cny_per_task: Decimal = Field(gt=0, allow_inf_nan=False)
    campaign_budget_cny: Decimal = Field(gt=0, allow_inf_nan=False)
    global_budget_cny: Decimal = Field(gt=0, allow_inf_nan=False)
    price_table: PriceTableV1


def load_harness_profile(path: Path) -> HarnessProfileV1: ...
def harness_profile_bytes(profile: HarnessProfileV1) -> bytes: ...
def harness_profile_fingerprint(profile: HarnessProfileV1) -> str: ...
```

Serialize with the existing `canonical_json_bytes` and `sha256_bytes` primitives. Re-read the file after parsing and reject it unless its bytes equal canonical JSON plus one newline. Scan both keys and values for secret-bearing names/patterns.

- [ ] **Step 4: Add the canonical profile**

Write exactly:

```json
{
  "campaign_budget_cny": "8",
  "context_policy": "current_product_v1",
  "dataset_split": "development",
  "development_case_count": 10,
  "endpoint_family": "anthropic",
  "global_budget_cny": "100",
  "max_attempts": 30,
  "max_cost_cny_per_task": "1",
  "max_model_rounds": 48,
  "model": "deepseek-v4-flash",
  "parallel_read_tools": true,
  "price_table": {
    "currency": "CNY",
    "input_cache_hit_cny_per_million": "0.02",
    "input_cache_miss_cny_per_million": "1",
    "model": "deepseek-v4-flash",
    "output_cny_per_million": "2",
    "retrieved_at": "2026-08-07",
    "source_url": "https://api-docs.deepseek.com/zh-cn/quick_start/pricing/"
  },
  "profile_id": "deepseek-v4-flash-anthropic-baseline-v1",
  "prompt_mode": "current_product_v1",
  "reasoning_effort": "provider_default",
  "repair_attempt_limit": 1,
  "repetitions": 3,
  "schema_version": 1,
  "stagnation_policy": "current_loop_guard_v1",
  "thinking_mode": "enabled_default",
  "tool_result_policy": "current_product_v1",
  "transport": "anthropic_messages",
  "verification_policy": "required_targeted_then_full_v1"
}
```

- [ ] **Step 5: Run profile tests**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_harness_profiles.py -q
uv run ruff check app/harness_profiles.py tests/test_harness_profiles.py
uv run mypy app/harness_profiles.py
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/eval-runner/app/harness_profiles.py \
  apps/eval-runner/tests/test_harness_profiles.py \
  apps/eval-runner/forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json
git commit -m "feat(eval): freeze DeepSeek H1 baseline profile"
```

---

### Task 2: Make the existing Forge runner consume and preserve measured identity

**Files:**

- Modify: `apps/eval-runner/app/runner.py`
- Modify: `apps/eval-runner/tests/test_runner.py`

- [ ] **Step 1: Write failing runner tests**

Add a fake headless command that asserts stdin contains:

```json
{
  "provider": "deepseek",
  "model": "deepseek-v4-flash",
  "task": {
    "max_model_rounds": 48,
    "max_repair_attempts": 1
  }
}
```

It returns `forge_agent_provider="deepseek"` and
`forge_agent_model="deepseek-v4-flash"`. Assert the resulting trace contains:

```python
assert identity_event(trace) == {
    "event_type": "eval_agent_identity",
    "provider": "deepseek",
    "model": "deepseek-v4-flash",
    "transport": "anthropic_messages",
    "harness_fingerprint": SHA,
}
```

Also test missing/mismatched returned provider/model. A mismatch must add an
`eval_agent_identity` event with `identity_matches_profile=False`; it must not
rewrite the provider's actual claim.

- [ ] **Step 2: Run the focused test and confirm failure**

```bash
cd apps/eval-runner
uv run pytest tests/test_runner.py -k "profiled_agent_identity" -q
```

Expected: FAIL because `ForgeAgentRunner` cannot receive the measured identity.

- [ ] **Step 3: Add optional measured identity to `ForgeAgentRunner`**

Extend `__init__` with:

```python
agent_provider: str | None = None,
agent_model: str | None = None,
agent_transport: str | None = None,
harness_fingerprint: str | None = None,
max_model_rounds: int | None = None,
max_repair_attempts: int | None = None,
```

Keep all defaults `None` so existing callers are unchanged. Before stdin
serialization, use a copied task payload and set:

```python
task_dict["max_model_rounds"] = min(
    value for value in (task.max_model_rounds, self.max_model_rounds) if value is not None
)
task_dict["max_repair_attempts"] = self.max_repair_attempts
payload["provider"] = self.agent_provider or str(self.provider)
payload["model"] = self.agent_model or self.model
```

Only set optional task fields when a measured value exists.

- [ ] **Step 4: Preserve returned agent identity**

In `_trace_from_payload`, append one normalized event:

```python
{
    "event_type": "eval_agent_identity",
    "provider": payload.get("forge_agent_provider"),
    "model": payload.get("forge_agent_model"),
    "transport": self.agent_transport,
    "harness_fingerprint": self.harness_fingerprint,
    "identity_matches_profile": (
        payload.get("forge_agent_provider") == self.agent_provider
        and payload.get("forge_agent_model") == self.agent_model
    ),
}
```

Do not expose API base URLs, environment values, credential references, or the
API key.

- [ ] **Step 5: Run runner and Rust identity contracts**

```bash
cd apps/eval-runner
uv run pytest tests/test_runner.py -k "profiled_agent_identity or forge_agent" -q
cd ../desktop
cargo test --manifest-path src-tauri/Cargo.toml eval_headless --lib
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/eval-runner/app/runner.py apps/eval-runner/tests/test_runner.py
git commit -m "feat(eval): bind Forge runs to measured identity"
```

---

### Task 3: Normalize provider usage and compute exact CNY spend

**Files:**

- Create: `apps/eval-runner/app/provider_usage.py`
- Create: `apps/eval-runner/tests/test_provider_usage.py`

- [ ] **Step 1: Write failing accounting tests**

Cover:

```python
def test_prices_cache_hit_miss_and_output_in_cny() -> None:
    accounting = account_provider_usage(
        provider_events=[
            event(input_tokens=1_000_000, cache_read_tokens=250_000, output_tokens=100_000)
        ],
        profile=PROFILE,
    )
    assert accounting.cache_hit_input_tokens == 250_000
    assert accounting.cache_miss_input_tokens == 750_000
    assert accounting.output_tokens == 100_000
    assert accounting.cost_cny == Decimal("0.955")
    assert accounting.status == "known"
```

Also require:

- multiple provider events are summed;
- adjacent legacy `usage` events are ignored to avoid double counting;
- missing events, `provider_omitted`, `pricing_unknown`, null token fields,
  negative/impossible cache counts, wrong provider/model, and mixed identities
  return `status="unknown"` with stable blockers;
- exact cap equality passes and a value above the cap sets
  `per_task_cap_exceeded=True`;
- no float participates in the CNY calculation.

- [ ] **Step 2: Confirm tests fail**

```bash
cd apps/eval-runner
uv run pytest tests/test_provider_usage.py -q
```

Expected: FAIL because `app.provider_usage` does not exist.

- [ ] **Step 3: Implement strict accounting**

Expose:

```python
class ProviderUsageAccounting(EvalModel):
    status: Literal["known", "unknown"]
    provider_event_count: int = Field(ge=0)
    input_tokens: int | None = Field(default=None, ge=0)
    cache_hit_input_tokens: int | None = Field(default=None, ge=0)
    cache_miss_input_tokens: int | None = Field(default=None, ge=0)
    output_tokens: int | None = Field(default=None, ge=0)
    reasoning_tokens: int | None = Field(default=None, ge=0)
    cost_cny: Decimal | None = Field(default=None, ge=0, allow_inf_nan=False)
    per_task_cap_exceeded: bool = False
    blockers: list[str] = Field(default_factory=list)


def provider_events_from_trace(trace: AgentTrace) -> list[dict[str, Any]]: ...
def account_provider_usage(
    provider_events: list[dict[str, Any]],
    profile: HarnessProfileV1,
) -> ProviderUsageAccounting: ...
```

Filter only canonical events with a non-null `provider_id`; never count
`source="legacy_usage"`. Calculate:

```python
cache_miss = input_tokens - cache_read_tokens
cost = (
    Decimal(cache_read_tokens) * hit_price
    + Decimal(cache_miss) * miss_price
    + Decimal(output_tokens) * output_price
) / Decimal(1_000_000)
```

Preserve the exact Decimal in artifacts as a JSON string.

- [ ] **Step 4: Run accounting tests and quality checks**

```bash
cd apps/eval-runner
uv run pytest tests/test_provider_usage.py -q
uv run ruff check app/provider_usage.py tests/test_provider_usage.py
uv run mypy app/provider_usage.py
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/eval-runner/app/provider_usage.py apps/eval-runner/tests/test_provider_usage.py
git commit -m "feat(eval): account DeepSeek spend in CNY"
```

---

### Task 4: Build the resumable, fail-closed baseline campaign

**Files:**

- Create: `apps/eval-runner/app/baseline_models.py`
- Create: `apps/eval-runner/app/baseline.py`
- Create: `apps/eval-runner/tests/test_baseline.py`

- [ ] **Step 1: Write failing campaign-model tests**

Define tests for:

```python
class BaselinePendingAttempt(EvalModel):
    schema_version: Literal[1] = 1
    attempt_id: str
    task_id: str
    repetition: int = Field(ge=1, le=3)
    state: Literal["provider_call_pending"] = "provider_call_pending"
    source_commit: GitSha1
    dirty_state_sha256: Sha256
    dataset_fingerprint: Sha256
    harness_fingerprint: Sha256
    reserved_cost_cny: Decimal = Field(gt=0, allow_inf_nan=False)
    started_at: datetime


class BaselineAttemptArtifact(EvalModel):
    schema_version: Literal[1] = 1
    attempt_id: str
    task_id: str
    repetition: int = Field(ge=1, le=3)
    profile: HarnessProfileV1
    source_commit: GitSha1
    dirty_state_sha256: Sha256
    dataset_fingerprint: Sha256
    trace_sha256: Sha256
    trace: AgentTrace
    usage: ProviderUsageAccounting
    trust_result: TrustGateResult
    primary_failure_category: str
    terminal: bool


class BaselineBudgetEntry(EvalModel):
    schema_version: Literal[1] = 1
    sequence: int = Field(ge=1)
    previous_entry_sha256: Sha256 | None
    attempt_id: str
    status: Literal["known", "reserved_unknown"]
    cost_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    cumulative_cost_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    artifact_sha256: Sha256
```

Reject blank IDs, extra fields, secret material, invalid sequence/hash links,
duplicate attempt/task-repetition pairs, decreasing cumulative cost, and an
artifact whose profile/fingerprint/trace digest disagrees.

- [ ] **Step 2: Write failing transaction tests**

Cover all boundaries:

- initialize only from the locked development dataset and the exact 10 task IDs;
- write/fsync pending record before invoking the fake provider;
- never invoke when `spent + max_cost_cny_per_task > campaign_budget_cny`;
- atomically publish the attempt artifact, append/fsync one budget entry, then
  remove the pending record;
- resume skips a completed verified task/repetition without another call;
- a pending record without a completed artifact blocks automatic resume as
  `interrupted_paid_attempt_unknown_cost`;
- recovery converts that pending attempt into a terminal reserved-unknown
  artifact and charges the full per-task reservation;
- unknown provider usage, identity mismatch, untrusted workspace evidence,
  patch replay failure, or per-task cap violation stops the batch before the
  next provider call;
- tampering with profile, dataset lock, task, pending record, trace, artifact,
  ledger, source commit, dirty digest, or cost invalidates the campaign;
- no holdout case path or ID is accepted.

- [ ] **Step 3: Confirm campaign tests fail**

```bash
cd apps/eval-runner
uv run pytest tests/test_baseline.py -q
```

Expected: FAIL because the baseline modules do not exist.

- [ ] **Step 4: Implement initialization and source identity**

`initialize_baseline_campaign(...)` must write `campaign.json` containing:

```python
{
    "schema_version": 1,
    "campaign_id": campaign_id,
    "source_commit": git("rev-parse", "HEAD"),
    "dirty_state_sha256": sha256_bytes(git_status_and_diff_bytes()),
    "dataset_fingerprint": DEVELOPMENT_FINGERPRINT,
    "dataset_lock_sha256": sha256_file(DATASET_LOCK),
    "harness_fingerprint": harness_profile_fingerprint(profile),
    "profile": profile.model_dump(mode="json"),
    "task_ids": sorted(development_task_ids),
    "repetitions": 3,
    "budget_cny": "8",
}
```

Write with a temporary file, `fsync`, `os.replace`, and directory `fsync`.
Artifacts contain repository-relative logical paths only; they do not store
home directories or environment variables.

- [ ] **Step 5: Implement the paid-attempt transaction**

Use deterministic IDs:

```python
attempt_id = f"{campaign_id}-r{repetition:02d}-{task.id}"
```

The order is repetition 1 task 1..10, repetition 2 task 1..10, repetition 3
task 1..10. Before `runner.run_task`, atomically write the pending record.
After the trace returns:

1. calculate usage from canonical provider events;
2. evaluate existing trust gates against the frozen development fingerprint;
3. build the strict artifact and hash its canonical bytes;
4. publish the artifact;
5. append/fsync the hash-chained budget entry;
6. remove/fsync the pending record directory.

At every entry point, fully verify existing campaign state before mutation.

- [ ] **Step 6: Implement recovery and verification**

`recover_baseline_campaign` may only:

- finish ledger publication for an already complete, valid attempt artifact; or
- turn a dangling pending record into a terminal
  `interrupted_paid_attempt_unknown_cost` artifact charged at the reserved
  per-task cap.

It never reissues the provider call. `verify_baseline_campaign` recomputes all
profile/dataset/source/artifact/trace/ledger hashes and returns stable blockers.

- [ ] **Step 7: Run campaign tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_baseline.py -q
uv run ruff check app/baseline.py app/baseline_models.py tests/test_baseline.py
uv run mypy app/baseline.py app/baseline_models.py
```

Expected: all PASS.

- [ ] **Step 8: Commit**

```bash
git add apps/eval-runner/app/baseline.py \
  apps/eval-runner/app/baseline_models.py \
  apps/eval-runner/tests/test_baseline.py
git commit -m "feat(eval): run resumable paid baselines"
```

---

### Task 5: Generate the trusted baseline report from verified artifacts

**Files:**

- Modify: `apps/eval-runner/app/baseline.py`
- Modify: `apps/eval-runner/app/baseline_models.py`
- Modify: `apps/eval-runner/tests/test_baseline.py`

- [ ] **Step 1: Write failing report tests**

Require a strict report with:

```python
class BaselineReport(EvalModel):
    schema_version: Literal[1] = 1
    campaign_id: str
    status: Literal["complete", "incomplete", "blocked"]
    source_commit: GitSha1
    dirty_state_sha256: Sha256
    dataset_fingerprint: Sha256
    harness_fingerprint: Sha256
    provider: Literal["deepseek"]
    model: Literal["deepseek-v4-flash"]
    transport: Literal["anthropic_messages"]
    expected_attempts: Literal[30]
    completed_attempts: int = Field(ge=0, le=30)
    trusted_attempts: int = Field(ge=0, le=30)
    trusted_success_rate: float = Field(ge=0, le=1)
    verification_pass_rate: float = Field(ge=0, le=1)
    scope_violation_rate: float = Field(ge=0, le=1)
    median_model_rounds: float = Field(ge=0)
    p95_model_rounds: float = Field(ge=0)
    median_cost_cny: Decimal | None
    p95_cost_cny: Decimal | None
    total_cost_cny: Decimal = Field(ge=0)
    unknown_cost_attempts: int = Field(ge=0)
    failure_categories: dict[str, int]
    per_task: dict[str, dict[str, int | float | bool]]
    trust_blockers: dict[str, int]
    attempt_artifact_index_sha256: Sha256
```

Test deterministic median/p95, flaky per-task results, primary failure
distribution, trust-blocker counts, incomplete status before 30 attempts,
blocked status for unknown cost/identity/trust, and complete status only when
all 30 artifacts and their ledger entries verify.

- [ ] **Step 2: Confirm report tests fail**

```bash
cd apps/eval-runner
uv run pytest tests/test_baseline.py -k "report" -q
```

Expected: FAIL because report generation is absent.

- [ ] **Step 3: Implement deterministic aggregation**

Sort attempts by `(repetition, task_id)`. Treat only
`trust_result.status == "trusted"` plus trace success as trusted success.
Unknown/untrusted attempts stay in denominators. Use nearest-rank p95 and
`statistics.median`; never coerce Decimal cost to float before percentile
selection. Write `artifact-index.json` with ordered path/SHA pairs and bind its
digest into `baseline-report.json`.

- [ ] **Step 4: Run report tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_baseline.py -q
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/eval-runner/app/baseline.py \
  apps/eval-runner/app/baseline_models.py \
  apps/eval-runner/tests/test_baseline.py
git commit -m "feat(eval): report trusted DeepSeek baseline"
```

---

### Task 6: Add the safe baseline CLI and offline acceptance gate

**Files:**

- Create: `apps/eval-runner/app/baseline_cli.py`
- Create: `apps/eval-runner/tests/test_baseline_cli.py`
- Modify: `apps/eval-runner/app/cli.py`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`
- Create: `docs/forgebench-h1-baseline.md`
- Modify: `README.md`
- Modify: `apps/eval-runner/README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Write failing CLI tests**

Exercise:

```text
python -m app.cli baseline init
python -m app.cli baseline run --max-attempts 1
python -m app.cli baseline status
python -m app.cli baseline recover
python -m app.cli baseline report
python -m app.cli baseline verify
```

All commands require explicit `--campaign` and `--profile`; mutation commands
never select “latest”. `run` defaults to the canonical development directory
and rejects any case not marked development. `status`, `report`, and `verify`
must not change Git HEAD, ledger bytes, or attempt bytes. CLI errors are stable
codes and must not echo prompts, absolute home paths, environment values, API
keys, or raw provider responses.

The fake Forge command emits canonical provider-usage events so the full CLI
test performs init → one attempt → report → verify without network access.

- [ ] **Step 2: Write failing acceptance assertions**

Require the gate:

```text
DeepSeek H1 paid-baseline contract
```

with command:

```bash
cd apps/eval-runner &&
uv run pytest tests/test_harness_profiles.py tests/test_provider_usage.py \
  tests/test_baseline.py tests/test_baseline_cli.py
```

Also require docs to contain `0.02 CNY`, `interrupted_paid_attempt_unknown_cost`,
`development only`, `baseline verify`, and `no holdout`.

- [ ] **Step 3: Confirm the red state**

```bash
cd apps/eval-runner
uv run pytest tests/test_baseline_cli.py -q
cd ../..
node --test scripts/acceptance.test.mjs
```

Expected: FAIL for the missing CLI/gate/docs.

- [ ] **Step 4: Implement isolated CLI dispatch**

Add this before generic parser construction:

```python
if raw_argv[:1] == ["baseline"]:
    return baseline_main(raw_argv[1:])
```

`run` takes `--max-attempts` for the one-call smoke. It reads the configured
Forge command from settings but never serializes the command environment into
artifacts.

- [ ] **Step 5: Add docs and acceptance gate**

Document the exact official price source, the 8 CNY H1 allocation, 1 CNY
per-task reservation, 100 CNY global ceiling, development-only rule, pending
record semantics, unknown-cost stop, commands, and the fact that H1 is a pinned
baseline rather than a frontier claim.

- [ ] **Step 6: Run offline H1 verification**

```bash
npm run test:eval
cd apps/eval-runner
uv run ruff check app tests
uv run mypy app
cd ../..
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
npm run build:desktop
```

Expected: all PASS.

- [ ] **Step 7: Run GitNexus change detection and commit**

```bash
git add apps/eval-runner/app/baseline_cli.py \
  apps/eval-runner/app/cli.py \
  apps/eval-runner/tests/test_baseline_cli.py \
  scripts/acceptance.sh scripts/acceptance.test.mjs \
  docs/forgebench-h1-baseline.md README.md \
  apps/eval-runner/README.md CHANGELOG.md
git commit -m "feat(eval): expose budgeted H1 baseline campaign"
```

---

### Task 7: Execute the paid smoke and 10 × 3 development baseline

**Files:**

- Generate locally: `apps/eval-runner/artifacts/h1-baseline/<campaign-id>/`
- Add after successful verification: `apps/eval-runner/forgebench/results/h1-baseline-report.json`
- Add after successful verification: `apps/eval-runner/forgebench/results/h1-baseline-artifact-index.json`
- Modify: `docs/forgebench-h1-baseline.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Verify a clean source state and build the headless binary**

Run from a clean checkout of the exact branch commit:

```bash
npm run test:eval
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --bin forge_eval_agent
```

Expected: PASS. Record commit and dirty-state digest during campaign init.

- [ ] **Step 2: Initialize the baseline campaign**

```bash
cd apps/eval-runner
uv run python -m app.cli baseline init \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --campaign-id h1-anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json
```

Expected: JSON reports 10 development tasks, 30 expected attempts, 8 CNY
campaign budget, 0 CNY spent, and the frozen profile/dataset fingerprints.

- [ ] **Step 3: Run exactly one paid smoke**

```bash
uv run python -m app.cli baseline run \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json \
  --max-attempts 1
```

Immediately stop if the result reports unknown usage, provider/model mismatch,
untrusted workspace evidence, patch replay failure, harness contract error, or
cost above 1 CNY. Do not retry the same attempt automatically.

- [ ] **Step 4: Verify the smoke before further spend**

```bash
uv run python -m app.cli baseline verify \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json
uv run python -m app.cli baseline status \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json
```

Expected: one terminal attempt, known provider usage, known CNY cost, intact
hash chain, and no trust blocker.

- [ ] **Step 5: Resume the remaining development attempts**

```bash
uv run python -m app.cli baseline run \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json
```

The controller stops before attempt 31, before exceeding the 8 CNY allocation,
and immediately after any unknown-cost/trust/identity blocker.

- [ ] **Step 6: Generate and independently verify the baseline report**

```bash
uv run python -m app.cli baseline report \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json
uv run python -m app.cli baseline verify \
  --campaign artifacts/h1-baseline/anthropic-control-v1 \
  --profile forgebench/profiles/deepseek-v4-flash-anthropic-baseline-v1.json
```

Expected: 30 attributable attempts or an explicit blocked/incomplete report;
the command never converts missing attempts or unknown usage into success.

- [ ] **Step 7: Publish the compact evidence**

Copy only the verified compact report and artifact index into
`forgebench/results/`. The index retains SHA-256 references to every local raw
attempt artifact; it contains no secrets, prompts beyond frozen case IDs, home
paths, or environment values. Update docs with actual success, scope, rounds,
cost, and failure distribution. State clearly that holdout remains untouched.

- [ ] **Step 8: Run final H1 gates and commit results**

```bash
cd ../../..
npm run test:eval
scripts/acceptance.sh --only "DeepSeek H1 paid-baseline contract"
git add apps/eval-runner/forgebench/results/h1-baseline-report.json \
  apps/eval-runner/forgebench/results/h1-baseline-artifact-index.json \
  docs/forgebench-h1-baseline.md CHANGELOG.md
git commit -m "test(eval): publish DeepSeek H1 paid baseline"
```

Expected: H1 report is trusted and fully attributable, or explicitly blocked.
Only a complete trusted 30-attempt report satisfies the H1 exit criterion.

---

## Plan self-review

- Spec coverage: H1 profile pinning, three development repetitions, official
  pricing, per-task and cumulative budget controls, paid-smoke stop rules,
  provider/model/transport identity, usage trust, failure distribution,
  resumability, and no-holdout enforcement all map to explicit tasks.
- Safety: `AgentTrace` remains untouched; actual secrets and environment
  values never enter artifacts; a crash cannot silently replay a paid call.
- Type consistency: `HarnessProfileV1`, `ProviderUsageAccounting`,
  `BaselinePendingAttempt`, `BaselineAttemptArtifact`,
  `BaselineBudgetEntry`, and `BaselineReport` retain the same names and fields
  across creation, CLI, verification, and reporting tasks.
- Placeholder scan: the plan contains no deferred implementation step or
  unspecified error-handling instruction.
