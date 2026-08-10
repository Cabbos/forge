from collections.abc import Sequence
from dataclasses import dataclass

from app.execution_identity import default_execution_identity
from app.harness_contracts import HarnessPolicyV1
from app.models import AgentTrace, EvalProvider, EvaluationTask, FailureCategory
from app.process_control import CancelRequested, never_cancelled
from app.runner import (
    DEFAULT_FORGE_TIMEOUT_SECONDS,
    DEFAULT_SETUP_TIMEOUT_SECONDS,
    DEFAULT_VALIDATION_TIMEOUT_SECONDS,
    EvalRunner,
    ForgeAgentRunner,
)


@dataclass(frozen=True)
class ProviderRouteSpec:
    provider: str
    model: str
    transport: str
    endpoint_family: str
    base_url: str
    api_key_env: str

    def child_env_overrides(self) -> dict[str, str]:
        base_url_env = {
            "custom_openai": "FORGE_CUSTOM_OPENAI_BASE_URL",
            "deepseek": "DEEPSEEK_BASE_URL",
        }.get(self.provider)
        if base_url_env is None:
            raise ValueError(f"provider chain route has no supported base URL env: {self.provider}")
        return {
            "FORGE_HEADLESS_PROVIDER": self.provider,
            "FORGE_HEADLESS_MODEL": self.model,
            base_url_env: self.base_url,
        }


@dataclass(frozen=True)
class ProviderChainSpec:
    name: str
    primary: ProviderRouteSpec
    fallback: ProviderRouteSpec


def named_provider_chain(name: str) -> ProviderChainSpec:
    """Return a checked-in development chain without accepting secret material."""

    if name != "kimi-k3-deepseek":
        raise ValueError(f"unsupported provider chain: {name}")
    return ProviderChainSpec(
        name=name,
        primary=ProviderRouteSpec(
            provider="custom_openai",
            model="k3",
            transport="openai_chat_completions",
            endpoint_family="openai",
            base_url="https://api.kimi.com/coding/v1",
            api_key_env="FORGE_CUSTOM_OPENAI_API_KEY",
        ),
        fallback=ProviderRouteSpec(
            provider="deepseek",
            model="deepseek-v4-flash",
            transport="anthropic_messages",
            endpoint_family="anthropic",
            base_url="https://api.deepseek.com/anthropic",
            api_key_env="DEEPSEEK_API_KEY",
        ),
    )


def build_provider_fallback_runner(
    chain: ProviderChainSpec,
    *,
    command: str | Sequence[str] | None,
    command_timeout_seconds: float = DEFAULT_FORGE_TIMEOUT_SECONDS,
    setup_timeout_seconds: float = DEFAULT_SETUP_TIMEOUT_SECONDS,
    validation_timeout_seconds: float = DEFAULT_VALIDATION_TIMEOUT_SECONDS,
    max_model_rounds: int | None = None,
    harness_policy: HarnessPolicyV1 | None = None,
) -> "ProviderFallbackRunner":
    return ProviderFallbackRunner(
        build_provider_route_runner(
            chain.primary,
            command=command,
            command_timeout_seconds=command_timeout_seconds,
            setup_timeout_seconds=setup_timeout_seconds,
            validation_timeout_seconds=validation_timeout_seconds,
            max_model_rounds=max_model_rounds,
            harness_policy=harness_policy,
        ),
        build_provider_route_runner(
            chain.fallback,
            command=command,
            command_timeout_seconds=command_timeout_seconds,
            setup_timeout_seconds=setup_timeout_seconds,
            validation_timeout_seconds=validation_timeout_seconds,
            max_model_rounds=max_model_rounds,
            harness_policy=harness_policy,
        ),
    )


def build_provider_route_runner(
    spec: ProviderRouteSpec,
    *,
    command: str | Sequence[str] | None,
    command_timeout_seconds: float = DEFAULT_FORGE_TIMEOUT_SECONDS,
    setup_timeout_seconds: float = DEFAULT_SETUP_TIMEOUT_SECONDS,
    validation_timeout_seconds: float = DEFAULT_VALIDATION_TIMEOUT_SECONDS,
    max_model_rounds: int | None = None,
    harness_policy: HarnessPolicyV1 | None = None,
) -> ForgeAgentRunner:
    """Build one route-pinned runner without adding a fallback edge."""

    return ForgeAgentRunner(
        provider=EvalProvider.FORGE,
        model=spec.model,
        command=command,
        command_timeout_seconds=command_timeout_seconds,
        setup_timeout_seconds=setup_timeout_seconds,
        validation_timeout_seconds=validation_timeout_seconds,
        max_model_rounds=max_model_rounds,
        harness_policy=harness_policy,
        agent_provider=spec.provider,
        agent_model=spec.model,
        agent_transport=spec.transport,
        agent_endpoint_family=spec.endpoint_family,
        agent_base_url=spec.base_url,
        api_key_env=spec.api_key_env,
        child_env_overrides=spec.child_env_overrides(),
        execution_identity=default_execution_identity(
            provider=spec.provider,
            model=spec.model,
            transport=spec.transport,
            base_url=spec.base_url,
        ),
    )


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
        accounting_event = _fallback_accounting_event(primary_trace, fallback_trace)
        return fallback_trace.model_copy(
            update={
                "raw_events": [*fallback_trace.raw_events, fallback_event, accounting_event],
                # A fallback is one logical eval attempt, but it contains two
                # provider executions.  Preserve the final fallback workspace
                # evidence while charging and measuring both executions.
                "model_rounds": primary_trace.model_rounds + fallback_trace.model_rounds,
                "confirm_requests": (
                    primary_trace.confirm_requests + fallback_trace.confirm_requests
                ),
                "repair_attempts_used": (
                    primary_trace.repair_attempts_used + fallback_trace.repair_attempts_used
                ),
                "validation_attempts": (
                    primary_trace.validation_attempts + fallback_trace.validation_attempts
                ),
                "input_tokens": _sum_known_int(
                    primary_trace.input_tokens,
                    fallback_trace.input_tokens,
                ),
                "output_tokens": _sum_known_int(
                    primary_trace.output_tokens,
                    fallback_trace.output_tokens,
                ),
                "cost_usd": _sum_known_float(
                    primary_trace.cost_usd,
                    fallback_trace.cost_usd,
                ),
                "duration_ms": primary_trace.duration_ms + fallback_trace.duration_ms,
            }
        )


def _fallback_accounting_event(
    primary: AgentTrace,
    fallback: AgentTrace,
) -> dict[str, object]:
    """Return secret-free accounting for both provider attempts.

    A missing primary cost is intentionally not treated as zero.  The final
    trace therefore becomes cost-unknown and cannot pass a known-cost gate
    while still retaining the fallback's functional evidence.
    """

    if primary.cost_usd is None:
        cost_status = "unknown_primary_cost"
    elif fallback.cost_usd is None:
        cost_status = "unknown_fallback_cost"
    else:
        cost_status = "known"
    return {
        "event_type": "eval_provider_fallback_accounting",
        "cost_status": cost_status,
        "primary_cost_usd": primary.cost_usd,
        "fallback_cost_usd": fallback.cost_usd,
        "total_cost_usd": _sum_known_float(primary.cost_usd, fallback.cost_usd),
        "primary_input_tokens": primary.input_tokens,
        "fallback_input_tokens": fallback.input_tokens,
        "total_input_tokens": _sum_known_int(primary.input_tokens, fallback.input_tokens),
        "primary_output_tokens": primary.output_tokens,
        "fallback_output_tokens": fallback.output_tokens,
        "total_output_tokens": _sum_known_int(primary.output_tokens, fallback.output_tokens),
        "primary_model_rounds": primary.model_rounds,
        "fallback_model_rounds": fallback.model_rounds,
        "total_model_rounds": primary.model_rounds + fallback.model_rounds,
        "primary_duration_ms": primary.duration_ms,
        "fallback_duration_ms": fallback.duration_ms,
        "total_duration_ms": primary.duration_ms + fallback.duration_ms,
    }


def _sum_known_int(left: int | None, right: int | None) -> int | None:
    if left is None or right is None:
        return None
    return left + right


def _sum_known_float(left: float | None, right: float | None) -> float | None:
    if left is None or right is None:
        return None
    return round(left + right, 12)


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
