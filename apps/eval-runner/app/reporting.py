from collections import Counter
from typing import Any

from app.metrics import calculate_metrics
from app.models import AgentTrace, BacktestReport, ContinuityReport, TraceSummary


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
            continuity=None,
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
            repair_attempts_used=trace.repair_attempts_used,
            validation_attempts=trace.validation_attempts,
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
        avg_repair_attempts_used=metrics.average_repair_attempts_used,
        avg_validation_attempts=metrics.average_validation_attempts,
        failure_categories=metrics.failure_categories,
        tasks=task_summaries,
        continuity=build_continuity_report(traces),
    )


def build_continuity_report(traces: list[AgentTrace]) -> ContinuityReport | None:
    total_tasks = len(traces)
    if total_tasks == 0:
        return None

    saw_continuity_signal = False
    tasks_with_db = 0
    tasks_with_formed_experiences = 0
    tasks_recall_ready = 0
    tasks_with_reflection_episodes = 0
    total_experiences = 0
    total_fts_rows = 0
    total_formed_reflections = 0
    total_reflection_episodes = 0
    covered_reflection_episodes = 0
    notable_failure_count = 0
    status_counts: Counter[str] = Counter()
    kind_counts: Counter[str] = Counter()

    for trace in traces:
        headless = latest_event(trace.raw_events, "eval_headless_continuity_diagnostic")
        diagnostic = latest_event(trace.raw_events, "eval_continuity_db_diagnostic")
        saw_continuity_signal = (
            saw_continuity_signal or headless is not None or diagnostic is not None
        )

        formed_count = int_value(headless, "formed_count")
        formed_reflection_count = int_value(diagnostic, "formed_reflection_count")
        if (formed_count or 0) > 0 or (formed_reflection_count or 0) > 0:
            tasks_with_formed_experiences += 1

        if not diagnostic or diagnostic.get("exists") is not True:
            continue

        tasks_with_db += 1
        experience_count = int_value(diagnostic, "experience_count") or 0
        fts_count = int_value(diagnostic, "fts_count") or 0
        formed_reflections = formed_reflection_count or 0
        reflection_episodes = list_value(diagnostic.get("reflection_episodes"))

        total_experiences += experience_count
        total_fts_rows += fts_count
        total_formed_reflections += formed_reflections
        if experience_count > 0 and fts_count > 0:
            tasks_recall_ready += 1
        if reflection_episodes:
            tasks_with_reflection_episodes += 1
        total_reflection_episodes += len(reflection_episodes)

        status_counts.update(counter_dict(diagnostic.get("experience_status_counts")))
        kind_counts.update(counter_dict(diagnostic.get("experience_kind_counts")))

        for episode in reflection_episodes:
            if not isinstance(episode, dict):
                continue
            changed_files = list_value(episode.get("changed_files"))
            if episode.get("user_goal_summary") and changed_files:
                covered_reflection_episodes += 1
            notable_failure_count += len(list_value(episode.get("notable_failures")))

    if not saw_continuity_signal:
        return None

    return ContinuityReport(
        tasks_with_db=tasks_with_db,
        tasks_with_formed_experiences=tasks_with_formed_experiences,
        formation_rate=tasks_with_formed_experiences / total_tasks,
        recall_ready_rate=tasks_recall_ready / total_tasks,
        total_experiences=total_experiences,
        total_fts_rows=total_fts_rows,
        total_formed_reflections=total_formed_reflections,
        total_reflection_episodes=total_reflection_episodes,
        reflection_episode_rate=tasks_with_reflection_episodes / total_tasks,
        reflection_coverage_rate=(
            covered_reflection_episodes / total_reflection_episodes
            if total_reflection_episodes
            else 0.0
        ),
        notable_failure_count=notable_failure_count,
        experience_status_counts=dict(status_counts),
        experience_kind_counts=dict(kind_counts),
    )


def latest_event(events: list[dict[str, Any]], event_type: str) -> dict[str, Any] | None:
    for event in reversed(events):
        if event.get("event_type") == event_type:
            return event
    return None


def int_value(source: dict[str, Any] | None, key: str) -> int | None:
    if source is None:
        return None
    value = source.get(key)
    if value is None:
        return None
    return int(value)


def list_value(value: Any) -> list[Any]:
    return value if isinstance(value, list) else []


def counter_dict(value: Any) -> dict[str, int]:
    if not isinstance(value, dict):
        return {}
    return {str(key): int(count) for key, count in value.items()}
