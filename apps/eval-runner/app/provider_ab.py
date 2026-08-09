"""Diagnostic paired comparison for the approved Kimi and DeepSeek eval routes.

This module deliberately sits outside the frozen H1/H2 baseline campaign models.
It is for development evidence only: each arm is a single provider, fallback
events invalidate the pair, and an untrusted arm can never produce a winner.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from random import Random
from typing import Literal

from pydantic import Field, field_validator, model_validator

from app.cases import CaseLoadError, expand_prompt_mutations, load_cases
from app.config import get_settings
from app.datasets import dataset_fingerprint
from app.execution import ExecutionOptions, execute_evaluation
from app.execution_identity import default_execution_identity, validate_execution_identity_event
from app.models import AgentTrace, BacktestReport, EvalModel, EvalProvider
from app.process_control import CancelRequested, never_cancelled
from app.provenance import canonical_json_bytes, sha256_bytes
from app.provenance_models import SECRET_PATTERN, GitSha1, Sha256
from app.provider_fallback import ProviderRouteSpec
from app.red_team import is_red_team_task
from app.scoring import UNIVERSAL_SCORE_NAMES, score_trace

_BOOTSTRAP_RESAMPLES = 2_000
_BOOTSTRAP_SEED = 814_2026

APPROVED_PROVIDER_AB_ROUTES: dict[str, ProviderRouteSpec] = {
    "deepseek_v4_flash": ProviderRouteSpec(
        provider="deepseek",
        model="deepseek-v4-flash",
        transport="anthropic_messages",
        endpoint_family="anthropic",
        base_url="https://api.deepseek.com/anthropic",
        api_key_env="DEEPSEEK_API_KEY",
    ),
    "kimi_k3": ProviderRouteSpec(
        provider="custom_openai",
        model="k3",
        transport="openai_chat_completions",
        endpoint_family="openai",
        base_url="https://api.kimi.com/coding/v1",
        api_key_env="FORGE_CUSTOM_OPENAI_API_KEY",
    ),
}


class ProviderABError(ValueError):
    """Raised when a paired comparison cannot be trusted."""


@dataclass(frozen=True)
class ProviderABArm:
    arm_id: str
    dataset_fingerprint: str
    traces: tuple[AgentTrace, ...]
    report: BacktestReport


class ProviderABRunContract(EvalModel):
    """Controller parameters that define one paid paired experiment."""

    trials: int = Field(ge=1)
    # New callers default to two so a single paired observation cannot be
    # reported as intelligence superiority (or regression) on its own. The
    # artifact loader explicitly injects ``None`` for pre-contract payloads.
    minimum_paired_attempts: int | None = Field(default=2, ge=2)
    max_total_cost_usd: float | None = Field(default=None, gt=0.0)
    require_known_cost: bool = False
    max_total_runtime_seconds: float | None = Field(default=None, gt=0.0)
    include_red_team: bool = False
    red_team_only: bool = False
    require_red_team: bool = False

    @model_validator(mode="after")
    def validate_selection(self) -> ProviderABRunContract:
        if self.include_red_team and self.red_team_only:
            raise ValueError("provider A/B run contract cannot include and select red-team-only")
        return self


class ProviderABInterval(EvalModel):
    estimate: float
    low: float
    high: float
    resamples: int = Field(ge=1)
    seed: int = Field(ge=0)

    @model_validator(mode="after")
    def validate_interval(self) -> ProviderABInterval:
        if self.low > self.high or not self.low <= self.estimate <= self.high:
            raise ValueError("provider A/B interval must contain estimate")
        return self


class ProviderABArmSummary(EvalModel):
    arm_id: str
    provider: str
    model: str
    transport: Literal["anthropic_messages", "openai_chat_completions"]
    endpoint_family: Literal["anthropic", "openai"]
    base_url: str
    attempts: int = Field(ge=1)
    trusted: bool
    trust_blockers: list[str] = Field(default_factory=list)
    success_rate: float = Field(ge=0.0, le=1.0)
    verification_pass_rate: float = Field(ge=0.0, le=1.0)
    scope_violation_rate: float = Field(ge=0.0, le=1.0)
    avg_model_rounds: float = Field(ge=0.0)
    avg_duration_ms: float = Field(ge=0.0)
    total_cost_usd: float = Field(ge=0.0)
    provider_cost_known_rate: float = Field(ge=0.0, le=1.0)
    score_summary: dict[str, float] = Field(default_factory=dict)
    # Populated for contract-bound runs. An empty mapping keeps artifacts
    # written before the distribution contract backward-compatible.
    distribution_summary: dict[str, float] = Field(default_factory=dict)
    failure_categories: dict[str, int] = Field(default_factory=dict)

    @field_validator("arm_id", "provider", "model", "base_url")
    @classmethod
    def require_non_empty(cls, value: str) -> str:
        if not value.strip():
            raise ValueError("provider A/B identity fields must be non-empty")
        return value


class ProviderABComparison(EvalModel):
    schema_version: Literal[1] = 1
    mode: Literal["provider_ab_diagnostic_v1"] = "provider_ab_diagnostic_v1"
    control_arm_id: str
    candidate_arm_id: str
    dataset_fingerprint: Sha256
    source_commit: GitSha1 = "0" * 40
    dirty_state_sha256: Sha256 = "0" * 64
    run_contract: ProviderABRunContract | None = None
    task_ids: list[str] = Field(default_factory=list)
    paired_attempts: int = Field(ge=1)
    control: ProviderABArmSummary
    candidate: ProviderABArmSummary
    outcome_changes: dict[
        str,
        Literal[
            "success->success",
            "success->failure",
            "failure->success",
            "failure->failure",
        ],
    ]
    deltas: dict[str, float]
    failure_category_deltas: dict[str, int] = Field(default_factory=dict)
    bootstrap_intervals: dict[str, ProviderABInterval]
    winner: Literal["control", "candidate", "no_material_difference", "blocked"]
    blockers: list[str] = Field(default_factory=list)
    warnings: list[str] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_comparison(self) -> ProviderABComparison:
        if self.control_arm_id == self.candidate_arm_id:
            raise ValueError("provider A/B arms must be distinct")
        if self.paired_attempts != len(self.outcome_changes):
            raise ValueError("provider A/B outcome count must match paired attempts")
        if self.task_ids and self.task_ids != sorted(self.task_ids):
            raise ValueError("provider A/B task ids must be sorted")
        if len(self.task_ids) != len(set(self.task_ids)):
            raise ValueError("provider A/B task ids must be unique")
        if self.winner == "blocked" and not self.blockers:
            raise ValueError("blocked provider A/B comparison requires blockers")
        if self.winner != "blocked" and self.blockers:
            raise ValueError("unblocked provider A/B comparison cannot contain blockers")
        return self


@dataclass(frozen=True)
class ProviderABExecution:
    control: ProviderABArm
    candidate: ProviderABArm
    comparison: ProviderABComparison


def run_provider_ab(
    *,
    cases_path: Path,
    forge_command: str | None,
    trials: int = 1,
    include_red_team: bool = False,
    red_team_only: bool = False,
    require_red_team: bool = False,
    max_total_cost_usd: float | None = None,
    require_known_cost: bool = False,
    max_total_runtime_seconds: float | None = None,
    cancel_requested: CancelRequested = never_cancelled,
    repository_root: Path | None = None,
) -> ProviderABExecution:
    """Run route-pure DeepSeek control and Kimi candidate arms over one dataset."""

    if trials < 1:
        raise ProviderABError("trials_must_be_positive")
    if max_total_runtime_seconds is not None and max_total_runtime_seconds <= 0:
        raise ProviderABError("max_total_runtime_seconds_must_be_positive")
    deadline = (
        time.monotonic() + max_total_runtime_seconds
        if max_total_runtime_seconds is not None
        else None
    )

    def should_cancel() -> bool:
        return cancel_requested() or (deadline is not None and time.monotonic() >= deadline)

    tasks = load_cases(cases_path)
    if red_team_only:
        tasks = [task for task in tasks if is_red_team_task(task)]
    elif not include_red_team:
        tasks = [task for task in tasks if not is_red_team_task(task)]
    tasks = expand_prompt_mutations(tasks, styles=[], mutations_only=False)
    if not tasks:
        raise ProviderABError("no_tasks_selected")
    trial_tasks = [task for _ in range(trials) for task in tasks]
    fingerprint = dataset_fingerprint(tasks)
    source_commit, dirty_state_sha256 = _git_source_identity(
        (repository_root or Path(__file__).resolve().parents[2]).resolve()
    )
    run_contract = ProviderABRunContract(
        trials=trials,
        minimum_paired_attempts=2,
        max_total_cost_usd=max_total_cost_usd,
        require_known_cost=require_known_cost,
        max_total_runtime_seconds=max_total_runtime_seconds,
        include_red_team=include_red_team,
        red_team_only=red_team_only,
        require_red_team=require_red_team,
    )
    settings = get_settings()

    arms: list[ProviderABArm] = []
    for arm_id in ("deepseek_v4_flash", "kimi_k3"):
        route = APPROVED_PROVIDER_AB_ROUTES[arm_id]
        execution = execute_evaluation(
            cases_path=cases_path,
            tasks=trial_tasks,
            options=ExecutionOptions(
                provider=EvalProvider.FORGE,
                model=route.model,
                forge_command=forge_command,
                command_timeout_seconds=settings.command_timeout_seconds,
                setup_timeout_seconds=settings.setup_timeout_seconds,
                validation_timeout_seconds=settings.validation_timeout_seconds,
                require_red_team=require_red_team,
                provider_route=route,
                max_total_cost_usd=max_total_cost_usd,
                require_known_cost=require_known_cost,
            ),
            cancel_requested=should_cancel,
        )
        if len(execution.traces) != len(trial_tasks):
            raise ProviderABError(
                f"arm_incomplete:{arm_id}:{len(execution.traces)}/{len(trial_tasks)}"
            )
        arms.append(
            ProviderABArm(
                arm_id=arm_id,
                dataset_fingerprint=fingerprint,
                traces=tuple(execution.traces),
                report=execution.report,
            )
        )
    control, candidate = arms
    return ProviderABExecution(
        control=control,
        candidate=candidate,
        comparison=compare_provider_ab(
            control=control,
            candidate=candidate,
            source_commit=source_commit,
            dirty_state_sha256=dirty_state_sha256,
            run_contract=run_contract,
            task_ids=[task.id for task in tasks],
        ),
    )


def write_provider_ab_artifact(
    output_path: Path,
    *,
    comparison: ProviderABComparison,
    control: ProviderABArm,
    candidate: ProviderABArm,
) -> str:
    """Write canonical paired evidence and its digest sidecar."""

    if comparison.source_commit == "0" * 40 or comparison.dirty_state_sha256 == "0" * 64:
        raise ProviderABError("source_provenance_missing")
    try:
        recomputed = compare_provider_ab(
            control=control,
            candidate=candidate,
            source_commit=comparison.source_commit,
            dirty_state_sha256=comparison.dirty_state_sha256,
            run_contract=comparison.run_contract,
            task_ids=comparison.task_ids,
        )
    except ProviderABError as exc:
        raise ProviderABError(f"comparison_invalid:{exc}") from exc
    if recomputed != comparison:
        raise ProviderABError("comparison_mismatch")

    payload = {
        "schema_version": 1,
        "kind": "provider_ab_diagnostic",
        "comparison": comparison.model_dump(mode="json"),
        "control": {
            "arm_id": control.arm_id,
            "dataset_fingerprint": control.dataset_fingerprint,
            "report": control.report.model_dump(mode="json"),
            "traces": [trace.model_dump(mode="json") for trace in control.traces],
        },
        "candidate": {
            "arm_id": candidate.arm_id,
            "dataset_fingerprint": candidate.dataset_fingerprint,
            "report": candidate.report.model_dump(mode="json"),
            "traces": [trace.model_dump(mode="json") for trace in candidate.traces],
        },
    }
    raw = canonical_json_bytes(payload) + b"\n"
    if SECRET_PATTERN.search(raw.decode("utf-8")) is not None:
        raise ProviderABError("artifact_secret_detected")
    digest = sha256_bytes(raw)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_bytes(raw)
    output_path.with_suffix(output_path.suffix + ".sha256").write_text(
        digest + "\n", encoding="utf-8"
    )
    return digest


def load_provider_ab_artifact(output_path: Path) -> ProviderABExecution:
    """Verify and load a provider A/B artifact without making provider calls."""

    try:
        raw = output_path.read_bytes()
        sidecar = output_path.with_suffix(output_path.suffix + ".sha256")
        sidecar_digest = sidecar.read_text(encoding="utf-8").strip()
    except OSError as exc:
        raise ProviderABError("artifact_read_failed") from exc
    actual_digest = sha256_bytes(raw)
    if sidecar_digest != actual_digest:
        raise ProviderABError("artifact_digest_mismatch")
    if SECRET_PATTERN.search(raw.decode("utf-8", errors="replace")) is not None:
        raise ProviderABError("artifact_secret_detected")
    try:
        payload = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ProviderABError("artifact_json_invalid") from exc
    if canonical_json_bytes(payload) + b"\n" != raw:
        raise ProviderABError("artifact_not_canonical")
    if not isinstance(payload, dict) or set(payload) != {
        "schema_version",
        "kind",
        "comparison",
        "control",
        "candidate",
    }:
        raise ProviderABError("artifact_schema_invalid")
    try:
        if payload["schema_version"] != 1 or payload["kind"] != "provider_ab_diagnostic":
            raise ProviderABError("artifact_schema_invalid")
        comparison_payload = payload["comparison"]
        if not isinstance(comparison_payload, dict):
            raise ProviderABError("artifact_schema_invalid")
        # Schema v1 artifacts emitted before the paired-attempt contract did
        # not carry this field. Preserve their legacy diagnostic semantics
        # while keeping direct/new contract construction fail-closed.
        legacy_contract = comparison_payload.get("run_contract")
        if isinstance(legacy_contract, dict) and "minimum_paired_attempts" not in legacy_contract:
            comparison_payload = {
                **comparison_payload,
                "run_contract": {**legacy_contract, "minimum_paired_attempts": None},
            }
        comparison = ProviderABComparison.model_validate(comparison_payload)
        control_payload = payload["control"]
        candidate_payload = payload["candidate"]
        if not isinstance(control_payload, dict) or not isinstance(candidate_payload, dict):
            raise ProviderABError("artifact_schema_invalid")
        if set(control_payload) != {"arm_id", "dataset_fingerprint", "report", "traces"}:
            raise ProviderABError("artifact_schema_invalid")
        if set(candidate_payload) != {"arm_id", "dataset_fingerprint", "report", "traces"}:
            raise ProviderABError("artifact_schema_invalid")
        control_traces = tuple(
            AgentTrace.model_validate(item) for item in control_payload["traces"]
        )
        candidate_traces = tuple(
            AgentTrace.model_validate(item) for item in candidate_payload["traces"]
        )
        control = ProviderABArm(
            arm_id=control_payload["arm_id"],
            dataset_fingerprint=control_payload["dataset_fingerprint"],
            traces=control_traces,
            report=BacktestReport.model_validate(control_payload["report"]),
        )
        candidate = ProviderABArm(
            arm_id=candidate_payload["arm_id"],
            dataset_fingerprint=candidate_payload["dataset_fingerprint"],
            traces=candidate_traces,
            report=BacktestReport.model_validate(candidate_payload["report"]),
        )
    except ProviderABError:
        raise
    except (KeyError, TypeError, ValueError) as exc:
        raise ProviderABError("artifact_schema_invalid") from exc
    if comparison.source_commit == "0" * 40 or comparison.dirty_state_sha256 == "0" * 64:
        raise ProviderABError("source_provenance_missing")
    try:
        recomputed = compare_provider_ab(
            control=control,
            candidate=candidate,
            source_commit=comparison.source_commit,
            dirty_state_sha256=comparison.dirty_state_sha256,
            run_contract=comparison.run_contract,
            task_ids=comparison.task_ids,
        )
    except ProviderABError as exc:
        raise ProviderABError(f"artifact_comparison_invalid:{exc}") from exc
    if recomputed != comparison:
        raise ProviderABError("artifact_comparison_mismatch")
    return ProviderABExecution(control=control, candidate=candidate, comparison=comparison)


def provider_ab_verify_main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Verify a provider A/B artifact without making provider calls."
    )
    parser.add_argument("--artifact", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        result = load_provider_ab_artifact(args.artifact)
        digest = sha256_bytes(args.artifact.read_bytes())
    except (OSError, ValueError) as exc:
        print(f"error: provider A/B artifact verification failed: {exc}")
        return 1
    print(
        json.dumps(
            {
                "ok": True,
                "artifact": str(args.artifact),
                "artifact_sha256": digest,
                "comparison": result.comparison.model_dump(mode="json"),
            },
            indent=2,
        )
    )
    return 0


def provider_ab_main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Run a route-pure DeepSeek control versus Kimi k3 diagnostic comparison."
    )
    parser.add_argument("--cases", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--trials", type=int, default=1)
    parser.add_argument("--include-red-team", action="store_true")
    parser.add_argument("--red-team-only", action="store_true")
    parser.add_argument("--require-red-team", action="store_true")
    parser.add_argument(
        "--max-total-cost-usd",
        type=float,
        default=None,
        help="Maximum cost per arm; partial arms fail closed and are not paired.",
    )
    parser.add_argument("--require-known-cost", action="store_true")
    parser.add_argument(
        "--max-total-runtime-seconds",
        type=float,
        default=None,
        help="Cancel both arms after this wall-clock budget; incomplete pairs fail closed.",
    )
    parser.add_argument("--report-only", action="store_true")
    args = parser.parse_args(argv)
    if args.trials < 1:
        parser.error("--trials must be at least 1")
    if args.include_red_team and args.red_team_only:
        parser.error("--include-red-team and --red-team-only are mutually exclusive")
    if args.max_total_runtime_seconds is not None and args.max_total_runtime_seconds <= 0:
        parser.error("--max-total-runtime-seconds must be positive")

    try:
        settings = get_settings()
        result = run_provider_ab(
            cases_path=args.cases,
            forge_command=settings.forge_agent_command,
            trials=args.trials,
            include_red_team=args.include_red_team,
            red_team_only=args.red_team_only,
            require_red_team=args.require_red_team,
            max_total_cost_usd=args.max_total_cost_usd,
            require_known_cost=args.require_known_cost,
            max_total_runtime_seconds=args.max_total_runtime_seconds,
        )
        digest = write_provider_ab_artifact(
            args.output,
            comparison=result.comparison,
            control=result.control,
            candidate=result.candidate,
        )
    except (CaseLoadError, OSError, ValueError) as exc:
        print(f"error: provider A/B failed closed: {exc}")
        return 2

    print(
        json.dumps(
            {
                "comparison": result.comparison.model_dump(mode="json"),
                "artifact": str(args.output),
                "artifact_sha256": digest,
            },
            indent=2,
        )
    )
    if result.comparison.winner == "blocked" and not args.report_only:
        return 1
    return 0


def compare_provider_ab(
    *,
    control: ProviderABArm,
    candidate: ProviderABArm,
    source_commit: str | None = None,
    dirty_state_sha256: str | None = None,
    run_contract: ProviderABRunContract | None = None,
    task_ids: list[str] | None = None,
) -> ProviderABComparison:
    """Compare two complete, route-pure executions over the same task order."""

    if control.arm_id != "deepseek_v4_flash":
        raise ProviderABError("control_arm_invalid")
    if candidate.arm_id != "kimi_k3":
        raise ProviderABError("candidate_arm_invalid")
    if control.dataset_fingerprint != candidate.dataset_fingerprint:
        raise ProviderABError("dataset_fingerprint_mismatch")
    _validate_arm(control)
    _validate_arm(candidate)
    control_ids = [trace.task_id for trace in control.traces]
    candidate_ids = [trace.task_id for trace in candidate.traces]
    if control_ids != candidate_ids:
        raise ProviderABError("task_order_mismatch")
    if len(control.traces) != len(candidate.traces):
        raise ProviderABError("attempt_count_mismatch")
    base_task_ids = sorted(set(control_ids))
    expected_task_ids = task_ids if task_ids is not None else base_task_ids
    if expected_task_ids != base_task_ids:
        raise ProviderABError("task_ids_mismatch")
    if run_contract is not None and run_contract.trials * len(expected_task_ids) != len(
        control.traces
    ):
        raise ProviderABError("run_contract_attempt_count_mismatch")
    source_commit = source_commit or "0" * 40
    dirty_state_sha256 = dirty_state_sha256 or "0" * 64

    include_distribution = (
        run_contract is not None and run_contract.minimum_paired_attempts is not None
    )
    control_summary = _arm_summary(control, include_distribution=include_distribution)
    candidate_summary = _arm_summary(candidate, include_distribution=include_distribution)
    failure_category_deltas = (
        _failure_category_deltas(control_summary, candidate_summary)
        if include_distribution
        else {}
    )
    outcome_changes = _outcome_changes(control.traces, candidate.traces)
    intervals = _bootstrap_intervals(control.traces, candidate.traces)
    deltas = {name: interval.estimate for name, interval in intervals.items()}
    blockers = _trust_blockers(control, "control") + _trust_blockers(candidate, "candidate")
    if (
        run_contract is not None
        and run_contract.minimum_paired_attempts is not None
        and len(control.traces) < run_contract.minimum_paired_attempts
    ):
        blockers.append(
            "insufficient_paired_attempts:"
            f"{len(control.traces)}/{run_contract.minimum_paired_attempts}"
        )
    winner = "blocked" if blockers else _select_winner(intervals)
    return ProviderABComparison(
        control_arm_id=control.arm_id,
        candidate_arm_id=candidate.arm_id,
        dataset_fingerprint=control.dataset_fingerprint,
        source_commit=source_commit,  # type: ignore[arg-type]
        dirty_state_sha256=dirty_state_sha256,  # type: ignore[arg-type]
        run_contract=run_contract,
        task_ids=expected_task_ids,
        paired_attempts=len(control.traces),
        control=control_summary,
        candidate=candidate_summary,
        outcome_changes=outcome_changes,
        deltas=deltas,
        failure_category_deltas=failure_category_deltas,
        bootstrap_intervals=intervals,
        winner=winner,  # type: ignore[arg-type]
        blockers=sorted(set(blockers)),
        warnings=(
            ["comparison_is_diagnostic_only"]
            if not blockers
            else ["comparison_is_diagnostic_only", "untrusted_arm_blocks_winner"]
        ),
    )


def _validate_arm(arm: ProviderABArm) -> None:
    route = APPROVED_PROVIDER_AB_ROUTES.get(arm.arm_id)
    if route is None:
        raise ProviderABError("arm_route_unknown")
    if not arm.traces or arm.report.total_tasks != len(arm.traces):
        raise ProviderABError("attempt_count_mismatch")
    if len(arm.dataset_fingerprint) != 64 or any(
        character not in "0123456789abcdef" for character in arm.dataset_fingerprint
    ):
        raise ProviderABError("dataset_fingerprint_invalid")
    expected_identity = default_execution_identity(
        provider=route.provider,
        model=route.model,
        transport=route.transport,
        base_url=route.base_url,
    )
    for trace in arm.traces:
        if trace.provider != EvalProvider.FORGE:
            raise ProviderABError("trace_provider_invalid")
        if trace.model != route.model:
            raise ProviderABError("trace_model_mismatch")
        identity_events = [
            event
            for event in trace.raw_events
            if event.get("event_type") == "eval_execution_identity"
        ]
        if len(identity_events) != 1:
            raise ProviderABError("execution_identity_missing_or_duplicated")
        try:
            validate_execution_identity_event(identity_events[0], expected=expected_identity)
        except ValueError as exc:
            raise ProviderABError("execution_identity_mismatch") from exc
        if any(event.get("event_type") == "eval_provider_fallback" for event in trace.raw_events):
            raise ProviderABError("fallback_contamination")


def _git_source_identity(repository_root: Path) -> tuple[str, str]:
    """Return commit and a secret-free digest of the current dirty state."""

    commit = _git_text(repository_root, "rev-parse", "HEAD")
    status = _git_bytes(repository_root, "status", "--porcelain=v1", "--untracked-files=all", "-z")
    diff = _git_bytes(repository_root, "diff", "--binary", "HEAD", "--", ".")
    untracked: list[dict[str, str]] = []
    for entry in (part for part in status.split(b"\0") if part):
        if entry[:2] != b"??" or len(entry) < 4:
            continue
        relative_path = os.fsdecode(entry[3:])
        candidate = repository_root / relative_path
        if candidate.is_file():
            untracked.append(
                {"path": relative_path, "sha256": sha256_bytes(candidate.read_bytes())}
            )
    dirty_digest = sha256_bytes(
        b"status\0"
        + status
        + b"\0diff\0"
        + diff
        + b"\0untracked\0"
        + canonical_json_bytes(sorted(untracked, key=lambda item: item["path"]))
    )
    return commit, dirty_digest


def _git_text(repository_root: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args], cwd=repository_root, text=True, capture_output=True, check=False
    )
    if completed.returncode != 0 or not completed.stdout.strip():
        raise ProviderABError("source_identity_unavailable")
    return completed.stdout.strip()


def _git_bytes(repository_root: Path, *args: str) -> bytes:
    completed = subprocess.run(
        ["git", *args], cwd=repository_root, capture_output=True, check=False
    )
    if completed.returncode != 0:
        raise ProviderABError("dirty_state_unavailable")
    return completed.stdout


def _trust_blockers(arm: ProviderABArm, label: str) -> list[str]:
    blockers = [f"{label}:{blocker}" for blocker in arm.report.trust_result.blockers]
    if not arm.report.trust_result.trusted and not blockers:
        blockers.append(f"{label}:trust_not_confirmed")
    if arm.report.provider_cost_known_rate < 1.0:
        blockers.append(f"{label}:provider_cost_unknown")
    return blockers


def _arm_summary(arm: ProviderABArm, *, include_distribution: bool = False) -> ProviderABArmSummary:
    route = APPROVED_PROVIDER_AB_ROUTES[arm.arm_id]
    return ProviderABArmSummary(
        arm_id=arm.arm_id,
        provider=route.provider,
        model=route.model,
        transport=route.transport,  # type: ignore[arg-type]
        endpoint_family=route.endpoint_family,  # type: ignore[arg-type]
        base_url=route.base_url,
        attempts=arm.report.total_tasks,
        trusted=arm.report.trust_result.trusted,
        trust_blockers=sorted(arm.report.trust_result.blockers),
        success_rate=arm.report.success_rate,
        verification_pass_rate=arm.report.verification_pass_rate,
        scope_violation_rate=arm.report.scope_violation_rate,
        avg_model_rounds=arm.report.avg_model_rounds,
        avg_duration_ms=arm.report.avg_duration_ms,
        total_cost_usd=arm.report.total_cost_usd,
        provider_cost_known_rate=arm.report.provider_cost_known_rate,
        score_summary=arm.report.score_summary,
        distribution_summary=(
            _distribution_summary(arm.traces) if include_distribution else {}
        ),
        failure_categories=(arm.report.failure_categories if include_distribution else {}),
    )


def _distribution_summary(traces: tuple[AgentTrace, ...]) -> dict[str, float]:
    """Return deterministic median/p95 tail metrics for a new run contract."""

    return {
        metric: _quantile(values, quantile)
        for metric, values, quantile in (
            (
                "median_model_rounds",
                [float(trace.model_rounds) for trace in traces],
                0.50,
            ),
            (
                "median_duration_ms",
                [float(trace.duration_ms) for trace in traces],
                0.50,
            ),
            (
                "median_cost_usd",
                [float(trace.cost_usd or 0.0) for trace in traces],
                0.50,
            ),
            (
                "p95_model_rounds",
                [float(trace.model_rounds) for trace in traces],
                0.95,
            ),
            (
                "p95_duration_ms",
                [float(trace.duration_ms) for trace in traces],
                0.95,
            ),
            (
                "p95_cost_usd",
                [float(trace.cost_usd or 0.0) for trace in traces],
                0.95,
            ),
        )
    }


def _quantile(values: list[float], quantile: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    rank = min(len(ordered) - 1, max(0, int(quantile * len(ordered) + 0.999999) - 1))
    return ordered[rank]


def _failure_category_deltas(
    control: ProviderABArmSummary,
    candidate: ProviderABArmSummary,
) -> dict[str, int]:
    categories = set(control.failure_categories) | set(candidate.failure_categories)
    return {
        category: candidate.failure_categories.get(category, 0)
        - control.failure_categories.get(category, 0)
        for category in sorted(categories)
    }


def _outcome_changes(
    control: tuple[AgentTrace, ...], candidate: tuple[AgentTrace, ...]
) -> dict[str, str]:
    seen: dict[str, int] = {}
    changes: dict[str, str] = {}
    for control_trace, candidate_trace in zip(control, candidate, strict=True):
        repetition = seen.get(control_trace.task_id, 0) + 1
        seen[control_trace.task_id] = repetition
        if control_trace.task_id != candidate_trace.task_id:
            raise ProviderABError("task_order_mismatch")
        control_outcome = "success" if _trace_passed(control_trace) else "failure"
        candidate_outcome = "success" if _trace_passed(candidate_trace) else "failure"
        changes[f"r{repetition}:{control_trace.task_id}"] = (
            f"{control_outcome}->{candidate_outcome}"
        )
    return changes


def _bootstrap_intervals(
    control: tuple[AgentTrace, ...], candidate: tuple[AgentTrace, ...]
) -> dict[str, ProviderABInterval]:
    metric_values: dict[
        str, tuple[Callable[[AgentTrace], float], Callable[[list[float]], float]]
    ] = {
        "success_rate": (lambda trace: float(_trace_passed(trace)), _mean),
        "verification_pass_rate": (
            lambda trace: float(
                trace.verification_result is not None and trace.verification_result.passed
            ),
            _mean,
        ),
        "scope_violation_rate": (lambda trace: float(bool(trace.scope_violations)), _mean),
        "avg_model_rounds": (lambda trace: float(trace.model_rounds), _mean),
        "avg_duration_ms": (lambda trace: float(trace.duration_ms), _mean),
        "cost_usd": (lambda trace: float(trace.cost_usd or 0.0), _mean),
    }
    for score_name in sorted(UNIVERSAL_SCORE_NAMES):
        metric_values[f"score_{score_name}"] = (
            _score_value(score_name),
            _mean,
        )
    intervals: dict[str, ProviderABInterval] = {}
    for index, (name, (value_of, estimator)) in enumerate(metric_values.items()):
        intervals[name] = _paired_bootstrap_interval(
            [value_of(trace) for trace in control],
            [value_of(trace) for trace in candidate],
            estimator=estimator,
            seed=_BOOTSTRAP_SEED + index,
        )
    return intervals


def _paired_bootstrap_interval(
    control_values: list[float],
    candidate_values: list[float],
    *,
    estimator: Callable[[list[float]], float],
    seed: int,
) -> ProviderABInterval:
    if len(control_values) != len(candidate_values) or not control_values:
        raise ProviderABError("attempt_count_mismatch")
    estimate = estimator(candidate_values) - estimator(control_values)
    rng = Random(seed)
    deltas: list[float] = []
    for _ in range(_BOOTSTRAP_RESAMPLES):
        indexes = [rng.randrange(len(control_values)) for _ in control_values]
        deltas.append(
            estimator([candidate_values[index] for index in indexes])
            - estimator([control_values[index] for index in indexes])
        )
    ordered = sorted(deltas)
    low = ordered[int(0.025 * (_BOOTSTRAP_RESAMPLES - 1))]
    high = ordered[int(0.975 * (_BOOTSTRAP_RESAMPLES - 1))]
    return ProviderABInterval(
        estimate=estimate,
        low=min(low, estimate),
        high=max(high, estimate),
        resamples=_BOOTSTRAP_RESAMPLES,
        seed=seed,
    )


def _select_winner(intervals: dict[str, ProviderABInterval]) -> str:
    success = intervals["success_rate"]
    verification = intervals["verification_pass_rate"]
    scope = intervals["scope_violation_rate"]
    score_intervals = {
        name: intervals[f"score_{name}"]
        for name in UNIVERSAL_SCORE_NAMES
        if f"score_{name}" in intervals
    }
    if (
        success.high < 0
        or verification.high < 0
        or scope.low > 0
        or any(interval.high < 0 for interval in score_intervals.values())
    ):
        return "control"
    if (
        success.low > 0
        and verification.low >= 0
        and scope.high <= 0
        and all(interval.low >= 0 for interval in score_intervals.values())
    ):
        return "candidate"
    return "no_material_difference"


def _trace_passed(trace: AgentTrace) -> bool:
    return (
        trace.error is None
        and not trace.scope_violations
        and trace.verification_result is not None
        and trace.verification_result.passed
    )


def _score_value(name: str) -> Callable[[AgentTrace], float]:
    def value(trace: AgentTrace) -> float:
        return float(score_trace(trace)[name].score)

    return value


def _mean(values: list[float]) -> float:
    return sum(values) / len(values) if values else 0.0
