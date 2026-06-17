# Eval Runner Phase 2 Ops Roadmap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Turn Forge Eval Runner from a trusted local backtest harness into an operator-ready release gate with baseline management, readable reports, case lifecycle tracking, flake triage, safer trace promotion, and CI-facing summaries.

**Architecture:** Keep `apps/eval-runner` independently runnable. Add small focused modules around the existing report artifact contract instead of introducing a web app or shared package: a baseline registry stores trusted report pointers, report rendering turns existing JSON into Markdown/HTML, case lifecycle metadata remains inside case JSON `metadata`, flake triage consumes repeated-trial reports, trace promotion v2 adds redaction/dedupe before writing cases, and CI summaries reuse the same comparison output.

**Tech Stack:** Python 3.11, Pydantic v2, SQLite/filesystem artifacts, FastAPI, pytest, uv, existing Node/npm wrapper scripts.

---

## Current Baseline

- The Phase 1 eval-runner roadmap is implemented and verified.
- `npm run test:eval` passes with 139 tests and one existing FastAPI/httpx deprecation warning.
- `uv run python -m app.cli --cases eval_cases --provider mock --min-success-rate 0.1 --max-scope-violation-rate 0.2` exits 0.
- `npm run eval:forge:smoke:dry-run` exits 0.
- Current useful primitives:
  - `BacktestReport` and trace artifacts in `apps/eval-runner/app/models.py`.
  - `compare_reports()` in `apps/eval-runner/app/report_compare.py`.
  - `write_backtest_artifact()` and `promote-trace` in `apps/eval-runner/app/cli.py`.
  - `aggregate_trial_metrics()` in `apps/eval-runner/app/reporting.py`.
  - Case quality checks in `apps/eval-runner/app/cases.py`.
  - Red-team filtering/scoring in `apps/eval-runner/app/red_team.py` and `apps/eval-runner/app/scoring.py`.

## Phase 2 Non-Goals

- Do not build a hosted dashboard or React frontend in this phase.
- Do not extract shared packages.
- Do not change desktop runtime behavior.
- Do not require a real model/API key for Phase 2 tests.
- Do not make LLM-as-judge scores gate CI unless calibration metadata marks them gateable.

## File Structure

- Create `apps/eval-runner/app/artifacts.py`: load and validate saved backtest artifacts from JSON files.
- Create `apps/eval-runner/app/baselines.py`: trusted baseline registry models and filesystem persistence.
- Create `apps/eval-runner/app/report_render.py`: Markdown and static HTML rendering for `BacktestReport` plus comparison summaries.
- Create `apps/eval-runner/app/case_lifecycle.py`: lifecycle status extraction and diagnostics from loaded cases.
- Create `apps/eval-runner/app/flake_triage.py`: repeated-trial classification and quarantine recommendations.
- Modify `apps/eval-runner/app/trace_import.py`: add deterministic redaction and dedupe support for promoted traces.
- Modify `apps/eval-runner/app/cli.py`: add subcommands for baseline, report rendering, case lifecycle diagnostics, flake triage, trace promotion v2, and CI summaries.
- Modify `apps/eval-runner/app/models.py`: add small Pydantic models for baseline, rendered report metadata, lifecycle status, flake triage, and trace promotion decisions.
- Add tests under `apps/eval-runner/tests/` beside each behavior:
  - `test_baselines.py`
  - `test_report_render.py`
  - `test_case_lifecycle.py`
  - `test_flake_triage.py`
  - extend `test_cases.py`, `test_cli.py`, and `test_reporting.py`
- Update docs:
  - `apps/eval-runner/README.md`
  - `apps/eval-runner/docs/ops.md`
  - `apps/eval-runner/docs/architecture.md`
  - `CHANGELOG.md`

---

### Task 0: Artifact Loading Boundary

**Files:**
- Create: `apps/eval-runner/app/artifacts.py`
- Test: `apps/eval-runner/tests/test_reporting.py`

- [x] **Step 1: Write failing tests for artifact report loading**

Add to `apps/eval-runner/tests/test_reporting.py`:

```python
import json


def test_load_report_artifact_accepts_cli_artifact_shape(tmp_path):
    from app.artifacts import load_report_artifact

    artifact = tmp_path / "run.json"
    artifact.write_text(
        json.dumps(
            {
                "report": {
                    "total_tasks": 1,
                    "success_rate": 1.0,
                    "verification_pass_rate": 1.0,
                    "scope_violation_rate": 0.0,
                    "avg_duration_ms": 12.0,
                    "avg_model_rounds": 1.0,
                    "avg_confirm_requests": 0.0,
                    "failure_categories": {},
                    "score_summary": {"functional_correctness": 1.0},
                    "tasks": [],
                },
                "traces": [],
            }
        ),
        encoding="utf-8",
    )

    loaded = load_report_artifact(artifact)

    assert loaded.path == artifact
    assert loaded.report.success_rate == 1.0
    assert loaded.trace_count == 0


def test_load_report_artifact_accepts_raw_report_shape(tmp_path):
    from app.artifacts import load_report_artifact

    artifact = tmp_path / "report.json"
    artifact.write_text(
        json.dumps(
            {
                "total_tasks": 1,
                "success_rate": 0.0,
                "verification_pass_rate": 0.0,
                "scope_violation_rate": 1.0,
                "avg_duration_ms": 40.0,
                "avg_model_rounds": 3.0,
                "avg_confirm_requests": 1.0,
                "failure_categories": {"scope_violation": 1},
                "score_summary": {},
                "tasks": [],
            }
        ),
        encoding="utf-8",
    )

    loaded = load_report_artifact(artifact)

    assert loaded.report.scope_violation_rate == 1.0
    assert loaded.trace_count == 0
```

- [x] **Step 2: Run focused tests and verify failure**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_reporting.py::test_load_report_artifact_accepts_cli_artifact_shape tests/test_reporting.py::test_load_report_artifact_accepts_raw_report_shape -v
```

Expected: FAIL because `app.artifacts` does not exist.

- [x] **Step 3: Add artifact loading models**

Add to `apps/eval-runner/app/models.py`:

```python
class ReportArtifact(EvalModel):
    path: str
    report: BacktestReport
    trace_count: int = Field(default=0, ge=0)
    experiment: dict[str, str] | None = None
```

- [x] **Step 4: Implement artifact loading**

Create `apps/eval-runner/app/artifacts.py`:

```python
import json
from pathlib import Path
from typing import Any

from app.models import BacktestReport, ReportArtifact


class ReportArtifactError(RuntimeError):
    pass


def load_report_artifact(path: Path) -> ReportArtifact:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ReportArtifactError(f"Report artifact must be a JSON object: {path}")

    report_payload: dict[str, Any]
    traces_payload: list[Any] = []
    experiment: dict[str, str] | None = None
    if "report" in payload:
        report_payload = object_value(payload.get("report"), key="report", path=path)
        traces = payload.get("traces", [])
        traces_payload = traces if isinstance(traces, list) else []
        experiment_value = payload.get("experiment")
        if isinstance(experiment_value, dict):
            experiment = {str(key): str(value) for key, value in experiment_value.items()}
    else:
        report_payload = payload

    return ReportArtifact(
        path=str(path),
        report=BacktestReport.model_validate(report_payload),
        trace_count=len(traces_payload),
        experiment=experiment,
    )


def object_value(value: Any, *, key: str, path: Path) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ReportArtifactError(f"{key} must be a JSON object in {path}")
    return value
```

- [x] **Step 5: Run focused tests and verify pass**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_reporting.py::test_load_report_artifact_accepts_cli_artifact_shape tests/test_reporting.py::test_load_report_artifact_accepts_raw_report_shape -v
```

Expected: PASS.

- [x] **Step 6: Commit**

```bash
git add apps/eval-runner/app/artifacts.py apps/eval-runner/app/models.py apps/eval-runner/tests/test_reporting.py
git commit -m "feat(eval): add report artifact loader"
```

---

### Task 1: Trusted Baseline Registry

**Files:**
- Create: `apps/eval-runner/app/baselines.py`
- Modify: `apps/eval-runner/app/models.py`
- Modify: `apps/eval-runner/app/cli.py`
- Test: `apps/eval-runner/tests/test_baselines.py`
- Test: `apps/eval-runner/tests/test_cli.py`
- Docs: `apps/eval-runner/docs/ops.md`

- [x] **Step 1: Write failing tests for baseline promotion and lookup**

Create `apps/eval-runner/tests/test_baselines.py`:

```python
import json


def minimal_artifact(success_rate: float = 1.0) -> dict:
    return {
        "report": {
            "total_tasks": 1,
            "success_rate": success_rate,
            "verification_pass_rate": success_rate,
            "scope_violation_rate": 0.0,
            "avg_duration_ms": 10.0,
            "avg_model_rounds": 1.0,
            "avg_confirm_requests": 0.0,
            "failure_categories": {},
            "score_summary": {"functional_correctness": success_rate},
            "tasks": [],
        },
        "traces": [],
        "experiment": {
            "name": "local-regression",
            "dataset_fingerprint": "dataset-123",
            "provider": "mock",
            "model": "deterministic-agent-v1",
        },
    }


def test_baseline_registry_promotes_latest_trusted_report(tmp_path):
    from app.baselines import BaselineRegistry

    artifact = tmp_path / "run.json"
    artifact.write_text(json.dumps(minimal_artifact()), encoding="utf-8")
    registry = BaselineRegistry(tmp_path / "baselines.json")

    record = registry.promote(
        artifact_path=artifact,
        name="local-regression",
        trusted=True,
        note="green release gate",
    )

    latest = registry.latest(name="local-regression")
    assert latest is not None
    assert latest.artifact_path == str(artifact)
    assert latest.dataset_fingerprint == "dataset-123"
    assert latest.success_rate == 1.0
    assert latest.trusted is True
    assert latest.note == "green release gate"


def test_baseline_registry_ignores_untrusted_for_latest_trusted(tmp_path):
    from app.baselines import BaselineRegistry

    good = tmp_path / "good.json"
    bad = tmp_path / "bad.json"
    good.write_text(json.dumps(minimal_artifact(1.0)), encoding="utf-8")
    bad.write_text(json.dumps(minimal_artifact(0.0)), encoding="utf-8")
    registry = BaselineRegistry(tmp_path / "baselines.json")

    registry.promote(artifact_path=good, name="local-regression", trusted=True)
    registry.promote(artifact_path=bad, name="local-regression", trusted=False)

    latest = registry.latest(name="local-regression")
    assert latest is not None
    assert latest.artifact_path == str(good)
```

- [x] **Step 2: Run focused tests and verify failure**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_baselines.py -v
```

Expected: FAIL because `app.baselines` does not exist.

- [x] **Step 3: Add baseline models**

Add to `apps/eval-runner/app/models.py`:

```python
class BaselineRecord(EvalModel):
    name: str
    artifact_path: str
    promoted_at: datetime
    trusted: bool = True
    dataset_fingerprint: str | None = None
    provider: str | None = None
    model: str | None = None
    success_rate: float = Field(ge=0.0, le=1.0)
    scope_violation_rate: float = Field(ge=0.0, le=1.0)
    note: str | None = None

    @field_serializer("promoted_at")
    def serialize_promoted_at(self, value: datetime) -> str:
        return value.isoformat()


class BaselineRegistryPayload(EvalModel):
    records: list[BaselineRecord] = Field(default_factory=list)
```

- [x] **Step 4: Implement registry persistence**

Create `apps/eval-runner/app/baselines.py`:

```python
import json
from datetime import UTC, datetime
from pathlib import Path

from app.artifacts import load_report_artifact
from app.models import BaselineRecord, BaselineRegistryPayload


class BaselineRegistry:
    def __init__(self, path: Path) -> None:
        self.path = path

    def promote(
        self,
        *,
        artifact_path: Path,
        name: str,
        trusted: bool = True,
        note: str | None = None,
    ) -> BaselineRecord:
        artifact = load_report_artifact(artifact_path)
        experiment = artifact.experiment or {}
        record = BaselineRecord(
            name=name,
            artifact_path=str(artifact_path),
            promoted_at=datetime.now(UTC),
            trusted=trusted,
            dataset_fingerprint=experiment.get("dataset_fingerprint"),
            provider=experiment.get("provider"),
            model=experiment.get("model"),
            success_rate=artifact.report.success_rate,
            scope_violation_rate=artifact.report.scope_violation_rate,
            note=note,
        )
        payload = self.load()
        payload.records.append(record)
        self.save(payload)
        return record

    def latest(self, *, name: str) -> BaselineRecord | None:
        records = [record for record in self.load().records if record.name == name and record.trusted]
        return records[-1] if records else None

    def load(self) -> BaselineRegistryPayload:
        if not self.path.exists():
            return BaselineRegistryPayload()
        return BaselineRegistryPayload.model_validate_json(self.path.read_text(encoding="utf-8"))

    def save(self, payload: BaselineRegistryPayload) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(
            json.dumps(payload.model_dump(mode="json"), indent=2),
            encoding="utf-8",
        )
```

- [x] **Step 5: Add CLI subcommands**

Modify the top of `main()` in `apps/eval-runner/app/cli.py`:

```python
    if raw_argv[:1] == ["baseline"]:
        return baseline_main(raw_argv[1:])
```

Add:

```python
def baseline_main(argv: list[str]) -> int:
    from app.baselines import BaselineRegistry

    parser = argparse.ArgumentParser(description="Manage trusted eval baselines.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    promote = subparsers.add_parser("promote")
    promote.add_argument("--registry", type=Path, required=True)
    promote.add_argument("--artifact", type=Path, required=True)
    promote.add_argument("--name", required=True)
    promote.add_argument("--note", default=None)
    promote.add_argument("--untrusted", action="store_true")

    latest = subparsers.add_parser("latest")
    latest.add_argument("--registry", type=Path, required=True)
    latest.add_argument("--name", required=True)

    args = parser.parse_args(argv)
    registry = BaselineRegistry(args.registry)
    if args.command == "promote":
        record = registry.promote(
            artifact_path=args.artifact,
            name=args.name,
            trusted=not args.untrusted,
            note=args.note,
        )
        print(record.model_dump_json(indent=2))
        return 0
    if args.command == "latest":
        record = registry.latest(name=args.name)
        if record is None:
            print("error: no trusted baseline found", file=sys.stderr)
            return 1
        print(record.model_dump_json(indent=2))
        return 0
    return 2
```

- [x] **Step 6: Add CLI tests**

Add to `apps/eval-runner/tests/test_cli.py`:

```python
def test_cli_promotes_and_reads_latest_baseline(tmp_path, capsys):
    import json
    from app.cli import main

    artifact = tmp_path / "run.json"
    artifact.write_text(
        json.dumps(
            {
                "report": {
                    "total_tasks": 1,
                    "success_rate": 1.0,
                    "verification_pass_rate": 1.0,
                    "scope_violation_rate": 0.0,
                    "avg_duration_ms": 10.0,
                    "avg_model_rounds": 1.0,
                    "avg_confirm_requests": 0.0,
                    "failure_categories": {},
                    "score_summary": {},
                    "tasks": [],
                },
                "traces": [],
            }
        ),
        encoding="utf-8",
    )
    registry = tmp_path / "baselines.json"

    assert main(["baseline", "promote", "--registry", str(registry), "--artifact", str(artifact), "--name", "dev"]) == 0
    assert main(["baseline", "latest", "--registry", str(registry), "--name", "dev"]) == 0

    output = capsys.readouterr().out
    assert '"name": "dev"' in output
```

- [x] **Step 7: Run tests**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_baselines.py tests/test_cli.py -v
```

Expected: PASS.

- [x] **Step 8: Commit**

```bash
git add apps/eval-runner/app/baselines.py apps/eval-runner/app/models.py apps/eval-runner/app/cli.py apps/eval-runner/tests/test_baselines.py apps/eval-runner/tests/test_cli.py apps/eval-runner/docs/ops.md
git commit -m "feat(eval): add trusted baseline registry"
```

---

### Task 2: Baseline Comparison Gate

**Files:**
- Modify: `apps/eval-runner/app/report_compare.py`
- Modify: `apps/eval-runner/app/cli.py`
- Test: `apps/eval-runner/tests/test_reporting.py`
- Test: `apps/eval-runner/tests/test_cli.py`
- Docs: `apps/eval-runner/README.md`

- [x] **Step 1: Write failing test for baseline comparison**

Add to `apps/eval-runner/tests/test_reporting.py`:

```python
def test_compare_reports_flags_score_summary_drop():
    from app.models import BacktestReport
    from app.report_compare import compare_reports

    previous = BacktestReport(
        total_tasks=1,
        success_rate=1.0,
        verification_pass_rate=1.0,
        scope_violation_rate=0.0,
        avg_duration_ms=10.0,
        avg_model_rounds=1.0,
        avg_confirm_requests=0.0,
        failure_categories={},
        score_summary={"regression_ok": 1.0},
        tasks=[],
    )
    current = previous.model_copy(update={"score_summary": {"regression_ok": 0.0}})

    result = compare_reports(previous, current)

    assert {
        "metric": "score_summary.regression_ok",
        "severity": "critical",
        "previous": 1.0,
        "current": 0.0,
        "delta": -1.0,
    } in result["regressions"]
```

- [x] **Step 2: Run focused test and verify failure**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_reporting.py::test_compare_reports_flags_score_summary_drop -v
```

Expected: FAIL because `compare_reports()` does not compare score summary keys.

- [x] **Step 3: Extend comparison logic**

Modify `apps/eval-runner/app/report_compare.py`:

```python
    score_names = set(previous.score_summary) | set(current.score_summary)
    for name in sorted(score_names):
        previous_score = previous.score_summary.get(name, 0.0)
        current_score = current.score_summary.get(name, 0.0)
        delta = current_score - previous_score
        if delta <= -0.5:
            regressions.append(
                {
                    "metric": f"score_summary.{name}",
                    "severity": "critical",
                    "previous": previous_score,
                    "current": current_score,
                    "delta": delta,
                }
            )
```

- [x] **Step 4: Add `baseline compare` CLI**

Add a `compare` subparser inside `baseline_main()`:

```python
    compare = subparsers.add_parser("compare")
    compare.add_argument("--registry", type=Path, required=True)
    compare.add_argument("--name", required=True)
    compare.add_argument("--artifact", type=Path, required=True)
    compare.add_argument("--fail-on-critical", action="store_true")
```

Add the command branch:

```python
    if args.command == "compare":
        from app.artifacts import load_report_artifact
        from app.report_compare import compare_reports

        record = registry.latest(name=args.name)
        if record is None:
            print("error: no trusted baseline found", file=sys.stderr)
            return 1
        previous = load_report_artifact(Path(record.artifact_path)).report
        current = load_report_artifact(args.artifact).report
        result = compare_reports(previous, current)
        print(json.dumps(result, indent=2))
        has_critical = any(item.get("severity") == "critical" for item in result["regressions"])
        return 1 if args.fail_on_critical and has_critical else 0
```

- [x] **Step 5: Add CLI regression test**

Add to `apps/eval-runner/tests/test_cli.py`:

```python
def test_cli_baseline_compare_fails_on_critical_regression(tmp_path, capsys):
    import json
    from app.cli import main

    def artifact(path, success_rate):
        path.write_text(
            json.dumps(
                {
                    "report": {
                        "total_tasks": 1,
                        "success_rate": success_rate,
                        "verification_pass_rate": success_rate,
                        "scope_violation_rate": 0.0,
                        "avg_duration_ms": 10.0,
                        "avg_model_rounds": 1.0,
                        "avg_confirm_requests": 0.0,
                        "failure_categories": {},
                        "score_summary": {},
                        "tasks": [],
                    },
                    "traces": [],
                }
            ),
            encoding="utf-8",
        )

    baseline = tmp_path / "baseline.json"
    candidate = tmp_path / "candidate.json"
    registry = tmp_path / "baselines.json"
    artifact(baseline, 1.0)
    artifact(candidate, 0.0)

    assert main(["baseline", "promote", "--registry", str(registry), "--artifact", str(baseline), "--name", "dev"]) == 0
    assert main(["baseline", "compare", "--registry", str(registry), "--name", "dev", "--artifact", str(candidate), "--fail-on-critical"]) == 1

    assert '"metric": "success_rate"' in capsys.readouterr().out
```

- [x] **Step 6: Run tests**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_reporting.py tests/test_cli.py -v
```

Expected: PASS.

- [x] **Step 7: Commit**

```bash
git add apps/eval-runner/app/report_compare.py apps/eval-runner/app/cli.py apps/eval-runner/tests/test_reporting.py apps/eval-runner/tests/test_cli.py apps/eval-runner/README.md
git commit -m "feat(eval): compare reports against trusted baselines"
```

---

### Task 3: Markdown and Static HTML Report Rendering

**Files:**
- Create: `apps/eval-runner/app/report_render.py`
- Modify: `apps/eval-runner/app/cli.py`
- Test: `apps/eval-runner/tests/test_report_render.py`
- Test: `apps/eval-runner/tests/test_cli.py`
- Docs: `apps/eval-runner/README.md`

- [x] **Step 1: Write failing renderer tests**

Create `apps/eval-runner/tests/test_report_render.py`:

```python
from app.models import BacktestReport


def sample_report() -> BacktestReport:
    return BacktestReport(
        total_tasks=2,
        success_rate=0.5,
        verification_pass_rate=0.5,
        scope_violation_rate=0.0,
        avg_duration_ms=20.0,
        avg_model_rounds=2.0,
        avg_confirm_requests=1.0,
        failure_categories={"verification_failed": 1},
        score_summary={"functional_correctness": 0.5, "scope_ok": 1.0},
        tasks=[],
    )


def test_render_markdown_report_contains_operator_summary():
    from app.report_render import render_markdown_report

    markdown = render_markdown_report(sample_report(), title="Local Regression")

    assert "# Local Regression" in markdown
    assert "| success_rate | 0.500 |" in markdown
    assert "verification_failed" in markdown
    assert "functional_correctness" in markdown


def test_render_html_report_escapes_title():
    from app.report_render import render_html_report

    html = render_html_report(sample_report(), title="<release>")

    assert "&lt;release&gt;" in html
    assert "<table" in html
```

- [x] **Step 2: Run focused tests and verify failure**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_report_render.py -v
```

Expected: FAIL because `app.report_render` does not exist.

- [x] **Step 3: Implement report renderer**

Create `apps/eval-runner/app/report_render.py`:

```python
from html import escape

from app.models import BacktestReport


def render_markdown_report(report: BacktestReport, *, title: str = "Forge Eval Report") -> str:
    lines = [
        f"# {title}",
        "",
        "| Metric | Value |",
        "|---|---:|",
        f"| total_tasks | {report.total_tasks} |",
        f"| success_rate | {report.success_rate:.3f} |",
        f"| verification_pass_rate | {report.verification_pass_rate:.3f} |",
        f"| scope_violation_rate | {report.scope_violation_rate:.3f} |",
        f"| avg_model_rounds | {report.avg_model_rounds:.3f} |",
        f"| total_cost_usd | {report.total_cost_usd:.3f} |",
        "",
        "## Failure Categories",
        "",
    ]
    if report.failure_categories:
        lines.extend(
            f"- `{name}`: {count}"
            for name, count in sorted(report.failure_categories.items())
        )
    else:
        lines.append("- None")

    lines.extend(["", "## Score Summary", ""])
    if report.score_summary:
        lines.extend(f"- `{name}`: {score:.3f}" for name, score in sorted(report.score_summary.items()))
    else:
        lines.append("- None")
    return "\n".join(lines) + "\n"


def render_html_report(report: BacktestReport, *, title: str = "Forge Eval Report") -> str:
    rows = [
        ("total_tasks", str(report.total_tasks)),
        ("success_rate", f"{report.success_rate:.3f}"),
        ("verification_pass_rate", f"{report.verification_pass_rate:.3f}"),
        ("scope_violation_rate", f"{report.scope_violation_rate:.3f}"),
        ("avg_model_rounds", f"{report.avg_model_rounds:.3f}"),
        ("total_cost_usd", f"{report.total_cost_usd:.3f}"),
    ]
    row_html = "\n".join(
        f"<tr><th>{escape(name)}</th><td>{escape(value)}</td></tr>"
        for name, value in rows
    )
    score_items = "\n".join(
        f"<li><code>{escape(name)}</code>: {score:.3f}</li>"
        for name, score in sorted(report.score_summary.items())
    ) or "<li>None</li>"
    failure_items = "\n".join(
        f"<li><code>{escape(name)}</code>: {count}</li>"
        for name, count in sorted(report.failure_categories.items())
    ) or "<li>None</li>"
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>{escape(title)}</title>
  <style>
    body {{ font-family: system-ui, sans-serif; margin: 32px; color: #111; }}
    table {{ border-collapse: collapse; min-width: 420px; }}
    th, td {{ border: 1px solid #ccc; padding: 8px 10px; text-align: left; }}
  </style>
</head>
<body>
  <h1>{escape(title)}</h1>
  <table>{row_html}</table>
  <h2>Failure Categories</h2>
  <ul>{failure_items}</ul>
  <h2>Score Summary</h2>
  <ul>{score_items}</ul>
</body>
</html>
"""
```

- [x] **Step 4: Add `render-report` CLI**

Add to the top of `main()` in `apps/eval-runner/app/cli.py`:

```python
    if raw_argv[:1] == ["render-report"]:
        return render_report_main(raw_argv[1:])
```

Add:

```python
def render_report_main(argv: list[str]) -> int:
    from app.artifacts import load_report_artifact
    from app.report_render import render_html_report, render_markdown_report

    parser = argparse.ArgumentParser(description="Render an eval report artifact.")
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--format", choices=["markdown", "html"], default="markdown")
    parser.add_argument("--title", default="Forge Eval Report")
    args = parser.parse_args(argv)

    report = load_report_artifact(args.artifact).report
    rendered = (
        render_html_report(report, title=args.title)
        if args.format == "html"
        else render_markdown_report(report, title=args.title)
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(rendered, encoding="utf-8")
    print(json.dumps({"output": str(args.output), "format": args.format}, indent=2))
    return 0
```

- [x] **Step 5: Add CLI test**

Add to `apps/eval-runner/tests/test_cli.py`:

```python
def test_cli_render_report_writes_markdown(tmp_path):
    import json
    from app.cli import main

    artifact = tmp_path / "run.json"
    output = tmp_path / "report.md"
    artifact.write_text(
        json.dumps(
            {
                "report": {
                    "total_tasks": 1,
                    "success_rate": 1.0,
                    "verification_pass_rate": 1.0,
                    "scope_violation_rate": 0.0,
                    "avg_duration_ms": 10.0,
                    "avg_model_rounds": 1.0,
                    "avg_confirm_requests": 0.0,
                    "failure_categories": {},
                    "score_summary": {},
                    "tasks": [],
                },
                "traces": [],
            }
        ),
        encoding="utf-8",
    )

    assert main(["render-report", "--artifact", str(artifact), "--output", str(output), "--title", "Release Gate"]) == 0
    assert "# Release Gate" in output.read_text(encoding="utf-8")
```

- [x] **Step 6: Run tests**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_report_render.py tests/test_cli.py -v
```

Expected: PASS.

- [x] **Step 7: Commit**

```bash
git add apps/eval-runner/app/report_render.py apps/eval-runner/app/cli.py apps/eval-runner/tests/test_report_render.py apps/eval-runner/tests/test_cli.py apps/eval-runner/README.md
git commit -m "feat(eval): render operator reports"
```

---

### Task 4: Case Lifecycle Diagnostics

**Files:**
- Create: `apps/eval-runner/app/case_lifecycle.py`
- Modify: `apps/eval-runner/app/models.py`
- Modify: `apps/eval-runner/app/cli.py`
- Test: `apps/eval-runner/tests/test_cases.py`
- Test: `apps/eval-runner/tests/test_cli.py`
- Docs: `apps/eval-runner/docs/ops.md`

- [x] **Step 1: Write failing lifecycle tests**

Add to `apps/eval-runner/tests/test_cases.py`:

```python
def test_case_lifecycle_reports_quarantined_and_missing_owner(tmp_path):
    case = tmp_path / "case.json"
    case.write_text(
        """
        {
          "id": "flaky-case",
          "title": "Flaky case",
          "prompt": "Fix parser",
          "metadata": {
            "lifecycle": {
              "status": "quarantined",
              "reason": "intermittent provider timeout"
            }
          }
        }
        """,
        encoding="utf-8",
    )
    from app.case_lifecycle import inspect_case_lifecycle
    from app.cases import load_cases

    result = inspect_case_lifecycle(load_cases(case))

    assert result.counts == {"quarantined": 1}
    assert result.issues[0].code == "missing_owner"
    assert result.issues[1].code == "quarantined_case"
```

- [x] **Step 2: Run focused test and verify failure**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_cases.py::test_case_lifecycle_reports_quarantined_and_missing_owner -v
```

Expected: FAIL because `app.case_lifecycle` does not exist.

- [x] **Step 3: Add lifecycle models**

Add to `apps/eval-runner/app/models.py`:

```python
class CaseLifecycleIssue(EvalModel):
    task_id: str
    code: str
    message: str


class CaseLifecycleReport(EvalModel):
    counts: dict[str, int] = Field(default_factory=dict)
    issues: list[CaseLifecycleIssue] = Field(default_factory=list)
```

- [x] **Step 4: Implement lifecycle diagnostics**

Create `apps/eval-runner/app/case_lifecycle.py`:

```python
from collections import Counter

from app.models import CaseLifecycleIssue, CaseLifecycleReport, EvaluationTask

VALID_STATUSES = {"active", "flaky", "quarantined", "retired"}


def inspect_case_lifecycle(tasks: list[EvaluationTask]) -> CaseLifecycleReport:
    counts: Counter[str] = Counter()
    issues: list[CaseLifecycleIssue] = []
    for task in tasks:
        lifecycle = task.metadata.get("lifecycle", {})
        lifecycle = lifecycle if isinstance(lifecycle, dict) else {}
        status = str(lifecycle.get("status", "active"))
        counts[status] += 1
        if status not in VALID_STATUSES:
            issues.append(
                CaseLifecycleIssue(
                    task_id=task.id,
                    code="invalid_lifecycle_status",
                    message=f"Unknown lifecycle status: {status}",
                )
            )
        if not lifecycle.get("owner"):
            issues.append(
                CaseLifecycleIssue(
                    task_id=task.id,
                    code="missing_owner",
                    message="Case lifecycle metadata should include owner.",
                )
            )
        if status == "quarantined":
            issues.append(
                CaseLifecycleIssue(
                    task_id=task.id,
                    code="quarantined_case",
                    message=str(lifecycle.get("reason") or "Case is quarantined."),
                )
            )
    return CaseLifecycleReport(counts=dict(counts), issues=issues)
```

- [x] **Step 5: Add CLI command**

Add to the top of `main()` in `apps/eval-runner/app/cli.py`:

```python
    if raw_argv[:1] == ["case-lifecycle"]:
        return case_lifecycle_main(raw_argv[1:])
```

Add:

```python
def case_lifecycle_main(argv: list[str]) -> int:
    from app.case_lifecycle import inspect_case_lifecycle

    parser = argparse.ArgumentParser(description="Inspect eval case lifecycle metadata.")
    parser.add_argument("--cases", type=Path, required=True)
    parser.add_argument("--fail-on-quarantined", action="store_true")
    args = parser.parse_args(argv)

    report = inspect_case_lifecycle(load_cases(args.cases))
    print(report.model_dump_json(indent=2))
    has_quarantined = report.counts.get("quarantined", 0) > 0
    return 1 if args.fail_on_quarantined and has_quarantined else 0
```

- [x] **Step 6: Run tests**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_cases.py tests/test_cli.py -v
```

Expected: PASS.

- [x] **Step 7: Commit**

```bash
git add apps/eval-runner/app/case_lifecycle.py apps/eval-runner/app/models.py apps/eval-runner/app/cli.py apps/eval-runner/tests/test_cases.py apps/eval-runner/tests/test_cli.py apps/eval-runner/docs/ops.md
git commit -m "feat(eval): add case lifecycle diagnostics"
```

---

### Task 5: Flake Triage for Repeated Trials

**Files:**
- Create: `apps/eval-runner/app/flake_triage.py`
- Modify: `apps/eval-runner/app/models.py`
- Modify: `apps/eval-runner/app/cli.py`
- Test: `apps/eval-runner/tests/test_reporting.py`
- Test: `apps/eval-runner/tests/test_cli.py`
- Docs: `apps/eval-runner/docs/ops.md`

- [x] **Step 1: Write failing flake triage tests**

Add to `apps/eval-runner/tests/test_reporting.py`:

```python
def test_flake_triage_classifies_mixed_trial_results():
    from app.flake_triage import triage_trial_metrics

    result = triage_trial_metrics(
        {
            "parser-case": {"attempts": 3, "pass_rate": 0.667, "flaky": True},
            "stable-fail": {"attempts": 3, "pass_rate": 0.0, "flaky": False},
            "stable-pass": {"attempts": 3, "pass_rate": 1.0, "flaky": False},
        }
    )

    assert result.items[0].task_id == "parser-case"
    assert result.items[0].classification == "flaky"
    assert result.items[1].classification == "stable_fail"
    assert result.items[2].classification == "stable_pass"
    assert result.quarantine_candidates == ["parser-case"]
```

- [x] **Step 2: Run focused test and verify failure**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_reporting.py::test_flake_triage_classifies_mixed_trial_results -v
```

Expected: FAIL because `app.flake_triage` does not exist.

- [x] **Step 3: Add flake triage models**

Add to `apps/eval-runner/app/models.py`:

```python
class FlakeTriageItem(EvalModel):
    task_id: str
    attempts: int = Field(ge=1)
    pass_rate: float = Field(ge=0.0, le=1.0)
    classification: str


class FlakeTriageReport(EvalModel):
    items: list[FlakeTriageItem] = Field(default_factory=list)
    quarantine_candidates: list[str] = Field(default_factory=list)
```

- [x] **Step 4: Implement triage**

Create `apps/eval-runner/app/flake_triage.py`:

```python
from app.models import FlakeTriageItem, FlakeTriageReport


def triage_trial_metrics(
    trial_metrics: dict[str, dict[str, float | int | bool]],
) -> FlakeTriageReport:
    items: list[FlakeTriageItem] = []
    quarantine_candidates: list[str] = []
    for task_id, metrics in sorted(trial_metrics.items()):
        attempts = int(metrics["attempts"])
        pass_rate = float(metrics["pass_rate"])
        flaky = bool(metrics.get("flaky", False))
        if flaky:
            classification = "flaky"
            quarantine_candidates.append(task_id)
        elif pass_rate == 0.0:
            classification = "stable_fail"
        elif pass_rate == 1.0:
            classification = "stable_pass"
        else:
            classification = "needs_review"
            quarantine_candidates.append(task_id)
        items.append(
            FlakeTriageItem(
                task_id=task_id,
                attempts=attempts,
                pass_rate=pass_rate,
                classification=classification,
            )
        )
    return FlakeTriageReport(items=items, quarantine_candidates=quarantine_candidates)
```

- [x] **Step 5: Add CLI `flake-triage`**

Add to the top of `main()`:

```python
    if raw_argv[:1] == ["flake-triage"]:
        return flake_triage_main(raw_argv[1:])
```

Add:

```python
def flake_triage_main(argv: list[str]) -> int:
    from app.artifacts import load_report_artifact
    from app.flake_triage import triage_trial_metrics
    from app.metrics import calculate_metrics
    from app.reporting import aggregate_trial_metrics

    parser = argparse.ArgumentParser(description="Classify repeated-trial flakes.")
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--fail-on-flaky", action="store_true")
    args = parser.parse_args(argv)

    artifact = load_report_artifact(args.artifact)
    artifact_payload = json.loads(artifact.path.read_text(encoding="utf-8"))
    traces = [AgentTrace.model_validate(trace) for trace in artifact_payload.get("traces", [])]
    report = triage_trial_metrics(aggregate_trial_metrics(calculate_metrics(traces).tasks))
    print(report.model_dump_json(indent=2))
    return 1 if args.fail_on_flaky and report.quarantine_candidates else 0
```

- [x] **Step 6: Run tests**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_reporting.py tests/test_cli.py -v
```

Expected: PASS.

- [x] **Step 7: Commit**

```bash
git add apps/eval-runner/app/flake_triage.py apps/eval-runner/app/models.py apps/eval-runner/app/cli.py apps/eval-runner/tests/test_reporting.py apps/eval-runner/tests/test_cli.py apps/eval-runner/docs/ops.md
git commit -m "feat(eval): add repeated-trial flake triage"
```

---

### Task 6: Production Trace Promotion v2

**Files:**
- Modify: `apps/eval-runner/app/trace_import.py`
- Modify: `apps/eval-runner/app/models.py`
- Modify: `apps/eval-runner/app/cli.py`
- Test: `apps/eval-runner/tests/test_cases.py`
- Test: `apps/eval-runner/tests/test_cli.py`
- Docs: `apps/eval-runner/docs/ops.md`

- [x] **Step 1: Write failing tests for redaction and dedupe**

Add to `apps/eval-runner/tests/test_cases.py`:

```python
def test_promote_trace_redacts_secret_like_values_and_dedupes(tmp_path):
    import json
    from app.trace_import import promote_failed_traces

    trace = {
        "task_id": "prod-secret-failure",
        "user_prompt": "Fix API key sk-live-1234567890abcdef",
        "model": "local-forge",
        "provider": "forge",
        "context_files": ["src/app.py"],
        "raw_events": [],
        "tool_calls": [],
        "shell_outputs": [],
        "file_diffs": [],
        "changed_files": ["src/app.py"],
        "scope_violations": [],
        "expected_files_changed": ["src/app.py"],
        "forbidden_files_changed": [".env"],
        "final_answer": "failed",
        "verification_result": None,
        "error": "verification_failed",
        "failure_reason": "secret in prompt",
        "failure_category": "verification_failed",
        "model_rounds": 1,
        "confirm_requests": 0,
        "started_at": "2026-06-17T00:00:00+00:00",
        "ended_at": "2026-06-17T00:00:01+00:00",
        "duration_ms": 1000,
    }
    artifact = tmp_path / "trace.json"
    artifact.write_text(json.dumps({"traces": [trace, trace]}), encoding="utf-8")

    written = promote_failed_traces(artifact, tmp_path / "cases", redact_secrets=True, dedupe=True)

    assert len(written) == 1
    case_text = written[0].read_text(encoding="utf-8")
    assert "sk-live" not in case_text
    assert "[REDACTED_SECRET]" in case_text
```

- [x] **Step 2: Run focused test and verify failure**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_cases.py::test_promote_trace_redacts_secret_like_values_and_dedupes -v
```

Expected: FAIL because `promote_failed_traces()` does not support redaction/dedupe arguments.

- [x] **Step 3: Add promotion decision model**

Add to `apps/eval-runner/app/models.py`:

```python
class TracePromotionDecision(EvalModel):
    task_id: str
    written: bool
    output_path: str | None = None
    reason: str | None = None
```

- [x] **Step 4: Implement redaction and dedupe**

Modify `apps/eval-runner/app/trace_import.py`:

```python
import hashlib
import re

SECRET_PATTERN = re.compile(r"(sk-[A-Za-z0-9_-]{10,}|ghp_[A-Za-z0-9_]{10,}|xox[baprs]-[A-Za-z0-9-]{10,})")


def redact_text(value: str) -> str:
    return SECRET_PATTERN.sub("[REDACTED_SECRET]", value)


def trace_dedupe_key(trace: AgentTrace) -> str:
    payload = "|".join(
        [
            trace.task_id,
            trace.user_prompt,
            trace.failure_category.value,
            trace.failure_reason or "",
        ]
    )
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()
```

Change the function signature:

```python
def promote_failed_traces(
    trace_path: Path,
    output_dir: Path,
    *,
    redact_secrets: bool = False,
    dedupe: bool = False,
) -> list[Path]:
```

Inside the trace loop, keep a `seen_keys: set[str] = set()`. Skip duplicate keys when `dedupe` is true. Before writing the case, use `prompt = redact_text(trace.user_prompt) if redact_secrets else trace.user_prompt`.

- [x] **Step 5: Extend CLI flags**

In `promote_trace_main()` add:

```python
    parser.add_argument("--redact-secrets", action="store_true")
    parser.add_argument("--dedupe", action="store_true")
```

Call:

```python
    written = promote_failed_traces(
        args.trace,
        args.output,
        redact_secrets=args.redact_secrets,
        dedupe=args.dedupe,
    )
```

- [x] **Step 6: Run tests**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_cases.py tests/test_cli.py -v
```

Expected: PASS.

- [x] **Step 7: Commit**

```bash
git add apps/eval-runner/app/trace_import.py apps/eval-runner/app/models.py apps/eval-runner/app/cli.py apps/eval-runner/tests/test_cases.py apps/eval-runner/tests/test_cli.py apps/eval-runner/docs/ops.md
git commit -m "feat(eval): harden trace promotion"
```

---

### Task 7: CI Markdown Summary

**Files:**
- Create: `apps/eval-runner/app/ci_summary.py`
- Modify: `apps/eval-runner/app/cli.py`
- Test: `apps/eval-runner/tests/test_report_render.py`
- Test: `apps/eval-runner/tests/test_cli.py`
- Docs: `apps/eval-runner/docs/ops.md`

- [x] **Step 1: Write failing CI summary test**

Add to `apps/eval-runner/tests/test_report_render.py`:

```python
def test_ci_summary_renders_regression_table():
    from app.ci_summary import render_ci_summary

    markdown = render_ci_summary(
        comparison={
            "regressions": [
                {
                    "metric": "success_rate",
                    "severity": "critical",
                    "previous": 1.0,
                    "current": 0.0,
                    "delta": -1.0,
                }
            ]
        }
    )

    assert "## Forge Eval CI Summary" in markdown
    assert "| success_rate | critical | 1.000 | 0.000 | -1.000 |" in markdown
```

- [x] **Step 2: Run focused test and verify failure**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_report_render.py::test_ci_summary_renders_regression_table -v
```

Expected: FAIL because `app.ci_summary` does not exist.

- [x] **Step 3: Implement CI summary renderer**

Create `apps/eval-runner/app/ci_summary.py`:

```python
from typing import Any


def render_ci_summary(comparison: dict[str, list[dict[str, Any]]]) -> str:
    regressions = comparison.get("regressions", [])
    lines = [
        "## Forge Eval CI Summary",
        "",
        "| Metric | Severity | Previous | Current | Delta |",
        "|---|---|---:|---:|---:|",
    ]
    if not regressions:
        lines.append("| none | pass | 0.000 | 0.000 | 0.000 |")
    for item in regressions:
        lines.append(
            "| {metric} | {severity} | {previous:.3f} | {current:.3f} | {delta:.3f} |".format(
                metric=item["metric"],
                severity=item["severity"],
                previous=float(item["previous"]),
                current=float(item["current"]),
                delta=float(item["delta"]),
            )
        )
    return "\n".join(lines) + "\n"
```

- [x] **Step 4: Add CLI command**

Add to the top of `main()`:

```python
    if raw_argv[:1] == ["ci-summary"]:
        return ci_summary_main(raw_argv[1:])
```

Add:

```python
def ci_summary_main(argv: list[str]) -> int:
    from app.artifacts import load_report_artifact
    from app.ci_summary import render_ci_summary
    from app.report_compare import compare_reports

    parser = argparse.ArgumentParser(description="Render a PR-friendly eval summary.")
    parser.add_argument("--previous", type=Path, required=True)
    parser.add_argument("--current", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)

    comparison = compare_reports(
        load_report_artifact(args.previous).report,
        load_report_artifact(args.current).report,
    )
    summary = render_ci_summary(comparison)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(summary, encoding="utf-8")
    print(json.dumps({"output": str(args.output)}, indent=2))
    return 0
```

- [x] **Step 5: Run tests**

Run:

```bash
cd apps/eval-runner
uv run pytest tests/test_report_render.py tests/test_cli.py -v
```

Expected: PASS.

- [x] **Step 6: Commit**

```bash
git add apps/eval-runner/app/ci_summary.py apps/eval-runner/app/cli.py apps/eval-runner/tests/test_report_render.py apps/eval-runner/tests/test_cli.py apps/eval-runner/docs/ops.md
git commit -m "feat(eval): render ci comparison summaries"
```

---

### Task 8: Documentation and Final Acceptance

**Files:**
- Modify: `apps/eval-runner/README.md`
- Modify: `apps/eval-runner/docs/ops.md`
- Modify: `apps/eval-runner/docs/architecture.md`
- Modify: `CHANGELOG.md`

- [x] **Step 1: Update operator docs**

Document these workflows in `apps/eval-runner/docs/ops.md`:

```text
1. Run a normal backtest with --output.
2. Promote a successful artifact to a trusted baseline.
3. Compare a candidate artifact against the latest trusted baseline.
4. Render Markdown/HTML reports for human review.
5. Run flake triage after repeated trials.
6. Promote production traces with --redact-secrets and --dedupe.
7. Render a CI summary for pull request comments.
```

- [x] **Step 2: Update README commands**

Add these examples to `apps/eval-runner/README.md`:

```bash
uv run python -m app.cli baseline promote --registry output/baselines.json --artifact output/local-regression.json --name local-regression
uv run python -m app.cli baseline compare --registry output/baselines.json --name local-regression --artifact output/candidate.json --fail-on-critical
uv run python -m app.cli render-report --artifact output/candidate.json --output output/candidate.md --format markdown
uv run python -m app.cli render-report --artifact output/candidate.json --output output/candidate.html --format html
uv run python -m app.cli flake-triage --artifact output/local-regression.json --fail-on-flaky
uv run python -m app.cli promote-trace --trace artifacts/run/trace.json --output eval_cases/promoted --redact-secrets --dedupe
uv run python -m app.cli ci-summary --previous output/baseline.json --current output/candidate.json --output output/ci-summary.md
```

- [x] **Step 3: Update architecture docs**

Add a Phase 2 section to `apps/eval-runner/docs/architecture.md`:

```text
Phase 2 keeps the report artifact as the boundary between execution and release
decisions. The baseline registry stores trusted report artifact pointers. Report
rendering, flake triage, case lifecycle diagnostics, and CI summaries all consume
the same BacktestReport contract, so operators do not need to inspect raw trace
JSON unless a task needs deeper debugging.
```

- [x] **Step 4: Run full validation**

Run:

```bash
npm run test:eval
cd apps/eval-runner
uv run python -m app.cli --cases eval_cases --provider mock --trials 3 --experiment-name local-regression --output output/local-regression.json --min-success-rate 0.1 --max-scope-violation-rate 0.2
uv run python -m app.cli baseline promote --registry output/baselines.json --artifact output/local-regression.json --name local-regression
uv run python -m app.cli baseline compare --registry output/baselines.json --name local-regression --artifact output/local-regression.json --fail-on-critical
uv run python -m app.cli render-report --artifact output/local-regression.json --output output/local-regression.md --format markdown
uv run python -m app.cli render-report --artifact output/local-regression.json --output output/local-regression.html --format html
uv run python -m app.cli flake-triage --artifact output/local-regression.json
uv run python -m app.cli ci-summary --previous output/local-regression.json --current output/local-regression.json --output output/ci-summary.md
git diff --check
```

Expected: all commands exit 0.

- [x] **Step 5: Run GitNexus change detection before commit**

Run:

```text
gitnexus_detect_changes(repo: "forge", scope: "staged")
```

Expected: affected scope is limited to eval-runner modules, eval-runner tests, eval-runner docs, and root changelog.

- [x] **Step 6: Commit**

```bash
git add apps/eval-runner/README.md apps/eval-runner/docs/ops.md apps/eval-runner/docs/architecture.md CHANGELOG.md
git commit -m "docs(eval): document phase 2 operator workflow"
```

---

## Priority Order

1. Task 0: artifact loading boundary.
2. Task 1: trusted baseline registry.
3. Task 2: baseline comparison gate.
4. Task 3: Markdown and static HTML report rendering.
5. Task 5: flake triage.
6. Task 4: case lifecycle diagnostics.
7. Task 6: trace promotion v2.
8. Task 7: CI Markdown summary.
9. Task 8: documentation and final acceptance.

This order creates a stable report-artifact boundary first, then makes baseline comparison usable, then adds operator readability, then adds maintenance controls for flaky cases and promoted production traces.

## Final Acceptance

- `npm run test:eval` passes.
- A report artifact can be loaded from both raw report JSON and CLI artifact JSON.
- A trusted baseline can be promoted, listed, and compared from CLI.
- Critical regressions in success rate, scope violation rate, and score summary can fail CI.
- Markdown and HTML reports render from an artifact without needing trace inspection.
- Case lifecycle diagnostics identify missing owners, invalid statuses, and quarantined cases.
- Repeated-trial reports produce flake classifications and quarantine candidates.
- Trace promotion supports deterministic secret redaction and duplicate suppression.
- CI summary command writes a PR-friendly Markdown comparison table.
- Docs explain the baseline, report, flake, trace promotion, and CI workflows.
- GitNexus staged change detection reports low/expected impact before each commit.

## Execution Notes

- Before editing any Python function/class/method, run GitNexus impact analysis for the target symbol and report the risk.
- Keep all Phase 2 outputs under `apps/eval-runner/output/` in examples; do not commit generated output artifacts.
- Do not stage unrelated desktop/A2A runtime changes while executing this plan.
- Preserve `apps/eval-runner` as an independently runnable app.
