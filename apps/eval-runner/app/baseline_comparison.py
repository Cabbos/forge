"""Fail-closed comparison of two independently verified baseline campaigns."""

from __future__ import annotations

import json
from collections import Counter
from collections.abc import Callable
from math import ceil
from pathlib import Path
from random import Random
from statistics import median
from typing import Any, Literal, TypeVar

from pydantic import ValidationError

from app.baseline import verify_baseline_campaign
from app.baseline_models import (
    BaselineAttemptArtifact,
    BaselineCampaignMetadata,
    BaselineComparison,
    BaselineComparisonMetrics,
    BaselineReport,
    BootstrapInterval,
)
from app.harness_profiles import HarnessProfileV1, harness_profile_fingerprint, load_harness_profile
from app.harness_telemetry import summarize_harness_telemetry
from app.harness_contracts import resolve_typed_failure_category
from app.metrics import trace_passed
from app.models import EvalModel
from app.provenance import canonical_json_bytes, sha256_bytes

_MODEL_T = TypeVar("_MODEL_T", bound=EvalModel)
_REPORT_NAME = "baseline-report.json"
_ATTEMPTS_DIR = "attempts"
_BUDGET_NAME = "budget.jsonl"
_ROUTE_FIELDS = {"profile_id", "transport", "endpoint_family", "base_url"}
_BOOTSTRAP_RESAMPLES = 2_000
_BOOTSTRAP_SEED = 20260808
ComparisonMode = Literal["transport_ab_v1", "policy_ab_v1"]


class BaselineComparisonError(ValueError):
    """Raised when a campaign pair cannot be compared without guessing."""

    def __init__(self, blocker: str, blockers: list[str] | None = None) -> None:
        self.blockers = blockers or [blocker]
        super().__init__(blocker)


def compare_baseline_campaigns(
    *,
    control_campaign_root: Path,
    candidate_campaign_root: Path,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    control_profile_path: Path,
    candidate_profile_path: Path,
    comparison_mode: ComparisonMode = "transport_ab_v1",
) -> BaselineComparison:
    """Verify and compare two explicit campaigns.

    The function intentionally receives both campaign and profile paths. It never
    resolves a "latest" directory, and it writes no campaign state while reading.
    """

    control = _load_verified_arm(
        control_campaign_root,
        repository_root=repository_root,
        cases_path=cases_path,
        dataset_lock_path=dataset_lock_path,
        profile_path=control_profile_path,
        arm="control",
    )
    candidate = _load_verified_arm(
        candidate_campaign_root,
        repository_root=repository_root,
        cases_path=cases_path,
        dataset_lock_path=dataset_lock_path,
        profile_path=candidate_profile_path,
        arm="candidate",
    )
    _validate_pair_alignment(control, candidate, comparison_mode=comparison_mode)

    control_metadata, control_profile, control_report, control_artifacts, control_order = control
    (
        candidate_metadata,
        candidate_profile,
        candidate_report,
        candidate_artifacts,
        candidate_order,
    ) = candidate
    control_metrics = _build_metrics(control_report, control_artifacts)
    candidate_metrics = _build_metrics(candidate_report, candidate_artifacts)
    bootstrap_intervals = _build_bootstrap_intervals(control_artifacts, candidate_artifacts)
    outcome_changes = {
        f"r{control_artifacts[control_key].repetition}:{control_artifacts[control_key].task_id}": (
            f"{_outcome(control_artifacts[control_key])}->{_outcome(candidate_artifacts[candidate_key])}"
        )
        for control_key, candidate_key in zip(control_order, candidate_order, strict=True)
    }
    winner = _select_winner(control_metrics, candidate_metrics, bootstrap_intervals)
    return BaselineComparison(
        control_campaign_id=control_metadata.campaign_id,
        candidate_campaign_id=candidate_metadata.campaign_id,
        dataset_fingerprint=control_metadata.dataset_fingerprint,
        model=control_profile.model,
        control_transport=control_profile.transport,  # type: ignore[arg-type]
        candidate_transport=candidate_profile.transport,  # type: ignore[arg-type]
        paired_attempts=30,
        outcome_changes=outcome_changes,
        control_metrics=control_metrics,
        candidate_metrics=candidate_metrics,
        winner=winner,
        blockers=[],
        control_profile_id=control_profile.profile_id,
        candidate_profile_id=candidate_profile.profile_id,
        control_harness_fingerprint=control_metadata.harness_fingerprint,
        candidate_harness_fingerprint=candidate_metadata.harness_fingerprint,
        source_commit=control_metadata.source_commit,
        bootstrap_intervals=bootstrap_intervals,
    )


def write_baseline_comparison(output_path: Path, comparison: BaselineComparison) -> str:
    """Write canonical comparison JSON and a SHA-256 sidecar, returning the digest."""

    payload = canonical_json_bytes(comparison.model_dump(mode="json")) + b"\n"
    digest = sha256_bytes(payload)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_bytes(payload)
    output_path.with_suffix(output_path.suffix + ".sha256").write_text(
        digest + "\n",
        encoding="utf-8",
    )
    return digest


def _load_verified_arm(
    campaign_root: Path,
    *,
    repository_root: Path,
    cases_path: Path,
    dataset_lock_path: Path,
    profile_path: Path,
    arm: str,
) -> tuple[
    BaselineCampaignMetadata,
    HarnessProfileV1,
    BaselineReport,
    dict[str, BaselineAttemptArtifact],
    list[str],
]:
    verification = verify_baseline_campaign(
        campaign_root,
        repository_root=repository_root,
        cases_path=cases_path,
        dataset_lock_path=dataset_lock_path,
        profile_path=profile_path,
    )
    if not verification.ok:
        _raise_arm_blocker(
            arm, verification.blockers[0] if verification.blockers else "campaign_unverified"
        )
    try:
        metadata = _read_canonical(campaign_root / "campaign.json", BaselineCampaignMetadata)
        profile = load_harness_profile(profile_path)
        if not (campaign_root / _REPORT_NAME).exists():
            _raise_arm_blocker(arm, "campaign_not_complete")
        report = _read_canonical(campaign_root / _REPORT_NAME, BaselineReport)
        artifacts = _load_artifacts(campaign_root / _ATTEMPTS_DIR)
        order = _load_order(campaign_root / _BUDGET_NAME)
    except BaselineComparisonError:
        raise
    except (OSError, ValidationError, ValueError, json.JSONDecodeError) as exc:
        _raise_arm_blocker(arm, "campaign_artifact_invalid")
        raise AssertionError("unreachable") from exc

    if report.status != "complete":
        _raise_arm_blocker(arm, "campaign_not_complete")
    if report.expected_attempts != 30 or report.completed_attempts != 30:
        _raise_arm_blocker(arm, "campaign_attempt_count_mismatch")
    if report.trusted_attempts != 30 or report.trust_blockers:
        _raise_arm_blocker(arm, "campaign_not_fully_trusted")
    if len(artifacts) != 30 or len(order) != 30:
        _raise_arm_blocker(arm, "campaign_attempt_count_mismatch")
    expected_order = [
        f"{metadata.campaign_id}-r{repetition:02d}-{task_id}"
        for repetition in range(1, metadata.repetitions + 1)
        for task_id in metadata.task_ids
    ]
    if order != expected_order:
        _raise_arm_blocker(arm, "attempt_order_mismatch")
    if set(order) != set(artifacts):
        _raise_arm_blocker(arm, "attempt_keys_mismatch")
    if any(not artifact.trust_result.trusted for artifact in artifacts.values()):
        _raise_arm_blocker(arm, "campaign_not_fully_trusted")
    return metadata, profile, report, artifacts, order


def _validate_pair_alignment(
    control: tuple[
        BaselineCampaignMetadata,
        HarnessProfileV1,
        BaselineReport,
        dict[str, BaselineAttemptArtifact],
        list[str],
    ],
    candidate: tuple[
        BaselineCampaignMetadata,
        HarnessProfileV1,
        BaselineReport,
        dict[str, BaselineAttemptArtifact],
        list[str],
    ],
    *,
    comparison_mode: ComparisonMode,
) -> None:
    control_metadata, control_profile, control_report, control_artifacts, control_order = control
    (
        candidate_metadata,
        candidate_profile,
        candidate_report,
        candidate_artifacts,
        candidate_order,
    ) = candidate
    if comparison_mode == "transport_ab_v1":
        if control_profile.transport == candidate_profile.transport:
            raise BaselineComparisonError("transport_same")
        if control_profile.transport != "anthropic_messages":
            raise BaselineComparisonError("control_transport_invalid")
        if candidate_profile.transport != "openai_chat_completions":
            raise BaselineComparisonError("candidate_transport_invalid")
    elif comparison_mode == "policy_ab_v1":
        if control_profile.transport != candidate_profile.transport:
            raise BaselineComparisonError("policy_route_mismatch")
        if (
            control_profile.endpoint_family != candidate_profile.endpoint_family
            or control_profile.resolved_base_url != candidate_profile.resolved_base_url
        ):
            raise BaselineComparisonError("policy_route_mismatch")
    else:
        raise BaselineComparisonError("comparison_mode_invalid")
    if control_profile.provider != candidate_profile.provider:
        raise BaselineComparisonError("provider_mismatch")
    if control_profile.model != candidate_profile.model:
        raise BaselineComparisonError("model_mismatch")
    if control_report.model != candidate_report.model:
        raise BaselineComparisonError("report_model_mismatch")
    for field_name in (
        "source_commit",
        "dirty_state_sha256",
        "dataset_fingerprint",
        "dataset_lock_sha256",
    ):
        if getattr(control_metadata, field_name) != getattr(candidate_metadata, field_name):
            raise BaselineComparisonError(f"{field_name}_mismatch")
    if control_metadata.task_ids != candidate_metadata.task_ids:
        raise BaselineComparisonError("task_order_mismatch")
    if (
        control_metadata.repetitions != candidate_metadata.repetitions
        or control_metadata.max_attempts != candidate_metadata.max_attempts
    ):
        raise BaselineComparisonError("repetition_policy_mismatch")
    control_order_keys = [
        (control_artifacts[item].repetition, control_artifacts[item].task_id)
        for item in control_order
    ]
    candidate_order_keys = [
        (candidate_artifacts[item].repetition, candidate_artifacts[item].task_id)
        for item in candidate_order
    ]
    if control_order_keys != candidate_order_keys:
        raise BaselineComparisonError("attempt_order_mismatch")
    control_attempt_keys = {
        (artifact.repetition, artifact.task_id) for artifact in control_artifacts.values()
    }
    candidate_attempt_keys = {
        (artifact.repetition, artifact.task_id) for artifact in candidate_artifacts.values()
    }
    if control_attempt_keys != candidate_attempt_keys:
        raise BaselineComparisonError("attempt_keys_mismatch")
    control_policy = control_profile_dump(control_profile)
    candidate_policy = control_profile_dump(candidate_profile)
    if comparison_mode == "transport_ab_v1":
        if control_policy != candidate_policy:
            raise BaselineComparisonError("profile_policy_mismatch")
    elif (
        control_policy.get("prompt_mode"),
        candidate_policy.get("prompt_mode"),
    ) not in {
        ("minimal_v1", "planning_v1"),
        ("planning_v1", "minimal_v1"),
    } or {
        key
        for key in set(control_policy) | set(candidate_policy)
        if control_policy.get(key) != candidate_policy.get(key)
    } != {"prompt_mode"}:
        raise BaselineComparisonError("policy_profile_mismatch")
    if harness_profile_fingerprint(control_profile) == harness_profile_fingerprint(
        candidate_profile
    ):
        raise BaselineComparisonError("profile_fingerprint_not_distinct")
    if control_metadata.harness_fingerprint == candidate_metadata.harness_fingerprint:
        raise BaselineComparisonError("harness_fingerprint_not_distinct")


def control_profile_dump(profile: HarnessProfileV1) -> dict[str, Any]:
    """Return profile identity with only the route dimensions removed."""

    dump = profile.model_dump(mode="json", exclude_none=True)
    for field_name in _ROUTE_FIELDS:
        dump.pop(field_name, None)
    return dump


def _build_metrics(
    report: BaselineReport,
    artifacts: dict[str, BaselineAttemptArtifact],
) -> BaselineComparisonMetrics:
    ordered = sorted(artifacts.values(), key=lambda item: (item.repetition, item.task_id))
    costs = [artifact.usage.cost_cny for artifact in ordered]
    if any(cost is None for cost in costs):
        raise BaselineComparisonError("provider_usage_unknown")
    cost_values = [cost for cost in costs if cost is not None]
    rounds = [artifact.trace.model_rounds for artifact in ordered]
    durations = [artifact.trace.duration_ms for artifact in ordered]
    provider_errors: Counter[str] = Counter()
    failure_categories: Counter[str] = Counter()
    trust_blockers: Counter[str] = Counter()
    provider_error_types = {
        "provider_error",
        "provider_call_error",
        "provider_request_error",
        "api_error",
    }
    for artifact in ordered:
        category = resolve_typed_failure_category(
            artifact.trace.raw_events, artifact.primary_failure_category
        )
        if category is not None:
            failure_categories[category] += 1
        trust_blockers.update(artifact.trust_result.blockers)
        for event in artifact.trace.raw_events:
            event_type = str(event.get("event_type", "")).lower()
            if event_type in provider_error_types or event.get("provider_error") is True:
                provider_errors[
                    str(event.get("error") or event.get("code") or "provider_error")
                ] += 1
        if artifact.trace.error and any(
            token in artifact.trace.error.lower()
            for token in ("provider", "api", "anthropic", "openai", "rate_limit", "http")
        ):
            provider_errors[artifact.trace.error] += 1
    return BaselineComparisonMetrics(
        trusted_attempts=report.trusted_attempts,
        trusted_success_rate=report.trusted_success_rate,
        verification_pass_rate=report.verification_pass_rate,
        scope_violation_rate=report.scope_violation_rate,
        median_model_rounds=float(median(rounds)),
        p95_model_rounds=float(_nearest_rank(rounds)),
        median_cost_cny=median(cost_values),
        p95_cost_cny=_nearest_rank(cost_values),
        median_duration_ms=float(median(durations)),
        p95_duration_ms=float(_nearest_rank(durations)),
        failure_categories=dict(sorted(failure_categories.items())),
        provider_errors=dict(sorted(provider_errors.items())),
        trust_blockers=dict(sorted(trust_blockers.items())),
        harness_telemetry=summarize_harness_telemetry([artifact.trace for artifact in ordered]),
    )


def _select_winner(
    control: BaselineComparisonMetrics,
    candidate: BaselineComparisonMetrics,
    intervals: dict[str, BootstrapInterval],
) -> str:
    success = intervals["trusted_success_rate_delta"]
    verification = intervals["verification_pass_rate_delta"]
    scope = intervals["scope_violation_rate_delta"]

    if success.high < 0 or verification.high < 0 or scope.low > 0:
        return "control"

    quality_noninferior = verification.low >= 0 and scope.high <= 0
    if not quality_noninferior or success.low < 0:
        return "no_material_difference"
    if success.low > 0:
        return "candidate"

    if (
        intervals["median_model_rounds_delta"].high < 0
        or intervals["median_duration_ms_delta"].high < 0
    ):
        return "candidate"
    return "no_material_difference"


def _build_bootstrap_intervals(
    control_artifacts: dict[str, BaselineAttemptArtifact],
    candidate_artifacts: dict[str, BaselineAttemptArtifact],
) -> dict[str, BootstrapInterval]:
    control_order = sorted(
        control_artifacts.values(), key=lambda item: (item.repetition, item.task_id)
    )
    candidate_order = sorted(
        candidate_artifacts.values(), key=lambda item: (item.repetition, item.task_id)
    )
    if len(control_order) != len(candidate_order) or not control_order:
        raise BaselineComparisonError("bootstrap_pair_count_mismatch")
    metrics: dict[
        str, tuple[Callable[[BaselineAttemptArtifact], float], Callable[[list[float]], float]]
    ] = {
        "trusted_success_rate_delta": (
            lambda artifact: float(trace_passed(artifact.trace)),
            _mean,
        ),
        "verification_pass_rate_delta": (
            lambda artifact: float(
                artifact.trace.verification_result is not None
                and artifact.trace.verification_result.passed
            ),
            _mean,
        ),
        "scope_violation_rate_delta": (
            lambda artifact: float(bool(artifact.trace.scope_violations)),
            _mean,
        ),
        "median_model_rounds_delta": (
            lambda artifact: float(artifact.trace.model_rounds),
            median,
        ),
        "median_duration_ms_delta": (
            lambda artifact: float(artifact.trace.duration_ms),
            median,
        ),
    }
    intervals: dict[str, BootstrapInterval] = {}
    for metric_index, (name, (value_of, estimator)) in enumerate(metrics.items()):
        intervals[name] = _paired_bootstrap_interval(
            [value_of(artifact) for artifact in control_order],
            [value_of(artifact) for artifact in candidate_order],
            estimator=estimator,
            seed=_BOOTSTRAP_SEED + metric_index,
        )
    return intervals


def _paired_bootstrap_interval(
    control_values: list[float],
    candidate_values: list[float],
    *,
    estimator: Callable[[list[float]], float],
    seed: int,
) -> BootstrapInterval:
    if len(control_values) != len(candidate_values) or not control_values:
        raise BaselineComparisonError("bootstrap_pair_count_mismatch")
    estimate = estimator(candidate_values) - estimator(control_values)
    rng = Random(seed)
    deltas: list[float] = []
    count = len(control_values)
    for _ in range(_BOOTSTRAP_RESAMPLES):
        sample_indexes = [rng.randrange(count) for _ in range(count)]
        control_sample = [control_values[index] for index in sample_indexes]
        candidate_sample = [candidate_values[index] for index in sample_indexes]
        deltas.append(estimator(candidate_sample) - estimator(control_sample))
    ordered = sorted(deltas)
    low = ordered[int(0.025 * (_BOOTSTRAP_RESAMPLES - 1))]
    high = ordered[int(0.975 * (_BOOTSTRAP_RESAMPLES - 1))]
    return BootstrapInterval(
        estimate=estimate,
        low=min(low, estimate),
        high=max(high, estimate),
        resamples=_BOOTSTRAP_RESAMPLES,
        seed=seed,
    )


def _mean(values: list[float]) -> float:
    return sum(values) / len(values) if values else 0.0


def _outcome(artifact: BaselineAttemptArtifact) -> str:
    return "success" if trace_passed(artifact.trace) else "failure"


def _load_artifacts(path: Path) -> dict[str, BaselineAttemptArtifact]:
    if not path.is_dir():
        raise ValueError("attempt artifact directory is missing")
    artifacts: dict[str, BaselineAttemptArtifact] = {}
    for artifact_path in sorted(path.glob("*.json")):
        artifact = _read_canonical(artifact_path, BaselineAttemptArtifact)
        if artifact.attempt_id in artifacts:
            raise ValueError("duplicate attempt artifact")
        artifacts[artifact.attempt_id] = artifact
    return artifacts


def _load_order(path: Path) -> list[str]:
    if not path.is_file():
        raise ValueError("campaign budget is missing")
    order: list[str] = []
    for line in path.read_bytes().splitlines():
        if not line:
            continue
        payload = json.loads(line)
        order.append(str(payload["attempt_id"]))
    return order


def _read_canonical(path: Path, model: type[_MODEL_T]) -> _MODEL_T:
    raw = path.read_bytes()
    value = model.model_validate_json(raw)
    if raw != canonical_json_bytes(value.model_dump(mode="json")) + b"\n":
        raise ValueError("non-canonical campaign artifact")
    return value


def _nearest_rank(values: list[Any]) -> Any:
    return sorted(values)[ceil(0.95 * len(values)) - 1]


def _raise_arm_blocker(arm: str, blocker: str) -> None:
    raise BaselineComparisonError(f"{arm}_{blocker}")
