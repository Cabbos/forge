from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from app.cases import validate_case_quality
from app.datasets import dataset_fingerprint
from app.harness_checks import run_golden_harness_check
from app.judge_calibration import score_can_gate_ci
from app.models import (
    AgentTrace,
    BacktestReport,
    EvalProvider,
    EvaluationTask,
    TrustGateResult,
)
from app.process_control import CancelRequested, never_cancelled
from app.red_team import is_red_team_task
from app.reporting import build_report
from app.runner import create_runner
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
    runner = create_runner(
        provider=options.provider,
        model=options.model,
        forge_command=options.forge_command,
        command_timeout_seconds=options.command_timeout_seconds,
        setup_timeout_seconds=options.setup_timeout_seconds,
        validation_timeout_seconds=options.validation_timeout_seconds,
    )
    traces: list[AgentTrace] = []
    for task in tasks:
        if cancel_requested():
            break
        traces.append(runner.run_task(task, cancel_requested=cancel_requested))
    return traces


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
    red_team_passed = (
        all(trace.error is None for trace in red_team_traces) if red_team_traces else None
    )
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
    report = report.model_copy(update={"trust_result": trust})
    return EvaluationExecution(traces=traces, report=report, trust_result=trust)
