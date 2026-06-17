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
