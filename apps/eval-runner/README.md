# Forge Eval Runner

CLI runs share the fail-closed trust orchestrator. Exit code `0` requires trusted evidence and passing thresholds, `1` indicates trust/threshold failure, and argparse/input errors use `2`; `--report-only` preserves report generation while ignoring trust blockers.
Reports include `score_coverage` entries with `observed`, `expected`, and `coverage` for every universal, emitted, or case-required score; unknown required score names fail case-quality validation.
Forge traces also carry a bounded `harness_contract_evidence` event: a minimal context manifest (hashes and byte/token estimates only), typed/truncation-aware tool-result envelopes, typed progress/stagnation evidence, and an explicit primary failure taxonomy. The `semantic_progress_v1` eval profile is wired through the task payload into Rust's isolated loop guard, which stops after two consecutive failed tool batches; ordinary desktop triggers retain the six-batch default. The controller contract still permits one evidence-driven redirect when typed progress signals are supplied, while missing signals remain explicit rather than being inferred as progress, and generic `runner_error` is never the terminal taxonomy emitted by the contract layer.
Baseline reports aggregate that evidence into tool-call histograms/repeat rates, context byte/token percentiles and tier totals, stagnation/repair compliance, and deterministic paired-bootstrap intervals for comparison deltas. Comparison artifacts also carry deterministic SHA-256 fingerprints over each arm's canonical campaign metadata, locks, budget, index, report, and 30 attempt artifacts, making the compared evidence set replayable and tamper-evident. Provider-error aggregates use bounded typed codes rather than raw provider response text, so comparison evidence stays secret-free and bounded. Comparison winner selection is quality-gated by those intervals; uncertain deltas stay `no_material_difference`.
Baseline report failure-category histograms use the same controller-owned typed taxonomy as comparisons, with legacy artifact categories retained only as the compatibility fallback; a published report and its comparison therefore cannot disagree merely because one path ignored `harness_contract_evidence`.
Measured eval runners can opt into the fingerprinted `HarnessPolicyV1` minimal prompt plus task-local context mode without changing desktop defaults; the current product policy remains the control.
Forge-provider eval prompts also render each task's controller-owned allowed and forbidden file scope and a bounded targeted-validation stop instruction before the first edit. The independent workspace snapshot and patch replay remain authoritative, so this is an early convergence guard rather than a weaker scope check.
When a Forge headless task carries that scope contract, the controller also copies it into an eval-only Rust write policy. File edits outside the exact allowlist, file deletes, and mutating shell confirmations are denied before execution and recorded as typed `eval_write_policy_block` evidence. Ordinary desktop sessions and headless requests without the explicit policy keep their existing permission behavior; the final filesystem snapshot and clean-fixture replay remain the authoritative trust gates.
The same controller contract now gives every Forge eval a twelve-round convergence target and forbids repeating an unchanged read-only command; this is advisory guidance for the model, while the configured hard round cap and terminal evidence remain authoritative.
The scope guard also names the observed helper-file traps (`.cache-*`, repo-local `tools/*`, and generated lockfiles) and requires cleanup before final verification; independent snapshots still decide whether the attempt is trusted.
The isolated Forge child now prepends a runtime-local `uv` shim that injects `--no-project` for unqualified `uv run` exploration, preventing accidental fixture lockfiles while preserving explicit project selection and recording the boundary in trace evidence.
The first real 30-attempt before/after diagnostic is recorded in [`docs/forgebench-h1-convergence-diagnostic.md`](../../docs/forgebench-h1-convergence-diagnostic.md): typical-loop metrics improved, but scope violations and p95 rounds regressed, so the guard is not promoted as a frontier claim.
The follow-up scope-firewall sequence is recorded in [`docs/forgebench-h1-scope-firewall-diagnostic.md`](../../docs/forgebench-h1-scope-firewall-diagnostic.md): the runtime `uv` shim plus exact allowlist hard boundary reached 30/30 trusted, verified, scope-clean development attempts, while the locked median-round gate remains open.
Each Forge task also receives a bounded `TaskExecutionContract` describing its shape (no-op, single-file, multi-file, or shell-heavy), required inspection paths, targeted checks, and stop condition. Adapters may return an eight-step maximum `task_plan`; the contract validates inspection coverage, in-scope edits, and an explicit verification phase, while missing plans stay visible as missing evidence.
The same raw event records advisory observed phase coverage from tool commands and controller verification; it never replaces filesystem snapshots or clean-fixture patch replay.
The observer's disposable-cache allowlist is explicit and narrow (`.bun-cache`, `.uv-cache`); an arm that fails patch replay remains terminal and is never replayed into a comparison.
The checked-in minimal policy profiles are `deepseek-v4-flash-anthropic-minimal-v1` and `deepseek-v4-flash-openai-chat-minimal-v1`; both are canonical, route-bound, carry explicit official base URLs, and keep secrets out of profile bytes. Only the original H1 Anthropic profile retains an omitted `base_url` for byte compatibility; route-bound baseline verification also requires a matching controller-owned policy event.
The plan-first development profiles are `deepseek-v4-flash-anthropic-planning-v1` and `deepseek-v4-flash-openai-chat-planning-v1`; they are separate fingerprints for A/B measurement and are not holdout profiles.
Baseline comparison supports the default `transport_ab_v1` route experiment and the fail-closed `policy_ab_v1` same-route minimal-versus-planning experiment; neither mode promotes a profile without complete trusted evidence. A canonical comparison must bind both profile IDs/fingerprints and the source commit, contain exactly 30 paired outcome records (10 per repetition), use the typed success/failure outcome vocabulary, and publish all five deterministic bootstrap intervals with estimates matching the aggregate metrics; incomplete hand-authored aggregate evidence is rejected before holdout initialization.
Forge-provider traces use independent filesystem snapshots as the changed-file authority. Setup, validation, regression, and post-validation commands run outside the observation window but now inherit the same isolated runtime environment and `uv` scope wrapper as the headless agent; subprocesses are bounded/cancellable, future repository state is scrubbed, and emitted diffs are replayed in a fresh fixture workspace.
The eval doctor precheck runs with a disposable home and never reads desktop session snapshots. Every real Forge attempt owns sibling `workspace` and `runtime-state` roots: `HOME`, `USERPROFILE`, registry, Continuity, logs, profiles, and snapshots are routed outside the observed product workspace, while `CARGO_HOME` and `RUSTUP_HOME` continue to resolve the installed toolchains. A golden harness is trusted only after a real observer change and clean-fixture patch replay both succeed.

ForgeBench is the canonical executable real-coding dataset: 10 development cases and 20 sealed holdout cases share one versioned product monorepo, run in independent copies with scenario overlays, and are frozen by content fingerprints for the fixture, case splits, and opaque hidden verifiers. This H0B state proves dataset integrity and polarity, not paid holdout success or a frontier KPI.

The offline H3 contract evidence is documented in [`docs/forgebench-h3-harness-contracts.md`](../../docs/forgebench-h3-harness-contracts.md). It is a harness-readiness gate, not a paid model-quality claim.
The H4 release decision boundary is implemented as a canonical 60-attempt holdout report and verifier in [`docs/forgebench-h4-holdout-gate.md`](../../docs/forgebench-h4-holdout-gate.md); reports are derived from unique case/repetition evidence records, and the offline gate validates the contract without fabricating a holdout result.
The final claim boundary is also executable: `python -m app.cli release verify --holdout-campaign <sealed-h4-campaign> --campaign <forge-campaign>` re-verifies the sealed H4 attempts, budget chain, trust evidence, artifact index, and canonical report before combining it with matching profile/dataset fingerprints, a verified Forge provenance campaign, and its clean full-monorepo verification. A standalone hand-authored aggregate report cannot satisfy the gate; it never starts a provider call and fails closed when either track is absent.
The integration campaign acceptance path applies the same route binding: when a locked profile declares an endpoint family or base URL, the submitted `eval_execution_identity` must match both fields before a Forge patch can enter the provenance ledger.
The sealed campaign runner adds the operational boundary around that report: `holdout init` requires a canonical comparison plus explicit control/candidate campaign and profile paths, separately pinned development cases/lock inputs, then independently recomputes the comparison before unlocking the holdout; the locked comparison source commit is bound to the holdout source commit and rechecked on every verification; `holdout run` persists a pending record before each provider call; `holdout recover` closes interrupted work as unknown-cost evidence without replay; and `holdout report` publishes a canonical report plus artifact index only after all 60 calls are terminal. Campaign state is intended to live outside Git so raw traces and credentials never enter the monorepo; the code, tests, locks, and acceptance gate remain in this monorepo.
The first real H4 control campaign closed all 60 attempts with known usage and complete evidence coverage, but failed the locked success, scope, and round thresholds. Its diagnostic metrics and failure concentration are recorded in [`docs/forgebench-h4-paid-result.md`](../../docs/forgebench-h4-paid-result.md); no threshold was relaxed and no frontier claim is made.
For post-publication artifact checks, `holdout verify --campaign ... --frozen` keeps the sealed profile, dataset, attempt, usage, trust, budget-chain, report, artifact-index, and threshold checks while intentionally skipping only the current worktree commit/digest comparison; it never starts a provider call.
Development arms have the same publication boundary: `baseline verify --campaign ... --frozen` rechecks a sealed arm after documentation or release commits without replaying or starting a provider call; `baseline report --campaign ... --frozen` can publish its canonical report and artifact index under that same frozen-source rule.
The repository-wide evidence index is [`docs/forge-monorepo-evidence.md`](../../docs/forge-monorepo-evidence.md); it links the desktop, eval-runner, website, acceptance matrix, provenance campaign, and the explicit H0–H4 claim boundaries.

Development-only provider chains are documented in [`docs/forgebench-provider-fallback.md`](../../docs/forgebench-provider-fallback.md). Run the explicit `--provider-chain kimi-k3-deepseek` option only for ordinary development experiments; it keeps Kimi/DeepSeek route identity and credentials separate from the locked DeepSeek comparison and holdout profiles, with a dedicated acceptance contract. The latest real 10-case Kimi diagnostic is [`docs/forgebench-kimi-development-run-2026-08-10.md`](../../docs/forgebench-kimi-development-run-2026-08-10.md); it is development evidence, not a holdout or frontier claim.

The route-pure Kimi/DeepSeek paired diagnostic is documented in [`docs/forgebench-provider-ab.md`](../../docs/forgebench-provider-ab.md). It runs the two approved routes as separate arms, rejects fallback contamination and unknown cost, pairs `(repetition, task_id)`, and publishes deterministic bootstrap intervals. It is diagnostic-only and cannot unlock the sealed holdout.

Forge Eval Runner is a Python MVP for evaluating coding agents. It focuses on the parts hiring teams usually care about for AI Product Engineer, LLM Application Engineer, and Agent Engineer roles: trace capture, task-level metrics, failure analysis, typed API contracts, tests, and containerized delivery.

The default version still uses a deterministic mock runner for reproducible tests. It now also has a `provider=forge` runner seam for connecting a real Forge headless command without changing the API shape. Both paths produce the same trace contract: prompts, context files, raw events, tool calls, shell output, file diffs, changed files, scope violations, final answer, verification result, timing, and failure reason.

## Why This Project Exists

Agent products are hard to debug when they only expose a final answer. A useful eval platform needs to answer:

- What did the agent see?
- Which tools did it call?
- What shell commands ran, and what did they return?
- Which files changed?
- Did verification run?
- Why did a task fail?
- How does one run compare with another at the metrics level?

Forge Eval Runner is a portfolio-oriented implementation of that workflow. It demonstrates how to turn an agent execution into structured data that can power dashboards, trace replay, regression analysis, and model/provider comparison.

## Architecture

```text
tasks/sample_tasks.json
        |
        v
InMemoryStorage ----> FastAPI routes
        |                  |
        v                  v
RunnerFactory -> DeterministicMockRunner / ForgeAgentRunner -> AgentTrace[]
        |                  |
        v                  v
VerificationResult    calculate_metrics()
```

## Project Structure

```text
forge-eval-runner/
  app/
    cases.py      JSON eval case loader for files or directories
    cli.py        Minimal backtest CLI for local/offline runs
    datasets.py   Stable dataset fingerprints for loaded case sets
    experiments.py  Immutable experiment artifact metadata
    trust_gates.py  Fail-closed run trust scorecard helpers
    scoring.py    Layered code, scope, red-team, and split-validation scorers
    judge_calibration.py  Golden-label calibration for report-only judges
    prompt_mutation.py  Deterministic developer-style prompt variants
    red_team.py   Red-team case classification helpers
    sandbox.py    Workspace cleanliness and future-state leakage checks
    patches.py    Patch replay and normalized patch helpers
    harness_checks.py  Golden harness self-checks
    harness_contracts.py  Bounded context/tool/progress evidence contracts
    agent_adapter.py  Stable external-agent trace adapter contract
    trace_import.py  Production trace to eval case promotion
    main.py       FastAPI app and route handlers
    models.py     Pydantic task, trace, run, and metrics schemas
    runner.py     Deterministic mock coding-agent runner
    trace.py      Trace timestamp and mock diff helpers
    metrics.py    Success, coverage, duration, and failure aggregation
    reporting.py  Backtest report aggregation and per-task summaries
    report_compare.py  Report-to-report regression helper
    storage.py    Memory and SQLite task/run/artifact storage boundary
    config.py     Environment-driven settings
  eval_cases/
    */case.json   Runtime, scorer, red-team, and adapter contract cases
  forgebench/
    forgebench-monorepo/  Canonical TypeScript/Python product fixture
    cases/development/    10 visible real-coding development cases
    cases/holdout/        20 sealed real-coding holdout cases
    verifiers/            Controller-owned opaque hidden checks
    dataset-lock.json     Content-addressed split and verifier lock
  tasks/
    sample_tasks.json
  tests/
    test_api.py
    test_metrics.py
    test_runner.py
```

## API

- `GET /health`
- `GET /tasks`
- `POST /runs`
- `GET /runs`
- `GET /runs/{run_id}`
- `GET /runs/{run_id}/trace`
- `GET /runs/{run_id}/metrics`
- `GET /runs/{run_id}/report`
- `GET /runs/{run_id}/artifacts`
- `POST /runs/{run_id}/cancel`
- `GET /queue/status`

`GET /health` is always unauthenticated. When `FORGE_EVAL_API_TOKEN` is set,
every product endpoint above requires `Authorization: Bearer <token>`. The
service refuses to start when `FORGE_EVAL_API_BIND_HOST` is non-loopback and no
token is configured; keep the application setting aligned with the host passed
to Uvicorn.

## Public-Beta Trust Contract

- `mock` and `forge` are the supported providers. Unknown providers are rejected;
  there is no silent fallback to the deterministic mock runner.
- `execution_status` records whether work completed, failed, timed out, or was
  cancelled. `trust_status` independently records whether the resulting evidence
  is decision-worthy. A completed run can still be untrusted.
- Reports publish score means separately from `score_coverage`. Universal and
  case-required scores keep their full expected denominator, so missing evidence
  cannot look like complete coverage.
- CLI release runs exit `0` only when execution is trusted and thresholds pass,
  exit `1` for trust or threshold failures, and exit `2` for argument/input
  errors. Use `--report-only` to inspect incomplete evidence without treating it
  as a passing release gate.
- API sync mode, queued workers, and the CLI share the same execution and trust
  orchestrator. Forge runs observe workspace changes independently, bound and
  cancel subprocess groups, scrub future repository state, and replay emitted
  patches in a fresh fixture.
- Every queued claim receives a rotating lease token. Heartbeats, task writes,
  retries, and terminal publication are fenced by worker id plus that token.
  Stale attempts cannot overwrite the active attempt. Cancelled partial evidence
  is retained under `artifacts/{run_id}/attempts/{lease_token}/`; only the active
  terminal attempt may publish canonical `trace.json` and `report.json`.

## Run Locally

```bash
cd apps/eval-runner
uv sync
uv run uvicorn app.main:app --reload --port 8000
```

Open:

- API docs: `http://localhost:8000/docs`
- Health check: `http://localhost:8000/health`

## SQLite Persistence

The API still defaults to in-memory storage for local smoke tests. V0.2 adds a SQLite-backed storage option for durable run, task, and artifact metadata:

```bash
FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run uvicorn app.main:app --reload --port 8000
```

SQLite creates three metadata tables:

- `eval_runs`: run status, requested task ids, metrics summary, report rates, failure category counts.
- `eval_run_tasks`: per-task pass/fail summary, duration, model rounds, confirm requests, changed files, scope violations.
- `eval_artifacts`: artifact kind, path, size, and created timestamp.

Large trace/report payloads are written to filesystem artifacts instead of database rows:

```text
artifacts/
  {run_id}/
    trace.json
    report.json
    attempts/
      {lease_token}/
        trace.json
        report.json
```

This preserves the current synchronous API contract while making completed runs, trace/report artifacts, and run lists queryable after the storage object or process restarts.

## Worker Mode

V0.3 adds a small local worker while keeping synchronous execution as the default. To make `POST /runs` return immediately with a `pending` run, start the API with queued execution:

```bash
FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_RUN_EXECUTION_MODE=queued \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run uvicorn app.main:app --reload --port 8000
```

Run one pending job:

```bash
FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run python -m app.worker --once
```

Or poll continuously:

```bash
FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run python -m app.worker
```

The worker claims the oldest `pending` run, marks it `running`, executes through
the same trust boundary as sync/API runs, heartbeats its lease, writes fenced
per-task summaries, and publishes terminal artifacts. Cancellation and worker
shutdown interrupt in-flight process groups within bounded time. Same-attempt
partial cancellation evidence is retained in the attempt directory, while a
stale or lease-lost worker cannot publish tasks, retry state, or canonical
artifacts. Failed tasks keep bounded stderr previews plus `failure_reason` so
operators can diagnose them without rerunning blindly. `GET /queue/status`
reports counts by run status plus the oldest pending/running run IDs, which is
the first check for stuck workers or stale leases.

## Run Eval Cases With CLI

`eval_cases/` remains the portable Forge runtime and contract regression pack. The default mock provider runs fully offline and is intended to validate case loading, trace shaping, scope checks, and report aggregation:

```bash
uv run python -m app.cli --cases eval_cases --provider mock
```

### ForgeBench Canonical Real-Coding Catalog

All executable real-coding benchmark tasks target the single
`forgebench/forgebench-monorepo` product fixture. Each scoring attempt receives
an independent copy plus its declared scenario overlay. The 10-case development
split and 20-case holdout split are content-fingerprinted; holdout inspection
prints only IDs, counts, and hashes:

```bash
uv run python -m app.cli forgebench verify-catalog
uv run python -m app.cli forgebench show-lock --split development
uv run python -m app.cli forgebench show-lock --split holdout
```

The older `eval_cases/_fixtures/continuity-ts-tooling` tree and standalone
micro-fixtures are retained while their runtime-contract assertions migrate.
They are excluded from frontier scoring and cannot satisfy
`metadata.real_coding=true`.

### Forge-built monorepo provenance

The scoring track and integration track are separate. Scoring attempts remain
immutable benchmark evidence; only an explicitly selected, trusted Forge patch
that passes scope, replay, visible, hidden, and full verification may enter the
integration campaign's product commit chain. The controller-authored canonical
baseline is benchmark scaffolding, not a Forge-authored patch. See
[`../../docs/forgebench-provenance.md`](../../docs/forgebench-provenance.md) for
the `campaign init`, `campaign accept`, `campaign finalize`, and
`campaign verify` workflow, the `FORGE-BUILD-MANIFEST.json` contract, controller
interventions, invalidation rules, and proof limits. H0C made no paid DeepSeek
call and does not claim holdout success.

### DeepSeek H1 paid baseline

The H1 controller pins DeepSeek V4 Flash over the Anthropic-compatible
transport to the frozen 10-case development split for three repetitions. It
records a durable pending marker before every provider call, accounts exact CNY
from canonical provider usage, enforces per-task and campaign caps, never
reissues an interrupted paid attempt, and emits a compact report plus
content-addressed artifact index. Before any pending marker or attempt is
created, `baseline run` sends a typed credential preflight to the exact Forge
headless command and frozen provider/model. The preflight creates no agent
session and starts no provider call; paid benchmark commands resolve the
canonical provider key from the inherited `DEEPSEEK_API_KEY` only and never
consult the optional system credential-store fallback. For local paid runs,
keep the value in this
directory's Git-ignored `.env` with file mode `600`, then explicitly export it
into the controller process (`set -a; source .env; set +a`). The child HOME
remains isolated, Python bytecode and pytest caches are routed into the
out-of-workspace runtime state, and the credential is never serialized into a
request or artifact. This keeps verification-only no-op attempts free of
synthetic cache-file patches. If an interruption occurs after the pending marker
is durable, recovery records only a bounded non-secret interruption stage
(runner, usage accounting, trust evaluation, or artifact publication) in the
pending evidence before reserving the unknown cost; it never replays the
provider call. No attempt or budget entry is published before recovery. All six
commands require
explicit
`--campaign` and `--profile` arguments:

```bash
uv run python -m app.cli baseline init --campaign <path> --campaign-id <id> --profile <profile>
uv run python -m app.cli baseline run --campaign <path> --profile <profile> --max-attempts 1
uv run python -m app.cli baseline status --campaign <path> --profile <profile>
uv run python -m app.cli baseline recover --campaign <path> --profile <profile>
uv run python -m app.cli baseline report --campaign <path> --profile <profile>
uv run python -m app.cli baseline verify --campaign <path> --profile <profile>
```

See
[`../../docs/forgebench-h1-baseline.md`](../../docs/forgebench-h1-baseline.md)
for the frozen official price, 8 CNY allocation, recovery semantics, offline
gate, and proof boundary. This is a pinned development baseline, not a frontier
claim; the sealed holdout remains untouched.

### DeepSeek H2 official transport A/B

H2 pairs that Anthropic H1 control with an OpenAI-compatible Chat Completions
candidate for the same DeepSeek V4 Flash development cases. The only allowed
route tuples are `deepseek` + `deepseek-v4-flash` +
`anthropic_messages` + `https://api.deepseek.com/anthropic`, and
`deepseek` + `deepseek-v4-flash` + `openai_chat_completions` +
`https://api.deepseek.com`. The controller derives a typed expected route and
writes a secret-free profile under the child process's isolated HOME; Rust
attests the effective profile before session creation. This is an eval-only
isolated route override: desktop defaults, user settings, and the shared session
builder remain unchanged.

The candidate campaign is fail-closed. It stops on route identity mismatch,
provider schema drift, unknown usage or cost, untrusted workspace evidence,
patch replay failure, or an interrupted paid call; pending paid calls are never
replayed. H2 does not implement or claim OpenAI Responses for DeepSeek. The
offline contract is:

```bash
scripts/acceptance.sh --only "DeepSeek H2 official transport A/B contract"
```

Every route-bound attempt also carries an additive `eval_execution_identity`
trace event owned by the controller. It binds the exact provider/model/route to
temperature policy, reasoning policy, output limit, the frozen controller-visible
tool-name catalog digest (`tool_names_v1`), controller version, and runtime
versions. The OpenAI candidate's 8,192-token output limit is written into the
isolated provider profile and checked against the Rust route contract, so this
is an effective adapter control rather than a report-only claim. The Python
runner checks that the event matches the request sent to Rust, while
baseline/campaign verification rejects missing or malformed candidate identity
evidence. This leaves the frozen H1 profile JSON bytes and fingerprint
unchanged; legacy H1 artifacts without a candidate route remain verifiable.

The v6 paired comparison is development evidence only; it selected
`no_material_difference` and is not a holdout or frontier-grade result. See
[`../../docs/forgebench-h2-transport-ab.md`](../../docs/forgebench-h2-transport-ab.md).
If a paid arm is interrupted with unknown cost, its bounded recovery evidence is
reported separately and no comparison is emitted; the verified v6 result is
recorded in
[`../../docs/forgebench-h2-paid-diagnostic.md`](../../docs/forgebench-h2-paid-diagnostic.md).

The CLI prints a JSON backtest report:

```json
{
  "total_tasks": 5,
  "success_rate": 0.4,
  "verification_pass_rate": 0.6,
  "scope_violation_rate": 0.2,
  "avg_duration_ms": 564.0,
  "avg_model_rounds": 2.0,
  "avg_confirm_requests": 0.6,
  "failure_categories": {
    "scope_violation": 1,
    "timeout": 1,
    "verification_failed": 1
  },
  "tasks": []
}
```

Use the same command shape for the Forge contract once `FORGE_EVAL_FORGE_AGENT_COMMAND` points to the Forge headless binary:

```bash
FORGE_ROOT=/absolute/path/to/forge \
FORGE_EVAL_FORGE_AGENT_COMMAND="cargo run --manifest-path $FORGE_ROOT/apps/desktop/src-tauri/Cargo.toml --bin forge_eval_agent --quiet" \
  uv run python -m app.cli --cases eval_cases/small-edit-success --provider forge --model local-forge
```

The Forge command reads the eval payload from stdin and writes one trace JSON object to stdout. The current headless default is DeepSeek `deepseek-v4-flash`; override it with `FORGE_HEADLESS_PROVIDER` or `FORGE_HEADLESS_MODEL` when needed.

When debugging a real agent run, add `--output` to keep the full trace artifact
while stdout remains the compact report:

```bash
FORGE_ROOT=/absolute/path/to/forge \
FORGE_EVAL_FORGE_AGENT_COMMAND="cargo run --manifest-path $FORGE_ROOT/apps/desktop/src-tauri/Cargo.toml --bin forge_eval_agent --quiet" \
  uv run python -m app.cli \
    --cases eval_cases/small-edit-success \
    --provider forge \
    --model local-forge \
    --output output/small-edit-success.trace.json
```

The output file contains:

```json
{
  "report": {},
  "traces": []
}
```

For CI gates, add threshold flags. The CLI still prints the report, then exits
with code `1` and writes threshold failures to stderr when any limit is missed:

```bash
uv run python -m app.cli \
  --cases eval_cases \
  --provider mock \
  --min-success-rate 0.75 \
  --max-scope-violation-rate 0.05 \
  --max-avg-model-rounds 25
```

Other useful gates are `--max-total-cost-usd`, `--require-known-cost`, `--red-team-only`,
`--include-red-team`, `--max-red-team-failure-rate`, `--trials`,
`--experiment-name`, `--prompt-mutation`, and `--mutations-only`.

For release checks or notebook analysis, `app.report_compare.compare_reports`
compares two `BacktestReport` payloads and returns critical regressions for
large success-rate drops or scope-violation spikes, plus warnings for sharp
model-round increases.

For PR evidence, `node ../../scripts/release-confidence-summary.mjs --markdown`
can combine acceptance metadata, optional gate results, eval reports, and
boundary evidence. It reads failing eval scores from either `score_summary` or
task-level `scores`, and includes failing task ids in the Markdown summary when
only task-level score evidence is available, reports gate-results execution completeness
and execution reason evidence, writes dashboard artifact output with `--out-dir`,
CI-default gate totals/pass/fail/unknown counts beside the full
matrix, and includes acceptance domain/tier breakdowns. Failed, manual, and unknown gate details
preserve gate detail metadata such as domain, status, tier, runtime cost, and manual-evidence state. Add
`--ci-default-only` when a PR fast-gate report should ignore non-CI gates without
supplied results. `scripts/acceptance.sh --results-json gate-results.json`
produces the gate-results payload consumed by `--gate-results`, including each
executed gate's authority domain, tier, runtime cost, manual-evidence flag, and
CI-default metadata; add `--no-acceptance-matrix` when that self-describing
gate-results file should be the only acceptance source. Boundary evidence can also list `capabilityClaims`; declared capabilities with passing
`evidenceGate` and `evidenceScore` references are listed as verified capability evidence,
while a missing acceptance gate, missing acceptance result, failing
acceptance gate, missing eval score, or failing eval score is reported as
missing capability evidence. Add `--fail-on-attention` when the summary is being
used as a gate and should exit nonzero for `attention_required` or `failed`.

## Forge Runtime Evidence

Desktop headless traces and `forge_session export-eval` emit
`ForgeRunEvidence` V2. The V2 payload includes completion eligibility evidence
alongside prepared context, memory audit, permissions, file effects,
verification, provider usage, recovery, A2A child capsules, memory recall,
gateway, and runtime recovery facts. Eval scoring includes schema identity scoring
for schema/source/session/loop-task evidence, permission decision evidence scoring
for replayable permission ledger records and full-access block boundaries,
verification evidence quality scoring for command/pass/exit/duration
consistency, prepared-turn evidence scoring for prompt/context-source quality,
file effects evidence scoring for changed-file and diff alignment,
tool/shell evidence scoring for replayable tool and shell facts,
usage unknown conflict scoring so unknown token usage keeps an explicit reason
and does not carry invented token or cost values,
development reports also expose `total_input_tokens`, `total_output_tokens`,
`provider_usage_known_rate`, and `provider_cost_known_rate`; a provider such as
Kimi K3 now carries a frozen official price snapshot in the trace and prices
missing cache metadata conservatively as cache-miss input; providers without a
frozen price profile remain explicitly unknown instead of appearing as free runs,
provider usage value validation for negative or malformed token/cost facts,
failure evidence scoring for category/reason alignment,
continuity lessons scoring for formed lesson metadata,
memory recall audit scoring for candidate decision reasons and injected
token/budget evidence,
context budget bucket scoring for explicit
`turn_prepared.context_estimate` buckets, plus a dedicated
completion-eligibility consistency score: `unknown` authority is accepted, while
commit-eligible claims with blockers, nonterminal status, or malformed fact
buckets are flagged. Explicit V1 evidence is still accepted; missing V2-only
fields are represented as `unknown` rather than pass, fail, or zero, and explicit
V1 schema identity is labeled `legacy_v1`.

The gateway eval pack lives in `eval_cases/gateway-*` and runs offline through
the deterministic mock provider. It covers local parity dry-run evidence,
degraded fallback with queued input preservation, and the gated read-only
diagnostics owner slice, patch proposal owner proposal-only eligibility,
direct-write owner blocked-by-default eligibility, lease timeout recovery, and
duplicate input prevention; these cases feed
`forge_gateway_runtime_safety_ok` without enabling live gateway ownership,
provider/tool execution, file writes, or automatic resume.

The A2A eval pack lives in `eval_cases/a2a-child-*` and runs offline through the
same mock provider. It covers completed child capsules with review evidence, a
context capsule contract with capsule ids, child goals, token estimates,
prepared-context references, and no full child transcript, review gate identity
binding across child task, parent task, and session, blocking review gates such
as stale review, wrong parent, and missing evidence, failed child capsules with
recovery actions, and a failure recovery policy that requires human approval,
new attempt/new lease retry evidence, and no auto-executed recovery. It also
checks runtime event file facts for compact child lifecycle lineage, plus
worktree worker facts including worker path, changed files, test report, and
cleanup or preserved-worktree inspection status, feeding
`forge_a2a_child_evidence_complete_ok` without launching live subagents.

The memory eval pack lives in `eval_cases/memory-recall-*` and also runs
offline. It covers correct recall selection, duplicate and over-budget
filtering, memory/continuity dedupe, wrong-project and wrong-profile filtering,
archived/forgotten record filtering, and hidden-body leak prevention. The same
scoring also detects when the same continuity record is injected through both
memory and continuity context sources, and detailed recall candidates must carry
decision reasons plus injected token/budget evidence; these cases feed
`forge_memory_recall_quality_ok` while keeping prepared context sources
body-free.

The runtime recovery eval pack lives in `eval_cases/runtime-recovery-*` and also
runs offline. It covers orphaned run recovery, interrupted shell evidence
export, pending confirmation restart replay, unknown provider usage preservation,
and missing verification recovery requests; these cases feed
`forge_runtime_recovery_quality_ok` without inventing token/cost values or
claiming commit readiness.

## Trusted Backtest Operator Path

Treat an eval run as two decisions: `execution_status` says whether the tasks
ran, while `trust_status` says whether the result is decision-worthy. A completed
run can still be untrusted when golden harness checks fail, the dataset has no
fingerprint, an LLM-as-judge scorer is not calibrated against goldens, or the
red-team lane fails.

For local release confidence:

1. Run golden/mock harness checks and keep uncalibrated judge or semantic scores
   report-only until `judge_calibration` marks them gateable.
2. Run normal cases with a stable dataset fingerprint, an immutable experiment
   snapshot, repeated trials, and threshold gates:

   ```bash
   uv run python -m app.cli \
     --cases eval_cases \
     --provider mock \
     --trials 3 \
     --experiment-name local-regression \
     --output output/local-regression.json \
     --min-success-rate 0.1 \
     --max-scope-violation-rate 0.2 \
     --max-total-cost-usd 1.00
   ```

   For paid provider runs, add `--require-known-cost` so incomplete billing
   evidence fails closed instead of being treated as zero spend.

3. Run prompt-mutation probes separately when you want user-style wording
   coverage without changing the source cases:

   ```bash
   uv run python -m app.cli \
     --cases eval_cases \
     --provider mock \
     --prompt-mutation terse-bug-report \
     --min-success-rate 0.1
   ```

4. Run adversarial cases as their own red-team lane:

   ```bash
   uv run python -m app.cli \
     --cases eval_cases \
     --provider mock \
     --red-team-only \
     --max-red-team-failure-rate 0
   ```

5. Inspect `score_summary` for layered scorers: functional correctness, scope,
   prompt injection, secret leakage, unsafe tool use, future-state leakage,
   `regression_ok`, and `bugfix_ok`.
6. Inspect trace artifacts for `trajectory_path` and `cost_usd`; SQLite-backed
   runs also emit per-task `*.trajectory.json` artifacts beside `trace.json` and
   `report.json`.
7. Compare the new report against the last trusted baseline with
   `app.report_compare.compare_reports`, then promote production failures back
   into offline regression cases with `python -m app.cli promote-trace`.

Executable cases should be verified. Prompt-only or adapter-contract cases can
set `metadata.contract_only: true`; they stay useful for contract coverage but
should not be treated like code-verified PASS_TO_PASS / FAIL_TO_PASS tasks.

## Three Ways to Run

The eval runner supports three execution modes, from fastest/most deterministic to slowest/most realistic:

### a. Mock Offline Backtest

Fully deterministic, no API keys, no network. Validates case loading, trace shaping, scope checks, and report aggregation:

```bash
uv run python -m app.cli --cases eval_cases --provider mock
```

### b. Forge Headless Local Real Backtest

Runs a real Forge agent through the headless `forge_eval_agent` binary. Requires a valid API key and Rust toolchain:

```bash
# From the monorepo root
npm run eval:forge:smoke:dry-run

# Or run the real backtest
npm run eval:forge:smoke
```

The `run-forge-backtest.mjs` script automatically resolves the eval-runner sibling directory and generates the correct `FORGE_EVAL_FORGE_AGENT_COMMAND` pointing to `apps/desktop/src-tauri/Cargo.toml`.

### c. Queued Worker + SQLite Service Mode

Start the FastAPI service with SQLite persistence and queued execution, then poll with the worker:

```bash
FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_RUN_EXECUTION_MODE=queued \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run uvicorn app.main:app --reload --port 8000
```

Run one pending job:

```bash
FORGE_EVAL_STORAGE_BACKEND=sqlite \
FORGE_EVAL_DB_PATH=./forge_eval.db \
FORGE_EVAL_ARTIFACTS_PATH=./artifacts \
uv run python -m app.worker --once
```

## Required Environment Variables

| Variable | Required For | Description |
|---|---|---|
| `FORGE_EVAL_FORGE_AGENT_COMMAND` | `provider=forge` | Command to launch the Forge headless agent. Use an absolute manifest path because the controller runs the command from an isolated temporary workspace. |
| `FORGE_HEADLESS_PROVIDER` | `provider=forge` | LLM provider for the headless agent (e.g. `anthropic`, `openai`, `deepseek`). Defaults to `deepseek`. |
| `FORGE_HEADLESS_MODEL` | `provider=forge` | Model ID for the headless agent. Defaults to `deepseek-v4-flash`. |
| `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `DEEPSEEK_API_KEY` | `provider=forge` | API key inherited by the eval controller. Paid baseline/holdout commands are env-only and never read `~/.forge/config.json` or a system credential store. |
| `FORGE_EVAL_RUNNER_PATH` | Optional | Override the default eval-runner directory. Defaults to the sibling `eval-runner` of `apps/desktop/`. |
| `FORGE_EVAL_API_BIND_HOST` | Exposed API | Declared API bind host. Defaults to `127.0.0.1`; a non-loopback value requires `FORGE_EVAL_API_TOKEN`. |
| `FORGE_EVAL_API_TOKEN` | Authenticated API | Bearer token for every product endpoint except `/health`. Required for non-loopback binds and Docker Compose. |
| `FORGE_EVAL_COMMAND_TIMEOUT_SECONDS` | Worker/Forge run | Maximum external agent command duration. Defaults to `900`. |
| `FORGE_EVAL_SETUP_TIMEOUT_SECONDS` | Worker/Forge run | Maximum setup-command duration. Defaults to `300`. |
| `FORGE_EVAL_VALIDATION_TIMEOUT_SECONDS` | Worker/Forge run | Maximum validation-command duration. Defaults to `300`. |
| `FORGE_EVAL_LEASE_DURATION_SECONDS` | Queued worker | Claim lease duration before another worker may reclaim it. Defaults to `300`. |

## Example Requests

List tasks:

```bash
curl -H "Authorization: Bearer $FORGE_EVAL_API_TOKEN" \
  http://localhost:8000/tasks
```

Create an eval run:

```bash
curl -X POST http://localhost:8000/runs \
  -H "Authorization: Bearer $FORGE_EVAL_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "task_ids": ["python-cli-dry-run", "parser-regression-failure"],
    "provider": "mock",
    "model": "deterministic-agent-v1"
  }'
```

Fetch trace:

```bash
curl -H "Authorization: Bearer $FORGE_EVAL_API_TOKEN" \
  http://localhost:8000/runs/<run_id>/trace
```

List runs:

```bash
curl -H "Authorization: Bearer $FORGE_EVAL_API_TOKEN" \
  http://localhost:8000/runs
```

Fetch metrics:

```bash
curl -H "Authorization: Bearer $FORGE_EVAL_API_TOKEN" \
  http://localhost:8000/runs/<run_id>/metrics
```

Fetch the backtest report:

```bash
curl -H "Authorization: Bearer $FORGE_EVAL_API_TOKEN" \
  http://localhost:8000/runs/<run_id>/report
```

Fetch artifact metadata:

```bash
curl -H "Authorization: Bearer $FORGE_EVAL_API_TOKEN" \
  http://localhost:8000/runs/<run_id>/artifacts
```

## Eval Case Format

Cases are dependency-free JSON files. A directory case uses `eval_cases/<case-id>/case.json` with fixture paths resolved relative to that file:

```json
{
  "schema_version": 1,
  "task": {
    "id": "small-edit-success",
    "title": "Small focused edit succeeds",
    "prompt": "Update src/calculator.py and run validation.",
    "fixture_path": "fixture",
    "context_files": ["src/calculator.py", "tests/test_calculator.py"],
    "validation_commands": ["python -m pytest tests/test_calculator.py"],
    "pass_to_pass_commands": ["python -m pytest tests/test_existing.py"],
    "fail_to_pass_commands": ["python -m pytest tests/test_bugfix.py"],
    "verification_command": "python -m pytest tests/test_calculator.py",
    "expected_files_changed": ["src/calculator.py"],
    "forbidden_files_changed": [".env"],
    "metadata": {
      "mock": {
        "changed_files": ["src/calculator.py"],
        "model_rounds": 2,
        "confirm_requests": 0
      }
    }
  }
}
```

`metadata.mock` is only used by the deterministic mock provider. It lets offline cases simulate changed files, raw events, tool commands, model rounds, confirmation requests, token counts, and failure categories without depending on a live Forge app.

Case quality diagnostics are available through `app.cases.validate_case_quality`.
The loader still accepts lightweight contract cases, but executable cases should
include either `verification_command` or `validation_commands`, expected changed
file assertions, and an existing `fixture_path` when one is declared. Prompt-only
contract cases can set `metadata.contract_only: true` to opt out of executable
quality warnings while still participating in API and trace-contract checks.

## Running Against Forge

Set `FORGE_EVAL_FORGE_AGENT_COMMAND` to the Forge headless binary, then create a run with `provider: "forge"`:

```bash
export FORGE_ROOT=/absolute/path/to/forge
export FORGE_EVAL_FORGE_AGENT_COMMAND="cargo run --manifest-path $FORGE_ROOT/apps/desktop/src-tauri/Cargo.toml --bin forge_eval_agent --quiet"

curl -X POST http://localhost:8000/runs \
  -H "Authorization: Bearer $FORGE_EVAL_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "task_ids": ["python-cli-dry-run"],
    "provider": "forge",
    "model": "local-forge"
  }'
```

The command receives JSON on stdin:

```json
{
  "task": {
    "id": "python-cli-dry-run",
    "prompt": "Add a --dry-run flag...",
    "context_files": ["src/cli.py"],
    "expected_files_changed": ["src/cli.py"],
    "forbidden_files_changed": [".env"]
  },
  "prompt": "Add a --dry-run flag...",
  "provider": "forge",
  "model": "local-forge",
  "workspace_path": "/tmp/forge-eval-.../workspace"
}
```

It should write a JSON object to stdout. The runner maps it into `AgentTrace`:

```json
{
  "raw_events": [{ "event_type": "tool_call_start", "tool_name": "read_file" }],
  "tool_calls": [{ "command": "read_file src/cli.py", "stdout": "loaded", "exit_code": 0 }],
  "shell_outputs": [{ "command": "pytest", "stdout": "1 passed", "exit_code": 0 }],
  "file_diffs": [{ "path": "src/cli.py", "change_type": "modified", "diff": "diff --git ..." }],
  "changed_files": ["src/cli.py"],
  "verification_result": {
    "command": "pytest",
    "passed": true,
    "exit_code": 0
  },
  "final_answer": "Completed.",
  "model_rounds": 2,
  "confirm_requests": 1,
  "input_tokens": 120,
  "output_tokens": 40
}
```

Scope checks are automatic: files in `forbidden_files_changed`, or files outside `expected_files_changed` when that list is provided, mark the trace as `scope_violation` even if verification passed.

For stronger backtests, put independent judge commands in `validation_commands`.
These run inside the disposable workspace after Forge finishes and override any
`verification_result` returned by the external command. Use
`pass_to_pass_commands` for existing behavior that must keep passing and
`fail_to_pass_commands` for bug-focused tests that must now pass. Split
validation failures are surfaced separately as `Regression validation failed`
or `Bug-fix validation failed`, and `score_summary` includes `regression_ok`
and `bugfix_ok`.

## Run With Docker

```bash
cd apps/eval-runner
export FORGE_EVAL_API_TOKEN="replace-with-a-random-secret"
docker compose up --build
```

Compose starts an authenticated queued API plus a worker. They share named
SQLite and artifact volumes, while the host port remains bound to
`127.0.0.1:8000`. Use the Bearer token for product endpoints; `/health` remains
available for health checks. This SQLite deployment is intended for one local
host/volume, not a network filesystem or multi-host database service.

## Test And Lint

```bash
uv run pytest
uv run ruff check .
uv run ruff format --check .
uv run mypy app
```

## Portfolio Value

This project maps directly to real agent-platform work:

- Agent eval platform: task ingestion, run creation, per-task pass/fail, aggregate metrics.
- Trace viewer backend: structured trace schema for tool calls, shell outputs, diffs, final answer, and verification.
- Debug analysis: failure categories and failure reasons are first-class fields, not log text.
- Engineering delivery: FastAPI service, Pydantic contracts, pytest coverage, ruff linting, Dockerfile, and docker-compose.
- Extensibility: the mock runner can later be replaced by a real Codex/OpenAI/DeepSeek-compatible adapter without changing the API shape.

## Next Iterations

- Add a frontend trace viewer with timeline and diff panels.
- Execute verification commands in an isolated container.
- Compare multiple models/providers across the same task set.
- Export run reports as JSON, Markdown, or HTML.

## Portfolio Demo Assets

项目包含可截图、可展示的作品集素材：

| 文件 | 说明 |
|---|---|
| `docs/api-examples.md` | API 用法和 curl 示例，适合截图 |
| `docs/portfolio-demo-guide.md` | 2 分钟演示脚本和面试讲解路径 |
| `docs/architecture.md` | Mermaid 架构图（系统、trace、metrics、Forge 关系） |
| `examples/sample-run-request.json` | 创建 run 的请求体示例 |
| `examples/sample-run-response.json` | 完整 run 响应（含 traces + metrics） |
| `examples/sample-trace-response.json` | trace 响应示例 |
| `examples/sample-metrics-response.json` | metrics 响应示例 |
| `scripts/capture_demo_assets.sh` | 一键刷新所有示例 JSON |

### How to Refresh Sample Assets

```bash
# 1. 启动服务
FORGE_EVAL_API_TOKEN=test-secret uv run uvicorn app.main:app --port 8000

# 2. 运行采集脚本
bash scripts/capture_demo_assets.sh
```

### 截图顺序建议

1. 浏览器访问 `http://localhost:8000/docs` — OpenAPI 文档页
2. 终端 `curl -H "Authorization: Bearer $FORGE_EVAL_API_TOKEN" http://localhost:8000/tasks` — 任务列表
3. 终端用同一 Bearer header `POST /runs` — 创建 run
4. 终端用同一 Bearer header `GET /runs/{id}/metrics` — metrics 汇总
5. 终端用同一 Bearer header `GET /runs/{id}/trace` — trace 详情
