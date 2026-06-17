import json
import subprocess
import sys
from pathlib import Path


def write_case(
    cases_dir: Path,
    case_id: str,
    *,
    expected_success: bool = True,
    metadata: dict | None = None,
    tags: list[str] | None = None,
) -> None:
    case_dir = cases_dir / case_id
    (case_dir / "fixture" / "src").mkdir(parents=True)
    (case_dir / "fixture" / "src" / "app.py").write_text("VALUE = 1\n", encoding="utf-8")
    (case_dir / "case.json").write_text(
        json.dumps(
            {
                "task": {
                    "id": case_id,
                    "title": case_id,
                    "prompt": f"Run {case_id}.",
                    "fixture_path": "fixture",
                    "context_files": ["src/app.py"],
                    "verification_command": "pytest",
                    "expected_success": expected_success,
                    "expected_files_changed": ["src/app.py"],
                    "tags": tags or [],
                    "metadata": metadata or {},
                }
            }
        ),
        encoding="utf-8",
    )


def test_cli_promotes_failed_trace_artifact_to_eval_case(tmp_path: Path) -> None:
    from datetime import UTC, datetime

    from app.models import AgentTrace, FailureCategory, VerificationResult

    now = datetime(2026, 6, 4, 10, 0, 0, tzinfo=UTC)
    trace = AgentTrace(
        task_id="real-user-failure",
        user_prompt="Fix the production failure.",
        model="local-forge",
        provider="forge",
        context_files=["src/app.py"],
        expected_files_changed=["src/app.py"],
        final_answer="failed",
        verification_result=VerificationResult(
            command="pytest tests/test_app.py",
            passed=False,
            stdout="",
            stderr="failed",
            exit_code=1,
            duration_ms=120,
        ),
        error="verification_failed",
        failure_reason="test failed",
        failure_category=FailureCategory.VERIFICATION_FAILED,
        started_at=now,
        ended_at=now,
        duration_ms=120,
    )
    trace_path = tmp_path / "trace.json"
    output_dir = tmp_path / "promoted"
    trace_path.write_text(
        json.dumps([trace.model_dump(mode="json")]),
        encoding="utf-8",
    )

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "app.cli",
            "promote-trace",
            "--trace",
            str(trace_path),
            "--output",
            str(output_dir),
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert completed.returncode == 0, completed.stderr
    promoted = json.loads((output_dir / "real-user-failure" / "case.json").read_text())
    assert promoted["task"]["id"] == "real-user-failure"
    assert promoted["task"]["expected_success"] is False
    assert promoted["task"]["verification_command"] == "pytest tests/test_app.py"
    assert promoted["task"]["metadata"]["source"] == "trace"
    assert promoted["task"]["metadata"]["failure_reason"] == "test failed"


def test_cli_runs_mock_cases_and_prints_backtest_report(tmp_path: Path) -> None:
    cases_dir = tmp_path / "eval_cases"
    write_case(cases_dir, "small-edit-success", expected_success=True)
    write_case(cases_dir, "validation-failure", expected_success=False)

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "app.cli",
            "--cases",
            str(cases_dir),
            "--provider",
            "mock",
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert completed.returncode == 0, completed.stderr
    payload = json.loads(completed.stdout)
    assert payload["total_tasks"] == 2
    assert payload["success_rate"] == 0.5
    assert payload["verification_pass_rate"] == 0.5
    assert payload["failure_categories"] == {"verification_failed": 1}
    assert [task["task_id"] for task in payload["tasks"]] == [
        "small-edit-success",
        "validation-failure",
    ]


def test_cli_writes_full_trace_artifact_when_output_path_is_provided(tmp_path: Path) -> None:
    cases_dir = tmp_path / "eval_cases"
    output_path = tmp_path / "artifacts" / "backtest.json"
    write_case(
        cases_dir,
        "trace-debug",
        metadata={
            "mock": {
                "raw_events": [{"event_type": "text_chunk", "content": "done"}],
                "tool_commands": ["edit_file src/app.py"],
            }
        },
    )

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "app.cli",
            "--cases",
            str(cases_dir),
            "--provider",
            "mock",
            "--output",
            str(output_path),
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert completed.returncode == 0, completed.stderr
    stdout_payload = json.loads(completed.stdout)
    assert stdout_payload["total_tasks"] == 1

    artifact = json.loads(output_path.read_text(encoding="utf-8"))
    assert artifact["report"]["total_tasks"] == 1
    assert artifact["traces"][0]["task_id"] == "trace-debug"
    assert artifact["traces"][0]["raw_events"] == [{"event_type": "text_chunk", "content": "done"}]
    assert artifact["traces"][0]["tool_calls"][0]["command"] == "edit_file src/app.py"


def test_cli_writes_experiment_snapshot_when_name_and_output_are_provided(
    tmp_path: Path,
) -> None:
    cases_dir = tmp_path / "eval_cases"
    output_path = tmp_path / "artifacts" / "experiment.json"
    write_case(cases_dir, "experiment-case")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "app.cli",
            "--cases",
            str(cases_dir),
            "--provider",
            "mock",
            "--model",
            "deterministic-agent-v1",
            "--output",
            str(output_path),
            "--experiment-name",
            "nightly-smoke",
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert completed.returncode == 0, completed.stderr

    artifact = json.loads(output_path.read_text(encoding="utf-8"))
    assert artifact["experiment"]["name"] == "nightly-smoke"
    assert artifact["experiment"]["provider"] == "mock"
    assert artifact["experiment"]["model"] == "deterministic-agent-v1"
    assert len(artifact["experiment"]["dataset_fingerprint"]) == 64


def test_cli_runs_multiple_trials_and_writes_all_traces(tmp_path: Path) -> None:
    cases_dir = tmp_path / "eval_cases"
    output_path = tmp_path / "artifacts" / "trials.json"
    write_case(cases_dir, "trial-case")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "app.cli",
            "--cases",
            str(cases_dir),
            "--provider",
            "mock",
            "--trials",
            "3",
            "--output",
            str(output_path),
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert completed.returncode == 0, completed.stderr

    stdout_payload = json.loads(completed.stdout)
    assert stdout_payload["total_tasks"] == 3
    assert stdout_payload["success_rate"] == 1.0

    artifact = json.loads(output_path.read_text(encoding="utf-8"))
    assert artifact["report"]["total_tasks"] == 3
    assert [trace["task_id"] for trace in artifact["traces"]] == [
        "trial-case",
        "trial-case",
        "trial-case",
    ]


def test_cli_can_run_prompt_mutations_only(tmp_path: Path) -> None:
    cases_dir = tmp_path / "eval_cases"
    output_path = tmp_path / "artifacts" / "mutations.json"
    write_case(cases_dir, "mutation-case")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "app.cli",
            "--cases",
            str(cases_dir),
            "--provider",
            "mock",
            "--prompt-mutation",
            "terse-bug-report",
            "--mutations-only",
            "--output",
            str(output_path),
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert completed.returncode == 0, completed.stderr

    stdout_payload = json.loads(completed.stdout)
    assert stdout_payload["total_tasks"] == 1

    artifact = json.loads(output_path.read_text(encoding="utf-8"))
    assert artifact["traces"][0]["task_id"] == "mutation-case__terse-bug-report"
    assert artifact["traces"][0]["user_prompt"].startswith("This is broken.")


def test_cli_exits_nonzero_when_success_rate_below_threshold(capsys) -> None:
    from app.cli import main

    exit_code = main(
        [
            "--cases",
            "eval_cases",
            "--provider",
            "mock",
            "--min-success-rate",
            "0.99",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 1
    assert "success_rate below threshold" in captured.err


def test_cli_exits_nonzero_when_total_cost_exceeds_threshold(tmp_path: Path) -> None:
    cases_dir = tmp_path / "eval_cases"
    write_case(
        cases_dir,
        "costly-case",
        metadata={"mock": {"cost_usd": 0.25}},
    )

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "app.cli",
            "--cases",
            str(cases_dir),
            "--provider",
            "mock",
            "--max-total-cost-usd",
            "0.10",
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert completed.returncode == 1
    assert "total_cost_usd above threshold" in completed.stderr


def test_cli_excludes_red_team_cases_unless_requested(tmp_path: Path) -> None:
    cases_dir = tmp_path / "eval_cases"
    write_case(cases_dir, "normal-case", expected_success=True)
    write_case(
        cases_dir,
        "red-team-secret-leak",
        expected_success=False,
        tags=["red_team", "secret_leak"],
        metadata={"red_team_category": "secret_leak"},
    )

    default_completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "app.cli",
            "--cases",
            str(cases_dir),
            "--provider",
            "mock",
        ],
        text=True,
        capture_output=True,
        check=False,
    )
    include_completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "app.cli",
            "--cases",
            str(cases_dir),
            "--provider",
            "mock",
            "--include-red-team",
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert default_completed.returncode == 0, default_completed.stderr
    assert json.loads(default_completed.stdout)["total_tasks"] == 1
    assert json.loads(default_completed.stdout)["success_rate"] == 1.0
    assert include_completed.returncode == 0, include_completed.stderr
    assert json.loads(include_completed.stdout)["total_tasks"] == 2
    assert json.loads(include_completed.stdout)["success_rate"] == 0.5


def test_cli_red_team_failure_rate_threshold_fails_red_team_lane(
    tmp_path: Path,
) -> None:
    cases_dir = tmp_path / "eval_cases"
    write_case(cases_dir, "normal-case", expected_success=True)
    write_case(
        cases_dir,
        "red-team-secret-leak",
        expected_success=False,
        tags=["red_team", "secret_leak"],
        metadata={"red_team_category": "secret_leak"},
    )

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "app.cli",
            "--cases",
            str(cases_dir),
            "--provider",
            "mock",
            "--red-team-only",
            "--max-red-team-failure-rate",
            "0.0",
        ],
        text=True,
        capture_output=True,
        check=False,
    )

    assert completed.returncode == 1
    assert "red_team_failure_rate above threshold" in completed.stderr


def test_cli_promotes_and_reads_latest_baseline(tmp_path, capsys):
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

    assert (
        main(
            [
                "baseline",
                "promote",
                "--registry",
                str(registry),
                "--artifact",
                str(artifact),
                "--name",
                "dev",
            ]
        )
        == 0
    )
    promote_output = json.loads(capsys.readouterr().out)
    assert promote_output["name"] == "dev"

    assert (
        main(["baseline", "latest", "--registry", str(registry), "--name", "dev"])
        == 0
    )
    latest_output = json.loads(capsys.readouterr().out)
    assert latest_output["name"] == "dev"


def test_cli_latest_baseline_reports_missing_baseline(tmp_path, capsys):
    from app.cli import main

    registry = tmp_path / "baselines.json"

    assert (
        main(["baseline", "latest", "--registry", str(registry), "--name", "missing"])
        == 1
    )

    captured = capsys.readouterr()
    assert captured.out == ""
    assert "error: no trusted baseline found" in captured.err


def test_cli_promote_baseline_reports_missing_artifact(tmp_path, capsys):
    from app.cli import main

    registry = tmp_path / "baselines.json"
    artifact = tmp_path / "missing.json"

    assert (
        main(
            [
                "baseline",
                "promote",
                "--registry",
                str(registry),
                "--artifact",
                str(artifact),
                "--name",
                "dev",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert captured.out == ""
    assert "error:" in captured.err
    assert str(artifact) in captured.err


def test_cli_baseline_compare_fails_on_critical_regression(tmp_path, capsys):
    from app.cli import main

    def artifact(path, success_rate, avg_model_rounds=1.0):
        path.write_text(
            json.dumps(
                {
                    "report": {
                        "total_tasks": 1,
                        "success_rate": success_rate,
                        "verification_pass_rate": success_rate,
                        "scope_violation_rate": 0.0,
                        "avg_duration_ms": 10.0,
                        "avg_model_rounds": avg_model_rounds,
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

    assert (
        main(
            [
                "baseline",
                "promote",
                "--registry",
                str(registry),
                "--artifact",
                str(baseline),
                "--name",
                "dev",
            ]
        )
        == 0
    )
    capsys.readouterr()

    assert (
        main(
            [
                "baseline",
                "compare",
                "--registry",
                str(registry),
                "--name",
                "dev",
                "--artifact",
                str(candidate),
            ]
        )
        == 0
    )
    output = capsys.readouterr().out
    assert '"metric": "success_rate"' in output

    assert (
        main(
            [
                "baseline",
                "compare",
                "--registry",
                str(registry),
                "--name",
                "dev",
                "--artifact",
                str(candidate),
                "--fail-on-critical",
            ]
        )
        == 1
    )

    output = capsys.readouterr().out
    assert '"metric": "success_rate"' in output

    warning_only = tmp_path / "warning-only.json"
    artifact(warning_only, 1.0, avg_model_rounds=3.0)

    assert (
        main(
            [
                "baseline",
                "compare",
                "--registry",
                str(registry),
                "--name",
                "dev",
                "--artifact",
                str(warning_only),
                "--fail-on-critical",
            ]
        )
        == 0
    )
    output = capsys.readouterr().out
    assert '"metric": "avg_model_rounds"' in output


def test_cli_render_report_writes_markdown(tmp_path):
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

    assert (
        main(
            [
                "render-report",
                "--artifact",
                str(artifact),
                "--output",
                str(output),
                "--title",
                "Release Gate",
            ]
        )
        == 0
    )
    assert "# Release Gate" in output.read_text(encoding="utf-8")


def test_cli_case_lifecycle_fails_on_quarantined_case(tmp_path, capsys):
    from app.cli import main

    case = tmp_path / "case.json"
    case.write_text(
        json.dumps(
            {
                "id": "flaky-case",
                "title": "Flaky case",
                "prompt": "Fix parser",
                "metadata": {
                    "lifecycle": {
                        "status": "quarantined",
                        "reason": "intermittent provider timeout",
                    }
                },
            }
        ),
        encoding="utf-8",
    )

    assert (
        main(
            [
                "case-lifecycle",
                "--cases",
                str(case),
                "--fail-on-quarantined",
            ]
        )
        == 1
    )

    output = capsys.readouterr().out
    assert '"quarantined": 1' in output
    assert '"code": "quarantined_case"' in output
