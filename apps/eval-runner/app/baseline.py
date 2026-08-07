import fcntl
import json
import os
import shutil
import stat
import subprocess
import tempfile
from collections import Counter, defaultdict
from collections.abc import Iterator
from contextlib import contextmanager, suppress
from datetime import UTC, datetime
from decimal import Decimal
from math import ceil
from pathlib import Path
from statistics import median
from typing import Any, TypeVar

from pydantic import ValidationError

from app.baseline_models import (
    BaselineArtifactIndex,
    BaselineArtifactIndexEntry,
    BaselineAttemptArtifact,
    BaselineBudgetEntry,
    BaselineCampaignMetadata,
    BaselinePendingAttempt,
    BaselineReport,
    BaselineRunResult,
    BaselineVerification,
    baseline_artifact_digest,
    baseline_entry_digest,
)
from app.cases import load_cases, validate_case_quality
from app.datasets import dataset_fingerprint
from app.harness_checks import run_observer_patch_replay_self_check
from app.harness_profiles import (
    HarnessProfileV1,
    harness_profile_bytes,
    harness_profile_fingerprint,
    load_harness_profile,
)
from app.harness_telemetry import summarize_harness_telemetry
from app.judge_calibration import score_can_gate_ci
from app.metrics import trace_passed
from app.models import (
    AgentTrace,
    DatasetSplit,
    EvalModel,
    EvalProvider,
    EvaluationTask,
    FailureCategory,
    TrustGateResult,
    TrustStatus,
)
from app.provenance import canonical_json_bytes, sha256_bytes
from app.provider_usage import (
    ProviderUsageAccounting,
    account_provider_usage,
    provider_events_from_trace,
)
from app.reporting import build_report
from app.runner import EvalRunner
from app.scoring import score_trace
from app.trust_gates import evaluate_trust_gates

_CRASH_POINTS = {"after_artifact_publish", "after_ledger_append"}
_CAMPAIGN_FILE = "campaign.json"
_BUDGET_FILE = "budget.jsonl"
_LOCK_FILE = ".lock"
_PENDING_DIR = "pending"
_ATTEMPTS_DIR = "attempts"
_LOCKS_DIR = "locks"
_PROFILE_LOCK = "profile.json"
_DATASET_LOCK = "dataset-lock.json"
_ARTIFACT_INDEX_FILE = "artifact-index.json"
_REPORT_FILE = "baseline-report.json"
_INTERRUPTED_BLOCKER = "interrupted_paid_attempt_unknown_cost"
_REPORT_SEMANTIC_BLOCKERS = {
    _INTERRUPTED_BLOCKER,
    "agent_identity_mismatch",
    "campaign_budget_exceeded",
    "forge_contract_error",
    "per_task_cost_cap_exceeded",
    "provider_usage_unknown",
    "trust_result_untrusted",
}
_ModelT = TypeVar("_ModelT", bound=EvalModel)


class BaselineCrashInjected(RuntimeError):
    """Deterministic crash point used to verify paid-attempt recovery."""


class CampaignStateError(ValueError):
    def __init__(self, blocker: str) -> None:
        super().__init__(blocker)
        self.blocker = blocker


def initialize_baseline_campaign(
    campaign_root: Path,
    *,
    campaign_id: str,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
) -> BaselineCampaignMetadata:
    if campaign_root.exists():
        raise ValueError("baseline campaign path already exists")

    profile = load_harness_profile(profile_path)
    tasks, dataset_lock = _load_locked_development_tasks(
        cases_path,
        dataset_lock_path,
        profile,
    )
    repository_root = repository_root.resolve()
    source_commit = _git_text(repository_root, "rev-parse", "HEAD")
    dirty_state_sha256 = _dirty_state_sha256(repository_root, campaign_root)
    metadata = BaselineCampaignMetadata(
        campaign_id=campaign_id,
        source_commit=source_commit,
        dirty_state_sha256=dirty_state_sha256,
        dataset_fingerprint=dataset_fingerprint(tasks),
        dataset_lock_sha256=sha256_bytes(dataset_lock_path.read_bytes()),
        harness_fingerprint=harness_profile_fingerprint(profile),
        profile=profile,
        task_ids=[task.id for task in tasks],
        repetitions=profile.repetitions,
        max_attempts=profile.max_attempts,
        budget_cny=profile.campaign_budget_cny,
    )

    campaign_root.parent.mkdir(parents=True, exist_ok=True)
    temporary_root = Path(
        tempfile.mkdtemp(prefix=f".{campaign_root.name}.", dir=campaign_root.parent)
    )
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
        _create_empty_file(temporary_root / _BUDGET_FILE)
        _create_empty_file(temporary_root / _LOCK_FILE)
        os.replace(temporary_root, campaign_root)
        _fsync_directory(campaign_root.parent)
    except BaseException:
        shutil.rmtree(temporary_root, ignore_errors=True)
        raise
    return metadata


def run_baseline_campaign(
    campaign_root: Path,
    *,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
    runner: EvalRunner,
    max_attempts: int | None = None,
    crash_at: str | None = None,
) -> BaselineRunResult:
    if max_attempts is not None and max_attempts < 1:
        raise ValueError("max_attempts must be positive")
    if crash_at is not None and crash_at not in _CRASH_POINTS:
        raise ValueError("unknown baseline crash point")

    with _campaign_lock(campaign_root):
        pending_paths = sorted((campaign_root / _PENDING_DIR).glob("*.json"))
        if pending_paths:
            return _blocked_result(campaign_root, [_INTERRUPTED_BLOCKER], calls_started=0)

        verification = verify_baseline_campaign(
            campaign_root,
            repository_root=repository_root,
            cases_path=cases_path,
            dataset_lock_path=dataset_lock_path,
            profile_path=profile_path,
        )
        if not verification.ok:
            return BaselineRunResult(
                status="blocked",
                calls_started=0,
                terminal_attempts=verification.attempt_count,
                spent_cny=verification.spent_cny,
                blockers=verification.blockers,
            )

        metadata = _read_canonical_model(
            campaign_root / _CAMPAIGN_FILE,
            BaselineCampaignMetadata,
            "campaign_metadata_invalid",
        )
        profile = load_harness_profile(profile_path)
        tasks, _dataset_lock = _load_locked_development_tasks(
            cases_path,
            dataset_lock_path,
            profile,
        )
        tasks_by_id = {task.id: task for task in tasks}
        entries = _load_budget_entries(campaign_root / _BUDGET_FILE)
        completed = {entry.attempt_id for entry in entries}
        calls_started = 0
        harness_check = run_observer_patch_replay_self_check()

        for repetition in range(1, metadata.repetitions + 1):
            for task_id in metadata.task_ids:
                task = tasks_by_id[task_id]
                attempt_id = _attempt_id(metadata.campaign_id, repetition, task_id)
                if attempt_id in completed:
                    continue
                if max_attempts is not None and calls_started >= max_attempts:
                    return _run_result(campaign_root, "paused", calls_started)

                spent = entries[-1].cumulative_cost_cny if entries else Decimal(0)
                if spent + profile.max_cost_cny_per_task > profile.campaign_budget_cny:
                    return _blocked_result(
                        campaign_root,
                        ["campaign_budget_reservation_exceeded"],
                        calls_started=calls_started,
                    )

                _remove_report_outputs(campaign_root)
                pending = BaselinePendingAttempt(
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
                _atomic_write_model(pending_path, pending)
                calls_started += 1

                interruption_stage = "runner"
                try:
                    trace = runner.run_task(task)
                    interruption_stage = "usage_accounting"
                    usage = account_provider_usage(
                        provider_events_from_trace(trace),
                        profile,
                    )
                    interruption_stage = "trust_evaluation"
                    trust_result = _trust_for_trace(
                        task,
                        trace,
                        dataset_fingerprint_value=metadata.dataset_fingerprint,
                        harness_check=harness_check,
                    )
                    interruption_stage = "artifact_publish"
                    artifact = _attempt_artifact(
                        pending,
                        profile,
                        trace,
                        usage,
                        trust_result,
                    )
                    artifact_path = campaign_root / _ATTEMPTS_DIR / f"{attempt_id}.json"
                    if artifact_path.exists():
                        raise CampaignStateError("duplicate_attempt_artifact")
                    _atomic_write_model(artifact_path, artifact)
                except BaselineCrashInjected:
                    raise
                except Exception as exc:
                    with suppress(OSError, ValidationError, ValueError):
                        _atomic_write_model(
                            pending_path,
                            pending.model_copy(
                                update={
                                    "interruption_stage": interruption_stage,
                                    "interruption_reason": type(exc).__name__,
                                }
                            ),
                        )
                    return _blocked_result(
                        campaign_root,
                        [_INTERRUPTED_BLOCKER],
                        calls_started=calls_started,
                    )

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
                    return _blocked_result(
                        campaign_root,
                        blockers,
                        calls_started=calls_started,
                    )

        return _run_result(campaign_root, "complete", calls_started)


def recover_baseline_campaign(
    campaign_root: Path,
    *,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
) -> BaselineRunResult:
    with _campaign_lock(campaign_root):
        try:
            metadata = _verify_campaign_inputs(
                campaign_root,
                repository_root=repository_root,
                cases_path=cases_path,
                dataset_lock_path=dataset_lock_path,
                profile_path=profile_path,
            )
            preflight = verify_baseline_campaign(
                campaign_root,
                repository_root=repository_root,
                cases_path=cases_path,
                dataset_lock_path=dataset_lock_path,
                profile_path=profile_path,
            )
            recoverable_preflight_blockers = _REPORT_SEMANTIC_BLOCKERS | {
                "artifact_budget_entry_count_mismatch",
                "artifact_budget_attempt_ids_mismatch",
            }
            if set(preflight.blockers) - recoverable_preflight_blockers:
                return _blocked_result(
                    campaign_root,
                    ["campaign_state_invalid"],
                    calls_started=0,
                )
            profile = load_harness_profile(profile_path)
            entries = _load_budget_entries(campaign_root / _BUDGET_FILE)
            entry_ids = {entry.attempt_id for entry in entries}
            pending_paths = sorted((campaign_root / _PENDING_DIR).glob("*.json"))
            if pending_paths:
                _remove_report_outputs(campaign_root)
            for pending_path in pending_paths:
                pending = _read_canonical_model(
                    pending_path,
                    BaselinePendingAttempt,
                    "pending_record_invalid",
                )
                _validate_pending_identity(pending, metadata, profile)
                artifact_path = campaign_root / _ATTEMPTS_DIR / f"{pending.attempt_id}.json"
                if artifact_path.exists():
                    artifact = _read_canonical_model(
                        artifact_path,
                        BaselineAttemptArtifact,
                        "attempt_artifact_invalid",
                    )
                else:
                    artifact = _interrupted_attempt_artifact(pending, profile)
                    _atomic_write_model(artifact_path, artifact)

                if pending.attempt_id not in entry_ids:
                    entry = _budget_entry_for_artifact(entries, artifact)
                    _append_budget_entry(campaign_root / _BUDGET_FILE, entry)
                    entries.append(entry)
                    entry_ids.add(pending.attempt_id)
                _remove_pending(pending_path)
        except (CampaignStateError, OSError, ValidationError, ValueError):
            return _blocked_result(
                campaign_root,
                ["campaign_state_invalid"],
                calls_started=0,
            )

        verification = verify_baseline_campaign(
            campaign_root,
            repository_root=repository_root,
            cases_path=cases_path,
            dataset_lock_path=dataset_lock_path,
            profile_path=profile_path,
        )
        if not verification.ok:
            return BaselineRunResult(
                status="blocked",
                calls_started=0,
                terminal_attempts=verification.attempt_count,
                spent_cny=verification.spent_cny,
                blockers=verification.blockers,
            )
        status = "complete" if verification.attempt_count == metadata.max_attempts else "paused"
        return BaselineRunResult(
            status=status,
            calls_started=0,
            terminal_attempts=verification.attempt_count,
            spent_cny=verification.spent_cny,
        )


def verify_baseline_campaign(
    campaign_root: Path,
    *,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
) -> BaselineVerification:
    blockers: list[str] = []
    attempt_count = 0
    pending_count = 0
    spent_cny = Decimal(0)
    try:
        metadata = _verify_campaign_inputs(
            campaign_root,
            repository_root=repository_root,
            cases_path=cases_path,
            dataset_lock_path=dataset_lock_path,
            profile_path=profile_path,
        )
        profile = load_harness_profile(profile_path)
        tasks, _dataset_lock = _load_locked_development_tasks(
            cases_path,
            dataset_lock_path,
            profile,
        )
        tasks_by_id = {task.id: task for task in tasks}
        artifacts = _load_attempt_artifacts(campaign_root / _ATTEMPTS_DIR)
        entries = _load_budget_entries(campaign_root / _BUDGET_FILE)
        pending = _load_pending_attempts(campaign_root / _PENDING_DIR)
        attempt_count = len(artifacts)
        pending_count = len(pending)
        spent_cny = entries[-1].cumulative_cost_cny if entries else Decimal(0)

        blockers.extend(_verify_pending(pending, metadata, profile))
        blockers.extend(
            _verify_attempts_and_budget(
                artifacts,
                entries,
                metadata,
                profile,
                tasks_by_id,
            )
        )
        blockers.extend(
            _verify_report_outputs(
                campaign_root,
                artifacts,
                entries,
                metadata,
            )
        )
        if spent_cny > metadata.budget_cny:
            blockers.append("campaign_budget_exceeded")
    except CampaignStateError as exc:
        blockers.append(exc.blocker)
    except (OSError, ValidationError, ValueError, json.JSONDecodeError):
        blockers.append("campaign_state_invalid")

    blockers = sorted(set(blockers))
    return BaselineVerification(
        ok=not blockers,
        attempt_count=attempt_count,
        pending_count=pending_count,
        spent_cny=spent_cny,
        blockers=blockers,
    )


def generate_baseline_report(
    campaign_root: Path,
    *,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
) -> BaselineReport:
    verification = verify_baseline_campaign(
        campaign_root,
        repository_root=repository_root,
        cases_path=cases_path,
        dataset_lock_path=dataset_lock_path,
        profile_path=profile_path,
    )
    if verification.pending_count:
        raise ValueError("baseline report requires recovery of pending paid attempts")
    structural_blockers = set(verification.blockers) - _REPORT_SEMANTIC_BLOCKERS
    if structural_blockers:
        raise ValueError("baseline campaign verification failed")

    metadata = _read_canonical_model(
        campaign_root / _CAMPAIGN_FILE,
        BaselineCampaignMetadata,
        "campaign_metadata_invalid",
    )
    artifacts = _load_attempt_artifacts(campaign_root / _ATTEMPTS_DIR)
    entries = _load_budget_entries(campaign_root / _BUDGET_FILE)
    artifact_index = _build_artifact_index(campaign_root, metadata, artifacts)
    index_bytes = canonical_json_bytes(artifact_index.model_dump(mode="json")) + b"\n"
    report = _build_baseline_report(
        metadata,
        artifacts,
        entries,
        index_sha256=sha256_bytes(index_bytes),
    )
    _atomic_write_bytes(campaign_root / _ARTIFACT_INDEX_FILE, index_bytes)
    _atomic_write_model(campaign_root / _REPORT_FILE, report)
    return report


def _build_artifact_index(
    campaign_root: Path,
    metadata: BaselineCampaignMetadata,
    artifacts: dict[str, BaselineAttemptArtifact],
) -> BaselineArtifactIndex:
    ordered = sorted(
        artifacts.values(),
        key=lambda artifact: (artifact.repetition, artifact.task_id),
    )
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


def _build_baseline_report(
    metadata: BaselineCampaignMetadata,
    artifacts: dict[str, BaselineAttemptArtifact],
    entries: list[BaselineBudgetEntry],
    *,
    index_sha256: str,
) -> BaselineReport:
    ordered = sorted(
        artifacts.values(),
        key=lambda artifact: (artifact.repetition, artifact.task_id),
    )
    completed = len(ordered)
    attempt_blockers = {artifact.attempt_id: _attempt_blockers(artifact) for artifact in ordered}
    trusted = [artifact for artifact in ordered if not attempt_blockers[artifact.attempt_id]]
    trusted_successes = sum(1 for artifact in trusted if trace_passed(artifact.trace))
    verification_passes = sum(
        1
        for artifact in ordered
        if artifact.trace.verification_result is not None
        and artifact.trace.verification_result.passed
    )
    scope_violations = sum(1 for artifact in ordered if artifact.trace.scope_violations)
    rounds = [artifact.trace.model_rounds for artifact in ordered]
    known_costs = [
        artifact.usage.cost_cny
        for artifact in ordered
        if artifact.usage.status == "known" and artifact.usage.cost_cny is not None
    ]
    unknown_cost_attempts = sum(
        1
        for artifact in ordered
        if artifact.usage.status != "known" or artifact.usage.cost_cny is None
    )

    failure_categories = Counter(artifact.primary_failure_category for artifact in ordered)
    trust_blocker_counts: Counter[str] = Counter()
    for artifact in ordered:
        blockers = set(attempt_blockers[artifact.attempt_id])
        blockers.update(artifact.trust_result.blockers)
        trust_blocker_counts.update(blockers)

    by_task: dict[str, list[BaselineAttemptArtifact]] = defaultdict(list)
    for artifact in ordered:
        by_task[artifact.task_id].append(artifact)
    per_task: dict[str, dict[str, int | float | bool]] = {}
    for task_id in metadata.task_ids:
        task_attempts = by_task[task_id]
        task_successes = [trace_passed(artifact.trace) for artifact in task_attempts]
        task_trusted = [
            artifact for artifact in task_attempts if not attempt_blockers[artifact.attempt_id]
        ]
        task_trusted_successes = sum(1 for artifact in task_trusted if trace_passed(artifact.trace))
        per_task[task_id] = {
            "attempts": len(task_attempts),
            "trusted_attempts": len(task_trusted),
            "trusted_successes": task_trusted_successes,
            "successes": sum(task_successes),
            "trusted_success_rate": (
                task_trusted_successes / len(task_attempts) if task_attempts else 0.0
            ),
            "verification_passes": sum(
                1
                for artifact in task_attempts
                if artifact.trace.verification_result is not None
                and artifact.trace.verification_result.passed
            ),
            "scope_violations": sum(
                1 for artifact in task_attempts if artifact.trace.scope_violations
            ),
            "median_model_rounds": (
                float(median(artifact.trace.model_rounds for artifact in task_attempts))
                if task_attempts
                else 0.0
            ),
            "flaky": bool(task_successes) and any(task_successes) and not all(task_successes),
        }

    if trust_blocker_counts:
        status = "blocked"
    elif completed == metadata.max_attempts:
        status = "complete"
    else:
        status = "incomplete"
    total_cost = entries[-1].cumulative_cost_cny if entries else Decimal(0)
    return BaselineReport(
        campaign_id=metadata.campaign_id,
        status=status,
        source_commit=metadata.source_commit,
        dirty_state_sha256=metadata.dirty_state_sha256,
        dataset_fingerprint=metadata.dataset_fingerprint,
        harness_fingerprint=metadata.harness_fingerprint,
        provider=metadata.profile.provider,
        model=metadata.profile.model,
        transport=metadata.profile.transport,
        expected_attempts=metadata.max_attempts,
        completed_attempts=completed,
        trusted_attempts=len(trusted),
        trusted_success_rate=trusted_successes / completed if completed else 0.0,
        verification_pass_rate=verification_passes / completed if completed else 0.0,
        scope_violation_rate=scope_violations / completed if completed else 0.0,
        median_model_rounds=float(median(rounds)) if rounds else 0.0,
        p95_model_rounds=float(_nearest_rank_int(rounds)) if rounds else 0.0,
        median_cost_cny=median(known_costs) if known_costs else None,
        p95_cost_cny=(_nearest_rank_decimal(known_costs) if known_costs else None),
        total_cost_cny=total_cost,
        unknown_cost_attempts=unknown_cost_attempts,
        failure_categories=dict(sorted(failure_categories.items())),
        per_task=per_task,
        trust_blockers=dict(sorted(trust_blocker_counts.items())),
        attempt_artifact_index_sha256=index_sha256,
        harness_telemetry=summarize_harness_telemetry([artifact.trace for artifact in ordered]),
    )


def _verify_report_outputs(
    campaign_root: Path,
    artifacts: dict[str, BaselineAttemptArtifact],
    entries: list[BaselineBudgetEntry],
    metadata: BaselineCampaignMetadata,
) -> list[str]:
    index_path = campaign_root / _ARTIFACT_INDEX_FILE
    report_path = campaign_root / _REPORT_FILE
    if not index_path.exists() and not report_path.exists():
        return []
    if not index_path.exists() or not report_path.exists():
        return ["baseline_report_outputs_incomplete"]
    try:
        actual_index = _read_canonical_model(
            index_path,
            BaselineArtifactIndex,
            "baseline_artifact_index_invalid",
        )
        actual_report = _read_canonical_model(
            report_path,
            BaselineReport,
            "baseline_report_invalid",
        )
        expected_index = _build_artifact_index(campaign_root, metadata, artifacts)
        if actual_index != expected_index:
            return ["baseline_artifact_index_mismatch"]
        expected_report = _build_baseline_report(
            metadata,
            artifacts,
            entries,
            index_sha256=sha256_bytes(index_path.read_bytes()),
        )
        if actual_report != expected_report:
            return ["baseline_report_mismatch"]
    except CampaignStateError as exc:
        return [exc.blocker]
    return []


def _nearest_rank_int(values: list[int]) -> int:
    ordered = sorted(values)
    return ordered[ceil(0.95 * len(ordered)) - 1]


def _nearest_rank_decimal(values: list[Decimal]) -> Decimal:
    ordered = sorted(values)
    return ordered[ceil(Decimal("0.95") * len(ordered)) - 1]


def _verify_campaign_inputs(
    campaign_root: Path,
    *,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
) -> BaselineCampaignMetadata:
    metadata = _read_canonical_model(
        campaign_root / _CAMPAIGN_FILE,
        BaselineCampaignMetadata,
        "campaign_metadata_invalid",
    )
    profile = load_harness_profile(profile_path)
    tasks, dataset_lock = _load_locked_development_tasks(
        cases_path,
        dataset_lock_path,
        profile,
    )
    locked_profile = campaign_root / _LOCKS_DIR / _PROFILE_LOCK
    if locked_profile.read_bytes() != harness_profile_bytes(profile) + b"\n":
        raise CampaignStateError("profile_lock_mismatch")
    locked_dataset = campaign_root / _LOCKS_DIR / _DATASET_LOCK
    if locked_dataset.read_bytes() != canonical_json_bytes(dataset_lock) + b"\n":
        raise CampaignStateError("dataset_lock_mismatch")
    if sha256_bytes(dataset_lock_path.read_bytes()) != metadata.dataset_lock_sha256:
        raise CampaignStateError("dataset_lock_digest_mismatch")
    if harness_profile_fingerprint(profile) != metadata.harness_fingerprint:
        raise CampaignStateError("harness_fingerprint_mismatch")
    if profile != metadata.profile:
        raise CampaignStateError("campaign_profile_mismatch")
    if dataset_fingerprint(tasks) != metadata.dataset_fingerprint:
        raise CampaignStateError("development_dataset_fingerprint_mismatch")
    if [task.id for task in tasks] != metadata.task_ids:
        raise CampaignStateError("development_task_ids_mismatch")
    repository_root = repository_root.resolve()
    if _git_text(repository_root, "rev-parse", "HEAD") != metadata.source_commit:
        raise CampaignStateError("source_commit_mismatch")
    if _dirty_state_sha256(repository_root, campaign_root) != metadata.dirty_state_sha256:
        raise CampaignStateError("dirty_state_digest_mismatch")
    return metadata


def _load_locked_development_tasks(
    cases_path: Path,
    dataset_lock_path: Path,
    profile: HarnessProfileV1,
) -> tuple[list[EvaluationTask], dict[str, Any]]:
    tasks = sorted(load_cases(cases_path), key=lambda task: task.id)
    if (
        len(tasks) != profile.development_case_count
        or any(task.dataset_split != DatasetSplit.DEVELOPMENT for task in tasks)
        or any("holdout" in task.id for task in tasks)
    ):
        raise ValueError("baseline accepts the exact development split only")
    lock = json.loads(dataset_lock_path.read_text(encoding="utf-8"))
    development = lock.get("development")
    if not isinstance(development, dict):
        raise ValueError("dataset lock is missing development split")
    if development.get("ids") != [task.id for task in tasks]:
        raise ValueError("development task ids do not match dataset lock")
    if development.get("count") != len(tasks):
        raise ValueError("development task count does not match dataset lock")
    if development.get("fingerprint") != dataset_fingerprint(tasks):
        raise ValueError("development fingerprint does not match dataset lock")
    return tasks, lock


def _attempt_artifact(
    pending: BaselinePendingAttempt,
    profile: HarnessProfileV1,
    trace: AgentTrace,
    usage: ProviderUsageAccounting,
    trust_result: TrustGateResult,
) -> BaselineAttemptArtifact:
    trace_sha256 = sha256_bytes(canonical_json_bytes(trace.model_dump(mode="json")))
    return BaselineAttemptArtifact(
        attempt_id=pending.attempt_id,
        task_id=pending.task_id,
        repetition=pending.repetition,
        profile=profile,
        source_commit=pending.source_commit,
        dirty_state_sha256=pending.dirty_state_sha256,
        dataset_fingerprint=pending.dataset_fingerprint,
        harness_fingerprint=pending.harness_fingerprint,
        trace_sha256=trace_sha256,
        trace=trace,
        usage=usage,
        trust_result=trust_result,
        primary_failure_category=trace.failure_category.value,
        terminal=True,
    )


def _interrupted_attempt_artifact(
    pending: BaselinePendingAttempt,
    profile: HarnessProfileV1,
) -> BaselineAttemptArtifact:
    now = datetime.now(UTC)
    stage_detail = (
        f" at the {pending.interruption_stage} stage"
        if pending.interruption_stage is not None
        else ""
    )
    trace = AgentTrace(
        task_id=pending.task_id,
        user_prompt="Interrupted paid baseline attempt.",
        model=profile.profile_id,
        provider=EvalProvider.FORGE,
        final_answer="",
        error=_INTERRUPTED_BLOCKER,
        failure_reason=(
            f"Paid attempt was interrupted after its pending record was durable{stage_detail}."
        ),
        failure_category=FailureCategory.RUNNER_ERROR,
        raw_events=(
            [
                {
                    "event_type": "baseline_interruption",
                    "stage": pending.interruption_stage or "unknown",
                    **(
                        {"reason": pending.interruption_reason}
                        if pending.interruption_reason is not None
                        else {}
                    ),
                }
            ]
        ),
        started_at=pending.started_at,
        ended_at=now,
        duration_ms=max(0, int((now - pending.started_at).total_seconds() * 1000)),
    )
    usage = ProviderUsageAccounting(
        status="unknown",
        provider_event_count=0,
        blockers=[_INTERRUPTED_BLOCKER],
    )
    trust = TrustGateResult(
        status=TrustStatus.UNKNOWN,
        trusted=False,
        blockers=[_INTERRUPTED_BLOCKER],
    )
    trace_sha256 = sha256_bytes(canonical_json_bytes(trace.model_dump(mode="json")))
    return BaselineAttemptArtifact(
        attempt_id=pending.attempt_id,
        task_id=pending.task_id,
        repetition=pending.repetition,
        profile=profile,
        source_commit=pending.source_commit,
        dirty_state_sha256=pending.dirty_state_sha256,
        dataset_fingerprint=pending.dataset_fingerprint,
        harness_fingerprint=pending.harness_fingerprint,
        trace_sha256=trace_sha256,
        trace=trace,
        usage=usage,
        trust_result=trust,
        primary_failure_category=_INTERRUPTED_BLOCKER,
        terminal=True,
    )


def _budget_entry_for_artifact(
    entries: list[BaselineBudgetEntry],
    artifact: BaselineAttemptArtifact,
) -> BaselineBudgetEntry:
    previous = entries[-1] if entries else None
    if artifact.usage.status == "known":
        if artifact.usage.cost_cny is None:
            raise CampaignStateError("known_usage_cost_missing")
        status = "known"
        cost = artifact.usage.cost_cny
    else:
        status = "reserved_unknown"
        cost = artifact.profile.max_cost_cny_per_task
    cumulative = (previous.cumulative_cost_cny if previous else Decimal(0)) + cost
    return BaselineBudgetEntry(
        sequence=len(entries) + 1,
        previous_entry_sha256=(baseline_entry_digest(previous) if previous is not None else None),
        attempt_id=artifact.attempt_id,
        status=status,
        cost_cny=cost,
        cumulative_cost_cny=cumulative,
        artifact_sha256=baseline_artifact_digest(artifact),
    )


def _verify_attempts_and_budget(
    artifacts: dict[str, BaselineAttemptArtifact],
    entries: list[BaselineBudgetEntry],
    metadata: BaselineCampaignMetadata,
    profile: HarnessProfileV1,
    tasks_by_id: dict[str, EvaluationTask],
) -> list[str]:
    blockers: list[str] = []
    if len(entries) != len(artifacts):
        blockers.append("artifact_budget_entry_count_mismatch")
    entry_ids = [entry.attempt_id for entry in entries]
    if len(entry_ids) != len(set(entry_ids)):
        blockers.append("duplicate_budget_attempt_id")
    if set(entry_ids) != set(artifacts):
        blockers.append("artifact_budget_attempt_ids_mismatch")

    harness_check = run_observer_patch_replay_self_check()
    for attempt_id, artifact in artifacts.items():
        task = tasks_by_id.get(artifact.task_id)
        expected_attempt_id = _attempt_id(
            metadata.campaign_id,
            artifact.repetition,
            artifact.task_id,
        )
        if task is None or attempt_id != expected_attempt_id:
            blockers.append("attempt_identity_invalid")
            continue
        if (
            artifact.source_commit != metadata.source_commit
            or artifact.dirty_state_sha256 != metadata.dirty_state_sha256
            or artifact.dataset_fingerprint != metadata.dataset_fingerprint
            or artifact.harness_fingerprint != metadata.harness_fingerprint
            or artifact.profile != profile
        ):
            blockers.append("attempt_source_identity_mismatch")
        if not artifact.terminal:
            blockers.append("attempt_not_terminal")

        if artifact.primary_failure_category == _INTERRUPTED_BLOCKER:
            blockers.append(_INTERRUPTED_BLOCKER)
        else:
            recalculated_usage = account_provider_usage(
                provider_events_from_trace(artifact.trace),
                profile,
            )
            if recalculated_usage != artifact.usage:
                blockers.append("provider_usage_accounting_mismatch")
            expected_trust = _trust_for_trace(
                task,
                artifact.trace,
                dataset_fingerprint_value=metadata.dataset_fingerprint,
                harness_check=harness_check,
            )
            if expected_trust != artifact.trust_result:
                blockers.append("trust_result_mismatch")
            if artifact.primary_failure_category != artifact.trace.failure_category.value:
                blockers.append("failure_category_mismatch")
        blockers.extend(_attempt_blockers(artifact))

    previous: BaselineBudgetEntry | None = None
    cumulative = Decimal(0)
    for index, entry in enumerate(entries, start=1):
        if entry.sequence != index:
            blockers.append("budget_sequence_mismatch")
        expected_previous = baseline_entry_digest(previous) if previous is not None else None
        if entry.previous_entry_sha256 != expected_previous:
            blockers.append("budget_hash_chain_mismatch")
        budget_artifact = artifacts.get(entry.attempt_id)
        if budget_artifact is not None:
            if entry.artifact_sha256 != baseline_artifact_digest(budget_artifact):
                blockers.append("budget_artifact_digest_mismatch")
            expected_status = (
                "known" if budget_artifact.usage.status == "known" else "reserved_unknown"
            )
            expected_cost = (
                budget_artifact.usage.cost_cny
                if budget_artifact.usage.status == "known"
                else profile.max_cost_cny_per_task
            )
            if entry.status != expected_status or entry.cost_cny != expected_cost:
                blockers.append("budget_cost_mismatch")
        cumulative += entry.cost_cny
        if entry.cumulative_cost_cny != cumulative:
            blockers.append("budget_cumulative_cost_mismatch")
        previous = entry
    return blockers


def _attempt_blockers(artifact: BaselineAttemptArtifact) -> list[str]:
    blockers: list[str] = []
    if artifact.primary_failure_category == _INTERRUPTED_BLOCKER:
        blockers.append(_INTERRUPTED_BLOCKER)
        return blockers
    if artifact.usage.status != "known" or artifact.usage.cost_cny is None:
        blockers.append("provider_usage_unknown")
    if artifact.usage.per_task_cap_exceeded:
        blockers.append("per_task_cost_cap_exceeded")
    if not artifact.trust_result.trusted:
        blockers.append("trust_result_untrusted")
    identity = next(
        (
            event
            for event in artifact.trace.raw_events
            if event.get("event_type") == "eval_agent_identity"
        ),
        None,
    )
    legacy_identity_without_route_fields = identity is not None and not any(
        field in identity for field in ("endpoint_family", "base_url")
    )
    route_identity_required = (
        artifact.profile.base_url is not None or not legacy_identity_without_route_fields
    )
    if (
        identity is None
        or identity.get("provider") != artifact.profile.provider
        or identity.get("model") != artifact.profile.model
        or identity.get("transport") != artifact.profile.transport
        or identity.get("harness_fingerprint") != artifact.harness_fingerprint
        or identity.get("identity_matches_profile") is not True
        or (
            route_identity_required
            and identity.get("endpoint_family") != artifact.profile.endpoint_family
        )
        or (
            route_identity_required
            and identity.get("base_url") != artifact.profile.resolved_base_url
        )
    ):
        blockers.append("agent_identity_mismatch")
    if artifact.trace.failure_category == FailureCategory.FORGE_CONTRACT_ERROR:
        blockers.append("forge_contract_error")
    return sorted(set(blockers))


def _trust_for_trace(
    task: EvaluationTask,
    trace: AgentTrace,
    *,
    dataset_fingerprint_value: str,
    harness_check: Any,
) -> TrustGateResult:
    report = build_report([trace])
    scorer_calibrated = all(score_can_gate_ci(score) for score in score_trace(trace).values())
    return evaluate_trust_gates(
        harness_check=harness_check,
        dataset_fingerprint=dataset_fingerprint_value,
        case_quality_issues=validate_case_quality([task]),
        traces=[trace],
        scorer_calibrated=scorer_calibrated,
        red_team_passed=None,
        require_red_team=False,
        score_coverage=report.score_coverage,
    )


def _verify_pending(
    pending: dict[str, BaselinePendingAttempt],
    metadata: BaselineCampaignMetadata,
    profile: HarnessProfileV1,
) -> list[str]:
    blockers: list[str] = []
    for attempt_id, item in pending.items():
        try:
            _validate_pending_identity(item, metadata, profile)
            if attempt_id != item.attempt_id:
                raise CampaignStateError("pending_attempt_filename_mismatch")
        except CampaignStateError as exc:
            blockers.append(exc.blocker)
    if pending:
        blockers.append(_INTERRUPTED_BLOCKER)
    return blockers


def _validate_pending_identity(
    pending: BaselinePendingAttempt,
    metadata: BaselineCampaignMetadata,
    profile: HarnessProfileV1,
) -> None:
    if (
        pending.source_commit != metadata.source_commit
        or pending.dirty_state_sha256 != metadata.dirty_state_sha256
        or pending.dataset_fingerprint != metadata.dataset_fingerprint
        or pending.harness_fingerprint != metadata.harness_fingerprint
        or pending.reserved_cost_cny != profile.max_cost_cny_per_task
        or pending.task_id not in metadata.task_ids
        or pending.attempt_id
        != _attempt_id(metadata.campaign_id, pending.repetition, pending.task_id)
    ):
        raise CampaignStateError("pending_attempt_identity_mismatch")


def _load_attempt_artifacts(path: Path) -> dict[str, BaselineAttemptArtifact]:
    artifacts: dict[str, BaselineAttemptArtifact] = {}
    for artifact_path in sorted(path.iterdir()):
        if artifact_path.suffix != ".json":
            raise CampaignStateError("unexpected_attempt_artifact")
        artifact = _read_canonical_model(
            artifact_path,
            BaselineAttemptArtifact,
            "attempt_artifact_invalid",
        )
        if artifact_path.stem != artifact.attempt_id or artifact.attempt_id in artifacts:
            raise CampaignStateError("duplicate_or_misnamed_attempt_artifact")
        artifacts[artifact.attempt_id] = artifact
    return artifacts


def _load_pending_attempts(path: Path) -> dict[str, BaselinePendingAttempt]:
    pending: dict[str, BaselinePendingAttempt] = {}
    for pending_path in sorted(path.iterdir()):
        if pending_path.suffix != ".json":
            raise CampaignStateError("unexpected_pending_artifact")
        item = _read_canonical_model(
            pending_path,
            BaselinePendingAttempt,
            "pending_record_invalid",
        )
        if pending_path.stem != item.attempt_id or item.attempt_id in pending:
            raise CampaignStateError("duplicate_or_misnamed_pending_record")
        pending[item.attempt_id] = item
    return pending


def _load_budget_entries(path: Path) -> list[BaselineBudgetEntry]:
    if stat.S_IMODE(path.stat().st_mode) != 0o600:
        raise CampaignStateError("budget_ledger_mode_invalid")
    content = path.read_bytes()
    if not content:
        return []
    if not content.endswith(b"\n"):
        raise CampaignStateError("budget_ledger_not_newline_terminated")
    entries: list[BaselineBudgetEntry] = []
    for line in content.splitlines():
        try:
            payload = json.loads(line)
            entry = BaselineBudgetEntry.model_validate(payload)
        except (json.JSONDecodeError, ValidationError, ValueError) as exc:
            raise CampaignStateError("budget_ledger_invalid") from exc
        if line != canonical_json_bytes(entry.model_dump(mode="json")):
            raise CampaignStateError("budget_ledger_noncanonical")
        entries.append(entry)
    return entries


def _append_budget_entry(path: Path, entry: BaselineBudgetEntry) -> None:
    existing = _load_budget_entries(path)
    if len(existing) + 1 != entry.sequence:
        raise CampaignStateError("budget_sequence_mismatch")
    expected_previous = baseline_entry_digest(existing[-1]) if existing else None
    if entry.previous_entry_sha256 != expected_previous:
        raise CampaignStateError("budget_hash_chain_mismatch")
    line = canonical_json_bytes(entry.model_dump(mode="json")) + b"\n"
    descriptor = os.open(path, os.O_APPEND | os.O_WRONLY)
    try:
        if os.write(descriptor, line) != len(line):
            raise OSError("short budget ledger append")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    _fsync_directory(path.parent)


def _read_canonical_model(
    path: Path,
    model_type: type[_ModelT],
    blocker: str,
) -> _ModelT:
    try:
        raw = path.read_bytes()
        payload = json.loads(raw)
        model = model_type.model_validate(payload)
    except (OSError, json.JSONDecodeError, ValidationError, ValueError) as exc:
        raise CampaignStateError(blocker) from exc
    if raw != canonical_json_bytes(model.model_dump(mode="json")) + b"\n":
        raise CampaignStateError(blocker)
    return model


def _atomic_write_model(path: Path, model: EvalModel) -> None:
    _atomic_write_bytes(
        path,
        canonical_json_bytes(model.model_dump(mode="json")) + b"\n",
    )


def _atomic_write_bytes(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.",
        suffix=".tmp",
        dir=path.parent,
    )
    temporary_path = Path(temporary_name)
    try:
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "wb") as temporary:
            temporary.write(content)
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_path, path)
        os.chmod(path, 0o600)
        _fsync_directory(path.parent)
    finally:
        temporary_path.unlink(missing_ok=True)


def _create_empty_file(path: Path) -> None:
    descriptor = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    _fsync_directory(path.parent)


def _remove_pending(path: Path) -> None:
    path.unlink()
    _fsync_directory(path.parent)


def _remove_report_outputs(campaign_root: Path) -> None:
    removed = False
    for file_name in (_ARTIFACT_INDEX_FILE, _REPORT_FILE):
        path = campaign_root / file_name
        if path.exists():
            path.unlink()
            removed = True
    if removed:
        _fsync_directory(campaign_root)


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


@contextmanager
def _campaign_lock(campaign_root: Path) -> Iterator[None]:
    lock_path = campaign_root / _LOCK_FILE
    descriptor = os.open(lock_path, os.O_RDWR)
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        yield
    finally:
        fcntl.flock(descriptor, fcntl.LOCK_UN)
        os.close(descriptor)


def _attempt_id(campaign_id: str, repetition: int, task_id: str) -> str:
    return f"{campaign_id}-r{repetition:02d}-{task_id}"


def _run_result(
    campaign_root: Path,
    status: str,
    calls_started: int,
) -> BaselineRunResult:
    entries = _load_budget_entries(campaign_root / _BUDGET_FILE)
    return BaselineRunResult(
        status=status,
        calls_started=calls_started,
        terminal_attempts=len(entries),
        spent_cny=entries[-1].cumulative_cost_cny if entries else Decimal(0),
    )


def _blocked_result(
    campaign_root: Path,
    blockers: list[str],
    *,
    calls_started: int,
) -> BaselineRunResult:
    try:
        entries = _load_budget_entries(campaign_root / _BUDGET_FILE)
    except (CampaignStateError, OSError):
        entries = []
    return BaselineRunResult(
        status="blocked",
        calls_started=calls_started,
        terminal_attempts=len(entries),
        spent_cny=entries[-1].cumulative_cost_cny if entries else Decimal(0),
        blockers=sorted(set(blockers)),
    )


def _git_text(repository_root: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=repository_root,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise ValueError("unable to resolve baseline Git source identity")
    return completed.stdout.strip()


def _git_bytes(repository_root: Path, *args: str) -> bytes:
    completed = subprocess.run(
        ["git", *args],
        cwd=repository_root,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise ValueError("unable to resolve baseline Git dirty state")
    return completed.stdout


def _dirty_state_sha256(repository_root: Path, campaign_root: Path) -> str:
    excluded = _repository_relative_path(repository_root, campaign_root)
    status = _git_bytes(
        repository_root,
        "status",
        "--porcelain=v1",
        "--untracked-files=all",
        "-z",
    )
    filtered_entries: list[bytes] = []
    untracked_paths: list[str] = []
    raw_entries = [entry for entry in status.split(b"\0") if entry]
    index = 0
    while index < len(raw_entries):
        entry = raw_entries[index]
        index += 1
        if len(entry) < 4:
            continue
        path = os.fsdecode(entry[3:])
        rename_source: bytes | None = None
        if (b"R" in entry[:2] or b"C" in entry[:2]) and index < len(raw_entries):
            rename_source = raw_entries[index]
            index += 1
        if excluded is not None and _path_is_within(path, excluded):
            continue
        filtered_entries.append(entry)
        if rename_source is not None:
            filtered_entries.append(rename_source)
        if entry[:2] == b"??":
            untracked_paths.append(path)

    diff_args = ["diff", "--binary", "HEAD", "--", "."]
    if excluded is not None:
        diff_args.append(f":(exclude){excluded}/**")
    diff = _git_bytes(repository_root, *diff_args)
    untracked_manifest: list[dict[str, str]] = []
    for relative_path in sorted(untracked_paths):
        candidate = repository_root / relative_path
        if candidate.is_file():
            untracked_manifest.append(
                {
                    "path": relative_path,
                    "sha256": sha256_bytes(candidate.read_bytes()),
                }
            )
    payload = (
        b"status\0"
        + b"\0".join(filtered_entries)
        + b"\0diff\0"
        + diff
        + b"\0untracked\0"
        + canonical_json_bytes(untracked_manifest)
    )
    return sha256_bytes(payload)


def _repository_relative_path(
    repository_root: Path,
    candidate: Path,
) -> str | None:
    try:
        return candidate.resolve().relative_to(repository_root.resolve()).as_posix()
    except ValueError:
        return None


def _path_is_within(path: str, parent: str) -> bool:
    return path == parent or path.startswith(parent.rstrip("/") + "/")
