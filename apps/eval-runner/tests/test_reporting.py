from datetime import UTC, datetime

from app.models import AgentTrace, FailureCategory, ShellOutput, VerificationResult
from app.reporting import build_report


def make_trace(
    task_id: str,
    *,
    verification_passed: bool | None,
    duration_ms: int,
    model_rounds: int,
    confirm_requests: int,
    failure_category: FailureCategory = FailureCategory.NONE,
    scope_violations: list[str] | None = None,
) -> AgentTrace:
    started_at = datetime(2026, 6, 4, 10, 0, 0, tzinfo=UTC)
    ended_at = datetime(2026, 6, 4, 10, 0, 1, tzinfo=UTC)
    return AgentTrace(
        task_id=task_id,
        user_prompt=f"Run {task_id}",
        model="deterministic-agent-v1",
        provider="mock",
        context_files=["src/app.py"],
        tool_calls=[ShellOutput(command="read_context"), ShellOutput(command="edit_files")],
        shell_outputs=[],
        file_diffs=[],
        changed_files=["src/app.py"],
        scope_violations=scope_violations or [],
        final_answer="done",
        verification_result=None
        if verification_passed is None
        else VerificationResult(
            command="pytest",
            passed=verification_passed,
            stdout="passed" if verification_passed else "failed",
            stderr="",
            exit_code=0 if verification_passed else 1,
            duration_ms=120,
        ),
        error=None if verification_passed else failure_category.value,
        failure_reason=None if verification_passed else "Task failed.",
        failure_category=failure_category,
        model_rounds=model_rounds,
        confirm_requests=confirm_requests,
        started_at=started_at,
        ended_at=ended_at,
        duration_ms=duration_ms,
    )


def test_build_report_outputs_backtest_rates_and_trace_summaries() -> None:
    report = build_report(
        [
            make_trace(
                "small-edit-success",
                verification_passed=True,
                duration_ms=1000,
                model_rounds=2,
                confirm_requests=0,
            ),
            make_trace(
                "forbidden-file-change",
                verification_passed=True,
                duration_ms=3000,
                model_rounds=4,
                confirm_requests=2,
                failure_category=FailureCategory.SCOPE_VIOLATION,
                scope_violations=["forbidden_change:.env"],
            ),
            make_trace(
                "validation-failure",
                verification_passed=False,
                duration_ms=5000,
                model_rounds=1,
                confirm_requests=1,
                failure_category=FailureCategory.VERIFICATION_FAILED,
            ),
        ]
    )

    assert report.total_tasks == 3
    assert report.success_rate == 1 / 3
    assert report.verification_pass_rate == 2 / 3
    assert report.scope_violation_rate == 1 / 3
    assert report.avg_duration_ms == 3000.0
    assert report.avg_model_rounds == 7 / 3
    assert report.avg_confirm_requests == 1.0
    assert report.failure_categories == {"scope_violation": 1, "verification_failed": 1}
    assert report.tasks[0].task_id == "small-edit-success"
    assert report.tasks[0].passed is True
    assert report.tasks[1].scope_violations == ["forbidden_change:.env"]
    assert report.tasks[2].failure_reason == "Task failed."
