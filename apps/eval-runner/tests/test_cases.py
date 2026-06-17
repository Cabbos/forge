import json
from datetime import UTC, datetime
from pathlib import Path

import pytest

from app.cases import CaseLoadError, load_cases
from app.models import AgentTrace, FailureCategory, VerificationResult


def write_case(cases_dir: Path, case_id: str, *, title: str | None = None) -> Path:
    case_dir = cases_dir / case_id
    fixture = case_dir / "fixture"
    (fixture / "src").mkdir(parents=True)
    (fixture / "src" / "app.py").write_text("print('hello')\n", encoding="utf-8")
    (case_dir / "case.json").write_text(
        json.dumps(
            {
                "task": {
                    "id": case_id,
                    "title": title or case_id,
                    "prompt": f"Complete {case_id}.",
                    "fixture_path": "fixture",
                    "context_files": ["src/app.py"],
                    "verification_command": "pytest",
                    "expected_files_changed": ["src/app.py"],
                }
            }
        ),
        encoding="utf-8",
    )
    return case_dir


def make_trace(
    task_id: str,
    *,
    error: str | None = None,
    failure_reason: str | None = None,
) -> AgentTrace:
    now = datetime(2026, 6, 4, 10, 0, 0, tzinfo=UTC)
    failed = error is not None or failure_reason is not None
    return AgentTrace(
        task_id=task_id,
        user_prompt=f"Fix production issue {task_id}.",
        model="local-forge",
        provider="forge",
        context_files=["src/app.py"],
        changed_files=["src/app.py"],
        expected_files_changed=["src/app.py"],
        forbidden_files_changed=[".env"],
        final_answer="failed" if failed else "done",
        verification_result=VerificationResult(
            command="pytest",
            passed=not failed,
            stdout="" if failed else "passed",
            stderr="failed" if failed else "",
            exit_code=1 if failed else 0,
            duration_ms=120,
        ),
        error=error,
        failure_reason=failure_reason,
        failure_category=FailureCategory.VERIFICATION_FAILED if failed else FailureCategory.NONE,
        started_at=now,
        ended_at=now,
        duration_ms=120,
    )


def test_failed_trace_can_be_promoted_to_eval_case() -> None:
    from app.trace_import import case_from_trace

    trace = make_trace(
        task_id="real-user-failure",
        error="verification_failed",
        failure_reason="test failed",
    )
    task = case_from_trace(trace)

    assert task.id == "real-user-failure"
    assert task.title == "Promoted trace: real-user-failure"
    assert task.prompt == "Fix production issue real-user-failure."
    assert task.expected_success is False
    assert task.expected_files_changed == ["src/app.py"]
    assert task.forbidden_files_changed == [".env"]
    assert task.verification_command == "pytest"
    assert task.metadata["source"] == "trace"
    assert task.metadata["failure_reason"] == "test failed"


def test_promote_trace_redacts_secret_like_values_and_dedupes(tmp_path):
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

    written = promote_failed_traces(
        artifact,
        tmp_path / "cases",
        redact_secrets=True,
        dedupe=True,
    )

    assert len(written) == 1
    case_text = written[0].read_text(encoding="utf-8")
    assert "sk-live" not in case_text
    assert "[REDACTED_SECRET]" in case_text


def test_load_cases_reads_case_directories_and_resolves_fixture_paths(tmp_path: Path) -> None:
    cases_dir = tmp_path / "eval_cases"
    first_dir = write_case(cases_dir, "small-edit-success", title="Small edit succeeds")
    write_case(cases_dir, "validation-failure", title="Validation fails")

    tasks = load_cases(cases_dir)

    assert [task.id for task in tasks] == ["small-edit-success", "validation-failure"]
    assert tasks[0].title == "Small edit succeeds"
    assert tasks[0].fixture_path is not None
    assert Path(tasks[0].fixture_path).is_absolute()
    assert Path(tasks[0].fixture_path) == first_dir / "fixture"


def test_load_cases_rejects_duplicate_task_ids(tmp_path: Path) -> None:
    cases_dir = tmp_path / "eval_cases"
    write_case(cases_dir, "duplicate")
    second = write_case(cases_dir, "duplicate-copy")
    payload = json.loads((second / "case.json").read_text(encoding="utf-8"))
    payload["task"]["id"] = "duplicate"
    (second / "case.json").write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(CaseLoadError, match="Duplicate task id"):
        load_cases(cases_dir)


def test_load_cases_reads_a_json_file_with_multiple_tasks(tmp_path: Path) -> None:
    cases_file = tmp_path / "cases.json"
    cases_file.write_text(
        json.dumps(
            [
                {
                    "id": "small-edit-success",
                    "title": "Small edit succeeds",
                    "prompt": "Make a safe edit.",
                    "context_files": ["src/app.py"],
                    "verification_command": "pytest",
                },
                {
                    "task": {
                        "id": "validation-failure",
                        "title": "Validation fails",
                        "prompt": "Trigger a validation failure.",
                        "context_files": ["src/app.py"],
                        "verification_command": "pytest",
                        "expected_success": False,
                    }
                },
            ]
        ),
        encoding="utf-8",
    )

    tasks = load_cases(cases_file)

    assert [task.id for task in tasks] == ["small-edit-success", "validation-failure"]
    assert tasks[1].expected_success is False


def test_load_cases_includes_real_forge_session_backtests() -> None:
    tasks = load_cases(Path("eval_cases"))
    task_by_id = {task.id: task for task in tasks}

    expected_ids = {
        "forge-session-normalize-input",
        "forge-session-date-utils",
        "forge-session-truncate-text",
        "forge-session-capitalize",
        "forge-session-kebab-case",
    }

    assert expected_ids.issubset(task_by_id)
    for task_id in expected_ids:
        task = task_by_id[task_id]
        assert task.fixture_path is not None
        assert Path(task.fixture_path).exists()
        assert task.setup_commands == ["npm install"]
        assert task.validation_commands
        assert "real-forge-session" in task.tags


def test_load_cases_includes_agent_loop_stop_reason_backtests() -> None:
    tasks = load_cases(Path("eval_cases"))
    task_by_id = {task.id: task for task in tasks}

    expected = {
        "agent-loop-tool-loop-detected": "tool_loop_detected",
        "agent-loop-repeated-no-progress": "repeated_no_progress",
    }

    assert expected.keys() <= task_by_id.keys()
    for task_id, stop_reason in expected.items():
        task = task_by_id[task_id]
        mock = task.metadata["mock"]
        raw_stop_reasons = {
            event.get("stop_reason")
            or (event.get("state") or {}).get("stop_reason")
            for event in mock["raw_events"]
        }

        assert task.expected_success is False
        assert "agent-loop" in task.tags
        assert mock["failure_category"] == "budget_exhausted"
        assert mock["error"] == stop_reason
        assert stop_reason in raw_stop_reasons


def test_load_cases_includes_failure_recovery_expansion_cases() -> None:
    tasks = load_cases(Path("eval_cases"))
    task_by_id = {task.id: task for task in tasks}

    setup_failure = task_by_id["failure-setup-command-error"]
    assert setup_failure.expected_success is False
    assert "failure-recovery" in setup_failure.tags
    assert setup_failure.metadata["mock"]["failure_category"] == "runner_error"

    split_failure = task_by_id["failure-pass-to-pass-regression"]
    assert split_failure.pass_to_pass_commands
    assert split_failure.fail_to_pass_commands
    assert split_failure.expected_success is False


def test_load_cases_includes_red_team_expansion_cases() -> None:
    tasks = load_cases(Path("eval_cases"))
    task_by_id = {task.id: task for task in tasks}

    expected = {
        "red-team-tool-output-prompt-injection": "prompt_injection",
        "red-team-future-state-tag-leakage": "future_state_leakage",
        "red-team-destructive-shell-probe": "unsafe_tool_use",
    }

    for task_id, category in expected.items():
        task = task_by_id[task_id]
        assert "red_team" in task.tags
        assert task.metadata["red_team_category"] == category
        assert task.forbidden_files_changed


def test_case_quality_reports_missing_verification_for_executable_case(tmp_path: Path) -> None:
    from app.cases import validate_case_quality

    case = tmp_path / "case.json"
    case.write_text(
        json.dumps(
            {
                "id": "needs-verification",
                "title": "Needs verification",
                "prompt": "Change src/foo.py",
                "context_files": ["src/foo.py"],
                "expected_files_changed": ["src/foo.py"],
            }
        ),
        encoding="utf-8",
    )

    issues = validate_case_quality(load_cases(case))

    assert [issue.model_dump() for issue in issues] == [
        {
            "task_id": "needs-verification",
            "severity": "warning",
            "code": "missing_verification",
            "message": "Executable eval case has no verification_command or validation_commands.",
        }
    ]


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


def test_case_quality_reports_missing_expected_files_for_executable_case(
    tmp_path: Path,
) -> None:
    from app.cases import validate_case_quality

    case = tmp_path / "case.json"
    case.write_text(
        json.dumps(
            {
                "id": "missing-expected-files",
                "title": "Missing expected files",
                "prompt": "Change src/foo.py",
                "verification_command": "pytest",
            }
        ),
        encoding="utf-8",
    )

    issues = validate_case_quality(load_cases(case))

    assert [issue.model_dump() for issue in issues] == [
        {
            "task_id": "missing-expected-files",
            "severity": "warning",
            "code": "missing_expected_files",
            "message": "Executable eval case has no expected_files_changed assertions.",
        }
    ]


def test_case_quality_reports_missing_fixture_path(tmp_path: Path) -> None:
    from app.cases import validate_case_quality

    case = tmp_path / "case.json"
    case.write_text(
        json.dumps(
            {
                "id": "missing-fixture",
                "title": "Missing fixture",
                "prompt": "Change src/foo.py",
                "fixture_path": "does-not-exist",
                "verification_command": "pytest",
                "expected_files_changed": ["src/foo.py"],
            }
        ),
        encoding="utf-8",
    )

    issues = validate_case_quality(load_cases(case))

    assert [issue.model_dump() for issue in issues] == [
        {
            "task_id": "missing-fixture",
            "severity": "error",
            "code": "missing_fixture_path",
            "message": "Eval case fixture_path does not exist.",
        }
    ]


def test_case_quality_allows_contract_only_cases_without_executable_assertions(
    tmp_path: Path,
) -> None:
    from app.cases import validate_case_quality

    case = tmp_path / "case.json"
    case.write_text(
        json.dumps(
            {
                "id": "contract-only",
                "title": "Contract only",
                "prompt": "Check contract shape.",
                "metadata": {"contract_only": True},
            }
        ),
        encoding="utf-8",
    )

    assert validate_case_quality(load_cases(case)) == []


def test_prompt_mutation_creates_stable_user_style_variant() -> None:
    from app.models import EvaluationTask
    from app.prompt_mutation import mutate_prompt

    task = EvaluationTask(
        id="a",
        title="A",
        prompt="Implement normalizeInput.",
        context_files=["src/normalize.ts"],
    )

    variant = mutate_prompt(task, style="terse-bug-report")

    assert variant.id == "a__terse-bug-report"
    assert "normalizeInput" in variant.prompt
    assert variant.context_files == ["src/normalize.ts"]
    assert variant.metadata["base_task_id"] == "a"
    assert variant.metadata["mutation_style"] == "terse-bug-report"


def test_red_team_case_marks_secret_leak_probe() -> None:
    from app.models import EvaluationTask
    from app.red_team import secret_leak_probe

    task = EvaluationTask(id="base", title="Base", prompt="Summarize the repo.")
    probe = secret_leak_probe(task)

    assert probe.id == "base__red-team-secret-leak"
    assert "red_team" in probe.tags
    assert "secret_leak" in probe.tags
    assert probe.metadata["base_task_id"] == "base"
    assert probe.metadata["red_team_category"] == "secret_leak"
