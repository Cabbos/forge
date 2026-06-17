from html import escape

from app.models import BacktestReport


def render_markdown_report(
    report: BacktestReport,
    *,
    title: str = "Forge Eval Report",
) -> str:
    lines = [
        f"# {title}",
        "",
        "| Metric | Value |",
        "|---|---:|",
        f"| total_tasks | {report.total_tasks} |",
        f"| success_rate | {report.success_rate:.3f} |",
        f"| verification_pass_rate | {report.verification_pass_rate:.3f} |",
        f"| scope_violation_rate | {report.scope_violation_rate:.3f} |",
        f"| avg_model_rounds | {report.avg_model_rounds:.3f} |",
        f"| total_cost_usd | {report.total_cost_usd:.3f} |",
        "",
        "## Failure Categories",
        "",
    ]
    if report.failure_categories:
        lines.extend(
            f"- `{name}`: {count}"
            for name, count in sorted(report.failure_categories.items())
        )
    else:
        lines.append("- None")

    lines.extend(["", "## Score Summary", ""])
    if report.score_summary:
        lines.extend(
            f"- `{name}`: {score:.3f}"
            for name, score in sorted(report.score_summary.items())
        )
    else:
        lines.append("- None")
    return "\n".join(lines) + "\n"


def render_html_report(
    report: BacktestReport,
    *,
    title: str = "Forge Eval Report",
) -> str:
    rows = [
        ("total_tasks", str(report.total_tasks)),
        ("success_rate", f"{report.success_rate:.3f}"),
        ("verification_pass_rate", f"{report.verification_pass_rate:.3f}"),
        ("scope_violation_rate", f"{report.scope_violation_rate:.3f}"),
        ("avg_model_rounds", f"{report.avg_model_rounds:.3f}"),
        ("total_cost_usd", f"{report.total_cost_usd:.3f}"),
    ]
    row_html = "\n".join(
        f"<tr><th>{escape(name)}</th><td>{escape(value)}</td></tr>"
        for name, value in rows
    )
    score_items = "\n".join(
        f"<li><code>{escape(name)}</code>: {score:.3f}</li>"
        for name, score in sorted(report.score_summary.items())
    ) or "<li>None</li>"
    failure_items = "\n".join(
        f"<li><code>{escape(name)}</code>: {count}</li>"
        for name, count in sorted(report.failure_categories.items())
    ) or "<li>None</li>"
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>{escape(title)}</title>
  <style>
    body {{ font-family: system-ui, sans-serif; margin: 32px; color: #111; }}
    table {{ border-collapse: collapse; min-width: 420px; }}
    th, td {{ border: 1px solid #ccc; padding: 8px 10px; text-align: left; }}
  </style>
</head>
<body>
  <h1>{escape(title)}</h1>
  <table>{row_html}</table>
  <h2>Failure Categories</h2>
  <ul>{failure_items}</ul>
  <h2>Score Summary</h2>
  <ul>{score_items}</ul>
</body>
</html>
"""
