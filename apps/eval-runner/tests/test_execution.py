import json
from dataclasses import replace
from pathlib import Path

import pytest

from app.cases import load_cases
from app.execution import ExecutionOptions, execute_evaluation
from app.harness_contracts import HarnessPolicyV1
from app.models import EvalProvider, TrustStatus, WorkspaceCheck
from app.provider_fallback import named_provider_chain
from app.runner import DeterministicMockRunner


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


def test_execute_tasks_wires_named_provider_chain_into_development_runner(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    from app.execution import execute_tasks

    cases_path = write_executable_case(tmp_path)
    tasks = load_cases(cases_path)
    chain = named_provider_chain("kimi-k3-deepseek")
    seen: dict[str, object] = {}

    def build_chain(received_chain, **kwargs):
        seen["chain"] = received_chain
        seen.update(kwargs)
        return DeterministicMockRunner()

    monkeypatch.setattr("app.execution.build_provider_fallback_runner", build_chain)
    options = replace(
        mock_options(),
        provider=EvalProvider.FORGE,
        model="k3",
        forge_command=["forge-headless"],
        provider_chain=chain,
        max_model_rounds=12,
        harness_policy=HarnessPolicyV1(context_policy="target_files_v1"),
    )

    traces = execute_tasks(tasks, options, lambda: False)

    assert len(traces) == 1
    assert seen["chain"] == chain
    assert seen["command"] == ["forge-headless"]
    assert seen["max_model_rounds"] == 12
    assert seen["harness_policy"].context_policy == "target_files_v1"


def test_execute_tasks_wires_single_provider_route_into_development_runner(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    from app.execution import execute_tasks

    cases_path = write_executable_case(tmp_path)
    tasks = load_cases(cases_path)
    route = named_provider_chain("kimi-k3-deepseek").primary
    seen: dict[str, object] = {}

    def build_route(received_route, **kwargs):
        seen["route"] = received_route
        seen.update(kwargs)
        return DeterministicMockRunner()

    monkeypatch.setattr("app.execution.build_provider_route_runner", build_route)
    options = replace(
        mock_options(),
        provider=EvalProvider.FORGE,
        model=route.model,
        forge_command=["forge-headless"],
        provider_route=route,
        max_model_rounds=12,
    )

    traces = execute_tasks(tasks, options, lambda: False)

    assert len(traces) == 1
    assert seen["route"] == route
    assert seen["command"] == ["forge-headless"]
    assert seen["max_model_rounds"] == 12


def test_execute_evaluation_stops_before_next_call_at_cost_boundary(
    tmp_path: Path,
) -> None:
    cases_path = write_executable_case(tmp_path)
    task = load_cases(cases_path)[0]
    first = task.model_copy(update={"metadata": {"mock": {"cost_usd": 1.0}}})
    second = task.model_copy(update={"id": "case-2", "metadata": {"mock": {"cost_usd": 0.1}}})

    execution = execute_evaluation(
        cases_path=cases_path,
        tasks=[first, second],
        options=replace(mock_options(), max_total_cost_usd=1.0),
    )

    assert [trace.task_id for trace in execution.traces] == ["case-1"]
    assert execution.report.total_cost_usd == 1.0
    assert execution.trust_result.status == TrustStatus.UNKNOWN
    assert "execution_stopped_at_cost_boundary" in execution.trust_result.blockers


def test_execute_evaluation_stops_after_unknown_cost_when_required(
    tmp_path: Path,
) -> None:
    cases_path = write_executable_case(tmp_path)
    task = load_cases(cases_path)[0]
    first = task
    second = task.model_copy(update={"id": "case-2", "metadata": {"mock": {"cost_usd": 0.1}}})

    execution = execute_evaluation(
        cases_path=cases_path,
        tasks=[first, second],
        options=replace(mock_options(), require_known_cost=True),
    )

    assert [trace.task_id for trace in execution.traces] == ["case-1"]
    assert execution.report.provider_cost_known_rate == 0.0
    assert execution.trust_result.status == TrustStatus.UNKNOWN
    assert "execution_stopped_at_cost_boundary" in execution.trust_result.blockers


def test_execute_evaluation_is_untrusted_when_observer_replay_self_check_fails(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    cases_path = write_executable_case(tmp_path)
    monkeypatch.setattr(
        "app.harness_checks.run_observer_patch_replay_self_check",
        lambda: WorkspaceCheck(ok=False, message="replay mismatch"),
    )

    execution = execute_evaluation(
        cases_path=cases_path,
        tasks=load_cases(cases_path),
        options=mock_options(),
    )

    assert execution.trust_result.status == TrustStatus.UNTRUSTED
    assert "harness_untrusted" in execution.trust_result.blockers


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
