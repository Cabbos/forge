from decimal import Decimal
from types import SimpleNamespace

from app.holdout_diagnostics import summarize_holdout_artifacts
from app.models import EvaluationTask, TrustGateResult, TrustStatus, VerificationResult
from tests.test_baseline import make_trace


def _artifact(
    task_id: str,
    repetition: int,
    *,
    rounds: int,
    duration_ms: int,
    cost_cny: str | None,
    passed: bool,
    trusted: bool,
    scope_violations: list[str] | None = None,
    provider_error: dict[str, object] | None = None,
) -> SimpleNamespace:
    task = EvaluationTask(id=task_id, title=task_id, prompt="Run the task.")
    trace = make_trace(task).model_copy(
        update={
            "model_rounds": rounds,
            "duration_ms": duration_ms,
            "scope_violations": scope_violations or [],
            "verification_result": VerificationResult(
                command="verify", passed=passed, exit_code=0 if passed else 1
            ),
            "raw_events": ([provider_error] if provider_error else []),
            "error": None if passed else "HTTP 429: Bearer secret-must-not-escape",
        }
    )
    return SimpleNamespace(
        task_id=task_id,
        repetition=repetition,
        trace=trace,
        usage=SimpleNamespace(cost_cny=Decimal(cost_cny) if cost_cny else None),
        trust_result=TrustGateResult(
            status=TrustStatus.TRUSTED if trusted else TrustStatus.UNTRUSTED,
            trusted=trusted,
            blockers=[] if trusted else ["workspace_unavailable"],
        ),
        primary_failure_category="verification_failed",
    )


def test_holdout_diagnostics_are_bounded_and_case_sorted() -> None:
    artifacts = [
        _artifact(
            "case-b",
            1,
            rounds=8,
            duration_ms=300,
            cost_cny="0.20",
            passed=True,
            trusted=True,
        ),
        _artifact(
            "case-a",
            2,
            rounds=5,
            duration_ms=100,
            cost_cny=None,
            passed=False,
            trusted=False,
            scope_violations=["outside_scope"],
            provider_error={
                "event_type": "provider_error",
                "error": "HTTP 429: Bearer secret-must-not-escape",
            },
        ),
        _artifact(
            "case-a",
            1,
            rounds=3,
            duration_ms=200,
            cost_cny="0.10",
            passed=True,
            trusted=True,
        ),
    ]

    diagnostics = summarize_holdout_artifacts(artifacts)

    assert diagnostics.attempts == 3
    assert diagnostics.cost_known_attempts == 2
    assert diagnostics.cost_known_rate == 2 / 3
    assert diagnostics.p95_model_rounds == 8
    assert diagnostics.p95_duration_ms == 300
    assert diagnostics.median_cost_cny == Decimal("0.15")
    assert diagnostics.provider_errors == {"rate_limit": 1}
    assert diagnostics.failure_categories == {"verification_failed": 1}
    assert diagnostics.trust_blockers == {"workspace_unavailable": 1}
    assert [case.case_id for case in diagnostics.cases] == ["case-a", "case-b"]
    assert diagnostics.cases[0].attempts == 2
    assert diagnostics.cases[0].trusted_attempts == 1
    assert diagnostics.cases[0].scope_violation_rate == 0.5
    assert diagnostics.cases[1].trusted_success_rate == 1.0

    serialized = diagnostics.model_dump_json()
    assert "secret-must-not-escape" not in serialized
    assert "Bearer" not in serialized
