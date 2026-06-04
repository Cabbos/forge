from app.metrics import calculate_metrics
from app.models import AgentTrace, BacktestReport, TraceSummary


def build_report(traces: list[AgentTrace]) -> BacktestReport:
    metrics = calculate_metrics(traces)
    total = metrics.total_tasks
    if total == 0:
        return BacktestReport(
            total_tasks=0,
            success_rate=0.0,
            verification_pass_rate=0.0,
            scope_violation_rate=0.0,
            avg_duration_ms=0.0,
            avg_model_rounds=0.0,
            avg_confirm_requests=0.0,
            failure_categories={},
            tasks=[],
        )

    verification_passed = sum(
        1
        for trace in traces
        if trace.verification_result is not None and trace.verification_result.passed
    )
    scope_violations = sum(1 for trace in traces if trace.scope_violations)
    task_summaries = [
        TraceSummary(
            task_id=trace.task_id,
            passed=task_metric.passed,
            verification_passed=task_metric.verification_passed,
            scope_violations=trace.scope_violations,
            changed_files=trace.changed_files,
            duration_ms=trace.duration_ms,
            model_rounds=trace.model_rounds,
            confirm_requests=trace.confirm_requests,
            failure_category=task_metric.failure_category,
            failure_reason=trace.failure_reason,
            error=trace.error,
        )
        for trace, task_metric in zip(traces, metrics.tasks, strict=False)
    ]

    return BacktestReport(
        total_tasks=total,
        success_rate=metrics.success_rate,
        verification_pass_rate=verification_passed / total,
        scope_violation_rate=scope_violations / total,
        avg_duration_ms=metrics.average_duration_ms,
        avg_model_rounds=metrics.average_model_rounds,
        avg_confirm_requests=metrics.average_confirm_requests,
        failure_categories=metrics.failure_categories,
        tasks=task_summaries,
    )
