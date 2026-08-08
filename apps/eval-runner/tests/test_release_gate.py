from pathlib import Path

import pytest

from app.campaign_verify import CampaignVerification
from app.harness_profiles import harness_profile_fingerprint, load_harness_profile
from app.holdout_campaign import HoldoutVerification
from app.holdout_gate import HoldoutGateReportV1, holdout_report_bytes
from app.release_cli import release_main
from app.release_gate import evaluate_frontier_release

PROFILE = (
    Path(__file__).resolve().parents[1]
    / "forgebench"
    / "profiles"
    / "deepseek-v4-flash-anthropic-baseline-v1.json"
)


def valid_report() -> HoldoutGateReportV1:
    profile = load_harness_profile(PROFILE)
    return HoldoutGateReportV1.model_validate(
        {
            "schema_version": 1,
            "campaign_id": "holdout-release-v1",
            "dataset_split": "holdout",
            "dataset_fingerprint": "a" * 64,
            "harness_fingerprint": harness_profile_fingerprint(profile),
            "profile_id": profile.profile_id,
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
    )


def write_campaign_lock(campaign_root: Path, report: HoldoutGateReportV1) -> None:
    (campaign_root / "locks").mkdir(parents=True)
    (campaign_root / "locks" / "harness-profile.json").write_bytes(PROFILE.read_bytes())
    (campaign_root / "campaign.json").write_text(
        '{"holdout_dataset_fingerprint": "' + report.dataset_fingerprint + '"}',
        encoding="utf-8",
    )
    (campaign_root / "holdout-report.json").write_bytes(holdout_report_bytes(report))


def passing_integration() -> CampaignVerification:
    return CampaignVerification(
        ok=True,
        blockers=[],
        entry_count=20,
        accepted_tasks=["task-1"],
        full_verification_passed=True,
    )


def passing_holdout() -> HoldoutVerification:
    return HoldoutVerification(
        ok=True,
        blockers=[],
        attempt_count=60,
        pending_count=0,
        spent_cny="31.20",
    )


def test_frontier_release_requires_passing_scoring_and_provenance(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    report = valid_report()
    write_campaign_lock(tmp_path, report)
    monkeypatch.setattr(
        "app.release_gate.verify_campaign",
        lambda *args, **kwargs: passing_integration(),
    )
    monkeypatch.setattr(
        "app.release_gate.verify_holdout_campaign",
        lambda *args, **kwargs: passing_holdout(),
    )

    decision = evaluate_frontier_release(
        report,
        tmp_path,
        holdout_campaign_root=tmp_path,
        repository_root=tmp_path,
        cases_path=tmp_path,
        dataset_lock_path=tmp_path / "dataset-lock.json",
        profile_path=PROFILE,
    )

    assert decision.passed is True
    assert decision.blockers == []
    assert decision.profile_id == report.profile_id
    assert decision.harness_fingerprint == report.harness_fingerprint


def test_frontier_release_never_hides_holdout_or_integration_blockers(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    report = valid_report().model_copy(update={"trusted_success_rate": 0.79})
    write_campaign_lock(tmp_path, valid_report())
    monkeypatch.setattr(
        "app.release_gate.verify_campaign",
        lambda *args, **kwargs: CampaignVerification(
            ok=False,
            blockers=["ledger_invalid"],
            full_verification_passed=False,
        ),
    )
    monkeypatch.setattr(
        "app.release_gate.verify_holdout_campaign",
        lambda *args, **kwargs: passing_holdout(),
    )

    decision = evaluate_frontier_release(
        report,
        tmp_path,
        holdout_campaign_root=tmp_path,
        repository_root=tmp_path,
        cases_path=tmp_path,
        dataset_lock_path=tmp_path / "dataset-lock.json",
        profile_path=PROFILE,
    )

    assert decision.passed is False
    assert "holdout:holdout_success_rate_below_threshold" in decision.blockers
    assert "integration:ledger_invalid" in decision.blockers
    assert "integration:verification_failed" in decision.blockers


def test_frontier_release_binds_holdout_identity_to_campaign_lock(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    report = valid_report()
    write_campaign_lock(tmp_path, report)
    (tmp_path / "campaign.json").write_text(
        '{"holdout_dataset_fingerprint": "' + "e" * 64 + '"}',
        encoding="utf-8",
    )
    monkeypatch.setattr(
        "app.release_gate.verify_campaign",
        lambda *args, **kwargs: passing_integration(),
    )
    monkeypatch.setattr(
        "app.release_gate.verify_holdout_campaign",
        lambda *args, **kwargs: passing_holdout(),
    )

    decision = evaluate_frontier_release(
        report,
        tmp_path,
        holdout_campaign_root=tmp_path,
        repository_root=tmp_path,
        cases_path=tmp_path,
        dataset_lock_path=tmp_path / "dataset-lock.json",
        profile_path=PROFILE,
    )

    assert decision.passed is False
    assert "identity:holdout_dataset_fingerprint_mismatch" in decision.blockers


def test_frontier_release_rejects_aggregate_without_verified_holdout_campaign(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    report = valid_report()
    write_campaign_lock(tmp_path, report)
    monkeypatch.setattr(
        "app.release_gate.verify_campaign",
        lambda *args, **kwargs: passing_integration(),
    )
    monkeypatch.setattr(
        "app.release_gate.verify_holdout_campaign",
        lambda *args, **kwargs: HoldoutVerification(
            ok=False,
            blockers=["holdout_attempt_count_incomplete"],
            attempt_count=1,
            pending_count=0,
            spent_cny="0",
        ),
    )

    decision = evaluate_frontier_release(
        report,
        tmp_path,
        holdout_campaign_root=tmp_path,
        repository_root=tmp_path,
        cases_path=tmp_path,
        dataset_lock_path=tmp_path / "dataset-lock.json",
        profile_path=PROFILE,
    )

    assert decision.passed is False
    assert "holdout_campaign:holdout_attempt_count_incomplete" in decision.blockers
    assert "holdout_campaign:verification_failed" in decision.blockers


def test_frontier_release_cli_fails_closed_for_missing_evidence(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    result = release_main(
        [
            "verify",
            "--holdout-campaign",
            str(tmp_path / "missing-holdout-campaign"),
            "--campaign",
            str(tmp_path / "missing-campaign"),
        ]
    )

    assert result == 1
    assert "frontier_release_verification_failed" in capsys.readouterr().err
