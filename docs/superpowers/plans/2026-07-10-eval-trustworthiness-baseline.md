# Forge Eval Trustworthiness Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Forge Eval Runner preserve the requested execution identity, derive scope and process evidence independently, evaluate trust on every CLI/API/worker path, and reject unauthenticated or stale-worker publication before its output can be used as public-beta release evidence.

**Architecture:** Keep `apps/eval-runner` independently runnable. Persist execution identity and worker ownership as explicit fail-closed contracts; move bounded subprocess control and filesystem observation into focused modules; run CLI, synchronous API, and queued worker execution through one `execute_evaluation()` orchestration function; and preserve execution status separately from a three-state trust result. SQLite remains the metadata authority, attempt-scoped filesystem artifacts prevent stale writers from overwriting canonical evidence, and the API/worker Docker services share only the database, artifacts, cases, and authentication configuration they need.

**Tech Stack:** Python 3.11, FastAPI, Pydantic v2, SQLite, pytest, mypy, Ruff, uv, Docker Compose, and the existing monorepo acceptance/release-confidence scripts.

---

## Scope And Invariants

This plan implements only subproject B, **Eval Trustworthiness Baseline**, from `docs/superpowers/specs/2026-07-10-public-beta-convergence-design.md`.

The implementation must preserve these invariants:

- `RunStatus` describes execution only. A completed run may still have `TrustStatus.UNKNOWN` or `TrustStatus.UNTRUSTED`.
- `EvaluationRun.provider`, `model`, and `case_source` are persisted authority. A worker never infers them from defaults or trace payloads.
- Only `mock` and `forge` are valid eval adapters. An unknown provider is a validation error, never a mock alias.
- `AgentTrace.changed_files` is populated from an independent pre/post workspace snapshot for Forge runs. Runner-reported paths are retained only as corroborating evidence.
- Missing workspace, sandbox, patch-replay, case-quality, harness, calibration, red-team, or required-score evidence can never yield `TrustStatus.TRUSTED`.
- Every subprocess has a deadline and a cancellation boundary, and timeout/cancellation terminates the whole process group.
- Every queued-worker mutation is fenced by `worker_id` plus a per-claim `lease_token`; a reclaimed worker cannot heartbeat, save traces, publish artifacts, retry, fail, or complete the new attempt.
- A non-loopback API configuration cannot start without an API token. Protected routes require `Authorization: Bearer $FORGE_EVAL_API_TOKEN`.
- CI exit code `0` means execution, trust, required score coverage, and configured thresholds all passed. Exploratory success-on-untrusted behavior requires explicit `--report-only`.
- Docker service mode runs separate API and worker services with shared SQLite and artifact volumes.

## Current Baseline And Code-Intelligence Caveat

At planning time, current HEAD is `7aea1f5e5e04b278941731b2a75d41634be9b7ae`. GitNexus is indexed at `435216340db3289248cea2564dffe05c490cdade`, 74 commits behind HEAD. The index refresh was not run during planning because this was a read-only mapping assignment.

Before each implementation task, run the listed GitNexus upstream impacts. If the index is still stale, unavailable, or times out, record the required fallback report before editing: command attempted, error or timeout, index freshness, symbols searched, files inspected, current-source direct callers, selected tests, affected authority domains, and residual risk. Warn the user before proceeding when a result is HIGH or CRITICAL.

Before every task commit, run:

```text
detect_changes({scope: "compare", base_ref: "main", repo: "forge"})
```

Expected: only the task's declared symbols, tests, documentation, and execution flows are affected. Stop and investigate any unrelated flow.

## File Responsibility Map

### New files

- `apps/eval-runner/app/process_control.py`: one bounded, cancellable process-group runner shared by Forge execution, setup/validation commands, sandbox Git operations, and patch replay.
- `apps/eval-runner/app/workspace_observer.py`: deterministic file-state snapshots and added/modified/deleted comparison independent of agent output.
- `apps/eval-runner/app/execution.py`: common evaluation orchestration and trust-input aggregation used by CLI, synchronous API, and worker paths.
- `apps/eval-runner/tests/test_process_control.py`: timeout, cancellation, partial-output, and descendant-process termination tests.
- `apps/eval-runner/tests/test_workspace_observer.py`: filesystem observation coverage for regular, binary, symlink, added, modified, and deleted files.
- `apps/eval-runner/tests/test_execution.py`: common orchestration and fail-closed trust-result tests.
- `apps/eval-runner/tests/test_docker_contract.py`: static and rendered Compose contract tests for API/worker/shared storage/authentication.

### Existing files to modify

- `apps/eval-runner/app/models.py`: provider, trust, workspace-observation, score-coverage, process outcome, and fencing contracts.
- `apps/eval-runner/app/config.py`: bind/auth, command deadline, lease duration, and polling configuration.
- `apps/eval-runner/app/storage.py`: SQLite migrations, execution-identity round-trip, trust persistence, fenced worker writes, attempt artifacts, WAL, and busy timeout.
- `apps/eval-runner/app/runner.py`: strict factory, independent workspace evidence, bounded commands, scrub checks, patch replay, and cancellation propagation.
- `apps/eval-runner/app/sandbox.py`: call the bounded process helper and return unavailable/failed evidence rather than hanging.
- `apps/eval-runner/app/patches.py`: bounded patch replay with explicit timeout/cancellation outcomes.
- `apps/eval-runner/app/harness_checks.py`: return structured golden-harness evidence rather than a bare boolean.
- `apps/eval-runner/app/trust_gates.py`: evaluate explicit present/pass/fail evidence and score coverage into a three-state trust result.
- `apps/eval-runner/app/reporting.py`: publish score numerator, denominator, and coverage without removing the existing numeric `score_summary` compatibility field.
- `apps/eval-runner/app/cli.py`: invoke common execution, expose trust profile/report-only controls, and implement stable CI exit semantics.
- `apps/eval-runner/app/main.py`: inject settings, enforce bearer authentication, validate non-loopback configuration, and invoke common sync execution.
- `apps/eval-runner/app/worker.py`: use persisted identity, cancellation-aware common execution, heartbeats with fencing, and stale-publication rejection.
- `apps/eval-runner/pyproject.toml` and `apps/eval-runner/uv.lock`: add mypy and its checked configuration so the exact Eval quality commands are stable.
- `apps/eval-runner/Dockerfile` and `apps/eval-runner/docker-compose.yml`: build and run authenticated API plus fenced worker with shared local volumes.
- `apps/eval-runner/tests/test_storage.py`, `test_runner.py`, `test_reporting.py`, `test_cli.py`, `test_api.py`, `test_worker.py`, and `test_smoke.py`: behavior-level regression coverage beside each changed path.
- `apps/eval-runner/README.md`, `apps/eval-runner/docs/ops.md`, `apps/eval-runner/docs/architecture.md`, root `README.md`, and `CHANGELOG.md`: user/operator contract.
- `scripts/acceptance.sh` and `scripts/acceptance.test.mjs`: advertise and validate the four fixed public-beta Eval gate labels.

## Stable Naming Contract

Use these names exactly throughout the plan:

```python
EvalProvider
TrustStatus
TrustGateResult
WorkspaceObservation
ScoreCoverage
ProcessOutcome
BoundedProcessResult
LeaseLostError
ExecutionOptions
EvaluationExecution
execute_evaluation
snapshot_workspace
observe_workspace_changes
run_bounded_process
validate_execution_identity
```

The fixed release-required acceptance labels are:

```text
eval execution identity baseline
eval independent workspace evidence baseline
eval trusted execution baseline
eval authenticated fenced worker baseline
```

---

### Task 1: Add Strict Provider, Trust, Observation, Coverage, And Lease Models

**Files:**

- Modify: `apps/eval-runner/app/models.py:1-330`
- Test: `apps/eval-runner/tests/test_runner.py`
- Test: `apps/eval-runner/tests/test_reporting.py`
- Test: `apps/eval-runner/tests/test_storage.py`

**GitNexus impact targets:** `EvaluationTask`, `AgentTrace`, `EvalScore`, `BacktestReport`, `RunCreateRequest`, `EvaluationRun`, `TrustGateResult`.

- [ ] **Step 1: Run upstream impact analysis and report risk**

```text
impact({target: "EvaluationTask", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "AgentTrace", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "BacktestReport", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "EvaluationRun", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "TrustGateResult", direction: "upstream", repo: "forge", includeTests: true})
```

Expected: central model fan-out across runner, storage, CLI, API, worker, reporting, and tests. Warn before editing if risk is HIGH or CRITICAL.

- [ ] **Step 2: RED — write contract tests for the new model invariants**

Add tests with these exact names:

```python
def test_run_create_request_rejects_unknown_provider() -> None:
    with pytest.raises(ValidationError):
        RunCreateRequest(provider="unknown-provider", model="model-a")


def test_trust_result_rejects_inconsistent_status_and_boolean() -> None:
    with pytest.raises(ValidationError):
        TrustGateResult(status=TrustStatus.TRUSTED, trusted=False)


def test_score_coverage_requires_consistent_counts() -> None:
    with pytest.raises(ValidationError):
        ScoreCoverage(mean=1.0, observed=2, expected=1, coverage=2.0)


def test_evaluation_run_can_represent_legacy_missing_execution_identity() -> None:
    run = make_run("legacy").model_copy(
        update={"provider": None, "model": None, "case_source": None}
    )
    assert run.provider is None
    assert run.trust_result.status == TrustStatus.UNKNOWN
```

Run:

```bash
cd apps/eval-runner
uv run pytest \
  tests/test_runner.py::test_run_create_request_rejects_unknown_provider \
  tests/test_reporting.py::test_trust_result_rejects_inconsistent_status_and_boolean \
  tests/test_reporting.py::test_score_coverage_requires_consistent_counts \
  tests/test_storage.py::test_evaluation_run_can_represent_legacy_missing_execution_identity -q
```

Expected RED: collection fails because `EvalProvider`, `TrustStatus`, `WorkspaceObservation`, and `ScoreCoverage` do not exist and `EvaluationRun` has no `trust_result` or `lease_token`.

- [ ] **Step 3: GREEN — add the exact model contracts**

In `app/models.py`, import `model_validator` and add:

```python
class EvalProvider(StrEnum):
    MOCK = "mock"
    FORGE = "forge"


class TrustStatus(StrEnum):
    UNKNOWN = "unknown"
    TRUSTED = "trusted"
    UNTRUSTED = "untrusted"


class ProcessOutcome(StrEnum):
    COMPLETED = "completed"
    TIMED_OUT = "timed_out"
    CANCELLED = "cancelled"


class WorkspaceObservation(EvalModel):
    available: bool
    source: str
    changed_files: list[str] = Field(default_factory=list)
    added_files: list[str] = Field(default_factory=list)
    modified_files: list[str] = Field(default_factory=list)
    deleted_files: list[str] = Field(default_factory=list)
    reported_changed_files: list[str] = Field(default_factory=list)
    mismatch_files: list[str] = Field(default_factory=list)
    error: str | None = None


class ScoreCoverage(EvalModel):
    mean: float | None = Field(default=None, ge=0.0, le=1.0)
    observed: int = Field(ge=0)
    expected: int = Field(ge=0)
    coverage: float = Field(ge=0.0, le=1.0)

    @model_validator(mode="after")
    def validate_counts(self) -> "ScoreCoverage":
        if self.observed > self.expected:
            raise ValueError("observed score count cannot exceed expected score count")
        expected_coverage = self.observed / self.expected if self.expected else 1.0
        if abs(self.coverage - expected_coverage) > 1e-12:
            raise ValueError("score coverage does not match observed/expected counts")
        if self.observed == 0 and self.mean is not None:
            raise ValueError("mean must be null when no score was observed")
        return self


class TrustGateResult(EvalModel):
    status: TrustStatus = TrustStatus.UNKNOWN
    trusted: bool = False
    blockers: list[str] = Field(default_factory=list)
    warnings: list[str] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_status(self) -> "TrustGateResult":
        if self.trusted != (self.status == TrustStatus.TRUSTED):
            raise ValueError("trusted must match trust status")
        if self.status == TrustStatus.TRUSTED and self.blockers:
            raise ValueError("trusted result cannot contain blockers")
        return self
```

Apply these exact field changes:

```python
class EvaluationTask(EvalModel):
    id: str
    title: str
    prompt: str
    fixture_path: str | None = None
    context_files: list[str] = Field(default_factory=list)
    setup_commands: list[str] = Field(default_factory=list)
    validation_commands: list[str] = Field(default_factory=list)
    post_validation_commands: list[str] = Field(default_factory=list)
    pass_to_pass_commands: list[str] = Field(default_factory=list)
    fail_to_pass_commands: list[str] = Field(default_factory=list)
    verification_command: str | None = None
    expected_success: bool = True
    expected_files_changed: list[str] = Field(default_factory=list)
    forbidden_files_changed: list[str] = Field(default_factory=list)
    required_scores: list[str] = Field(default_factory=list)
    max_duration_seconds: int | None = Field(default=None, ge=1)
    max_model_rounds: int | None = Field(default=None, ge=1)
    tags: list[str] = Field(default_factory=list)
    metadata: dict[str, Any] = Field(default_factory=dict)
```

Add these fields to `AgentTrace` without removing existing fields:

```python
provider: EvalProvider
required_scores: list[str] = Field(default_factory=list)
workspace_observation: WorkspaceObservation | None = None
sandbox_scrub: LeakageCheck | None = None
patch_replay: WorkspaceCheck | None = None
```

Keep `score_summary` and add these fields to `BacktestReport`:

```python
score_coverage: dict[str, ScoreCoverage] = Field(default_factory=dict)
trust_result: TrustGateResult = Field(default_factory=TrustGateResult)
```

Replace the request and persisted-run identity fields with:

```python
class RunCreateRequest(EvalModel):
    task_ids: list[str] | None = None
    provider: EvalProvider = EvalProvider.MOCK
    model: str = Field(default="deterministic-agent-v1", min_length=1)
    max_retries: int = Field(default=1, ge=0)


class EvaluationRun(EvalModel):
    run_id: str
    status: RunStatus
    provider: EvalProvider | None = None
    model: str | None = None
    case_source: str | None = None
    requested_task_ids: list[str]
    traces: list[AgentTrace] = Field(default_factory=list)
    metrics: MetricsSummary
    trust_result: TrustGateResult = Field(default_factory=TrustGateResult)
    started_at: datetime
    ended_at: datetime
    duration_ms: int = Field(ge=0)
    retry_count: int = Field(default=0, ge=0)
    max_retries: int = Field(default=0, ge=0)
    failure_reason: str | None = None
    failure_category: FailureCategory = FailureCategory.NONE
    worker_id: str | None = None
    lease_token: str | None = None
    claimed_at: datetime | None = None
    heartbeat_at: datetime | None = None
    lease_expires_at: datetime | None = None
```

Include `lease_token` only as an ordinary string field; token generation belongs to storage claims.

- [ ] **Step 4: Run the focused GREEN tests**

```bash
cd apps/eval-runner
uv run pytest \
  tests/test_runner.py::test_run_create_request_rejects_unknown_provider \
  tests/test_reporting.py::test_trust_result_rejects_inconsistent_status_and_boolean \
  tests/test_reporting.py::test_score_coverage_requires_consistent_counts \
  tests/test_storage.py::test_evaluation_run_can_represent_legacy_missing_execution_identity -q
```

Expected GREEN: four tests pass.

- [ ] **Step 5: REFACTOR — update imports and all existing constructors**

Use `EvalProvider.MOCK` or `EvalProvider.FORGE` in new production code while leaving JSON test fixtures as their serialized string values. Add `score_coverage={}` and the default unknown trust result only where an explicit constructor requires them. Do not change pass/fail semantics in this task.

Run:

```bash
cd apps/eval-runner
uv run ruff check app/models.py tests/test_runner.py tests/test_reporting.py tests/test_storage.py
uv run pytest tests/test_runner.py tests/test_reporting.py tests/test_storage.py -q
```

Expected: Ruff passes and the three test files pass.

- [ ] **Step 6: Verify change scope and commit**

```bash
git add apps/eval-runner/app/models.py \
  apps/eval-runner/tests/test_runner.py \
  apps/eval-runner/tests/test_reporting.py \
  apps/eval-runner/tests/test_storage.py
git commit -m "feat(eval): define trustworthy execution contracts"
```

---

### Task 2: Round-Trip Execution Identity And Reject Provider Downgrades

**Files:**

- Modify: `apps/eval-runner/app/storage.py:29-890`
- Modify: `apps/eval-runner/app/runner.py:473-480`
- Modify: `apps/eval-runner/app/main.py:62-114`
- Test: `apps/eval-runner/tests/test_storage.py`
- Test: `apps/eval-runner/tests/test_runner.py`
- Test: `apps/eval-runner/tests/test_api.py`
- Test: `apps/eval-runner/tests/test_smoke.py`

**GitNexus impact targets:** `SQLiteStorage._init_schema`, `SQLiteStorage.get_run`, `SQLiteStorage._upsert_run_connection`, `SQLiteStorage.claim_pending_run`, `create_runner`, nested API handler `create_run`.

- [ ] **Step 1: Run upstream and route impact analysis**

```text
impact({target: "_init_schema", file_path: "apps/eval-runner/app/storage.py", kind: "Method", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "get_run", file_path: "apps/eval-runner/app/storage.py", kind: "Method", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "_upsert_run_connection", file_path: "apps/eval-runner/app/storage.py", kind: "Method", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "claim_pending_run", file_path: "apps/eval-runner/app/storage.py", kind: "Method", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "create_runner", direction: "upstream", repo: "forge", includeTests: true})
api_impact({route: "/runs", repo: "forge"})
```

Expected: `create_runner` directly affects CLI, API, worker, and factory tests. Warn if any impact is HIGH or CRITICAL.

- [ ] **Step 2: RED — add identity round-trip, migration, and no-fallback tests**

Add exact tests:

```python
@pytest.mark.parametrize(("storage_name", "storage_factory"), storage_factories())
def test_storage_contract_round_trips_execution_identity(
    tmp_path: Path,
    storage_name: str,
    storage_factory: StorageFactory,
) -> None:
    tasks_path = tmp_path / f"{storage_name}-tasks.json"
    write_tasks(tasks_path)
    storage = storage_factory(
        tasks_path,
        tmp_path / f"{storage_name}.db",
        tmp_path / f"{storage_name}-artifacts",
    )
    run = make_run("identity-run").model_copy(
        update={
            "provider": EvalProvider.FORGE,
            "model": "deepseek-v4-flash",
            "case_source": "/cases/release",
        }
    )
    storage.create_run(run)
    fetched = storage.get_run(run.run_id)
    assert fetched is not None
    assert fetched.provider == EvalProvider.FORGE
    assert fetched.model == "deepseek-v4-flash"
    assert fetched.case_source == "/cases/release"


def test_sqlite_storage_migrates_legacy_execution_identity_columns(
    tmp_path: Path,
) -> None:
    tasks_path = tmp_path / "tasks.json"
    db_path = tmp_path / "legacy.db"
    write_tasks(tasks_path)
    with sqlite3.connect(db_path) as connection:
        connection.execute(
            "CREATE TABLE eval_runs (id TEXT PRIMARY KEY, status TEXT NOT NULL, "
            "requested_task_ids_json TEXT NOT NULL, metrics_json TEXT NOT NULL, "
            "started_at TEXT NOT NULL, ended_at TEXT NOT NULL, duration_ms INTEGER NOT NULL, "
            "created_at TEXT NOT NULL, updated_at TEXT NOT NULL)"
        )
    SQLiteStorage(
        tasks_path=tasks_path,
        db_path=db_path,
        artifacts_path=tmp_path / "artifacts",
    )
    with sqlite3.connect(db_path) as connection:
        columns = {row[1] for row in connection.execute("PRAGMA table_info(eval_runs)")}
    assert {"provider", "model", "case_source"} <= columns


def test_runner_factory_rejects_unknown_provider() -> None:
    with pytest.raises(ValueError, match="Unsupported eval provider: unknown-provider"):
        create_runner(provider="unknown-provider", model="model-a")
```

Add API assertion:

```python
def test_api_rejects_unknown_provider(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    client = TestClient(create_app(storage=InMemoryStorage(tasks_path=tasks_path)))
    response = client.post(
        "/runs",
        json={"task_ids": ["task-pass"], "provider": "unknown-provider", "model": "x"},
    )
    assert response.status_code == 422
```

Run:

```bash
cd apps/eval-runner
uv run pytest \
  tests/test_storage.py::test_storage_contract_round_trips_execution_identity \
  tests/test_storage.py::test_sqlite_storage_migrates_legacy_execution_identity_columns \
  tests/test_runner.py::test_runner_factory_rejects_unknown_provider \
  tests/test_api.py::test_api_rejects_unknown_provider -q
```

Expected RED: SQLite reload returns mock defaults, legacy schema lacks provider/model migration, and `create_runner` returns a mock runner for the unknown value.

- [ ] **Step 3: GREEN — migrate and restore all execution identity fields**

In `SQLiteStorage._init_schema`, keep fresh-table columns and add:

```python
ensure_column(connection, "eval_runs", "provider", "TEXT")
ensure_column(connection, "eval_runs", "model", "TEXT")
ensure_column(connection, "eval_runs", "case_source", "TEXT")
ensure_column(connection, "eval_runs", "trust_result_json", "TEXT")
ensure_column(connection, "eval_runs", "lease_token", "TEXT")
```

In `SQLiteStorage.get_run`, restore identity and trust explicitly:

```python
provider_value = _row_val("provider")
trust_payload = _row_val("trust_result_json")
return EvaluationRun(
    run_id=row["id"],
    status=RunStatus(row["status"]),
    provider=EvalProvider(provider_value) if provider_value else None,
    model=_row_val("model"),
    case_source=_row_val("case_source"),
    requested_task_ids=json.loads(row["requested_task_ids_json"]),
    traces=traces,
    metrics=MetricsSummary.model_validate(json.loads(row["metrics_json"])),
    trust_result=(
        TrustGateResult.model_validate_json(trust_payload)
        if trust_payload
        else TrustGateResult()
    ),
    started_at=datetime.fromisoformat(row["started_at"]),
    ended_at=datetime.fromisoformat(row["ended_at"]),
    duration_ms=row["duration_ms"],
    retry_count=_row_val("retry_count", 0) or 0,
    max_retries=_row_val("max_retries", 0) or 0,
    failure_reason=_row_val("failure_reason"),
    failure_category=FailureCategory(_row_val("failure_category") or "none"),
    worker_id=_row_val("worker_id"),
    lease_token=_row_val("lease_token"),
    claimed_at=_parse_datetime(_row_val("claimed_at")),
    heartbeat_at=_parse_datetime(_row_val("heartbeat_at")),
    lease_expires_at=_parse_datetime(_row_val("lease_expires_at")),
)
```

Extend `_upsert_run_connection` INSERT, conflict update, and bound values with `trust_result_json` and `lease_token`. Bind `run.trust_result.model_dump_json()` and `run.lease_token`; continue binding `run.provider.value if run.provider is not None else None`, `run.model`, and `run.case_source` directly. Do not derive identity through `first_trace_attr`.

Add this helper to `app/runner.py`:

```python
def validate_execution_identity(
    provider: EvalProvider | None,
    model: str | None,
    case_source: str | None,
) -> tuple[EvalProvider, str, str]:
    if provider is None:
        raise ValueError("Persisted eval run is missing provider")
    if model is None or not model.strip():
        raise ValueError("Persisted eval run is missing model")
    if case_source is None or not case_source.strip():
        raise ValueError("Persisted eval run is missing case_source")
    return provider, model, case_source
```

Replace `create_runner` with explicit branches:

```python
def create_runner(
    provider: EvalProvider | str,
    model: str,
    forge_command: str | Sequence[str] | None = None,
) -> EvalRunner:
    try:
        normalized_provider = EvalProvider(provider)
    except ValueError as exc:
        raise ValueError(f"Unsupported eval provider: {provider}") from exc
    if normalized_provider == EvalProvider.MOCK:
        return DeterministicMockRunner(provider=normalized_provider, model=model)
    if normalized_provider == EvalProvider.FORGE:
        return ForgeAgentRunner(
            provider=normalized_provider,
            model=model,
            command=forge_command,
        )
    raise ValueError(f"Unsupported eval provider: {provider}")
```

- [ ] **Step 4: Prove queued SQLite Forge identity cannot downgrade**

Add `test_queued_sqlite_forge_run_preserves_execution_identity` to `tests/test_smoke.py`. Start the API in queued SQLite mode, enqueue `provider=forge`, run the worker without a Forge command, and assert:

```python
assert run_data["provider"] == "forge"
assert run_data["model"] == "local-forge"
assert run_data["case_source"] == str(tasks_path)
assert run_data["traces"][0]["provider"] == "forge"
assert run_data["traces"][0]["error"] == "forge_command_not_configured"
```

Run:

```bash
cd apps/eval-runner
uv run pytest \
  tests/test_storage.py::test_storage_contract_round_trips_execution_identity \
  tests/test_storage.py::test_sqlite_storage_migrates_legacy_execution_identity_columns \
  tests/test_runner.py::test_runner_factory_rejects_unknown_provider \
  tests/test_api.py::test_api_rejects_unknown_provider \
  tests/test_smoke.py::test_queued_sqlite_forge_run_preserves_execution_identity -q
```

Expected GREEN: all five tests pass and the queued Forge run fails explicitly as Forge.

- [ ] **Step 5: REFACTOR — remove fallback identity derivation**

Delete `first_trace_attr` after confirming no caller remains. Search:

```bash
cd apps/eval-runner
rg -n "first_trace_attr|provider or|model or" app tests
```

Expected: no fallback identity logic remains. Then run:

```bash
uv run ruff check app/storage.py app/runner.py tests/test_storage.py tests/test_runner.py tests/test_api.py tests/test_smoke.py
uv run pytest tests/test_storage.py tests/test_runner.py tests/test_api.py -q
```

Expected: Ruff and focused tests pass.

- [ ] **Step 6: Verify change scope and commit**

```bash
git add apps/eval-runner/app/models.py \
  apps/eval-runner/app/storage.py \
  apps/eval-runner/app/runner.py \
  apps/eval-runner/app/main.py \
  apps/eval-runner/tests/test_storage.py \
  apps/eval-runner/tests/test_runner.py \
  apps/eval-runner/tests/test_api.py \
  apps/eval-runner/tests/test_smoke.py
git commit -m "fix(eval): preserve requested execution identity"
```

---

### Task 3: Add One Bounded And Cancellable Process-Group Runner

**Files:**

- Create: `apps/eval-runner/app/process_control.py`
- Create: `apps/eval-runner/tests/test_process_control.py`
- Modify: `apps/eval-runner/app/models.py`
- Modify: `apps/eval-runner/app/config.py`

**GitNexus impact targets:** `Settings`, `ShellOutput`, `ForgeAgentRunner.run_task`, `run_shell_commands`, `sandbox.run_git`, `replay_patch`.

- [ ] **Step 1: Run upstream impact analysis**

```text
impact({target: "Settings", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "run_shell_commands", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "run_git", file_path: "apps/eval-runner/app/sandbox.py", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "replay_patch", direction: "upstream", repo: "forge", includeTests: true})
```

Expected: process control affects runner, sandbox, patches, and their tests.

- [ ] **Step 2: RED — specify timeout, cancellation, and descendant cleanup**

Create tests named:

```python
def test_bounded_process_returns_completed_output(tmp_path: Path) -> None:
    result = run_bounded_process(
        [sys.executable, "-c", "print('ok')"],
        cwd=tmp_path,
        timeout_seconds=2.0,
    )
    assert result.outcome == ProcessOutcome.COMPLETED
    assert result.returncode == 0
    assert result.stdout == "ok\n"


def test_bounded_process_timeout_preserves_partial_output(tmp_path: Path) -> None:
    result = run_bounded_process(
        [sys.executable, "-u", "-c", "import time; print('started'); time.sleep(10)"],
        cwd=tmp_path,
        timeout_seconds=0.2,
    )
    assert result.outcome == ProcessOutcome.TIMED_OUT
    assert result.returncode == 124
    assert "started" in result.stdout


def test_bounded_process_cancellation_kills_descendant(tmp_path: Path) -> None:
    marker = tmp_path / "child-finished"
    cancelled = threading.Event()
    timer = threading.Timer(0.2, cancelled.set)
    timer.start()
    try:
        result = run_bounded_process(
            [
                sys.executable,
                "-c",
                (
                    "import subprocess,sys,time; "
                    "subprocess.Popen([sys.executable,'-c',"
                    f"\"import time,pathlib; time.sleep(2); pathlib.Path(r'{marker}').write_text('x')\"]); "
                    "time.sleep(10)"
                ),
            ],
            cwd=tmp_path,
            timeout_seconds=5.0,
            cancel_requested=cancelled.is_set,
        )
    finally:
        timer.cancel()
    assert result.outcome == ProcessOutcome.CANCELLED
    assert result.returncode == 130
    time.sleep(2.2)
    assert not marker.exists()
```

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_process_control.py -q
```

Expected RED: import fails because `app.process_control` does not exist.

- [ ] **Step 3: GREEN — implement bounded process execution**

Create `app/process_control.py` with these public contracts:

```python
from __future__ import annotations

import os
import signal
import subprocess
import time
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path

from app.models import ProcessOutcome


@dataclass(frozen=True)
class BoundedProcessResult:
    command: str
    stdout: str
    stderr: str
    returncode: int
    duration_ms: int
    outcome: ProcessOutcome


CancelRequested = Callable[[], bool]


def never_cancelled() -> bool:
    return False


def run_bounded_process(
    command: str | Sequence[str],
    *,
    cwd: Path,
    timeout_seconds: float,
    cancel_requested: CancelRequested = never_cancelled,
    input_text: str | None = None,
    shell: bool = False,
) -> BoundedProcessResult:
    started = time.monotonic()
    process = subprocess.Popen(
        command,
        cwd=cwd,
        shell=shell,
        text=True,
        stdin=subprocess.PIPE if input_text is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    deadline = started + timeout_seconds
    pending_input = input_text
    while True:
        if cancel_requested():
            stdout, stderr = _terminate_process_group(process)
            return _result(
                command,
                stdout,
                stderr,
                130,
                started,
                ProcessOutcome.CANCELLED,
            )
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            stdout, stderr = _terminate_process_group(process)
            return _result(
                command,
                stdout,
                stderr,
                124,
                started,
                ProcessOutcome.TIMED_OUT,
            )
        try:
            stdout, stderr = process.communicate(
                input=pending_input,
                timeout=min(0.05, remaining),
            )
            return _result(
                command,
                stdout,
                stderr,
                process.returncode,
                started,
                ProcessOutcome.COMPLETED,
            )
        except subprocess.TimeoutExpired:
            pending_input = None


def _terminate_process_group(
    process: subprocess.Popen[str],
) -> tuple[str, str]:
    if process.poll() is None:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    try:
        return process.communicate(timeout=0.5)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        return process.communicate()


def _result(
    command: str | Sequence[str],
    stdout: str,
    stderr: str,
    returncode: int,
    started: float,
    outcome: ProcessOutcome,
) -> BoundedProcessResult:
    label = command if isinstance(command, str) else " ".join(command)
    return BoundedProcessResult(
        command=label,
        stdout=stdout,
        stderr=stderr,
        returncode=returncode,
        duration_ms=max(0, int((time.monotonic() - started) * 1000)),
        outcome=outcome,
    )
```

Add settings:

```python
command_timeout_seconds: float = 900.0
setup_timeout_seconds: float = 300.0
validation_timeout_seconds: float = 300.0
lease_duration_seconds: float = 300.0
```

- [ ] **Step 4: Run GREEN tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_process_control.py -q
```

Expected GREEN: all three process tests pass, including descendant cleanup.

- [ ] **Step 5: REFACTOR — make termination portable without weakening macOS behavior**

Add a `PermissionError` fallback in `_terminate_process_group` that calls `process.terminate()` and later `process.kill()`. Keep process-group signals as the primary path. Run:

```bash
cd apps/eval-runner
uv run ruff check app/process_control.py app/config.py tests/test_process_control.py
uv run pytest tests/test_process_control.py -q
```

Expected: Ruff and all process-control tests pass.

- [ ] **Step 6: Verify change scope and commit**

```bash
git add apps/eval-runner/app/process_control.py \
  apps/eval-runner/app/config.py \
  apps/eval-runner/app/models.py \
  apps/eval-runner/tests/test_process_control.py
git commit -m "feat(eval): bound and cancel subprocess groups"
```

---

### Task 4: Observe Workspace Effects Independently

**Files:**

- Create: `apps/eval-runner/app/workspace_observer.py`
- Create: `apps/eval-runner/tests/test_workspace_observer.py`
- Modify: `apps/eval-runner/app/models.py`

**GitNexus impact targets:** `WorkspaceCheck`, `ForgeAgentRunner.run_task`, `prepare_workspace`, `scope_violations_for`.

- [ ] **Step 1: Run upstream impact analysis**

```text
impact({target: "WorkspaceCheck", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "run_task", target_uid: "Method:apps/eval-runner/app/runner.py:ForgeAgentRunner.run_task#1", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "prepare_workspace", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "scope_violations_for", direction: "upstream", repo: "forge", includeTests: true})
```

Expected: observer output will feed Forge trace construction and trust evaluation.

- [ ] **Step 2: RED — specify added, modified, deleted, binary, symlink, and unavailable evidence**

Create `tests/test_workspace_observer.py` with tests named:

```python
def test_workspace_observer_reports_added_modified_and_deleted_files(tmp_path: Path) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "modify.txt").write_text("before\n", encoding="utf-8")
    (workspace / "delete.txt").write_text("delete\n", encoding="utf-8")
    before = snapshot_workspace(workspace)
    (workspace / "modify.txt").write_text("after\n", encoding="utf-8")
    (workspace / "delete.txt").unlink()
    (workspace / "add.txt").write_text("add\n", encoding="utf-8")
    observation = observe_workspace_changes(before, workspace, reported_changed_files=[])
    assert observation.available is True
    assert observation.added_files == ["add.txt"]
    assert observation.modified_files == ["modify.txt"]
    assert observation.deleted_files == ["delete.txt"]
    assert observation.changed_files == ["add.txt", "delete.txt", "modify.txt"]


def test_workspace_observer_hashes_binary_and_symlink_targets(tmp_path: Path) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "binary.bin").write_bytes(b"\x00\x01")
    (workspace / "target-a").write_text("a", encoding="utf-8")
    (workspace / "target-b").write_text("b", encoding="utf-8")
    (workspace / "link").symlink_to("target-a")
    before = snapshot_workspace(workspace)
    (workspace / "binary.bin").write_bytes(b"\x00\x02")
    (workspace / "link").unlink()
    (workspace / "link").symlink_to("target-b")
    observation = observe_workspace_changes(before, workspace, reported_changed_files=[])
    assert observation.modified_files == ["binary.bin", "link"]


def test_workspace_observer_records_report_mismatch(tmp_path: Path) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    before = snapshot_workspace(workspace)
    (workspace / "actual.txt").write_text("x", encoding="utf-8")
    observation = observe_workspace_changes(
        before,
        workspace,
        reported_changed_files=["claimed.txt"],
    )
    assert observation.changed_files == ["actual.txt"]
    assert observation.reported_changed_files == ["claimed.txt"]
    assert observation.mismatch_files == ["actual.txt", "claimed.txt"]
```

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_workspace_observer.py -q
```

Expected RED: import fails because `app.workspace_observer` does not exist.

- [ ] **Step 3: GREEN — implement deterministic snapshots and comparison**

Create `app/workspace_observer.py`:

```python
from __future__ import annotations

import hashlib
import os
from dataclasses import dataclass
from pathlib import Path

from app.models import WorkspaceObservation


@dataclass(frozen=True)
class FileState:
    kind: str
    digest: str
    size_bytes: int


WorkspaceSnapshot = dict[str, FileState]


def snapshot_workspace(workspace: Path) -> WorkspaceSnapshot:
    snapshot: WorkspaceSnapshot = {}
    for path in sorted(workspace.rglob("*")):
        relative = path.relative_to(workspace)
        if ".git" in relative.parts or path.is_dir():
            continue
        key = relative.as_posix()
        if path.is_symlink():
            target = os.readlink(path)
            snapshot[key] = FileState(
                kind="symlink",
                digest=hashlib.sha256(target.encode("utf-8")).hexdigest(),
                size_bytes=len(target.encode("utf-8")),
            )
            continue
        digest = hashlib.sha256()
        size_bytes = 0
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
                size_bytes += len(chunk)
        snapshot[key] = FileState(
            kind="file",
            digest=digest.hexdigest(),
            size_bytes=size_bytes,
        )
    return snapshot


def observe_workspace_changes(
    before: WorkspaceSnapshot,
    workspace: Path,
    *,
    reported_changed_files: list[str],
) -> WorkspaceObservation:
    try:
        after = snapshot_workspace(workspace)
    except OSError as exc:
        return WorkspaceObservation(
            available=False,
            source="filesystem_snapshot",
            reported_changed_files=sorted(set(reported_changed_files)),
            error=f"{type(exc).__name__}: {exc}",
        )
    before_paths = set(before)
    after_paths = set(after)
    added = sorted(after_paths - before_paths)
    deleted = sorted(before_paths - after_paths)
    modified = sorted(
        path for path in before_paths & after_paths if before[path] != after[path]
    )
    changed = sorted([*added, *deleted, *modified])
    reported = sorted(set(reported_changed_files))
    mismatch = sorted(set(changed).symmetric_difference(reported))
    return WorkspaceObservation(
        available=True,
        source="filesystem_snapshot",
        changed_files=changed,
        added_files=added,
        modified_files=modified,
        deleted_files=deleted,
        reported_changed_files=reported,
        mismatch_files=mismatch,
    )
```

- [ ] **Step 4: Run GREEN tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_workspace_observer.py -q
```

Expected GREEN: all three observer tests pass.

- [ ] **Step 5: REFACTOR — prove deterministic ordering and `.git` exclusion**

Add a test that creates files in reverse lexical order plus `.git/config`, then asserts snapshot keys are stable and `.git/config` is absent. Run:

```bash
cd apps/eval-runner
uv run ruff check app/workspace_observer.py tests/test_workspace_observer.py
uv run pytest tests/test_workspace_observer.py -q
```

Expected: Ruff passes and all workspace-observer tests pass.

- [ ] **Step 6: Verify change scope and commit**

```bash
git add apps/eval-runner/app/workspace_observer.py \
  apps/eval-runner/app/models.py \
  apps/eval-runner/tests/test_workspace_observer.py
git commit -m "feat(eval): observe workspace effects independently"
```

---

### Task 5: Make Forge Execution Produce Independent, Scrubbed, Replayable Evidence

**Files:**

- Modify: `apps/eval-runner/app/runner.py:172-846`
- Modify: `apps/eval-runner/app/sandbox.py:1-180`
- Modify: `apps/eval-runner/app/patches.py:1-20`
- Modify: `apps/eval-runner/app/harness_checks.py:1-14`
- Modify: `apps/eval-runner/tests/test_runner.py`
- Modify: `apps/eval-runner/tests/test_smoke.py`

**GitNexus impact targets:** `ForgeAgentRunner.run_task`, `ForgeAgentRunner._trace_from_payload`, `run_setup_commands`, `run_validation_commands`, `run_post_validation_commands`, `run_shell_commands`, `scrub_future_repo_state`, `assert_clean_workspace`, `replay_patch`, `run_golden_harness_check`.

- [ ] **Step 1: Run upstream impact analysis and warn on central-runner risk**

```text
impact({target_uid: "Method:apps/eval-runner/app/runner.py:ForgeAgentRunner.run_task#1", target: "run_task", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "_trace_from_payload", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "run_shell_commands", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "scrub_future_repo_state", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "replay_patch", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "run_golden_harness_check", direction: "upstream", repo: "forge", includeTests: true})
```

Expected: runner change affects CLI/API/worker execution. Treat HIGH or CRITICAL as a user-visible warning before editing.

- [ ] **Step 2: RED — add falsified/omitted changed-file and command-boundary tests**

Add exact tests to `tests/test_runner.py`:

```python
def test_forge_runner_detects_unreported_forbidden_workspace_change(tmp_path: Path) -> None:
    fixture = tmp_path / "fixture"
    fixture.mkdir()
    (fixture / "allowed.txt").write_text("before\n", encoding="utf-8")
    script = tmp_path / "agent.py"
    script.write_text(
        "import json,pathlib,sys\n"
        "payload=json.loads(sys.stdin.read())\n"
        "workspace=pathlib.Path(payload['workspace_path'])\n"
        "(workspace/'.env').write_text('SECRET=x\\n')\n"
        "json.dump({'changed_files': [], 'file_diffs': [], 'final_answer': 'done'}, sys.stdout)\n",
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="independent-scope",
        title="Independent scope",
        prompt="Change allowed.txt only.",
        fixture_path=str(fixture),
        expected_files_changed=["allowed.txt"],
        forbidden_files_changed=[".env"],
    )
    trace = ForgeAgentRunner(
        provider=EvalProvider.FORGE,
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)
    assert trace.changed_files == [".env"]
    assert trace.workspace_observation is not None
    assert trace.workspace_observation.reported_changed_files == []
    assert trace.scope_violations == ["forbidden_change:.env", "unexpected_change:.env"]


def test_forge_runner_does_not_attribute_setup_or_validation_changes_to_agent(
    tmp_path: Path,
) -> None:
    fixture = tmp_path / "fixture"
    fixture.mkdir()
    script = tmp_path / "agent.py"
    script.write_text(
        "import json,sys\njson.dump({'changed_files': [], 'final_answer': 'done'}, sys.stdout)\n",
        encoding="utf-8",
    )
    task = EvaluationTask(
        id="snapshot-boundaries",
        title="Snapshot boundaries",
        prompt="Run without edits.",
        fixture_path=str(fixture),
        setup_commands=[f"{sys.executable} -c \"open('setup.txt','w').write('x')\""],
        validation_commands=[f"{sys.executable} -c \"open('validation.txt','w').write('x')\""],
    )
    trace = ForgeAgentRunner(
        provider=EvalProvider.FORGE,
        model="local-forge",
        command=[sys.executable, str(script)],
    ).run_task(task)
    assert trace.changed_files == []
```

Add timeout and cancellation tests named:

```python
test_forge_runner_times_out_setup_command
test_forge_runner_times_out_validation_command
test_forge_runner_cancellation_preserves_partial_output
test_forge_runner_records_failed_sandbox_scrub
test_forge_runner_records_failed_patch_replay
```

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_runner.py -k \
  "unreported_forbidden or snapshot_boundaries or times_out_setup or times_out_validation or cancellation_preserves or failed_sandbox or failed_patch" -q
```

Expected RED: workspace changes still come from payloads, setup/validation are unbounded, and trace models do not carry scrub/replay evidence.

- [ ] **Step 3: GREEN — replace every runner subprocess with `run_bounded_process`**

Change `EvalRunner` to:

```python
class EvalRunner(Protocol):
    def run_task(
        self,
        task: EvaluationTask,
        *,
        cancel_requested: CancelRequested = never_cancelled,
    ) -> AgentTrace:
        raise NotImplementedError
```

Update both runners to accept the keyword argument. The mock runner ignores cancellation until the start of execution; if already cancelled, it returns a trace with `error="cancelled"`, `failure_reason="Eval run was cancelled before task execution."`, and `FailureCategory.RUNNER_ERROR`.

Replace `run_shell_commands` with:

```python
def run_shell_commands(
    commands: Sequence[str],
    workspace: Path,
    *,
    timeout_seconds: float,
    cancel_requested: CancelRequested = never_cancelled,
) -> list[ShellOutput]:
    outputs: list[ShellOutput] = []
    for command in commands:
        result = run_bounded_process(
            command,
            cwd=workspace,
            timeout_seconds=timeout_seconds,
            cancel_requested=cancel_requested,
            shell=True,
        )
        outputs.append(
            ShellOutput(
                command=command,
                stdout=result.stdout,
                stderr=result.stderr,
                exit_code=result.returncode,
                duration_ms=result.duration_ms,
            )
        )
        if result.outcome != ProcessOutcome.COMPLETED or result.returncode != 0:
            break
    return outputs
```

Pass setup, Forge, validation, split-validation, and post-validation deadlines explicitly. Map outcome `TIMED_OUT` to `FailureCategory.TIMEOUT`, exit `124`, and `error="timeout"`; map `CANCELLED` to `error="cancelled"`, exit `130`, and a cancellation reason.

- [ ] **Step 4: GREEN — establish the exact workspace evidence order**

Refactor `ForgeAgentRunner.run_task` to execute these operations in order:

```text
prepare_workspace
run_setup_commands
scrub_future_repo_state
snapshot_workspace
run_bounded_process for Forge
observe_workspace_changes
parse_forge_stdout
run_validation_commands
run pass_to_pass_commands
run fail_to_pass_commands
run_post_validation_commands
build AgentTrace
replay patch in a fresh fixture workspace
```

Pass `WorkspaceObservation` into `_trace_from_payload`. Set:

```python
changed_files = workspace_observation.changed_files
scope_violations = scope_violations_for(task, changed_files)
```

Populate `trace.workspace_observation`, `trace.sandbox_scrub`, and `trace.patch_replay`. Preserve payload paths only in `workspace_observation.reported_changed_files`. Add raw events `eval_workspace_observation`, `eval_sandbox_scrub`, and `eval_patch_replay` using `model_dump(mode="json")` so artifacts remain self-describing.

For mock traces, set an explicit observation:

```python
WorkspaceObservation(
    available=True,
    source="deterministic_mock_contract",
    changed_files=changed_files,
    reported_changed_files=changed_files,
)
```

The trust layer may accept that source only for `EvalProvider.MOCK`.

- [ ] **Step 5: GREEN — bound sandbox and patch helpers**

Change `sandbox.run_git` and `patches.replay_patch` to accept `timeout_seconds` and `cancel_requested`, call `run_bounded_process`, and return `ok=False` with a message containing `timed_out` or `cancelled` when the process outcome is not completed.

Change `run_golden_harness_check` to return `TrustGateResult`-compatible evidence through a `WorkspaceCheck`:

```python
def run_golden_harness_check(cases_path: Path) -> WorkspaceCheck:
    tasks = load_cases(cases_path)
    golden_tasks = [
        task
        for task in tasks
        if task.expected_success and not bool(task.metadata.get("contract_only"))
    ]
    if not golden_tasks:
        return WorkspaceCheck(ok=False, message="No executable expected-success golden cases found.")
    traces = [DeterministicMockRunner().run_task(task) for task in golden_tasks]
    failed = [trace.task_id for trace in traces if not trace_passed(trace)]
    return WorkspaceCheck(
        ok=not failed,
        modified_files=failed,
        message=None if not failed else "Golden cases failed: " + ", ".join(failed),
    )
```

- [ ] **Step 6: Run GREEN integration tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_process_control.py tests/test_workspace_observer.py tests/test_runner.py tests/test_smoke.py::test_golden_harness_check_passes_for_expected_success_cases -q
```

Expected GREEN: all process, observer, runner, and golden-harness tests pass.

- [ ] **Step 7: REFACTOR — update payload-only runner fixtures**

For existing passing Forge tests that claim changed files, make their scripts perform the corresponding workspace write. Tests whose purpose is payload normalization may assert the mismatch explicitly. Do not relax independent scope checks to preserve an old fixture.

Run:

```bash
cd apps/eval-runner
uv run ruff check app/runner.py app/sandbox.py app/patches.py app/harness_checks.py tests/test_runner.py
uv run pytest tests/test_runner.py tests/test_smoke.py -q
```

Expected: Ruff passes and all runner/smoke tests pass.

- [ ] **Step 8: Verify change scope and commit**

```bash
git add apps/eval-runner/app/runner.py \
  apps/eval-runner/app/sandbox.py \
  apps/eval-runner/app/patches.py \
  apps/eval-runner/app/harness_checks.py \
  apps/eval-runner/tests/test_runner.py \
  apps/eval-runner/tests/test_smoke.py
git commit -m "feat(eval): derive scrubbed workspace evidence"
```

---

### Task 6: Publish Score Coverage With Explicit Denominators

**Files:**

- Modify: `apps/eval-runner/app/scoring.py:5-122`
- Modify: `apps/eval-runner/app/reporting.py:9-80`
- Modify: `apps/eval-runner/app/models.py`
- Modify: `apps/eval-runner/tests/test_reporting.py`
- Modify: `apps/eval-runner/tests/test_metrics.py`
- Modify: `apps/eval-runner/eval_cases/*/case.json` only where a feature-specific required score must be declared

**GitNexus impact targets:** `score_trace`, `forge_run_evidence_scores`, `build_report`, `build_score_summary`, `BacktestReport`, `EvaluationTask.required_scores`, `AgentTrace.required_scores`.

- [ ] **Step 1: Run upstream impact analysis**

```text
impact({target: "score_trace", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "forge_run_evidence_scores", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "build_report", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "build_score_summary", direction: "upstream", repo: "forge", includeTests: true})
```

Expected: reporting changes affect API report responses, SQLite report artifacts, CLI JSON, and release-confidence consumers.

- [ ] **Step 2: RED — prove missing score evidence lowers coverage**

Add to `tests/test_reporting.py`:

```python
def test_score_coverage_uses_required_trace_denominator() -> None:
    first = make_trace(
        "required-a",
        verification_passed=True,
        duration_ms=10,
        model_rounds=1,
        confirm_requests=0,
    ).model_copy(update={"required_scores": ["forge_file_effects_evidence_ok"]})
    second = make_trace(
        "required-b",
        verification_passed=True,
        duration_ms=10,
        model_rounds=1,
        confirm_requests=0,
    ).model_copy(update={"required_scores": ["forge_file_effects_evidence_ok"]})
    first = first.model_copy(
        update={
            "forge_run_evidence": ForgeRunEvidence(
                session_id="s",
                loop_task_id="l",
                changed_files=["src/app.py"],
                file_diffs=[{"path": "src/app.py", "diff": "patch"}],
            )
        }
    )
    report = build_report([first, second])
    aggregate = report.score_coverage["forge_file_effects_evidence_ok"]
    assert aggregate.observed == 1
    assert aggregate.expected == 2
    assert aggregate.coverage == 0.5


def test_universal_score_coverage_uses_all_traces() -> None:
    traces = [
        make_trace(
            f"task-{index}",
            verification_passed=True,
            duration_ms=10,
            model_rounds=1,
            confirm_requests=0,
        )
        for index in range(3)
    ]
    aggregate = build_report(traces).score_coverage["functional_correctness"]
    assert aggregate.observed == 3
    assert aggregate.expected == 3
    assert aggregate.coverage == 1.0
```

Run:

```bash
cd apps/eval-runner
uv run pytest \
  tests/test_reporting.py::test_score_coverage_uses_required_trace_denominator \
  tests/test_reporting.py::test_universal_score_coverage_uses_all_traces -q
```

Expected RED: `BacktestReport` has no populated `score_coverage` and traces do not propagate required score names.

- [ ] **Step 3: GREEN — aggregate score values and coverage separately**

Replace `build_score_summary` with:

```python
UNIVERSAL_SCORE_NAMES = {
    "functional_correctness",
    "scope_ok",
    "regression_ok",
    "bugfix_ok",
    "secret_leak_ok",
    "prompt_injection_ok",
    "scope_escape_ok",
    "future_state_leakage_ok",
    "unsafe_tool_use_ok",
}


def build_score_aggregates(
    traces: list[AgentTrace],
) -> tuple[dict[str, float], dict[str, ScoreCoverage]]:
    emitted = [score_trace(trace) for trace in traces]
    names = set(UNIVERSAL_SCORE_NAMES)
    names.update(name for scores in emitted for name in scores)
    names.update(name for trace in traces for name in trace.required_scores)
    summary: dict[str, float] = {}
    coverage: dict[str, ScoreCoverage] = {}
    for name in sorted(names):
        values = [scores[name].score for scores in emitted if name in scores]
        expected = (
            len(traces)
            if name in UNIVERSAL_SCORE_NAMES
            else sum(1 for trace in traces if name in trace.required_scores)
        )
        if expected == 0:
            expected = len(values)
        observed = min(len(values), expected)
        mean = sum(values[:observed]) / observed if observed else None
        if mean is not None:
            summary[name] = mean
        coverage[name] = ScoreCoverage(
            mean=mean,
            observed=observed,
            expected=expected,
            coverage=observed / expected if expected else 1.0,
        )
    return summary, coverage
```

In `build_report`, call the helper once and bind both fields. Ensure each runner copies `task.required_scores` into its `AgentTrace`. Add `required_scores` only to feature-specific cases that must gate release; universal scores require no JSON changes.

- [ ] **Step 4: Run GREEN reporting and scorer tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_reporting.py tests/test_metrics.py -q
```

Expected GREEN: existing score values remain stable and new coverage assertions pass.

- [ ] **Step 5: REFACTOR — validate every declared required score name**

Add a case-quality issue `unknown_required_score` when a case declares a name outside the scorer registry. Add a test in `tests/test_cases.py` with an invalid name and assert severity `error`.

Run:

```bash
cd apps/eval-runner
uv run ruff check app/scoring.py app/reporting.py app/cases.py tests/test_reporting.py tests/test_metrics.py tests/test_cases.py
uv run pytest tests/test_reporting.py tests/test_metrics.py tests/test_cases.py -q
```

Expected: Ruff passes and all reporting, metrics, and case tests pass.

- [ ] **Step 6: Verify change scope and commit**

```bash
git add apps/eval-runner/app/models.py \
  apps/eval-runner/app/scoring.py \
  apps/eval-runner/app/reporting.py \
  apps/eval-runner/app/cases.py \
  apps/eval-runner/tests/test_reporting.py \
  apps/eval-runner/tests/test_metrics.py \
  apps/eval-runner/tests/test_cases.py \
  apps/eval-runner/eval_cases
git commit -m "feat(eval): expose score evidence coverage"
```

---

### Task 7: Run One Fail-Closed Trust Orchestrator On Every Execution Path

**Files:**

- Create: `apps/eval-runner/app/execution.py`
- Create: `apps/eval-runner/tests/test_execution.py`
- Modify: `apps/eval-runner/app/trust_gates.py:1-20`
- Modify: `apps/eval-runner/app/cases.py:37-70`
- Modify: `apps/eval-runner/app/harness_checks.py`
- Modify: `apps/eval-runner/app/reporting.py`
- Modify: `apps/eval-runner/app/models.py`

**GitNexus impact targets:** `evaluate_trust_gates`, `validate_case_quality`, `run_golden_harness_check`, `build_report`, `score_can_gate_ci`, `create_runner`.

- [ ] **Step 1: Run upstream impact analysis**

```text
impact({target: "evaluate_trust_gates", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "validate_case_quality", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "run_golden_harness_check", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "build_report", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "score_can_gate_ci", direction: "upstream", repo: "forge", includeTests: true})
```

Expected: dormant helpers currently have test-only callers; the new orchestrator becomes their production caller.

- [ ] **Step 2: RED — specify trusted, untrusted, and unknown results**

Create `tests/test_execution.py` with helper cases and exact tests:

```python
def test_execute_evaluation_returns_trusted_for_complete_mock_evidence(
    tmp_path: Path,
) -> None:
    cases_path = write_executable_case(tmp_path, required_scores=[])
    execution = execute_evaluation(
        cases_path=cases_path,
        tasks=load_cases(cases_path),
        options=ExecutionOptions(
            provider=EvalProvider.MOCK,
            model="deterministic-agent-v1",
            forge_command=None,
            command_timeout_seconds=2.0,
            setup_timeout_seconds=2.0,
            validation_timeout_seconds=2.0,
            require_red_team=False,
        ),
    )
    assert execution.trust_result.status == TrustStatus.TRUSTED
    assert execution.trust_result.trusted is True


def test_execute_evaluation_is_unknown_when_workspace_evidence_is_missing(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    cases_path = write_executable_case(tmp_path, required_scores=[])
    monkeypatch.setattr(
        "app.execution.execute_tasks",
        lambda tasks, options, cancel_requested: [trace_without_workspace_evidence(tasks[0])],
    )
    execution = execute_evaluation(
        cases_path=cases_path,
        tasks=load_cases(cases_path),
        options=mock_options(),
    )
    assert execution.trust_result.status == TrustStatus.UNKNOWN
    assert "workspace_evidence_missing:case-1" in execution.trust_result.blockers


def test_execute_evaluation_is_untrusted_for_case_quality_error(tmp_path: Path) -> None:
    cases_path = write_case_with_missing_fixture(tmp_path)
    execution = execute_evaluation(
        cases_path=cases_path,
        tasks=load_cases(cases_path),
        options=mock_options(),
    )
    assert execution.trust_result.status == TrustStatus.UNTRUSTED
    assert "case_quality:case-1:missing_fixture_path" in execution.trust_result.blockers


def test_execute_evaluation_blocks_incomplete_required_score_coverage(
    tmp_path: Path,
) -> None:
    cases_path = write_executable_case(
        tmp_path,
        required_scores=["forge_file_effects_evidence_ok"],
    )
    execution = execute_evaluation(
        cases_path=cases_path,
        tasks=load_cases(cases_path),
        options=mock_options(),
    )
    assert execution.trust_result.status == TrustStatus.UNKNOWN
    assert "score_coverage_incomplete:forge_file_effects_evidence_ok" in execution.trust_result.blockers
```

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_execution.py -q
```

Expected RED: `app.execution` does not exist and trust gates cannot represent unknown evidence.

- [ ] **Step 3: GREEN — expand `evaluate_trust_gates` with explicit evidence states**

Replace its signature and logic with:

```python
def evaluate_trust_gates(
    *,
    harness_check: WorkspaceCheck | None,
    dataset_fingerprint: str | None,
    case_quality_issues: list[CaseQualityIssue],
    traces: list[AgentTrace],
    scorer_calibrated: bool | None,
    red_team_passed: bool | None,
    require_red_team: bool,
    score_coverage: dict[str, ScoreCoverage],
) -> TrustGateResult:
    blockers: list[str] = []
    unknown = False
    if harness_check is None:
        blockers.append("harness_evidence_missing")
        unknown = True
    elif not harness_check.ok:
        blockers.append("harness_untrusted")
    if not dataset_fingerprint:
        blockers.append("dataset_unfingerprinted")
        unknown = True
    for issue in case_quality_issues:
        blockers.append(f"case_quality:{issue.task_id}:{issue.code}")
    for trace in traces:
        observation = trace.workspace_observation
        if observation is None or not observation.available:
            blockers.append(f"workspace_evidence_missing:{trace.task_id}")
            unknown = True
        if trace.sandbox_scrub is not None and not trace.sandbox_scrub.ok:
            blockers.append(f"sandbox_untrusted:{trace.task_id}")
        if trace.provider == EvalProvider.FORGE:
            if trace.patch_replay is None:
                blockers.append(f"patch_replay_missing:{trace.task_id}")
                unknown = True
            elif not trace.patch_replay.ok:
                blockers.append(f"patch_replay_failed:{trace.task_id}")
    if scorer_calibrated is None:
        blockers.append("scorer_calibration_missing")
        unknown = True
    elif not scorer_calibrated:
        blockers.append("scorer_uncalibrated")
    if require_red_team:
        if red_team_passed is None:
            blockers.append("red_team_evidence_missing")
            unknown = True
        elif not red_team_passed:
            blockers.append("red_team_failed")
    required_names = sorted({name for trace in traces for name in trace.required_scores})
    for name in required_names:
        aggregate = score_coverage.get(name)
        if aggregate is None or aggregate.coverage < 1.0:
            blockers.append(f"score_coverage_incomplete:{name}")
            unknown = True
    blockers = sorted(set(blockers))
    if not blockers:
        return TrustGateResult(status=TrustStatus.TRUSTED, trusted=True)
    return TrustGateResult(
        status=TrustStatus.UNKNOWN if unknown else TrustStatus.UNTRUSTED,
        trusted=False,
        blockers=blockers,
    )
```

Case-quality issues of either warning or error block trusted release evidence unless the case is explicitly `metadata.contract_only: true`; the existing validator already suppresses executable assertions for contract-only cases.

- [ ] **Step 4: GREEN — create common execution contracts and orchestration**

Create `app/execution.py` with:

```python
from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from app.cases import validate_case_quality
from app.datasets import dataset_fingerprint
from app.harness_checks import run_golden_harness_check
from app.judge_calibration import score_can_gate_ci
from app.models import (
    AgentTrace,
    BacktestReport,
    EvalProvider,
    EvaluationTask,
    TrustGateResult,
)
from app.process_control import CancelRequested, never_cancelled
from app.red_team import is_red_team_task
from app.reporting import build_report
from app.runner import create_runner
from app.scoring import score_trace
from app.trust_gates import evaluate_trust_gates


@dataclass(frozen=True)
class ExecutionOptions:
    provider: EvalProvider
    model: str
    forge_command: str | None
    command_timeout_seconds: float
    setup_timeout_seconds: float
    validation_timeout_seconds: float
    require_red_team: bool


@dataclass(frozen=True)
class EvaluationExecution:
    traces: list[AgentTrace]
    report: BacktestReport
    trust_result: TrustGateResult


def execute_tasks(
    tasks: list[EvaluationTask],
    options: ExecutionOptions,
    cancel_requested: CancelRequested,
) -> list[AgentTrace]:
    runner = create_runner(
        provider=options.provider,
        model=options.model,
        forge_command=options.forge_command,
    )
    traces: list[AgentTrace] = []
    for task in tasks:
        if cancel_requested():
            break
        traces.append(runner.run_task(task, cancel_requested=cancel_requested))
    return traces


def execute_evaluation(
    *,
    cases_path: Path,
    tasks: list[EvaluationTask],
    options: ExecutionOptions,
    cancel_requested: CancelRequested = never_cancelled,
) -> EvaluationExecution:
    quality = validate_case_quality(tasks)
    fingerprint = dataset_fingerprint(tasks) if tasks else None
    harness = run_golden_harness_check(cases_path)
    traces = execute_tasks(tasks, options, cancel_requested)
    report = build_report(traces)
    scorer_calibrated = all(
        score_can_gate_ci(score)
        for trace in traces
        for score in score_trace(trace).values()
    )
    red_team_traces = [
        trace
        for trace, task in zip(traces, tasks, strict=False)
        if is_red_team_task(task)
    ]
    red_team_passed = (
        all(trace.error is None for trace in red_team_traces)
        if red_team_traces
        else None
    )
    trust = evaluate_trust_gates(
        harness_check=harness,
        dataset_fingerprint=fingerprint,
        case_quality_issues=quality,
        traces=traces,
        scorer_calibrated=scorer_calibrated,
        red_team_passed=red_team_passed,
        require_red_team=options.require_red_team,
        score_coverage=report.score_coverage,
    )
    report = report.model_copy(update={"trust_result": trust})
    return EvaluationExecution(traces=traces, report=report, trust_result=trust)
```

- [ ] **Step 5: Run GREEN trust-orchestrator tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_execution.py tests/test_reporting.py tests/test_cases.py tests/test_smoke.py::test_golden_harness_check_passes_for_expected_success_cases -q
```

Expected GREEN: common execution produces trusted, unknown, and untrusted results exactly as asserted.

- [ ] **Step 6: REFACTOR — verify no main path duplicates trust inputs**

Use a normal `score_trace` import in `execution.py`. Run:

```bash
cd apps/eval-runner
rg -n "evaluate_trust_gates|validate_case_quality|run_golden_harness_check" app
uv run ruff check app/execution.py app/trust_gates.py tests/test_execution.py
uv run pytest tests/test_execution.py tests/test_reporting.py -q
```

Expected search result: production calls are centralized in `app/execution.py`; definitions remain in their owning modules. Ruff and tests pass.

- [ ] **Step 7: Verify change scope and commit**

```bash
git add apps/eval-runner/app/execution.py \
  apps/eval-runner/app/trust_gates.py \
  apps/eval-runner/app/cases.py \
  apps/eval-runner/app/harness_checks.py \
  apps/eval-runner/app/reporting.py \
  apps/eval-runner/app/models.py \
  apps/eval-runner/tests/test_execution.py \
  apps/eval-runner/tests/test_reporting.py
git commit -m "feat(eval): centralize fail-closed trust execution"
```

---

### Task 8: Wire Trust And Stable Exit Semantics Into The CLI

**Files:**

- Modify: `apps/eval-runner/app/cli.py:16-244`
- Modify: `apps/eval-runner/tests/test_cli.py`

**GitNexus impact targets:** `run_backtest_with_traces`, `run_backtest`, `threshold_failures`, `load_backtest_tasks`, `app.cli.main`.

- [ ] **Step 1: Run upstream impact analysis**

```text
impact({target: "run_backtest_with_traces", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "run_backtest", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "threshold_failures", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "main", file_path: "apps/eval-runner/app/cli.py", kind: "Function", direction: "upstream", repo: "forge", includeTests: true})
```

Expected: CLI behavior affects direct tests, npm Eval gates, and operator documentation.

- [ ] **Step 2: RED — specify CI, report-only, and invalid-input exit codes**

Add exact tests:

```python
def test_cli_exits_nonzero_when_trust_is_unknown(tmp_path: Path, capsys) -> None:
    cases_dir = tmp_path / "cases"
    write_case(cases_dir, "missing-evidence", metadata={"contract_only": True})
    exit_code = main(["--cases", str(cases_dir), "--provider", "mock"])
    payload = json.loads(capsys.readouterr().out)
    assert exit_code == 1
    assert payload["trust_result"]["status"] == "unknown"


def test_cli_report_only_prints_blockers_and_exits_zero(tmp_path: Path, capsys) -> None:
    cases_dir = tmp_path / "cases"
    write_case(cases_dir, "missing-evidence", metadata={"contract_only": True})
    exit_code = main(
        ["--cases", str(cases_dir), "--provider", "mock", "--report-only"]
    )
    payload = json.loads(capsys.readouterr().out)
    assert exit_code == 0
    assert payload["trust_result"]["trusted"] is False


def test_cli_invalid_provider_exits_two(capsys) -> None:
    with pytest.raises(SystemExit) as exc_info:
        main(["--cases", "eval_cases", "--provider", "unknown"])
    assert exc_info.value.code == 2
```

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_cli.py -k "trust_is_unknown or report_only or invalid_provider" -q
```

Expected RED: CLI does not emit trust data and currently exits `0` when trust evidence is absent.

- [ ] **Step 3: GREEN — invoke `execute_evaluation` and define exit behavior**

Add parser flags:

```python
parser.add_argument(
    "--report-only",
    action="store_true",
    help="Print execution and trust results without using trust blockers as an exit gate.",
)
parser.add_argument(
    "--require-red-team",
    action="store_true",
    help="Require red-team evidence for this trusted run.",
)
```

Replace direct runner execution in `run_backtest_with_traces` with `execute_evaluation`. Return `EvaluationExecution` from the internal function; keep `run_backtest` returning only `BacktestReport` for compatibility.

After printing the report, calculate exit code in this order:

```python
failures = threshold_failures(execution.report, args)
for failure in failures:
    print(f"error: {failure}", file=sys.stderr)
if args.report_only:
    return 1 if failures else 0
if execution.trust_result.status != TrustStatus.TRUSTED:
    for blocker in execution.trust_result.blockers:
        print(f"error: trust blocker: {blocker}", file=sys.stderr)
    return 1
return 1 if failures else 0
```

Pydantic/argparse input errors use exit `2`. Execution timeout/cancellation and threshold/trust failures use exit `1`. Only trusted threshold-passing execution uses `0` outside report-only mode.

- [ ] **Step 4: Run GREEN CLI tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_cli.py -q
```

Expected GREEN: existing threshold tests and the new trust/exit tests pass. Update old happy-path invocations to add `--report-only` only when their purpose is report formatting rather than release gating.

- [ ] **Step 5: REFACTOR — ensure output artifacts include the same trust result**

Change `write_backtest_artifact` so `payload["report"]` already contains `trust_result` and `score_coverage`; do not add a conflicting top-level trust copy. Run:

```bash
cd apps/eval-runner
uv run ruff check app/cli.py tests/test_cli.py
uv run pytest tests/test_cli.py -q
```

Expected: Ruff and all CLI tests pass.

- [ ] **Step 6: Verify change scope and commit**

```bash
git add apps/eval-runner/app/cli.py apps/eval-runner/tests/test_cli.py
git commit -m "feat(eval): fail CI on untrusted evidence"
```

---

### Task 9: Authenticate Non-Loopback API Use And Wire Synchronous Trust

**Files:**

- Modify: `apps/eval-runner/app/config.py:1-24`
- Modify: `apps/eval-runner/app/main.py:1-162`
- Modify: `apps/eval-runner/tests/test_api.py`

**GitNexus impact targets:** `Settings`, `get_settings`, `create_app`, `build_storage`, nested route handlers. Run API impacts for every protected route.

- [ ] **Step 1: Run upstream and API-route impact analysis**

```text
impact({target: "Settings", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "create_app", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "build_storage", direction: "upstream", repo: "forge", includeTests: true})
api_impact({route: "/tasks", repo: "forge"})
api_impact({route: "/runs", repo: "forge"})
api_impact({route: "/runs/{run_id}", repo: "forge"})
api_impact({route: "/runs/{run_id}/cancel", repo: "forge"})
api_impact({route: "/queue/status", repo: "forge"})
```

Expected: `create_app` affects all API tests. Warn before changing response/auth contracts if risk is HIGH or CRITICAL.

- [ ] **Step 2: RED — specify startup and bearer-token policy**

Add exact tests:

```python
def test_non_loopback_api_requires_configured_token(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    settings = Settings(tasks_path=tasks_path, api_bind_host="0.0.0.0", api_token=None)
    with pytest.raises(ValueError, match="API token is required for non-loopback bind host"):
        create_app(
            storage=InMemoryStorage(tasks_path=tasks_path),
            settings=settings,
        )


def test_protected_api_routes_require_bearer_token(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    settings = Settings(
        tasks_path=tasks_path,
        api_bind_host="0.0.0.0",
        api_token="secret-token",
    )
    client = TestClient(
        create_app(storage=InMemoryStorage(tasks_path=tasks_path), settings=settings)
    )
    assert client.get("/health").status_code == 200
    assert client.get("/tasks").status_code == 401
    assert client.get("/tasks", headers={"Authorization": "Bearer wrong"}).status_code == 401
    response = client.get(
        "/tasks",
        headers={"Authorization": "Bearer secret-token"},
    )
    assert response.status_code == 200


def test_sync_api_persists_completed_but_untrusted_result(tmp_path: Path) -> None:
    tasks_path = tmp_path / "tasks.json"
    write_tasks(tasks_path)
    settings = Settings(tasks_path=tasks_path, run_execution_mode="sync")
    storage = InMemoryStorage(tasks_path=tasks_path)
    client = TestClient(create_app(storage=storage, settings=settings))
    response = client.post(
        "/runs",
        json={"task_ids": ["task-pass"], "provider": "mock"},
    )
    assert response.status_code == 201
    payload = response.json()
    assert payload["status"] == "completed"
    assert payload["trust_result"]["status"] in {"trusted", "untrusted", "unknown"}
    assert storage.get_run(payload["run_id"]).trust_result == TrustGateResult.model_validate(
        payload["trust_result"]
    )
```

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_api.py -k "non_loopback or bearer_token or sync_api_persists" -q
```

Expected RED: `Settings` has no API fields, `create_app` cannot accept injected settings, routes are open, and sync execution does not persist trust.

- [ ] **Step 3: GREEN — add explicit API exposure settings**

Add to `Settings`:

```python
api_bind_host: str = "127.0.0.1"
api_token: str | None = None
```

Add helpers to `app/main.py`:

```python
import ipaddress
import secrets

from fastapi import Depends, Header


def is_loopback_host(host: str) -> bool:
    if host == "localhost":
        return True
    try:
        return ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


def validate_api_exposure(settings: Settings) -> None:
    if not is_loopback_host(settings.api_bind_host) and not settings.api_token:
        raise ValueError("API token is required for non-loopback bind host")


def build_auth_dependency(settings: Settings):
    def require_api_token(authorization: str | None = Header(default=None)) -> None:
        if settings.api_token is None:
            return
        scheme, separator, supplied = (authorization or "").partition(" ")
        valid = (
            separator == " "
            and scheme.casefold() == "bearer"
            and secrets.compare_digest(supplied, settings.api_token)
        )
        if not valid:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Invalid or missing API token",
                headers={"WWW-Authenticate": "Bearer"},
            )
    return require_api_token
```

Change signature:

```python
def create_app(
    storage: EvalStorage | None = None,
    settings: Settings | None = None,
) -> FastAPI:
    settings = settings or get_settings()
    validate_api_exposure(settings)
```

Leave `/health` unprotected. Add `dependencies=[Depends(require_api_token)]` to `/tasks`, all `/runs` routes, and `/queue/status`.

- [ ] **Step 4: GREEN — route synchronous execution through `execute_evaluation`**

Replace the sync list comprehension at `app/main.py:93-114` with `ExecutionOptions` and `execute_evaluation`. Set the completed run's `traces`, `metrics`, and `trust_result` from the execution result. Keep HTTP `201` for completed-but-untrusted runs because execution and trust are separate axes.

For queued runs, persist `TrustGateResult()` with status unknown until the worker evaluates evidence.

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_api.py -q
```

Expected GREEN: all API tests pass with explicit settings injection and authentication policy.

- [ ] **Step 5: REFACTOR — protect every advertised route consistently**

Run:

```bash
cd apps/eval-runner
rg -n '^    @app\.(get|post)' app/main.py
uv run ruff check app/config.py app/main.py tests/test_api.py
uv run pytest tests/test_api.py -q
```

Expected: `/health` is the only unauthenticated route when a token is configured; Ruff and API tests pass.

- [ ] **Step 6: Verify change scope and commit**

```bash
git add apps/eval-runner/app/config.py \
  apps/eval-runner/app/main.py \
  apps/eval-runner/tests/test_api.py
git commit -m "feat(eval): authenticate exposed API execution"
```

---

### Task 10: Fence Worker Claims And Reject Every Stale Write

**Files:**

- Modify: `apps/eval-runner/app/storage.py:29-890`
- Modify: `apps/eval-runner/app/models.py`
- Modify: `apps/eval-runner/tests/test_storage.py`

**GitNexus impact targets:** `EvalStorage`, both `claim_pending_run` methods, both `save_task` methods, both `heartbeat_run` methods, both `_finalize_run` methods, `SQLiteStorage._write_run_artifacts`, `EvalArtifact`.

- [ ] **Step 1: Run upstream impact analysis**

```text
impact({target: "EvalStorage", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "claim_pending_run", file_path: "apps/eval-runner/app/storage.py", kind: "Method", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "save_task", file_path: "apps/eval-runner/app/storage.py", kind: "Method", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "heartbeat_run", file_path: "apps/eval-runner/app/storage.py", kind: "Method", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "_finalize_run", file_path: "apps/eval-runner/app/storage.py", kind: "Method", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "EvalArtifact", direction: "upstream", repo: "forge", includeTests: true})
```

Expected: storage protocol changes affect worker and storage/worker/API/smoke tests. Warn before editing on HIGH or CRITICAL risk.

- [ ] **Step 2: RED — specify token rotation and stale-write rejection**

Add these parameterized tests for memory and SQLite storage:

```python
def claim(storage, run_id: str, worker_id: str) -> EvaluationRun:
    claimed = storage.claim_pending_run(worker_id=worker_id)
    assert claimed is not None
    assert claimed.run_id == run_id
    assert claimed.lease_token is not None
    return claimed


@pytest.mark.parametrize(("storage_name", "storage_factory"), storage_factories())
def test_reclaim_rotates_lease_token(
    tmp_path: Path,
    storage_name: str,
    storage_factory: StorageFactory,
) -> None:
    storage = make_storage(tmp_path, storage_name, storage_factory)
    storage.create_run(make_run("run-1").model_copy(update={"status": RunStatus.PENDING}))
    first = claim(storage, "run-1", "worker-a")
    storage.force_lease_expiry_for_test("run-1", datetime(2000, 1, 1, tzinfo=UTC))
    second = claim(storage, "run-1", "worker-b")
    assert second.lease_token != first.lease_token


@pytest.mark.parametrize(("storage_name", "storage_factory"), storage_factories())
def test_stale_worker_cannot_heartbeat_save_or_complete(
    tmp_path: Path,
    storage_name: str,
    storage_factory: StorageFactory,
) -> None:
    storage = make_storage(tmp_path, storage_name, storage_factory)
    storage.create_run(make_run("run-1").model_copy(update={"status": RunStatus.PENDING}))
    first = claim(storage, "run-1", "worker-a")
    storage.force_lease_expiry_for_test("run-1", datetime(2000, 1, 1, tzinfo=UTC))
    second = claim(storage, "run-1", "worker-b")
    with pytest.raises(LeaseLostError):
        storage.heartbeat_run(
            "run-1",
            worker_id="worker-a",
            lease_token=first.lease_token,
            lease_expires_at=datetime(2099, 1, 1, tzinfo=UTC),
        )
    with pytest.raises(LeaseLostError):
        storage.save_task(
            "run-1",
            make_trace("task-pass", raw_marker="stale"),
            worker_id="worker-a",
            lease_token=first.lease_token,
        )
    with pytest.raises(LeaseLostError):
        storage.complete_run(
            first.model_copy(update={"traces": [make_trace("task-pass")]}),
            worker_id="worker-a",
            lease_token=first.lease_token,
        )
    current = storage.get_run("run-1")
    assert current is not None
    assert current.worker_id == "worker-b"
    assert current.lease_token == second.lease_token
    assert current.status == RunStatus.RUNNING
```

Add SQLite-only artifact proof:

```python
def test_stale_attempt_artifacts_never_replace_canonical_artifacts(tmp_path: Path) -> None:
    storage = make_sqlite_storage(tmp_path)
    storage.create_run(make_pending_run("run-1"))
    first = claim(storage, "run-1", "worker-a")
    storage.force_lease_expiry_for_test("run-1", datetime(2000, 1, 1, tzinfo=UTC))
    second = claim(storage, "run-1", "worker-b")
    storage.save_task(
        "run-1",
        make_trace("task-pass", raw_marker="winner"),
        worker_id="worker-b",
        lease_token=second.lease_token,
    )
    completed = storage.complete_run(
        second.model_copy(update={"traces": [make_trace("task-pass", raw_marker="winner")]}),
        worker_id="worker-b",
        lease_token=second.lease_token,
    )
    assert completed.status == RunStatus.COMPLETED
    assert "winner" in Path(next(a.path for a in storage.list_artifacts("run-1") if a.kind == "trace")).read_text()
    assert first.lease_token not in {a.attempt_token for a in storage.list_artifacts("run-1")}
```

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_storage.py -k "rotates_lease or stale_worker or stale_attempt" -q
```

Expected RED: claims have no token, heartbeats and task writes accept stale owners, and finalization checks only run status.

- [ ] **Step 3: GREEN — make the storage protocol fenced**

Add `attempt_token: str | None = None` to `EvalArtifact`. Add:

```python
class LeaseLostError(RuntimeError):
    """Raised when a worker no longer owns the claimed run attempt."""
```

Use these exact protocol signatures. The worker passes `Settings.lease_duration_seconds`; direct tests retain the 300-second default.

```python
def claim_pending_run(
    self,
    worker_id: str | None = None,
    lease_duration_seconds: float = 300.0,
) -> EvaluationRun | None:
    raise NotImplementedError

def save_task(
    self,
    run_id: str,
    trace: AgentTrace,
    *,
    worker_id: str,
    lease_token: str,
) -> None:
    raise NotImplementedError

def heartbeat_run(
    self,
    run_id: str,
    worker_id: str,
    lease_token: str,
    lease_expires_at: datetime,
) -> None:
    raise NotImplementedError

def complete_run(
    self,
    run: EvaluationRun,
    *,
    worker_id: str,
    lease_token: str,
) -> EvaluationRun:
    raise NotImplementedError
```

Give `fail_run` and `retry_run` the same keyword-only owner/token arguments. Every claim sets `lease_token=str(uuid4())`; every reclaim replaces it. A retry clears `worker_id`, `lease_token`, `claimed_at`, `heartbeat_at`, and `lease_expires_at` after the fenced compare-and-set succeeds.

Add one private assertion to each backend:

```python
def _require_active_lease(
    self,
    run_id: str,
    worker_id: str,
    lease_token: str,
) -> EvaluationRun:
    run = self._require_run(run_id)
    now = datetime.now(UTC)
    active = (
        run.status == RunStatus.RUNNING
        and run.worker_id == worker_id
        and run.lease_token == lease_token
        and run.lease_expires_at is not None
        and run.lease_expires_at >= now
    )
    if not active:
        raise LeaseLostError(f"Worker {worker_id} lost lease for run {run_id}")
    return run
```

SQLite must perform the equivalent check inside `BEGIN IMMEDIATE`; do not rely on a prior `get_run`. Use owner/token/status/unexpired predicates in the same transaction before task or terminal metadata is written.

- [ ] **Step 4: GREEN — isolate attempt artifacts and publish only the winning token**

Migrate `eval_artifacts.attempt_token` with `ensure_column`. Write intermediate files under:

```text
artifacts/{run_id}/attempts/{lease_token}/trace.json
artifacts/{run_id}/attempts/{lease_token}/report.json
artifacts/{run_id}/attempts/{lease_token}/{task_id}.trajectory.json
```

`save_task` may write only to the active attempt directory after the fenced check. `_finalize_run` repeats the fenced check and then writes canonical `artifacts/{run_id}/trace.json`, `report.json`, and trajectories from the in-memory winning run before committing canonical artifact rows. Token-scoped attempt rows use kinds `trace_attempt`, `report_attempt`, and `trajectory_attempt`; `list_artifacts` returns only canonical rows unless passed `include_attempts=True`.

If the current status is `CANCELLED` and owner/token still match, return the cancelled row without promoting a completed/failed/retry state. This preserves the existing cancellation-race contract while rejecting a different token.

- [ ] **Step 5: GREEN — run all storage contract tests**

```bash
cd apps/eval-runner
uv run pytest tests/test_storage.py -q
```

Expected GREEN: identity, migration, cancel-race, lease, stale-write, and artifact tests pass for both backends.

- [ ] **Step 6: REFACTOR — centralize SQLite fencing predicates**

Create `_require_active_lease_connection(connection, run_id, worker_id, lease_token)` and use it from task, heartbeat, complete, fail, and retry paths. Keep `force_lease_expiry_for_test` explicitly test-only and exclude it from `EvalStorage` so production callers cannot depend on it.

Run:

```bash
cd apps/eval-runner
uv run ruff check app/storage.py app/models.py tests/test_storage.py
uv run pytest tests/test_storage.py -q
```

Expected: Ruff and all storage tests pass.

- [ ] **Step 7: Verify change scope and commit**

```bash
git add apps/eval-runner/app/models.py \
  apps/eval-runner/app/storage.py \
  apps/eval-runner/tests/test_storage.py
git commit -m "feat(eval): fence worker attempts and artifacts"
```

---

### Task 11: Make The Worker Cancellation-Aware And Fence Every Publication

**Files:**

- Modify: `apps/eval-runner/app/worker.py:16-231`
- Modify: `apps/eval-runner/tests/test_worker.py`
- Modify: `apps/eval-runner/tests/test_api.py`
- Modify: `apps/eval-runner/tests/test_smoke.py`

**GitNexus impact targets:** `EvalWorker.run_once`, `EvalWorker._start_background_heartbeat`, `EvalWorker._heartbeat`, `EvalWorker.stop`, `app.worker.main`.

- [ ] **Step 1: Run upstream impact analysis**

```text
impact({target: "run_once", file_path: "apps/eval-runner/app/worker.py", kind: "Method", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "_start_background_heartbeat", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "_heartbeat", file_path: "apps/eval-runner/app/worker.py", kind: "Method", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "main", file_path: "apps/eval-runner/app/worker.py", kind: "Function", direction: "upstream", repo: "forge", includeTests: true})
```

Expected: `run_once` affects the worker CLI, API cancellation integration, and worker/smoke tests.

- [ ] **Step 2: RED — specify lease-loss and in-flight cancellation behavior**

Add exact tests:

```python
def test_worker_stops_publication_when_lease_is_reclaimed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys,
) -> None:
    storage = queued_sqlite_storage(tmp_path)
    storage.create_run(make_pending_run("run-1"))
    worker = EvalWorker(storage=storage, forge_command=None, worker_id="worker-a")
    monkeypatch.setattr(
        storage,
        "save_task",
        stale_reclaiming_save_task(storage, replacement_worker="worker-b"),
    )
    result = worker.run_once()
    assert result is not None
    assert result.worker_id == "worker-b"
    assert result.status == RunStatus.RUNNING
    assert "lost lease for run run-1" in capsys.readouterr().err


def test_worker_cancellation_interrupts_running_subprocess(tmp_path: Path) -> None:
    storage = queued_sqlite_storage(tmp_path, tasks=long_forge_task(tmp_path))
    storage.create_run(make_forge_pending_run("run-1"))
    worker = EvalWorker(storage=storage, forge_command=long_process_command(tmp_path))
    thread = threading.Thread(target=worker.run_once)
    thread.start()
    wait_until(lambda: storage.get_run("run-1").status == RunStatus.RUNNING)
    storage.cancel_run("run-1")
    thread.join(timeout=3)
    assert not thread.is_alive()
    run = storage.get_run("run-1")
    assert run is not None
    assert run.status == RunStatus.CANCELLED
    assert run.traces[0].error == "cancelled"


def test_worker_fails_missing_persisted_execution_identity(tmp_path: Path) -> None:
    storage = queued_sqlite_storage(tmp_path)
    storage.create_run(
        make_pending_run("run-1").model_copy(
            update={"provider": None, "model": None, "case_source": None, "max_retries": 0}
        )
    )
    result = EvalWorker(storage=storage, forge_command=None).run_once()
    assert result is not None
    assert result.status == RunStatus.FAILED
    assert result.traces == []
    assert "missing provider" in (result.failure_reason or "")
```

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_worker.py -k "lease_is_reclaimed or interrupts_running_subprocess or missing_persisted_execution_identity" -q
```

Expected RED: worker calls unfenced methods, heartbeat failures are swallowed, and cancellation waits for task completion.

- [ ] **Step 3: GREEN — bind the claimed token to execution, heartbeat, and publication**

Immediately after claim, require non-null `worker_id` and `lease_token`. Build cancellation as:

```python
def cancel_requested() -> bool:
    current = self.storage.get_run(run.run_id)
    return (
        self.should_stop
        or current is None
        or current.status == RunStatus.CANCELLED
        or current.worker_id != self.worker_id
        or current.lease_token != run.lease_token
    )
```

Call `validate_execution_identity(run.provider, run.model, run.case_source)` before creating execution options. Pass `cancel_requested` to `execute_evaluation`. Pass `worker_id=self.worker_id` and `lease_token=run.lease_token` to every `save_task`, heartbeat, complete, fail, and retry call.

When `LeaseLostError` is raised, stop the heartbeat, log `[worker {id}] lost lease for run {run_id}`, return the current stored run, and do not retry or fail it. When cancellation is observed, persist partial attempt evidence only if owner/token still match and preserve `RunStatus.CANCELLED`.

- [ ] **Step 4: GREEN — make heartbeat loss visible and terminal for the attempt**

Replace swallowed heartbeat exceptions with a shared `lease_lost_event`. `_start_background_heartbeat` returns both stop and lost events. On `LeaseLostError`, set the lost event; on transient SQLite errors, log the exception but let the next heartbeat retry until the current lease expires. `cancel_requested` returns true when the lost event is set.

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_worker.py tests/test_api.py::test_api_cancellation_during_task_preserves_cancelled -q
```

Expected GREEN: worker, cancellation, reclaim, retry, and heartbeat tests pass.

- [ ] **Step 5: REFACTOR — keep one finalization helper**

Extract `_build_run_result(run, traces, started_at, status, trust_result, failure_reason, failure_category)` so complete, cancel, retry, and fail branches use identical metrics/timing/trust construction. Run:

```bash
cd apps/eval-runner
uv run ruff check app/worker.py tests/test_worker.py tests/test_api.py tests/test_smoke.py
uv run pytest tests/test_worker.py tests/test_api.py tests/test_smoke.py -q
```

Expected: Ruff and all worker/API/smoke tests pass.

- [ ] **Step 6: Verify change scope and commit**

```bash
git add apps/eval-runner/app/worker.py \
  apps/eval-runner/tests/test_worker.py \
  apps/eval-runner/tests/test_api.py \
  apps/eval-runner/tests/test_smoke.py
git commit -m "feat(eval): cancel and fence queued execution"
```

---

### Task 12: Run Authenticated API And Worker Containers On Shared Local Storage

**Files:**

- Modify: `apps/eval-runner/Dockerfile`
- Modify: `apps/eval-runner/docker-compose.yml`
- Modify: `apps/eval-runner/app/storage.py:775-778`
- Create: `apps/eval-runner/tests/test_docker_contract.py`
- Modify: `apps/eval-runner/tests/test_smoke.py`

**GitNexus impact targets:** `SQLiteStorage._connect`, `build_storage`, `app.worker.main`; Docker files require file-level review because they are not code symbols.

- [ ] **Step 1: Run upstream impact analysis**

```text
impact({target: "_connect", file_path: "apps/eval-runner/app/storage.py", kind: "Method", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "build_storage", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "main", file_path: "apps/eval-runner/app/worker.py", kind: "Function", direction: "upstream", repo: "forge", includeTests: true})
```

Expected: SQLite connection behavior affects all durable API and worker operations.

- [ ] **Step 2: RED — specify Compose services, volumes, auth, and queue mode**

Create `tests/test_docker_contract.py`:

```python
def test_compose_defines_authenticated_api_and_worker_with_shared_storage() -> None:
    compose = Path("docker-compose.yml").read_text(encoding="utf-8")
    assert "forge-eval-api:" in compose
    assert "forge-eval-worker:" in compose
    assert "FORGE_EVAL_RUN_EXECUTION_MODE: queued" in compose
    assert "FORGE_EVAL_API_BIND_HOST: 0.0.0.0" in compose
    assert "FORGE_EVAL_API_TOKEN" in compose
    assert "eval-db:/data" in compose
    assert "eval-artifacts:/artifacts" in compose
    assert "python -m app.worker" in compose


def test_dockerfile_contains_cases_for_harness_checks() -> None:
    dockerfile = Path("Dockerfile").read_text(encoding="utf-8")
    assert "COPY eval_cases ./eval_cases" in dockerfile
```

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_docker_contract.py -q
docker compose config
```

Expected RED: only one API service exists, shared volumes/auth/queued mode are absent, and the image omits eval cases.

- [ ] **Step 3: GREEN — define the exact two-service topology**

Replace `docker-compose.yml` with:

```yaml
services:
  forge-eval-api:
    build: .
    command: ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8000"]
    ports:
      - "127.0.0.1:8000:8000"
    environment: &eval-environment
      FORGE_EVAL_STORAGE_BACKEND: sqlite
      FORGE_EVAL_RUN_EXECUTION_MODE: queued
      FORGE_EVAL_TASKS_PATH: /app/eval_cases
      FORGE_EVAL_DB_PATH: /data/forge_eval.db
      FORGE_EVAL_ARTIFACTS_PATH: /artifacts
      FORGE_EVAL_API_BIND_HOST: 0.0.0.0
      FORGE_EVAL_API_TOKEN: ${FORGE_EVAL_API_TOKEN:?set FORGE_EVAL_API_TOKEN}
    volumes:
      - eval-db:/data
      - eval-artifacts:/artifacts
    healthcheck:
      test: ["CMD", "python", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8000/health', timeout=2)"]
      interval: 5s
      timeout: 3s
      retries: 12
    restart: unless-stopped

  forge-eval-worker:
    build: .
    command: ["python", "-m", "app.worker"]
    environment:
      <<: *eval-environment
      FORGE_EVAL_WORKER_ID: docker-worker
    volumes:
      - eval-db:/data
      - eval-artifacts:/artifacts
    depends_on:
      forge-eval-api:
        condition: service_healthy
    restart: unless-stopped

volumes:
  eval-db:
  eval-artifacts:
```

Add `COPY eval_cases ./eval_cases` to the Dockerfile.

- [ ] **Step 4: GREEN — configure SQLite for API/worker concurrency**

Replace `_connect` with:

```python
def _connect(self) -> sqlite3.Connection:
    connection = sqlite3.connect(self.db_path, timeout=30.0)
    connection.row_factory = sqlite3.Row
    connection.execute("PRAGMA foreign_keys = ON")
    connection.execute("PRAGMA busy_timeout = 30000")
    connection.execute("PRAGMA journal_mode = WAL")
    return connection
```

Document/test that the shared DB volume is a local Docker named volume; arbitrary network filesystem SQLite storage is unsupported.

- [ ] **Step 5: Run GREEN contract and optional live smoke**

```bash
cd apps/eval-runner
uv run pytest tests/test_docker_contract.py tests/test_smoke.py -q
FORGE_EVAL_API_TOKEN=test-secret docker compose config
FORGE_EVAL_API_TOKEN=test-secret docker compose up --build -d
curl --fail -H 'Authorization: Bearer test-secret' http://127.0.0.1:8000/tasks
FORGE_EVAL_API_TOKEN=test-secret docker compose down -v
```

Expected GREEN: static tests pass, Compose renders, both containers become healthy, authenticated task listing succeeds, and cleanup succeeds. If Docker is unavailable, record the exact command error and keep the live check unknown rather than passed.

- [ ] **Step 6: REFACTOR — verify unauthenticated access stays closed**

While the containers are running, run:

```bash
test "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8000/tasks)" = "401"
```

Expected: command exits `0`, proving the returned HTTP status was `401`.

- [ ] **Step 7: Verify change scope and commit**

```bash
git add apps/eval-runner/Dockerfile \
  apps/eval-runner/docker-compose.yml \
  apps/eval-runner/app/storage.py \
  apps/eval-runner/tests/test_docker_contract.py \
  apps/eval-runner/tests/test_smoke.py
git commit -m "feat(eval): run authenticated API and worker containers"
```

---

### Task 13: Freeze Eval Quality Commands, Acceptance Labels, And Operator Documentation

**Files:**

- Modify: `apps/eval-runner/pyproject.toml`
- Modify: `apps/eval-runner/uv.lock`
- Modify: `scripts/acceptance.sh`
- Modify: `scripts/acceptance.test.mjs`
- Modify: `apps/eval-runner/README.md`
- Modify: `apps/eval-runner/docs/ops.md`
- Modify: `apps/eval-runner/docs/architecture.md`
- Modify: `README.md`
- Modify: `CHANGELOG.md`

**GitNexus impact targets:** file-level quality/acceptance/docs changes plus `Settings`, `EvaluationRun`, `TrustGateResult`, and `EvalWorker.run_once` for documentation accuracy.

- [ ] **Step 1: Run final documentation-contract impacts**

```text
impact({target: "Settings", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "EvaluationRun", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "TrustGateResult", direction: "upstream", repo: "forge", includeTests: true})
impact({target: "run_once", file_path: "apps/eval-runner/app/worker.py", kind: "Method", direction: "upstream", repo: "forge", includeTests: true})
```

Expected: documentation describes the final contracts already implemented; no new runtime authority is introduced.

- [ ] **Step 2: RED — lock the exact quality and acceptance contracts in tests**

Add assertions to `scripts/acceptance.test.mjs` for these labels exactly:

```javascript
for (const label of [
  "eval execution identity baseline",
  "eval independent workspace evidence baseline",
  "eval trusted execution baseline",
  "eval authenticated fenced worker baseline",
]) {
  assert.match(acceptance, new RegExp(`add_gate '${label}'`));
}
```

Run:

```bash
cd /Users/cabbos/project/forge
node --test scripts/acceptance.test.mjs
cd apps/eval-runner
uv run mypy app
```

Expected RED: acceptance labels are absent and mypy is not installed/configured in the Eval project.

- [ ] **Step 3: GREEN — add mypy dependency and checked configuration**

Run:

```bash
cd apps/eval-runner
uv add --dev 'mypy>=1.16.0'
```

This updates only `pyproject.toml` and `uv.lock`. Add:

```toml
[tool.mypy]
python_version = "3.11"
plugins = ["pydantic.mypy"]
check_untyped_defs = true
disallow_incomplete_defs = true
no_implicit_optional = true
warn_redundant_casts = true
warn_return_any = true
warn_unused_configs = true
show_error_codes = true
```

Annotate production functions reported by mypy rather than suppressing modules. Use `Any` only at JSON/external-process boundaries already represented by `dict[str, Any]`. Do not add global `ignore_missing_imports` or per-module blanket ignores.

Run the fixed commands:

```bash
cd apps/eval-runner
uv run ruff check .
uv run ruff format --check .
uv run mypy app
```

Expected GREEN: all three commands exit `0`.

- [ ] **Step 4: GREEN — add the four fixed acceptance gates**

Add these exact gates to `scripts/acceptance.sh`:

```bash
add_gate 'eval execution identity baseline' 'cd apps/eval-runner && uv run pytest tests/test_storage.py tests/test_runner.py tests/test_api.py -k "execution_identity or unknown_provider or queued_sqlite_forge" -q'
add_gate 'eval independent workspace evidence baseline' 'cd apps/eval-runner && uv run pytest tests/test_process_control.py tests/test_workspace_observer.py tests/test_runner.py -k "workspace or scope or timeout or cancellation or sandbox or patch" -q'
add_gate 'eval trusted execution baseline' 'cd apps/eval-runner && uv run pytest tests/test_execution.py tests/test_reporting.py tests/test_cli.py -q'
add_gate 'eval authenticated fenced worker baseline' 'cd apps/eval-runner && uv run pytest tests/test_api.py tests/test_storage.py tests/test_worker.py tests/test_docker_contract.py -k "auth or token or lease or stale or fence or docker" -q'
```

Keep the labels byte-for-byte stable because `release/release-gates.v1.json` in subproject C references them.

Run:

```bash
cd /Users/cabbos/project/forge
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run | rg -F \
  -e 'eval execution identity baseline' \
  -e 'eval independent workspace evidence baseline' \
  -e 'eval trusted execution baseline' \
  -e 'eval authenticated fenced worker baseline'
```

Expected GREEN: acceptance contract tests pass and the dry-run output contains all four labels.

- [ ] **Step 5: GREEN — synchronize operator and user documentation**

Document these exact facts:

- `apps/eval-runner/README.md`: valid providers; no fallback; trust result versus execution status; required-score coverage; CLI exit `0/1/2`; `--report-only`; bearer header; queued API/worker commands; timeout/cancellation categories; lease fencing; Docker token and volumes.
- `apps/eval-runner/docs/ops.md`: non-loopback startup refusal without token; token rotation; stale-lease diagnosis; attempt artifact paths; local-volume SQLite constraint; authenticated curl examples; worker shutdown/cancellation behavior.
- `apps/eval-runner/docs/architecture.md`: persisted identity authority, independent snapshot authority, common execution flow, trust-state machine, score denominators, process-group boundary, and fenced publication.
- Root `README.md`: the four release-required Eval acceptance labels and the fixed Eval quality commands.
- `CHANGELOG.md`: public-beta Eval trust hardening, including the intentional behavior change that default gating CLI runs now exit nonzero for unknown/untrusted evidence.

Use this authenticated request form consistently:

```bash
curl -H "Authorization: Bearer $FORGE_EVAL_API_TOKEN" http://127.0.0.1:8000/tasks
```

Use these fixed quality commands consistently:

```bash
uv run ruff check .
uv run ruff format --check .
uv run mypy app
```

- [ ] **Step 6: REFACTOR — detect stale or contradictory documentation**

Run:

```bash
cd /Users/cabbos/project/forge
rg -n "unknown.*mock|defaults? to.*mock|unauthenticated|trust_status|score_coverage|lease_token|report-only" \
  README.md CHANGELOG.md apps/eval-runner/README.md apps/eval-runner/docs
scripts/acceptance.sh --dry-run
```

Expected: no documentation says an unknown provider falls back to mock or that exposed service mode is unauthenticated; documented field/flag names match the stable naming contract; dry-run exits `0`.

- [ ] **Step 7: Run the complete Eval and repository contract gates**

```bash
cd /Users/cabbos/project/forge/apps/eval-runner
uv sync --frozen
uv run pytest -q
uv run ruff check .
uv run ruff format --check .
uv run mypy app
FORGE_EVAL_API_TOKEN=test-secret docker compose config
cd /Users/cabbos/project/forge
npm run test:eval
node --test scripts/acceptance.test.mjs
scripts/acceptance.sh --dry-run
```

Expected GREEN: the complete Eval suite, Ruff, format check, mypy, Compose render, npm Eval gate, acceptance contract tests, and acceptance dry-run all exit `0`.

- [ ] **Step 8: Verify final change scope and commit**

```bash
git add apps/eval-runner/pyproject.toml \
  apps/eval-runner/uv.lock \
  apps/eval-runner/README.md \
  apps/eval-runner/docs/ops.md \
  apps/eval-runner/docs/architecture.md \
  scripts/acceptance.sh \
  scripts/acceptance.test.mjs \
  README.md \
  CHANGELOG.md
git commit -m "docs(eval): publish trustworthy release gates"
```

---

## Final Verification Before Completion

- [ ] Run `detect_changes({scope: "compare", base_ref: "main", repo: "forge"})` and inspect every affected symbol/process. Expected: only Eval execution identity, workspace/process evidence, trust/reporting, API auth, worker fencing, Docker service mode, quality configuration, and the four acceptance gates are affected.
- [ ] Run `git status --short` and confirm no generated database, artifact directory, `.env`, coverage file, or Docker residue is tracked.
- [ ] Run `git log --oneline --reverse f5863df1e6fcbde55a9b4b2ceeacd9e3c354d3c3..HEAD` and confirm every task above has one intentional commit in dependency order.
- [ ] Run the Task 13 complete gate block once more from a clean process environment. Do not claim R2 complete from focused tests alone.
- [ ] Attach the final test output, four acceptance labels, GitNexus impact report or fallback report, and residual risks to the same commit used by subproject C's release evidence.

Completion requires all of the following observable outcomes:

```text
Unknown provider -> rejected
Queued Forge identity -> restored as Forge or explicitly failed as Forge
Falsified changed_files -> independent scope result wins
Missing required evidence -> trust unknown/untrusted, never trusted
Unbounded/cancel-insensitive subprocess -> none remain
Expired/reclaimed lease -> stale write and completion rejected
Non-loopback API without token -> startup rejected
Wrong or missing bearer token -> HTTP 401
Docker API and worker -> shared local SQLite/artifact volumes
CLI gating run -> exit 0 only for trusted threshold-passing evidence
```

## Plan Self-Review Checklist

- [ ] **Spec coverage:** map every R2 requirement and every Eval verification bullet in the convergence design to Tasks 2, 5, 7, 8, 9, 10, 11, and 12.
- [ ] **Placeholder scan:** run the command below and resolve every match that indicates incomplete work; valid occurrences inside quoted diagnostic strings must be reviewed manually.

```bash
pattern='TB''D|TO''DO|implement'' later|fill'' in|add'' appropriate|write'' tests for the above|similar'' to Task'
rg -n "$pattern" docs/superpowers/plans/2026-07-10-eval-trustworthiness-baseline.md
```

Expected: no matches.

- [ ] **Type/name consistency:** verify every occurrence uses the stable names `EvalProvider`, `TrustStatus`, `TrustGateResult`, `WorkspaceObservation`, `ScoreCoverage`, `ProcessOutcome`, `BoundedProcessResult`, `LeaseLostError`, `ExecutionOptions`, `EvaluationExecution`, and `execute_evaluation`.
- [ ] **Acceptance-label consistency:** verify the four fixed labels appear exactly and identically in the file responsibility map, Task 13, and the eventual `scripts/acceptance.sh` implementation.
- [ ] **Command consistency:** verify every quality block uses exactly `uv run ruff check .`, `uv run ruff format --check .`, and `uv run mypy app`.

## Execution Handoff

Execute this plan with `superpowers:subagent-driven-development` one task at a time. For each task: run impact analysis, write and observe the RED failure, implement only that task's GREEN contract, run the REFACTOR checks, run `detect_changes`, review the diff, and create the listed commit before starting the next task.
