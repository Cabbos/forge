import json
import shutil
import stat
import subprocess
from pathlib import Path

import pytest

from app.artifact_hashing import canonical_tree_digest
from app.campaign import initialize_campaign

FORGEBENCH_ROOT = Path(__file__).resolve().parents[1] / "forgebench"
FIXTURE = FORGEBENCH_ROOT / "forgebench-monorepo"
FIXTURE_LOCK = FORGEBENCH_ROOT / "fixture-lock.json"
CASES = FORGEBENCH_ROOT / "cases"
DATASET_LOCK = FORGEBENCH_ROOT / "dataset-lock.json"


def write_harness_profile(path: Path, *, secret: bool = False) -> None:
    payload = {
        "schema_version": 1,
        "profile_id": "deepseek-v4-flash-frontier-v1",
        "provider": "deepseek",
        "model": "deepseek-v4-flash",
        "transport": "anthropic-compatible",
        "tool_policy": "forge-default",
        "max_model_rounds": 40,
    }
    if secret:
        payload["api_key"] = "sk-" + "x" * 32
    path.write_text(json.dumps(payload), encoding="utf-8")


def initialize(
    tmp_path: Path,
    *,
    fixture: Path = FIXTURE,
    fixture_lock: Path = FIXTURE_LOCK,
    cases: Path = CASES,
    dataset_lock: Path = DATASET_LOCK,
    harness_profile: Path | None = None,
) -> Path:
    campaign = tmp_path / "campaign"
    profile = harness_profile or tmp_path / "harness-profile.json"
    if harness_profile is None:
        write_harness_profile(profile)
    initialize_campaign(
        campaign,
        campaign_id="campaign-test",
        fixture=fixture,
        fixture_lock=fixture_lock,
        cases=cases,
        dataset_lock=dataset_lock,
        harness_profile=profile,
        provider="deepseek",
        model="deepseek-v4-flash",
        transport="anthropic-compatible",
        controller_version="forge-eval-controller/1",
        runtime_version="forge-runtime/1",
    )
    return campaign


def git(worktree: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=worktree,
        text=True,
        capture_output=True,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    return completed.stdout.strip()


def test_initialize_campaign_creates_isolated_locked_baseline(tmp_path: Path) -> None:
    campaign = initialize(tmp_path)
    worktree = campaign / "worktree"
    metadata = json.loads((campaign / "campaign.json").read_text(encoding="utf-8"))

    assert worktree.is_dir()
    assert (worktree / ".git").is_dir()
    assert git(worktree, "status", "--porcelain") == ""
    assert git(worktree, "remote") == ""
    assert git(worktree, "branch", "--show-current") == "forge-built"
    assert git(worktree, "log", "-1", "--pretty=%s") == (
        "chore: initialize canonical ForgeBench baseline"
    )
    assert metadata["baseline_commit"] == git(worktree, "rev-parse", "HEAD")
    assert metadata["baseline_tree_sha256"] == canonical_tree_digest(worktree)
    assert metadata["fixture_tree_sha256"] == canonical_tree_digest(FIXTURE)
    assert metadata["worktree"] == "worktree"
    assert metadata["ledger"] == "ledger.jsonl"
    assert metadata["provider"] == "deepseek"
    assert metadata["model"] == "deepseek-v4-flash"
    assert metadata["transport"] == "anthropic-compatible"
    assert metadata["controller_version"] == "forge-eval-controller/1"
    assert metadata["runtime_version"] == "forge-runtime/1"
    assert metadata["controller_interventions"] == [
        {
            "sequence": 0,
            "kind": "campaign_init",
            "reason": "Initialize the locked canonical ForgeBench baseline.",
            "invalidates_campaign": False,
        }
    ]
    assert str(tmp_path) not in json.dumps(metadata)
    assert stat.S_IMODE((campaign / "campaign.json").stat().st_mode) == 0o600
    assert stat.S_IMODE((campaign / "ledger.jsonl").stat().st_mode) == 0o600
    assert (campaign / "ledger.jsonl").read_bytes() == b""
    assert {path.name for path in (campaign / "artifacts").iterdir()} == {
        "traces",
        "reports",
        "patches",
        "verification",
    }
    assert {path.name for path in (campaign / "locks").iterdir()} == {
        "fixture-lock.json",
        "dataset-lock.json",
        "harness-profile.json",
    }
    assert not (FIXTURE / ".git").exists()


def test_initialize_rejects_fixture_content_drift_without_partial_campaign(
    tmp_path: Path,
) -> None:
    fixture = tmp_path / "fixture"
    shutil.copytree(FIXTURE, fixture)
    (fixture / "README.md").write_text("tampered\n", encoding="utf-8")

    with pytest.raises(ValueError, match="fixture lock"):
        initialize(tmp_path, fixture=fixture)

    assert not (tmp_path / "campaign").exists()


@pytest.mark.parametrize("generated_name", [".git", "node_modules", "__pycache__"])
def test_initialize_rejects_generated_or_git_state(
    tmp_path: Path,
    generated_name: str,
) -> None:
    fixture = tmp_path / "fixture"
    shutil.copytree(FIXTURE, fixture)
    (fixture / generated_name).mkdir()

    with pytest.raises(ValueError, match="generated state"):
        initialize(tmp_path, fixture=fixture)

    assert not (tmp_path / "campaign").exists()


def test_initialize_rejects_dataset_lock_drift(tmp_path: Path) -> None:
    lock = tmp_path / "dataset-lock.json"
    payload = json.loads(DATASET_LOCK.read_text(encoding="utf-8"))
    payload["holdout"]["count"] = 19
    lock.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(ValueError, match="dataset lock"):
        initialize(tmp_path, dataset_lock=lock)

    assert not (tmp_path / "campaign").exists()


def test_initialize_rejects_secret_bearing_harness_profile(tmp_path: Path) -> None:
    profile = tmp_path / "harness-profile.json"
    write_harness_profile(profile, secret=True)

    with pytest.raises(ValueError, match="secret"):
        initialize(tmp_path, harness_profile=profile)

    assert not (tmp_path / "campaign").exists()
