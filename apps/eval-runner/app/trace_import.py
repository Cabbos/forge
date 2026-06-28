import json
from pathlib import Path

from pydantic import TypeAdapter

from app.models import AgentTrace, EvaluationTask, FailureCategory


def case_from_trace(trace: AgentTrace) -> EvaluationTask:
    verification_command = (
        trace.verification_result.command if trace.verification_result is not None else None
    )
    return EvaluationTask(
        id=trace.task_id,
        title=f"Promoted trace: {trace.task_id}",
        prompt=trace.user_prompt,
        context_files=trace.context_files,
        verification_command=verification_command,
        expected_success=False,
        expected_files_changed=trace.expected_files_changed,
        forbidden_files_changed=trace.forbidden_files_changed,
        metadata={
            "source": "trace",
            "failure_reason": trace.failure_reason,
            "failure_category": trace.failure_category.value,
        },
    )


def load_traces(trace_path: Path) -> list[AgentTrace]:
    payload = json.loads(trace_path.read_text(encoding="utf-8"))
    raw_traces = payload.get("traces") if isinstance(payload, dict) else payload
    return TypeAdapter(list[AgentTrace]).validate_python(raw_traces)


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
