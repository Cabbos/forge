"""Offline, secret-free diagnostics for a verified H4 holdout campaign."""

from __future__ import annotations

from collections import Counter, defaultdict
from collections.abc import Sequence
from decimal import Decimal
from math import ceil
from statistics import median
from typing import Literal

from pydantic import Field, model_validator

from app.baseline_models import BaselineAttemptArtifact, HarnessTelemetrySummary
from app.harness_contracts import resolve_typed_failure_category
from app.harness_telemetry import summarize_harness_telemetry
from app.metrics import trace_passed
from app.models import EvalModel
from app.provider_errors import summarize_provider_errors


class HoldoutCaseDiagnosticV1(EvalModel):
    """Bounded per-case failure concentration for release investigation."""

    schema_version: Literal[1] = 1
    case_id: str
    attempts: int = Field(ge=1)
    trusted_attempts: int = Field(ge=0)
    trusted_success_rate: float = Field(ge=0.0, le=1.0)
    scope_violation_rate: float = Field(ge=0.0, le=1.0)
    median_model_rounds: float = Field(ge=0.0)
    p95_model_rounds: float = Field(ge=0.0)
    failure_categories: dict[str, int] = Field(default_factory=dict)
    provider_errors: dict[str, int] = Field(default_factory=dict)

    @model_validator(mode="after")
    def validate_counts(self) -> HoldoutCaseDiagnosticV1:
        if self.trusted_attempts > self.attempts:
            raise ValueError("trusted case attempts cannot exceed attempts")
        for field_name in ("failure_categories", "provider_errors"):
            if any(
                not key.strip() or value < 0 for key, value in getattr(self, field_name).items()
            ):
                raise ValueError(f"holdout {field_name} must contain nonnegative counts")
        return self


class HoldoutDiagnosticsV1(EvalModel):
    """Diagnostics attached to a release decision, never a threshold bypass."""

    schema_version: Literal[1] = 1
    attempts: int = Field(ge=1)
    cost_known_attempts: int = Field(ge=0)
    cost_known_rate: float = Field(ge=0.0, le=1.0)
    p95_model_rounds: float = Field(ge=0.0)
    median_duration_ms: float = Field(ge=0.0)
    p95_duration_ms: float = Field(ge=0.0)
    median_cost_cny: Decimal | None = Field(default=None, ge=0, allow_inf_nan=False)
    p95_cost_cny: Decimal | None = Field(default=None, ge=0, allow_inf_nan=False)
    failure_categories: dict[str, int] = Field(default_factory=dict)
    provider_errors: dict[str, int] = Field(default_factory=dict)
    trust_blockers: dict[str, int] = Field(default_factory=dict)
    harness_telemetry: HarnessTelemetrySummary = Field(default_factory=HarnessTelemetrySummary)
    cases: list[HoldoutCaseDiagnosticV1] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_counts(self) -> HoldoutDiagnosticsV1:
        if self.cost_known_attempts > self.attempts:
            raise ValueError("known-cost attempts cannot exceed attempts")
        expected_rate = self.cost_known_attempts / self.attempts
        if abs(self.cost_known_rate - expected_rate) > 1e-12:
            raise ValueError("holdout cost-known rate does not match attempt counts")
        for field_name in ("failure_categories", "provider_errors", "trust_blockers"):
            if any(
                not key.strip() or value < 0 for key, value in getattr(self, field_name).items()
            ):
                raise ValueError(f"holdout {field_name} must contain nonnegative counts")
        return self


def summarize_holdout_artifacts(
    artifacts: Sequence[BaselineAttemptArtifact],
) -> HoldoutDiagnosticsV1:
    """Aggregate verified artifacts without exposing raw trace/error text."""

    if not artifacts:
        raise ValueError("holdout diagnostics require at least one artifact")
    ordered = sorted(artifacts, key=lambda item: (item.repetition, item.task_id))
    costs = [artifact.usage.cost_cny for artifact in ordered if artifact.usage.cost_cny is not None]
    rounds = [float(artifact.trace.model_rounds) for artifact in ordered]
    durations = [float(artifact.trace.duration_ms) for artifact in ordered]
    failure_categories: Counter[str] = Counter()
    provider_errors: Counter[str] = Counter()
    trust_blockers: Counter[str] = Counter()
    for artifact in ordered:
        _update_failure_categories(failure_categories, artifact)
        provider_errors.update(
            summarize_provider_errors(artifact.trace.raw_events, artifact.trace.error)
        )
        trust_blockers.update(artifact.trust_result.blockers)

    by_case: dict[str, list[BaselineAttemptArtifact]] = defaultdict(list)
    for artifact in ordered:
        by_case[artifact.task_id].append(artifact)
    cases = [
        _case_diagnostic(case_id, case_artifacts)
        for case_id, case_artifacts in sorted(by_case.items())
    ]
    known_cost_count = len(costs)
    return HoldoutDiagnosticsV1(
        attempts=len(ordered),
        cost_known_attempts=known_cost_count,
        cost_known_rate=known_cost_count / len(ordered),
        p95_model_rounds=_nearest_rank(rounds),
        median_duration_ms=float(median(durations)),
        p95_duration_ms=_nearest_rank(durations),
        median_cost_cny=median(costs) if costs else None,
        p95_cost_cny=_nearest_rank(costs) if costs else None,
        failure_categories=dict(sorted(failure_categories.items())),
        provider_errors=dict(sorted(provider_errors.items())),
        trust_blockers=dict(sorted(trust_blockers.items())),
        harness_telemetry=summarize_harness_telemetry([artifact.trace for artifact in ordered]),
        cases=cases,
    )


def _case_diagnostic(
    case_id: str,
    artifacts: Sequence[BaselineAttemptArtifact],
) -> HoldoutCaseDiagnosticV1:
    rounds = [float(artifact.trace.model_rounds) for artifact in artifacts]
    failure_categories: Counter[str] = Counter()
    provider_errors: Counter[str] = Counter()
    for artifact in artifacts:
        _update_failure_categories(failure_categories, artifact)
        provider_errors.update(
            summarize_provider_errors(artifact.trace.raw_events, artifact.trace.error)
        )
    return HoldoutCaseDiagnosticV1(
        case_id=case_id,
        attempts=len(artifacts),
        trusted_attempts=sum(artifact.trust_result.trusted for artifact in artifacts),
        trusted_success_rate=sum(
            artifact.trust_result.trusted and trace_passed(artifact.trace) for artifact in artifacts
        )
        / len(artifacts),
        scope_violation_rate=sum(bool(artifact.trace.scope_violations) for artifact in artifacts)
        / len(artifacts),
        median_model_rounds=float(median(rounds)),
        p95_model_rounds=_nearest_rank(rounds),
        failure_categories=dict(sorted(failure_categories.items())),
        provider_errors=dict(sorted(provider_errors.items())),
    )


def _update_failure_categories(
    categories: Counter[str],
    artifact: BaselineAttemptArtifact,
) -> None:
    if trace_passed(artifact.trace):
        return
    category = resolve_typed_failure_category(
        artifact.trace.raw_events,
        artifact.primary_failure_category,
    )
    if category in {None, "", "none", "passed"}:
        category = "scope_violation" if artifact.trace.scope_violations else "runner_error"
    categories[category] += 1


def _nearest_rank(values: Sequence[float | Decimal]) -> float | Decimal:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = min(len(ordered) - 1, max(0, ceil(0.95 * len(ordered)) - 1))
    return ordered[index]
