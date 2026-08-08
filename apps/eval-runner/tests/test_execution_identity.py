from __future__ import annotations

import sys
from pathlib import Path

import pytest

from app.execution_identity import (
    ExecutionIdentityV1,
    default_execution_identity,
    tool_catalog_sha256,
    validate_execution_identity_event,
)
from app.models import EvalProvider, EvaluationTask
from app.runner import ForgeAgentRunner


def test_default_route_identity_freezes_controls_and_catalog() -> None:
    identity = default_execution_identity(
        provider="deepseek",
        model="deepseek-v4-flash",
        transport="openai_chat_completions",
        base_url="https://api.deepseek.com",
    )

    assert identity.temperature_policy == "omitted"
    assert identity.temperature is None
    assert identity.reasoning_effort == "provider_default"
    assert identity.max_output_tokens == 8192
    assert identity.tool_catalog_sha256 == tool_catalog_sha256("openai_chat_completions")
    assert identity.runtime_versions["python"]


def test_execution_identity_rejects_wrong_catalog_or_route() -> None:
    identity = default_execution_identity(
        provider="deepseek",
        model="deepseek-v4-flash",
        transport="anthropic_messages",
        base_url="https://api.deepseek.com/anthropic",
    )
    payload = identity.model_dump(mode="json")
    payload["tool_catalog_sha256"] = "0" * 64
    with pytest.raises(ValueError, match="tool catalog"):
        ExecutionIdentityV1.model_validate(payload)

    payload = identity.model_dump(mode="json")
    payload["endpoint_family"] = "openai"
    with pytest.raises(ValueError, match="endpoint family"):
        ExecutionIdentityV1.model_validate(payload)

    payload = identity.model_dump(mode="json")
    payload["max_output_tokens"] = 8192
    with pytest.raises(ValueError, match="output limit"):
        ExecutionIdentityV1.model_validate(payload)


def test_trace_execution_identity_must_match_controller_expectation() -> None:
    identity = default_execution_identity(
        provider="deepseek",
        model="deepseek-v4-flash",
        transport="openai_chat_completions",
        base_url="https://api.deepseek.com",
    )
    event = {"event_type": "eval_execution_identity", **identity.model_dump(mode="json")}
    validate_execution_identity_event(event, expected=identity)

    event["max_output_tokens"] = 1024
    with pytest.raises(ValueError, match="execution_identity_mismatch"):
        validate_execution_identity_event(event, expected=identity)


def test_explicit_temperature_requires_value() -> None:
    identity = default_execution_identity(
        provider="deepseek",
        model="deepseek-v4-flash",
        transport="openai_chat_completions",
        base_url="https://api.deepseek.com",
    )
    payload = identity.model_dump(mode="json")
    payload["temperature_policy"] = "explicit"
    with pytest.raises(ValueError, match="temperature"):
        ExecutionIdentityV1.model_validate(payload)

    payload["temperature"] = 0.2
    with pytest.raises(ValueError, match="frozen route contract"):
        ExecutionIdentityV1.model_validate(payload)


def test_unwired_reasoning_effort_is_rejected() -> None:
    identity = default_execution_identity(
        provider="deepseek",
        model="deepseek-v4-flash",
        transport="openai_chat_completions",
        base_url="https://api.deepseek.com",
    )
    payload = identity.model_dump(mode="json")
    payload["reasoning_effort"] = "high"
    with pytest.raises(ValueError, match="frozen route contract"):
        ExecutionIdentityV1.model_validate(payload)


def test_forge_runner_binds_rust_execution_identity_to_trace(tmp_path: Path) -> None:
    agent = tmp_path / "identity-agent.py"
    agent.write_text(
        "import json, sys\n"
        "payload = json.load(sys.stdin)\n"
        "identity = dict(payload['execution_identity'])\n"
        "identity.update({\n"
        "  'event_type': 'eval_execution_identity',\n"
        "  'schema_version': 1,\n"
        "  'provider': payload['provider'],\n"
        "  'model': payload['model'],\n"
        "  'transport': payload['provider_route']['transport'],\n"
        "  'endpoint_family': 'openai',\n"
        "  'base_url': payload['provider_route']['base_url'],\n"
        "})\n"
        "json.dump({\n"
        "  'forge_agent_provider': 'deepseek',\n"
        "  'forge_agent_model': 'deepseek-v4-flash',\n"
        "  'raw_events': [identity],\n"
        "  'final_answer': 'done',\n"
        "  'verification_result': {'command': 'true', 'passed': True, 'stdout': '', "
        "'stderr': '', 'exit_code': 0, 'duration_ms': 1}\n"
        "}, sys.stdout)\n",
        encoding="utf-8",
    )
    expected = default_execution_identity(
        provider="deepseek",
        model="deepseek-v4-flash",
        transport="openai_chat_completions",
        base_url="https://api.deepseek.com",
    )
    runner = ForgeAgentRunner(
        provider=EvalProvider.FORGE,
        command=[sys.executable, str(agent)],
        agent_provider=expected.provider,
        agent_model=expected.model,
        agent_transport=expected.transport,
        agent_endpoint_family=expected.endpoint_family,
        agent_base_url=expected.base_url,
        execution_identity=expected,
    )

    trace = runner.run_task(
        EvaluationTask(id="execution-identity", title="Identity", prompt="Verify identity.")
    )

    assert trace.error is None
    event = next(
        event for event in trace.raw_events if event.get("event_type") == "eval_execution_identity"
    )
    assert event["tool_catalog_sha256"] == expected.tool_catalog_sha256
