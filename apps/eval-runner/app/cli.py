import argparse
import json
import sys
from pathlib import Path

from app.cases import CaseLoadError, load_cases
from app.config import get_settings
from app.models import AgentTrace, BacktestReport
from app.reporting import build_report
from app.runner import create_runner


def run_backtest_with_traces(
    cases_path: Path,
    *,
    provider: str,
    model: str,
    forge_command: str | None,
) -> tuple[BacktestReport, list[AgentTrace]]:
    tasks = load_cases(cases_path)
    runner = create_runner(provider=provider, model=model, forge_command=forge_command)
    traces = [runner.run_task(task) for task in tasks]
    return build_report(traces), traces


def run_backtest(
    cases_path: Path,
    *,
    provider: str,
    model: str,
    forge_command: str | None,
) -> BacktestReport:
    report, _ = run_backtest_with_traces(
        cases_path,
        provider=provider,
        model=model,
        forge_command=forge_command,
    )
    return report


def write_backtest_artifact(
    output_path: Path,
    *,
    report: BacktestReport,
    traces: list[AgentTrace],
) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "report": report.model_dump(mode="json"),
        "traces": [trace.model_dump(mode="json") for trace in traces],
    }
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
    args = parser.parse_args(argv)

    model = args.model or ("local-forge" if args.provider == "forge" else "deterministic-agent-v1")
    settings = get_settings()
    try:
        report, traces = run_backtest_with_traces(
            args.cases,
            provider=args.provider,
            model=model,
            forge_command=settings.forge_agent_command,
        )
    except CaseLoadError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    if args.output is not None:
        write_backtest_artifact(args.output, report=report, traces=traces)

    print(report.model_dump_json(indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
