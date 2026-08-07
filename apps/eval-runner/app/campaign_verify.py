import json
import shutil
import subprocess
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any, Literal

from pydantic import Field, ValidationError, field_validator

from app.artifact_hashing import (
    IGNORED_TREE_PARTS,
    canonical_manifest_digest,
    canonical_tree_digest,
    canonical_tree_manifest,
)
from app.models import EvalModel, ShellOutput
from app.provenance import (
    ARTIFACT_SPECS,
    load_verified_entries,
    sha256_bytes,
    verify_ledger,
)
from app.provenance_models import ForgeBuildManifest, ForgeProvenanceEntry, Sha256
from app.runner import run_shell_commands

BUILD_MANIFEST_NAME = "FORGE-BUILD-MANIFEST.json"
ARTIFACT_INDEX_NAME = "artifact-index.json"
FINALIZE_COMMIT_SUBJECT = "chore(forgebench): finalize Forge build manifest"


class ArtifactIndexEntry(EvalModel):
    kind: Literal["traces", "reports", "patches", "verification"]
    path: str
    sha256: Sha256
    size_bytes: int = Field(ge=0)

    @field_validator("path")
    @classmethod
    def validate_path(cls, value: str) -> str:
        path = PurePosixPath(value)
        if not value or path.is_absolute() or ".." in path.parts:
            raise ValueError("artifact index path must be campaign-relative")
        return value


class ArtifactIndex(EvalModel):
    schema_version: Literal[1] = 1
    artifacts: list[ArtifactIndexEntry]


class CampaignVerification(EvalModel):
    ok: bool
    blockers: list[str] = Field(default_factory=list)
    entry_count: int = 0
    accepted_tasks: list[str] = []
    baseline_commit: str | None = None
    final_product_commit: str | None = None
    manifest_commit: str | None = None
    full_verification_passed: bool | None = None


def build_artifact_index(campaign_root: Path) -> ArtifactIndex:
    artifacts_root = campaign_root / "artifacts"
    entries: list[ArtifactIndexEntry] = []
    for kind, (suffix, _digest_field) in ARTIFACT_SPECS.items():
        directory = artifacts_root / kind
        if not directory.is_dir():
            raise ValueError(f"artifact directory is missing: {kind}")
        for path in sorted(directory.rglob("*")):
            if path.is_dir():
                continue
            if path.is_symlink() or not path.is_file() or path.suffix != suffix:
                raise ValueError(f"unexpected campaign artifact: {path.name}")
            entries.append(
                ArtifactIndexEntry(
                    kind=kind,
                    path=path.relative_to(campaign_root).as_posix(),
                    sha256=sha256_bytes(path.read_bytes()),
                    size_bytes=path.stat().st_size,
                )
            )
    return ArtifactIndex(
        artifacts=sorted(entries, key=lambda item: item.path),
    )


def final_tree_digest(worktree: Path) -> str:
    manifest = [
        entry for entry in canonical_tree_manifest(worktree) if entry["path"] != BUILD_MANIFEST_NAME
    ]
    return canonical_manifest_digest(manifest)


def tree_digest_at_commit(worktree: Path, commit: str) -> str | None:
    with tempfile.TemporaryDirectory(prefix="forge-campaign-tree-") as temporary:
        checkout = Path(temporary) / "repo"
        clone = subprocess.run(
            ["git", "clone", "--quiet", "--no-hardlinks", str(worktree), str(checkout)],
            capture_output=True,
            check=False,
        )
        if clone.returncode != 0:
            return None
        if _git_result(checkout, "checkout", "--quiet", "--detach", commit) != 0:
            return None
        return canonical_tree_digest(checkout)


def expected_artifact_paths(
    entries: list[ForgeProvenanceEntry],
) -> set[str]:
    return {
        f"artifacts/{kind}/{entry.run_id}{suffix}"
        for entry in entries
        for kind, (suffix, _digest_field) in ARTIFACT_SPECS.items()
    }


def accepted_commit_message(entry: ForgeProvenanceEntry) -> str:
    return (
        f"feat(forgebench): accept {entry.task_id}\n\n"
        f"Forge-Run-Id: {entry.run_id}\n"
        f"Forge-Task-Id: {entry.task_id}\n"
        f"Forge-Trace-SHA256: {entry.trace_sha256}\n"
        f"Forge-Patch-SHA256: {entry.patch_sha256}\n"
    )


def verify_accepted_commit_chain(
    worktree: Path,
    artifacts_root: Path,
    entries: list[ForgeProvenanceEntry],
) -> list[str]:
    blockers: list[str] = []
    for entry in entries:
        if not _commit_exists(worktree, entry.output_commit):
            blockers.append("accepted_commit_missing")
            continue
        parents = _git(worktree, "rev-list", "--parents", "-n", "1", entry.output_commit)
        if parents.split() != [entry.output_commit, entry.input_commit]:
            blockers.append("accepted_commit_parent_mismatch")
        message = _git(worktree, "log", "-1", "--format=%B", entry.output_commit)
        if message.rstrip() + "\n" != accepted_commit_message(entry):
            blockers.append("accepted_commit_message_mismatch")
        patch_path = artifacts_root / "patches" / f"{entry.run_id}.patch"
        if not patch_path.is_file():
            blockers.append("accepted_patch_missing")
            continue
        if not _patch_replays_to_commit(worktree, entry, patch_path):
            blockers.append("accepted_patch_commit_mismatch")
    return list(dict.fromkeys(blockers))


def verify_campaign(
    campaign_root: Path,
    *,
    run_full_verification: bool = True,
) -> CampaignVerification:
    try:
        return _verify_campaign(
            campaign_root,
            run_full_verification=run_full_verification,
        )
    except Exception:
        return CampaignVerification(
            ok=False,
            blockers=["campaign_verification_error"],
        )


def _verify_campaign(
    campaign_root: Path,
    *,
    run_full_verification: bool,
) -> CampaignVerification:
    blockers: list[str] = []
    metadata = _load_json_object(campaign_root / "campaign.json")
    if metadata is None:
        return CampaignVerification(ok=False, blockers=["campaign_metadata_invalid"])

    worktree = campaign_root / "worktree"
    manifest_path = worktree / BUILD_MANIFEST_NAME
    manifest: ForgeBuildManifest | None = None
    try:
        manifest = ForgeBuildManifest.model_validate_json(manifest_path.read_bytes())
    except (OSError, ValidationError, ValueError):
        blockers.append("manifest_invalid")

    manifest_commit = _git_or_none(worktree, "rev-parse", "HEAD")
    if _git_or_none(worktree, "status", "--porcelain"):
        blockers.append("worktree_dirty")
    if (campaign_root / "pending-accept.json").exists():
        blockers.append("pending_accept_present")
    if metadata.get("status") != "complete":
        blockers.append("campaign_not_complete")
    if manifest_commit is None or metadata.get("manifest_commit") != manifest_commit:
        blockers.append("manifest_commit_mismatch")

    if manifest is not None:
        _verify_manifest_files(campaign_root, metadata, manifest, blockers)
        if metadata.get("campaign_id") != manifest.campaign_id:
            blockers.append("campaign_id_mismatch")
        if metadata.get("final_commit") != manifest.final_commit:
            blockers.append("final_product_commit_mismatch")
        if metadata.get("controller_interventions") != [
            item.model_dump(mode="json") for item in manifest.controller_interventions
        ]:
            blockers.append("controller_interventions_mismatch")
        if metadata.get("manifest_sha256") != sha256_bytes(manifest_path.read_bytes()):
            blockers.append("manifest_digest_mismatch")
        if metadata.get("artifact_index_sha256") != _sha256_file_or_none(
            campaign_root / ARTIFACT_INDEX_NAME
        ):
            blockers.append("artifact_index_digest_mismatch")
        if manifest.tree_digest_profile != "forgebench-v1":
            blockers.append("tree_digest_profile_mismatch")
        if final_tree_digest(worktree) != manifest.final_tree.sha256:
            blockers.append("final_tree_digest_mismatch")

        if manifest_commit is not None:
            parents = _git_or_none(
                worktree,
                "rev-list",
                "--parents",
                "-n",
                "1",
                manifest_commit,
            )
            if parents is None or parents.split() != [
                manifest_commit,
                manifest.final_commit,
            ]:
                blockers.append("manifest_commit_parent_mismatch")
            subject = _git_or_none(worktree, "log", "-1", "--format=%s")
            if subject != FINALIZE_COMMIT_SUBJECT:
                blockers.append("manifest_commit_message_mismatch")
            changed = _git_or_none(
                worktree,
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "-r",
                manifest_commit,
            )
            if changed != BUILD_MANIFEST_NAME:
                blockers.append("manifest_commit_scope_mismatch")
        if not _commit_exists(worktree, manifest.baseline_commit):
            blockers.append("baseline_commit_missing")
        else:
            baseline_line = _git_or_none(
                worktree,
                "rev-list",
                "--parents",
                "-n",
                "1",
                manifest.baseline_commit,
            )
            if baseline_line != manifest.baseline_commit:
                blockers.append("baseline_commit_parent_mismatch")

    ledger_path = campaign_root / "ledger.jsonl"
    artifacts_root = campaign_root / "artifacts"
    ledger_result = verify_ledger(ledger_path, artifacts_root)
    if not ledger_result.ok:
        blockers.append("ledger_invalid")
    entries: list[ForgeProvenanceEntry] = []
    if ledger_result.ok:
        try:
            entries = load_verified_entries(ledger_path, artifacts_root)
        except ValueError:
            blockers.append("ledger_invalid")
    if manifest is not None:
        if manifest.provenance_ledger.sha256 != _sha256_file_or_none(ledger_path):
            blockers.append("ledger_digest_mismatch")
        if [entry.task_id for entry in entries] != manifest.accepted_tasks:
            blockers.append("accepted_tasks_mismatch")
        try:
            actual_index = ArtifactIndex.model_validate_json(
                (campaign_root / ARTIFACT_INDEX_NAME).read_bytes()
            )
            if {item.path for item in actual_index.artifacts} != expected_artifact_paths(entries):
                blockers.append("artifact_index_ledger_mismatch")
        except (OSError, ValidationError, ValueError):
            blockers.append("artifact_index_invalid")
        if entries:
            if entries[0].input_commit != manifest.baseline_commit:
                blockers.append("baseline_commit_mismatch")
            if entries[-1].output_commit != manifest.final_commit:
                blockers.append("final_product_commit_mismatch")
        else:
            blockers.append("accepted_entries_missing")
        blockers.extend(verify_accepted_commit_chain(worktree, artifacts_root, entries))

    full_verification_passed: bool | None = None
    blockers = list(dict.fromkeys(blockers))
    if run_full_verification and not blockers and manifest is not None:
        output = _run_clean_verification(
            worktree,
            manifest.final_verification.command,
        )
        full_verification_passed = output.exit_code == 0
        if not full_verification_passed:
            blockers.append("full_verification_failed")

    return CampaignVerification(
        ok=not blockers,
        blockers=list(dict.fromkeys(blockers)),
        entry_count=len(entries),
        accepted_tasks=[entry.task_id for entry in entries],
        baseline_commit=manifest.baseline_commit if manifest is not None else None,
        final_product_commit=manifest.final_commit if manifest is not None else None,
        manifest_commit=manifest_commit,
        full_verification_passed=full_verification_passed,
    )


def _verify_manifest_files(
    campaign_root: Path,
    metadata: dict[str, Any],
    manifest: ForgeBuildManifest,
    blockers: list[str],
) -> None:
    fixture_lock = campaign_root / "locks/fixture-lock.json"
    dataset_lock = campaign_root / "locks/dataset-lock.json"
    harness_lock = campaign_root / "locks/harness-profile.json"
    artifact_index_path = campaign_root / ARTIFACT_INDEX_NAME
    expected_paths = {
        "fixture": "locks/fixture-lock.json",
        "development_dataset": "locks/dataset-lock.json",
        "holdout_dataset": "locks/dataset-lock.json",
        "harness_profile": "locks/harness-profile.json",
        "provenance_ledger": "ledger.jsonl",
        "artifact_index": ARTIFACT_INDEX_NAME,
        "final_tree": "worktree",
    }
    if any(
        getattr(manifest, field_name).path != expected_path
        for field_name, expected_path in expected_paths.items()
    ):
        blockers.append("manifest_reference_path_mismatch")
    if _sha256_file_or_none(fixture_lock) != metadata.get("fixture_lock_sha256"):
        blockers.append("fixture_lock_digest_mismatch")
    if _sha256_file_or_none(dataset_lock) != metadata.get("dataset_lock_sha256"):
        blockers.append("dataset_lock_digest_mismatch")
    if _sha256_file_or_none(harness_lock) != metadata.get("harness_profile_sha256"):
        blockers.append("harness_lock_digest_mismatch")
    if manifest.fixture.sha256 != _sha256_file_or_none(fixture_lock):
        blockers.append("fixture_manifest_digest_mismatch")
    if manifest.harness_profile.sha256 != _sha256_file_or_none(harness_lock):
        blockers.append("harness_manifest_digest_mismatch")
    fixture = _load_json_object(fixture_lock)
    if fixture is None:
        blockers.append("fixture_lock_invalid")
    else:
        if manifest.final_verification.command != fixture.get("verification_command"):
            blockers.append("verification_command_mismatch")
        baseline_tree = tree_digest_at_commit(
            campaign_root / "worktree",
            manifest.baseline_commit,
        )
        if baseline_tree != fixture.get("canonical_tree_sha256"):
            blockers.append("baseline_fixture_tree_mismatch")

    dataset_digest = _sha256_file_or_none(dataset_lock)
    if manifest.development_dataset.sha256 != dataset_digest:
        blockers.append("development_dataset_mismatch")
    if manifest.holdout_dataset.sha256 != dataset_digest:
        blockers.append("holdout_dataset_mismatch")
    dataset = _load_json_object(dataset_lock)
    if dataset is None:
        blockers.append("dataset_lock_invalid")
    else:
        if metadata.get("development_dataset_fingerprint") != dataset.get("development", {}).get(
            "fingerprint"
        ):
            blockers.append("development_dataset_fingerprint_mismatch")
        if metadata.get("holdout_dataset_fingerprint") != dataset.get("holdout", {}).get(
            "fingerprint"
        ):
            blockers.append("holdout_dataset_fingerprint_mismatch")

    actual_index_digest = _sha256_file_or_none(artifact_index_path)
    if manifest.artifact_index.sha256 != actual_index_digest:
        blockers.append("artifact_index_digest_mismatch")
    try:
        actual_index = ArtifactIndex.model_validate_json(artifact_index_path.read_bytes())
        expected_index = build_artifact_index(campaign_root)
        if actual_index != expected_index:
            blockers.append("artifact_index_mismatch")
    except (OSError, ValidationError, ValueError):
        blockers.append("artifact_index_invalid")


def _patch_replays_to_commit(
    worktree: Path,
    entry: ForgeProvenanceEntry,
    patch_path: Path,
) -> bool:
    with tempfile.TemporaryDirectory(prefix="forge-campaign-replay-") as temporary:
        replay = Path(temporary) / "repo"
        clone = subprocess.run(
            ["git", "clone", "--quiet", "--no-hardlinks", str(worktree), str(replay)],
            capture_output=True,
            check=False,
        )
        if clone.returncode != 0:
            return False
        if _git_result(replay, "checkout", "--quiet", "--detach", entry.input_commit) != 0:
            return False
        applied = subprocess.run(
            ["git", "apply", "--index", "--whitespace=nowarn", "-"],
            cwd=replay,
            input=patch_path.read_bytes(),
            capture_output=True,
            check=False,
        )
        if applied.returncode != 0:
            return False
        replay_tree = _git_or_none(replay, "write-tree")
        output_tree = _git_or_none(worktree, "rev-parse", f"{entry.output_commit}^{{tree}}")
        return replay_tree is not None and replay_tree == output_tree


def _run_clean_verification(worktree: Path, command: str) -> ShellOutput:
    with tempfile.TemporaryDirectory(prefix="forge-campaign-verify-") as temporary:
        clean = Path(temporary) / "worktree"
        shutil.copytree(
            worktree,
            clean,
            ignore=shutil.ignore_patterns(".git", *IGNORED_TREE_PARTS),
        )
        outputs = run_shell_commands([command], clean, timeout_seconds=900)
        if len(outputs) != 1:
            raise RuntimeError("campaign full verification did not produce one result")
        return outputs[0]


def _load_json_object(path: Path) -> dict[str, Any] | None:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return payload if isinstance(payload, dict) else None


def _sha256_file_or_none(path: Path) -> str | None:
    try:
        return sha256_bytes(path.read_bytes()) if path.is_file() else None
    except OSError:
        return None


def _commit_exists(worktree: Path, commit: str) -> bool:
    try:
        return (
            subprocess.run(
                ["git", "cat-file", "-e", f"{commit}^{{commit}}"],
                cwd=worktree,
                capture_output=True,
                check=False,
            ).returncode
            == 0
        )
    except OSError:
        return False


def _git(worktree: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=worktree,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} failed")
    return completed.stdout.strip()


def _git_or_none(worktree: Path, *args: str) -> str | None:
    try:
        completed = subprocess.run(
            ["git", *args],
            cwd=worktree,
            text=True,
            capture_output=True,
            check=False,
        )
        return completed.stdout.strip() if completed.returncode == 0 else None
    except OSError:
        return None


def _git_result(worktree: Path, *args: str) -> int:
    try:
        return subprocess.run(
            ["git", *args],
            cwd=worktree,
            capture_output=True,
            check=False,
        ).returncode
    except OSError:
        return 1
