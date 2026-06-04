from datetime import datetime
from enum import StrEnum
from typing import Any

from pydantic import BaseModel, ConfigDict, Field, field_serializer


class FailureCategory(StrEnum):
    NONE = "none"
    VERIFICATION_FAILED = "verification_failed"
    NO_VERIFICATION = "no_verification"
    RUNNER_ERROR = "runner_error"
    TOOL_ERROR = "tool_error"
    TIMEOUT = "timeout"
    SCOPE_VIOLATION = "scope_violation"
    FORGE_CONTRACT_ERROR = "forge_contract_error"


class RunStatus(StrEnum):
    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"
    TIMEOUT = "timeout"


class EvalModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


class EvaluationTask(EvalModel):
    id: str
    title: str
    prompt: str
    fixture_path: str | None = None
    context_files: list[str] = Field(default_factory=list)
    setup_commands: list[str] = Field(default_factory=list)
    validation_commands: list[str] = Field(default_factory=list)
    verification_command: str | None = None
    expected_success: bool = True
    expected_files_changed: list[str] = Field(default_factory=list)
    forbidden_files_changed: list[str] = Field(default_factory=list)
    max_duration_seconds: int | None = Field(default=None, ge=1)
    tags: list[str] = Field(default_factory=list)
    metadata: dict[str, Any] = Field(default_factory=dict)


class ShellOutput(EvalModel):
    command: str
    stdout: str = ""
    stderr: str = ""
    exit_code: int = 0
    duration_ms: int = Field(default=0, ge=0)


class FileDiff(EvalModel):
    path: str
    change_type: str = "modified"
    diff: str


class VerificationResult(EvalModel):
    command: str
    passed: bool
    stdout: str = ""
    stderr: str = ""
    exit_code: int
    duration_ms: int = Field(default=0, ge=0)


class AgentTrace(EvalModel):
    task_id: str
    user_prompt: str
    model: str
    provider: str
    context_files: list[str] = Field(default_factory=list)
    raw_events: list[dict[str, Any]] = Field(default_factory=list)
    tool_calls: list[ShellOutput] = Field(default_factory=list)
    shell_outputs: list[ShellOutput] = Field(default_factory=list)
    file_diffs: list[FileDiff] = Field(default_factory=list)
    changed_files: list[str] = Field(default_factory=list)
    scope_violations: list[str] = Field(default_factory=list)
    final_answer: str
    verification_result: VerificationResult | None = None
    error: str | None = None
    failure_reason: str | None = None
    failure_category: FailureCategory = FailureCategory.NONE
    model_rounds: int = Field(default=0, ge=0)
    confirm_requests: int = Field(default=0, ge=0)
    input_tokens: int | None = Field(default=None, ge=0)
    output_tokens: int | None = Field(default=None, ge=0)
    started_at: datetime
    ended_at: datetime
    duration_ms: int = Field(ge=0)

    @field_serializer("started_at", "ended_at")
    def serialize_datetime(self, value: datetime) -> str:
        return value.isoformat()


class TaskMetric(EvalModel):
    task_id: str
    passed: bool
    verification_passed: bool | None
    scope_ok: bool = True
    changed_files: int = 0
    tool_calls: int
    model_rounds: int = 0
    confirm_requests: int = 0
    duration_ms: int
    failure_category: FailureCategory


class MetricsSummary(EvalModel):
    total_tasks: int
    passed_tasks: int
    failed_tasks: int
    success_rate: float
    verification_coverage: float
    average_tool_calls: float
    average_model_rounds: float
    average_confirm_requests: float
    average_duration_ms: float
    scope_violation_count: int = 0
    failure_categories: dict[str, int] = Field(default_factory=dict)
    tasks: list[TaskMetric] = Field(default_factory=list)


class TraceSummary(EvalModel):
    task_id: str
    passed: bool
    verification_passed: bool | None
    scope_violations: list[str] = Field(default_factory=list)
    changed_files: list[str] = Field(default_factory=list)
    duration_ms: int = Field(ge=0)
    model_rounds: int = Field(default=0, ge=0)
    confirm_requests: int = Field(default=0, ge=0)
    failure_category: FailureCategory
    failure_reason: str | None = None
    error: str | None = None


class BacktestReport(EvalModel):
    total_tasks: int
    success_rate: float
    verification_pass_rate: float
    scope_violation_rate: float
    avg_duration_ms: float
    avg_model_rounds: float
    avg_confirm_requests: float
    failure_categories: dict[str, int] = Field(default_factory=dict)
    tasks: list[TraceSummary] = Field(default_factory=list)


class EvalArtifact(EvalModel):
    id: str
    run_id: str
    kind: str
    path: str
    size_bytes: int = Field(ge=0)
    created_at: datetime

    @field_serializer("created_at")
    def serialize_datetime(self, value: datetime) -> str:
        return value.isoformat()


class RunCreateRequest(EvalModel):
    task_ids: list[str] | None = None
    provider: str = "mock"
    model: str = "deterministic-agent-v1"


class EvaluationRun(EvalModel):
    run_id: str
    status: RunStatus
    provider: str = "mock"
    model: str = "deterministic-agent-v1"
    case_source: str | None = None
    requested_task_ids: list[str]
    traces: list[AgentTrace] = Field(default_factory=list)
    metrics: MetricsSummary
    started_at: datetime
    ended_at: datetime
    duration_ms: int = Field(ge=0)

    @field_serializer("started_at", "ended_at")
    def serialize_datetime(self, value: datetime) -> str:
        return value.isoformat()
