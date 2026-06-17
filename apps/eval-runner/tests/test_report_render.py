from app.models import BacktestReport


def sample_report() -> BacktestReport:
    return BacktestReport(
        total_tasks=2,
        success_rate=0.5,
        verification_pass_rate=0.5,
        scope_violation_rate=0.0,
        avg_duration_ms=20.0,
        avg_model_rounds=2.0,
        avg_confirm_requests=1.0,
        failure_categories={"verification_failed": 1},
        score_summary={"functional_correctness": 0.5, "scope_ok": 1.0},
        tasks=[],
    )


def test_render_markdown_report_contains_operator_summary():
    from app.report_render import render_markdown_report

    markdown = render_markdown_report(sample_report(), title="Local Regression")

    assert "# Local Regression" in markdown
    assert "| success_rate | 0.500 |" in markdown
    assert "verification_failed" in markdown
    assert "functional_correctness" in markdown


def test_render_html_report_escapes_title():
    from app.report_render import render_html_report

    html = render_html_report(sample_report(), title="<release>")

    assert "&lt;release&gt;" in html
    assert "<table" in html
