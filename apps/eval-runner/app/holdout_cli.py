"""CLI for verifying a frozen ForgeBench holdout report."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from app.holdout_gate import evaluate_holdout_gate, load_holdout_report


def holdout_main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        prog="python -m app.cli holdout",
        description="Verify the frozen ForgeBench holdout release gate.",
    )
    commands = parser.add_subparsers(dest="command", required=True)
    verify = commands.add_parser("verify")
    verify.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)

    try:
        report = load_holdout_report(args.report)
        decision = evaluate_holdout_gate(report)
    except (OSError, ValueError, json.JSONDecodeError):
        print("error: holdout_verify_failed", file=sys.stderr)
        return 1

    print(json.dumps(decision.model_dump(mode="json"), sort_keys=True))
    return 0 if decision.passed else 1
