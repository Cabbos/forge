import argparse
import json
import subprocess
import sys
from decimal import Decimal
from pathlib import Path
from typing import Any

from app.campaign import (
    accept_campaign_patch,
    finalize_campaign,
    initialize_campaign,
    recover_pending_accept,
)
from app.campaign_verify import verify_campaign
from app.cases import CaseLoadError, load_cases
from app.forgebench import (
    DEFAULT_CASES,
    DEFAULT_DATASET_LOCK,
    FORGEBENCH_ROOT,
)
from app.provenance import load_verified_entries, sha256_bytes, verify_ledger
from app.provenance_models import SECRET_PATTERN

DEFAULT_FIXTURE = FORGEBENCH_ROOT / "forgebench-monorepo"
DEFAULT_FIXTURE_LOCK = FORGEBENCH_ROOT / "fixture-lock.json"


class CampaignCliInputError(ValueError):
    pass


def campaign_main(argv: list[str]) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    try:
        if args.command == "init":
            payload = _initialize(args)
            exit_code = 0
        elif args.command == "accept":
            payload = _accept(args)
            exit_code = 0
        elif args.command == "status":
            payload, exit_code = _status(args.campaign)
        elif args.command == "finalize":
            _require_campaign(args.campaign)
            payload = finalize_campaign(args.campaign)
            exit_code = 0
        elif args.command == "verify":
            if not args.campaign.is_dir():
                raise CampaignCliInputError("campaign path does not exist")
            result = verify_campaign(args.campaign)
            payload = result.model_dump(mode="json")
            exit_code = 0 if result.ok else 1
        elif args.command == "recover":
            _require_campaign(args.campaign)
            payload = recover_pending_accept(args.campaign)
            exit_code = 0
        else:
            raise CampaignCliInputError("unknown campaign command")
    except CampaignCliInputError:
        print("error: campaign input is invalid", file=sys.stderr)
        return 2
    except (CaseLoadError, OSError, json.JSONDecodeError, ValueError, RuntimeError):
        print(
            json.dumps(
                {"ok": False, "blockers": [f"campaign_{args.command}_rejected"]},
                sort_keys=True,
            )
        )
        return 1

    print(json.dumps(payload, sort_keys=True))
    return exit_code


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="python -m app.cli campaign",
        description="Manage a machine-verifiable Forge integration campaign.",
    )
    commands = parser.add_subparsers(dest="command", required=True)

    init = commands.add_parser("init")
    _add_campaign_argument(init)
    init.add_argument("--campaign-id")
    init.add_argument("--fixture", type=Path, default=DEFAULT_FIXTURE)
    init.add_argument("--fixture-lock", type=Path, default=DEFAULT_FIXTURE_LOCK)
    init.add_argument("--cases", type=Path, default=DEFAULT_CASES)
    init.add_argument("--dataset-lock", type=Path, default=DEFAULT_DATASET_LOCK)
    init.add_argument("--harness-profile", type=Path, required=True)
    init.add_argument("--controller-version", default="forge-eval-controller/1")
    init.add_argument("--runtime-version", default="forge-runtime/1")

    accept = commands.add_parser("accept")
    _add_campaign_argument(accept)
    accept.add_argument("--cases", type=Path, default=DEFAULT_CASES)
    accept.add_argument("--task-id", required=True)
    accept.add_argument("--trace", type=Path, required=True)
    accept.add_argument("--report", type=Path, required=True)
    accept.add_argument("--patch", type=Path, required=True)

    for name in ("status", "finalize", "verify", "recover"):
        command = commands.add_parser(name)
        _add_campaign_argument(command)
    return parser


def _add_campaign_argument(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--campaign", type=Path, required=True)


def _initialize(args: argparse.Namespace) -> dict[str, Any]:
    required_files = (
        args.fixture_lock,
        args.dataset_lock,
        args.harness_profile,
    )
    if not args.fixture.is_dir() or not args.cases.is_dir():
        raise CampaignCliInputError("fixture or cases path does not exist")
    if any(not path.is_file() for path in required_files):
        raise CampaignCliInputError("campaign lock input does not exist")
    profile = _load_json_object(args.harness_profile)
    identity = {}
    for key in ("provider", "model", "transport"):
        value = profile.get(key)
        if not isinstance(value, str) or not value:
            raise CampaignCliInputError("harness profile identity is incomplete")
        identity[key] = value
    metadata = initialize_campaign(
        args.campaign,
        campaign_id=args.campaign_id or args.campaign.name,
        fixture=args.fixture,
        fixture_lock=args.fixture_lock,
        cases=args.cases,
        dataset_lock=args.dataset_lock,
        harness_profile=args.harness_profile,
        provider=identity["provider"],
        model=identity["model"],
        transport=identity["transport"],
        controller_version=args.controller_version,
        runtime_version=args.runtime_version,
    )
    return {
        "campaign_id": metadata["campaign_id"],
        "status": metadata["status"],
        "baseline_commit": metadata["baseline_commit"],
        "fixture_tree_sha256": metadata["fixture_tree_sha256"],
        "dataset_fingerprint": metadata["dataset_fingerprint"],
        "harness_profile_sha256": metadata["harness_profile_sha256"],
        "provider": metadata["provider"],
        "model": metadata["model"],
        "transport": metadata["transport"],
    }


def _accept(args: argparse.Namespace) -> dict[str, str]:
    _require_campaign(args.campaign)
    for path in (args.trace, args.report, args.patch):
        if not path.is_file():
            raise CampaignCliInputError("explicit campaign artifact does not exist")
    if not args.cases.is_dir():
        raise CampaignCliInputError("case catalog does not exist")
    tasks = [task for task in load_cases(args.cases) if task.id == args.task_id]
    if len(tasks) != 1:
        raise CampaignCliInputError("task id is not unique in case catalog")
    trace_sha256 = sha256_bytes(args.trace.read_bytes())
    report_sha256 = sha256_bytes(args.report.read_bytes())
    patch_sha256 = sha256_bytes(args.patch.read_bytes())
    result = accept_campaign_patch(
        args.campaign,
        task=tasks[0],
        trace_path=args.trace,
        report_path=args.report,
        patch_path=args.patch,
        expected_trace_sha256=trace_sha256,
        expected_report_sha256=report_sha256,
        expected_patch_sha256=patch_sha256,
    )
    return {
        **result,
        "trace_sha256": trace_sha256,
        "report_sha256": report_sha256,
        "patch_sha256": patch_sha256,
    }


def _status(campaign: Path) -> tuple[dict[str, Any], int]:
    metadata = _require_campaign(campaign)
    ledger_path = campaign / "ledger.jsonl"
    artifacts_root = campaign / "artifacts"
    result = verify_ledger(ledger_path, artifacts_root)
    entries = load_verified_entries(ledger_path, artifacts_root) if result.ok else []
    current_commit = _git_or_input_error(campaign / "worktree", "rev-parse", "HEAD")
    payload = {
        "campaign_id": metadata.get("campaign_id"),
        "status": metadata.get("status"),
        "baseline_commit": metadata.get("baseline_commit"),
        "current_commit": current_commit,
        "provider": metadata.get("provider"),
        "model": metadata.get("model"),
        "transport": metadata.get("transport"),
        "ledger_verified": result.ok,
        "entry_count": len(entries),
        "task_ids": [entry.task_id for entry in entries],
        "run_ids": [entry.run_id for entry in entries],
        "total_cost_cny": str(sum((entry.cost_cny for entry in entries), start=Decimal("0"))),
        "pending_accept": (campaign / "pending-accept.json").exists(),
        "manifest_exists": (campaign / "worktree/FORGE-BUILD-MANIFEST.json").is_file(),
        "blockers": result.blockers,
    }
    return payload, 0 if result.ok else 1


def _require_campaign(campaign: Path) -> dict[str, Any]:
    if not campaign.is_dir():
        raise CampaignCliInputError("campaign path does not exist")
    metadata_path = campaign / "campaign.json"
    if not metadata_path.is_file():
        raise CampaignCliInputError("campaign metadata does not exist")
    try:
        content = metadata_path.read_text(encoding="utf-8")
        if SECRET_PATTERN.search(content) is not None:
            raise CampaignCliInputError("campaign metadata contains secret-like content")
        payload = json.loads(content)
        if not isinstance(payload, dict):
            raise CampaignCliInputError("campaign metadata must be an object")
        return payload
    except (OSError, json.JSONDecodeError) as exc:
        raise CampaignCliInputError("campaign metadata is malformed") from exc


def _load_json_object(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise CampaignCliInputError("JSON input must be an object")
    return payload


def _git_or_input_error(worktree: Path, *args: str) -> str:
    try:
        completed = subprocess.run(
            ["git", *args],
            cwd=worktree,
            text=True,
            capture_output=True,
            check=False,
        )
    except OSError as exc:
        raise CampaignCliInputError("campaign worktree is invalid") from exc
    if completed.returncode != 0:
        raise CampaignCliInputError("campaign worktree is invalid")
    return completed.stdout.strip()
