import json
from pathlib import Path

import pytest

from app.cases import load_cases
from app.execution import ExecutionOptions, execute_evaluation
from app.models import EvalProvider, TrustStatus


def write_executable_case(
    tmp_path: Path,
    *,
    required_scores: list[str] | None = None,
    missing_fixture: bool = False,
) -> Path:
    cases = tmp_path / "cases"
    case_dir = cases / "case-1"
    fixture = case_dir / "fixture"
    case_dir.mkdir(parents=True)
    if not missing_fixture:
        fixture.mkdir()
        (fixture / "app.py").write_text("before\n", encoding="utf-8")
    (case_dir / "case.json").write_text(
        json.dumps(
            {
                "task": {
                    "id": "case-1",
                    "title": "Case 1",
                    "prompt": "Update app.py.",
                    "fixture_path": "fixture",
                    "context_files": ["app.py"],
                    "verification_command": "python -c 'print(1)'",
                    "expected_files_changed": ["app.py"],
                    "required_scores": required_scores or [],
                }
            }
        ),
        encoding="utf-8",
    )
    return cases


def mock_options() -> ExecutionOptions:
    return ExecutionOptions(
        provider=EvalProvider.MOCK,
        model="deterministic-agent-v1",
        forge_command=None,
        command_timeout_seconds=2.0,
        setup_timeout_seconds=2.0,
        validation_timeout_seconds=2.0,
        require_red_team=False,
    )


def test_execute_evaluation_returns_trusted_for_complete_mock_evidence(
    tmp_path: Path,
) -> None:
    cases_path = write_executable_case(tmp_path)
    execution = execute_evaluation(
        cases_path=cases_path,
        tasks=load_cases(cases_path),
        options=mock_options(),
    )

    assert execution.trust_result.status == TrustStatus.TRUSTED
    assert execution.trust_result.trusted is True


def test_execute_evaluation_is_unknown_when_workspace_evidence_is_missing(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    cases_path = write_executable_case(tmp_path)
    task = load_cases(cases_path)[0]
    monkeypatch.setattr(
        "app.execution.execute_tasks",
        lambda tasks, options, cancel_requested: [
            __import__("app.runner", fromlist=["DeterministicMockRunner"])
            .DeterministicMockRunner()
            .run_task(task)
            .model_copy(update={"workspace_observation": None})
        ],
    )

    execution = execute_evaluation(
        cases_path=cases_path,
        tasks=[task],
        options=mock_options(),
    )

    assert execution.trust_result.status == TrustStatus.UNKNOWN
    assert "workspace_evidence_missing:case-1" in execution.trust_result.blockers


def test_execute_evaluation_is_untrusted_for_case_quality_error(tmp_path: Path) -> None:
    cases_path = write_executable_case(tmp_path, missing_fixture=True)
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
    assert (
        "score_coverage_incomplete:forge_file_effects_evidence_ok"
        in execution.trust_result.blockers
    )
