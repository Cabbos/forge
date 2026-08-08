import json
import shutil
import subprocess
from pathlib import Path

import pytest
from test_campaign import accept, initialize, write_acceptance_artifacts

from app.campaign import finalize_campaign
from app.campaign_verify import verify_campaign
from app.models import ShellOutput
from app.provenance import sha256_bytes


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


@pytest.fixture(scope="module")
def finalized_template(tmp_path_factory: pytest.TempPathFactory) -> Path:
    root = tmp_path_factory.mktemp("finalized-campaign")
    campaign = initialize(root)
    trace_path, report_path, patch_path, _trace, _report = write_acceptance_artifacts(
        campaign, root / "inputs"
    )
    accept(campaign, trace_path, report_path, patch_path)
    finalize_campaign(campaign)
    return campaign


@pytest.fixture
def finalized_campaign(finalized_template: Path, tmp_path: Path) -> Path:
    campaign = tmp_path / "campaign"
    shutil.copytree(finalized_template, campaign)
    return campaign


def test_verify_campaign_recomputes_complete_chain(
    finalized_campaign: Path,
) -> None:
    result = verify_campaign(finalized_campaign)

    assert result.ok is True
    assert result.blockers == []
    assert result.entry_count == 1
    assert result.full_verification_passed is True
    assert result.manifest_commit == git(finalized_campaign / "worktree", "rev-parse", "HEAD")


def test_verify_campaign_rejects_changed_final_verification_exit_code(
    finalized_campaign: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    import app.campaign_verify as campaign_verify

    original = campaign_verify._run_clean_verification

    def changed_output(worktree: Path, command: str) -> ShellOutput:
        output = original(worktree, command)
        return output.model_copy(update={"exit_code": 1})

    monkeypatch.setattr(campaign_verify, "_run_clean_verification", changed_output)

    result = verify_campaign(finalized_campaign)

    assert result.ok is False
    assert result.full_verification_passed is False
    assert "final_verification_exit_code_mismatch" in result.blockers
    assert "full_verification_failed" in result.blockers


@pytest.mark.parametrize(
    ("tamper", "expected_blocker"),
    [
        ("source", "worktree_dirty"),
        ("manifest", "worktree_dirty"),
        ("ledger", "ledger_digest_mismatch"),
        ("trace", "artifact_index_mismatch"),
        ("report", "artifact_index_mismatch"),
        ("patch", "artifact_index_mismatch"),
        ("verification_artifact", "artifact_index_mismatch"),
        ("artifact_index", "artifact_index_digest_mismatch"),
        ("fixture_lock", "fixture_lock_digest_mismatch"),
        ("dataset_lock", "dataset_lock_digest_mismatch"),
        ("harness_lock", "harness_lock_digest_mismatch"),
        ("history", "manifest_commit_mismatch"),
        ("verification", "manifest_invalid"),
    ],
)
def test_verify_campaign_fails_closed_after_tampering(
    finalized_campaign: Path,
    tamper: str,
    expected_blocker: str,
) -> None:
    worktree = finalized_campaign / "worktree"
    manifest_path = worktree / "FORGE-BUILD-MANIFEST.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    run_id = json.loads(
        (finalized_campaign / "ledger.jsonl").read_text(encoding="utf-8").splitlines()[0]
    )["run_id"]

    if tamper == "source":
        source = worktree / "packages/domain/src/task-title.ts"
        source.write_text(
            source.read_text(encoding="utf-8") + "\n// tampered\n",
            encoding="utf-8",
        )
    elif tamper == "manifest":
        manifest["campaign_id"] = "tampered"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    elif tamper == "ledger":
        ledger = finalized_campaign / "ledger.jsonl"
        ledger.write_bytes(ledger.read_bytes()[:-2] + b"\n")
    elif tamper == "trace":
        path = finalized_campaign / f"artifacts/traces/{run_id}.json"
        path.write_bytes(path.read_bytes() + b" ")
    elif tamper == "report":
        path = finalized_campaign / f"artifacts/reports/{run_id}.json"
        path.write_bytes(path.read_bytes() + b" ")
    elif tamper == "patch":
        path = finalized_campaign / f"artifacts/patches/{run_id}.patch"
        path.write_bytes(path.read_bytes() + b"\n")
    elif tamper == "verification_artifact":
        path = finalized_campaign / f"artifacts/verification/{run_id}.json"
        path.write_bytes(path.read_bytes() + b" ")
    elif tamper == "artifact_index":
        path = finalized_campaign / "artifact-index.json"
        payload = json.loads(path.read_text(encoding="utf-8"))
        payload["artifacts"][0]["sha256"] = "0" * 64
        path.write_text(json.dumps(payload), encoding="utf-8")
    elif tamper == "dataset_lock":
        path = finalized_campaign / "locks/dataset-lock.json"
        path.write_bytes(path.read_bytes() + b" ")
    elif tamper == "fixture_lock":
        path = finalized_campaign / "locks/fixture-lock.json"
        path.write_bytes(path.read_bytes() + b" ")
    elif tamper == "harness_lock":
        path = finalized_campaign / "locks/harness-profile.json"
        path.write_bytes(path.read_bytes() + b" ")
    elif tamper == "history":
        git(
            worktree,
            "commit",
            "--amend",
            "--no-gpg-sign",
            "-m",
            "tampered manifest commit",
        )
    elif tamper == "verification":
        manifest["final_verification"]["passed"] = False
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    else:
        raise AssertionError(tamper)

    result = verify_campaign(finalized_campaign, run_full_verification=False)

    assert result.ok is False
    assert expected_blocker in result.blockers
    assert result.full_verification_passed is None
    assert all("sk-" not in blocker for blocker in result.blockers)


def test_manifest_and_index_digests_are_raw_file_hashes(
    finalized_campaign: Path,
) -> None:
    manifest = json.loads(
        (finalized_campaign / "worktree/FORGE-BUILD-MANIFEST.json").read_text(encoding="utf-8")
    )
    assert manifest["provenance_ledger"]["sha256"] == sha256_bytes(
        (finalized_campaign / "ledger.jsonl").read_bytes()
    )
    assert manifest["artifact_index"]["sha256"] == sha256_bytes(
        (finalized_campaign / "artifact-index.json").read_bytes()
    )
    dataset_lock_sha256 = sha256_bytes(
        (finalized_campaign / "locks/dataset-lock.json").read_bytes()
    )
    assert manifest["development_dataset"]["sha256"] == dataset_lock_sha256
    assert manifest["holdout_dataset"]["sha256"] == dataset_lock_sha256
