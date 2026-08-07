import json
from datetime import datetime
from decimal import Decimal
from pathlib import PurePosixPath
from typing import Any, Literal

from pydantic import Field, field_validator, model_validator

from app.harness_profiles import HarnessProfileV1, harness_profile_fingerprint
from app.models import AgentTrace, EvalModel, EvalProvider, TrustGateResult
from app.provenance import canonical_json_bytes, sha256_bytes
from app.provenance_models import (
    SAFE_ID_PATTERN,
    SECRET_PATTERN,
    GitSha1,
    Sha256,
)
from app.provider_usage import ProviderUsageAccounting


class BaselinePendingAttempt(EvalModel):
    schema_version: Literal[1] = 1
    attempt_id: str
    task_id: str
    repetition: int = Field(ge=1, le=3)
    state: Literal["provider_call_pending"] = "provider_call_pending"
    source_commit: GitSha1
    dirty_state_sha256: Sha256
    dataset_fingerprint: Sha256
    harness_fingerprint: Sha256
    reserved_cost_cny: Decimal = Field(gt=0, allow_inf_nan=False)
    started_at: datetime
    interruption_stage: (
        Literal[
            "runner",
            "usage_accounting",
            "trust_evaluation",
            "artifact_publish",
        ]
        | None
    ) = Field(default=None, exclude_if=lambda value: value is None)

    @model_validator(mode="after")
    def validate_pending_attempt(self) -> "BaselinePendingAttempt":
        _validate_safe_ids(self.attempt_id, self.task_id)
        _reject_secret_material(self.model_dump(mode="json"))
        return self


class BaselineAttemptArtifact(EvalModel):
    schema_version: Literal[1] = 1
    attempt_id: str
    task_id: str
    repetition: int = Field(ge=1, le=3)
    profile: HarnessProfileV1
    source_commit: GitSha1
    dirty_state_sha256: Sha256
    dataset_fingerprint: Sha256
    harness_fingerprint: Sha256
    trace_sha256: Sha256
    trace: AgentTrace
    usage: ProviderUsageAccounting
    trust_result: TrustGateResult
    primary_failure_category: str
    terminal: bool

    @model_validator(mode="after")
    def validate_attempt_artifact(self) -> "BaselineAttemptArtifact":
        _validate_safe_ids(self.attempt_id, self.task_id)
        if not self.primary_failure_category.strip():
            raise ValueError("primary failure category must not be empty")
        if self.trace.task_id != self.task_id:
            raise ValueError("trace task identity does not match artifact")
        if self.trace.provider != EvalProvider.FORGE:
            raise ValueError("baseline trace must come from Forge")
        if harness_profile_fingerprint(self.profile) != self.harness_fingerprint:
            raise ValueError("profile fingerprint does not match artifact")
        actual_trace_sha256 = sha256_bytes(canonical_json_bytes(self.trace.model_dump(mode="json")))
        if actual_trace_sha256 != self.trace_sha256:
            raise ValueError("trace digest does not match artifact")
        _reject_secret_material(self.model_dump(mode="json"))
        return self


class BaselineBudgetEntry(EvalModel):
    schema_version: Literal[1] = 1
    sequence: int = Field(ge=1)
    previous_entry_sha256: Sha256 | None
    attempt_id: str
    status: Literal["known", "reserved_unknown"]
    cost_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    cumulative_cost_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    artifact_sha256: Sha256

    @model_validator(mode="after")
    def validate_budget_entry(self) -> "BaselineBudgetEntry":
        _validate_safe_ids(self.attempt_id)
        if self.sequence == 1 and self.previous_entry_sha256 is not None:
            raise ValueError("first budget entry must not have a previous link")
        if self.sequence > 1 and self.previous_entry_sha256 is None:
            raise ValueError("non-first budget entry requires a previous link")
        if self.cumulative_cost_cny < self.cost_cny:
            raise ValueError("cumulative cost cannot be less than entry cost")
        _reject_secret_material(self.model_dump(mode="json"))
        return self


class BaselineCampaignMetadata(EvalModel):
    schema_version: Literal[1] = 1
    campaign_id: str
    source_commit: GitSha1
    dirty_state_sha256: Sha256
    dataset_fingerprint: Sha256
    dataset_lock_sha256: Sha256
    harness_fingerprint: Sha256
    profile: HarnessProfileV1
    task_ids: list[str]
    repetitions: Literal[3]
    max_attempts: Literal[30]
    budget_cny: Decimal = Field(gt=0, allow_inf_nan=False)

    @field_validator("task_ids")
    @classmethod
    def validate_task_ids(cls, task_ids: list[str]) -> list[str]:
        if len(task_ids) != 10 or len(set(task_ids)) != len(task_ids):
            raise ValueError("baseline requires exactly 10 unique development task ids")
        for task_id in task_ids:
            _validate_safe_ids(task_id)
            if "holdout" in task_id:
                raise ValueError("baseline task ids must not contain holdout cases")
        if task_ids != sorted(task_ids):
            raise ValueError("baseline task ids must be sorted")
        return task_ids

    @model_validator(mode="after")
    def validate_campaign_metadata(self) -> "BaselineCampaignMetadata":
        _validate_safe_ids(self.campaign_id)
        if harness_profile_fingerprint(self.profile) != self.harness_fingerprint:
            raise ValueError("campaign profile fingerprint does not match")
        if self.profile.campaign_budget_cny != self.budget_cny:
            raise ValueError("campaign budget does not match profile")
        if self.profile.max_attempts != self.max_attempts:
            raise ValueError("campaign attempt count does not match profile")
        _reject_secret_material(self.model_dump(mode="json"))
        return self


class BaselineVerification(EvalModel):
    ok: bool
    attempt_count: int = Field(ge=0)
    pending_count: int = Field(ge=0)
    spent_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    blockers: list[str] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_verification(self) -> "BaselineVerification":
        if self.ok == bool(self.blockers):
            raise ValueError("verification ok must be the inverse of blockers")
        return self


class BaselineRunResult(EvalModel):
    status: Literal["complete", "paused", "blocked"]
    calls_started: int = Field(ge=0)
    terminal_attempts: int = Field(ge=0)
    spent_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    blockers: list[str] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_run_result(self) -> "BaselineRunResult":
        if self.status == "blocked" and not self.blockers:
            raise ValueError("blocked baseline result requires blockers")
        if self.status != "blocked" and self.blockers:
            raise ValueError("non-blocked baseline result cannot contain blockers")
        return self


class BaselineArtifactIndexEntry(EvalModel):
    path: str
    sha256: Sha256

    @field_validator("path")
    @classmethod
    def validate_path(cls, value: str) -> str:
        path = PurePosixPath(value)
        if (
            not value
            or path.is_absolute()
            or ".." in path.parts
            or path.parts[:1] != ("attempts",)
            or path.suffix != ".json"
        ):
            raise ValueError("artifact index path must be a logical attempt path")
        return value


class BaselineArtifactIndex(EvalModel):
    schema_version: Literal[1] = 1
    campaign_id: str
    artifacts: list[BaselineArtifactIndexEntry]

    @model_validator(mode="after")
    def validate_artifact_index(self) -> "BaselineArtifactIndex":
        _validate_safe_ids(self.campaign_id)
        paths = [artifact.path for artifact in self.artifacts]
        if paths != sorted(paths) or len(paths) != len(set(paths)):
            raise ValueError("artifact index paths must be sorted and unique")
        return self


class HarnessTelemetrySummary(EvalModel):
    """Bounded aggregate evidence emitted by the H3 harness contract."""

    attempts_with_contract: int = Field(default=0, ge=0)
    contract_coverage: float = Field(default=0.0, ge=0.0, le=1.0)
    median_tool_calls: float = Field(default=0.0, ge=0.0)
    p95_tool_calls: float = Field(default=0.0, ge=0.0)
    repeated_tool_call_rate: float = Field(default=0.0, ge=0.0, le=1.0)
    tool_call_histogram: dict[str, int] = Field(default_factory=dict)
    median_context_bytes: float = Field(default=0.0, ge=0.0)
    p95_context_bytes: float = Field(default=0.0, ge=0.0)
    median_context_tokens: float = Field(default=0.0, ge=0.0)
    p95_context_tokens: float = Field(default=0.0, ge=0.0)
    context_duplicate_rate: float = Field(default=0.0, ge=0.0, le=1.0)
    context_conflict_rate: float = Field(default=0.0, ge=0.0, le=1.0)
    context_bytes_by_tier: dict[str, int] = Field(default_factory=dict)
    context_items_by_tier: dict[str, int] = Field(default_factory=dict)
    stagnation_redirects: int = Field(default=0, ge=0)
    stagnation_stops: int = Field(default=0, ge=0)
    single_repair_compliance_rate: float = Field(default=0.0, ge=0.0, le=1.0)

    @model_validator(mode="after")
    def validate_histogram(self) -> "HarnessTelemetrySummary":
        if any(not key.isdigit() or value < 0 for key, value in self.tool_call_histogram.items()):
            raise ValueError("tool call histogram keys must be nonnegative integer strings")
        if any(value < 0 for value in self.context_bytes_by_tier.values()):
            raise ValueError("context bytes by tier must be nonnegative")
        if any(value < 0 for value in self.context_items_by_tier.values()):
            raise ValueError("context items by tier must be nonnegative")
        return self


class BaselineReport(EvalModel):
    schema_version: Literal[1] = 1
    campaign_id: str
    status: Literal["complete", "incomplete", "blocked"]
    source_commit: GitSha1
    dirty_state_sha256: Sha256
    dataset_fingerprint: Sha256
    harness_fingerprint: Sha256
    provider: Literal["deepseek"]
    model: Literal["deepseek-v4-flash"]
    transport: Literal["anthropic_messages", "openai_chat_completions"]
    expected_attempts: Literal[30]
    completed_attempts: int = Field(ge=0, le=30)
    trusted_attempts: int = Field(ge=0, le=30)
    trusted_success_rate: float = Field(ge=0, le=1)
    verification_pass_rate: float = Field(ge=0, le=1)
    scope_violation_rate: float = Field(ge=0, le=1)
    median_model_rounds: float = Field(ge=0)
    p95_model_rounds: float = Field(ge=0)
    median_cost_cny: Decimal | None = Field(default=None, ge=0, allow_inf_nan=False)
    p95_cost_cny: Decimal | None = Field(default=None, ge=0, allow_inf_nan=False)
    total_cost_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    unknown_cost_attempts: int = Field(ge=0, le=30)
    failure_categories: dict[str, int]
    per_task: dict[str, dict[str, int | float | bool]]
    trust_blockers: dict[str, int]
    attempt_artifact_index_sha256: Sha256
    harness_telemetry: HarnessTelemetrySummary = Field(
        default_factory=lambda: HarnessTelemetrySummary()
    )

    @model_validator(mode="after")
    def validate_report(self) -> "BaselineReport":
        _validate_safe_ids(self.campaign_id)
        if self.trusted_attempts > self.completed_attempts:
            raise ValueError("trusted attempts cannot exceed completed attempts")
        if self.unknown_cost_attempts > self.completed_attempts:
            raise ValueError("unknown-cost attempts cannot exceed completed attempts")
        if (self.median_cost_cny is None) != (self.p95_cost_cny is None):
            raise ValueError("cost percentiles must both be known or both be null")
        if self.status == "complete" and (
            self.completed_attempts != self.expected_attempts
            or self.trusted_attempts != self.completed_attempts
            or self.trust_blockers
        ):
            raise ValueError("complete report requires all attempts to be trusted")
        if self.status == "incomplete" and (
            self.completed_attempts >= self.expected_attempts or self.trust_blockers
        ):
            raise ValueError("incomplete report must be unblocked and unfinished")
        if self.status == "blocked" and not self.trust_blockers:
            raise ValueError("blocked report requires trust blockers")
        if any(not key.strip() or value < 0 for key, value in self.failure_categories.items()):
            raise ValueError("failure category counts must be nonnegative")
        if any(not key.strip() or value < 1 for key, value in self.trust_blockers.items()):
            raise ValueError("trust blocker counts must be positive")
        if self.harness_telemetry.attempts_with_contract > self.completed_attempts:
            raise ValueError("harness telemetry coverage cannot exceed completed attempts")
        expected_coverage = (
            self.harness_telemetry.attempts_with_contract / self.completed_attempts
            if self.completed_attempts
            else 0.0
        )
        if abs(self.harness_telemetry.contract_coverage - expected_coverage) > 1e-12:
            raise ValueError("harness telemetry coverage does not match attempt counts")
        _reject_secret_material(self.model_dump(mode="json"))
        return self


class BaselineComparisonMetrics(EvalModel):
    """Aggregate metrics for one independently verified baseline arm."""

    attempts: Literal[30] = 30
    trusted_attempts: int = Field(ge=0, le=30)
    trusted_success_rate: float = Field(ge=0, le=1)
    verification_pass_rate: float = Field(ge=0, le=1)
    scope_violation_rate: float = Field(ge=0, le=1)
    median_model_rounds: float = Field(ge=0)
    p95_model_rounds: float = Field(ge=0)
    median_cost_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    p95_cost_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    median_duration_ms: float = Field(ge=0)
    p95_duration_ms: float = Field(ge=0)
    failure_categories: dict[str, int]
    provider_errors: dict[str, int]
    trust_blockers: dict[str, int]
    harness_telemetry: HarnessTelemetrySummary = Field(
        default_factory=lambda: HarnessTelemetrySummary()
    )

    @model_validator(mode="after")
    def validate_comparison_metrics(self) -> "BaselineComparisonMetrics":
        if any(not key.strip() or value < 0 for key, value in self.failure_categories.items()):
            raise ValueError("comparison failure category counts must be nonnegative")
        if any(not key.strip() or value < 0 for key, value in self.provider_errors.items()):
            raise ValueError("comparison provider error counts must be nonnegative")
        if any(not key.strip() or value < 1 for key, value in self.trust_blockers.items()):
            raise ValueError("comparison trust blocker counts must be positive")
        if self.harness_telemetry.attempts_with_contract > self.attempts:
            raise ValueError("comparison telemetry coverage cannot exceed attempts")
        expected_coverage = self.harness_telemetry.attempts_with_contract / self.attempts
        if abs(self.harness_telemetry.contract_coverage - expected_coverage) > 1e-12:
            raise ValueError("comparison telemetry coverage does not match attempt counts")
        _reject_secret_material(self.model_dump(mode="json"))
        return self


class BootstrapInterval(EvalModel):
    """Deterministic paired bootstrap interval for one comparison delta."""

    estimate: float
    low: float
    high: float
    method: Literal["paired_bootstrap"] = "paired_bootstrap"
    resamples: int = Field(ge=1)
    seed: int = Field(ge=0)

    @model_validator(mode="after")
    def validate_interval(self) -> "BootstrapInterval":
        if self.low > self.high:
            raise ValueError("bootstrap interval low bound cannot exceed high bound")
        if not self.low <= self.estimate <= self.high:
            raise ValueError("bootstrap interval must contain its point estimate")
        return self


class BaselineComparison(EvalModel):
    """Fail-closed paired comparison of two complete 30-attempt campaigns."""

    schema_version: Literal[1] = 1
    control_campaign_id: str
    candidate_campaign_id: str
    dataset_fingerprint: Sha256
    model: Literal["deepseek-v4-flash"]
    control_transport: Literal["anthropic_messages"]
    candidate_transport: Literal["openai_chat_completions"]
    paired_attempts: Literal[30] = 30
    outcome_changes: dict[str, str]
    control_metrics: BaselineComparisonMetrics
    candidate_metrics: BaselineComparisonMetrics
    winner: Literal["control", "candidate", "no_material_difference"]
    blockers: list[str] = Field(default_factory=list)
    control_profile_id: str | None = None
    candidate_profile_id: str | None = None
    control_harness_fingerprint: Sha256 | None = None
    candidate_harness_fingerprint: Sha256 | None = None
    source_commit: GitSha1 | None = None
    bootstrap_intervals: dict[str, BootstrapInterval] = Field(default_factory=dict)

    @model_validator(mode="after")
    def validate_comparison(self) -> "BaselineComparison":
        _validate_safe_ids(self.control_campaign_id, self.candidate_campaign_id)
        if self.control_campaign_id == self.candidate_campaign_id:
            raise ValueError("comparison campaigns must be distinct")
        if self.control_metrics.trusted_attempts != self.paired_attempts:
            raise ValueError("control campaign must have all attempts trusted")
        if self.candidate_metrics.trusted_attempts != self.paired_attempts:
            raise ValueError("candidate campaign must have all attempts trusted")
        if any(not key.strip() or not value.strip() for key, value in self.outcome_changes.items()):
            raise ValueError("comparison outcomes must contain nonblank keys and values")
        if any(not key.strip() for key in self.bootstrap_intervals):
            raise ValueError("bootstrap interval names must be nonblank")
        _reject_secret_material(self.model_dump(mode="json"))
        return self


def baseline_entry_digest(entry: BaselineBudgetEntry) -> str:
    return sha256_bytes(canonical_json_bytes(entry.model_dump(mode="json")))


def baseline_artifact_digest(artifact: BaselineAttemptArtifact) -> str:
    return sha256_bytes(canonical_json_bytes(artifact.model_dump(mode="json")))


def _validate_safe_ids(*values: str) -> None:
    for value in values:
        if not value.strip() or SAFE_ID_PATTERN.fullmatch(value) is None:
            raise ValueError("baseline identifiers must be nonblank safe identifiers")


def _reject_secret_material(value: Any) -> None:
    serialized = json.dumps(value, sort_keys=True, default=str)
    if SECRET_PATTERN.search(serialized) is not None:
        raise ValueError("baseline artifact contains secret-like material")
