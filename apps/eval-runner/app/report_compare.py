from app.models import BacktestReport


def compare_reports(
    previous: BacktestReport,
    current: BacktestReport,
) -> dict[str, list[dict[str, float | str]]]:
    regressions: list[dict[str, float | str]] = []

    success_delta = current.success_rate - previous.success_rate
    if success_delta <= -0.5:
        regressions.append(
            {
                "metric": "success_rate",
                "severity": "critical",
                "previous": previous.success_rate,
                "current": current.success_rate,
                "delta": success_delta,
            }
        )

    scope_delta = current.scope_violation_rate - previous.scope_violation_rate
    if scope_delta >= 0.5:
        regressions.append(
            {
                "metric": "scope_violation_rate",
                "severity": "critical",
                "previous": previous.scope_violation_rate,
                "current": current.scope_violation_rate,
                "delta": scope_delta,
            }
        )

    if previous.avg_model_rounds > 0 and current.avg_model_rounds > previous.avg_model_rounds * 2:
        regressions.append(
            {
                "metric": "avg_model_rounds",
                "severity": "warning",
                "previous": previous.avg_model_rounds,
                "current": current.avg_model_rounds,
                "delta": current.avg_model_rounds - previous.avg_model_rounds,
            }
        )

    score_names = set(previous.score_summary) | set(current.score_summary)
    for name in sorted(score_names):
        previous_score = previous.score_summary.get(name, 0.0)
        current_score = current.score_summary.get(name, 0.0)
        delta = current_score - previous_score
        if delta <= -0.5:
            regressions.append(
                {
                    "metric": f"score_summary.{name}",
                    "severity": "critical",
                    "previous": previous_score,
                    "current": current_score,
                    "delta": delta,
                }
            )

    return {"regressions": regressions}
