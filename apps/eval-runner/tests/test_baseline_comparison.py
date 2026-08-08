import json
from decimal import Decimal
from pathlib import Path
from types import SimpleNamespace

import pytest

from app.baseline import (
    generate_baseline_report,
    initialize_baseline_campaign,
    run_baseline_campaign,
)
from app.baseline_comparison import (
    BaselineComparisonError,
    _build_metrics,
    _select_winner,
    compare_baseline_campaigns,
    write_baseline_comparison,
)
from app.baseline_models import BaselineComparisonMetrics, BootstrapInterval
from app.harness_profiles import harness_profile_bytes, load_harness_profile
from app.models import EvaluationTask
from tests.test_baseline import FakeRunner, make_trace

REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
FORGEBENCH_ROOT = Path(__file__).resolve().parents[1] / "forgebench"
DEVELOPMENT_CASES = FORGEBENCH_ROOT / "cases" / "development"
DATASET_LOCK = FORGEBENCH_ROOT / "dataset-lock.json"
CONTROL_PROFILE = FORGEBENCH_ROOT / "profiles" / "deepseek-v4-flash-anthropic-baseline-v1.json"
CANDIDATE_PROFILE = FORGEBENCH_ROOT / "profiles" / "deepseek-v4-flash-openai-chat-control-v1.json"
MINIMAL_PROFILE = FORGEBENCH_ROOT / "profiles" / "deepseek-v4-flash-anthropic-minimal-v1.json"
PLANNING_PROFILE = FORGEBENCH_ROOT / "profiles" / "deepseek-v4-flash-anthropic-planning-v1.json"


def _campaign(
    tmp_path: Path,
    name: str,
    profile_path: Path,
    *,
    runner: FakeRunner | None = None,
) -> Path:
    campaign = tmp_path / name
    initialize_baseline_campaign(
        campaign,
        campaign_id=name,
        repository_root=REPOSITORY_ROOT,
        cases_path=DEVELOPMENT_CASES,
        dataset_lock_path=DATASET_LOCK,
        profile_path=profile_path,
    )
    effective_runner = runner or FakeRunner(
        campaign_root=campaign,
        identity_overrides=(
            {
                "transport": "openai_chat_completions",
                "endpoint_family": "openai",
                "base_url": "https://api.deepseek.com",
            }
            if load_harness_profile(profile_path).transport == "openai_chat_completions"
            else None
        ),
    )
    run_baseline_campaign(
        campaign,
        repository_root=REPOSITORY_ROOT,
        cases_path=DEVELOPMENT_CASES,
        dataset_lock_path=DATASET_LOCK,
        profile_path=profile_path,
        runner=effective_runner,
    )
    generate_baseline_report(
        campaign,
        repository_root=REPOSITORY_ROOT,
        cases_path=DEVELOPMENT_CASES,
        dataset_lock_path=DATASET_LOCK,
        profile_path=profile_path,
    )
    return campaign


def test_compare_requires_complete_trusted_orthogonal_h2_arms(tmp_path: Path) -> None:
    control = _campaign(tmp_path, "control", CONTROL_PROFILE)
    candidate = _campaign(tmp_path, "candidate", CANDIDATE_PROFILE)

    result = compare_baseline_campaigns(
        control_campaign_root=control,
        control_profile_path=CONTROL_PROFILE,
        candidate_campaign_root=candidate,
        candidate_profile_path=CANDIDATE_PROFILE,
        repository_root=REPOSITORY_ROOT,
        cases_path=DEVELOPMENT_CASES,
        dataset_lock_path=DATASET_LOCK,
    )

    assert result.paired_attempts == 30
    assert result.control_transport == "anthropic_messages"
    assert result.candidate_transport == "openai_chat_completions"
    assert result.control_metrics.trusted_attempts == 30
    assert result.candidate_metrics.trusted_attempts == 30
    assert result.blockers == []
    assert set(result.bootstrap_intervals) == {
        "trusted_success_rate_delta",
        "verification_pass_rate_delta",
        "scope_violation_rate_delta",
        "median_model_rounds_delta",
        "median_duration_ms_delta",
    }
    assert all(
        interval.method == "paired_bootstrap" and interval.resamples == 2_000
        for interval in result.bootstrap_intervals.values()
    )
    assert result.selection_policy == "paired_bootstrap_quality_gated_v1"
    assert set(result.outcome_changes) == {
        f"r{repetition}:{task_id}"
        for repetition in range(1, 4)
        for task_id in json.loads(DATASET_LOCK.read_text())["development"]["ids"]
    }


def test_selection_is_uncertainty_gated_and_fail_closed() -> None:
    metrics = BaselineComparisonMetrics(
        attempts=30,
        trusted_attempts=30,
        trusted_success_rate=0.5,
        verification_pass_rate=1.0,
        scope_violation_rate=0.0,
        median_model_rounds=3.0,
        p95_model_rounds=4.0,
        median_cost_cny=1.0,
        p95_cost_cny=1.0,
        median_duration_ms=100.0,
        p95_duration_ms=100.0,
        failure_categories={},
        provider_errors={},
        trust_blockers={},
    )

    def interval(estimate: float, low: float, high: float) -> BootstrapInterval:
        return BootstrapInterval(estimate=estimate, low=low, high=high, resamples=2_000, seed=1)

    intervals = {
        "trusted_success_rate_delta": interval(0.1, -0.1, 0.3),
        "verification_pass_rate_delta": interval(0.0, 0.0, 0.0),
        "scope_violation_rate_delta": interval(0.0, 0.0, 0.0),
        "median_model_rounds_delta": interval(-1.0, -1.0, -0.1),
        "median_duration_ms_delta": interval(-10.0, -10.0, -1.0),
    }

    assert _select_winner(metrics, metrics, intervals) == "no_material_difference"
    intervals["trusted_success_rate_delta"] = interval(0.1, 0.01, 0.3)
    assert _select_winner(metrics, metrics, intervals) == "candidate"
    intervals["scope_violation_rate_delta"] = interval(0.1, 0.01, 0.2)
    assert _select_winner(metrics, metrics, intervals) == "control"


def test_comparison_metrics_use_typed_failure_taxonomy() -> None:
    task = EvaluationTask(id="taxonomy-case", title="Taxonomy", prompt="Run the task.")
    trace = make_trace(task).model_copy(
        update={
            "raw_events": [
                {
                    "event_type": "harness_contract_evidence",
                    "failure_taxonomy": {"primary": "passed"},
                }
            ]
        }
    )
    report = SimpleNamespace(
        trusted_attempts=30,
        trusted_success_rate=1.0,
        verification_pass_rate=1.0,
        scope_violation_rate=0.0,
        failure_categories={"none": 30},
    )
    artifact = SimpleNamespace(
        repetition=1,
        task_id=task.id,
        trace=trace,
        usage=SimpleNamespace(cost_cny=Decimal("0.1")),
        trust_result=SimpleNamespace(blockers=[]),
        primary_failure_category="none",
    )

    metrics = _build_metrics(
        report,
        {
            f"attempt-{index}": artifact
            for index in range(30)
        },
    )

    assert metrics.failure_categories == {"passed": 30}


def test_compare_rejects_same_transport_and_incomplete_campaign(tmp_path: Path) -> None:
    control = _campaign(tmp_path, "control", CONTROL_PROFILE)
    same_route = _campaign(tmp_path, "same", CONTROL_PROFILE)

    with pytest.raises(BaselineComparisonError, match="transport_same"):
        compare_baseline_campaigns(
            control_campaign_root=control,
            control_profile_path=CONTROL_PROFILE,
            candidate_campaign_root=same_route,
            candidate_profile_path=CONTROL_PROFILE,
            repository_root=REPOSITORY_ROOT,
            cases_path=DEVELOPMENT_CASES,
            dataset_lock_path=DATASET_LOCK,
        )

    incomplete = tmp_path / "incomplete"
    initialize_baseline_campaign(
        incomplete,
        campaign_id="incomplete",
        repository_root=REPOSITORY_ROOT,
        cases_path=DEVELOPMENT_CASES,
        dataset_lock_path=DATASET_LOCK,
        profile_path=CANDIDATE_PROFILE,
    )
    with pytest.raises(BaselineComparisonError, match="campaign_not_complete"):
        compare_baseline_campaigns(
            control_campaign_root=control,
            control_profile_path=CONTROL_PROFILE,
            candidate_campaign_root=incomplete,
            candidate_profile_path=CANDIDATE_PROFILE,
            repository_root=REPOSITORY_ROOT,
            cases_path=DEVELOPMENT_CASES,
            dataset_lock_path=DATASET_LOCK,
        )


def test_compare_rejects_non_route_policy_drift(tmp_path: Path) -> None:
    control = _campaign(tmp_path, "control", CONTROL_PROFILE)
    original = load_harness_profile(CANDIDATE_PROFILE)
    drifted = original.model_copy(update={"max_model_rounds": original.max_model_rounds - 1})
    drift_path = tmp_path / "drifted.json"
    drift_path.write_bytes(harness_profile_bytes(drifted) + b"\n")
    candidate = _campaign(tmp_path, "candidate", drift_path)

    with pytest.raises(BaselineComparisonError, match="profile_policy_mismatch"):
        compare_baseline_campaigns(
            control_campaign_root=control,
            control_profile_path=CONTROL_PROFILE,
            candidate_campaign_root=candidate,
            candidate_profile_path=drift_path,
            repository_root=REPOSITORY_ROOT,
            cases_path=DEVELOPMENT_CASES,
            dataset_lock_path=DATASET_LOCK,
        )


def test_policy_comparison_allows_only_minimal_to_planning_same_route(tmp_path: Path) -> None:
    minimal = _campaign(tmp_path, "minimal", MINIMAL_PROFILE)
    planning = _campaign(tmp_path, "planning", PLANNING_PROFILE)

    result = compare_baseline_campaigns(
        control_campaign_root=minimal,
        control_profile_path=MINIMAL_PROFILE,
        candidate_campaign_root=planning,
        candidate_profile_path=PLANNING_PROFILE,
        repository_root=REPOSITORY_ROOT,
        cases_path=DEVELOPMENT_CASES,
        dataset_lock_path=DATASET_LOCK,
        comparison_mode="policy_ab_v1",
    )

    assert result.control_transport == "anthropic_messages"
    assert result.candidate_transport == "anthropic_messages"
    assert result.control_profile_id == "deepseek-v4-flash-anthropic-minimal-v1"
    assert result.candidate_profile_id == "deepseek-v4-flash-anthropic-planning-v1"


def test_policy_comparison_rejects_route_or_unrelated_policy_drift(tmp_path: Path) -> None:
    minimal = _campaign(tmp_path, "minimal", MINIMAL_PROFILE)
    candidate = _campaign(tmp_path, "candidate", CANDIDATE_PROFILE)

    with pytest.raises(BaselineComparisonError, match="policy_route_mismatch"):
        compare_baseline_campaigns(
            control_campaign_root=minimal,
            control_profile_path=MINIMAL_PROFILE,
            candidate_campaign_root=candidate,
            candidate_profile_path=CANDIDATE_PROFILE,
            repository_root=REPOSITORY_ROOT,
            cases_path=DEVELOPMENT_CASES,
            dataset_lock_path=DATASET_LOCK,
            comparison_mode="policy_ab_v1",
        )


def test_comparison_output_is_canonical_and_hash_bound(tmp_path: Path) -> None:
    control = _campaign(tmp_path, "control", CONTROL_PROFILE)
    candidate = _campaign(tmp_path, "candidate", CANDIDATE_PROFILE)
    result = compare_baseline_campaigns(
        control_campaign_root=control,
        control_profile_path=CONTROL_PROFILE,
        candidate_campaign_root=candidate,
        candidate_profile_path=CANDIDATE_PROFILE,
        repository_root=REPOSITORY_ROOT,
        cases_path=DEVELOPMENT_CASES,
        dataset_lock_path=DATASET_LOCK,
    )
    output = tmp_path / "comparison.json"

    digest = write_baseline_comparison(output, result)

    assert output.read_bytes().endswith(b"\n")
    assert json.loads(output.read_text()) == result.model_dump(mode="json")
    sidecar = output.with_suffix(output.suffix + ".sha256")
    assert sidecar.read_text() == digest + "\n"
