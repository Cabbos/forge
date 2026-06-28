from app.models import TrustGateResult


def evaluate_trust_gates(
    *,
    harness_ok: bool,
    dataset_fingerprint: str | None,
    scorer_calibrated: bool,
    red_team_passed: bool,
) -> TrustGateResult:
    blockers: list[str] = []
    if not harness_ok:
        blockers.append("harness_untrusted")
    if not dataset_fingerprint:
        blockers.append("dataset_unfingerprinted")
    if not scorer_calibrated:
        blockers.append("scorer_uncalibrated")
    if not red_team_passed:
        blockers.append("red_team_failed")
    return TrustGateResult(trusted=not blockers, blockers=blockers)
