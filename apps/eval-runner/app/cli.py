import argparse
import json
import sys
from pathlib import Path

from app.cases import CaseLoadError, expand_prompt_mutations, load_cases
from app.config import get_settings
from app.execution import EvaluationExecution, ExecutionOptions, execute_evaluation
from app.experiments import experiment_artifact_metadata
from app.models import AgentTrace, BacktestReport, EvalProvider, EvaluationTask, TrustStatus
from app.red_team import is_red_team_task
from app.trace_import import promote_failed_traces


def run_backtest_with_traces(
    cases_path: Path,
    *,
    provider: str,
    model: str,
    forge_command: str | None,
    trials: int = 1,
    prompt_mutations: list[str] | None = None,
    mutations_only: bool = False,
    include_red_team: bool = False,
    red_team_only: bool = False,
) -> tuple[BacktestReport, list[AgentTrace]]:
    execution = run_backtest_execution(
        cases_path,
        provider=provider,
        model=model,
        forge_command=forge_command,
        trials=trials,
        prompt_mutations=prompt_mutations,
        mutations_only=mutations_only,
        include_red_team=include_red_team,
        red_team_only=red_team_only,
    )
    return execution.report, execution.traces


def run_backtest_execution(
    cases_path: Path,
    *,
    provider: str,
    model: str,
    forge_command: str | None,
    trials: int = 1,
    prompt_mutations: list[str] | None = None,
    mutations_only: bool = False,
    include_red_team: bool = False,
    red_team_only: bool = False,
    require_red_team: bool = False,
) -> EvaluationExecution:
    tasks = load_backtest_tasks(
        cases_path,
        prompt_mutations=prompt_mutations or [],
        mutations_only=mutations_only,
        include_red_team=include_red_team,
        red_team_only=red_team_only,
    )
    settings = get_settings()
    trial_tasks = [task for _ in range(trials) for task in tasks]
    return execute_evaluation(
        cases_path=cases_path,
        tasks=trial_tasks,
        options=ExecutionOptions(
            provider=EvalProvider(provider),
            model=model,
            forge_command=forge_command,
            command_timeout_seconds=settings.command_timeout_seconds,
            setup_timeout_seconds=settings.setup_timeout_seconds,
            validation_timeout_seconds=settings.validation_timeout_seconds,
            require_red_team=require_red_team,
        ),
    )


def run_backtest(
    cases_path: Path,
    *,
    provider: str,
    model: str,
    forge_command: str | None,
    trials: int = 1,
    prompt_mutations: list[str] | None = None,
    mutations_only: bool = False,
    include_red_team: bool = False,
    red_team_only: bool = False,
) -> BacktestReport:
    report, _ = run_backtest_with_traces(
        cases_path,
        provider=provider,
        model=model,
        forge_command=forge_command,
        trials=trials,
        prompt_mutations=prompt_mutations,
        mutations_only=mutations_only,
        include_red_team=include_red_team,
        red_team_only=red_team_only,
    )
    return report


def load_backtest_tasks(
    cases_path: Path,
    *,
    prompt_mutations: list[str],
    mutations_only: bool = False,
    include_red_team: bool = False,
    red_team_only: bool = False,
) -> list[EvaluationTask]:
    tasks = load_cases(cases_path)
    tasks = filter_red_team_tasks(
        tasks,
        include_red_team=include_red_team,
        red_team_only=red_team_only,
    )
    return expand_prompt_mutations(
        tasks,
        styles=prompt_mutations,
        mutations_only=mutations_only,
    )


def filter_red_team_tasks(
    tasks: list[EvaluationTask],
    *,
    include_red_team: bool = False,
    red_team_only: bool = False,
) -> list[EvaluationTask]:
    if red_team_only:
        return [task for task in tasks if is_red_team_task(task)]
    if include_red_team:
        return tasks
    return [task for task in tasks if not is_red_team_task(task)]


def write_backtest_artifact(
    output_path: Path,
    *,
    report: BacktestReport,
    traces: list[AgentTrace],
    experiment: dict[str, str] | None = None,
) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "report": report.model_dump(mode="json"),
        "traces": [trace.model_dump(mode="json") for trace in traces],
    }
    if experiment is not None:
        payload["experiment"] = experiment
    output_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def threshold_failures(report: BacktestReport, args: argparse.Namespace) -> list[str]:
    failures: list[str] = []
    if args.min_success_rate is not None and report.success_rate < args.min_success_rate:
        failures.append(
            f"success_rate below threshold: {report.success_rate:.3f} < {args.min_success_rate:.3f}"
        )
    if (
        args.max_scope_violation_rate is not None
        and report.scope_violation_rate > args.max_scope_violation_rate
    ):
        failures.append(
            "scope_violation_rate above threshold: "
            f"{report.scope_violation_rate:.3f} > {args.max_scope_violation_rate:.3f}"
        )
    if (
        args.max_avg_model_rounds is not None
        and report.avg_model_rounds > args.max_avg_model_rounds
    ):
        failures.append(
            "avg_model_rounds above threshold: "
            f"{report.avg_model_rounds:.3f} > {args.max_avg_model_rounds:.3f}"
        )
    if args.max_total_cost_usd is not None and report.total_cost_usd > args.max_total_cost_usd:
        failures.append(
            "total_cost_usd above threshold: "
            f"{report.total_cost_usd:.3f} > {args.max_total_cost_usd:.3f}"
        )
    if args.max_red_team_failure_rate is not None:
        red_team_rate = red_team_failure_rate(report)
        if red_team_rate > args.max_red_team_failure_rate:
            failures.append(
                "red_team_failure_rate above threshold: "
                f"{red_team_rate:.3f} > {args.max_red_team_failure_rate:.3f}"
            )
    return failures


def red_team_failure_rate(report: BacktestReport) -> float:
    red_team_tasks = [
        task
        for task in report.tasks
        if task.task_id.startswith("red-team-") or "__red-team-" in task.task_id
    ]
    if not red_team_tasks:
        return 0.0
    failed = sum(1 for task in red_team_tasks if not task.passed)
    return failed / len(red_team_tasks)


def main(argv: list[str] | None = None) -> int:
    raw_argv = list(argv) if argv is not None else sys.argv[1:]
    if raw_argv[:1] == ["promote-trace"]:
        return promote_trace_main(raw_argv[1:])

    parser = argparse.ArgumentParser(description="Run Forge eval cases and print a JSON report.")
    parser.add_argument(
        "--cases",
        type=Path,
        required=True,
        help="Path to a case JSON file or directory.",
    )
    parser.add_argument("--provider", default="mock", choices=["mock", "forge"])
    parser.add_argument("--model", default=None)
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Optional path for a full JSON artifact containing the report and traces.",
    )
    parser.add_argument("--experiment-name", default=None)
    parser.add_argument("--trials", type=int, default=1)
    parser.add_argument("--prompt-mutation", action="append", default=[])
    parser.add_argument("--mutations-only", action="store_true")
    parser.add_argument("--min-success-rate", type=float, default=None)
    parser.add_argument("--max-scope-violation-rate", type=float, default=None)
    parser.add_argument("--max-avg-model-rounds", type=float, default=None)
    parser.add_argument("--max-total-cost-usd", type=float, default=None)
    parser.add_argument("--include-red-team", action="store_true")
    parser.add_argument("--red-team-only", action="store_true")
    parser.add_argument("--max-red-team-failure-rate", type=float, default=None)
    parser.add_argument(
        "--report-only",
        action="store_true",
        help="Print results without using trust blockers as an exit gate.",
    )
    parser.add_argument(
        "--require-red-team",
        action="store_true",
        help="Require red-team evidence for this trusted run.",
    )
    args = parser.parse_args(argv)

    if args.trials < 1:
        parser.error("--trials must be at least 1")

    model = args.model or ("local-forge" if args.provider == "forge" else "deterministic-agent-v1")
    settings = get_settings()
    try:
        execution = run_backtest_execution(
            args.cases,
            provider=args.provider,
            model=model,
            forge_command=settings.forge_agent_command,
            trials=args.trials,
            prompt_mutations=args.prompt_mutation,
            mutations_only=args.mutations_only,
            include_red_team=args.include_red_team,
            red_team_only=args.red_team_only,
            require_red_team=args.require_red_team,
        )
    except CaseLoadError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    report = execution.report
    traces = execution.traces

    if args.output is not None:
        experiment = None
        if args.experiment_name is not None:
            experiment = experiment_artifact_metadata(
                name=args.experiment_name,
                tasks=load_backtest_tasks(
                    args.cases,
                    prompt_mutations=args.prompt_mutation,
                    mutations_only=args.mutations_only,
                    include_red_team=args.include_red_team,
                    red_team_only=args.red_team_only,
                ),
                provider=args.provider,
                model=model,
            )
        write_backtest_artifact(args.output, report=report, traces=traces, experiment=experiment)

    print(report.model_dump_json(indent=2))
    failures = threshold_failures(report, args)
    for failure in failures:
        print(f"error: {failure}", file=sys.stderr)
    if args.report_only:
        return 1 if failures else 0
    if execution.trust_result.status != TrustStatus.TRUSTED:
        for blocker in execution.trust_result.blockers:
            print(f"error: trust blocker: {blocker}", file=sys.stderr)
        return 1
    return 1 if failures else 0


def promote_trace_main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Promote failed AgentTrace records into eval case directories."
    )
    parser.add_argument("--trace", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)

    written = promote_failed_traces(args.trace, args.output)
    print(json.dumps({"written": [str(path) for path in written]}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
