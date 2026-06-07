from collections import Counter

from app.models import AgentTrace, FailureCategory, MetricsSummary, TaskMetric


def trace_passed(trace: AgentTrace) -> bool:
    return (
        trace.verification_result is not None
        and trace.verification_result.passed
        and trace.error is None
        and not trace.scope_violations
    )


def calculate_metrics(traces: list[AgentTrace]) -> MetricsSummary:
    total = len(traces)
    if total == 0:
        return MetricsSummary(
            total_tasks=0,
            passed_tasks=0,
            failed_tasks=0,
            success_rate=0.0,
            verification_coverage=0.0,
            average_tool_calls=0.0,
            average_model_rounds=0.0,
            average_confirm_requests=0.0,
            average_duration_ms=0.0,
            scope_violation_count=0,
            failure_categories={},
            tasks=[],
        )

    task_metrics: list[TaskMetric] = []
    failures: Counter[str] = Counter()

    for trace in traces:
        passed = trace_passed(trace)
        verification_passed = (
            trace.verification_result.passed if trace.verification_result is not None else None
        )
        category = trace.failure_category
        if trace.scope_violations:
            category = FailureCategory.SCOPE_VIOLATION
        if not passed:
            if category == FailureCategory.NONE:
                category = FailureCategory.RUNNER_ERROR
            failures[category.value] += 1

        task_metrics.append(
            TaskMetric(
                task_id=trace.task_id,
                passed=passed,
                verification_passed=verification_passed,
                scope_ok=not trace.scope_violations,
                changed_files=len(trace.changed_files),
                tool_calls=len(trace.tool_calls),
                model_rounds=trace.model_rounds,
                confirm_requests=trace.confirm_requests,
                repair_attempts_used=trace.repair_attempts_used,
                validation_attempts=trace.validation_attempts,
                duration_ms=trace.duration_ms,
                failure_category=category,
            )
        )

    passed_tasks = sum(1 for task in task_metrics if task.passed)
    verified_tasks = sum(1 for trace in traces if trace.verification_result is not None)

    return MetricsSummary(
        total_tasks=total,
        passed_tasks=passed_tasks,
        failed_tasks=total - passed_tasks,
        success_rate=passed_tasks / total,
        verification_coverage=verified_tasks / total,
        average_tool_calls=sum(len(trace.tool_calls) for trace in traces) / total,
        average_model_rounds=sum(trace.model_rounds for trace in traces) / total,
        average_confirm_requests=sum(trace.confirm_requests for trace in traces) / total,
        average_repair_attempts_used=sum(trace.repair_attempts_used for trace in traces) / total,
        average_validation_attempts=sum(trace.validation_attempts for trace in traces) / total,
        average_duration_ms=sum(trace.duration_ms for trace in traces) / total,
        scope_violation_count=sum(len(trace.scope_violations) for trace in traces),
        failure_categories=dict(failures),
        tasks=task_metrics,
    )
