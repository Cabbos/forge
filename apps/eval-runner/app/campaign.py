import json
import os
import shutil
import subprocess
import tempfile
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from app.artifact_hashing import IGNORED_TREE_PARTS, canonical_tree_digest
from app.forgebench import verify_catalog
from app.provenance import sha256_bytes
from app.provenance_models import SECRET_PATTERN

ARTIFACT_DIRECTORIES = ("traces", "reports", "patches", "verification")


def initialize_campaign(
    campaign_root: Path,
    *,
    campaign_id: str,
    fixture: Path,
    fixture_lock: Path,
    cases: Path,
    dataset_lock: Path,
    harness_profile: Path,
    provider: str,
    model: str,
    transport: str,
    controller_version: str,
    runtime_version: str,
) -> dict[str, Any]:
    if campaign_root.exists():
        raise ValueError(f"campaign path already exists: {campaign_root}")

    fixture_identity = _verify_fixture_lock(fixture, fixture_lock)
    try:
        catalog_summary = verify_catalog(cases, dataset_lock)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        raise ValueError("dataset lock does not match ForgeBench catalog") from exc
    profile = _load_harness_profile(harness_profile, provider, model, transport)

    campaign_root.parent.mkdir(parents=True, exist_ok=True)
    temporary_root = Path(
        tempfile.mkdtemp(
            prefix=f".{campaign_root.name}.",
            dir=campaign_root.parent,
        )
    )
    try:
        metadata = _materialize_campaign(
            temporary_root,
            campaign_id=campaign_id,
            fixture=fixture,
            fixture_lock=fixture_lock,
            dataset_lock=dataset_lock,
            harness_profile=harness_profile,
            fixture_identity=fixture_identity,
            catalog_summary=catalog_summary,
            profile=profile,
            provider=provider,
            model=model,
            transport=transport,
            controller_version=controller_version,
            runtime_version=runtime_version,
        )
        os.replace(temporary_root, campaign_root)
        return metadata
    except BaseException:
        shutil.rmtree(temporary_root, ignore_errors=True)
        raise


def _verify_fixture_lock(fixture: Path, fixture_lock: Path) -> dict[str, Any]:
    _assert_fixture_is_clean(fixture)
    payload = json.loads(fixture_lock.read_text(encoding="utf-8"))
    required = {
        "fixture_id",
        "fixture_version",
        "canonical_tree_sha256",
        "verification_command",
    }
    if not required.issubset(payload):
        raise ValueError("fixture lock is incomplete")
    actual_digest = canonical_tree_digest(fixture)
    if payload["canonical_tree_sha256"] != actual_digest:
        raise ValueError("fixture lock does not match fixture contents")
    return {
        "fixture_id": payload["fixture_id"],
        "fixture_version": payload["fixture_version"],
        "canonical_tree_sha256": actual_digest,
        "verification_command": payload["verification_command"],
    }


def _assert_fixture_is_clean(fixture: Path) -> None:
    if not fixture.is_dir():
        raise ValueError(f"ForgeBench fixture does not exist: {fixture}")
    generated = sorted(
        path.relative_to(fixture).as_posix()
        for path in fixture.rglob("*")
        if path.is_dir() and path.name in IGNORED_TREE_PARTS
    )
    if generated:
        raise ValueError(f"ForgeBench fixture contains generated state: {generated}")
    for path in fixture.rglob("*"):
        if path.is_symlink() and not path.resolve().is_relative_to(fixture.resolve()):
            raise ValueError("ForgeBench fixture contains an escaping symlink")


def _load_harness_profile(
    harness_profile: Path,
    provider: str,
    model: str,
    transport: str,
) -> dict[str, Any]:
    content = harness_profile.read_text(encoding="utf-8")
    if SECRET_PATTERN.search(content) is not None:
        raise ValueError("harness profile contains secret-like content")
    payload = json.loads(content)
    if not isinstance(payload, dict):
        raise ValueError("harness profile must be a JSON object")
    expected = {"provider": provider, "model": model, "transport": transport}
    if any(payload.get(key) != value for key, value in expected.items()):
        raise ValueError("harness profile does not match the frozen campaign profile")
    return payload


def _materialize_campaign(
    root: Path,
    *,
    campaign_id: str,
    fixture: Path,
    fixture_lock: Path,
    dataset_lock: Path,
    harness_profile: Path,
    fixture_identity: dict[str, Any],
    catalog_summary: dict[str, Any],
    profile: dict[str, Any],
    provider: str,
    model: str,
    transport: str,
    controller_version: str,
    runtime_version: str,
) -> dict[str, Any]:
    locks = root / "locks"
    artifacts = root / "artifacts"
    worktree = root / "worktree"
    locks.mkdir()
    artifacts.mkdir()
    for directory in ARTIFACT_DIRECTORIES:
        (artifacts / directory).mkdir()
    shutil.copy2(fixture_lock, locks / "fixture-lock.json")
    shutil.copy2(dataset_lock, locks / "dataset-lock.json")
    shutil.copy2(harness_profile, locks / "harness-profile.json")
    shutil.copytree(fixture, worktree)

    _git(worktree, "init", "-b", "forge-built")
    _git(worktree, "config", "user.name", "Forge Integration Controller")
    _git(worktree, "config", "user.email", "forge-integration@local.invalid")
    _git(worktree, "config", "commit.gpgsign", "false")
    _git(worktree, "add", "--all")
    _git(
        worktree,
        "commit",
        "--no-gpg-sign",
        "-m",
        "chore: initialize canonical ForgeBench baseline",
    )
    baseline_commit = _git(worktree, "rev-parse", "HEAD")
    baseline_tree_digest = canonical_tree_digest(worktree)

    (root / "ledger.jsonl").write_bytes(b"")
    os.chmod(root / "ledger.jsonl", 0o600)
    metadata = {
        "schema_version": 1,
        "campaign_id": campaign_id,
        "status": "active",
        "created_at": datetime.now(UTC).isoformat(),
        "worktree": "worktree",
        "ledger": "ledger.jsonl",
        "artifacts": "artifacts",
        "locks": {
            "fixture": "locks/fixture-lock.json",
            "dataset": "locks/dataset-lock.json",
            "harness_profile": "locks/harness-profile.json",
        },
        "fixture_id": fixture_identity["fixture_id"],
        "fixture_version": fixture_identity["fixture_version"],
        "fixture_tree_sha256": fixture_identity["canonical_tree_sha256"],
        "fixture_lock_sha256": sha256_bytes((locks / "fixture-lock.json").read_bytes()),
        "dataset_lock_sha256": sha256_bytes((locks / "dataset-lock.json").read_bytes()),
        "dataset_fingerprint": catalog_summary["combined_fingerprint"],
        "harness_profile_sha256": sha256_bytes((locks / "harness-profile.json").read_bytes()),
        "harness_profile_id": profile.get("profile_id"),
        "provider": provider,
        "model": model,
        "transport": transport,
        "controller_version": controller_version,
        "runtime_version": runtime_version,
        "verification_command": fixture_identity["verification_command"],
        "baseline_commit": baseline_commit,
        "baseline_tree_sha256": baseline_tree_digest,
        "controller_interventions": [
            {
                "sequence": 0,
                "kind": "campaign_init",
                "reason": "Initialize the locked canonical ForgeBench baseline.",
                "invalidates_campaign": False,
            }
        ],
    }
    serialized = json.dumps(metadata, sort_keys=True)
    if SECRET_PATTERN.search(serialized) is not None:
        raise ValueError("campaign metadata contains secret-like content")
    _atomic_write_json(root / "campaign.json", metadata)
    return metadata


def _git(worktree: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=worktree,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} failed: {completed.stderr.strip()}")
    return completed.stdout.strip()


def _atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.",
        suffix=".tmp",
        dir=path.parent,
    )
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as temporary:
            json.dump(payload, temporary, indent=2, sort_keys=True)
            temporary.write("\n")
            temporary.flush()
            os.fsync(temporary.fileno())
        os.chmod(temporary_name, 0o600)
        os.replace(temporary_name, path)
    except BaseException:
        Path(temporary_name).unlink(missing_ok=True)
        raise
