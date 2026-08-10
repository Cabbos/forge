from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from app.cases import validate_case_quality
from app.datasets import dataset_fingerprint
from app.harness_checks import run_golden_harness_check
from app.harness_contracts import HarnessPolicyV1
from app.judge_calibration import score_can_gate_ci
from app.models import (
    AgentTrace,
    BacktestReport,
    EvalProvider,
    EvaluationTask,
    TrustGateResult,
    TrustStatus,
)
from app.process_control import CancelRequested, never_cancelled
from app.provider_fallback import (
    ProviderChainSpec,
    ProviderRouteSpec,
    build_provider_fallback_runner,
    build_provider_route_runner,
)
from app.red_team import is_red_team_task, red_team_batch_passed
from app.reporting import build_report
from app.runner import EvalRunner, create_runner
from app.scoring import score_trace
from app.trust_gates import evaluate_trust_gates


@dataclass(frozen=True)
class ExecutionOptions:
    provider: EvalProvider
    model: str
    forge_command: str | None
    command_timeout_seconds: float
    setup_timeout_seconds: float
    validation_timeout_seconds: float
    require_red_team: bool
    provider_chain: ProviderChainSpec | None = None
    provider_route: ProviderRouteSpec | None = None
    max_total_cost_usd: float | None = None
    require_known_cost: bool = False
    max_model_rounds: int | None = None
    harness_policy: HarnessPolicyV1 | None = None


@dataclass(frozen=True)
class EvaluationExecution:
    traces: list[AgentTrace]
    report: BacktestReport
    trust_result: TrustGateResult


def execute_tasks(
    tasks: list[EvaluationTask],
    options: ExecutionOptions,
    cancel_requested: CancelRequested,
) -> list[AgentTrace]:
    runner: EvalRunner
    if options.provider_chain is not None and options.provider_route is not None:
        raise ValueError("provider chain and single provider route are mutually exclusive")
    if options.provider_route is not None:
        if options.provider != EvalProvider.FORGE:
            raise ValueError("provider routes require provider=forge")
        if options.harness_policy is not None:
            runner = build_provider_route_runner(
                options.provider_route,
                command=options.forge_command,
                command_timeout_seconds=options.command_timeout_seconds,
                setup_timeout_seconds=options.setup_timeout_seconds,
                validation_timeout_seconds=options.validation_timeout_seconds,
                max_model_rounds=options.max_model_rounds,
                harness_policy=options.harness_policy,
            )
        else:
            runner = build_provider_route_runner(
                options.provider_route,
                command=options.forge_command,
                command_timeout_seconds=options.command_timeout_seconds,
                setup_timeout_seconds=options.setup_timeout_seconds,
                validation_timeout_seconds=options.validation_timeout_seconds,
                max_model_rounds=options.max_model_rounds,
            )
    elif options.provider_chain is not None:
        if options.provider != EvalProvider.FORGE:
            raise ValueError("provider chains require provider=forge")
        if options.harness_policy is not None:
            runner = build_provider_fallback_runner(
                options.provider_chain,
                command=options.forge_command,
                command_timeout_seconds=options.command_timeout_seconds,
                setup_timeout_seconds=options.setup_timeout_seconds,
                validation_timeout_seconds=options.validation_timeout_seconds,
                max_model_rounds=options.max_model_rounds,
                harness_policy=options.harness_policy,
            )
        else:
            runner = build_provider_fallback_runner(
                options.provider_chain,
                command=options.forge_command,
                command_timeout_seconds=options.command_timeout_seconds,
                setup_timeout_seconds=options.setup_timeout_seconds,
                validation_timeout_seconds=options.validation_timeout_seconds,
                max_model_rounds=options.max_model_rounds,
            )
    else:
        if options.harness_policy is not None:
            runner = create_runner(
                provider=options.provider,
                model=options.model,
                forge_command=options.forge_command,
                command_timeout_seconds=options.command_timeout_seconds,
                setup_timeout_seconds=options.setup_timeout_seconds,
                validation_timeout_seconds=options.validation_timeout_seconds,
                max_model_rounds=options.max_model_rounds,
                harness_policy=options.harness_policy,
            )
        else:
            runner = create_runner(
                provider=options.provider,
                model=options.model,
                forge_command=options.forge_command,
                command_timeout_seconds=options.command_timeout_seconds,
                setup_timeout_seconds=options.setup_timeout_seconds,
                validation_timeout_seconds=options.validation_timeout_seconds,
                max_model_rounds=options.max_model_rounds,
            )
    traces: list[AgentTrace] = []
    for task in tasks:
        if cancel_requested():
            break
        if _cost_boundary_reached(traces, options):
            break
        traces.append(runner.run_task(task, cancel_requested=cancel_requested))
        if _cost_boundary_reached(traces, options):
            break
    return traces


def _cost_boundary_reached(traces: list[AgentTrace], options: ExecutionOptions) -> bool:
    if options.require_known_cost and any(trace.cost_usd is None for trace in traces):
        return True
    if options.max_total_cost_usd is None:
        return False
    if any(trace.cost_usd is None for trace in traces):
        return True
    return sum(trace.cost_usd or 0.0 for trace in traces) >= options.max_total_cost_usd


def execute_evaluation(
    *,
    cases_path: Path,
    tasks: list[EvaluationTask],
    options: ExecutionOptions,
    cancel_requested: CancelRequested = never_cancelled,
) -> EvaluationExecution:
    quality = validate_case_quality(tasks)
    fingerprint = dataset_fingerprint(tasks) if tasks else None
    harness = run_golden_harness_check(cases_path)
    traces = execute_tasks(tasks, options, cancel_requested)
    report = build_report(traces)
    scorer_calibrated = all(
        score_can_gate_ci(score) for trace in traces for score in score_trace(trace).values()
    )
    red_team_traces = [
        trace for trace, task in zip(traces, tasks, strict=False) if is_red_team_task(task)
    ]
    red_team_passed = red_team_batch_passed(tasks, traces) if red_team_traces else None
    trust = evaluate_trust_gates(
        harness_check=harness,
        dataset_fingerprint=fingerprint,
        case_quality_issues=quality,
        traces=traces,
        scorer_calibrated=scorer_calibrated,
        red_team_passed=red_team_passed,
        require_red_team=options.require_red_team,
        score_coverage=report.score_coverage,
    )
    if len(traces) < len(tasks) and _cost_boundary_reached(traces, options):
        trust = trust.model_copy(
            update={
                "status": TrustStatus.UNKNOWN,
                "trusted": False,
                "blockers": sorted(set([*trust.blockers, "execution_stopped_at_cost_boundary"])),
            }
        )
    report = report.model_copy(update={"trust_result": trust})
    return EvaluationExecution(traces=traces, report=report, trust_result=trust)
