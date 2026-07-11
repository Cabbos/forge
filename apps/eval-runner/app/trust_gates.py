from app.models import (
    AgentTrace,
    CaseQualityIssue,
    EvalProvider,
    ScoreCoverage,
    TrustGateResult,
    TrustStatus,
    WorkspaceCheck,
)


def evaluate_trust_gates(
    *,
    harness_check: WorkspaceCheck | None,
    dataset_fingerprint: str | None,
    case_quality_issues: list[CaseQualityIssue],
    traces: list[AgentTrace],
    scorer_calibrated: bool | None,
    red_team_passed: bool | None,
    require_red_team: bool,
    score_coverage: dict[str, ScoreCoverage],
) -> TrustGateResult:
    blockers: list[str] = []
    unknown = False
    if harness_check is None:
        blockers.append("harness_evidence_missing")
        unknown = True
    elif not harness_check.ok:
        blockers.append("harness_untrusted")
    if not dataset_fingerprint:
        blockers.append("dataset_unfingerprinted")
        unknown = True
    for issue in case_quality_issues:
        blockers.append(f"case_quality:{issue.task_id}:{issue.code}")
    for trace in traces:
        observation = trace.workspace_observation
        if observation is None or not observation.available:
            blockers.append(f"workspace_evidence_missing:{trace.task_id}")
            unknown = True
        if trace.sandbox_scrub is not None and not trace.sandbox_scrub.ok:
            blockers.append(f"sandbox_untrusted:{trace.task_id}")
        if trace.provider == EvalProvider.FORGE:
            if trace.patch_replay is None:
                blockers.append(f"patch_replay_missing:{trace.task_id}")
                unknown = True
            elif not trace.patch_replay.ok:
                blockers.append(f"patch_replay_failed:{trace.task_id}")
    if scorer_calibrated is None:
        blockers.append("scorer_calibration_missing")
        unknown = True
    elif not scorer_calibrated:
        blockers.append("scorer_uncalibrated")
    if require_red_team:
        if red_team_passed is None:
            blockers.append("red_team_evidence_missing")
            unknown = True
        elif not red_team_passed:
            blockers.append("red_team_failed")
    required_names = sorted({name for trace in traces for name in trace.required_scores})
    for name in required_names:
        aggregate = score_coverage.get(name)
        if aggregate is None or aggregate.coverage < 1.0:
            blockers.append(f"score_coverage_incomplete:{name}")
            unknown = True
    blockers = sorted(set(blockers))
    if not blockers:
        return TrustGateResult(status=TrustStatus.TRUSTED, trusted=True)
    return TrustGateResult(
        status=TrustStatus.UNKNOWN if unknown else TrustStatus.UNTRUSTED,
        trusted=False,
        blockers=blockers,
    )
