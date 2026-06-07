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
    repair_attempts_used: int = 0,
    validation_attempts: int = 0,
    failure_category: FailureCategory = FailureCategory.NONE,
    scope_violations: list[str] | None = None,
    raw_events: list[dict] | None = None,
) -> AgentTrace:
    started_at = datetime(2026, 6, 4, 10, 0, 0, tzinfo=UTC)
    ended_at = datetime(2026, 6, 4, 10, 0, 1, tzinfo=UTC)
    return AgentTrace(
        task_id=task_id,
        user_prompt=f"Run {task_id}",
        model="deterministic-agent-v1",
        provider="mock",
        context_files=["src/app.py"],
        raw_events=raw_events or [],
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
        repair_attempts_used=repair_attempts_used,
        validation_attempts=validation_attempts,
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
    assert report.avg_repair_attempts_used == 0.0
    assert report.avg_validation_attempts == 0.0
    assert report.failure_categories == {"scope_violation": 1, "verification_failed": 1}
    assert report.tasks[0].task_id == "small-edit-success"
    assert report.tasks[0].passed is True
    assert report.tasks[1].scope_violations == ["forbidden_change:.env"]
    assert report.tasks[2].failure_reason == "Task failed."


def test_build_report_summarizes_continuity_benefit_diagnostics() -> None:
    report = build_report(
        [
            make_trace(
                "continuity-rich-task",
                verification_passed=True,
                duration_ms=1000,
                model_rounds=2,
                confirm_requests=0,
                raw_events=[
                    {
                        "event_type": "eval_headless_continuity_diagnostic",
                        "formed_count": 2,
                        "error": None,
                    },
                    {
                        "event_type": "eval_continuity_db_diagnostic",
                        "exists": True,
                        "experience_count": 3,
                        "fts_count": 3,
                        "formed_reflection_count": 2,
                        "experience_status_counts": {"candidate": 2, "accepted": 1},
                        "experience_kind_counts": {"workflow": 2, "lesson": 1},
                        "reflection_episodes": [
                            {
                                "user_goal_summary": "Add CSV export",
                                "changed_files": ["src/export.ts"],
                                "notable_failures": [],
                            },
                            {
                                "user_goal_summary": "Verify CSV export",
                                "changed_files": ["src/export.test.ts"],
                                "notable_failures": [
                                    {"tool_name": "shell", "summary": "first test failed"}
                                ],
                            },
                        ],
                    },
                ],
            ),
            make_trace(
                "continuity-empty-task",
                verification_passed=True,
                duration_ms=1000,
                model_rounds=2,
                confirm_requests=0,
                raw_events=[
                    {
                        "event_type": "eval_headless_continuity_diagnostic",
                        "formed_count": 0,
                        "error": None,
                    },
                    {
                        "event_type": "eval_continuity_db_diagnostic",
                        "exists": True,
                        "experience_count": 0,
                        "fts_count": 0,
                        "formed_reflection_count": 0,
                        "experience_status_counts": {},
                        "experience_kind_counts": {},
                        "reflection_episodes": [],
                    },
                ],
            ),
        ]
    )

    assert report.continuity is not None
    assert report.continuity.tasks_with_db == 2
    assert report.continuity.tasks_with_formed_experiences == 1
    assert report.continuity.formation_rate == 0.5
    assert report.continuity.recall_ready_rate == 0.5
    assert report.continuity.total_experiences == 3
    assert report.continuity.total_fts_rows == 3
    assert report.continuity.total_formed_reflections == 2
    assert report.continuity.total_reflection_episodes == 2
    assert report.continuity.reflection_episode_rate == 0.5
    assert report.continuity.reflection_coverage_rate == 1.0
    assert report.continuity.notable_failure_count == 1
    assert report.continuity.experience_status_counts == {"candidate": 2, "accepted": 1}
    assert report.continuity.experience_kind_counts == {"workflow": 2, "lesson": 1}
