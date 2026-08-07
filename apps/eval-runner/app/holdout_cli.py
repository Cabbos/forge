"""CLI for the sealed, recoverable ForgeBench H4 campaign."""

from __future__ import annotations

import argparse
import json
import sys
from decimal import Decimal
from pathlib import Path
from typing import Any

from app.config import get_settings
from app.harness_contracts import HarnessPolicyV1
from app.harness_profiles import harness_profile_fingerprint, load_harness_profile
from app.holdout_campaign import (
    generate_holdout_report,
    initialize_holdout_campaign,
    recover_holdout_campaign,
    run_holdout_campaign,
    verify_holdout_campaign,
)
from app.holdout_gate import evaluate_holdout_gate, load_holdout_report
from app.models import EvalProvider
from app.runner import ForgeAgentRunner

EVAL_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = EVAL_ROOT.parents[1]
FORGEBENCH_ROOT = EVAL_ROOT / "forgebench"
DEFAULT_CASES = FORGEBENCH_ROOT / "cases" / "holdout"
DEFAULT_DATASET_LOCK = FORGEBENCH_ROOT / "dataset-lock.json"


def holdout_main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        prog="python -m app.cli holdout",
        description="Manage the sealed, budgeted ForgeBench holdout release gate.",
    )
    commands = parser.add_subparsers(dest="command", required=True)
    init = commands.add_parser("init")
    _add_campaign_inputs(init)
    init.add_argument("--campaign-id", required=True)
    init.add_argument("--development-comparison", type=Path, required=True)
    init.add_argument("--budget-cny", default="35")
    run = commands.add_parser("run")
    _add_campaign_inputs(run)
    run.add_argument("--max-attempts", type=int)
    for name in ("status", "recover", "report"):
        subparser = commands.add_parser(name)
        _add_campaign_inputs(subparser)
    verify = commands.add_parser("verify")
    verify.add_argument("--report", type=Path)
    _add_campaign_inputs(verify, required=False)
    args = parser.parse_args(argv)

    try:
        if args.command == "verify" and args.report is not None:
            if args.campaign is not None:
                raise ValueError("verify accepts either --report or --campaign")
            report = load_holdout_report(args.report)
            decision = evaluate_holdout_gate(report)
            print(json.dumps(decision.model_dump(mode="json"), sort_keys=True))
            return 0 if decision.passed else 1
        if args.command == "init":
            metadata = initialize_holdout_campaign(
                args.campaign,
                campaign_id=args.campaign_id,
                repository_root=REPOSITORY_ROOT,
                cases_path=args.cases,
                dataset_lock_path=args.dataset_lock,
                profile_path=args.profile,
                development_comparison_path=args.development_comparison,
                budget_cny=Decimal(args.budget_cny),
            )
            payload = {
                "campaign_id": metadata.campaign_id,
                "status": "active",
                "case_count": len(metadata.case_ids),
                "expected_attempts": metadata.max_attempts,
                "budget_cny": str(metadata.budget_cny),
                "source_commit": metadata.source_commit,
                "dataset_fingerprint": metadata.dataset_fingerprint,
                "harness_fingerprint": metadata.harness_fingerprint,
                "provider": metadata.profile.provider,
                "model": metadata.profile.model,
                "transport": metadata.profile.transport,
            }
            print(json.dumps(payload, sort_keys=True))
            return 0
        if args.command == "run":
            profile = load_harness_profile(args.profile)
            settings = get_settings()
            command = settings.forge_agent_command
            if command is None or not command.strip():
                raise ValueError("holdout_run_command_missing")
            runner = ForgeAgentRunner(
                provider=EvalProvider.FORGE,
                model=profile.profile_id,
                command=command,
                command_timeout_seconds=settings.command_timeout_seconds,
                setup_timeout_seconds=settings.setup_timeout_seconds,
                validation_timeout_seconds=settings.validation_timeout_seconds,
                agent_provider=profile.provider,
                agent_model=profile.model,
                agent_transport=profile.transport,
                agent_endpoint_family=profile.endpoint_family,
                agent_base_url=profile.resolved_base_url,
                harness_fingerprint=harness_profile_fingerprint(profile),
                max_model_rounds=profile.max_model_rounds,
                max_repair_attempts=profile.repair_attempt_limit,
                harness_policy=HarnessPolicyV1(
                    prompt_mode=profile.prompt_mode,
                    context_policy=profile.context_policy,
                    tool_result_policy=profile.tool_result_policy,
                    stagnation_policy=profile.stagnation_policy,
                    verification_policy=profile.verification_policy,
                    repair_attempt_limit=profile.repair_attempt_limit,
                    parallel_read_tools=profile.parallel_read_tools,
                ),
                use_provider_credential_store=True,
            )
            preflight = runner.credential_preflight()
            if not preflight.ok:
                raise ValueError("holdout_run_preflight_failed")
            result = run_holdout_campaign(
                args.campaign,
                repository_root=REPOSITORY_ROOT,
                cases_path=args.cases,
                dataset_lock_path=args.dataset_lock,
                profile_path=args.profile,
                runner=runner,
                max_attempts=args.max_attempts,
            )
            print(json.dumps(result.model_dump(mode="json"), sort_keys=True))
            return 1 if result.status == "blocked" else 0
        if args.command == "status":
            status_result = verify_holdout_campaign(**_campaign_kwargs(args))
            metadata = _metadata(args.campaign)
            payload = {
                "campaign_id": metadata.campaign_id,
                "status": "blocked"
                if not status_result.ok
                else (
                    "complete" if status_result.attempt_count == metadata.max_attempts else "active"
                ),
                "attempt_count": status_result.attempt_count,
                "pending_count": status_result.pending_count,
                "expected_attempts": metadata.max_attempts,
                "budget_cny": str(metadata.budget_cny),
                "spent_cny": str(status_result.spent_cny),
                "blockers": status_result.blockers,
            }
            print(json.dumps(payload, sort_keys=True))
            return 0 if status_result.ok else 1
        if args.command == "recover":
            result = recover_holdout_campaign(**_campaign_kwargs(args))
            print(json.dumps(result.model_dump(mode="json"), sort_keys=True))
            return 1 if result.status == "blocked" else 0
        if args.command == "report":
            report = generate_holdout_report(**_campaign_kwargs(args))
            print(json.dumps(report.model_dump(mode="json"), sort_keys=True))
            return 0 if evaluate_holdout_gate(report).passed else 1
        if args.command == "verify":
            if args.campaign is None:
                raise ValueError("verify requires --report or --campaign")
            verify_result = verify_holdout_campaign(**_campaign_kwargs(args))
            payload = verify_result.model_dump(mode="json")
            report_path = args.campaign / "holdout-report.json"
            if report_path.exists():
                decision = evaluate_holdout_gate(load_holdout_report(report_path))
                payload["gate"] = decision.model_dump(mode="json")
                result_ok = verify_result.ok and decision.passed
            else:
                result_ok = False
            print(json.dumps(payload, sort_keys=True))
            return 0 if result_ok else 1
        raise ValueError("unknown holdout command")
    except (OSError, ValueError, json.JSONDecodeError):
        print("error: holdout_verify_failed", file=sys.stderr)
        return 1


def _add_campaign_inputs(parser: argparse.ArgumentParser, *, required: bool = True) -> None:
    parser.add_argument("--campaign", type=Path, required=required)
    parser.add_argument("--profile", type=Path, required=required)
    parser.add_argument("--cases", type=Path, default=DEFAULT_CASES)
    parser.add_argument("--dataset-lock", type=Path, default=DEFAULT_DATASET_LOCK)


def _campaign_kwargs(args: argparse.Namespace) -> dict[str, Path]:
    return {
        "campaign_root": args.campaign,
        "repository_root": REPOSITORY_ROOT,
        "cases_path": args.cases,
        "dataset_lock_path": args.dataset_lock,
        "profile_path": args.profile,
    }


def _metadata(campaign: Path) -> Any:
    from app.holdout_campaign import HoldoutCampaignMetadata

    return HoldoutCampaignMetadata.model_validate_json((campaign / "campaign.json").read_bytes())
