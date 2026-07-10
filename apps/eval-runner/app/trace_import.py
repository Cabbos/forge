import json
from pathlib import Path
from typing import Any

from pydantic import TypeAdapter

from app.models import AgentTrace, EvaluationTask, FailureCategory, ForgeRunEvidence


def case_from_trace(trace: AgentTrace) -> EvaluationTask:
    verification_command = (
        trace.verification_result.command if trace.verification_result is not None else None
    )
    metadata: dict[str, Any] = {
        "source": "trace",
        "failure_reason": trace.failure_reason,
        "failure_category": trace.failure_category.value,
    }
    if trace.forge_run_evidence is not None:
        evidence = trace.forge_run_evidence
        metadata["forge_run_evidence"] = evidence.model_dump(mode="json")
        if evidence.normalized_goal:
            metadata["normalized_goal"] = evidence.normalized_goal
        if evidence.failure_category:
            metadata["failure_category"] = evidence.failure_category

    return EvaluationTask(
        id=trace.task_id,
        title=f"Promoted trace: {trace.task_id}",
        prompt=trace.user_prompt,
        context_files=trace.context_files,
        verification_command=verification_command,
        expected_success=False,
        expected_files_changed=trace.expected_files_changed,
        forbidden_files_changed=trace.forbidden_files_changed,
        metadata=metadata,
    )


def load_traces(trace_path: Path) -> list[AgentTrace]:
    payload = json.loads(trace_path.read_text(encoding="utf-8"))
    raw_traces = normalize_trace_payload(payload)
    return TypeAdapter(list[AgentTrace]).validate_python(raw_traces)


def normalize_trace_payload(payload: Any) -> list[dict[str, Any]]:
    if isinstance(payload, list):
        return [normalize_trace_object(item) for item in payload if isinstance(item, dict)]
    if not isinstance(payload, dict):
        return []
    if isinstance(payload.get("traces"), list):
        return [
            normalize_trace_object(item) for item in payload["traces"] if isinstance(item, dict)
        ]
    return [normalize_trace_object(payload)]


def normalize_trace_object(raw: dict[str, Any]) -> dict[str, Any]:
    trace = {key: value for key, value in raw.items() if key in AgentTrace.model_fields}
    trace.setdefault("context_files", [])
    trace.setdefault("raw_events", [])
    trace.setdefault("tool_calls", [])
    trace.setdefault("shell_outputs", [])
    trace.setdefault("file_diffs", [])
    trace.setdefault("changed_files", [])
    trace.setdefault("scope_violations", [])
    trace.setdefault("expected_files_changed", [])
    trace.setdefault("forbidden_files_changed", [])
    trace.setdefault("final_answer", "")
    trace.setdefault("model_rounds", 0)
    trace.setdefault("confirm_requests", 0)
    trace.setdefault("repair_attempts_used", 0)
    trace.setdefault("validation_attempts", 0)
    trace.setdefault("duration_ms", int(raw.get("duration_ms") or 0))
    trace.setdefault("started_at", raw.get("started_at") or "1970-01-01T00:00:00+00:00")
    trace.setdefault("ended_at", raw.get("ended_at") or "1970-01-01T00:00:00+00:00")
    if trace.get("user_prompt") is None:
        trace["user_prompt"] = str(raw.get("prompt") or "")
    if trace.get("task_id") is None:
        trace["task_id"] = str(raw.get("task_id") or raw.get("session_id") or "forge-trace")
    if trace.get("provider") is None:
        trace["provider"] = str(raw.get("provider") or "forge")
    if trace.get("model") is None:
        trace["model"] = str(raw.get("model") or "local-forge")
    trace["failure_category"] = normalize_failure_category(raw.get("failure_category"))
    if raw.get("forge_run_evidence") is not None or is_desktop_trace_payload(raw):
        trace["forge_run_evidence"] = normalize_forge_run_evidence(raw)
    return trace


def normalize_failure_category(value: Any) -> str:
    raw = str(value or FailureCategory.NONE)
    return raw if raw in FailureCategory._value2member_map_ else FailureCategory.RUNNER_ERROR.value


def normalize_forge_run_evidence(raw: dict[str, Any]) -> dict[str, Any]:
    existing = raw.get("forge_run_evidence")
    if isinstance(existing, dict):
        evidence = dict(existing)
        evidence.setdefault("schema_version", 1)
    else:
        raw_loop_task = raw.get("loop_task")
        loop_task: dict[str, Any] = raw_loop_task if isinstance(raw_loop_task, dict) else {}
        evidence = {
            "schema_version": 2,
            "source": "desktop_trace",
            "session_id": raw.get("session_id"),
            "loop_task_id": loop_task.get("task_id"),
            "prompt": raw.get("user_prompt") or raw.get("prompt"),
            "normalized_goal": raw.get("task_id"),
            "prepared_context": {
                "compact_count": raw.get("compact_count", 0),
                "compact_events": raw.get("compact_events", []),
            },
            "memory_audit": {},
            "permission_decisions": permission_decisions_from_raw_events(raw.get("raw_events")),
            "tool_calls": raw.get("tool_calls", []),
            "shell_outputs": raw.get("shell_outputs", []),
            "changed_files": raw.get("changed_files", []),
            "file_diffs": raw.get("file_diffs", []),
            "verification": raw.get("verification_result"),
            "provider_usage": {
                "input_tokens": raw.get("input_tokens"),
                "output_tokens": raw.get("output_tokens"),
                "events": provider_usage_events_from_raw_events(raw.get("raw_events")),
            },
            "failure_category": raw.get("failure_category"),
            "failure_reason": raw.get("failure_reason"),
            "recovery": loop_task.get("recovery_state"),
            "completion_eligibility": completion_eligibility_from_loop_task(loop_task),
            "a2a_child_capsules": raw.get("a2a_child_capsules", []),
            "continuity_lessons": continuity_from_desktop_trace(raw),
        }
    evidence.setdefault("completion_eligibility", {"status": "unknown"})
    evidence.setdefault("source", "forge_trace")
    return {key: value for key, value in evidence.items() if key in ForgeRunEvidence.model_fields}


def completion_eligibility_from_loop_task(loop_task: dict[str, Any]) -> dict[str, Any]:
    result = loop_task.get("completion_result")
    if not isinstance(result, dict):
        return {"status": "unknown"}
    eligibility: dict[str, Any] = {
        "status": str(result.get("status") or "unknown"),
        "commit_eligible": result.get("commit_eligible")
        if isinstance(result.get("commit_eligible"), bool)
        else None,
        "commit_blockers": result.get("commit_blockers")
        if isinstance(result.get("commit_blockers"), list)
        else [],
    }
    facts = result.get("eligibility_facts")
    if isinstance(facts, dict):
        eligibility["facts"] = facts
    return eligibility


def is_desktop_trace_payload(raw: dict[str, Any]) -> bool:
    return any(
        key in raw
        for key in [
            "session_id",
            "loop_task",
            "compact_count",
            "compact_events",
            "headless_continuity_formed_count",
            "headless_continuity_error",
        ]
    )


def permission_decisions_from_raw_events(raw_events: Any) -> list[dict[str, Any]]:
    decisions: list[dict[str, Any]] = []
    for event in raw_events if isinstance(raw_events, list) else []:
        if not isinstance(event, dict):
            continue
        event_type = event.get("event_type")
        if event_type not in {"permission_decision", "confirm_ask", "confirm_response"}:
            continue
        evidence = event.get("evidence") or event.get("permission_evidence")
        if evidence is None:
            continue
        decisions.append(
            {
                "event_type": event_type,
                "block_id": event.get("block_id"),
                "approved": event.get("approved"),
                "evidence": evidence,
            }
        )
    return decisions


def provider_usage_events_from_raw_events(raw_events: Any) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    for event in raw_events if isinstance(raw_events, list) else []:
        if isinstance(event, dict) and event.get("event_type") in {"provider_usage", "usage"}:
            events.append(event)
    return events


def continuity_from_desktop_trace(raw: dict[str, Any]) -> list[dict[str, Any]]:
    if (
        raw.get("headless_continuity_formed_count") is None
        and raw.get("headless_continuity_error") is None
    ):
        return []
    return [
        {
            "formed_count": raw.get("headless_continuity_formed_count"),
            "error": raw.get("headless_continuity_error"),
        }
    ]


def promote_failed_traces(trace_path: Path, output_dir: Path) -> list[Path]:
    written: list[Path] = []
    for trace in load_traces(trace_path):
        if not is_failed_trace(trace):
            continue
        task = case_from_trace(trace)
        written.append(write_promoted_case(task, output_dir))
    return written


def is_failed_trace(trace: AgentTrace) -> bool:
    if trace.failure_category != FailureCategory.NONE:
        return True
    if trace.error is not None or trace.failure_reason is not None:
        return True
    return trace.verification_result is not None and not trace.verification_result.passed


def write_promoted_case(task: EvaluationTask, output_dir: Path) -> Path:
    case_dir = output_dir / safe_case_dir_name(task.id)
    case_dir.mkdir(parents=True, exist_ok=True)
    case_path = case_dir / "case.json"
    case_path.write_text(
        json.dumps({"task": task.model_dump(mode="json")}, indent=2),
        encoding="utf-8",
    )
    return case_path


def safe_case_dir_name(task_id: str) -> str:
    return task_id.replace("/", "_").replace("\\", "_")
