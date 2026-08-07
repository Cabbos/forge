from decimal import Decimal

import pytest
from pydantic import ValidationError

from app.provenance_models import (
    ArtifactDigest,
    ControllerIntervention,
    ForgeBuildManifest,
    ForgeProvenanceEntry,
    VerificationDigest,
)

SHA_A = "a" * 64
SHA_B = "b" * 64
GIT_A = "a" * 40
GIT_B = "b" * 40


def make_entry(**updates) -> ForgeProvenanceEntry:
    payload = {
        "sequence": 1,
        "previous_entry_sha256": None,
        "task_id": "forgebench-dev-domain-normalize-title",
        "run_id": "run-1",
        "provider": "deepseek",
        "model": "deepseek-v4-flash",
        "transport": "anthropic-compatible",
        "input_commit": GIT_A,
        "patch_sha256": SHA_A,
        "output_commit": GIT_B,
        "trace_sha256": SHA_A,
        "report_sha256": SHA_B,
        "harness_fingerprint": SHA_A,
        "dataset_fingerprint": SHA_B,
        "verification_digest": SHA_A,
        "cost_cny": Decimal("0.42"),
    }
    payload.update(updates)
    return ForgeProvenanceEntry.model_validate(payload)


def artifact(artifact_id: str, path: str, digest: str = SHA_A) -> ArtifactDigest:
    return ArtifactDigest(id=artifact_id, path=path, sha256=digest)


def make_manifest(**updates) -> ForgeBuildManifest:
    payload = {
        "campaign_id": "campaign-1",
        "status": "complete",
        "baseline_commit": GIT_A,
        "final_commit": GIT_B,
        "fixture": artifact("fixture", "locks/fixture-lock.json"),
        "development_dataset": artifact("development", "locks/dataset-lock.json", SHA_B),
        "holdout_dataset": artifact("holdout", "locks/dataset-lock.json", SHA_B),
        "harness_profile": artifact("harness", "locks/harness-profile.json"),
        "provenance_ledger": artifact("ledger", "ledger.jsonl"),
        "artifact_index": artifact("artifact-index", "artifact-index.json"),
        "final_tree": artifact("final-tree", "worktree"),
        "tree_digest_profile": "forgebench-v1",
        "final_verification": VerificationDigest(
            command="python3 tools/release/verify.py",
            exit_code=0,
            stdout_sha256=SHA_A,
            stderr_sha256=SHA_B,
            verified_commit=GIT_B,
            passed=True,
        ),
        "ledger_verified": True,
        "accepted_tasks": ["task-a", "task-b"],
        "controller_interventions": [
            ControllerIntervention(
                sequence=0,
                kind="campaign_init",
                reason="Initialize the locked canonical benchmark.",
                invalidates_campaign=False,
            ),
            ControllerIntervention(
                sequence=1,
                kind="manifest_finalize",
                reason="Write generated provenance metadata.",
                invalidates_campaign=False,
            ),
        ],
    }
    payload.update(updates)
    return ForgeBuildManifest.model_validate(payload)


def test_provenance_entry_is_strict_trusted_forge_evidence() -> None:
    entry = make_entry()
    restored = ForgeProvenanceEntry.model_validate_json(entry.model_dump_json())

    assert entry.schema_version == 1
    assert entry.agent == "forge"
    assert entry.trust_status == "trusted"
    assert entry.cost_cny == Decimal("0.42")
    assert restored == entry

    for update in (
        {"schema_version": 2},
        {"agent": "controller"},
        {"trust_status": "unknown"},
        {"patch_sha256": "A" * 64},
        {"trace_sha256": "abc"},
        {"input_commit": "f" * 39},
        {"cost_cny": Decimal("-0.01")},
        {"provider": "sk-" + "x" * 32},
        {"api_key": "secret"},
    ):
        with pytest.raises(ValidationError):
            make_entry(**update)


def test_manifest_references_every_required_artifact_and_ordered_task() -> None:
    manifest = make_manifest()

    assert manifest.status == "complete"
    assert manifest.accepted_tasks == ["task-a", "task-b"]
    assert manifest.fixture.id == "fixture"
    assert manifest.development_dataset.id == "development"
    assert manifest.holdout_dataset.id == "holdout"
    assert manifest.harness_profile.id == "harness"
    assert manifest.provenance_ledger.id == "ledger"
    assert manifest.artifact_index.id == "artifact-index"
    assert manifest.tree_digest_profile == "forgebench-v1"
    assert manifest.final_tree.id == "final-tree"
    assert manifest.final_verification.verified_commit == manifest.final_commit


@pytest.mark.parametrize(
    "updates",
    [
        {"ledger_verified": False},
        {
            "final_verification": VerificationDigest(
                command="python3 tools/release/verify.py",
                exit_code=1,
                stdout_sha256=SHA_A,
                stderr_sha256=SHA_B,
                verified_commit=GIT_B,
                passed=False,
            )
        },
        {
            "controller_interventions": [
                ControllerIntervention(
                    sequence=0,
                    kind="campaign_init",
                    reason="Initialize.",
                    invalidates_campaign=False,
                ),
                ControllerIntervention(
                    sequence=1,
                    kind="product_source_edit",
                    reason="Controller patched source.",
                    invalidates_campaign=True,
                ),
                ControllerIntervention(
                    sequence=2,
                    kind="manifest_finalize",
                    reason="Finalize.",
                    invalidates_campaign=False,
                ),
            ]
        },
        {"accepted_tasks": ["task-a", "task-a"]},
        {
            "controller_interventions": [
                ControllerIntervention(
                    sequence=1,
                    kind="campaign_init",
                    reason="Wrong sequence.",
                    invalidates_campaign=False,
                )
            ]
        },
    ],
)
def test_complete_manifest_fails_closed_on_broken_evidence(updates) -> None:
    with pytest.raises(ValidationError):
        make_manifest(**updates)


def test_invalid_manifest_can_preserve_failed_verification_for_audit() -> None:
    failed = VerificationDigest(
        command="python3 tools/release/verify.py",
        exit_code=1,
        stdout_sha256=SHA_A,
        stderr_sha256=SHA_B,
        verified_commit=GIT_B,
        passed=False,
    )

    manifest = make_manifest(
        status="invalid",
        ledger_verified=False,
        final_verification=failed,
    )

    assert manifest.status == "invalid"
    assert manifest.final_verification.passed is False
