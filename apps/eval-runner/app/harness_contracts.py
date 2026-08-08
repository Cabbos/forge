"""Bounded, typed evidence contracts for frontier harness experiments.

The contracts are intentionally eval-side.  They describe what an agent run
returned and what the harness is allowed to retain; they do not change the
desktop agent's tool or prompt semantics.
"""

from __future__ import annotations

import hashlib
import json
from collections import defaultdict
from collections.abc import Iterable
from enum import StrEnum
from pathlib import Path
from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field, model_validator

MAX_TOOL_SUMMARY_CHARS = 500
MAX_TOOL_FACT_CHARS = 300
MAX_TOOL_CONTENT_CHARS = 2_000
MAX_NEXT_EVIDENCE_CHARS = 240
MAX_CONTEXT_ITEMS = 64
MAX_MINIMAL_PROMPT_CHARS = 3_600
MAX_TASK_PLAN_STEPS = 8
MAX_TASK_PLAN_FILES = 64
MAX_TASK_PLAN_COMMANDS = 16


class HarnessContractModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


class HarnessPolicyV1(HarnessContractModel):
    """Eval-only policy identity for measured prompt/context experiments."""

    schema_version: Literal[1] = 1
    prompt_mode: Literal["current_product_v1", "minimal_v1"] = "current_product_v1"
    context_policy: Literal["current_product_v1", "task_local_v1"] = "current_product_v1"
    tool_result_policy: Literal["current_product_v1", "bounded_structured_v1"] = (
        "current_product_v1"
    )
    stagnation_policy: Literal["current_loop_guard_v1", "semantic_progress_v1"] = (
        "current_loop_guard_v1"
    )
    verification_policy: Literal["required_targeted_then_full_v1"] = (
        "required_targeted_then_full_v1"
    )
    repair_attempt_limit: int = Field(default=1, ge=0, le=1)
    parallel_read_tools: Literal[True] = True

    def selected_context_files(self, paths: Iterable[str]) -> list[str]:
        values = list(dict.fromkeys(str(path) for path in paths))
        if self.context_policy == "task_local_v1":
            return values[:16]
        return values

    def render_prompt(self, prompt: str, context_files: Iterable[str]) -> str:
        if self.prompt_mode == "current_product_v1":
            return prompt
        selected = self.selected_context_files(context_files)
        context_line = ", ".join(selected) if selected else "(none declared)"
        rendered = (
            "Outcome: " + prompt.strip() + "\n\n"
            "Scope: inspect the declared task-local context before editing; change only "
            "files required for the outcome.\n"
            f"Task-local context: {context_line}\n"
            "Execution contract: make the smallest actionable change, run the targeted "
            "verification, and use at most one evidence-driven repair before reporting "
            "changed files and verification."
        )
        return rendered[:MAX_MINIMAL_PROMPT_CHARS]

    def fingerprint(self) -> str:
        payload = json.dumps(
            self.model_dump(mode="json"),
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
        ).encode("utf-8")
        return hashlib.sha256(payload).hexdigest()


class TaskContractKind(StrEnum):
    """Controller-owned shape of the change an eval task permits."""

    NO_CHANGE = "no_change"
    SINGLE_FILE = "single_file"
    MULTI_FILE = "multi_file"
    SHELL_HEAVY = "shell_heavy"


class TaskExecutionContract(HarnessContractModel):
    """Bounded execution guidance derived from the sealed task definition.

    This is deliberately not a new agent permission.  It makes the evaluator's
    expectations explicit before the first edit and gives the trace verifier a
    stable shape against which an optional agent plan can be checked.
    """

    schema_version: Literal[1] = 1
    kind: TaskContractKind
    allowed_changed_files: list[str] = Field(default_factory=list, max_length=MAX_TASK_PLAN_FILES)
    forbidden_changed_paths: list[str] = Field(default_factory=list, max_length=MAX_TASK_PLAN_FILES)
    required_inspection_files: list[str] = Field(
        default_factory=list, max_length=MAX_TASK_PLAN_FILES
    )
    required_validation_commands: list[str] = Field(
        default_factory=list, max_length=MAX_TASK_PLAN_COMMANDS
    )
    stop_condition: str = Field(min_length=1, max_length=360)


class TaskPlanAction(StrEnum):
    INSPECT = "inspect"
    EDIT = "edit"
    VERIFY = "verify"
    RECOVER = "recover"
    STOP = "stop"


class TaskPlanStep(HarnessContractModel):
    id: str = Field(min_length=1, max_length=40, pattern=r"^[a-z0-9][a-z0-9_-]*$")
    action: TaskPlanAction
    files: list[str] = Field(default_factory=list, max_length=MAX_TASK_PLAN_FILES)
    evidence: str = Field(default="", max_length=240)


class AgentTaskPlan(HarnessContractModel):
    """Optional structured plan returned by the Forge headless adapter."""

    schema_version: Literal[1] = 1
    goal: str = Field(min_length=1, max_length=500)
    steps: list[TaskPlanStep] = Field(
        min_length=1,
        max_length=MAX_TASK_PLAN_STEPS,
    )

    @model_validator(mode="after")
    def validate_unique_step_ids(self) -> AgentTaskPlan:
        ids = [step.id for step in self.steps]
        if len(ids) != len(set(ids)):
            raise ValueError("task plan step ids must be unique")
        return self


def build_task_execution_contract(
    *,
    expected_files_changed: Iterable[str],
    forbidden_files_changed: Iterable[str],
    context_files: Iterable[str],
    validation_commands: Iterable[str],
    verification_command: str | None,
    tags: Iterable[str],
) -> TaskExecutionContract:
    """Derive bounded planning facts without exposing controller-only checks."""

    allowed = _dedupe_nonblank(expected_files_changed, limit=MAX_TASK_PLAN_FILES)
    forbidden = _dedupe_nonblank(forbidden_files_changed, limit=MAX_TASK_PLAN_FILES)
    context = _dedupe_nonblank(context_files, limit=MAX_TASK_PLAN_FILES)
    commands = _dedupe_nonblank(
        [*validation_commands, verification_command or ""],
        limit=MAX_TASK_PLAN_COMMANDS,
    )
    normalized_tags = {str(tag).strip().lower() for tag in tags if str(tag).strip()}
    if not allowed:
        kind = TaskContractKind.NO_CHANGE
        inspection = context
        stop_condition = (
            "Leave the workspace unchanged and prove the requested no-op with validation."
        )
    elif "shell-heavy" in normalized_tags:
        kind = TaskContractKind.SHELL_HEAVY
        inspection = allowed
        stop_condition = (
            "Capture the bounded command evidence, pass targeted validation, then stop."
        )
    elif len(allowed) > 1 or "multi-file" in normalized_tags:
        kind = TaskContractKind.MULTI_FILE
        inspection = allowed
        stop_condition = (
            "Inspect every allowed file before editing, preserve the cross-file contract, "
            "run targeted validation, then stop."
        )
    else:
        kind = TaskContractKind.SINGLE_FILE
        inspection = allowed
        stop_condition = "Inspect the allowed file, make the smallest fix, validate it, then stop."
    return TaskExecutionContract(
        kind=kind,
        allowed_changed_files=allowed,
        forbidden_changed_paths=forbidden,
        required_inspection_files=inspection,
        required_validation_commands=commands,
        stop_condition=stop_condition,
    )


def validate_task_plan(
    value: Any,
    contract: TaskExecutionContract,
) -> AgentTaskPlan:
    """Validate an agent plan against controller-owned paths and phases."""

    try:
        plan = AgentTaskPlan.model_validate(value)
    except Exception as exc:
        raise ValueError("task_plan_invalid") from exc
    allowed = set(contract.allowed_changed_files)
    inspection_required = set(contract.required_inspection_files)
    inspected: set[str] = set()
    has_verification = False
    has_edit = False
    for step in plan.steps:
        files = set(step.files)
        if files - allowed and step.action in {TaskPlanAction.EDIT, TaskPlanAction.RECOVER}:
            raise ValueError("task_plan_edit_out_of_scope")
        if step.action == TaskPlanAction.INSPECT:
            inspected.update(files)
        elif step.action == TaskPlanAction.VERIFY:
            has_verification = True
        elif step.action in {TaskPlanAction.EDIT, TaskPlanAction.RECOVER}:
            has_edit = True
    missing = inspection_required - inspected
    if missing:
        raise ValueError("task_plan_missing_inspection")
    if not has_verification:
        raise ValueError("task_plan_missing_verification")
    if contract.kind == TaskContractKind.NO_CHANGE and has_edit:
        raise ValueError("task_plan_no_change_edit")
    return plan


def task_plan_evidence(
    contract: TaskExecutionContract,
    provided_plan: Any = None,
) -> dict[str, Any]:
    """Return bounded plan evidence; absence is explicit, never inferred as success."""

    if provided_plan is None:
        return {
            "source": "missing",
            "status": "missing",
            "required": False,
            "reason": "agent did not provide task_plan",
        }
    plan = validate_task_plan(provided_plan, contract)
    inspected = sorted(
        {
            path
            for step in plan.steps
            if step.action == TaskPlanAction.INSPECT
            for path in step.files
        }
    )
    return {
        "source": "agent_payload",
        "status": "valid",
        "required": False,
        "inspected_files": inspected,
        "step_count": len(plan.steps),
        "plan": plan.model_dump(mode="json"),
    }


def observed_task_execution_evidence(
    contract: TaskExecutionContract,
    *,
    tool_calls: Iterable[tuple[str, str, str, int]],
    changed_files: Iterable[str],
    verification_passed: bool | None,
) -> dict[str, Any]:
    """Summarize observable phase coverage without pretending it is a plan."""

    calls = [str(command) for command, _stdout, _stderr, _exit_code in tool_calls]
    inspection_commands = (
        "read",
        "inspect",
        "cat",
        "grep",
        "rg ",
        "git diff",
        "list",
    )
    inspected = sorted(
        {
            path
            for path in contract.required_inspection_files
            if any(
                path in command and any(token in command.lower() for token in inspection_commands)
                for command in calls
            )
        }
    )
    changed = _dedupe_nonblank(changed_files, limit=MAX_TASK_PLAN_FILES)
    unexpected = sorted(set(changed) - set(contract.allowed_changed_files))
    missing_inspection = sorted(set(contract.required_inspection_files) - set(inspected))
    complete = (
        verification_passed is True
        and not missing_inspection
        and not unexpected
        and (contract.kind != TaskContractKind.NO_CHANGE or not changed)
    )
    return {
        "status": "complete" if complete else "incomplete",
        "inspected_files": inspected,
        "missing_inspection_files": missing_inspection,
        "changed_files": changed,
        "unexpected_changed_files": unexpected,
        "verification_passed": verification_passed,
    }


def _dedupe_nonblank(values: Iterable[str], *, limit: int) -> list[str]:
    result: list[str] = []
    for value in values:
        normalized = str(value).strip()
        if normalized and normalized not in result:
            result.append(normalized)
        if len(result) >= limit:
            break
    return result


class ToolResultStatus(StrEnum):
    OK = "ok"
    ERROR = "error"
    CANCELLED = "cancelled"
    TRUNCATED = "truncated"


class ToolResultEnvelope(HarnessContractModel):
    schema_version: int = 1
    tool: str = Field(min_length=1, max_length=120)
    status: ToolResultStatus
    summary: str = Field(max_length=MAX_TOOL_SUMMARY_CHARS)
    facts: list[str] = Field(default_factory=list, max_length=8)
    content: str = Field(default="", max_length=MAX_TOOL_CONTENT_CHARS)
    next_evidence: list[str] = Field(default_factory=list, max_length=5)
    artifact_digest: str | None = Field(default=None, pattern=r"^[0-9a-f]{64}$")
    artifact_ref: str | None = Field(default=None, max_length=240)
    truncated: bool = False
    retryable: bool = False

    @model_validator(mode="after")
    def validate_bounded_fields(self) -> ToolResultEnvelope:
        if any(len(fact) > MAX_TOOL_FACT_CHARS for fact in self.facts):
            raise ValueError("tool result facts exceed the bounded field limit")
        if any(len(item) > MAX_NEXT_EVIDENCE_CHARS for item in self.next_evidence):
            raise ValueError("tool result next_evidence exceeds the bounded field limit")
        if self.truncated and self.artifact_digest is None:
            raise ValueError("truncated tool results require an artifact_digest")
        if self.status == ToolResultStatus.TRUNCATED and not self.truncated:
            raise ValueError("truncated status requires truncated=true")
        return self


class ProgressKind(StrEnum):
    INSPECTION = "inspection"
    MUTATION = "mutation"
    VERIFICATION = "verification"
    RECOVERY = "recovery"
    COMPLETION = "completion"
    NO_PROGRESS = "no_progress"


class ProgressSignal(HarnessContractModel):
    round: int = Field(ge=0)
    kind: ProgressKind
    evidence: str = Field(max_length=500)
    changed_files: list[str] = Field(default_factory=list, max_length=64)
    verification_passed: bool | None = None


class StagnationAction(StrEnum):
    CONTINUE = "continue"
    REDIRECT = "redirect"
    STOP = "stop"


class StagnationDecision(HarnessContractModel):
    action: StagnationAction
    consecutive_no_progress: int = Field(ge=0)
    redirects_used: int = Field(ge=0, le=1)
    reason: str = Field(max_length=300)
    recovery_instruction: str | None = Field(default=None, max_length=500)


class PrimaryFailureCategory(StrEnum):
    PASSED = "passed"
    MODEL_INCORRECT = "model_incorrect"
    MODEL_INCOMPLETE = "model_incomplete"
    MODEL_NO_PROGRESS = "model_no_progress"
    MODEL_BUDGET_EXHAUSTED = "model_budget_exhausted"
    PROVIDER_ERROR = "provider_error"
    TOOL_ERROR = "tool_error"
    PERMISSION_BLOCKED = "permission_blocked"
    VERIFICATION_FAILED = "verification_failed"
    SCOPE_VIOLATION = "scope_violation"
    WORKSPACE_OBSERVER_UNTRUSTED = "workspace_observer_untrusted"
    PATCH_REPLAY_FAILED = "patch_replay_failed"
    HARNESS_CONTRACT_ERROR = "harness_contract_error"
    CANCELLED = "cancelled"
    TIMED_OUT = "timed_out"


class FailureTaxonomyEvidence(HarnessContractModel):
    primary: PrimaryFailureCategory
    source: str = "controller"
    legacy_category: str | None = None
    reason: str = Field(max_length=500)


def classify_failure(
    *,
    error: str | None,
    legacy_category: str | None,
    verification_passed: bool | None,
    scope_violation: bool = False,
) -> FailureTaxonomyEvidence:
    """Map legacy runner outcomes to the explicit frontier taxonomy."""

    normalized_error = (error or "").lower()
    normalized_legacy = (legacy_category or "").lower()
    if scope_violation or normalized_legacy == "scope_violation":
        primary = PrimaryFailureCategory.SCOPE_VIOLATION
        reason = "changed files violated the declared task scope"
    elif normalized_legacy in {"cancelled", "canceled"} or "cancel" in normalized_error:
        primary = PrimaryFailureCategory.CANCELLED
        reason = "the attempt was cancelled before terminal success"
    elif normalized_legacy in {"timeout", "timed_out"} or "timeout" in normalized_error:
        primary = PrimaryFailureCategory.TIMED_OUT
        reason = "the bounded attempt timeout fired"
    elif normalized_legacy in {
        "budget_exhausted",
        "max_model_rounds_exceeded",
        "model_budget_exhausted",
    }:
        primary = PrimaryFailureCategory.MODEL_BUDGET_EXHAUSTED
        reason = "the model or harness budget was exhausted"
    elif normalized_legacy in {"verification_failed", "verification"}:
        primary = PrimaryFailureCategory.VERIFICATION_FAILED
        reason = "controller-owned verification did not pass"
    elif normalized_legacy in {"tool_error", "tool", "tool_failed"}:
        primary = PrimaryFailureCategory.TOOL_ERROR
        reason = "a tool result reported an error"
    elif normalized_legacy in {"permission_blocked", "permission_denied"}:
        primary = PrimaryFailureCategory.PERMISSION_BLOCKED
        reason = "the requested operation was blocked by permission policy"
    elif normalized_legacy in {"patch_replay_failed", "replay_failed"}:
        primary = PrimaryFailureCategory.PATCH_REPLAY_FAILED
        reason = "the emitted patch did not replay cleanly"
    elif normalized_legacy in {"workspace_observer_untrusted", "observer_untrusted"}:
        primary = PrimaryFailureCategory.WORKSPACE_OBSERVER_UNTRUSTED
        reason = "workspace observer evidence was unavailable or contradictory"
    elif normalized_legacy in {"provider_error", "api", "provider"} or any(
        token in normalized_error for token in ("provider", "api", "network", "credential")
    ):
        primary = PrimaryFailureCategory.PROVIDER_ERROR
        reason = "the provider boundary returned an error"
    elif normalized_legacy in {
        "model_no_progress",
        "repeated_no_progress",
        "no_progress",
    }:
        primary = PrimaryFailureCategory.MODEL_NO_PROGRESS
        reason = "typed progress evidence reached the stagnation stop"
    elif normalized_legacy in {"model_incorrect", "incorrect"}:
        primary = PrimaryFailureCategory.MODEL_INCORRECT
        reason = "verification evidence contradicts the requested outcome"
    elif normalized_legacy in {"model_incomplete", "incomplete", "no_verification"}:
        primary = PrimaryFailureCategory.MODEL_INCOMPLETE
        reason = "the attempt ended without complete success evidence"
    elif normalized_legacy in {"forge_contract_error", "harness_contract_error"}:
        primary = PrimaryFailureCategory.HARNESS_CONTRACT_ERROR
        reason = "the emitted trace violated the harness contract"
    elif not error and verification_passed is True:
        primary = PrimaryFailureCategory.PASSED
        reason = "verification and scope evidence passed"
    elif not error and verification_passed is False:
        primary = PrimaryFailureCategory.VERIFICATION_FAILED
        reason = "verification evidence did not pass"
    elif not error:
        primary = PrimaryFailureCategory.MODEL_INCOMPLETE
        reason = "the attempt has no terminal success evidence"
    else:
        primary = PrimaryFailureCategory.HARNESS_CONTRACT_ERROR
        reason = "the legacy runner error had no safe explicit taxonomy mapping"
    return FailureTaxonomyEvidence(
        primary=primary,
        legacy_category=legacy_category,
        reason=reason,
    )


def evaluate_stagnation(
    signals: Iterable[ProgressSignal],
    *,
    redirects_used: int = 0,
) -> StagnationDecision:
    """Apply the bounded two-round no-progress policy.

    A run may receive one recovery redirect.  The next consecutive pair of
    no-progress rounds stops instead of allowing an unbounded tool loop.
    """

    if redirects_used not in {0, 1}:
        raise ValueError("redirects_used must be 0 or 1")
    consecutive = 0
    for signal in signals:
        if signal.kind == ProgressKind.NO_PROGRESS:
            consecutive += 1
        else:
            consecutive = 0
    if consecutive < 2:
        return StagnationDecision(
            action=StagnationAction.CONTINUE,
            consecutive_no_progress=consecutive,
            redirects_used=redirects_used,
            reason="progress threshold not reached",
        )
    if redirects_used == 0:
        return StagnationDecision(
            action=StagnationAction.REDIRECT,
            consecutive_no_progress=consecutive,
            redirects_used=1,
            reason="two consecutive rounds made no typed progress",
            recovery_instruction=(
                "Inspect the latest verification evidence, make one minimal actionable change, "
                "then rerun the targeted check."
            ),
        )
    return StagnationDecision(
        action=StagnationAction.STOP,
        consecutive_no_progress=consecutive,
        redirects_used=redirects_used,
        reason="no progress persisted after the single bounded recovery redirect",
    )


class ContextTier(StrEnum):
    TASK_LOCAL = "task_local"
    PROJECT_RETRIEVAL = "project_retrieval"
    CONTINUITY = "continuity"


class ContextItem(HarnessContractModel):
    path: str = Field(min_length=1, max_length=500)
    tier: ContextTier
    origin: str = Field(min_length=1, max_length=160)
    priority: int = Field(ge=0, le=100)
    bytes: int = Field(ge=0)
    token_estimate: int = Field(ge=0)
    sha256: str | None = Field(default=None, pattern=r"^[0-9a-f]{64}$")


class ContextManifest(HarnessContractModel):
    schema_version: int = 1
    mode: str = Field(default="minimal", pattern=r"^[a-z0-9_\-]{1,64}$")
    items: list[ContextItem] = Field(default_factory=list, max_length=MAX_CONTEXT_ITEMS)
    total_bytes: int = Field(ge=0)
    total_token_estimate: int = Field(ge=0)
    duplicate_groups: list[list[str]] = Field(default_factory=list, max_length=32)
    conflict_groups: list[list[str]] = Field(default_factory=list, max_length=32)


def build_context_manifest(
    paths: Iterable[str],
    workspace: Path,
    *,
    mode: str = "minimal",
) -> ContextManifest:
    """Hash only the selected context files and retain no source content."""

    items: list[ContextItem] = []
    for raw_path in list(paths)[:MAX_CONTEXT_ITEMS]:
        candidate = Path(raw_path)
        absolute = candidate if candidate.is_absolute() else workspace / candidate
        relative = candidate.as_posix() if not candidate.is_absolute() else raw_path
        try:
            data = absolute.read_bytes() if absolute.is_file() else b""
            digest = hashlib.sha256(data).hexdigest() if absolute.is_file() else None
        except OSError:
            data = b""
            digest = None
        size = len(data)
        items.append(
            ContextItem(
                path=relative,
                tier=ContextTier.TASK_LOCAL,
                origin="task.context_files",
                priority=100,
                bytes=size,
                token_estimate=(size + 3) // 4,
                sha256=digest,
            )
        )

    by_digest: dict[str, list[str]] = defaultdict(list)
    by_basename: dict[str, list[ContextItem]] = defaultdict(list)
    for item in items:
        if item.sha256 is not None:
            by_digest[item.sha256].append(item.path)
        by_basename[Path(item.path).name].append(item)
    duplicate_groups = sorted(
        sorted(paths_for_digest)
        for paths_for_digest in by_digest.values()
        if len(paths_for_digest) > 1
    )
    conflict_groups = sorted(
        sorted(item.path for item in group)
        for group in by_basename.values()
        if len({item.sha256 for item in group}) > 1
    )
    return ContextManifest(
        mode=mode,
        items=items,
        total_bytes=sum(item.bytes for item in items),
        total_token_estimate=sum(item.token_estimate for item in items),
        duplicate_groups=duplicate_groups,
        conflict_groups=conflict_groups,
    )


def envelope_from_shell_output(
    command: str,
    stdout: str,
    stderr: str,
    exit_code: int,
) -> ToolResultEnvelope:
    """Convert legacy shell evidence to the bounded result contract."""

    full_content = stdout or stderr
    digest = hashlib.sha256(full_content.encode("utf-8")).hexdigest()
    bounded = full_content[:MAX_TOOL_CONTENT_CHARS]
    truncated = len(full_content) > len(bounded)
    status = ToolResultStatus.OK if exit_code == 0 else ToolResultStatus.ERROR
    if truncated:
        status = ToolResultStatus.TRUNCATED
    next_evidence = ["retrieve the full shell artifact by digest"] if truncated else []
    return ToolResultEnvelope(
        tool=command[:120],
        status=status,
        summary=("command completed" if exit_code == 0 else "command failed"),
        facts=[f"exit_code={exit_code}"],
        content=bounded,
        next_evidence=next_evidence,
        artifact_digest=digest,
        truncated=truncated,
        retryable=exit_code in {124, 130},
    )


def harness_contract_evidence(
    *,
    task_context_files: Iterable[str],
    workspace: Path,
    tool_calls: Iterable[tuple[str, str, str, int]],
    provided_envelopes: Any = None,
    provided_signals: Any = None,
    context_mode: str = "minimal",
    failure_category: str | None = None,
    error: str | None = None,
    verification_passed: bool | None = None,
    scope_violation: bool = False,
    task_contract: TaskExecutionContract | None = None,
    provided_plan: Any = None,
    changed_files: Iterable[str] = (),
) -> dict[str, Any]:
    """Build a JSON-safe evidence event without retaining unbounded output."""

    call_values = list(tool_calls)
    if provided_envelopes is None:
        envelopes = [envelope_from_shell_output(*call) for call in call_values]
        envelope_source = "derived_shell_output"
    else:
        envelopes = [ToolResultEnvelope.model_validate(value) for value in provided_envelopes]
        envelope_source = "agent_payload"

    if provided_signals is None:
        signals: list[ProgressSignal] = []
        signal_source = "missing"
        decision = StagnationDecision(
            action=StagnationAction.CONTINUE,
            consecutive_no_progress=0,
            redirects_used=0,
            reason="agent did not provide typed progress signals",
        )
    else:
        signals = [ProgressSignal.model_validate(value) for value in provided_signals]
        signal_source = "agent_payload"
        decision = evaluate_stagnation(signals)
    manifest = build_context_manifest(task_context_files, workspace, mode=context_mode)
    failure = classify_failure(
        error=error,
        legacy_category=failure_category,
        verification_passed=verification_passed,
        scope_violation=scope_violation,
    )
    resolved_contract = task_contract or build_task_execution_contract(
        expected_files_changed=[],
        forbidden_files_changed=[],
        context_files=task_context_files,
        validation_commands=[],
        verification_command=None,
        tags=[],
    )
    return {
        "event_type": "harness_contract_evidence",
        "schema_version": 1,
        "context_manifest": manifest.model_dump(mode="json"),
        "tool_results": {
            "source": envelope_source,
            "bounded_content_chars": MAX_TOOL_CONTENT_CHARS,
            "envelopes": [envelope.model_dump(mode="json") for envelope in envelopes],
        },
        "progress": {
            "source": signal_source,
            "signals": [signal.model_dump(mode="json") for signal in signals],
            "stagnation": decision.model_dump(mode="json"),
        },
        "failure_taxonomy": failure.model_dump(mode="json"),
        "task_execution_contract": resolved_contract.model_dump(mode="json"),
        "task_plan": task_plan_evidence(resolved_contract, provided_plan),
        "task_execution_observed": observed_task_execution_evidence(
            resolved_contract,
            tool_calls=call_values,
            changed_files=changed_files,
            verification_passed=verification_passed,
        ),
    }
