"""Offline aggregation gate for a Forge frontier release claim.

The scoring and integration tracks are intentionally separate while they run.
This gate is the only place that combines them: a passing holdout report must
match the locked profile/dataset in a verified Forge-built campaign whose final
full verification also passes.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Literal

from app.campaign_verify import CampaignVerification, verify_campaign
from app.harness_profiles import (
    HarnessProfileError,
    harness_profile_fingerprint,
    load_harness_profile,
)
from app.holdout_gate import (
    HoldoutGateDecisionV1,
    HoldoutGateReportV1,
    evaluate_holdout_gate,
)
from app.models import EvalModel
from app.provenance_models import Sha256


class FrontierReleaseDecisionV1(EvalModel):
    """The machine-readable boundary for combining scoring and provenance."""

    schema_version: Literal[1] = 1
    passed: bool
    blockers: list[str]
    holdout_gate: HoldoutGateDecisionV1
    integration: CampaignVerification
    profile_id: str | None = None
    dataset_fingerprint: Sha256 | None = None
    harness_fingerprint: Sha256 | None = None


def evaluate_frontier_release(
    report: HoldoutGateReportV1,
    campaign_root: Path,
    *,
    run_full_verification: bool = True,
) -> FrontierReleaseDecisionV1:
    """Combine independently verified holdout and Forge-built evidence."""

    holdout_gate = evaluate_holdout_gate(report)
    integration = verify_campaign(
        campaign_root,
        run_full_verification=run_full_verification,
    )
    blockers = [f"holdout:{item}" for item in holdout_gate.blockers]
    blockers.extend(f"integration:{item}" for item in integration.blockers)
    profile_id: str | None = None
    dataset_fingerprint: str | None = None
    harness_fingerprint: str | None = None

    metadata = _load_campaign_metadata(campaign_root)
    profile_path = campaign_root / "locks" / "harness-profile.json"
    try:
        profile = load_harness_profile(profile_path)
    except (HarnessProfileError, OSError, ValueError):
        profile = None
        blockers.append("integration:harness_profile_invalid")

    if metadata is None:
        blockers.append("integration:campaign_metadata_invalid")
    else:
        dataset_fingerprint = _string_or_none(metadata.get("holdout_dataset_fingerprint"))
        if dataset_fingerprint != report.dataset_fingerprint:
            blockers.append("identity:holdout_dataset_fingerprint_mismatch")

    if profile is not None:
        profile_id = profile.profile_id
        harness_fingerprint = harness_profile_fingerprint(profile)
        if profile_id != report.profile_id:
            blockers.append("identity:profile_id_mismatch")
        if profile.provider != report.provider:
            blockers.append("identity:provider_mismatch")
        if profile.model != report.model:
            blockers.append("identity:model_mismatch")
        if profile.transport != report.transport:
            blockers.append("identity:transport_mismatch")
        if harness_fingerprint != report.harness_fingerprint:
            blockers.append("identity:harness_fingerprint_mismatch")

    if not integration.ok:
        blockers.append("integration:verification_failed")
    elif integration.full_verification_passed is not True:
        blockers.append("integration:full_verification_missing")

    blockers = sorted(set(blockers))
    return FrontierReleaseDecisionV1(
        passed=not blockers,
        blockers=blockers,
        holdout_gate=holdout_gate,
        integration=integration,
        profile_id=profile_id,
        dataset_fingerprint=dataset_fingerprint,
        harness_fingerprint=harness_fingerprint,
    )


def _load_campaign_metadata(campaign_root: Path) -> dict[str, object] | None:
    try:
        payload = json.loads((campaign_root / "campaign.json").read_bytes())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return None
    return payload if isinstance(payload, dict) else None


def _string_or_none(value: object) -> str | None:
    return value if isinstance(value, str) else None
