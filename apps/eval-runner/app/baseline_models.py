import json
from datetime import datetime
from decimal import Decimal
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
        actual_trace_sha256 = sha256_bytes(
            canonical_json_bytes(self.trace.model_dump(mode="json"))
        )
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
