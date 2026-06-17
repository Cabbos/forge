from app.models import FlakeTriageItem, FlakeTriageReport


def triage_trial_metrics(
    trial_metrics: dict[str, dict[str, float | int | bool]],
) -> FlakeTriageReport:
    items: list[FlakeTriageItem] = []
    quarantine_candidates: list[str] = []
    for task_id, metrics in sorted(trial_metrics.items()):
        attempts = int(metrics["attempts"])
        pass_rate = float(metrics["pass_rate"])
        flaky = bool(metrics.get("flaky", False))
        if flaky:
            classification = "flaky"
            quarantine_candidates.append(task_id)
        elif pass_rate == 0.0:
            classification = "stable_fail"
        elif pass_rate == 1.0:
            classification = "stable_pass"
        else:
            classification = "needs_review"
            quarantine_candidates.append(task_id)
        items.append(
            FlakeTriageItem(
                task_id=task_id,
                attempts=attempts,
                pass_rate=pass_rate,
                classification=classification,
            )
        )
    return FlakeTriageReport(
        items=items,
        quarantine_candidates=quarantine_candidates,
    )
