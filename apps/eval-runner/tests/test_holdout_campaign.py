import json
from pathlib import Path

import pytest

import app.holdout_campaign as holdout_campaign
from app.baseline_models import BaselineComparison, BaselineComparisonMetrics
from app.harness_profiles import load_harness_profile
from app.holdout_campaign import (
    HoldoutRunResult,
    generate_holdout_report,
    initialize_holdout_campaign,
    recover_holdout_campaign,
    run_holdout_campaign,
    verify_holdout_campaign,
)
from app.models import EvaluationTask
from app.provenance import canonical_json_bytes, sha256_bytes
from tests.test_baseline import FakeRunner, make_trace

REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
EVAL_ROOT = Path(__file__).resolve().parents[1]
HOLDOUT_CASES = EVAL_ROOT / "forgebench" / "cases" / "holdout"
DATASET_LOCK = EVAL_ROOT / "forgebench" / "dataset-lock.json"
PROFILE = EVAL_ROOT / "forgebench" / "profiles" / "deepseek-v4-flash-anthropic-baseline-v1.json"
PLANNING_PROFILE = (
    EVAL_ROOT / "forgebench" / "profiles" / "deepseek-v4-flash-anthropic-planning-v1.json"
)


def _comparison(tmp_path: Path, profile_path: Path = PROFILE) -> Path:
    profile = load_harness_profile(profile_path)
    metrics = BaselineComparisonMetrics(
        trusted_attempts=30,
        trusted_success_rate=0.9,
        verification_pass_rate=1.0,
        scope_violation_rate=0.0,
        median_model_rounds=10.0,
        p95_model_rounds=12.0,
        median_cost_cny=0.1,
        p95_cost_cny=0.2,
        median_duration_ms=1.0,
        p95_duration_ms=2.0,
        failure_categories={},
        provider_errors={},
        trust_blockers={},
    )
    comparison = BaselineComparison(
        control_campaign_id="control-v6",
        candidate_campaign_id="candidate-v6",
        dataset_fingerprint="a" * 64,
        model="deepseek-v4-flash",
        control_transport="anthropic_messages",
        candidate_transport="openai_chat_completions",
        outcome_changes={"r1:case": "passed->passed"},
        control_metrics=metrics,
        candidate_metrics=metrics,
        winner="no_material_difference",
        control_profile_id=profile.profile_id,
        candidate_profile_id="deepseek-v4-flash-openai-chat-control-v1",
        control_harness_fingerprint="b" * 64,
        candidate_harness_fingerprint="c" * 64,
        source_commit=__import__("subprocess")
        .check_output(["git", "rev-parse", "HEAD"], cwd=REPOSITORY_ROOT, text=True)
        .strip(),
    )
    path = tmp_path / "comparison.json"
    raw = canonical_json_bytes(comparison.model_dump(mode="json")) + b"\n"
    path.write_bytes(raw)
    path.with_suffix(path.suffix + ".sha256").write_text(sha256_bytes(raw) + "\n")
    return path


def _initialize(tmp_path: Path) -> Path:
    campaign = tmp_path / "holdout"
    initialize_holdout_campaign(
        campaign,
        campaign_id="h4-test",
        repository_root=REPOSITORY_ROOT,
        cases_path=HOLDOUT_CASES,
        dataset_lock_path=DATASET_LOCK,
        profile_path=PROFILE,
        development_comparison_path=_comparison(tmp_path),
    )
    return campaign


def _kwargs(campaign: Path) -> dict[str, Path]:
    return {
        "campaign_root": campaign,
        "repository_root": REPOSITORY_ROOT,
        "cases_path": HOLDOUT_CASES,
        "dataset_lock_path": DATASET_LOCK,
        "profile_path": PROFILE,
    }


def test_holdout_evidence_prefers_typed_failure_taxonomy_and_keeps_legacy_fallback() -> None:
    task = EvaluationTask(id="taxonomy-case", title="Taxonomy", prompt="Run the task.")
    legacy_trace = make_trace(task)
    assert holdout_campaign._terminal_failure_category(legacy_trace, "none") == "none"

    typed_trace = legacy_trace.model_copy(
        update={
            "raw_events": [
                {
                    "event_type": "harness_contract_evidence",
                    "failure_taxonomy": {"primary": "passed"},
                }
            ]
        }
    )
    assert holdout_campaign._terminal_failure_category(typed_trace, "none") == "passed"


def test_holdout_init_locks_sealed_split_and_development_comparison(tmp_path: Path) -> None:
    campaign = _initialize(tmp_path)

    metadata = json.loads((campaign / "campaign.json").read_text())
    assert metadata["max_attempts"] == 60
    assert metadata["case_ids"] == sorted(metadata["case_ids"])
    assert (campaign / "locks" / "development-comparison.json").exists()
    verification = verify_holdout_campaign(**_kwargs(campaign))
    assert verification.ok is False
    assert "holdout_attempt_count_incomplete" in verification.blockers


def test_holdout_init_rejects_planning_only_profile(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="planning-only profile"):
        initialize_holdout_campaign(
            tmp_path / "planning-holdout",
            campaign_id="h4-planning-rejected",
            repository_root=REPOSITORY_ROOT,
            cases_path=HOLDOUT_CASES,
            dataset_lock_path=DATASET_LOCK,
            profile_path=PLANNING_PROFILE,
            development_comparison_path=_comparison(tmp_path, PLANNING_PROFILE),
        )


def test_holdout_runner_is_resumable_and_never_replays_terminal_attempts(tmp_path: Path) -> None:
    campaign = _initialize(tmp_path)
    runner = FakeRunner(campaign_root=campaign)

    first = run_holdout_campaign(**_kwargs(campaign), runner=runner, max_attempts=1)
    assert first.status == "paused"
    assert first.calls_started == 1
    second = run_holdout_campaign(**_kwargs(campaign), runner=runner, max_attempts=1)
    assert second.status == "paused"
    assert second.calls_started == 1
    assert len(runner.calls) == 2
    assert runner.calls[0] != runner.calls[1]


def test_holdout_recovery_closes_pending_attempt_as_unknown_cost(tmp_path: Path) -> None:
    campaign = _initialize(tmp_path)
    runner = FakeRunner(campaign_root=campaign, fail_after_pending=True)
    result = run_holdout_campaign(**_kwargs(campaign), runner=runner)
    assert result.status == "blocked"

    recovered = recover_holdout_campaign(**_kwargs(campaign))
    assert isinstance(recovered, HoldoutRunResult)
    assert recovered.status == "blocked"
    assert "interrupted_paid_attempt_unknown_cost" in recovered.blockers
    assert not list((campaign / "pending").glob("*.json"))
    assert len(list((campaign / "attempts").glob("*.json"))) == 1


def test_holdout_report_requires_all_sixty_closed_attempts(tmp_path: Path) -> None:
    campaign = _initialize(tmp_path)
    runner = FakeRunner(campaign_root=campaign)
    run_holdout_campaign(**_kwargs(campaign), runner=runner, max_attempts=1)

    with pytest.raises(ValueError, match="60 closed"):
        generate_holdout_report(**_kwargs(campaign))


def test_holdout_complete_campaign_publishes_index_and_canonical_report(tmp_path: Path) -> None:
    campaign = _initialize(tmp_path)
    runner = FakeRunner(campaign_root=campaign)
    result = run_holdout_campaign(**_kwargs(campaign), runner=runner)
    assert result.status == "complete"
    report = generate_holdout_report(**_kwargs(campaign))
    assert report.completed_attempts == 60
    assert (campaign / "holdout-artifact-index.json").exists()
    verified = verify_holdout_campaign(**_kwargs(campaign))
    assert verified.ok is True

    monkeypatch = pytest.MonkeyPatch()
    monkeypatch.setattr(holdout_campaign, "_git_text", lambda *args: "0" * 40)
    monkeypatch.setattr(holdout_campaign, "_dirty_state_sha256", lambda *args: "0" * 64)
    try:
        strict = verify_holdout_campaign(**_kwargs(campaign))
        assert strict.ok is False
        assert strict.blockers == ["campaign_state_invalid"]

        frozen = verify_holdout_campaign(**_kwargs(campaign), require_current_source=False)
        assert frozen.ok is True
    finally:
        monkeypatch.undo()
