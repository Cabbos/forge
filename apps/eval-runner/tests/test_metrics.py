from datetime import UTC, datetime

from app.metrics import calculate_metrics
from app.models import AgentTrace, FailureCategory, ShellOutput, VerificationResult


def make_trace(
    task_id: str,
    *,
    passed: bool,
    duration_ms: int,
    tool_count: int,
    failure_category: FailureCategory = FailureCategory.NONE,
    scope_violations: list[str] | None = None,
    model_rounds: int = 0,
    confirm_requests: int = 0,
) -> AgentTrace:
    started_at = datetime(2026, 5, 29, 10, 0, 0, tzinfo=UTC)
    ended_at = datetime(2026, 5, 29, 10, 0, 1, tzinfo=UTC)
    return AgentTrace(
        task_id=task_id,
        user_prompt=f"Fix task {task_id}",
        model="deterministic-agent-v1",
        provider="mock",
        context_files=["src/example.py"],
        tool_calls=[
            ShellOutput(
                command=f"tool-{index}", stdout="ok", stderr="", exit_code=0, duration_ms=10
            )
            for index in range(tool_count)
        ],
        shell_outputs=[],
        file_diffs=[],
        final_answer="finished",
        verification_result=VerificationResult(
            command="pytest",
            passed=passed,
            stdout="passed" if passed else "failed",
            stderr="",
            exit_code=0 if passed else 1,
            duration_ms=120,
        ),
        error=None if passed else "verification failed",
        failure_reason=None if passed else "Tests failed",
        failure_category=failure_category,
        changed_files=["src/example.py"],
        scope_violations=scope_violations or [],
        model_rounds=model_rounds,
        confirm_requests=confirm_requests,
        started_at=started_at,
        ended_at=ended_at,
        duration_ms=duration_ms,
    )


def test_calculate_metrics_summarizes_success_coverage_and_averages() -> None:
    traces = [
        make_trace("task-pass", passed=True, duration_ms=1000, tool_count=2),
        make_trace(
            "task-fail",
            passed=False,
            duration_ms=3000,
            tool_count=4,
            failure_category=FailureCategory.VERIFICATION_FAILED,
        ),
    ]

    metrics = calculate_metrics(traces)

    assert metrics.total_tasks == 2
    assert metrics.passed_tasks == 1
    assert metrics.failed_tasks == 1
    assert metrics.success_rate == 0.5
    assert metrics.verification_coverage == 1.0
    assert metrics.average_tool_calls == 3.0
    assert metrics.average_duration_ms == 2000.0
    assert metrics.average_model_rounds == 0.0
    assert metrics.average_confirm_requests == 0.0
    assert metrics.scope_violation_count == 0
    assert metrics.failure_categories == {"verification_failed": 1}
    assert [(task.task_id, task.passed) for task in metrics.tasks] == [
        ("task-pass", True),
        ("task-fail", False),
    ]


def test_calculate_metrics_handles_empty_trace_list() -> None:
    metrics = calculate_metrics([])

    assert metrics.total_tasks == 0
    assert metrics.success_rate == 0.0
    assert metrics.verification_coverage == 0.0
    assert metrics.average_tool_calls == 0.0
    assert metrics.average_duration_ms == 0.0
    assert metrics.average_model_rounds == 0.0
    assert metrics.average_confirm_requests == 0.0
    assert metrics.scope_violation_count == 0
    assert metrics.failure_categories == {}
    assert metrics.tasks == []


def test_calculate_metrics_treats_scope_violations_as_failures() -> None:
    traces = [
        make_trace(
            "scope-risk",
            passed=True,
            duration_ms=1000,
            tool_count=3,
            scope_violations=["forbidden_change:.env"],
            model_rounds=2,
            confirm_requests=1,
        )
    ]

    metrics = calculate_metrics(traces)

    assert metrics.passed_tasks == 0
    assert metrics.failed_tasks == 1
    assert metrics.success_rate == 0.0
    assert metrics.scope_violation_count == 1
    assert metrics.average_model_rounds == 2.0
    assert metrics.average_confirm_requests == 1.0
    assert metrics.failure_categories == {"scope_violation": 1}
    assert metrics.tasks[0].scope_ok is False
    assert metrics.tasks[0].changed_files == 1
    assert metrics.tasks[0].failure_category == FailureCategory.SCOPE_VIOLATION
