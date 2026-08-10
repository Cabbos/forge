"""CLI for the offline Forge frontier release gate."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from app.forgebench import DEFAULT_DATASET_LOCK, FORGEBENCH_ROOT
from app.holdout_gate import load_holdout_report
from app.release_gate import evaluate_frontier_release

EVAL_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = EVAL_ROOT.parents[1]
DEFAULT_HOLDOUT_CASES = FORGEBENCH_ROOT / "cases" / "holdout"


def release_main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        prog="python -m app.cli release",
        description="Combine the frozen holdout gate with Forge provenance evidence.",
    )
    commands = parser.add_subparsers(dest="command", required=True)
    verify = commands.add_parser("verify")
    verify.add_argument(
        "--holdout-campaign",
        type=Path,
        required=True,
        help="sealed H4 campaign containing holdout-report.json and all attempt evidence",
    )
    verify.add_argument("--campaign", type=Path, required=True)
    args = parser.parse_args(argv)

    try:
        report = load_holdout_report(args.holdout_campaign / "holdout-report.json")
        profile_path = EVAL_ROOT / "forgebench" / "profiles" / f"{report.profile_id}.json"
        decision = evaluate_frontier_release(
            report,
            args.campaign,
            holdout_campaign_root=args.holdout_campaign,
            repository_root=REPOSITORY_ROOT,
            cases_path=DEFAULT_HOLDOUT_CASES,
            dataset_lock_path=DEFAULT_DATASET_LOCK,
            profile_path=profile_path,
        )
    except (OSError, ValueError, json.JSONDecodeError):
        print("error: frontier_release_verification_failed", file=sys.stderr)
        return 1

    print(json.dumps(decision.model_dump(mode="json"), sort_keys=True))
    return 0 if decision.passed else 1
