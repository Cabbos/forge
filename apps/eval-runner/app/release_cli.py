"""CLI for the offline Forge frontier release gate."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from app.holdout_gate import load_holdout_report
from app.release_gate import evaluate_frontier_release


def release_main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        prog="python -m app.cli release",
        description="Combine the frozen holdout gate with Forge provenance evidence.",
    )
    commands = parser.add_subparsers(dest="command", required=True)
    verify = commands.add_parser("verify")
    verify.add_argument("--holdout-report", type=Path, required=True)
    verify.add_argument("--campaign", type=Path, required=True)
    args = parser.parse_args(argv)

    try:
        report = load_holdout_report(args.holdout_report)
        decision = evaluate_frontier_release(report, args.campaign)
    except (OSError, ValueError, json.JSONDecodeError):
        print("error: frontier_release_verification_failed", file=sys.stderr)
        return 1

    print(json.dumps(decision.model_dump(mode="json"), sort_keys=True))
    return 0 if decision.passed else 1
