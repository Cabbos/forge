"""Fail-closed release gate for the frozen ForgeBench holdout split."""

from __future__ import annotations

import json
import re
from collections.abc import Sequence
from decimal import Decimal
from pathlib import Path
from statistics import median
from typing import Literal

from pydantic import Field, model_validator

from app.models import EvalModel
from app.provenance import canonical_json_bytes, sha256_bytes
from app.provenance_models import GitSha1, Sha256

_SAFE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")


class HoldoutGateThresholdsV1(EvalModel):
    """The immutable release thresholds from the frontier harness design."""

    schema_version: Literal[1] = 1
    expected_case_count: Literal[20] = 20
    repetitions: Literal[3] = 3
    expected_attempts: Literal[60] = 60
    minimum_trusted_success_rate: float = Field(default=0.80, ge=0.80, le=1.0)
    maximum_scope_violation_rate: float = Field(default=0.02, ge=0.0, le=0.02)
    maximum_median_model_rounds: float = Field(default=12.0, ge=0.0, le=12.0)
    minimum_score_coverage: float = Field(default=1.0, ge=1.0, le=1.0)
    minimum_contract_coverage: float = Field(default=1.0, ge=1.0, le=1.0)
    minimum_workspace_evidence_rate: float = Field(default=1.0, ge=1.0, le=1.0)
    minimum_patch_replay_rate: float = Field(default=1.0, ge=1.0, le=1.0)
    minimum_verification_evidence_rate: float = Field(default=1.0, ge=1.0, le=1.0)
    minimum_usage_known_rate: float = Field(default=1.0, ge=1.0, le=1.0)
    minimum_terminal_failure_category_rate: float = Field(default=1.0, ge=1.0, le=1.0)
    maximum_budget_cny: Decimal = Field(default=Decimal("35"), ge=0, allow_inf_nan=False)

    @model_validator(mode="after")
    def validate_locked_values(self) -> HoldoutGateThresholdsV1:
        locked = {
            "expected_case_count": 20,
            "repetitions": 3,
            "expected_attempts": 60,
            "minimum_trusted_success_rate": 0.80,
            "maximum_scope_violation_rate": 0.02,
            "maximum_median_model_rounds": 12.0,
            "minimum_score_coverage": 1.0,
            "minimum_contract_coverage": 1.0,
            "minimum_workspace_evidence_rate": 1.0,
            "minimum_patch_replay_rate": 1.0,
            "minimum_verification_evidence_rate": 1.0,
            "minimum_usage_known_rate": 1.0,
            "minimum_terminal_failure_category_rate": 1.0,
            "maximum_budget_cny": Decimal("35"),
        }
        for field_name, expected in locked.items():
            if getattr(self, field_name) != expected:
                raise ValueError(f"holdout threshold is not locked: {field_name}")
        return self


DEFAULT_HOLDOUT_THRESHOLDS = HoldoutGateThresholdsV1()


class HoldoutAttemptEvidenceV1(EvalModel):
    """One immutable, already-observed holdout attempt."""

    schema_version: Literal[1] = 1
    attempt_id: str
    case_id: str
    repetition: Literal[1, 2, 3]
    trusted: bool
    passed: bool
    scope_violation: bool
    model_rounds: int = Field(ge=0)
    score_coverage: bool
    contract_evidence: bool
    workspace_evidence: bool
    patch_replay: bool
    verification_evidence: bool
    usage_known: bool
    terminal_failure_category: str | None
    cost_cny: Decimal = Field(ge=0, allow_inf_nan=False)

    @model_validator(mode="after")
    def validate_identity(self) -> HoldoutAttemptEvidenceV1:
        if not _SAFE_ID.fullmatch(self.attempt_id) or not _SAFE_ID.fullmatch(self.case_id):
            raise ValueError("holdout attempt and case ids must be safe identifiers")
        if (
            self.terminal_failure_category is not None
            and not self.terminal_failure_category.strip()
        ):
            raise ValueError("terminal failure category must be nonblank when present")
        return self


class HoldoutGateReportV1(EvalModel):
    """Canonical summary emitted only after all 60 holdout attempts are closed."""

    schema_version: Literal[1] = 1
    campaign_id: str
    dataset_split: Literal["holdout"]
    dataset_fingerprint: Sha256
    harness_fingerprint: Sha256
    profile_id: Literal[
        "deepseek-v4-flash-anthropic-baseline-v1",
        "deepseek-v4-flash-openai-chat-control-v1",
        "deepseek-v4-flash-anthropic-minimal-v1",
        "deepseek-v4-flash-openai-chat-minimal-v1",
    ]
    source_commit: GitSha1
    price_table_fingerprint: Sha256
    provider: Literal["deepseek"]
    model: Literal["deepseek-v4-flash"]
    transport: Literal["anthropic_messages", "openai_chat_completions"]
    case_count: Literal[20] = 20
    repetitions: Literal[3] = 3
    expected_attempts: Literal[60] = 60
    completed_attempts: int = Field(ge=0, le=60)
    trusted_attempts: int = Field(ge=0, le=60)
    trusted_success_rate: float = Field(ge=0, le=1)
    scope_violation_rate: float = Field(ge=0, le=1)
    median_model_rounds: float = Field(ge=0)
    score_coverage: float = Field(ge=0, le=1)
    contract_coverage: float = Field(ge=0, le=1)
    workspace_evidence_rate: float = Field(ge=0, le=1)
    patch_replay_rate: float = Field(ge=0, le=1)
    verification_evidence_rate: float = Field(ge=0, le=1)
    usage_known_rate: float = Field(ge=0, le=1)
    terminal_failure_category_rate: float = Field(ge=0, le=1)
    total_cost_cny: Decimal = Field(ge=0, allow_inf_nan=False)
    budget_cny: Decimal = Field(gt=0, allow_inf_nan=False)
    status: Literal["complete", "incomplete", "blocked"]
    blockers: list[str] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_shape(self) -> HoldoutGateReportV1:
        if not _SAFE_ID.fullmatch(self.campaign_id):
            raise ValueError("holdout campaign id must be a safe identifier")
        if self.trusted_attempts > self.completed_attempts:
            raise ValueError("trusted attempts cannot exceed completed attempts")
        if any(not blocker.strip() for blocker in self.blockers):
            raise ValueError("holdout blockers must be nonblank")
        return self


class HoldoutGateDecisionV1(EvalModel):
    schema_version: Literal[1] = 1
    campaign_id: str
    passed: bool
    blockers: list[str] = Field(default_factory=list)
    thresholds: HoldoutGateThresholdsV1 = DEFAULT_HOLDOUT_THRESHOLDS

    @model_validator(mode="after")
    def validate_decision(self) -> HoldoutGateDecisionV1:
        if self.passed != (not self.blockers):
            raise ValueError("holdout pass status must match blockers")
        return self


def evaluate_holdout_gate(
    report: HoldoutGateReportV1,
    thresholds: HoldoutGateThresholdsV1 = DEFAULT_HOLDOUT_THRESHOLDS,
) -> HoldoutGateDecisionV1:
    """Evaluate release thresholds without inferring missing evidence."""

    blockers = list(report.blockers)
    checks = (
        (report.status != "complete", "holdout_status_not_complete"),
        (
            report.completed_attempts != thresholds.expected_attempts,
            "holdout_attempt_count_incomplete",
        ),
        (
            report.trusted_attempts != report.completed_attempts,
            "holdout_trusted_attempts_incomplete",
        ),
        (
            report.trusted_success_rate < thresholds.minimum_trusted_success_rate,
            "holdout_success_rate_below_threshold",
        ),
        (
            report.scope_violation_rate > thresholds.maximum_scope_violation_rate,
            "holdout_scope_rate_above_threshold",
        ),
        (
            report.median_model_rounds > thresholds.maximum_median_model_rounds,
            "holdout_model_rounds_above_threshold",
        ),
        (
            report.score_coverage < thresholds.minimum_score_coverage,
            "holdout_score_coverage_incomplete",
        ),
        (
            report.contract_coverage < thresholds.minimum_contract_coverage,
            "holdout_contract_coverage_incomplete",
        ),
        (
            report.workspace_evidence_rate < thresholds.minimum_workspace_evidence_rate,
            "holdout_workspace_evidence_incomplete",
        ),
        (
            report.patch_replay_rate < thresholds.minimum_patch_replay_rate,
            "holdout_patch_replay_incomplete",
        ),
        (
            report.verification_evidence_rate < thresholds.minimum_verification_evidence_rate,
            "holdout_verification_evidence_incomplete",
        ),
        (
            report.usage_known_rate < thresholds.minimum_usage_known_rate,
            "holdout_usage_accounting_incomplete",
        ),
        (
            report.terminal_failure_category_rate
            < thresholds.minimum_terminal_failure_category_rate,
            "holdout_failure_taxonomy_incomplete",
        ),
        (
            report.budget_cny > thresholds.maximum_budget_cny,
            "holdout_budget_declaration_exceeds_cap",
        ),
        (report.total_cost_cny > thresholds.maximum_budget_cny, "holdout_budget_exceeded"),
    )
    blockers.extend(reason for failed, reason in checks if failed and reason not in blockers)
    return HoldoutGateDecisionV1(
        campaign_id=report.campaign_id,
        passed=not blockers,
        blockers=sorted(set(blockers)),
        thresholds=thresholds,
    )


def holdout_report_bytes(report: HoldoutGateReportV1) -> bytes:
    return canonical_json_bytes(report.model_dump(mode="json")) + b"\n"


def holdout_report_fingerprint(report: HoldoutGateReportV1) -> str:
    return sha256_bytes(holdout_report_bytes(report))


def summarize_holdout_attempts(
    attempts: Sequence[HoldoutAttemptEvidenceV1],
    *,
    campaign_id: str,
    case_ids: Sequence[str],
    dataset_fingerprint: str,
    harness_fingerprint: str,
    profile_id: str,
    source_commit: str,
    price_table_fingerprint: str,
    transport: Literal["anthropic_messages", "openai_chat_completions"],
    budget_cny: Decimal,
) -> HoldoutGateReportV1:
    """Derive a holdout report from every observed case/repetition pair."""

    expected_cases = set(case_ids)
    if len(expected_cases) != DEFAULT_HOLDOUT_THRESHOLDS.expected_case_count:
        raise ValueError("holdout case id list must contain exactly 20 unique cases")
    if len(attempts) != DEFAULT_HOLDOUT_THRESHOLDS.expected_attempts:
        raise ValueError("holdout attempt list must contain exactly 60 attempts")
    observed_keys = [(attempt.repetition, attempt.case_id) for attempt in attempts]
    expected_keys = [
        (repetition, case_id)
        for repetition in range(1, DEFAULT_HOLDOUT_THRESHOLDS.repetitions + 1)
        for case_id in sorted(expected_cases)
    ]
    if len(set(observed_keys)) != len(observed_keys) or set(observed_keys) != set(expected_keys):
        raise ValueError("holdout attempts must contain each unique case/repetition pair")
    if len({attempt.attempt_id for attempt in attempts}) != len(attempts):
        raise ValueError("holdout attempt ids must be unique")

    total = len(attempts)
    trusted_successes = sum(attempt.trusted and attempt.passed for attempt in attempts)
    return HoldoutGateReportV1(
        campaign_id=campaign_id,
        dataset_split="holdout",
        dataset_fingerprint=dataset_fingerprint,
        harness_fingerprint=harness_fingerprint,
        profile_id=profile_id,
        source_commit=source_commit,
        price_table_fingerprint=price_table_fingerprint,
        provider="deepseek",
        model="deepseek-v4-flash",
        transport=transport,
        completed_attempts=total,
        trusted_attempts=sum(attempt.trusted for attempt in attempts),
        trusted_success_rate=trusted_successes / total,
        scope_violation_rate=sum(attempt.scope_violation for attempt in attempts) / total,
        median_model_rounds=float(median(attempt.model_rounds for attempt in attempts)),
        score_coverage=sum(attempt.score_coverage for attempt in attempts) / total,
        contract_coverage=sum(attempt.contract_evidence for attempt in attempts) / total,
        workspace_evidence_rate=sum(attempt.workspace_evidence for attempt in attempts) / total,
        patch_replay_rate=sum(attempt.patch_replay for attempt in attempts) / total,
        verification_evidence_rate=sum(attempt.verification_evidence for attempt in attempts)
        / total,
        usage_known_rate=sum(attempt.usage_known for attempt in attempts) / total,
        terminal_failure_category_rate=sum(
            attempt.terminal_failure_category is not None for attempt in attempts
        )
        / total,
        total_cost_cny=sum((attempt.cost_cny for attempt in attempts), Decimal(0)),
        budget_cny=budget_cny,
        status="complete",
    )


def load_holdout_report(path: Path) -> HoldoutGateReportV1:
    raw = path.read_bytes()
    try:
        report = HoldoutGateReportV1.model_validate(json.loads(raw))
    except (UnicodeDecodeError, json.JSONDecodeError, TypeError, ValueError) as exc:
        raise ValueError("holdout report is invalid") from exc
    if raw != holdout_report_bytes(report):
        raise ValueError("holdout report is not canonical")
    return report
