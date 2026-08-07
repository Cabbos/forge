import fcntl
import json
import os
import re
import shutil
import subprocess
import tempfile
from collections.abc import Iterator
from contextlib import contextmanager
from datetime import UTC, datetime
from decimal import Decimal
from fnmatch import fnmatchcase
from pathlib import Path, PurePosixPath
from typing import Any, Literal

from pydantic import Field, model_validator

from app.artifact_hashing import IGNORED_TREE_PARTS, canonical_tree_digest
from app.cases import load_cases
from app.forgebench import verify_catalog
from app.hidden_verifiers import run_hidden_verifier
from app.models import (
    AgentTrace,
    BacktestReport,
    EvalModel,
    EvaluationTask,
    FailureCategory,
    ShellOutput,
    TrustStatus,
)
from app.provenance import (
    SECRET_BYTES_PATTERN,
    append_entry,
    canonical_json_bytes,
    entry_digest,
    load_verified_entries,
    sha256_bytes,
    verify_ledger,
)
from app.provenance_models import (
    SAFE_ID_PATTERN,
    SECRET_PATTERN,
    ForgeProvenanceEntry,
    GitSha1,
    Sha256,
)
from app.runner import run_shell_commands

ARTIFACT_DIRECTORIES = ("traces", "reports", "patches", "verification")
PENDING_ACCEPT_FILE = "pending-accept.json"
PENDING_ARTIFACT_DIRECTORY = ".pending"
ACCEPT_CRASH_POINTS = {
    "after_patch",
    "after_verification",
    "after_artifact_prepare",
    "after_pending_before_commit",
    "after_staging",
    "after_commit",
    "after_artifact_publish",
    "after_ledger_append",
}
SECRET_BYTES_RE = re.compile(SECRET_BYTES_PATTERN)


class CampaignCrashInjected(RuntimeError):
    """Deterministic crash used to exercise campaign recovery."""


class CampaignTraceArtifact(EvalModel):
    schema_version: Literal[1] = 1
    run_id: str
    provider: str
    model: str
    transport: str
    input_commit: GitSha1
    dataset_fingerprint: Sha256
    harness_fingerprint: Sha256
    cost_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    trace: AgentTrace

    @model_validator(mode="after")
    def validate_identity(self) -> "CampaignTraceArtifact":
        if self.trace.provider.value != "forge":
            raise ValueError("campaign trace must come from Forge")
        if self.trace.model != self.model:
            raise ValueError("campaign trace model identity does not match")
        return self


class CampaignReportArtifact(EvalModel):
    schema_version: Literal[1] = 1
    run_id: str
    provider: str
    model: str
    transport: str
    dataset_fingerprint: Sha256
    harness_fingerprint: Sha256
    cost_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    report: BacktestReport


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
    case_fingerprints = {
        task.id: sha256_bytes(canonical_json_bytes(task.model_dump(mode="json")))
        for task in load_cases(cases)
    }

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
            case_fingerprints=case_fingerprints,
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


def accept_campaign_patch(
    campaign_root: Path,
    *,
    task: EvaluationTask,
    trace_path: Path,
    report_path: Path,
    patch_path: Path,
    expected_trace_sha256: str,
    expected_report_sha256: str,
    expected_patch_sha256: str,
    crash_at: str | None = None,
) -> dict[str, str]:
    with _campaign_lock(campaign_root):
        return _accept_campaign_patch_locked(
            campaign_root,
            task=task,
            trace_path=trace_path,
            report_path=report_path,
            patch_path=patch_path,
            expected_trace_sha256=expected_trace_sha256,
            expected_report_sha256=expected_report_sha256,
            expected_patch_sha256=expected_patch_sha256,
            crash_at=crash_at,
        )


def _accept_campaign_patch_locked(
    campaign_root: Path,
    *,
    task: EvaluationTask,
    trace_path: Path,
    report_path: Path,
    patch_path: Path,
    expected_trace_sha256: str,
    expected_report_sha256: str,
    expected_patch_sha256: str,
    crash_at: str | None = None,
) -> dict[str, str]:
    """Accept one trusted Forge patch as an append-only campaign transaction."""

    if crash_at is not None and crash_at not in ACCEPT_CRASH_POINTS:
        raise ValueError(f"unknown campaign crash point: {crash_at}")
    state = _load_campaign_state(campaign_root, refuse_pending=True)
    worktree = state["worktree"]
    input_commit = _git(worktree, "rev-parse", "HEAD")
    if _git(worktree, "status", "--porcelain"):
        raise ValueError("campaign worktree must be clean before accepting a patch")
    if input_commit != state["expected_input_commit"]:
        raise ValueError("campaign worktree HEAD breaks provenance commit continuity")

    trace_bytes = trace_path.read_bytes()
    report_bytes = report_path.read_bytes()
    patch_bytes = patch_path.read_bytes()
    _assert_explicit_digest("trace", trace_bytes, expected_trace_sha256)
    _assert_explicit_digest("report", report_bytes, expected_report_sha256)
    _assert_explicit_digest("patch", patch_bytes, expected_patch_sha256)
    _assert_secret_free_artifact("trace", trace_bytes)
    _assert_secret_free_artifact("report", report_bytes)
    _assert_secret_free_artifact("patch", patch_bytes)

    trace_artifact = CampaignTraceArtifact.model_validate_json(trace_bytes)
    report_artifact = CampaignReportArtifact.model_validate_json(report_bytes)
    _validate_acceptance_evidence(
        state,
        task=task,
        trace_artifact=trace_artifact,
        report_artifact=report_artifact,
        patch_bytes=patch_bytes,
        input_commit=input_commit,
    )
    trace = trace_artifact.trace
    changed_files = _validated_changed_files(task, trace)
    expected_patch = "\n".join(file_diff.diff for file_diff in trace.file_diffs).encode()
    if patch_bytes != expected_patch:
        raise ValueError("patch bytes do not exactly match the trusted Forge trace")

    _git_apply(worktree, patch_bytes, check=True)
    patch_applied = False
    pending_written = False
    commit_created = False
    temporary_directory: Path | None = None
    try:
        _git_apply(worktree, patch_bytes)
        patch_applied = True
        actual_changed = _status_changed_files(worktree)
        if actual_changed != changed_files:
            raise ValueError("applied patch changes do not exactly match trusted trace files")
        for relative_path in changed_files:
            if (worktree / relative_path).is_symlink():
                raise ValueError("accepted patch may not create or modify symlinks")
        if crash_at == "after_patch":
            raise CampaignCrashInjected(crash_at)

        verification = _run_accept_verification(task, worktree)
        if verification.get("passed") is not True:
            raise ValueError("independent campaign verification failed")
        if crash_at == "after_verification":
            raise CampaignCrashInjected(crash_at)
        verification_payload = {
            "schema_version": 1,
            "run_id": trace_artifact.run_id,
            "task_id": task.id,
            "input_commit": input_commit,
            "verified_at": datetime.now(UTC).isoformat(),
            **verification,
        }
        verification_bytes = canonical_json_bytes(verification_payload) + b"\n"
        _assert_secret_free_artifact("verification", verification_bytes)

        temporary_directory, temporary_artifacts = _prepare_pending_artifacts(
            campaign_root,
            run_id=trace_artifact.run_id,
            trace_bytes=trace_bytes,
            report_bytes=report_bytes,
            patch_bytes=patch_bytes,
            verification_bytes=verification_bytes,
        )
        if crash_at == "after_artifact_prepare":
            raise CampaignCrashInjected(crash_at)
        entry_template = {
            "schema_version": 1,
            "sequence": len(state["entries"]) + 1,
            "previous_entry_sha256": (
                entry_digest(state["entries"][-1]) if state["entries"] else None
            ),
            "task_id": task.id,
            "run_id": trace_artifact.run_id,
            "agent": "forge",
            "provider": trace_artifact.provider,
            "model": trace_artifact.model,
            "transport": trace_artifact.transport,
            "input_commit": input_commit,
            "patch_sha256": expected_patch_sha256,
            "trace_sha256": expected_trace_sha256,
            "report_sha256": expected_report_sha256,
            "harness_fingerprint": trace_artifact.harness_fingerprint,
            "dataset_fingerprint": trace_artifact.dataset_fingerprint,
            "verification_digest": sha256_bytes(verification_bytes),
            "trust_status": "trusted",
            "cost_cny": str(trace_artifact.cost_cny),
        }
        pending = {
            "schema_version": 1,
            "phase": "prepared",
            "task_id": task.id,
            "run_id": trace_artifact.run_id,
            "input_commit": input_commit,
            "changed_files": changed_files,
            "temporary_directory": temporary_directory.relative_to(campaign_root).as_posix(),
            "temporary_artifacts": temporary_artifacts,
            "entry_template": entry_template,
        }
        _atomic_write_json(campaign_root / PENDING_ACCEPT_FILE, pending)
        pending_written = True
        if crash_at == "after_pending_before_commit":
            raise CampaignCrashInjected(crash_at)

        _git(worktree, "add", "--", *changed_files)
        staged_files = _staged_changed_files(worktree)
        if staged_files != changed_files:
            raise ValueError("staged campaign patch differs from trusted trace scope")
        pending["phase"] = "commit_started"
        _atomic_write_json(campaign_root / PENDING_ACCEPT_FILE, pending)
        if crash_at == "after_staging":
            raise CampaignCrashInjected(crash_at)
        _commit_accepted_patch(
            worktree,
            task_id=task.id,
            run_id=trace_artifact.run_id,
            trace_sha256=expected_trace_sha256,
            patch_sha256=expected_patch_sha256,
        )
        commit_created = True
        output_commit = _git(worktree, "rev-parse", "HEAD")
        entry = ForgeProvenanceEntry.model_validate(
            {**entry_template, "output_commit": output_commit}
        )
        pending["phase"] = "committed"
        pending["output_commit"] = output_commit
        pending["entry"] = entry.model_dump(mode="json")
        _atomic_write_json(campaign_root / PENDING_ACCEPT_FILE, pending)
        if crash_at == "after_commit":
            raise CampaignCrashInjected(crash_at)

        _finalize_pending_accept(
            campaign_root,
            pending,
            entry,
            crash_at=crash_at,
        )
        return {
            "status": "accepted",
            "task_id": task.id,
            "run_id": trace_artifact.run_id,
            "input_commit": input_commit,
            "output_commit": output_commit,
        }
    except CampaignCrashInjected:
        if not pending_written and patch_applied:
            _rollback_applied_patch(worktree, patch_bytes)
            if temporary_directory is not None:
                _discard_unpublished_artifacts(campaign_root, temporary_directory)
        raise
    except BaseException:
        if commit_created:
            raise
        if pending_written:
            try:
                _recover_pending_accept_locked(campaign_root)
            except BaseException as recovery_error:
                raise RuntimeError(
                    "campaign accept failed and exact recovery also failed"
                ) from recovery_error
        elif patch_applied:
            _rollback_applied_patch(worktree, patch_bytes)
            if temporary_directory is not None:
                _discard_unpublished_artifacts(campaign_root, temporary_directory)
        raise


def recover_pending_accept(campaign_root: Path) -> dict[str, str]:
    with _campaign_lock(campaign_root):
        return _recover_pending_accept_locked(campaign_root)


def _recover_pending_accept_locked(campaign_root: Path) -> dict[str, str]:
    """Recover one exact pending accept without resetting or cleaning the worktree."""

    pending_path = campaign_root / PENDING_ACCEPT_FILE
    if not pending_path.is_file():
        return {"status": "nothing_pending"}
    state = _load_campaign_state(campaign_root, refuse_pending=False)
    pending = _load_pending_accept(campaign_root, pending_path)
    worktree = state["worktree"]
    head = _git(worktree, "rev-parse", "HEAD")
    phase = pending["phase"]

    if phase == "prepared":
        if head != pending["input_commit"]:
            raise ValueError("prepared campaign transaction is no longer at input commit")
        if _staged_changed_files(worktree):
            raise ValueError("prepared campaign transaction has unexpected staged changes")
        if _status_changed_files(worktree) != sorted(pending["changed_files"]):
            raise ValueError("prepared campaign transaction worktree has drifted")
    if phase == "commit_started" and head == pending["input_commit"]:
        if _staged_changed_files(worktree) != sorted(pending["changed_files"]):
            raise ValueError("started campaign transaction index has drifted")
        if _working_tree_changed_files(worktree):
            raise ValueError("started campaign transaction worktree has drifted")
    if phase == "prepared" or (phase == "commit_started" and head == pending["input_commit"]):
        changed_files = pending["changed_files"]
        if phase == "commit_started":
            _git(worktree, "restore", "--staged", "--", *changed_files)
        patch_bytes = _read_pending_artifact(
            campaign_root, pending, "patches", expected_suffix=".patch"
        )
        _rollback_applied_patch(worktree, patch_bytes)
        _remove_pending_transaction(campaign_root, pending)
        return {
            "status": "rolled_back",
            "task_id": pending["task_id"],
            "run_id": pending["run_id"],
        }

    if phase == "commit_started":
        if _git(worktree, "rev-parse", f"{head}^") != pending["input_commit"]:
            raise ValueError("pending campaign commit does not descend from input commit")
        _verify_pending_commit_message(worktree, pending)
        entry = ForgeProvenanceEntry.model_validate(
            {**pending["entry_template"], "output_commit": head}
        )
        pending["phase"] = "committed"
        pending["output_commit"] = head
        pending["entry"] = entry.model_dump(mode="json")
        _atomic_write_json(pending_path, pending)
    elif phase == "committed":
        entry = ForgeProvenanceEntry.model_validate(pending["entry"])
    else:
        raise ValueError("pending campaign transaction has an unknown phase")

    if head != entry.output_commit:
        raise ValueError("pending campaign output commit does not match HEAD")
    _verify_pending_commit_message(worktree, pending)
    _finalize_pending_accept(campaign_root, pending, entry)
    return {
        "status": "completed",
        "task_id": entry.task_id,
        "run_id": entry.run_id,
        "output_commit": entry.output_commit,
    }


@contextmanager
def _campaign_lock(campaign_root: Path) -> Iterator[None]:
    if not campaign_root.is_dir():
        raise ValueError("campaign path does not exist")
    lock_path = campaign_root / ".campaign.lock"
    descriptor = os.open(lock_path, os.O_CREAT | os.O_RDWR, 0o600)
    os.chmod(lock_path, 0o600)
    try:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as exc:
            raise ValueError("campaign is locked by another mutation") from exc
        yield
    finally:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
        finally:
            os.close(descriptor)


def _load_campaign_state(
    campaign_root: Path,
    *,
    refuse_pending: bool,
) -> dict[str, Any]:
    metadata_path = campaign_root / "campaign.json"
    if not metadata_path.is_file():
        raise ValueError("campaign metadata is missing")
    metadata_bytes = metadata_path.read_bytes()
    _assert_secret_free_artifact("campaign metadata", metadata_bytes)
    metadata = json.loads(metadata_bytes)
    if not isinstance(metadata, dict) or metadata.get("schema_version") != 1:
        raise ValueError("campaign metadata schema is unsupported")
    if metadata.get("status") != "active":
        raise ValueError("campaign is not active")
    expected_paths = {
        "worktree": "worktree",
        "ledger": "ledger.jsonl",
        "artifacts": "artifacts",
    }
    if any(metadata.get(key) != value for key, value in expected_paths.items()):
        raise ValueError("campaign metadata paths are not canonical")
    expected_locks = {
        "fixture": "locks/fixture-lock.json",
        "dataset": "locks/dataset-lock.json",
        "harness_profile": "locks/harness-profile.json",
    }
    if metadata.get("locks") != expected_locks:
        raise ValueError("campaign lock paths are not canonical")

    worktree = campaign_root / "worktree"
    ledger = campaign_root / "ledger.jsonl"
    artifacts = campaign_root / "artifacts"
    if not (worktree / ".git").is_dir():
        raise ValueError("campaign worktree git repository is missing")
    for directory in ARTIFACT_DIRECTORIES:
        if not (artifacts / directory).is_dir():
            raise ValueError(f"campaign artifact directory is missing: {directory}")

    lock_specs = {
        "fixture_lock_sha256": campaign_root / "locks/fixture-lock.json",
        "dataset_lock_sha256": campaign_root / "locks/dataset-lock.json",
        "harness_profile_sha256": campaign_root / "locks/harness-profile.json",
    }
    for metadata_key, lock_path in lock_specs.items():
        if not lock_path.is_file():
            raise ValueError(f"campaign lock is missing: {lock_path.name}")
        if sha256_bytes(lock_path.read_bytes()) != metadata.get(metadata_key):
            raise ValueError(f"campaign lock has drifted: {lock_path.name}")
    profile = json.loads((campaign_root / "locks/harness-profile.json").read_text())
    for key in ("provider", "model", "transport"):
        if profile.get(key) != metadata.get(key):
            raise ValueError("campaign harness profile identity has drifted")
    dataset_lock = json.loads(
        (campaign_root / "locks/dataset-lock.json").read_text(encoding="utf-8")
    )
    expected_dataset_fields = {
        "dataset_fingerprint": dataset_lock.get("combined_fingerprint"),
        "development_dataset_fingerprint": dataset_lock.get("development", {}).get("fingerprint"),
        "holdout_dataset_fingerprint": dataset_lock.get("holdout", {}).get("fingerprint"),
    }
    if any(metadata.get(key) != value for key, value in expected_dataset_fields.items()):
        raise ValueError("campaign dataset identities have drifted")
    expected_case_ids = set(dataset_lock.get("development", {}).get("ids", [])) | set(
        dataset_lock.get("holdout", {}).get("ids", [])
    )
    case_fingerprints = metadata.get("case_fingerprints")
    if (
        not isinstance(case_fingerprints, dict)
        or set(case_fingerprints) != expected_case_ids
        or any(
            not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None
            for value in case_fingerprints.values()
        )
    ):
        raise ValueError("campaign frozen case fingerprints are invalid")

    ledger_result = verify_ledger(ledger, artifacts)
    if not ledger_result.ok:
        raise ValueError(
            "campaign provenance ledger is invalid: " + ", ".join(ledger_result.blockers)
        )
    entries = load_verified_entries(ledger, artifacts)
    expected_input_commit = (
        entries[-1].output_commit if entries else metadata.get("baseline_commit")
    )
    if not isinstance(expected_input_commit, str):
        raise ValueError("campaign expected input commit is missing")
    if refuse_pending and (campaign_root / PENDING_ACCEPT_FILE).exists():
        raise ValueError("campaign has a pending accept; recover it before continuing")
    return {
        "metadata": metadata,
        "dataset_lock": dataset_lock,
        "worktree": worktree,
        "ledger": ledger,
        "artifacts": artifacts,
        "entries": entries,
        "expected_input_commit": expected_input_commit,
    }


def _validate_acceptance_evidence(
    state: dict[str, Any],
    *,
    task: EvaluationTask,
    trace_artifact: CampaignTraceArtifact,
    report_artifact: CampaignReportArtifact,
    patch_bytes: bytes,
    input_commit: str,
) -> None:
    metadata = state["metadata"]
    trace = trace_artifact.trace
    report = report_artifact.report
    if SAFE_ID_PATTERN.fullmatch(trace_artifact.run_id) is None:
        raise ValueError("campaign run id must be a safe identifier")
    if SAFE_ID_PATTERN.fullmatch(task.id) is None:
        raise ValueError("campaign task id must be a safe identifier")
    entries: list[ForgeProvenanceEntry] = state["entries"]
    if any(entry.task_id == task.id for entry in entries):
        raise ValueError("campaign task id is already present in provenance ledger")
    if any(entry.run_id == trace_artifact.run_id for entry in entries):
        raise ValueError("campaign run id is already present in provenance ledger")
    if any(entry.patch_sha256 == sha256_bytes(patch_bytes) for entry in entries):
        raise ValueError("campaign patch is already present in provenance ledger")
    task_fingerprint = sha256_bytes(canonical_json_bytes(task.model_dump(mode="json")))
    if metadata.get("case_fingerprints", {}).get(task.id) != task_fingerprint:
        raise ValueError("campaign task definition does not match the frozen catalog")
    identity_fields = (
        "run_id",
        "provider",
        "model",
        "transport",
        "dataset_fingerprint",
        "harness_fingerprint",
        "cost_cny",
    )
    for field_name in identity_fields:
        if getattr(trace_artifact, field_name) != getattr(report_artifact, field_name):
            raise ValueError(f"trace and report {field_name} identities do not match")
    for field_name in ("provider", "model", "transport"):
        if getattr(trace_artifact, field_name) != metadata.get(field_name):
            raise ValueError(f"campaign {field_name} identity does not match")
    if trace_artifact.harness_fingerprint != metadata.get("harness_profile_sha256"):
        raise ValueError("campaign harness fingerprint does not match")
    if task.dataset_split is None:
        raise ValueError("campaign task has no dataset split")
    split_key = f"{task.dataset_split.value}_dataset_fingerprint"
    if trace_artifact.dataset_fingerprint != metadata.get(split_key):
        raise ValueError("campaign dataset fingerprint does not match task split")
    if trace_artifact.input_commit != input_commit:
        raise ValueError("trusted trace input commit does not match campaign HEAD")

    if trace.task_id != task.id or trace.user_prompt != task.prompt:
        raise ValueError("trusted trace task identity does not match")
    if trace.error is not None or trace.failure_category != FailureCategory.NONE:
        raise ValueError("trusted trace records a failed Forge run")
    if trace.verification_result is None or not trace.verification_result.passed:
        raise ValueError("trusted trace has no passing verification")
    if trace.sandbox_scrub is None or not trace.sandbox_scrub.ok:
        raise ValueError("trusted trace sandbox evidence is not clean")
    if trace.patch_replay is None or not trace.patch_replay.ok:
        raise ValueError("trusted trace patch replay did not pass")
    observation = trace.workspace_observation
    if observation is None or not observation.available:
        raise ValueError("trusted trace has no workspace observation")
    if observation.error or observation.mismatch_files:
        raise ValueError("trusted trace workspace observation has a mismatch")
    if trace.scope_violations:
        raise ValueError("trusted trace reports scope violations")
    if not trace.file_diffs or not patch_bytes:
        raise ValueError("campaign accept requires a non-empty Forge patch")

    if report.trust_result.status != TrustStatus.TRUSTED or not report.trust_result.trusted:
        raise ValueError("campaign report is not trusted")
    if report.trust_result.blockers:
        raise ValueError("campaign report contains trust blockers")
    if report.total_tasks != 1 or len(report.tasks) != 1:
        raise ValueError("campaign report must contain exactly one task")
    task_summary = report.tasks[0]
    if task_summary.task_id != task.id:
        raise ValueError("campaign report task identity does not match")
    if not task_summary.passed or task_summary.verification_passed is not True:
        raise ValueError("campaign report task did not pass")
    if task_summary.scope_violations:
        raise ValueError("campaign report task has scope violations")


def _validated_changed_files(
    task: EvaluationTask,
    trace: AgentTrace,
) -> list[str]:
    changed_files = sorted(set(trace.changed_files))
    if len(changed_files) != len(trace.changed_files):
        raise ValueError("trusted trace changed files contain duplicates")
    diff_files = sorted(file_diff.path for file_diff in trace.file_diffs)
    if changed_files != diff_files:
        raise ValueError("trusted trace file diffs do not match changed files")
    observation = trace.workspace_observation
    assert observation is not None
    if sorted(observation.changed_files) != changed_files:
        raise ValueError("workspace observation does not match trusted changed files")
    observed_union = sorted(
        set(observation.added_files + observation.modified_files + observation.deleted_files)
    )
    if observed_union and observed_union != changed_files:
        raise ValueError("workspace observation change kinds do not match changed files")
    if sorted(observation.reported_changed_files) != changed_files:
        raise ValueError("Forge-reported changed files do not match observed files")

    allowed = task.metadata.get("allowed_source_paths")
    allowed_patterns = (
        [str(item) for item in allowed]
        if isinstance(allowed, list)
        else list(task.expected_files_changed)
    )
    if not allowed_patterns:
        raise ValueError("campaign task has no allowed product source paths")
    for value in changed_files:
        _validate_campaign_relative_path(value)
        if not _matches_any(value, allowed_patterns):
            raise ValueError(f"patch changes a file outside task scope: {value}")
        if _matches_any(value, task.forbidden_files_changed):
            raise ValueError(f"patch changes a forbidden file: {value}")
    return changed_files


def _validate_campaign_relative_path(value: str) -> None:
    path = PurePosixPath(value)
    if (
        not value
        or path.is_absolute()
        or ".." in path.parts
        or value == "."
        or "\\" in value
        or any(part in IGNORED_TREE_PARTS for part in path.parts)
    ):
        raise ValueError(f"unsafe campaign patch path: {value}")


def _matches_any(value: str, patterns: list[str]) -> bool:
    return any(fnmatchcase(value, pattern) for pattern in patterns)


def _assert_explicit_digest(kind: str, content: bytes, expected: str) -> None:
    if not re.fullmatch(r"[0-9a-f]{64}", expected):
        raise ValueError(f"expected {kind} digest is not a SHA-256")
    if sha256_bytes(content) != expected:
        raise ValueError(f"{kind} artifact digest does not match explicit digest")


def _assert_secret_free_artifact(kind: str, content: bytes) -> None:
    if SECRET_BYTES_RE.search(content) is not None:
        raise ValueError(f"{kind} contains secret-like content")


def _run_accept_verification(
    task: EvaluationTask,
    worktree: Path,
) -> dict[str, Any]:
    if (
        not task.validation_commands
        or task.hidden_verifier_id is None
        or task.full_verification_command is None
    ):
        raise ValueError("campaign task has incomplete independent verification")
    timeout_seconds = float(task.max_duration_seconds or 900)
    with tempfile.TemporaryDirectory(prefix="forge-accept-verify-") as temporary:
        verification_worktree = Path(temporary) / "worktree"
        shutil.copytree(
            worktree,
            verification_worktree,
            ignore=shutil.ignore_patterns(".git", *IGNORED_TREE_PARTS),
        )
        _overlay_verification_scenario(task, verification_worktree)
        checks: list[dict[str, Any]] = []
        if task.setup_commands:
            setup_outputs = run_shell_commands(
                task.setup_commands,
                verification_worktree,
                timeout_seconds=timeout_seconds,
            )
            checks.extend(_verification_checks("setup", setup_outputs))
            if not _outputs_passed(setup_outputs, len(task.setup_commands)):
                return {"passed": False, "checks": checks}

        visible_outputs = run_shell_commands(
            task.validation_commands,
            verification_worktree,
            timeout_seconds=timeout_seconds,
        )
        checks.extend(_verification_checks("visible", visible_outputs))
        if not _outputs_passed(visible_outputs, len(task.validation_commands)):
            return {"passed": False, "checks": checks}

        hidden_output = run_hidden_verifier(
            task.hidden_verifier_id,
            verification_worktree,
            timeout_seconds=timeout_seconds,
        )
        checks.extend(_verification_checks("hidden", [hidden_output]))
        if hidden_output.exit_code != 0:
            return {"passed": False, "checks": checks}

        full_outputs = run_shell_commands(
            [task.full_verification_command],
            verification_worktree,
            timeout_seconds=timeout_seconds,
        )
        checks.extend(_verification_checks("full", full_outputs))
        return {
            "passed": _outputs_passed(full_outputs, 1),
            "checks": checks,
        }


def _overlay_verification_scenario(
    task: EvaluationTask,
    verification_worktree: Path,
) -> None:
    if task.scenario_path is None:
        return
    scenario = Path(task.scenario_path)
    if not scenario.is_dir() or scenario.is_symlink():
        raise ValueError("campaign verification scenario is invalid")
    protected = set(task.expected_files_changed)
    root = verification_worktree.resolve()
    for source in sorted(
        scenario.rglob("*"),
        key=lambda path: path.relative_to(scenario).as_posix(),
    ):
        relative = source.relative_to(scenario)
        relative_posix = relative.as_posix()
        if source.is_symlink():
            raise ValueError("campaign verification scenario contains a symlink")
        if relative_posix in protected:
            continue
        target = verification_worktree / relative
        if not target.resolve(strict=False).is_relative_to(root):
            raise ValueError("campaign verification scenario escapes the workspace")
        if source.is_dir():
            target.mkdir(parents=True, exist_ok=True)
        elif source.is_file():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)


def _verification_checks(
    kind: str,
    outputs: list[ShellOutput],
) -> list[dict[str, Any]]:
    return [
        {
            "kind": kind,
            "command": output.command,
            "exit_code": output.exit_code,
            "duration_ms": output.duration_ms,
            "stdout_sha256": sha256_bytes(output.stdout.encode()),
            "stderr_sha256": sha256_bytes(output.stderr.encode()),
        }
        for output in outputs
    ]


def _outputs_passed(outputs: list[ShellOutput], expected_count: int) -> bool:
    return len(outputs) == expected_count and all(output.exit_code == 0 for output in outputs)


def _prepare_pending_artifacts(
    campaign_root: Path,
    *,
    run_id: str,
    trace_bytes: bytes,
    report_bytes: bytes,
    patch_bytes: bytes,
    verification_bytes: bytes,
) -> tuple[Path, dict[str, dict[str, str]]]:
    temporary_directory = campaign_root / "artifacts" / PENDING_ARTIFACT_DIRECTORY / run_id
    if temporary_directory.exists():
        raise ValueError("pending artifact directory already exists for run")
    final_specs = {
        "traces": (f"{run_id}.json", trace_bytes),
        "reports": (f"{run_id}.json", report_bytes),
        "patches": (f"{run_id}.patch", patch_bytes),
        "verification": (f"{run_id}.json", verification_bytes),
    }
    for kind, (file_name, _content) in final_specs.items():
        if (campaign_root / "artifacts" / kind / file_name).exists():
            raise ValueError(f"campaign artifact already exists for run: {kind}")
    temporary_directory.mkdir(parents=True)
    artifacts: dict[str, dict[str, str]] = {}
    try:
        for kind, (file_name, content) in final_specs.items():
            temporary_path = temporary_directory / kind / file_name
            temporary_path.parent.mkdir()
            _write_bytes_secure(temporary_path, content)
            artifacts[kind] = {
                "temporary_path": temporary_path.relative_to(campaign_root).as_posix(),
                "final_path": (Path("artifacts") / kind / file_name).as_posix(),
                "sha256": sha256_bytes(content),
            }
        _fsync_directory(temporary_directory)
        return temporary_directory, artifacts
    except BaseException:
        shutil.rmtree(temporary_directory, ignore_errors=True)
        raise


def _commit_accepted_patch(
    worktree: Path,
    *,
    task_id: str,
    run_id: str,
    trace_sha256: str,
    patch_sha256: str,
) -> None:
    subject = f"feat(forgebench): accept {task_id}"
    trailers = "\n".join(
        (
            f"Forge-Run-Id: {run_id}",
            f"Forge-Task-Id: {task_id}",
            f"Forge-Trace-SHA256: {trace_sha256}",
            f"Forge-Patch-SHA256: {patch_sha256}",
        )
    )
    _git(
        worktree,
        "commit",
        "--no-gpg-sign",
        "-m",
        subject,
        "-m",
        trailers,
    )


def _finalize_pending_accept(
    campaign_root: Path,
    pending: dict[str, Any],
    entry: ForgeProvenanceEntry,
    *,
    crash_at: str | None = None,
) -> None:
    state = _load_campaign_state(campaign_root, refuse_pending=False)
    worktree = state["worktree"]
    if _git(worktree, "rev-parse", "HEAD") != entry.output_commit:
        raise ValueError("cannot finalize pending accept at a different HEAD")
    if _git(worktree, "status", "--porcelain"):
        raise ValueError("cannot finalize pending accept with a dirty worktree")
    existing_entries = state["entries"]
    if len(existing_entries) not in {entry.sequence - 1, entry.sequence}:
        raise ValueError("pending provenance sequence no longer follows ledger")
    predecessor_entries = (
        existing_entries[:-1] if len(existing_entries) == entry.sequence else existing_entries
    )
    expected_previous = entry_digest(predecessor_entries[-1]) if predecessor_entries else None
    if entry.previous_entry_sha256 != expected_previous:
        raise ValueError("pending provenance predecessor no longer matches ledger")
    if len(existing_entries) == entry.sequence and existing_entries[-1] != entry:
        raise ValueError("ledger already contains a conflicting pending entry")

    for kind in ARTIFACT_DIRECTORIES:
        spec = pending["temporary_artifacts"][kind]
        temporary_path = _safe_campaign_path(
            campaign_root, spec["temporary_path"], require_file=False
        )
        final_path = _safe_campaign_path(campaign_root, spec["final_path"], require_file=False)
        if temporary_path.is_file():
            content = temporary_path.read_bytes()
        elif final_path.is_file():
            content = final_path.read_bytes()
        else:
            raise ValueError(f"pending {kind} artifact is missing")
        if sha256_bytes(content) != spec["sha256"]:
            raise ValueError(f"pending {kind} artifact digest has drifted")
        if final_path.exists():
            if not final_path.is_file() or final_path.read_bytes() != content:
                raise ValueError(f"final {kind} artifact conflicts with pending artifact")
            temporary_path.unlink(missing_ok=True)
        else:
            os.replace(temporary_path, final_path)
            os.chmod(final_path, 0o600)
            _fsync_directory(final_path.parent)
    if crash_at == "after_artifact_publish":
        raise CampaignCrashInjected(crash_at)

    current_entries = load_verified_entries(
        campaign_root / "ledger.jsonl",
        campaign_root / "artifacts",
    )
    if len(current_entries) == entry.sequence:
        if current_entries[-1] != entry:
            raise ValueError("ledger already contains a conflicting pending entry")
    elif len(current_entries) == entry.sequence - 1:
        append_entry(
            campaign_root / "ledger.jsonl",
            entry,
            campaign_root / "artifacts",
        )
    else:
        raise ValueError("ledger sequence changed while finalizing pending accept")
    if crash_at == "after_ledger_append":
        raise CampaignCrashInjected(crash_at)

    result = verify_ledger(
        campaign_root / "ledger.jsonl",
        campaign_root / "artifacts",
    )
    if not result.ok or result.output_commits[-1:] != [entry.output_commit]:
        raise ValueError("finalized provenance ledger did not verify")
    _remove_pending_transaction(campaign_root, pending)


def _load_pending_accept(
    campaign_root: Path,
    pending_path: Path,
) -> dict[str, Any]:
    payload = json.loads(pending_path.read_text(encoding="utf-8"))
    required = {
        "schema_version",
        "phase",
        "task_id",
        "run_id",
        "input_commit",
        "changed_files",
        "temporary_directory",
        "temporary_artifacts",
        "entry_template",
    }
    if not isinstance(payload, dict) or not required.issubset(payload):
        raise ValueError("pending campaign transaction is incomplete")
    if payload["schema_version"] != 1:
        raise ValueError("pending campaign transaction schema is unsupported")
    if payload["phase"] not in {"prepared", "commit_started", "committed"}:
        raise ValueError("pending campaign transaction phase is invalid")
    if not isinstance(payload["changed_files"], list) or not payload["changed_files"]:
        raise ValueError("pending campaign transaction changed files are invalid")
    for path in payload["changed_files"]:
        if not isinstance(path, str):
            raise ValueError("pending campaign transaction path is invalid")
        _validate_campaign_relative_path(path)
    expected_temporary = (
        Path("artifacts") / PENDING_ARTIFACT_DIRECTORY / payload["run_id"]
    ).as_posix()
    if payload["temporary_directory"] != expected_temporary:
        raise ValueError("pending campaign temporary directory is not canonical")
    expected_artifact_keys = set(ARTIFACT_DIRECTORIES)
    artifacts = payload["temporary_artifacts"]
    if not isinstance(artifacts, dict) or set(artifacts) != expected_artifact_keys:
        raise ValueError("pending campaign artifact index is invalid")
    for kind, spec in artifacts.items():
        if not isinstance(spec, dict) or set(spec) != {
            "temporary_path",
            "final_path",
            "sha256",
        }:
            raise ValueError(f"pending {kind} artifact metadata is invalid")
        temporary = PurePosixPath(spec["temporary_path"])
        final = PurePosixPath(spec["final_path"])
        if temporary.parent.as_posix() != f"{expected_temporary}/{kind}":
            raise ValueError(f"pending {kind} temporary path is not canonical")
        if final.parent.as_posix() != f"artifacts/{kind}":
            raise ValueError(f"pending {kind} final path is not canonical")
        if temporary.name != final.name:
            raise ValueError(f"pending {kind} artifact names do not match")
        if re.fullmatch(r"[0-9a-f]{64}", spec["sha256"]) is None:
            raise ValueError(f"pending {kind} artifact digest is invalid")
        temporary_path = _safe_campaign_path(
            campaign_root, spec["temporary_path"], require_file=False
        )
        final_path = _safe_campaign_path(campaign_root, spec["final_path"], require_file=False)
        if not temporary_path.is_file() and not final_path.is_file():
            raise ValueError(f"pending {kind} artifact is missing")
        for existing_path in (temporary_path, final_path):
            if existing_path.exists() and (
                not existing_path.is_file()
                or sha256_bytes(existing_path.read_bytes()) != spec["sha256"]
            ):
                raise ValueError(f"pending {kind} artifact digest has drifted")
    return payload


def _read_pending_artifact(
    campaign_root: Path,
    pending: dict[str, Any],
    kind: str,
    *,
    expected_suffix: str,
) -> bytes:
    spec = pending["temporary_artifacts"][kind]
    path = _safe_campaign_path(
        campaign_root,
        spec["temporary_path"],
        require_file=True,
    )
    if path.suffix != expected_suffix:
        raise ValueError(f"pending {kind} artifact has an invalid suffix")
    content = path.read_bytes()
    if sha256_bytes(content) != spec["sha256"]:
        raise ValueError(f"pending {kind} artifact digest has drifted")
    return content


def _verify_pending_commit_message(
    worktree: Path,
    pending: dict[str, Any],
) -> None:
    message = _git(worktree, "log", "-1", "--pretty=%B")
    template = pending["entry_template"]
    expected = (
        f"Forge-Run-Id: {pending['run_id']}",
        f"Forge-Task-Id: {pending['task_id']}",
        f"Forge-Trace-SHA256: {template['trace_sha256']}",
        f"Forge-Patch-SHA256: {template['patch_sha256']}",
    )
    if any(trailer not in message.splitlines() for trailer in expected):
        raise ValueError("pending campaign commit trailers do not match transaction")
    committed_files = sorted(
        path
        for path in _git(
            worktree, "diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"
        ).splitlines()
        if path
    )
    if committed_files != sorted(pending["changed_files"]):
        raise ValueError("pending campaign commit scope does not match transaction")


def _remove_pending_transaction(
    campaign_root: Path,
    pending: dict[str, Any],
) -> None:
    temporary_directory = _safe_campaign_path(
        campaign_root,
        pending["temporary_directory"],
        require_file=False,
    )
    if temporary_directory.exists():
        shutil.rmtree(temporary_directory)
    pending_root = campaign_root / "artifacts" / PENDING_ARTIFACT_DIRECTORY
    if pending_root.is_dir() and not any(pending_root.iterdir()):
        pending_root.rmdir()
    (campaign_root / PENDING_ACCEPT_FILE).unlink(missing_ok=True)
    _fsync_directory(campaign_root)


def _discard_unpublished_artifacts(
    campaign_root: Path,
    temporary_directory: Path,
) -> None:
    if temporary_directory.exists():
        shutil.rmtree(temporary_directory)
    pending_root = campaign_root / "artifacts" / PENDING_ARTIFACT_DIRECTORY
    if pending_root.is_dir() and not any(pending_root.iterdir()):
        pending_root.rmdir()


def _safe_campaign_path(
    campaign_root: Path,
    relative_path: str,
    *,
    require_file: bool,
) -> Path:
    pure_path = PurePosixPath(relative_path)
    if not relative_path or pure_path.is_absolute() or ".." in pure_path.parts:
        raise ValueError("campaign transaction contains an unsafe path")
    root = campaign_root.resolve()
    path = campaign_root / pure_path
    if not path.resolve(strict=False).is_relative_to(root):
        raise ValueError("campaign transaction path escapes campaign root")
    if require_file and not path.is_file():
        raise ValueError(f"campaign transaction artifact is missing: {relative_path}")
    return path


def _git_apply(
    worktree: Path,
    patch_bytes: bytes,
    *,
    check: bool = False,
    reverse: bool = False,
) -> None:
    arguments = ["git", "apply", "--whitespace=nowarn"]
    if check:
        arguments.append("--check")
    if reverse:
        arguments.append("--reverse")
    arguments.append("-")
    completed = subprocess.run(
        arguments,
        cwd=worktree,
        input=patch_bytes,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        detail = completed.stderr.decode(errors="replace").strip()
        raise ValueError(f"campaign patch cannot be applied exactly: {detail}")


def _rollback_applied_patch(worktree: Path, patch_bytes: bytes) -> None:
    _git_apply(worktree, patch_bytes, check=True, reverse=True)
    _git_apply(worktree, patch_bytes, reverse=True)
    if _git(worktree, "status", "--porcelain"):
        raise ValueError("exact campaign patch rollback did not restore a clean worktree")


def _working_tree_changed_files(worktree: Path) -> list[str]:
    return sorted(
        path for path in _git(worktree, "diff", "--name-only", "--no-renames").splitlines() if path
    )


def _status_changed_files(worktree: Path) -> list[str]:
    completed = subprocess.run(
        [
            "git",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ],
        cwd=worktree,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            "git status failed: " + completed.stderr.decode(errors="replace").strip()
        )
    records = [record for record in completed.stdout.split(b"\0") if record]
    paths: list[str] = []
    index = 0
    while index < len(records):
        record = records[index]
        if len(record) < 4 or record[2:3] != b" ":
            raise ValueError("campaign git status output is malformed")
        status = record[:2]
        paths.append(record[3:].decode(errors="strict"))
        index += 2 if b"R" in status or b"C" in status else 1
    return sorted(paths)


def _staged_changed_files(worktree: Path) -> list[str]:
    return sorted(
        path
        for path in _git(
            worktree,
            "diff",
            "--cached",
            "--name-only",
            "--no-renames",
        ).splitlines()
        if path
    )


def _write_bytes_secure(path: Path, content: bytes) -> None:
    descriptor = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    try:
        if os.write(descriptor, content) != len(content):
            raise OSError("short write while storing pending campaign artifact")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


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
    case_fingerprints: dict[str, str],
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
        "development_dataset_fingerprint": catalog_summary["development_fingerprint"],
        "holdout_dataset_fingerprint": catalog_summary["holdout_fingerprint"],
        "case_fingerprints": dict(sorted(case_fingerprints.items())),
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
