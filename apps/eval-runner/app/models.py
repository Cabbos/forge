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
    BUDGET_EXHAUSTED = "budget_exhausted"
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


class TrustGateResult(EvalModel):
    trusted: bool
    blockers: list[str] = Field(default_factory=list)
    warnings: list[str] = Field(default_factory=list)


class CaseQualityIssue(EvalModel):
    task_id: str
    severity: str
    code: str
    message: str


class WorkspaceCheck(EvalModel):
    ok: bool
    untracked_files: list[str] = Field(default_factory=list)
    modified_files: list[str] = Field(default_factory=list)
    message: str | None = None


class LeakageCheck(EvalModel):
    ok: bool
    findings: list[str] = Field(default_factory=list)
    scrubbed_items: list[str] = Field(default_factory=list)


class EvaluationTask(EvalModel):
    id: str
    title: str
    prompt: str
    fixture_path: str | None = None
    context_files: list[str] = Field(default_factory=list)
    setup_commands: list[str] = Field(default_factory=list)
    validation_commands: list[str] = Field(default_factory=list)
    post_validation_commands: list[str] = Field(default_factory=list)
    verification_command: str | None = None
    expected_success: bool = True
    expected_files_changed: list[str] = Field(default_factory=list)
    forbidden_files_changed: list[str] = Field(default_factory=list)
    max_duration_seconds: int | None = Field(default=None, ge=1)
    max_model_rounds: int | None = Field(default=None, ge=1)
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


class EvalScore(EvalModel):
    name: str
    score: float = Field(ge=0.0, le=1.0)
    label: str
    explanation: str | None = None
    source: str = "code"
    calibration_dataset_id: str | None = None
    calibration_agreement: float | None = Field(default=None, ge=0.0, le=1.0)
    calibration_threshold: float | None = Field(default=None, ge=0.0, le=1.0)
    gate_ci: bool = True


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
    expected_files_changed: list[str] = Field(default_factory=list)
    forbidden_files_changed: list[str] = Field(default_factory=list)
    final_answer: str
    verification_result: VerificationResult | None = None
    error: str | None = None
    failure_reason: str | None = None
    failure_category: FailureCategory = FailureCategory.NONE
    model_rounds: int = Field(default=0, ge=0)
    confirm_requests: int = Field(default=0, ge=0)
    repair_attempts_used: int = Field(default=0, ge=0)
    validation_attempts: int = Field(default=0, ge=0)
    input_tokens: int | None = Field(default=None, ge=0)
    output_tokens: int | None = Field(default=None, ge=0)
    trajectory_path: str | None = None
    cost_usd: float | None = Field(default=None, ge=0)
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
    repair_attempts_used: int = 0
    validation_attempts: int = 0
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
    average_repair_attempts_used: float = 0.0
    average_validation_attempts: float = 0.0
    average_duration_ms: float
    scope_violation_count: int = 0
    failure_categories: dict[str, int] = Field(default_factory=dict)
    tasks: list[TaskMetric] = Field(default_factory=list)


class QueueStatus(EvalModel):
    counts: dict[str, int] = Field(default_factory=dict)
    oldest_pending_run_id: str | None = None
    oldest_running_run_id: str | None = None


class TraceSummary(EvalModel):
    task_id: str
    passed: bool
    verification_passed: bool | None
    scope_violations: list[str] = Field(default_factory=list)
    changed_files: list[str] = Field(default_factory=list)
    expected_files_changed: list[str] = Field(default_factory=list)
    forbidden_files_changed: list[str] = Field(default_factory=list)
    duration_ms: int = Field(ge=0)
    model_rounds: int = Field(default=0, ge=0)
    confirm_requests: int = Field(default=0, ge=0)
    repair_attempts_used: int = Field(default=0, ge=0)
    validation_attempts: int = Field(default=0, ge=0)
    failure_category: FailureCategory
    failure_reason: str | None = None
    error: str | None = None


class ContinuityReport(EvalModel):
    tasks_with_db: int = 0
    tasks_with_formed_experiences: int = 0
    formation_rate: float = 0.0
    recall_ready_rate: float = 0.0
    total_experiences: int = 0
    total_fts_rows: int = 0
    total_formed_reflections: int = 0
    total_reflection_episodes: int = 0
    reflection_episode_rate: float = 0.0
    reflection_coverage_rate: float = 0.0
    notable_failure_count: int = 0
    experience_status_counts: dict[str, int] = Field(default_factory=dict)
    experience_kind_counts: dict[str, int] = Field(default_factory=dict)


class BacktestReport(EvalModel):
    total_tasks: int
    success_rate: float
    verification_pass_rate: float
    scope_violation_rate: float
    avg_duration_ms: float
    avg_model_rounds: float
    avg_confirm_requests: float
    avg_repair_attempts_used: float = 0.0
    avg_validation_attempts: float = 0.0
    total_cost_usd: float = 0.0
    failure_categories: dict[str, int] = Field(default_factory=dict)
    score_summary: dict[str, float] = Field(default_factory=dict)
    tasks: list[TraceSummary] = Field(default_factory=list)
    continuity: ContinuityReport | None = None


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


class ExperimentSnapshot(EvalModel):
    experiment_id: str
    run_id: str
    dataset_fingerprint: str
    provider: str
    model: str
    git_commit: str | None = None
    command: str | None = None
    environment: dict[str, str] = Field(default_factory=dict)
    created_at: datetime

    @field_serializer("created_at")
    def serialize_datetime(self, value: datetime) -> str:
        return value.isoformat()


class RunCreateRequest(EvalModel):
    task_ids: list[str] | None = None
    provider: str = "mock"
    model: str = "deterministic-agent-v1"
    max_retries: int = Field(default=1, ge=0)


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
    retry_count: int = Field(default=0, ge=0)
    max_retries: int = Field(default=0, ge=0)
    failure_reason: str | None = None
    failure_category: FailureCategory = FailureCategory.NONE
    worker_id: str | None = None
    claimed_at: datetime | None = None
    heartbeat_at: datetime | None = None
    lease_expires_at: datetime | None = None

    @field_serializer("started_at", "ended_at", "claimed_at", "heartbeat_at", "lease_expires_at")
    def serialize_datetime(self, value: datetime | None) -> str | None:
        return value.isoformat() if value is not None else None
