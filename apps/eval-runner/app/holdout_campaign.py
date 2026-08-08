"""Sealed, recoverable ForgeBench holdout campaign execution.

The development baseline runner is deliberately constrained to ten visible cases.  This
module is the separate H4 boundary: it unlocks only the sealed holdout split, requires a
complete development comparison as a release precondition, and persists every paid call
before allowing the next one to start.
"""

from __future__ import annotations

import json
from collections.abc import Sequence
from contextlib import suppress
from datetime import UTC, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any, Literal

from pydantic import Field, model_validator

from app.baseline import (
    _REPORT_SEMANTIC_BLOCKERS,
    BaselineCrashInjected,
    CampaignStateError,
    _append_budget_entry,
    _atomic_write_bytes,
    _atomic_write_model,
    _attempt_artifact,
    _attempt_blockers,
    _attempt_id,
    _budget_entry_for_artifact,
    _campaign_lock,
    _create_empty_file,
    _dirty_state_sha256,
    _fsync_directory,
    _git_text,
    _interrupted_attempt_artifact,
    _load_attempt_artifacts,
    _load_budget_entries,
    _load_pending_attempts,
    _read_canonical_model,
    _remove_pending,
    _trust_for_trace,
)
from app.baseline_models import (
    BaselineArtifactIndex,
    BaselineArtifactIndexEntry,
    BaselineAttemptArtifact,
    BaselineBudgetEntry,
    BaselineComparison,
    BaselinePendingAttempt,
)
from app.cases import load_cases, validate_case_quality
from app.datasets import dataset_fingerprint
from app.harness_checks import run_observer_patch_replay_self_check
from app.harness_contracts import resolve_typed_failure_category
from app.harness_profiles import (
    HarnessProfileV1,
    harness_profile_bytes,
    harness_profile_fingerprint,
    load_harness_profile,
)
from app.harness_telemetry import summarize_harness_telemetry
from app.holdout_gate import (
    HoldoutAttemptEvidenceV1,
    HoldoutGateReportV1,
    holdout_report_bytes,
    load_holdout_report,
    summarize_holdout_attempts,
)
from app.metrics import trace_passed
from app.models import DatasetSplit, EvalModel, EvaluationTask
from app.provenance import canonical_json_bytes, sha256_bytes
from app.provenance_models import SAFE_ID_PATTERN, SECRET_PATTERN, GitSha1, Sha256
from app.provider_usage import account_provider_usage, provider_events_from_trace
from app.reporting import build_report

_CAMPAIGN_FILE = "campaign.json"
_BUDGET_FILE = "budget.jsonl"
_LOCK_FILE = ".lock"
_PENDING_DIR = "pending"
_ATTEMPTS_DIR = "attempts"
_LOCKS_DIR = "locks"
_PROFILE_LOCK = "profile.json"
_DATASET_LOCK = "dataset-lock.json"
_COMPARISON_LOCK = "development-comparison.json"
_ARTIFACT_INDEX_FILE = "holdout-artifact-index.json"
_REPORT_FILE = "holdout-report.json"
_REPORT_SHA_FILE = "holdout-report.json.sha256"
_INTERRUPTED_BLOCKER = "interrupted_paid_attempt_unknown_cost"
_HOLDOUT_IDS = tuple(
    (
        "forgebench-holdout-admin-pagination-query",
        "forgebench-holdout-admin-permission-guard",
        "forgebench-holdout-ambiguous-notification-badge",
        "forgebench-holdout-api-conditional-update",
        "forgebench-holdout-api-validation",
        "forgebench-holdout-config-feature-flag",
        "forgebench-holdout-config-port-range",
        "forgebench-holdout-contract-task-forward-compat",
        "forgebench-holdout-domain-due-date-zone",
        "forgebench-holdout-domain-stable-sort",
        "forgebench-holdout-integration-bulk-edit",
        "forgebench-holdout-long-context-audit-log",
        "forgebench-holdout-release-checksums",
        "forgebench-holdout-release-version-match",
        "forgebench-holdout-ui-empty-state-escape",
        "forgebench-holdout-ui-keyboard-shortcut",
        "forgebench-holdout-web-optimistic-rollback",
        "forgebench-holdout-web-saved-filters",
        "forgebench-holdout-worker-dedupe",
        "forgebench-holdout-worker-quarantine",
    )
)
_HOLDOUT_PROFILE_IDS = frozenset(
    {
        "deepseek-v4-flash-anthropic-baseline-v1",
        "deepseek-v4-flash-openai-chat-control-v1",
        "deepseek-v4-flash-anthropic-minimal-v1",
        "deepseek-v4-flash-openai-chat-minimal-v1",
    }
)


class HoldoutCampaignMetadata(EvalModel):
    schema_version: Literal[1] = 1
    campaign_id: str
    source_commit: GitSha1
    dirty_state_sha256: Sha256
    dataset_fingerprint: Sha256
    dataset_lock_sha256: Sha256
    harness_fingerprint: Sha256
    profile: HarnessProfileV1
    case_ids: list[str]
    repetitions: Literal[3] = 3
    max_attempts: Literal[60] = 60
    budget_cny: Decimal = Field(gt=0, allow_inf_nan=False)
    development_comparison_sha256: Sha256
    development_comparison_source_commit: GitSha1

    @model_validator(mode="after")
    def validate_metadata(self) -> HoldoutCampaignMetadata:
        if SAFE_ID_PATTERN.fullmatch(self.campaign_id) is None:
            raise ValueError("holdout campaign id must be a safe identifier")
        if tuple(self.case_ids) != _HOLDOUT_IDS:
            raise ValueError("holdout campaign must lock the canonical 20 case ids")
        if harness_profile_fingerprint(self.profile) != self.harness_fingerprint:
            raise ValueError("holdout profile fingerprint does not match")
        if self.profile.dataset_split != DatasetSplit.DEVELOPMENT:
            raise ValueError("holdout route profile must retain development profile policy")
        if self.profile.profile_id not in _HOLDOUT_PROFILE_IDS:
            raise ValueError("holdout campaign cannot use a planning-only profile")
        if self.profile.max_cost_cny_per_task > self.budget_cny:
            raise ValueError("holdout reservation cap exceeds campaign budget")
        if self.budget_cny != Decimal("35"):
            raise ValueError("holdout budget is locked at 35 CNY")
        if SECRET_PATTERN.search(json.dumps(self.model_dump(mode="json"), sort_keys=True)):
            raise ValueError("holdout campaign metadata contains secret material")
        return self


class HoldoutVerification(EvalModel):
    ok: bool
    attempt_count: int = Field(ge=0)
    pending_count: int = Field(ge=0)
    spent_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    blockers: list[str] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_result(self) -> HoldoutVerification:
        if self.ok == bool(self.blockers):
            raise ValueError("holdout verification ok must be inverse of blockers")
        return self


class HoldoutRunResult(EvalModel):
    status: Literal["complete", "paused", "blocked"]
    calls_started: int = Field(ge=0)
    terminal_attempts: int = Field(ge=0)
    spent_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    blockers: list[str] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_result(self) -> HoldoutRunResult:
        if self.status == "blocked" and not self.blockers:
            raise ValueError("blocked holdout result requires blockers")
        if self.status != "blocked" and self.blockers:
            raise ValueError("non-blocked holdout result cannot contain blockers")
        return self


def initialize_holdout_campaign(
    campaign_root: Path,
    *,
    campaign_id: str,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
    development_comparison_path: Path,
    budget_cny: Decimal = Decimal("35"),
) -> HoldoutCampaignMetadata:
    """Create a sealed campaign after verifying the development A/B release gate."""

    if campaign_root.exists():
        raise ValueError("holdout campaign path already exists")
    if budget_cny != Decimal("35"):
        raise ValueError("holdout budget is locked at 35 CNY")
    profile = load_harness_profile(profile_path)
    tasks, dataset_lock = _load_locked_holdout_tasks(cases_path, dataset_lock_path)
    repository_root = repository_root.resolve()
    source_commit = _git_text(repository_root, "rev-parse", "HEAD")
    comparison, comparison_digest = _load_development_comparison(
        development_comparison_path,
        source_commit=source_commit,
        profile=profile,
    )
    metadata = HoldoutCampaignMetadata(
        campaign_id=campaign_id,
        source_commit=source_commit,
        dirty_state_sha256=_dirty_state_sha256(repository_root, campaign_root),
        dataset_fingerprint=dataset_fingerprint(tasks),
        dataset_lock_sha256=sha256_bytes(dataset_lock_path.read_bytes()),
        harness_fingerprint=harness_profile_fingerprint(profile),
        profile=profile,
        case_ids=[task.id for task in tasks],
        budget_cny=budget_cny,
        development_comparison_sha256=comparison_digest,
        development_comparison_source_commit=comparison.source_commit or source_commit,
    )
    temporary_root = campaign_root.parent / f".{campaign_root.name}.holdout-tmp"
    if temporary_root.exists():
        raise ValueError("stale holdout campaign temporary path exists")
    temporary_root.mkdir(parents=True)
    try:
        for directory in (_PENDING_DIR, _ATTEMPTS_DIR, _LOCKS_DIR):
            (temporary_root / directory).mkdir()
        _atomic_write_model(temporary_root / _CAMPAIGN_FILE, metadata)
        _atomic_write_bytes(
            temporary_root / _LOCKS_DIR / _PROFILE_LOCK,
            harness_profile_bytes(profile) + b"\n",
        )
        _atomic_write_bytes(
            temporary_root / _LOCKS_DIR / _DATASET_LOCK,
            canonical_json_bytes(dataset_lock) + b"\n",
        )
        _atomic_write_bytes(
            temporary_root / _LOCKS_DIR / _COMPARISON_LOCK,
            development_comparison_path.read_bytes(),
        )
        _create_empty_file(temporary_root / _BUDGET_FILE)
        _create_empty_file(temporary_root / _LOCK_FILE)
        os_replace(temporary_root, campaign_root)
        _fsync_directory(campaign_root.parent)
    except BaseException:
        import shutil

        shutil.rmtree(temporary_root, ignore_errors=True)
        raise
    return metadata


def run_holdout_campaign(
    campaign_root: Path,
    *,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
    runner: Any,
    max_attempts: int | None = None,
    crash_at: str | None = None,
) -> HoldoutRunResult:
    if max_attempts is not None and max_attempts < 1:
        raise ValueError("max_attempts must be positive")
    if crash_at not in (None, "after_artifact_publish", "after_ledger_append"):
        raise ValueError("unknown holdout crash point")
    with _campaign_lock(campaign_root):
        pending = list((campaign_root / _PENDING_DIR).glob("*.json"))
        if pending:
            return _blocked_result(campaign_root, [_INTERRUPTED_BLOCKER], 0)
        verification = verify_holdout_campaign(
            campaign_root,
            repository_root=repository_root,
            cases_path=cases_path,
            dataset_lock_path=dataset_lock_path,
            profile_path=profile_path,
        )
        if not verification.ok:
            # An unverified but empty campaign is allowed to proceed only when it is
            # incomplete; semantic blockers (for example an old unknown-cost attempt)
            # are terminal and must never be replayed.
            allowed = {"holdout_attempt_count_incomplete", "holdout_report_missing"}
            if set(verification.blockers) - allowed:
                return _blocked_result(campaign_root, verification.blockers, 0)
        metadata = _read_canonical_model(
            campaign_root / _CAMPAIGN_FILE, HoldoutCampaignMetadata, "campaign_metadata_invalid"
        )
        profile = load_harness_profile(profile_path)
        tasks, _ = _load_locked_holdout_tasks(cases_path, dataset_lock_path)
        tasks_by_id = {task.id: task for task in tasks}
        entries = _load_budget_entries(campaign_root / _BUDGET_FILE)
        completed = {entry.attempt_id for entry in entries}
        calls_started = 0
        harness_check = run_observer_patch_replay_self_check()
        for repetition in range(1, metadata.repetitions + 1):
            for task_id in metadata.case_ids:
                task = tasks_by_id[task_id]
                attempt_id = _attempt_id(metadata.campaign_id, repetition, task_id)
                if attempt_id in completed:
                    continue
                if max_attempts is not None and calls_started >= max_attempts:
                    return _run_result(campaign_root, "paused", calls_started)
                spent = entries[-1].cumulative_cost_cny if entries else Decimal(0)
                if spent + profile.max_cost_cny_per_task > metadata.budget_cny:
                    return _blocked_result(
                        campaign_root, ["holdout_budget_reservation_exceeded"], calls_started
                    )
                _remove_holdout_report_outputs(campaign_root)
                pending_record = BaselinePendingAttempt(
                    attempt_id=attempt_id,
                    task_id=task.id,
                    repetition=repetition,
                    source_commit=metadata.source_commit,
                    dirty_state_sha256=metadata.dirty_state_sha256,
                    dataset_fingerprint=metadata.dataset_fingerprint,
                    harness_fingerprint=metadata.harness_fingerprint,
                    reserved_cost_cny=profile.max_cost_cny_per_task,
                    started_at=datetime.now(UTC),
                    interruption_stage="runner",
                )
                pending_path = campaign_root / _PENDING_DIR / f"{attempt_id}.json"
                _atomic_write_model(pending_path, pending_record)
                calls_started += 1
                interruption_stage = "runner"
                try:
                    trace = runner.run_task(task)
                    interruption_stage = "usage_accounting"
                    usage = account_provider_usage(provider_events_from_trace(trace), profile)
                    interruption_stage = "trust_evaluation"
                    trust = _trust_for_trace(
                        task,
                        trace,
                        dataset_fingerprint_value=metadata.dataset_fingerprint,
                        harness_check=harness_check,
                    )
                    interruption_stage = "artifact_publish"
                    artifact = _attempt_artifact(pending_record, profile, trace, usage, trust)
                    artifact_path = campaign_root / _ATTEMPTS_DIR / f"{attempt_id}.json"
                    if artifact_path.exists():
                        raise CampaignStateError("duplicate_attempt_artifact")
                    _atomic_write_model(artifact_path, artifact)
                except BaselineCrashInjected:
                    raise
                except Exception as exc:
                    with suppress(OSError, ValueError):
                        _atomic_write_model(
                            pending_path,
                            pending_record.model_copy(
                                update={
                                    "interruption_stage": interruption_stage,
                                    "interruption_reason": type(exc).__name__,
                                }
                            ),
                        )
                    return _blocked_result(campaign_root, [_INTERRUPTED_BLOCKER], calls_started)
                if crash_at == "after_artifact_publish":
                    raise BaselineCrashInjected("after_artifact_publish")
                entry = _budget_entry_for_artifact(entries, artifact)
                _append_budget_entry(campaign_root / _BUDGET_FILE, entry)
                entries.append(entry)
                completed.add(attempt_id)
                if crash_at == "after_ledger_append":
                    raise BaselineCrashInjected("after_ledger_append")
                _remove_pending(pending_path)
                blockers = _attempt_blockers(artifact)
                if blockers:
                    return _blocked_result(campaign_root, blockers, calls_started)
        return _run_result(campaign_root, "complete", calls_started)


def recover_holdout_campaign(
    campaign_root: Path,
    *,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
) -> HoldoutRunResult:
    with _campaign_lock(campaign_root):
        try:
            metadata = _verify_campaign_inputs(
                campaign_root,
                repository_root=repository_root,
                cases_path=cases_path,
                dataset_lock_path=dataset_lock_path,
                profile_path=profile_path,
            )
            profile = load_harness_profile(profile_path)
            entries = _load_budget_entries(campaign_root / _BUDGET_FILE)
            for pending_path in sorted((campaign_root / _PENDING_DIR).glob("*.json")):
                pending = _read_canonical_model(
                    pending_path, BaselinePendingAttempt, "pending_record_invalid"
                )
                _validate_pending_identity(pending, metadata, profile)
                artifact_path = campaign_root / _ATTEMPTS_DIR / f"{pending.attempt_id}.json"
                if not artifact_path.exists():
                    _atomic_write_model(
                        artifact_path, _interrupted_attempt_artifact(pending, profile)
                    )
                artifact = _read_canonical_model(
                    artifact_path, BaselineAttemptArtifact, "attempt_artifact_invalid"
                )
                if pending.attempt_id not in {entry.attempt_id for entry in entries}:
                    entry = _budget_entry_for_artifact(entries, artifact)
                    _append_budget_entry(campaign_root / _BUDGET_FILE, entry)
                    entries.append(entry)
                _remove_pending(pending_path)
        except (CampaignStateError, OSError, ValueError):
            return _blocked_result(campaign_root, ["campaign_state_invalid"], 0)
        verification = verify_holdout_campaign(
            campaign_root,
            repository_root=repository_root,
            cases_path=cases_path,
            dataset_lock_path=dataset_lock_path,
            profile_path=profile_path,
        )
        if not verification.ok:
            return HoldoutRunResult(
                status="blocked",
                calls_started=0,
                terminal_attempts=verification.attempt_count,
                spent_cny=verification.spent_cny,
                blockers=verification.blockers,
            )
        metadata = _read_canonical_model(
            campaign_root / _CAMPAIGN_FILE, HoldoutCampaignMetadata, "campaign_metadata_invalid"
        )
        return HoldoutRunResult(
            status="complete" if verification.attempt_count == metadata.max_attempts else "paused",
            calls_started=0,
            terminal_attempts=verification.attempt_count,
            spent_cny=verification.spent_cny,
        )


def verify_holdout_campaign(
    campaign_root: Path,
    *,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
    require_current_source: bool = True,
) -> HoldoutVerification:
    blockers: list[str] = []
    attempts = 0
    pending_count = 0
    spent = Decimal(0)
    try:
        metadata = _verify_campaign_inputs(
            campaign_root,
            repository_root=repository_root,
            cases_path=cases_path,
            dataset_lock_path=dataset_lock_path,
            profile_path=profile_path,
            require_current_source=require_current_source,
        )
        profile = load_harness_profile(profile_path)
        tasks, _ = _load_locked_holdout_tasks(cases_path, dataset_lock_path)
        artifacts = _load_attempt_artifacts(campaign_root / _ATTEMPTS_DIR)
        entries = _load_budget_entries(campaign_root / _BUDGET_FILE)
        pending = _load_pending_attempts(campaign_root / _PENDING_DIR)
        attempts = len(artifacts)
        pending_count = len(pending)
        spent = entries[-1].cumulative_cost_cny if entries else Decimal(0)
        if pending:
            blockers.append(_INTERRUPTED_BLOCKER)
        if len(entries) != len(artifacts) or set(entry.attempt_id for entry in entries) != set(
            artifacts
        ):
            blockers.append("artifact_budget_alignment_mismatch")
        tasks_by_id = {task.id: task for task in tasks}
        harness_check = run_observer_patch_replay_self_check()
        for attempt_id, artifact in artifacts.items():
            expected_id = _attempt_id(metadata.campaign_id, artifact.repetition, artifact.task_id)
            task = tasks_by_id.get(artifact.task_id)
            if attempt_id != expected_id or task is None:
                blockers.append("attempt_identity_invalid")
                continue
            if (
                artifact.profile != profile
                or artifact.source_commit != metadata.source_commit
                or artifact.dirty_state_sha256 != metadata.dirty_state_sha256
                or artifact.dataset_fingerprint != metadata.dataset_fingerprint
                or artifact.harness_fingerprint != metadata.harness_fingerprint
            ):
                blockers.append("attempt_source_identity_mismatch")
            if artifact.primary_failure_category != _INTERRUPTED_BLOCKER:
                expected_usage = account_provider_usage(
                    provider_events_from_trace(artifact.trace), profile
                )
                expected_trust = _trust_for_trace(
                    task,
                    artifact.trace,
                    dataset_fingerprint_value=metadata.dataset_fingerprint,
                    harness_check=harness_check,
                )
                if expected_usage != artifact.usage:
                    blockers.append("provider_usage_accounting_mismatch")
                if expected_trust != artifact.trust_result:
                    blockers.append("trust_result_mismatch")
            blockers.extend(_attempt_blockers(artifact))
        previous: BaselineBudgetEntry | None = None
        cumulative = Decimal(0)
        for sequence, entry in enumerate(entries, 1):
            if entry.sequence != sequence:
                blockers.append("budget_sequence_mismatch")
            expected_previous = _entry_digest(previous) if previous else None
            if entry.previous_entry_sha256 != expected_previous:
                blockers.append("budget_hash_chain_mismatch")
            artifact_for_entry = artifacts.get(entry.attempt_id)
            if artifact_for_entry is not None and entry.artifact_sha256 != _artifact_digest(
                artifact_for_entry
            ):
                blockers.append("budget_artifact_digest_mismatch")
            cumulative += entry.cost_cny
            if entry.cumulative_cost_cny != cumulative:
                blockers.append("budget_cumulative_cost_mismatch")
            previous = entry
        if spent > metadata.budget_cny:
            blockers.append("holdout_budget_exceeded")
        if attempts != metadata.max_attempts:
            blockers.append("holdout_attempt_count_incomplete")
        report_path = campaign_root / _REPORT_FILE
        if report_path.exists():
            report = load_holdout_report(report_path)
            if (
                report.campaign_id != metadata.campaign_id
                or report.dataset_fingerprint != metadata.dataset_fingerprint
                or report.harness_fingerprint != metadata.harness_fingerprint
            ):
                blockers.append("holdout_report_identity_mismatch")
            sidecar = campaign_root / _REPORT_SHA_FILE
            if not sidecar.exists() or sidecar.read_text(encoding="utf-8").strip() != sha256_bytes(
                report_path.read_bytes()
            ):
                blockers.append("holdout_report_digest_mismatch")
            if attempts == metadata.max_attempts and not pending:
                expected_report = _build_holdout_report(metadata, profile, artifacts)
                if report != expected_report:
                    blockers.append("holdout_report_mismatch")
        elif attempts == metadata.max_attempts and not pending:
            blockers.append("holdout_report_missing")
        if attempts == metadata.max_attempts and not pending:
            index_path = campaign_root / _ARTIFACT_INDEX_FILE
            if not index_path.exists():
                blockers.append("holdout_artifact_index_missing")
            else:
                actual_index = _read_canonical_model(
                    index_path, BaselineArtifactIndex, "holdout_artifact_index_invalid"
                )
                if actual_index != _build_artifact_index(campaign_root, metadata, artifacts):
                    blockers.append("holdout_artifact_index_mismatch")
    except (CampaignStateError, OSError, ValueError, json.JSONDecodeError):
        blockers.append("campaign_state_invalid")
    return HoldoutVerification(
        ok=not blockers,
        attempt_count=attempts,
        pending_count=pending_count,
        spent_cny=spent,
        blockers=sorted(set(blockers)),
    )


def generate_holdout_report(
    campaign_root: Path,
    *,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
) -> HoldoutGateReportV1:
    verification = verify_holdout_campaign(
        campaign_root,
        repository_root=repository_root,
        cases_path=cases_path,
        dataset_lock_path=dataset_lock_path,
        profile_path=profile_path,
    )
    structural = (
        set(verification.blockers)
        - {
            "holdout_attempt_count_incomplete",
            "holdout_report_missing",
            "holdout_artifact_index_missing",
        }
        - _REPORT_SEMANTIC_BLOCKERS
    )
    if verification.pending_count or structural or verification.attempt_count != 60:
        raise ValueError("holdout report requires 60 closed, structurally valid attempts")
    metadata = _read_canonical_model(
        campaign_root / _CAMPAIGN_FILE, HoldoutCampaignMetadata, "campaign_metadata_invalid"
    )
    profile = load_harness_profile(profile_path)
    artifacts = _load_attempt_artifacts(campaign_root / _ATTEMPTS_DIR)
    report = _build_holdout_report(metadata, profile, artifacts)
    index = _build_artifact_index(campaign_root, metadata, artifacts)
    _atomic_write_bytes(
        campaign_root / _ARTIFACT_INDEX_FILE,
        canonical_json_bytes(index.model_dump(mode="json")) + b"\n",
    )
    _atomic_write_bytes(campaign_root / _REPORT_FILE, holdout_report_bytes(report))
    _atomic_write_bytes(
        campaign_root / _REPORT_SHA_FILE,
        (sha256_bytes(holdout_report_bytes(report)) + "\n").encode(),
    )
    return report


def _load_locked_holdout_tasks(
    cases_path: Path, dataset_lock_path: Path
) -> tuple[list[EvaluationTask], dict[str, Any]]:
    tasks = sorted(load_cases(cases_path), key=lambda task: task.id)
    lock = json.loads(dataset_lock_path.read_text(encoding="utf-8"))
    holdout = lock.get("holdout")
    if (
        len(tasks) != 20
        or any(task.dataset_split != DatasetSplit.HOLDOUT for task in tasks)
        or [task.id for task in tasks] != list(_HOLDOUT_IDS)
    ):
        raise ValueError("holdout accepts the exact sealed 20-case split only")
    if (
        not isinstance(holdout, dict)
        or holdout.get("ids") != list(_HOLDOUT_IDS)
        or holdout.get("count") != 20
        or holdout.get("fingerprint") != dataset_fingerprint(tasks)
    ):
        raise ValueError("holdout dataset lock does not match sealed cases")
    issues = validate_case_quality(tasks)
    if any(issue.severity == "error" for issue in issues):
        raise ValueError("holdout case quality is not release-safe")
    if any(task.metadata.get("holdout_unsealed") is not False for task in tasks):
        raise ValueError("holdout cases must remain sealed")
    return tasks, lock


def _load_development_comparison(
    path: Path, *, source_commit: str, profile: HarnessProfileV1
) -> tuple[BaselineComparison, str]:
    raw = path.read_bytes()
    comparison = BaselineComparison.model_validate(json.loads(raw))
    if raw != canonical_json_bytes(comparison.model_dump(mode="json")) + b"\n":
        raise ValueError("development comparison is not canonical")
    sidecar = path.with_suffix(path.suffix + ".sha256")
    if not sidecar.exists() or sidecar.read_text(encoding="utf-8").strip() != sha256_bytes(raw):
        raise ValueError("development comparison digest sidecar mismatch")
    if (
        comparison.blockers
        or comparison.paired_attempts != 30
        or comparison.source_commit != source_commit
    ):
        raise ValueError("development comparison is not a complete current-source pair")
    if profile.profile_id not in {comparison.control_profile_id, comparison.candidate_profile_id}:
        raise ValueError("holdout profile is not one of the compared development arms")
    return comparison, sha256_bytes(raw)


def _verify_campaign_inputs(
    campaign_root: Path,
    *,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
    require_current_source: bool = True,
) -> HoldoutCampaignMetadata:
    metadata = _read_canonical_model(
        campaign_root / _CAMPAIGN_FILE, HoldoutCampaignMetadata, "campaign_metadata_invalid"
    )
    profile = load_harness_profile(profile_path)
    tasks, dataset_lock = _load_locked_holdout_tasks(cases_path, dataset_lock_path)
    if (campaign_root / _LOCKS_DIR / _PROFILE_LOCK).read_bytes() != harness_profile_bytes(
        profile
    ) + b"\n":
        raise CampaignStateError("profile_lock_mismatch")
    if (campaign_root / _LOCKS_DIR / _DATASET_LOCK).read_bytes() != canonical_json_bytes(
        dataset_lock
    ) + b"\n":
        raise CampaignStateError("dataset_lock_mismatch")
    comparison_lock = campaign_root / _LOCKS_DIR / _COMPARISON_LOCK
    comparison_raw = comparison_lock.read_bytes()
    try:
        comparison = BaselineComparison.model_validate(json.loads(comparison_raw))
    except (json.JSONDecodeError, ValueError) as exc:
        raise CampaignStateError("comparison_lock_invalid") from exc
    if comparison_raw != canonical_json_bytes(comparison.model_dump(mode="json")) + b"\n":
        raise CampaignStateError("comparison_lock_noncanonical")
    if sha256_bytes(comparison_raw) != metadata.development_comparison_sha256:
        raise CampaignStateError("comparison_lock_digest_mismatch")
    if sha256_bytes(dataset_lock_path.read_bytes()) != metadata.dataset_lock_sha256:
        raise CampaignStateError("dataset_lock_digest_mismatch")
    if (
        profile != metadata.profile
        or harness_profile_fingerprint(profile) != metadata.harness_fingerprint
    ):
        raise CampaignStateError("campaign_profile_mismatch")
    if (
        dataset_fingerprint(tasks) != metadata.dataset_fingerprint
        or [task.id for task in tasks] != metadata.case_ids
    ):
        raise CampaignStateError("holdout_dataset_fingerprint_mismatch")
    if require_current_source:
        if _git_text(repository_root.resolve(), "rev-parse", "HEAD") != metadata.source_commit:
            raise CampaignStateError("source_commit_mismatch")
        if (
            _dirty_state_sha256(repository_root.resolve(), campaign_root)
            != metadata.dirty_state_sha256
        ):
            raise CampaignStateError("dirty_state_digest_mismatch")
    return metadata


def _validate_pending_identity(
    pending: BaselinePendingAttempt, metadata: HoldoutCampaignMetadata, profile: HarnessProfileV1
) -> None:
    if (
        pending.source_commit != metadata.source_commit
        or pending.dirty_state_sha256 != metadata.dirty_state_sha256
        or pending.dataset_fingerprint != metadata.dataset_fingerprint
        or pending.harness_fingerprint != metadata.harness_fingerprint
        or pending.reserved_cost_cny != profile.max_cost_cny_per_task
        or pending.task_id not in metadata.case_ids
        or pending.attempt_id
        != _attempt_id(metadata.campaign_id, pending.repetition, pending.task_id)
    ):
        raise CampaignStateError("pending_attempt_identity_mismatch")


def _ordered_attempt_ids(metadata: HoldoutCampaignMetadata) -> list[str]:
    return [
        _attempt_id(metadata.campaign_id, repetition, task_id)
        for repetition in range(1, 4)
        for task_id in metadata.case_ids
    ]


def _evidence_for_artifact(
    artifact: BaselineAttemptArtifact, profile: HarnessProfileV1
) -> HoldoutAttemptEvidenceV1:
    score_coverage = build_report([artifact.trace]).score_coverage
    telemetry = summarize_harness_telemetry([artifact.trace])
    usage_known = artifact.usage.status == "known" and artifact.usage.cost_cny is not None
    blockers = _attempt_blockers(artifact)
    return HoldoutAttemptEvidenceV1(
        attempt_id=artifact.attempt_id,
        case_id=artifact.task_id,
        repetition=artifact.repetition,
        trusted=not blockers,
        passed=trace_passed(artifact.trace),
        scope_violation=bool(artifact.trace.scope_violations),
        model_rounds=artifact.trace.model_rounds,
        score_coverage=bool(score_coverage)
        and all(item.coverage >= 1.0 for item in score_coverage.values()),
        contract_evidence=telemetry.contract_coverage == 1.0,
        workspace_evidence=artifact.trace.workspace_observation is not None
        and artifact.trace.workspace_observation.available,
        patch_replay=artifact.trace.patch_replay is not None and artifact.trace.patch_replay.ok,
        verification_evidence=artifact.trace.verification_result is not None,
        usage_known=usage_known,
        terminal_failure_category=resolve_typed_failure_category(
            artifact.trace.raw_events, artifact.primary_failure_category
        ),
        cost_cny=artifact.usage.cost_cny
        if usage_known and artifact.usage.cost_cny is not None
        else profile.max_cost_cny_per_task,
    )

def _build_holdout_report(
    metadata: HoldoutCampaignMetadata,
    profile: HarnessProfileV1,
    artifacts: dict[str, BaselineAttemptArtifact],
) -> HoldoutGateReportV1:
    evidence = [
        _evidence_for_artifact(artifacts[attempt_id], profile)
        for attempt_id in _ordered_attempt_ids(metadata)
    ]
    return summarize_holdout_attempts(
        evidence,
        campaign_id=metadata.campaign_id,
        case_ids=metadata.case_ids,
        dataset_fingerprint=metadata.dataset_fingerprint,
        harness_fingerprint=metadata.harness_fingerprint,
        profile_id=profile.profile_id,
        source_commit=metadata.source_commit,
        price_table_fingerprint=sha256_bytes(
            canonical_json_bytes(profile.price_table.model_dump(mode="json"))
        ),
        transport=profile.transport,
        budget_cny=metadata.budget_cny,
    )


def _build_artifact_index(
    campaign_root: Path,
    metadata: HoldoutCampaignMetadata,
    artifacts: dict[str, BaselineAttemptArtifact],
) -> BaselineArtifactIndex:
    ordered = sorted(artifacts.values(), key=lambda item: (item.repetition, item.task_id))
    return BaselineArtifactIndex(
        campaign_id=metadata.campaign_id,
        artifacts=[
            BaselineArtifactIndexEntry(
                path=f"{_ATTEMPTS_DIR}/{artifact.attempt_id}.json",
                sha256=sha256_bytes(
                    (campaign_root / _ATTEMPTS_DIR / f"{artifact.attempt_id}.json").read_bytes()
                ),
            )
            for artifact in ordered
        ],
    )


def _remove_holdout_report_outputs(campaign_root: Path) -> None:
    removed = False
    for filename in (_REPORT_FILE, _REPORT_SHA_FILE, _ARTIFACT_INDEX_FILE):
        path = campaign_root / filename
        if path.exists():
            path.unlink()
            removed = True
    if removed:
        _fsync_directory(campaign_root)


def _verify_report_outputs(campaign_root: Path, metadata: HoldoutCampaignMetadata) -> list[str]:
    return []


def _entry_digest(entry: BaselineBudgetEntry) -> str:
    return sha256_bytes(canonical_json_bytes(entry.model_dump(mode="json")))


def _artifact_digest(artifact: BaselineAttemptArtifact) -> str:
    return sha256_bytes(canonical_json_bytes(artifact.model_dump(mode="json")))


def _run_result(
    campaign_root: Path, status: Literal["complete", "paused"], calls_started: int
) -> HoldoutRunResult:
    entries = _load_budget_entries(campaign_root / _BUDGET_FILE)
    return HoldoutRunResult(
        status=status,
        calls_started=calls_started,
        terminal_attempts=len(entries),
        spent_cny=entries[-1].cumulative_cost_cny if entries else Decimal(0),
    )


def _blocked_result(
    campaign_root: Path, blockers: Sequence[str], calls_started: int
) -> HoldoutRunResult:
    with suppress(CampaignStateError, OSError):
        entries = _load_budget_entries(campaign_root / _BUDGET_FILE)
        return HoldoutRunResult(
            status="blocked",
            calls_started=calls_started,
            terminal_attempts=len(entries),
            spent_cny=entries[-1].cumulative_cost_cny if entries else Decimal(0),
            blockers=sorted(set(blockers)),
        )
    return HoldoutRunResult(
        status="blocked",
        calls_started=calls_started,
        terminal_attempts=0,
        spent_cny=Decimal(0),
        blockers=sorted(set(blockers)),
    )


def os_replace(source: Path, target: Path) -> None:
    import os

    os.replace(source, target)
