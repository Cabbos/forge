from typing import Any


def render_ci_summary(comparison: dict[str, list[dict[str, Any]]]) -> str:
    regressions = comparison.get("regressions", [])
    lines = [
        "## Forge Eval CI Summary",
        "",
        "| Metric | Severity | Previous | Current | Delta |",
        "|---|---|---:|---:|---:|",
    ]
    if not regressions:
        lines.append("| none | pass | 0.000 | 0.000 | 0.000 |")
    for item in regressions:
        lines.append(
            "| {metric} | {severity} | {previous:.3f} | {current:.3f} | {delta:.3f} |".format(
                metric=item["metric"],
                severity=item["severity"],
                previous=float(item["previous"]),
                current=float(item["current"]),
                delta=float(item["delta"]),
            )
        )
    return "\n".join(lines) + "\n"
