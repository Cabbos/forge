import json
from datetime import UTC, datetime

from app.models import AgentTrace, EvalProvider, EvaluationTask, FailureCategory
from app.provider_fallback import (
    ProviderFallbackRunner,
    build_provider_fallback_runner,
    build_provider_route_runner,
    named_provider_chain,
)


def _trace(
    *,
    model: str,
    error: str | None = None,
    reason: str | None = None,
    category: FailureCategory = FailureCategory.NONE,
    raw_events: list[dict[str, object]] | None = None,
    model_rounds: int = 0,
    confirm_requests: int = 0,
    repair_attempts_used: int = 0,
    validation_attempts: int = 0,
    input_tokens: int | None = None,
    output_tokens: int | None = None,
    cost_usd: float | None = None,
    duration_ms: int = 0,
) -> AgentTrace:
    now = datetime.now(UTC)
    return AgentTrace(
        task_id="fallback-task",
        user_prompt="Do the task.",
        model=model,
        provider=EvalProvider.FORGE,
        final_answer="done" if error is None else "failed",
        error=error,
        failure_reason=reason,
        failure_category=category,
        raw_events=raw_events or [],
        model_rounds=model_rounds,
        confirm_requests=confirm_requests,
        repair_attempts_used=repair_attempts_used,
        validation_attempts=validation_attempts,
        input_tokens=input_tokens,
        output_tokens=output_tokens,
        cost_usd=cost_usd,
        started_at=now,
        ended_at=now,
        duration_ms=duration_ms,
    )


class StubRunner:
    def __init__(self, trace: AgentTrace, provider: str, model: str) -> None:
        self.trace = trace
        self.agent_provider = provider
        self.agent_model = model
        self.calls = 0

    def run_task(self, task: EvaluationTask, *, cancel_requested) -> AgentTrace:
        self.calls += 1
        return self.trace


def test_provider_fallback_retries_auth_or_quota_failures_without_copying_error_text() -> None:
    primary = StubRunner(
        _trace(
            model="kimi-k3-dev",
            error="agent_error",
            reason="HTTP 429 quota exceeded; Authorization: Bearer secret-value",
            category=FailureCategory.RUNNER_ERROR,
            raw_events=[
                {
                    "event_type": "error",
                    "code": "api_error",
                    "message": "HTTP 429 quota exceeded; Authorization: Bearer secret-value",
                }
            ],
        ),
        provider="custom_openai",
        model="k3",
    )
    fallback = StubRunner(
        _trace(model="deepseek-v4-flash-fallback"),
        provider="deepseek",
        model="deepseek-v4-flash",
    )
    runner = ProviderFallbackRunner(primary, fallback)

    result = runner.run_task(EvaluationTask(id="fallback-task", title="Fallback", prompt="Do it."))

    assert result.model == "deepseek-v4-flash-fallback"
    assert primary.calls == 1
    assert fallback.calls == 1
    event = next(
        item for item in result.raw_events if item.get("event_type") == "eval_provider_fallback"
    )
    assert event == {
        "event_type": "eval_provider_fallback",
        "reason_code": "rate_limit",
        "primary_provider": "custom_openai",
        "primary_model": "k3",
        "fallback_provider": "deepseek",
        "fallback_model": "deepseek-v4-flash",
    }
    assert "secret-value" not in json.dumps(event)


def test_provider_fallback_aggregates_attempt_metrics_and_emits_bounded_accounting() -> None:
    primary = StubRunner(
        _trace(
            model="kimi-k3-dev",
            error="agent_error",
            reason="HTTP 429 quota exceeded; Authorization: Bearer secret-value",
            category=FailureCategory.RUNNER_ERROR,
            model_rounds=4,
            confirm_requests=1,
            repair_attempts_used=1,
            validation_attempts=2,
            input_tokens=100,
            output_tokens=20,
            cost_usd=0.1,
            duration_ms=30,
        ),
        provider="custom_openai",
        model="k3",
    )
    fallback = StubRunner(
        _trace(
            model="deepseek-v4-flash-fallback",
            model_rounds=5,
            confirm_requests=2,
            repair_attempts_used=0,
            validation_attempts=3,
            input_tokens=200,
            output_tokens=40,
            cost_usd=0.2,
            duration_ms=70,
        ),
        provider="deepseek",
        model="deepseek-v4-flash",
    )

    result = ProviderFallbackRunner(primary, fallback).run_task(
        EvaluationTask(id="fallback-task", title="Fallback", prompt="Do it.")
    )

    assert result.model_rounds == 9
    assert result.confirm_requests == 3
    assert result.repair_attempts_used == 1
    assert result.validation_attempts == 5
    assert result.input_tokens == 300
    assert result.output_tokens == 60
    assert result.cost_usd == 0.3
    assert result.duration_ms == 100
    accounting = next(
        item
        for item in result.raw_events
        if item.get("event_type") == "eval_provider_fallback_accounting"
    )
    assert accounting == {
        "event_type": "eval_provider_fallback_accounting",
        "cost_status": "known",
        "primary_cost_usd": 0.1,
        "fallback_cost_usd": 0.2,
        "total_cost_usd": 0.3,
        "primary_input_tokens": 100,
        "fallback_input_tokens": 200,
        "total_input_tokens": 300,
        "primary_output_tokens": 20,
        "fallback_output_tokens": 40,
        "total_output_tokens": 60,
        "primary_model_rounds": 4,
        "fallback_model_rounds": 5,
        "total_model_rounds": 9,
        "primary_duration_ms": 30,
        "fallback_duration_ms": 70,
        "total_duration_ms": 100,
    }


def test_provider_fallback_keeps_cost_unknown_when_primary_attempt_cost_is_unknown() -> None:
    primary = StubRunner(
        _trace(
            model="kimi-k3-dev",
            error="agent_error",
            reason="HTTP 429 quota exceeded",
            category=FailureCategory.RUNNER_ERROR,
            model_rounds=1,
            input_tokens=100,
            output_tokens=20,
            duration_ms=10,
        ),
        provider="custom_openai",
        model="k3",
    )
    fallback = StubRunner(
        _trace(
            model="deepseek-v4-flash-fallback",
            model_rounds=2,
            input_tokens=200,
            output_tokens=40,
            cost_usd=0.2,
            duration_ms=20,
        ),
        provider="deepseek",
        model="deepseek-v4-flash",
    )

    result = ProviderFallbackRunner(primary, fallback).run_task(
        EvaluationTask(id="fallback-task", title="Fallback", prompt="Do it.")
    )

    assert result.cost_usd is None
    assert result.input_tokens == 300
    assert result.output_tokens == 60
    accounting = next(
        item
        for item in result.raw_events
        if item.get("event_type") == "eval_provider_fallback_accounting"
    )
    assert accounting["cost_status"] == "unknown_primary_cost"
    assert accounting["primary_cost_usd"] is None
    assert accounting["fallback_cost_usd"] == 0.2
    assert accounting["total_cost_usd"] is None


def test_provider_fallback_does_not_retry_verification_or_scope_failures() -> None:
    primary = StubRunner(
        _trace(
            model="kimi-k3-dev",
            error="verification_failed",
            reason="targeted verification failed",
            category=FailureCategory.VERIFICATION_FAILED,
        ),
        provider="custom_openai",
        model="k3",
    )
    fallback = StubRunner(
        _trace(model="deepseek-v4-flash-fallback"),
        provider="deepseek",
        model="deepseek-v4-flash",
    )
    runner = ProviderFallbackRunner(primary, fallback)

    result = runner.run_task(EvaluationTask(id="fallback-task", title="Fallback", prompt="Do it."))

    assert result.error == "verification_failed"
    assert primary.calls == 1
    assert fallback.calls == 0
    assert not any(item.get("event_type") == "eval_provider_fallback" for item in result.raw_events)


def test_named_kimi_chain_is_explicit_and_route_bound() -> None:
    chain = named_provider_chain("kimi-k3-deepseek")

    assert chain.primary.provider == "custom_openai"
    assert chain.primary.model == "k3"
    assert chain.primary.transport == "openai_chat_completions"
    assert chain.primary.base_url == "https://api.kimi.com/coding/v1"
    assert chain.primary.api_key_env == "FORGE_CUSTOM_OPENAI_API_KEY"
    assert chain.fallback.provider == "deepseek"
    assert chain.fallback.model == "deepseek-v4-flash"
    assert chain.fallback.transport == "anthropic_messages"
    assert chain.fallback.base_url == "https://api.deepseek.com/anthropic"
    assert chain.fallback.api_key_env == "DEEPSEEK_API_KEY"


def test_named_provider_chain_rejects_implicit_or_unknown_names() -> None:
    import pytest

    with pytest.raises(ValueError, match="unsupported provider chain"):
        named_provider_chain("kimi-k3-deepseek-extra")


def test_provider_chain_isolates_route_selector_for_each_child() -> None:
    chain = named_provider_chain("kimi-k3-deepseek")
    runner = build_provider_fallback_runner(chain, command=["forge-headless"])

    assert runner.primary.child_env_overrides == {
        "FORGE_HEADLESS_PROVIDER": "custom_openai",
        "FORGE_HEADLESS_MODEL": "k3",
        "FORGE_CUSTOM_OPENAI_BASE_URL": "https://api.kimi.com/coding/v1",
    }
    assert runner.fallback.child_env_overrides == {
        "FORGE_HEADLESS_PROVIDER": "deepseek",
        "FORGE_HEADLESS_MODEL": "deepseek-v4-flash",
        "DEEPSEEK_BASE_URL": "https://api.deepseek.com/anthropic",
    }
    assert runner.primary.execution_identity is not None
    assert runner.primary.execution_identity.provider == "custom_openai"
    assert runner.fallback.execution_identity is not None
    assert runner.fallback.execution_identity.provider == "deepseek"


def test_single_provider_route_runner_preserves_exact_route_identity() -> None:
    chain = named_provider_chain("kimi-k3-deepseek")

    runner = build_provider_route_runner(chain.primary, command=["forge-headless"])

    assert runner.agent_provider == "custom_openai"
    assert runner.agent_model == "k3"
    assert runner.agent_transport == "openai_chat_completions"
    assert runner.agent_endpoint_family == "openai"
    assert runner.agent_base_url == "https://api.kimi.com/coding/v1"
    assert runner.api_key_env == "FORGE_CUSTOM_OPENAI_API_KEY"
