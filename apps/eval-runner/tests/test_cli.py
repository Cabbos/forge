import json
import subprocess
import sys
from pathlib import Path

import pytest
from test_campaign import write_acceptance_artifacts

from app.provenance import sha256_bytes


def run_campaign_cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "-m", "app.cli", "campaign", *args],
        text=True,
        capture_output=True,
        check=False,
    )


def write_campaign_profile(path: Path) -> None:
    path.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "profile_id": "deepseek-v4-flash-frontier-v1",
                "provider": "deepseek",
                "model": "deepseek-v4-flash",
                "transport": "anthropic-compatible",
                "tool_policy": "forge-default",
                "max_model_rounds": 40,
            }
        ),
        encoding="utf-8",
    )


@pytest.mark.parametrize("command", ["init", "accept", "status", "finalize", "verify"])
def test_campaign_cli_mutations_and_reads_require_explicit_campaign(
    command: str,
) -> None:
    completed = run_campaign_cli(command)

    assert completed.returncode == 2
    assert "--campaign" in completed.stderr


def test_campaign_cli_runs_explicit_end_to_end_workflow_without_leaks(
    tmp_path: Path,
) -> None:
    campaign = tmp_path / "campaign"
    profile = tmp_path / "harness-profile.json"
    write_campaign_profile(profile)
    initialized = run_campaign_cli(
        "init",
        "--campaign",
        str(campaign),
        "--harness-profile",
        str(profile),
    )
    assert initialized.returncode == 0, initialized.stderr
    initialized_payload = json.loads(initialized.stdout)
    assert initialized_payload["status"] == "active"
    assert initialized_payload["provider"] == "deepseek"
    assert "worktree" not in initialized.stdout
    assert str(tmp_path) not in initialized.stdout

    trace_path, report_path, patch_path, _trace, _report = write_acceptance_artifacts(
        campaign, tmp_path / "inputs"
    )
    accepted = run_campaign_cli(
        "accept",
        "--campaign",
        str(campaign),
        "--task-id",
        "forgebench-dev-domain-normalize-title",
        "--trace",
        str(trace_path),
        "--report",
        str(report_path),
        "--patch",
        str(patch_path),
    )
    assert accepted.returncode == 0, accepted.stderr
    accepted_payload = json.loads(accepted.stdout)
    assert accepted_payload["status"] == "accepted"
    assert accepted_payload["trace_sha256"] == sha256_bytes(trace_path.read_bytes())
    assert accepted_payload["report_sha256"] == sha256_bytes(report_path.read_bytes())
    assert accepted_payload["patch_sha256"] == sha256_bytes(patch_path.read_bytes())

    ledger_before = (campaign / "ledger.jsonl").read_bytes()
    head_before = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=campaign / "worktree",
        text=True,
        capture_output=True,
        check=True,
    ).stdout
    status = run_campaign_cli("status", "--campaign", str(campaign))
    assert status.returncode == 0, status.stderr
    status_payload = json.loads(status.stdout)
    assert status_payload["entry_count"] == 1
    assert status_payload["ledger_verified"] is True
    assert status_payload["pending_accept"] is False
    assert (campaign / "ledger.jsonl").read_bytes() == ledger_before
    assert (
        subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=campaign / "worktree",
            text=True,
            capture_output=True,
            check=True,
        ).stdout
        == head_before
    )

    finalized = run_campaign_cli("finalize", "--campaign", str(campaign))
    assert finalized.returncode == 0, finalized.stderr
    finalized_payload = json.loads(finalized.stdout)
    assert finalized_payload["status"] == "complete"
    assert len(finalized_payload["manifest_sha256"]) == 64

    finalized_ledger = (campaign / "ledger.jsonl").read_bytes()
    finalized_head = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=campaign / "worktree",
        text=True,
        capture_output=True,
        check=True,
    ).stdout
    verified = run_campaign_cli("verify", "--campaign", str(campaign))
    assert verified.returncode == 0, verified.stderr
    verified_payload = json.loads(verified.stdout)
    assert verified_payload["ok"] is True
    assert verified_payload["blockers"] == []
    assert (campaign / "ledger.jsonl").read_bytes() == finalized_ledger
    assert (
        subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=campaign / "worktree",
            text=True,
            capture_output=True,
            check=True,
        ).stdout
        == finalized_head
    )
    combined_output = (
        initialized.stdout + accepted.stdout + status.stdout + finalized.stdout + verified.stdout
    ).lower()
    assert "prompt" not in combined_output
    assert "environment" not in combined_output
    assert "api_key" not in combined_output
    assert "sk-" not in combined_output


def test_campaign_cli_rejects_implicit_inputs_and_reports_tamper(
    tmp_path: Path,
) -> None:
    campaign = tmp_path / "campaign"
    profile = tmp_path / "harness-profile.json"
    write_campaign_profile(profile)
    initialized = run_campaign_cli(
        "init",
        "--campaign",
        str(campaign),
        "--harness-profile",
        str(profile),
    )
    assert initialized.returncode == 0, initialized.stderr

    missing_inputs = run_campaign_cli(
        "accept",
        "--campaign",
        str(campaign),
        "--task-id",
        "forgebench-dev-domain-normalize-title",
    )
    assert missing_inputs.returncode == 2
    assert "--trace" in missing_inputs.stderr
    assert "latest" not in missing_inputs.stderr.lower()

    unknown = run_campaign_cli("status", "--campaign", str(tmp_path / "missing"))
    assert unknown.returncode == 2
    malformed = tmp_path / "malformed"
    malformed.mkdir()
    (malformed / "campaign.json").write_text("{", encoding="utf-8")
    malformed_result = run_campaign_cli("status", "--campaign", str(malformed))
    assert malformed_result.returncode == 2
    secret_campaign = tmp_path / "secret-campaign"
    secret_campaign.mkdir()
    secret_value = "sk-" + "x" * 32
    (secret_campaign / "campaign.json").write_text(
        json.dumps({"schema_version": 1, "api_key": secret_value}),
        encoding="utf-8",
    )
    secret_result = run_campaign_cli(
        "status",
        "--campaign",
        str(secret_campaign),
    )
    assert secret_result.returncode == 2
    assert secret_value not in secret_result.stdout + secret_result.stderr

    (campaign / "campaign.json").write_text("{", encoding="utf-8")
    tampered = run_campaign_cli("verify", "--campaign", str(campaign))
    assert tampered.returncode == 1
    assert json.loads(tampered.stdout)["blockers"] == ["campaign_metadata_invalid"]


def test_cli_verifies_frozen_forgebench_catalog_without_leaking_holdout() -> None:
    completed = subprocess.run(
        [sys.executable, "-m", "app.cli", "forgebench", "verify-catalog"],
        text=True,
        capture_output=True,
        check=False,
    )

    assert completed.returncode == 0, completed.stderr
    payload = json.loads(completed.stdout)
    assert payload["development_count"] == 10
    assert payload["holdout_count"] == 20
    assert len(payload["combined_fingerprint"]) == 64
    assert "prompt" not in completed.stdout.lower()


def test_cli_shows_sealed_forgebench_split_metadata_only() -> None:
    outputs = {}
    for split, expected_count in (("development", 10), ("holdout", 20)):
        completed = subprocess.run(
            [
                sys.executable,
                "-m",
                "app.cli",
                "forgebench",
                "show-lock",
                "--split",
                split,
            ],
            text=True,
            capture_output=True,
            check=False,
        )

        assert completed.returncode == 0, completed.stderr
        payload = json.loads(completed.stdout)
        assert set(payload) == {"split", "count", "ids", "fingerprint"}
        assert payload["split"] == split
        assert payload["count"] == expected_count
        assert len(payload["ids"]) == expected_count
        assert all(
            task_id.startswith(f"forgebench-{'dev' if split == 'development' else 'holdout'}-")
            for task_id in payload["ids"]
        )
        outputs[split] = completed.stdout

    assert "prompt" not in outputs["holdout"].lower()
    assert "verifier" not in outputs["holdout"].lower()


def test_cli_exits_nonzero_when_trust_is_unknown(tmp_path: Path, capsys) -> None:
    from app.cli import main

    cases_dir = tmp_path / "empty-cases"
    cases_dir.mkdir()

    exit_code = main(["--cases", str(cases_dir), "--provider", "mock"])
    payload = json.loads(capsys.readouterr().out)

    assert exit_code == 1
    assert payload["trust_result"]["status"] == "unknown"


def test_cli_report_only_prints_blockers_and_exits_zero(tmp_path: Path, capsys) -> None:
    from app.cli import main

    cases_dir = tmp_path / "empty-cases"
    cases_dir.mkdir()

    exit_code = main(["--cases", str(cases_dir), "--provider", "mock", "--report-only"])
    payload = json.loads(capsys.readouterr().out)

    assert exit_code == 0
    assert payload["trust_result"]["trusted"] is False


def test_cli_invalid_provider_exits_two(capsys) -> None:
    from app.cli import main

    with pytest.raises(SystemExit) as exc_info:
        main(["--cases", "eval_cases", "--provider", "unknown"])
    assert exc_info.value.code == 2


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
