import argparse
import json
import sys
from pathlib import Path
from typing import Any

from app.baseline import (
    generate_baseline_report,
    initialize_baseline_campaign,
    recover_baseline_campaign,
    run_baseline_campaign,
    verify_baseline_campaign,
)
from app.baseline_models import BaselineCampaignMetadata
from app.config import get_settings
from app.harness_profiles import (
    harness_profile_fingerprint,
    load_harness_profile,
)
from app.models import EvalProvider
from app.runner import ForgeAgentRunner

EVAL_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
FORGEBENCH_ROOT = EVAL_ROOT / "forgebench"
DEFAULT_CASES = FORGEBENCH_ROOT / "cases" / "development"
DEFAULT_DATASET_LOCK = FORGEBENCH_ROOT / "dataset-lock.json"


def baseline_main(argv: list[str]) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)

    try:
        if args.command == "init":
            payload = _initialize(args)
            exit_code = 0
        elif args.command == "run":
            payload, exit_code = _run(args)
        elif args.command == "status":
            payload, exit_code = _status(args)
        elif args.command == "recover":
            payload, exit_code = _recover(args)
        elif args.command == "report":
            payload, exit_code = _report(args)
        elif args.command == "verify":
            payload, exit_code = _verify(args)
        else:
            raise ValueError("unknown baseline command")
    except _AlreadyReportedError:
        return 1
    except Exception:
        print(f"error: baseline_{args.command}_failed", file=sys.stderr)
        return 1

    print(json.dumps(payload, sort_keys=True))
    return exit_code


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="python -m app.cli baseline",
        description="Manage a budgeted, machine-verifiable Forge baseline.",
    )
    commands = parser.add_subparsers(dest="command", required=True)

    init = commands.add_parser("init")
    _add_campaign_inputs(init)
    init.add_argument("--campaign-id", required=True)

    run = commands.add_parser("run")
    _add_campaign_inputs(run)
    run.add_argument("--max-attempts", type=int)

    for name in ("status", "recover", "report", "verify"):
        command = commands.add_parser(name)
        _add_campaign_inputs(command)
    return parser


def _add_campaign_inputs(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--campaign", type=Path, required=True)
    parser.add_argument("--profile", type=Path, required=True)
    parser.add_argument("--cases", type=Path, default=DEFAULT_CASES)
    parser.add_argument("--dataset-lock", type=Path, default=DEFAULT_DATASET_LOCK)


def _initialize(args: argparse.Namespace) -> dict[str, Any]:
    metadata = initialize_baseline_campaign(
        args.campaign,
        campaign_id=args.campaign_id,
        repository_root=REPOSITORY_ROOT,
        cases_path=args.cases,
        dataset_lock_path=args.dataset_lock,
        profile_path=args.profile,
    )
    return {
        "campaign_id": metadata.campaign_id,
        "status": "active",
        "task_count": len(metadata.task_ids),
        "expected_attempts": metadata.max_attempts,
        "budget_cny": str(metadata.budget_cny),
        "spent_cny": "0",
        "source_commit": metadata.source_commit,
        "dirty_state_sha256": metadata.dirty_state_sha256,
        "dataset_fingerprint": metadata.dataset_fingerprint,
        "harness_fingerprint": metadata.harness_fingerprint,
        "provider": metadata.profile.provider,
        "model": metadata.profile.model,
        "transport": metadata.profile.transport,
    }


def _run(args: argparse.Namespace) -> tuple[dict[str, Any], int]:
    settings = get_settings()
    command = settings.forge_agent_command
    if command is None or not command.strip():
        print("error: baseline_run_command_missing", file=sys.stderr)
        raise _AlreadyReportedError

    profile = load_harness_profile(args.profile)
    result = run_baseline_campaign(
        args.campaign,
        repository_root=REPOSITORY_ROOT,
        cases_path=args.cases,
        dataset_lock_path=args.dataset_lock,
        profile_path=args.profile,
        runner=ForgeAgentRunner(
            provider=EvalProvider.FORGE,
            model=profile.profile_id,
            command=command,
            command_timeout_seconds=settings.command_timeout_seconds,
            setup_timeout_seconds=settings.setup_timeout_seconds,
            validation_timeout_seconds=settings.validation_timeout_seconds,
            agent_provider=profile.provider,
            agent_model=profile.model,
            agent_transport=profile.transport,
            harness_fingerprint=harness_profile_fingerprint(profile),
            max_model_rounds=profile.max_model_rounds,
            max_repair_attempts=profile.repair_attempt_limit,
        ),
        max_attempts=args.max_attempts,
    )
    return result.model_dump(mode="json"), 1 if result.status == "blocked" else 0


def _status(args: argparse.Namespace) -> tuple[dict[str, Any], int]:
    verification = verify_baseline_campaign(
        args.campaign,
        repository_root=REPOSITORY_ROOT,
        cases_path=args.cases,
        dataset_lock_path=args.dataset_lock,
        profile_path=args.profile,
    )
    metadata = _load_campaign_metadata(args.campaign)
    status = (
        "blocked"
        if not verification.ok
        else ("complete" if verification.attempt_count == metadata.max_attempts else "active")
    )
    payload = {
        "campaign_id": metadata.campaign_id,
        "status": status,
        "attempt_count": verification.attempt_count,
        "pending_count": verification.pending_count,
        "expected_attempts": metadata.max_attempts,
        "budget_cny": str(metadata.budget_cny),
        "spent_cny": str(verification.spent_cny),
        "dataset_fingerprint": metadata.dataset_fingerprint,
        "harness_fingerprint": metadata.harness_fingerprint,
        "provider": metadata.profile.provider,
        "model": metadata.profile.model,
        "transport": metadata.profile.transport,
        "blockers": verification.blockers,
    }
    return payload, 0 if verification.ok else 1


def _recover(args: argparse.Namespace) -> tuple[dict[str, Any], int]:
    result = recover_baseline_campaign(
        args.campaign,
        repository_root=REPOSITORY_ROOT,
        cases_path=args.cases,
        dataset_lock_path=args.dataset_lock,
        profile_path=args.profile,
    )
    return result.model_dump(mode="json"), 1 if result.status == "blocked" else 0


def _report(args: argparse.Namespace) -> tuple[dict[str, Any], int]:
    report = generate_baseline_report(
        args.campaign,
        repository_root=REPOSITORY_ROOT,
        cases_path=args.cases,
        dataset_lock_path=args.dataset_lock,
        profile_path=args.profile,
    )
    return report.model_dump(mode="json"), 1 if report.status == "blocked" else 0


def _verify(args: argparse.Namespace) -> tuple[dict[str, Any], int]:
    result = verify_baseline_campaign(
        args.campaign,
        repository_root=REPOSITORY_ROOT,
        cases_path=args.cases,
        dataset_lock_path=args.dataset_lock,
        profile_path=args.profile,
    )
    return result.model_dump(mode="json"), 0 if result.ok else 1


def _load_campaign_metadata(campaign: Path) -> BaselineCampaignMetadata:
    return BaselineCampaignMetadata.model_validate_json((campaign / "campaign.json").read_bytes())


class _AlreadyReportedError(Exception):
    pass
