from datetime import UTC, datetime

import pytest
from pydantic import ValidationError

from app.models import (
    AgentTrace,
    BacktestReport,
    FailureCategory,
    ScoreCoverage,
    ShellOutput,
    TaskMetric,
    TrustGateResult,
    TrustStatus,
    VerificationResult,
)
from app.reporting import build_report


def test_trust_result_rejects_inconsistent_status_and_boolean() -> None:
    with pytest.raises(ValidationError):
        TrustGateResult(status=TrustStatus.TRUSTED, trusted=False)


def test_trust_result_infers_trusted_status_for_legacy_constructor() -> None:
    result = TrustGateResult(trusted=True)

    assert result.status == TrustStatus.TRUSTED


def test_score_coverage_requires_consistent_counts() -> None:
    with pytest.raises(ValidationError):
        ScoreCoverage(mean=1.0, observed=2, expected=1, coverage=1.0)


def test_backtest_report_defaults_to_unknown_trust_and_empty_score_coverage() -> None:
    report = BacktestReport(
        total_tasks=0,
        success_rate=0.0,
        verification_pass_rate=0.0,
        scope_violation_rate=0.0,
        avg_duration_ms=0.0,
        avg_model_rounds=0.0,
        avg_confirm_requests=0.0,
    )

    assert report.score_coverage == {}
    assert report.trust_result.status == TrustStatus.UNKNOWN


def test_trust_gates_fail_closed_without_harness_check() -> None:
    from app.trust_gates import evaluate_trust_gates

    result = evaluate_trust_gates(
        harness_ok=False,
        dataset_fingerprint="abc",
        scorer_calibrated=True,
        red_team_passed=True,
    )

    assert result.trusted is False
    assert result.blockers == ["harness_untrusted"]


def test_trust_gates_fail_closed_without_dataset_fingerprint() -> None:
    from app.trust_gates import evaluate_trust_gates

    result = evaluate_trust_gates(
        harness_ok=True,
        dataset_fingerprint=None,
        scorer_calibrated=True,
        red_team_passed=True,
    )

    assert result.trusted is False
    assert result.blockers == ["dataset_unfingerprinted"]


def test_trust_gates_fail_closed_without_scorer_calibration() -> None:
    from app.trust_gates import evaluate_trust_gates

    result = evaluate_trust_gates(
        harness_ok=True,
        dataset_fingerprint="abc",
        scorer_calibrated=False,
        red_team_passed=True,
    )

    assert result.trusted is False
    assert result.blockers == ["scorer_uncalibrated"]


def test_trust_gates_fail_closed_when_red_team_fails() -> None:
    from app.trust_gates import evaluate_trust_gates

    result = evaluate_trust_gates(
        harness_ok=True,
        dataset_fingerprint="abc",
        scorer_calibrated=True,
        red_team_passed=False,
    )

    assert result.trusted is False
    assert result.blockers == ["red_team_failed"]


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
    cost_usd: float | None = None,
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
        cost_usd=cost_usd,
        started_at=started_at,
        ended_at=ended_at,
        duration_ms=duration_ms,
    )


def make_report(**overrides) -> BacktestReport:
    report = BacktestReport(
        total_tasks=2,
        success_rate=1.0,
        verification_pass_rate=1.0,
        scope_violation_rate=0.0,
        avg_duration_ms=100.0,
        avg_model_rounds=2.0,
        avg_confirm_requests=0.0,
    )
    return report.model_copy(update=overrides)


def test_compare_reports_flags_success_rate_regression() -> None:
    from app.report_compare import compare_reports

    previous = make_report()
    current = previous.model_copy(update={"success_rate": 0.0})

    result = compare_reports(previous, current)

    assert result["regressions"][0]["metric"] == "success_rate"
    assert result["regressions"][0]["severity"] == "critical"


def test_compare_reports_flags_scope_violation_regression() -> None:
    from app.report_compare import compare_reports

    result = compare_reports(
        make_report(scope_violation_rate=0.0),
        make_report(scope_violation_rate=0.75),
    )

    assert result["regressions"][0]["metric"] == "scope_violation_rate"
    assert result["regressions"][0]["severity"] == "critical"


def test_compare_reports_flags_model_round_warning() -> None:
    from app.report_compare import compare_reports

    result = compare_reports(
        make_report(avg_model_rounds=2.0),
        make_report(avg_model_rounds=5.0),
    )

    assert result["regressions"][0]["metric"] == "avg_model_rounds"
    assert result["regressions"][0]["severity"] == "warning"


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
    assert report.score_summary["functional_correctness"] == 1 / 3
    assert report.score_summary["scope_ok"] == 2 / 3
    assert report.tasks[0].task_id == "small-edit-success"
    assert report.tasks[0].passed is True
    assert report.tasks[1].scope_violations == ["forbidden_change:.env"]
    assert report.tasks[2].failure_reason == "Task failed."


def test_build_report_sums_total_cost_usd() -> None:
    report = build_report(
        [
            make_trace(
                "cost-a",
                verification_passed=True,
                duration_ms=1000,
                model_rounds=2,
                confirm_requests=0,
                cost_usd=0.05,
            ),
            make_trace(
                "cost-b",
                verification_passed=True,
                duration_ms=1000,
                model_rounds=2,
                confirm_requests=0,
                cost_usd=0.15,
            ),
        ]
    )

    assert report.total_cost_usd == 0.2


def test_trial_aggregation_marks_flaky_task() -> None:
    from app.reporting import aggregate_trial_metrics

    trials = [
        TaskMetric(
            task_id="a",
            passed=True,
            verification_passed=True,
            tool_calls=1,
            duration_ms=10,
            failure_category=FailureCategory.NONE,
        ),
        TaskMetric(
            task_id="a",
            passed=False,
            verification_passed=False,
            tool_calls=1,
            duration_ms=12,
            failure_category=FailureCategory.VERIFICATION_FAILED,
        ),
    ]

    result = aggregate_trial_metrics(trials)

    assert result["a"]["attempts"] == 2
    assert result["a"]["pass_rate"] == 0.5
    assert result["a"]["flaky"] is True


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
