from decimal import Decimal

import pytest
from pydantic import ValidationError

from app.holdout_gate import (
    DEFAULT_HOLDOUT_THRESHOLDS,
    HoldoutAttemptEvidenceV1,
    HoldoutGateReportV1,
    evaluate_holdout_gate,
    holdout_report_bytes,
    holdout_report_fingerprint,
    summarize_holdout_attempts,
)


def valid_holdout_report_payload() -> dict[str, object]:
    return {
        "schema_version": 1,
        "campaign_id": "deepseek-holdout-v1",
        "dataset_split": "holdout",
        "dataset_fingerprint": "a" * 64,
        "harness_fingerprint": "b" * 64,
        "profile_id": "deepseek-v4-flash-anthropic-minimal-v1",
        "source_commit": "c" * 40,
        "price_table_fingerprint": "d" * 64,
        "provider": "deepseek",
        "model": "deepseek-v4-flash",
        "transport": "anthropic_messages",
        "case_count": 20,
        "repetitions": 3,
        "expected_attempts": 60,
        "completed_attempts": 60,
        "trusted_attempts": 60,
        "trusted_success_rate": 0.85,
        "scope_violation_rate": 0.0,
        "median_model_rounds": 10.0,
        "score_coverage": 1.0,
        "contract_coverage": 1.0,
        "workspace_evidence_rate": 1.0,
        "patch_replay_rate": 1.0,
        "verification_evidence_rate": 1.0,
        "usage_known_rate": 1.0,
        "terminal_failure_category_rate": 1.0,
        "total_cost_cny": "31.20",
        "budget_cny": "35",
        "status": "complete",
        "blockers": [],
    }


def test_holdout_gate_requires_all_60_runs_and_evidence() -> None:
    report = HoldoutGateReportV1.model_validate(valid_holdout_report_payload())

    decision = evaluate_holdout_gate(report)

    assert decision.passed is True
    assert decision.blockers == []
    assert decision.thresholds == DEFAULT_HOLDOUT_THRESHOLDS
    assert holdout_report_bytes(report).endswith(b"\n")
    assert len(holdout_report_fingerprint(report)) == 64


@pytest.mark.parametrize(
    ("field", "value", "blocker"),
    [
        ("completed_attempts", 59, "holdout_attempt_count_incomplete"),
        ("trusted_success_rate", 0.79, "holdout_success_rate_below_threshold"),
        ("scope_violation_rate", 0.021, "holdout_scope_rate_above_threshold"),
        ("median_model_rounds", 12.1, "holdout_model_rounds_above_threshold"),
        ("score_coverage", 0.99, "holdout_score_coverage_incomplete"),
        ("contract_coverage", 0.99, "holdout_contract_coverage_incomplete"),
        ("workspace_evidence_rate", 0.99, "holdout_workspace_evidence_incomplete"),
        ("patch_replay_rate", 0.99, "holdout_patch_replay_incomplete"),
        ("verification_evidence_rate", 0.99, "holdout_verification_evidence_incomplete"),
        ("usage_known_rate", 0.99, "holdout_usage_accounting_incomplete"),
        ("terminal_failure_category_rate", 0.99, "holdout_failure_taxonomy_incomplete"),
        ("total_cost_cny", "35.01", "holdout_budget_exceeded"),
    ],
)
def test_holdout_gate_reports_each_release_blocker(field: str, value: object, blocker: str) -> None:
    payload = valid_holdout_report_payload()
    payload[field] = value
    if field == "completed_attempts":
        payload["trusted_attempts"] = 59
    report = HoldoutGateReportV1.model_validate(payload)

    decision = evaluate_holdout_gate(report)

    assert decision.passed is False
    assert blocker in decision.blockers


def test_holdout_gate_rejects_non_locked_thresholds() -> None:
    with pytest.raises(ValidationError):
        HoldoutGateReportV1.model_validate(
            {
                **valid_holdout_report_payload(),
                "thresholds": {"minimum_trusted_success_rate": 0.7},
            }
        )


def test_holdout_report_rejects_secret_material() -> None:
    payload = valid_holdout_report_payload()
    payload["api_key"] = "sk-secret-value"

    with pytest.raises(ValidationError):
        HoldoutGateReportV1.model_validate(payload)


def test_holdout_report_cost_is_decimal_and_canonical() -> None:
    report = HoldoutGateReportV1.model_validate(valid_holdout_report_payload())

    assert report.total_cost_cny == Decimal("31.20")
    assert b'"total_cost_cny":"31.20"' in holdout_report_bytes(report)


def _attempts(*, missing_terminal_category: bool = False) -> list[HoldoutAttemptEvidenceV1]:
    attempts: list[HoldoutAttemptEvidenceV1] = []
    for repetition in range(1, 4):
        for case_number in range(1, 21):
            passed = not (
                (repetition == 1 and case_number > 8) or (repetition == 3 and case_number == 20)
            )
            attempts.append(
                HoldoutAttemptEvidenceV1(
                    attempt_id=f"r{repetition}-case{case_number}",
                    case_id=f"case-{case_number}",
                    repetition=repetition,
                    trusted=not (repetition == 3 and case_number == 20),
                    passed=passed,
                    scope_violation=False,
                    model_rounds=10,
                    score_coverage=True,
                    contract_evidence=True,
                    workspace_evidence=True,
                    patch_replay=True,
                    verification_evidence=True,
                    usage_known=True,
                    terminal_failure_category=(
                        None
                        if missing_terminal_category
                        else ("passed" if passed else "verification_failed")
                    ),
                    cost_cny=Decimal("0.52"),
                )
            )
    return attempts


def test_holdout_summary_is_derived_from_unique_attempt_evidence() -> None:
    report = summarize_holdout_attempts(
        _attempts(),
        campaign_id="derived-holdout-v1",
        case_ids=[f"case-{number}" for number in range(1, 21)],
        dataset_fingerprint="a" * 64,
        harness_fingerprint="b" * 64,
        profile_id="deepseek-v4-flash-anthropic-minimal-v1",
        source_commit="c" * 40,
        price_table_fingerprint="d" * 64,
        transport="anthropic_messages",
        budget_cny=Decimal("35"),
    )

    assert report.completed_attempts == 60
    assert report.trusted_attempts == 59
    assert report.trusted_success_rate == pytest.approx(47 / 60)
    assert report.total_cost_cny == Decimal("31.20")
    assert "holdout_trusted_attempts_incomplete" in evaluate_holdout_gate(report).blockers


def test_holdout_summary_rejects_duplicate_or_missing_case_repetitions() -> None:
    attempts = _attempts()
    attempts[-1] = attempts[-2].model_copy(update={"attempt_id": "duplicate"})

    with pytest.raises(ValueError, match="unique case/repetition"):
        summarize_holdout_attempts(
            attempts,
            campaign_id="derived-holdout-v1",
            case_ids=[f"case-{number}" for number in range(1, 21)],
            dataset_fingerprint="a" * 64,
            harness_fingerprint="b" * 64,
            profile_id="deepseek-v4-flash-anthropic-minimal-v1",
            source_commit="c" * 40,
            price_table_fingerprint="d" * 64,
            transport="anthropic_messages",
            budget_cny=Decimal("35"),
        )


def test_holdout_summary_preserves_missing_terminal_failure_category() -> None:
    report = summarize_holdout_attempts(
        _attempts(missing_terminal_category=True),
        campaign_id="derived-holdout-v1",
        case_ids=[f"case-{number}" for number in range(1, 21)],
        dataset_fingerprint="a" * 64,
        harness_fingerprint="b" * 64,
        profile_id="deepseek-v4-flash-anthropic-minimal-v1",
        source_commit="c" * 40,
        price_table_fingerprint="d" * 64,
        transport="anthropic_messages",
        budget_cny=Decimal("35"),
    )

    assert report.terminal_failure_category_rate == 0.0
    assert "holdout_failure_taxonomy_incomplete" in evaluate_holdout_gate(report).blockers
