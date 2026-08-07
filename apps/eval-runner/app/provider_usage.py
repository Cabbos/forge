from decimal import Decimal
from typing import Any, Literal

from pydantic import Field

from app.harness_profiles import HarnessProfileV1
from app.models import AgentTrace, EvalModel

_TOKENS_PER_MILLION = Decimal(1_000_000)
_KNOWN_REASONS = {"provider_reported"}


class ProviderUsageAccounting(EvalModel):
    status: Literal["known", "unknown"]
    provider_event_count: int = Field(ge=0)
    input_tokens: int | None = Field(default=None, ge=0)
    cache_hit_input_tokens: int | None = Field(default=None, ge=0)
    cache_miss_input_tokens: int | None = Field(default=None, ge=0)
    output_tokens: int | None = Field(default=None, ge=0)
    reasoning_tokens: int | None = Field(default=None, ge=0)
    cost_cny: Decimal | None = Field(default=None, ge=0, allow_inf_nan=False)
    per_task_cap_exceeded: bool = False
    blockers: list[str] = Field(default_factory=list)


def provider_events_from_trace(trace: AgentTrace) -> list[dict[str, Any]]:
    if trace.forge_run_evidence is not None:
        events = trace.forge_run_evidence.provider_usage.get("events")
        if isinstance(events, list):
            return [event for event in events if isinstance(event, dict)]

    return [
        event for event in trace.raw_events if _event_kind(event) in {"provider_usage", "usage"}
    ]


def account_provider_usage(
    provider_events: list[dict[str, Any]],
    profile: HarnessProfileV1,
) -> ProviderUsageAccounting:
    canonical_events = [
        event
        for event in provider_events
        if event.get("provider_id") is not None and event.get("source") != "legacy_usage"
    ]
    blockers: list[str] = []
    if not canonical_events:
        blockers.append("provider_usage_events_missing")
        return _unknown_accounting(0, blockers)

    providers = [event.get("provider_id") for event in canonical_events]
    models = [event.get("model") for event in canonical_events]
    if _has_mixed_identity(providers):
        blockers.append("provider_usage_mixed_provider_identity")
    if _has_mixed_identity(models):
        blockers.append("provider_usage_mixed_model_identity")
    if any(provider != profile.provider for provider in providers):
        blockers.append("provider_usage_provider_mismatch")
    if any(model != profile.model for model in models):
        blockers.append("provider_usage_model_mismatch")

    input_tokens = 0
    cache_hit_input_tokens = 0
    cache_miss_input_tokens = 0
    output_tokens = 0
    reasoning_tokens = 0
    reasoning_tokens_known = True
    for event in canonical_events:
        reason = event.get("reason")
        if reason not in _KNOWN_REASONS:
            blockers.append(
                f"provider_usage_reason_{reason}"
                if reason in {"provider_omitted", "pricing_unknown"}
                else "provider_usage_reason_unknown"
            )

        event_input = _required_token_count(event, "input_tokens", blockers)
        event_cache_hit = _required_token_count(event, "cache_read_tokens", blockers)
        event_output = _required_token_count(event, "output_tokens", blockers)
        event_reasoning = _optional_token_count(event, "reasoning_tokens", blockers)
        event_cache_creation = (
            _required_token_count(event, "cache_creation_tokens", blockers)
            if event.get("source") == "anthropic"
            else 0
        )
        if event_reasoning is None:
            reasoning_tokens_known = False

        if (
            event_input is None
            or event_cache_hit is None
            or event_cache_creation is None
            or event_output is None
        ):
            continue

        if event.get("source") == "anthropic":
            event_cache_miss = event_input + event_cache_creation
            event_total_input = event_cache_miss + event_cache_hit
        else:
            if event_cache_hit > event_input:
                blockers.append("provider_usage_cache_read_exceeds_input")
                continue
            event_total_input = event_input
            event_cache_miss = event_input - event_cache_hit

        input_tokens += event_total_input
        cache_hit_input_tokens += event_cache_hit
        cache_miss_input_tokens += event_cache_miss
        output_tokens += event_output
        if event_reasoning is not None:
            reasoning_tokens += event_reasoning

    if blockers:
        return _unknown_accounting(len(canonical_events), blockers)

    prices = profile.price_table
    cost_cny = (
        Decimal(cache_hit_input_tokens) * prices.input_cache_hit_cny_per_million
        + Decimal(cache_miss_input_tokens) * prices.input_cache_miss_cny_per_million
        + Decimal(output_tokens) * prices.output_cny_per_million
    ) / _TOKENS_PER_MILLION
    return ProviderUsageAccounting(
        status="known",
        provider_event_count=len(canonical_events),
        input_tokens=input_tokens,
        cache_hit_input_tokens=cache_hit_input_tokens,
        cache_miss_input_tokens=cache_miss_input_tokens,
        output_tokens=output_tokens,
        reasoning_tokens=reasoning_tokens if reasoning_tokens_known else None,
        cost_cny=cost_cny,
        per_task_cap_exceeded=cost_cny > profile.max_cost_cny_per_task,
    )


def _event_kind(event: dict[str, Any]) -> Any:
    return event.get("event_type", event.get("type"))


def _required_token_count(
    event: dict[str, Any],
    field_name: str,
    blockers: list[str],
) -> int | None:
    value = event.get(field_name)
    if value is None:
        blockers.append(f"provider_usage_{field_name}_unknown")
        return None
    if not _is_token_count(value):
        blockers.append(f"provider_usage_{field_name}_invalid")
        return None
    return int(value)


def _optional_token_count(
    event: dict[str, Any],
    field_name: str,
    blockers: list[str],
) -> int | None:
    value = event.get(field_name)
    if value is None:
        return None
    if not _is_token_count(value):
        blockers.append(f"provider_usage_{field_name}_invalid")
        return None
    return int(value)


def _is_token_count(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _has_mixed_identity(values: list[Any]) -> bool:
    return any(value != values[0] for value in values[1:])


def _unknown_accounting(
    provider_event_count: int,
    blockers: list[str],
) -> ProviderUsageAccounting:
    return ProviderUsageAccounting(
        status="unknown",
        provider_event_count=provider_event_count,
        blockers=sorted(set(blockers)),
    )
