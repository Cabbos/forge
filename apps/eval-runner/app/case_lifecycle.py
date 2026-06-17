from collections import Counter

from app.models import CaseLifecycleIssue, CaseLifecycleReport, EvaluationTask

VALID_STATUSES = {"active", "flaky", "quarantined", "retired"}


def inspect_case_lifecycle(tasks: list[EvaluationTask]) -> CaseLifecycleReport:
    counts: Counter[str] = Counter()
    issues: list[CaseLifecycleIssue] = []
    for task in tasks:
        lifecycle = task.metadata.get("lifecycle", {})
        lifecycle = lifecycle if isinstance(lifecycle, dict) else {}
        status = str(lifecycle.get("status", "active"))
        counts[status] += 1
        if status not in VALID_STATUSES:
            issues.append(
                CaseLifecycleIssue(
                    task_id=task.id,
                    code="invalid_lifecycle_status",
                    message=f"Unknown lifecycle status: {status}",
                )
            )
        if not lifecycle.get("owner"):
            issues.append(
                CaseLifecycleIssue(
                    task_id=task.id,
                    code="missing_owner",
                    message="Case lifecycle metadata should include owner.",
                )
            )
        if status == "quarantined":
            issues.append(
                CaseLifecycleIssue(
                    task_id=task.id,
                    code="quarantined_case",
                    message=str(lifecycle.get("reason") or "Case is quarantined."),
                )
            )
    return CaseLifecycleReport(counts=dict(counts), issues=issues)
