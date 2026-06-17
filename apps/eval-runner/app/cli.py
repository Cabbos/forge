import argparse
import json
import sys
from pathlib import Path

from app.cases import CaseLoadError, expand_prompt_mutations, load_cases
from app.config import get_settings
from app.experiments import experiment_artifact_metadata
from app.models import AgentTrace, BacktestReport
from app.reporting import build_report
from app.runner import create_runner


def run_backtest_with_traces(
    cases_path: Path,
    *,
    provider: str,
    model: str,
    forge_command: str | None,
    trials: int = 1,
    prompt_mutations: list[str] | None = None,
    mutations_only: bool = False,
) -> tuple[BacktestReport, list[AgentTrace]]:
    tasks = load_backtest_tasks(
        cases_path,
        prompt_mutations=prompt_mutations or [],
        mutations_only=mutations_only,
    )
    runner = create_runner(provider=provider, model=model, forge_command=forge_command)
    traces = [runner.run_task(task) for _ in range(trials) for task in tasks]
    return build_report(traces), traces


def run_backtest(
    cases_path: Path,
    *,
    provider: str,
    model: str,
    forge_command: str | None,
    trials: int = 1,
    prompt_mutations: list[str] | None = None,
    mutations_only: bool = False,
) -> BacktestReport:
    report, _ = run_backtest_with_traces(
        cases_path,
        provider=provider,
        model=model,
        forge_command=forge_command,
        trials=trials,
        prompt_mutations=prompt_mutations,
        mutations_only=mutations_only,
    )
    return report


def load_backtest_tasks(
    cases_path: Path,
    *,
    prompt_mutations: list[str],
    mutations_only: bool = False,
):
    tasks = load_cases(cases_path)
    return expand_prompt_mutations(
        tasks,
        styles=prompt_mutations,
        mutations_only=mutations_only,
    )


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


def main(argv: list[str] | None = None) -> int:
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
    args = parser.parse_args(argv)

    if args.trials < 1:
        parser.error("--trials must be at least 1")

    model = args.model or ("local-forge" if args.provider == "forge" else "deterministic-agent-v1")
    settings = get_settings()
    try:
        report, traces = run_backtest_with_traces(
            args.cases,
            provider=args.provider,
            model=model,
            forge_command=settings.forge_agent_command,
            trials=args.trials,
            prompt_mutations=args.prompt_mutation,
            mutations_only=args.mutations_only,
        )
    except CaseLoadError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    if args.output is not None:
        experiment = None
        if args.experiment_name is not None:
            experiment = experiment_artifact_metadata(
                name=args.experiment_name,
                tasks=load_backtest_tasks(
                    args.cases,
                    prompt_mutations=args.prompt_mutation,
                    mutations_only=args.mutations_only,
                ),
                provider=args.provider,
                model=model,
            )
        write_backtest_artifact(args.output, report=report, traces=traces, experiment=experiment)

    print(report.model_dump_json(indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
