import argparse
import json
import sys
from pathlib import Path

from app.baseline_cli import baseline_main
from app.campaign_cli import campaign_main
from app.cases import CaseLoadError, expand_prompt_mutations, load_cases
from app.config import get_settings
from app.execution import EvaluationExecution, ExecutionOptions, execute_evaluation
from app.experiments import experiment_artifact_metadata
from app.forgebench import main as forgebench_main
from app.harness_contracts import HarnessPolicyV1
from app.holdout_cli import holdout_main
from app.models import AgentTrace, BacktestReport, EvalProvider, EvaluationTask, TrustStatus
from app.provider_ab import provider_ab_main, provider_ab_verify_main
from app.provider_fallback import ProviderChainSpec, named_provider_chain
from app.red_team import is_red_team_task, red_team_failure_rate_for_traces
from app.release_cli import release_main
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
    provider_chain: ProviderChainSpec | None = None,
    max_total_cost_usd: float | None = None,
    require_known_cost: bool = False,
    max_model_rounds: int | None = None,
    harness_policy: HarnessPolicyV1 | None = None,
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
        provider_chain=provider_chain,
        max_total_cost_usd=max_total_cost_usd,
        require_known_cost=require_known_cost,
        max_model_rounds=max_model_rounds,
        harness_policy=harness_policy,
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
    provider_chain: ProviderChainSpec | None = None,
    max_total_cost_usd: float | None = None,
    require_known_cost: bool = False,
    max_model_rounds: int | None = None,
    harness_policy: HarnessPolicyV1 | None = None,
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
            provider_chain=provider_chain,
            max_total_cost_usd=max_total_cost_usd,
            require_known_cost=require_known_cost,
            max_model_rounds=max_model_rounds,
            harness_policy=harness_policy,
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
    provider_chain: ProviderChainSpec | None = None,
    max_total_cost_usd: float | None = None,
    require_known_cost: bool = False,
    max_model_rounds: int | None = None,
    harness_policy: HarnessPolicyV1 | None = None,
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
        provider_chain=provider_chain,
        max_total_cost_usd=max_total_cost_usd,
        require_known_cost=require_known_cost,
        max_model_rounds=max_model_rounds,
        harness_policy=harness_policy,
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


def threshold_failures(
    report: BacktestReport,
    args: argparse.Namespace,
    *,
    red_team_rate: float | None = None,
) -> list[str]:
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
    if args.require_known_cost and report.provider_cost_known_rate < 1.0:
        failures.append(
            f"provider_cost_known_rate below required 1.000: {report.provider_cost_known_rate:.3f}"
        )
    if args.max_red_team_failure_rate is not None:
        measured_red_team_rate = (
            red_team_rate if red_team_rate is not None else red_team_failure_rate(report)
        )
        if measured_red_team_rate > args.max_red_team_failure_rate:
            failures.append(
                "red_team_failure_rate above threshold: "
                f"{measured_red_team_rate:.3f} > {args.max_red_team_failure_rate:.3f}"
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
    if raw_argv[:1] == ["forgebench"]:
        return forgebench_main(raw_argv[1:])
    if raw_argv[:1] == ["campaign"]:
        return campaign_main(raw_argv[1:])
    if raw_argv[:1] == ["baseline"]:
        return baseline_main(raw_argv[1:])
    if raw_argv[:1] == ["holdout"]:
        return holdout_main(raw_argv[1:])
    if raw_argv[:1] == ["release"]:
        return release_main(raw_argv[1:])
    if raw_argv[:1] == ["provider-ab"]:
        return provider_ab_main(raw_argv[1:])
    if raw_argv[:1] == ["provider-ab-verify"]:
        return provider_ab_verify_main(raw_argv[1:])

    parser = argparse.ArgumentParser(description="Run Forge eval cases and print a JSON report.")
    parser.add_argument(
        "--cases",
        type=Path,
        required=True,
        help="Path to a case JSON file or directory.",
    )
    parser.add_argument("--provider", default="mock", choices=["mock", "forge"])
    parser.add_argument(
        "--provider-chain",
        choices=["kimi-k3-deepseek"],
        default=None,
        help="Development-only Kimi k3 chain with DeepSeek V4 Flash auth/quota fallback.",
    )
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
    parser.add_argument(
        "--max-model-rounds",
        type=int,
        default=None,
        help="Optional controller cap forwarded to each Forge task (development diagnostics).",
    )
    parser.add_argument(
        "--harness-context-policy",
        choices=["current_product_v1", "task_local_v1", "target_files_v1"],
        default=None,
        help="Optional development context strategy; target_files_v1 prefills allowed files.",
    )
    parser.add_argument("--max-total-cost-usd", type=float, default=None)
    parser.add_argument(
        "--require-known-cost",
        action="store_true",
        help="Fail the threshold gate unless every trace has known provider cost.",
    )
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
    if args.max_model_rounds is not None and args.max_model_rounds < 1:
        parser.error("--max-model-rounds must be at least 1")

    harness_policy = (
        HarnessPolicyV1(context_policy=args.harness_context_policy)
        if args.harness_context_policy is not None
        else None
    )

    provider_chain = named_provider_chain(args.provider_chain) if args.provider_chain else None
    if provider_chain is not None and args.provider != "forge":
        parser.error("--provider-chain requires --provider forge")
    if provider_chain is not None and args.model not in (None, provider_chain.primary.model):
        parser.error("--provider-chain fixes the primary model to k3")
    model = args.model or (
        provider_chain.primary.model
        if provider_chain is not None
        else ("local-forge" if args.provider == "forge" else "deterministic-agent-v1")
    )
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
            provider_chain=provider_chain,
            max_total_cost_usd=args.max_total_cost_usd,
            require_known_cost=args.require_known_cost,
            max_model_rounds=args.max_model_rounds,
            harness_policy=harness_policy,
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
    red_team_rate = None
    if args.max_red_team_failure_rate is not None or args.require_red_team:
        red_team_rate = red_team_failure_rate_for_traces(
            load_backtest_tasks(
                args.cases,
                prompt_mutations=args.prompt_mutation,
                mutations_only=args.mutations_only,
                include_red_team=args.include_red_team,
                red_team_only=args.red_team_only,
            ),
            traces,
        )
    failures = threshold_failures(report, args, red_team_rate=red_team_rate)
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
