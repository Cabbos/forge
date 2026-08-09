from app.models import AgentTrace, EvalProvider, EvaluationTask, FailureCategory
from app.process_control import CancelRequested, never_cancelled
from app.runner import EvalRunner


class ProviderFallbackRunner:
    """Retry a Forge task on a secondary provider after a provider-boundary failure.

    This is intentionally narrower than a general retry loop. Verification,
    scope, tool, and model failures stay with the primary trace; only an
    authentication or capacity response from the upstream provider causes a
    clean second attempt. Each wrapped runner owns its own isolated workspace,
    credentials, and runtime state.
    """

    def __init__(self, primary: EvalRunner, fallback: EvalRunner) -> None:
        self.primary = primary
        self.fallback = fallback

    def run_task(
        self,
        task: EvaluationTask,
        *,
        cancel_requested: CancelRequested = never_cancelled,
    ) -> AgentTrace:
        primary_trace = self.primary.run_task(task, cancel_requested=cancel_requested)
        reason_code = provider_fallback_reason(primary_trace)
        if reason_code is None:
            return primary_trace

        fallback_trace = self.fallback.run_task(task, cancel_requested=cancel_requested)
        fallback_event = {
            "event_type": "eval_provider_fallback",
            "reason_code": reason_code,
            "primary_provider": _runner_identity(self.primary, "agent_provider"),
            "primary_model": _runner_identity(self.primary, "agent_model"),
            "fallback_provider": _runner_identity(self.fallback, "agent_provider"),
            "fallback_model": _runner_identity(self.fallback, "agent_model"),
        }
        return fallback_trace.model_copy(
            update={
                "raw_events": [*fallback_trace.raw_events, fallback_event],
            }
        )


def provider_fallback_reason(trace: AgentTrace) -> str | None:
    """Return a safe reason code only for auth/capacity provider failures."""

    if trace.failure_category != FailureCategory.RUNNER_ERROR:
        return None
    error_text = " ".join(
        text
        for text in (
            trace.error,
            trace.failure_reason,
            *(_provider_event_text(event) for event in trace.raw_events),
        )
        if text
    ).lower()
    if not error_text:
        return None
    if any(
        marker in error_text
        for marker in (
            "429",
            "rate_limit",
            "rate limit",
            "too many requests",
            "insufficient_quota",
            "quota exceeded",
        )
    ):
        return "rate_limit"
    if any(
        marker in error_text
        for marker in (
            "401",
            "403",
            "unauthorized",
            "forbidden",
            "invalid_authentication",
            "authentication failed",
        )
    ):
        return "authentication"
    return None


def _provider_event_text(event: object) -> str:
    if not isinstance(event, dict):
        return ""
    event_type = event.get("event_type")
    if event_type not in {"error", "api_error", "provider_error"}:
        return ""
    parts: list[str] = []
    for key in ("code", "error_type", "message", "error", "status"):
        value = event.get(key)
        if value is not None:
            parts.append(str(value))
    return " ".join(parts)


def _runner_identity(runner: EvalRunner, attribute: str) -> str:
    value = getattr(runner, attribute, None)
    if isinstance(value, EvalProvider):
        return value.value
    if isinstance(value, str) and value.strip():
        return value.strip()
    return "unknown"
