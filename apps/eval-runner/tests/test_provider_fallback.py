import json
from datetime import UTC, datetime

from app.models import AgentTrace, EvalProvider, EvaluationTask, FailureCategory
from app.provider_fallback import ProviderFallbackRunner


def _trace(
    *,
    model: str,
    error: str | None = None,
    reason: str | None = None,
    category: FailureCategory = FailureCategory.NONE,
    raw_events: list[dict[str, object]] | None = None,
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
        started_at=now,
        ended_at=now,
        duration_ms=0,
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
