from datetime import UTC, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any

import pytest

from app.harness_profiles import HarnessProfileV1, load_harness_profile
from app.models import AgentTrace, EvalProvider, ForgeRunEvidence
from app.provider_usage import (
    KIMI_K3_PRICE_SOURCE,
    account_provider_usage,
    estimate_provider_cost_usd,
    is_kimi_k3_usage,
    kimi_k3_pricing_snapshot,
    provider_events_from_trace,
)

PROFILE_PATH = (
    Path(__file__).parents[1]
    / "forgebench"
    / "profiles"
    / "deepseek-v4-flash-anthropic-baseline-v1.json"
)
OPENAI_PROFILE_PATH = (
    Path(__file__).parents[1]
    / "forgebench"
    / "profiles"
    / "deepseek-v4-flash-openai-chat-control-v1.json"
)


@pytest.fixture
def profile() -> HarnessProfileV1:
    return load_harness_profile(PROFILE_PATH)


@pytest.fixture
def openai_profile() -> HarnessProfileV1:
    return load_harness_profile(OPENAI_PROFILE_PATH)


def provider_event(
    *,
    provider_id: Any = "deepseek",
    model: Any = "deepseek-v4-flash",
    input_tokens: Any = 1_000,
    cache_read_tokens: Any = 250,
    cache_creation_tokens: Any = 0,
    output_tokens: Any = 100,
    reasoning_tokens: Any = 50,
    source: Any = "deepseek",
    reason: Any = "provider_reported",
) -> dict[str, Any]:
    return {
        "provider_id": provider_id,
        "model": model,
        "input_tokens": input_tokens,
        "cache_read_tokens": cache_read_tokens,
        "cache_creation_tokens": cache_creation_tokens,
        "output_tokens": output_tokens,
        "reasoning_tokens": reasoning_tokens,
        "source": source,
        "reason": reason,
    }


def test_prices_cache_hit_miss_and_output_in_cny(profile: HarnessProfileV1) -> None:
    accounting = account_provider_usage(
        provider_events=[
            provider_event(
                input_tokens=1_000_000,
                cache_read_tokens=250_000,
                output_tokens=100_000,
            )
        ],
        profile=profile,
    )

    assert accounting.provider_event_count == 1
    assert accounting.input_tokens == 1_000_000
    assert accounting.cache_hit_input_tokens == 250_000
    assert accounting.cache_miss_input_tokens == 750_000
    assert accounting.output_tokens == 100_000
    assert accounting.reasoning_tokens == 50
    assert accounting.cost_cny == Decimal("0.955")
    assert accounting.status == "known"
    assert accounting.blockers == []
    assert accounting.model_dump(mode="json")["cost_cny"] == "0.955"


def test_kimi_k3_cost_uses_conservative_cache_miss_when_cache_is_omitted() -> None:
    cost = estimate_provider_cost_usd(
        [
            provider_event(
                provider_id="custom_openai",
                model="k3",
                input_tokens=1_000,
                cache_read_tokens=None,
                cache_creation_tokens=None,
                output_tokens=100,
                source="openai_compatible",
                reason="pricing_unknown",
            )
        ]
    )

    assert KIMI_K3_PRICE_SOURCE.endswith("chat-k3")
    assert cost == pytest.approx(0.0045)


def test_kimi_k3_cost_applies_reported_cache_hits() -> None:
    cost = estimate_provider_cost_usd(
        [
            provider_event(
                provider_id="custom_openai",
                model="k3",
                input_tokens=1_000,
                cache_read_tokens=600,
                cache_creation_tokens=None,
                output_tokens=100,
                source="openai_compatible",
                reason="pricing_unknown",
            )
        ]
    )

    assert cost == pytest.approx(0.00288)


def test_cost_estimate_stays_unknown_for_incomplete_or_foreign_usage() -> None:
    assert (
        estimate_provider_cost_usd(
            [provider_event(provider_id="custom_openai", model="k3", input_tokens=None)]
        )
        is None
    )
    assert estimate_provider_cost_usd([provider_event(provider_id="other", model="k3")]) is None


def test_kimi_pricing_snapshot_is_secret_free_and_typed() -> None:
    snapshot = kimi_k3_pricing_snapshot()

    assert is_kimi_k3_usage([provider_event(provider_id="custom_openai", model="k3")])
    assert snapshot == {
        "event_type": "eval_provider_pricing",
        "provider": "custom_openai",
        "model": "k3",
        "currency": "USD",
        "source_url": KIMI_K3_PRICE_SOURCE,
        "retrieved_at": "2026-08-10",
        "input_cache_hit_usd_per_million": "0.30",
        "input_cache_miss_usd_per_million": "3.00",
        "output_usd_per_million": "15.00",
        "missing_cache_metadata_policy": "conservative_cache_miss",
    }


def test_openai_usage_treats_cache_read_as_inclusive_input(
    openai_profile: HarnessProfileV1,
) -> None:
    accounting = account_provider_usage(
        provider_events=[
            provider_event(
                input_tokens=2_400,
                cache_read_tokens=2_000,
                output_tokens=600,
                source="openai",
            )
        ],
        profile=openai_profile,
    )

    assert accounting.status == "known"
    assert accounting.input_tokens == 2_400
    assert accounting.cache_hit_input_tokens == 2_000
    assert accounting.cache_miss_input_tokens == 400
    assert accounting.output_tokens == 600
    assert accounting.cost_cny == Decimal("0.00164")
    assert accounting.blockers == []


def test_anthropic_usage_treats_input_as_uncached_and_adds_cache_tokens(
    profile: HarnessProfileV1,
) -> None:
    accounting = account_provider_usage(
        provider_events=[
            provider_event(
                source="anthropic",
                input_tokens=173,
                cache_read_tokens=2_176,
                cache_creation_tokens=10,
                output_tokens=144,
                reasoning_tokens=None,
            )
        ],
        profile=profile,
    )

    assert accounting.status == "known"
    assert accounting.input_tokens == 2_359
    assert accounting.cache_hit_input_tokens == 2_176
    assert accounting.cache_miss_input_tokens == 183
    assert accounting.output_tokens == 144
    assert accounting.reasoning_tokens is None
    assert accounting.cost_cny == Decimal("0.00051452")
    assert accounting.blockers == []


def test_sums_multiple_provider_events_and_ignores_adjacent_legacy_usage(
    profile: HarnessProfileV1,
) -> None:
    accounting = account_provider_usage(
        provider_events=[
            provider_event(
                input_tokens=800_000,
                cache_read_tokens=200_000,
                output_tokens=50_000,
                reasoning_tokens=25_000,
            ),
            {
                "provider_id": None,
                "model": None,
                "input_tokens": 800_000,
                "output_tokens": 50_000,
                "estimated_cost_usd": 99.0,
                "source": "legacy_usage",
            },
            provider_event(
                input_tokens=200_000,
                cache_read_tokens=50_000,
                output_tokens=50_000,
                reasoning_tokens=25_000,
            ),
        ],
        profile=profile,
    )

    assert accounting.provider_event_count == 2
    assert accounting.input_tokens == 1_000_000
    assert accounting.cache_hit_input_tokens == 250_000
    assert accounting.cache_miss_input_tokens == 750_000
    assert accounting.output_tokens == 100_000
    assert accounting.reasoning_tokens == 50_000
    assert accounting.cost_cny == Decimal("0.955")


@pytest.mark.parametrize(
    ("events", "blocker"),
    [
        ([], "provider_usage_events_missing"),
        (
            [
                provider_event(
                    input_tokens=None,
                    cache_read_tokens=None,
                    output_tokens=None,
                    reason="provider_omitted",
                )
            ],
            "provider_usage_reason_provider_omitted",
        ),
        (
            [provider_event(reason="pricing_unknown")],
            "provider_usage_reason_pricing_unknown",
        ),
        (
            [provider_event(input_tokens=None)],
            "provider_usage_input_tokens_unknown",
        ),
        (
            [provider_event(cache_read_tokens=None)],
            "provider_usage_cache_read_tokens_unknown",
        ),
        (
            [provider_event(output_tokens=None)],
            "provider_usage_output_tokens_unknown",
        ),
        (
            [provider_event(cache_read_tokens=-1)],
            "provider_usage_cache_read_tokens_invalid",
        ),
        (
            [provider_event(cache_read_tokens=1_001)],
            "provider_usage_cache_read_exceeds_input",
        ),
        (
            [provider_event(provider_id="not-deepseek")],
            "provider_usage_provider_mismatch",
        ),
        (
            [provider_event(provider_id={"unexpected": "object"})],
            "provider_usage_provider_mismatch",
        ),
        (
            [provider_event(model="not-v4-flash")],
            "provider_usage_model_mismatch",
        ),
    ],
)
def test_unknown_usage_returns_stable_blocker(
    profile: HarnessProfileV1,
    events: list[dict[str, Any]],
    blocker: str,
) -> None:
    accounting = account_provider_usage(events, profile)

    assert accounting.status == "unknown"
    assert accounting.cost_cny is None
    assert blocker in accounting.blockers


def test_mixed_provider_and_model_identities_are_unknown(
    profile: HarnessProfileV1,
) -> None:
    accounting = account_provider_usage(
        [
            provider_event(),
            provider_event(provider_id="other", model="other-model"),
        ],
        profile,
    )

    assert accounting.status == "unknown"
    assert accounting.blockers == [
        "provider_usage_mixed_model_identity",
        "provider_usage_mixed_provider_identity",
        "provider_usage_model_mismatch",
        "provider_usage_provider_mismatch",
    ]


def test_exact_cap_equality_passes_and_above_cap_is_flagged(
    profile: HarnessProfileV1,
) -> None:
    at_cap = account_provider_usage(
        [
            provider_event(
                input_tokens=1_000_000,
                cache_read_tokens=0,
                output_tokens=0,
                reasoning_tokens=0,
            )
        ],
        profile,
    )
    above_cap = account_provider_usage(
        [
            provider_event(
                input_tokens=1_000_001,
                cache_read_tokens=0,
                output_tokens=0,
                reasoning_tokens=0,
            )
        ],
        profile,
    )

    assert at_cap.cost_cny == Decimal("1")
    assert at_cap.per_task_cap_exceeded is False
    assert above_cap.cost_cny == Decimal("1.000001")
    assert above_cap.per_task_cap_exceeded is True


@pytest.mark.parametrize(
    ("field", "value", "blocker"),
    [
        ("input_tokens", 1.5, "provider_usage_input_tokens_invalid"),
        ("input_tokens", True, "provider_usage_input_tokens_invalid"),
        ("output_tokens", -1, "provider_usage_output_tokens_invalid"),
        ("reasoning_tokens", 0.5, "provider_usage_reasoning_tokens_invalid"),
    ],
)
def test_non_integer_or_negative_tokens_never_enter_decimal_calculation(
    profile: HarnessProfileV1,
    field: str,
    value: object,
    blocker: str,
) -> None:
    event = provider_event()
    event[field] = value

    accounting = account_provider_usage([event], profile)

    assert accounting.status == "unknown"
    assert accounting.cost_cny is None
    assert blocker in accounting.blockers


def test_provider_events_are_read_from_forge_evidence_without_raw_event_duplication() -> None:
    canonical = provider_event()
    legacy = {
        "provider_id": None,
        "source": "legacy_usage",
        "input_tokens": 1_000,
        "output_tokens": 100,
    }
    now = datetime.now(UTC)
    trace = AgentTrace(
        task_id="usage-evidence",
        user_prompt="Run Forge.",
        model="forge-harness",
        provider=EvalProvider.FORGE,
        raw_events=[
            {"event_type": "provider_usage", **canonical},
            {"event_type": "usage", **legacy},
        ],
        final_answer="Done.",
        forge_run_evidence=ForgeRunEvidence(provider_usage={"events": [canonical, legacy]}),
        started_at=now,
        ended_at=now,
        duration_ms=0,
    )

    assert provider_events_from_trace(trace) == [canonical, legacy]
