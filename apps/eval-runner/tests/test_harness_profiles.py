import json
from datetime import date
from decimal import Decimal
from pathlib import Path

import pytest
from pydantic import ValidationError

from app.harness_profiles import (
    HarnessProfileError,
    HarnessProfileV1,
    harness_profile_bytes,
    harness_profile_fingerprint,
    load_harness_profile,
)
from app.models import DatasetSplit

PROFILE_PATH = (
    Path(__file__).parents[1]
    / "forgebench"
    / "profiles"
    / "deepseek-v4-flash-anthropic-baseline-v1.json"
)
CONTROL_PROFILE_PATH = (
    Path(__file__).parents[1]
    / "forgebench"
    / "profiles"
    / "deepseek-v4-flash-openai-chat-control-v1.json"
)
MINIMAL_PROFILE_PATHS = (
    Path(__file__).parents[1]
    / "forgebench"
    / "profiles"
    / "deepseek-v4-flash-anthropic-minimal-v1.json",
    Path(__file__).parents[1]
    / "forgebench"
    / "profiles"
    / "deepseek-v4-flash-openai-chat-minimal-v1.json",
)
PLANNING_PROFILE_PATHS = (
    Path(__file__).parents[1]
    / "forgebench"
    / "profiles"
    / "deepseek-v4-flash-anthropic-planning-v1.json",
    Path(__file__).parents[1]
    / "forgebench"
    / "profiles"
    / "deepseek-v4-flash-openai-chat-planning-v1.json",
)


def valid_profile_payload() -> dict[str, object]:
    return {
        "schema_version": 1,
        "profile_id": "deepseek-v4-flash-anthropic-baseline-v1",
        "provider": "deepseek",
        "model": "deepseek-v4-flash",
        "transport": "anthropic_messages",
        "endpoint_family": "anthropic",
        "base_url": "https://api.deepseek.com/anthropic",
        "reasoning_effort": "provider_default",
        "thinking_mode": "enabled_default",
        "prompt_mode": "current_product_v1",
        "context_policy": "current_product_v1",
        "tool_result_policy": "current_product_v1",
        "parallel_read_tools": True,
        "max_model_rounds": 48,
        "stagnation_policy": "current_loop_guard_v1",
        "verification_policy": "required_targeted_then_full_v1",
        "repair_attempt_limit": 1,
        "dataset_split": "development",
        "development_case_count": 10,
        "repetitions": 3,
        "max_attempts": 30,
        "max_cost_cny_per_task": "1",
        "campaign_budget_cny": "8",
        "global_budget_cny": "100",
        "price_table": {
            "currency": "CNY",
            "model": "deepseek-v4-flash",
            "input_cache_hit_cny_per_million": "0.02",
            "input_cache_miss_cny_per_million": "1",
            "output_cny_per_million": "2",
            "source_url": "https://api-docs.deepseek.com/zh-cn/quick_start/pricing/",
            "retrieved_at": "2026-08-07",
        },
    }


def valid_control_profile_payload() -> dict[str, object]:
    return {
        **valid_profile_payload(),
        "profile_id": "deepseek-v4-flash-openai-chat-control-v1",
        "transport": "openai_chat_completions",
        "endpoint_family": "openai",
        "base_url": "https://api.deepseek.com",
    }


def valid_minimal_profile_payload(*, openai: bool = False) -> dict[str, object]:
    payload = valid_control_profile_payload() if openai else valid_profile_payload()
    payload.update(
        {
            "profile_id": (
                "deepseek-v4-flash-openai-chat-minimal-v1"
                if openai
                else "deepseek-v4-flash-anthropic-minimal-v1"
            ),
            "prompt_mode": "minimal_v1",
            "context_policy": "task_local_v1",
            "tool_result_policy": "bounded_structured_v1",
            "stagnation_policy": "semantic_progress_v1",
        }
    )
    return payload


def valid_planning_profile_payload(*, openai: bool = False) -> dict[str, object]:
    payload = valid_control_profile_payload() if openai else valid_profile_payload()
    payload.update(
        {
            "profile_id": (
                "deepseek-v4-flash-openai-chat-planning-v1"
                if openai
                else "deepseek-v4-flash-anthropic-planning-v1"
            ),
            "prompt_mode": "planning_v1",
            "context_policy": "task_local_v1",
            "tool_result_policy": "bounded_structured_v1",
            "stagnation_policy": "semantic_progress_v1",
        }
    )
    return payload


def test_h2_committed_profiles_are_canonical_distinct_and_route_only() -> None:
    baseline = load_harness_profile(PROFILE_PATH)
    control = load_harness_profile(CONTROL_PROFILE_PATH)

    assert PROFILE_PATH.read_bytes() == harness_profile_bytes(baseline) + b"\n"
    assert CONTROL_PROFILE_PATH.read_bytes() == harness_profile_bytes(control) + b"\n"
    assert baseline.base_url is None
    assert control.base_url == "https://api.deepseek.com"
    assert harness_profile_fingerprint(baseline) != harness_profile_fingerprint(control)

    route_fields = {"profile_id", "transport", "endpoint_family", "base_url"}
    baseline_dimensions = baseline.model_dump(exclude=route_fields)
    control_dimensions = control.model_dump(exclude=route_fields)
    assert control_dimensions == baseline_dimensions


def test_h3_committed_minimal_profiles_are_canonical_and_policy_bound() -> None:
    profiles = [load_harness_profile(path) for path in MINIMAL_PROFILE_PATHS]

    assert all(
        path.read_bytes() == harness_profile_bytes(profile) + b"\n"
        for path, profile in zip(MINIMAL_PROFILE_PATHS, profiles, strict=True)
    )
    assert all(profile.prompt_mode == "minimal_v1" for profile in profiles)
    assert all(profile.context_policy == "task_local_v1" for profile in profiles)
    assert all(profile.tool_result_policy == "bounded_structured_v1" for profile in profiles)
    assert all(profile.stagnation_policy == "semantic_progress_v1" for profile in profiles)
    assert len({harness_profile_fingerprint(profile) for profile in profiles}) == 2


def test_h3_committed_planning_profiles_are_canonical_and_policy_bound() -> None:
    profiles = [load_harness_profile(path) for path in PLANNING_PROFILE_PATHS]

    assert all(
        path.read_bytes() == harness_profile_bytes(profile) + b"\n"
        for path, profile in zip(PLANNING_PROFILE_PATHS, profiles, strict=True)
    )
    assert all(profile.prompt_mode == "planning_v1" for profile in profiles)
    assert all(profile.context_policy == "task_local_v1" for profile in profiles)
    assert all(profile.tool_result_policy == "bounded_structured_v1" for profile in profiles)
    assert all(profile.stagnation_policy == "semantic_progress_v1" for profile in profiles)
    assert len({harness_profile_fingerprint(profile) for profile in profiles}) == 2


def test_h2_preserves_legacy_h1_profile_bytes_and_fingerprint(tmp_path: Path) -> None:
    legacy_payload = valid_profile_payload()
    legacy_payload.pop("base_url")
    legacy_bytes = json.dumps(
        legacy_payload,
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    path = tmp_path / "legacy-h1-profile.json"
    path.write_bytes(legacy_bytes + b"\n")

    profile = load_harness_profile(path)

    assert profile.base_url is None
    assert harness_profile_bytes(profile) == legacy_bytes
    assert (
        harness_profile_fingerprint(profile)
        == "2fb6612000dc669ab1cf2e794fc9dc0f8831c195bd75230f31ababe63f4a2f66"
    )
    assert PROFILE_PATH.read_bytes() == harness_profile_bytes(profile) + b"\n"


@pytest.mark.parametrize(
    "payload",
    [
        valid_profile_payload(),
        valid_control_profile_payload(),
    ],
)
def test_h2_profile_accepts_only_the_two_committed_route_tuples(
    payload: dict[str, object],
) -> None:
    profile = HarnessProfileV1.model_validate(payload)

    assert (
        profile.profile_id,
        profile.transport,
        profile.endpoint_family,
        profile.base_url,
    ) in {
        (
            "deepseek-v4-flash-anthropic-baseline-v1",
            "anthropic_messages",
            "anthropic",
            "https://api.deepseek.com/anthropic",
        ),
        (
            "deepseek-v4-flash-openai-chat-control-v1",
            "openai_chat_completions",
            "openai",
            "https://api.deepseek.com",
        ),
    }


@pytest.mark.parametrize(
    ("profile_id", "transport", "endpoint_family", "base_url"),
    [
        (
            "deepseek-v4-flash-anthropic-baseline-v1",
            "openai_chat_completions",
            "openai",
            "https://api.deepseek.com",
        ),
        (
            "deepseek-v4-flash-openai-chat-control-v1",
            "anthropic_messages",
            "anthropic",
            "https://api.deepseek.com/anthropic",
        ),
        (
            "deepseek-v4-flash-anthropic-baseline-v1",
            "anthropic_messages",
            "openai",
            "https://api.deepseek.com/anthropic",
        ),
        (
            "deepseek-v4-flash-anthropic-baseline-v1",
            "anthropic_messages",
            "anthropic",
            "https://api.deepseek.com",
        ),
        (
            "deepseek-v4-flash-openai-chat-control-v1",
            "openai_responses",
            "openai",
            "https://api.deepseek.com",
        ),
        (
            "deepseek-v4-flash-anthropic-baseline-v1",
            "anthropic_messages",
            "anthropic",
            "http://api.deepseek.com/anthropic",
        ),
        (
            "deepseek-v4-flash-anthropic-baseline-v1",
            "anthropic_messages",
            "anthropic",
            "https://example.com/anthropic",
        ),
        (
            "deepseek-v4-flash-openai-chat-control-v1",
            "openai_chat_completions",
            "openai",
            "https://api.deepseek.com/v1",
        ),
    ],
)
def test_h2_profile_rejects_crossed_or_untrusted_routes(
    profile_id: str,
    transport: str,
    endpoint_family: str,
    base_url: str,
) -> None:
    payload = {
        **valid_profile_payload(),
        "profile_id": profile_id,
        "transport": transport,
        "endpoint_family": endpoint_family,
        "base_url": base_url,
    }

    with pytest.raises(ValidationError):
        HarnessProfileV1.model_validate(payload)


def test_h2_candidate_profile_requires_explicit_base_url() -> None:
    payload = valid_control_profile_payload()
    payload.pop("base_url")

    with pytest.raises(ValidationError):
        HarnessProfileV1.model_validate(payload)


def test_h1_profile_is_strict_canonical_and_development_only() -> None:
    profile = load_harness_profile(PROFILE_PATH)

    assert profile.schema_version == 1
    assert profile.profile_id == "deepseek-v4-flash-anthropic-baseline-v1"
    assert profile.provider == "deepseek"
    assert profile.model == "deepseek-v4-flash"
    assert profile.transport == "anthropic_messages"
    assert profile.base_url is None
    assert profile.dataset_split == DatasetSplit.DEVELOPMENT
    assert profile.repetitions == 3
    assert profile.max_attempts == 30
    assert profile.max_cost_cny_per_task == Decimal("1")
    assert profile.campaign_budget_cny == Decimal("8")
    assert profile.global_budget_cny == Decimal("100")
    assert (
        harness_profile_fingerprint(profile)
        == "2fb6612000dc669ab1cf2e794fc9dc0f8831c195bd75230f31ababe63f4a2f66"
    )
    assert PROFILE_PATH.read_bytes() == harness_profile_bytes(profile) + b"\n"


@pytest.mark.parametrize("openai", [False, True])
def test_h3_minimal_profiles_bind_policy_identity(openai: bool) -> None:
    profile = HarnessProfileV1.model_validate(valid_minimal_profile_payload(openai=openai))

    assert profile.prompt_mode == "minimal_v1"
    assert profile.context_policy == "task_local_v1"
    assert profile.tool_result_policy == "bounded_structured_v1"
    assert profile.stagnation_policy == "semantic_progress_v1"


@pytest.mark.parametrize("openai", [False, True])
def test_h3_planning_profiles_bind_policy_identity(openai: bool) -> None:
    profile = HarnessProfileV1.model_validate(valid_planning_profile_payload(openai=openai))

    assert profile.prompt_mode == "planning_v1"
    assert profile.context_policy == "task_local_v1"
    assert profile.tool_result_policy == "bounded_structured_v1"
    assert profile.stagnation_policy == "semantic_progress_v1"


def test_h3_minimal_profile_rejects_mixed_policy_fields() -> None:
    payload = valid_minimal_profile_payload()
    payload["context_policy"] = "current_product_v1"

    with pytest.raises(ValidationError):
        HarnessProfileV1.model_validate(payload)


def test_h3_openai_minimal_profile_requires_explicit_base_url() -> None:
    payload = valid_minimal_profile_payload(openai=True)
    payload.pop("base_url")

    with pytest.raises(ValidationError):
        HarnessProfileV1.model_validate(payload)


def test_h1_price_table_matches_official_cny_snapshot() -> None:
    profile = load_harness_profile(PROFILE_PATH)

    assert profile.price_table.input_cache_hit_cny_per_million == Decimal("0.02")
    assert profile.price_table.input_cache_miss_cny_per_million == Decimal("1")
    assert profile.price_table.output_cny_per_million == Decimal("2")
    assert (
        str(profile.price_table.source_url)
        == "https://api-docs.deepseek.com/zh-cn/quick_start/pricing/"
    )
    assert profile.price_table.retrieved_at == date(2026, 8, 7)


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("profile_id", "deepseek-v4-flash-latest"),
        ("provider", "forge"),
        ("model", "deepseek-v4-pro"),
        ("transport", "openai_responses"),
        ("endpoint_family", "openai"),
        ("dataset_split", "holdout"),
        ("development_case_count", 20),
        ("repetitions", 2),
        ("max_attempts", 29),
        ("max_cost_cny_per_task", "0"),
        ("campaign_budget_cny", "-1"),
        ("global_budget_cny", "Infinity"),
    ],
)
def test_h1_profile_rejects_identity_and_budget_drift(field: str, value: object) -> None:
    payload = valid_profile_payload()
    payload[field] = value

    with pytest.raises(ValidationError):
        HarnessProfileV1.model_validate(payload)


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("currency", "USD"),
        ("model", "deepseek-v4-pro"),
        ("input_cache_hit_cny_per_million", "0"),
        ("input_cache_miss_cny_per_million", "NaN"),
        ("output_cny_per_million", "-1"),
        ("source_url", "http://api-docs.deepseek.com/pricing"),
    ],
)
def test_h1_profile_rejects_price_table_drift(field: str, value: object) -> None:
    payload = valid_profile_payload()
    price_table = dict(payload["price_table"])  # type: ignore[arg-type]
    price_table[field] = value
    payload["price_table"] = price_table

    with pytest.raises(ValidationError):
        HarnessProfileV1.model_validate(payload)


def test_h1_profile_forbids_extra_fields() -> None:
    payload = valid_profile_payload()
    payload["temperature"] = 0.0

    with pytest.raises(ValidationError, match="Extra inputs"):
        HarnessProfileV1.model_validate(payload)


@pytest.mark.parametrize(
    "injection",
    [
        {"api_key": "sk-secret-value"},
        {"notes": "Bearer secret-value"},
        {"metadata": {"authorization": "secret-value"}},
    ],
)
def test_loader_rejects_secret_bearing_profile(
    tmp_path: Path, injection: dict[str, object]
) -> None:
    payload = {**valid_profile_payload(), **injection}
    path = tmp_path / "profile.json"
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")

    with pytest.raises(HarnessProfileError, match="secret"):
        load_harness_profile(path)


def test_loader_rejects_noncanonical_profile_bytes(tmp_path: Path) -> None:
    path = tmp_path / "profile.json"
    path.write_text(json.dumps(valid_profile_payload(), indent=2), encoding="utf-8")

    with pytest.raises(HarnessProfileError, match="canonical"):
        load_harness_profile(path)


def test_fingerprint_is_stable_across_dictionary_order() -> None:
    payload = valid_profile_payload()
    reversed_payload = dict(reversed(list(payload.items())))

    assert harness_profile_fingerprint(
        HarnessProfileV1.model_validate(payload)
    ) == harness_profile_fingerprint(HarnessProfileV1.model_validate(reversed_payload))
