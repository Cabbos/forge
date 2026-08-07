"""Bounded, typed evidence contracts for frontier harness experiments.

The contracts are intentionally eval-side.  They describe what an agent run
returned and what the harness is allowed to retain; they do not change the
desktop agent's tool or prompt semantics.
"""

from __future__ import annotations

import hashlib
from collections import defaultdict
from collections.abc import Iterable
from enum import StrEnum
from pathlib import Path
from typing import Any

from pydantic import BaseModel, ConfigDict, Field, model_validator

MAX_TOOL_SUMMARY_CHARS = 500
MAX_TOOL_FACT_CHARS = 300
MAX_TOOL_CONTENT_CHARS = 2_000
MAX_NEXT_EVIDENCE_CHARS = 240
MAX_CONTEXT_ITEMS = 64


class HarnessContractModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


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
) -> dict[str, Any]:
    """Build a JSON-safe evidence event without retaining unbounded output."""

    if provided_envelopes is None:
        envelopes = [envelope_from_shell_output(*call) for call in tool_calls]
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
    }
