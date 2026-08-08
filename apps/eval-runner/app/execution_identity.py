"""Typed execution identity shared by paid ForgeBench attempts.

The profile JSON remains byte-compatible with the frozen H1/H2 profiles.  This
additive identity records the request controls and runtime that produced a
trace, so an artifact cannot silently change a model-control dimension while
keeping the same profile fingerprint.
"""

from __future__ import annotations

import hashlib
import json
import platform
from typing import Any, Literal

from pydantic import Field, field_validator, model_validator

from app.models import EvalModel

TOOL_CATALOG_FORMAT: Literal["tool_names_v1"] = "tool_names_v1"
_TOOL_NAMES: dict[str, tuple[str, ...]] = {
    "anthropic_messages": (
        "ask_user",
        "delegate_task",
        "edit_file",
        "git_diff",
        "list_directory",
        "read_file",
        "run_shell",
        "search_content",
        "search_files",
        "web_fetch",
        "web_search",
        "write_to_file",
    ),
    "openai_chat_completions": (
        "edit_file",
        "list_directory",
        "read_file",
        "run_shell",
        "search_content",
        "search_files",
        "web_fetch",
        "web_search",
        "write_to_file",
    ),
}
_MAX_OUTPUT_TOKENS = {
    "anthropic_messages": 384_000,
    "openai_chat_completions": 8_192,
}


def tool_catalog_sha256(transport: str) -> str:
    """Hash the frozen controller-visible tool-name catalog for one route."""

    try:
        tools = _TOOL_NAMES[transport]
    except KeyError as exc:
        raise ValueError(f"unsupported execution identity transport: {transport}") from exc
    payload: dict[str, Any] = {
        "format": TOOL_CATALOG_FORMAT,
        "transport": transport,
        "tools": list(tools),
    }
    raw = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(raw).hexdigest()


class ExecutionIdentityV1(EvalModel):
    schema_version: Literal[1] = 1
    provider: str = Field(min_length=1, max_length=80)
    model: str = Field(min_length=1, max_length=160)
    transport: Literal["anthropic_messages", "openai_chat_completions"]
    endpoint_family: Literal["anthropic", "openai"]
    base_url: str = Field(min_length=1, max_length=500)
    temperature_policy: Literal["explicit", "omitted", "provider_default"]
    temperature: float | None = Field(default=None, ge=0.0, le=2.0)
    reasoning_effort: str = Field(min_length=1, max_length=80)
    max_output_tokens: int = Field(gt=0, le=1_000_000)
    tool_catalog_format: Literal["tool_names_v1"] = TOOL_CATALOG_FORMAT
    tool_catalog_sha256: str = Field(pattern=r"^[0-9a-f]{64}$")
    controller_version: str = Field(min_length=1, max_length=160)
    runtime_versions: dict[str, str] = Field(min_length=1, max_length=16)

    @field_validator("runtime_versions")
    @classmethod
    def validate_runtime_versions(cls, value: dict[str, str]) -> dict[str, str]:
        if any(not key.strip() or not item.strip() for key, item in value.items()):
            raise ValueError("runtime versions must contain non-empty values")
        if any(len(key) > 80 or len(item) > 240 for key, item in value.items()):
            raise ValueError("runtime version entries are too long")
        return dict(sorted(value.items()))

    @model_validator(mode="after")
    def validate_identity(self) -> ExecutionIdentityV1:
        expected_family = {
            "anthropic_messages": "anthropic",
            "openai_chat_completions": "openai",
        }[self.transport]
        if self.endpoint_family != expected_family:
            raise ValueError("execution identity endpoint family does not match transport")
        if self.temperature_policy == "explicit" and self.temperature is None:
            raise ValueError("explicit temperature policy requires temperature")
        if self.temperature_policy != "explicit" and self.temperature is not None:
            raise ValueError("non-explicit temperature policy must omit temperature")
        if self.tool_catalog_sha256 != tool_catalog_sha256(self.transport):
            raise ValueError("tool catalog fingerprint does not match the frozen route catalog")
        return self


def default_execution_identity(
    *, provider: str, model: str, transport: str, base_url: str
) -> ExecutionIdentityV1:
    """Build the pinned DeepSeek execution controls for a Forge eval process."""

    endpoint_family = "anthropic" if transport == "anthropic_messages" else "openai"
    max_output_tokens = _MAX_OUTPUT_TOKENS.get(transport)
    if max_output_tokens is None:
        raise ValueError(f"unsupported execution identity transport: {transport}")
    return ExecutionIdentityV1(
        provider=provider,
        model=model,
        transport=transport,
        endpoint_family=endpoint_family,
        base_url=base_url,
        temperature_policy="omitted",
        reasoning_effort="provider_default",
        max_output_tokens=max_output_tokens,
        tool_catalog_sha256=tool_catalog_sha256(transport),
        controller_version="forge-eval-runner/1",
        runtime_versions={
            "python": platform.python_version(),
            "platform": platform.platform(),
        },
    )


def validate_execution_identity_event(
    event: Any,
    *,
    expected: ExecutionIdentityV1,
) -> None:
    """Fail closed when a Rust headless trace omits or mutates the identity."""

    if not isinstance(event, dict):
        raise ValueError("execution_identity_missing")
    actual = ExecutionIdentityV1.model_validate(
        {key: value for key, value in event.items() if key != "event_type"}
    )
    if actual != expected:
        raise ValueError("execution_identity_mismatch")
