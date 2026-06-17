import hashlib
import json
import re
from pathlib import Path

from pydantic import TypeAdapter

from app.models import AgentTrace, EvaluationTask, FailureCategory

SECRET_PATTERN = re.compile(
    r"(sk-[A-Za-z0-9_-]{10,}|ghp_[A-Za-z0-9_]{10,}|xox[baprs]-[A-Za-z0-9-]{10,})"
)


def redact_text(value: str) -> str:
    return SECRET_PATTERN.sub("[REDACTED_SECRET]", value)


def trace_dedupe_key(trace: AgentTrace) -> str:
    payload = "|".join(
        [
            trace.task_id,
            trace.user_prompt,
            trace.failure_category.value,
            trace.failure_reason or "",
        ]
    )
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def case_from_trace(trace: AgentTrace, *, prompt: str | None = None) -> EvaluationTask:
    verification_command = (
        trace.verification_result.command if trace.verification_result is not None else None
    )
    return EvaluationTask(
        id=trace.task_id,
        title=f"Promoted trace: {trace.task_id}",
        prompt=prompt if prompt is not None else trace.user_prompt,
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


def promote_failed_traces(
    trace_path: Path,
    output_dir: Path,
    *,
    redact_secrets: bool = False,
    dedupe: bool = False,
) -> list[Path]:
    written: list[Path] = []
    seen_keys: set[str] = set()
    for trace in load_traces(trace_path):
        if not is_failed_trace(trace):
            continue
        if dedupe:
            key = trace_dedupe_key(trace)
            if key in seen_keys:
                continue
            seen_keys.add(key)
        prompt = redact_text(trace.user_prompt) if redact_secrets else trace.user_prompt
        task = case_from_trace(trace, prompt=prompt)
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
