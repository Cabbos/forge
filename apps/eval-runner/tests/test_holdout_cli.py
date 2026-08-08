import json
from decimal import Decimal
from pathlib import Path
from types import SimpleNamespace

import pytest

import app.holdout_cli as holdout_cli
from app.cli import main
from app.holdout_cli import holdout_main
from app.holdout_gate import HoldoutGateReportV1, holdout_report_bytes


def holdout_payload() -> dict[str, object]:
    return {
        "schema_version": 1,
        "campaign_id": "deepseek-holdout-cli-v1",
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


def test_holdout_cli_emits_machine_readable_pass_decision(tmp_path: Path, capsys) -> None:
    report = HoldoutGateReportV1.model_validate(holdout_payload())
    report_path = tmp_path / "holdout-report.json"
    report_path.write_bytes(holdout_report_bytes(report))

    assert main(["holdout", "verify", "--report", str(report_path)]) == 0

    output = json.loads(capsys.readouterr().out)
    assert output["passed"] is True
    assert output["campaign_id"] == "deepseek-holdout-cli-v1"


def test_holdout_cli_rejects_noncanonical_report(tmp_path: Path, capsys) -> None:
    report = HoldoutGateReportV1.model_validate(holdout_payload())
    report_path = tmp_path / "holdout-report.json"
    report_path.write_text(json.dumps(report.model_dump(mode="json"), indent=2), encoding="utf-8")

    assert main(["holdout", "verify", "--report", str(report_path)]) == 1

    captured = capsys.readouterr()
    assert "error: holdout_verify_failed" in captured.err


def test_holdout_init_requires_explicit_development_arm_paths() -> None:
    with pytest.raises(SystemExit) as error:
        holdout_main(
            [
                "init",
                "--campaign",
                "/tmp/holdout",
                "--campaign-id",
                "holdout-test",
                "--profile",
                "/tmp/profile.json",
                "--development-comparison",
                "/tmp/comparison.json",
            ]
        )
    assert error.value.code == 2


def test_holdout_init_passes_development_split_separately(
    monkeypatch: pytest.MonkeyPatch, capsys
) -> None:
    captured: dict[str, object] = {}

    def fake_initialize(campaign_root: Path, **kwargs: object) -> SimpleNamespace:
        captured["campaign_root"] = campaign_root
        captured.update(kwargs)
        return SimpleNamespace(
            campaign_id="holdout-test",
            case_ids=["case-1"],
            max_attempts=60,
            budget_cny=Decimal("35"),
            source_commit="c" * 40,
            dataset_fingerprint="a" * 64,
            harness_fingerprint="b" * 64,
            profile=SimpleNamespace(
                provider="deepseek",
                model="deepseek-v4-flash",
                transport="anthropic_messages",
            ),
        )

    monkeypatch.setattr(holdout_cli, "initialize_holdout_campaign", fake_initialize)
    result = holdout_cli.holdout_main(
        [
            "init",
            "--campaign",
            "/tmp/holdout",
            "--campaign-id",
            "holdout-test",
            "--profile",
            "/tmp/profile.json",
            "--control-campaign",
            "/tmp/control",
            "--control-profile",
            "/tmp/control-profile.json",
            "--candidate-campaign",
            "/tmp/candidate",
            "--candidate-profile",
            "/tmp/candidate-profile.json",
            "--development-comparison",
            "/tmp/comparison.json",
        ]
    )

    assert result == 0
    assert captured["cases_path"] == holdout_cli.DEFAULT_CASES
    assert captured["development_cases_path"] == holdout_cli.DEFAULT_DEVELOPMENT_CASES
    assert captured["development_dataset_lock_path"] == holdout_cli.DEFAULT_DEVELOPMENT_DATASET_LOCK
    assert json.loads(capsys.readouterr().out)["status"] == "active"
