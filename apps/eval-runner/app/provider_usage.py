from decimal import Decimal
from typing import Any, Literal

from pydantic import Field

from app.harness_profiles import HarnessProfileV1
from app.models import AgentTrace, EvalModel

_TOKENS_PER_MILLION = Decimal(1_000_000)
_KNOWN_REASONS = {"provider_reported"}
KIMI_K3_PRICE_SOURCE = "https://platform.kimi.ai/docs/pricing/chat-k3"
KIMI_K3_PRICING_RETRIEVED_AT = "2026-08-10"
KIMI_K3_INPUT_CACHE_HIT_USD_PER_MILLION = Decimal("0.30")
KIMI_K3_INPUT_CACHE_MISS_USD_PER_MILLION = Decimal("3.00")
KIMI_K3_OUTPUT_USD_PER_MILLION = Decimal("15.00")


def kimi_k3_pricing_snapshot() -> dict[str, str]:
    """Return the secret-free frozen price evidence attached to live traces."""

    return {
        "event_type": "eval_provider_pricing",
        "provider": "custom_openai",
        "model": "k3",
        "currency": "USD",
        "source_url": KIMI_K3_PRICE_SOURCE,
        "retrieved_at": KIMI_K3_PRICING_RETRIEVED_AT,
        "input_cache_hit_usd_per_million": str(KIMI_K3_INPUT_CACHE_HIT_USD_PER_MILLION),
        "input_cache_miss_usd_per_million": str(KIMI_K3_INPUT_CACHE_MISS_USD_PER_MILLION),
        "output_usd_per_million": str(KIMI_K3_OUTPUT_USD_PER_MILLION),
        "missing_cache_metadata_policy": "conservative_cache_miss",
    }


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


def estimate_provider_cost_usd(provider_events: list[dict[str, Any]]) -> float | None:
    """Estimate cost from canonical usage without inventing missing token facts.

    Kimi K3's official price table is frozen above for the ordinary development
    chain.  Kimi's coding route currently reports total input/output tokens but
    may omit cache-hit counts; that case is priced conservatively as all cache
    miss input.  Providers that report billing-grade micros are trusted directly.
    """

    canonical_events = [
        event
        for event in provider_events
        if event.get("provider_id") is not None and event.get("model") is not None
    ]
    if not canonical_events:
        return None

    reported_micros = [event.get("estimated_cost_micros") for event in canonical_events]
    if all(_is_token_count(value) for value in reported_micros):
        return float(sum(reported_micros, 0) / Decimal(1_000_000))

    identities = {(event.get("provider_id"), event.get("model")) for event in canonical_events}
    if identities != {("custom_openai", "k3")}:
        return None

    input_cache_hit = 0
    input_cache_miss = 0
    output_tokens = 0
    for event in canonical_events:
        input_tokens = event.get("input_tokens")
        output = event.get("output_tokens")
        if not _is_token_count(input_tokens) or not _is_token_count(output):
            return None
        cache_hit = event.get("cache_read_tokens")
        # Missing cache metadata is conservatively treated as a miss, so the
        # budget cannot understate spend.  This is explicit in the artifact's
        # frozen Kimi price-profile documentation.
        if cache_hit is None:
            cache_hit = 0
        if not _is_token_count(cache_hit) or cache_hit > input_tokens:
            return None
        input_cache_hit += cache_hit
        input_cache_miss += input_tokens - cache_hit
        output_tokens += output

    cost = (
        Decimal(input_cache_hit) * KIMI_K3_INPUT_CACHE_HIT_USD_PER_MILLION
        + Decimal(input_cache_miss) * KIMI_K3_INPUT_CACHE_MISS_USD_PER_MILLION
        + Decimal(output_tokens) * KIMI_K3_OUTPUT_USD_PER_MILLION
    ) / _TOKENS_PER_MILLION
    return float(cost)


def is_kimi_k3_usage(provider_events: list[dict[str, Any]]) -> bool:
    canonical_events = [
        event
        for event in provider_events
        if event.get("provider_id") is not None and event.get("model") is not None
    ]
    return bool(canonical_events) and {
        (event.get("provider_id"), event.get("model")) for event in canonical_events
    } == {("custom_openai", "k3")}


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
