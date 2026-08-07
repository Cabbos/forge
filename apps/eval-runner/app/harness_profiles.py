import json
import re
from datetime import date
from decimal import Decimal
from pathlib import Path
from typing import Any, Literal

from pydantic import AnyHttpUrl, Field, field_validator, model_validator

from app.models import DatasetSplit, EvalModel
from app.provenance import canonical_json_bytes, sha256_bytes

PROFILE_ID = "deepseek-v4-flash-anthropic-baseline-v1"
PRICE_SOURCE = "https://api-docs.deepseek.com/zh-cn/quick_start/pricing/"
PRICE_RETRIEVED_AT = date(2026, 8, 7)
OFFICIAL_DEEPSEEK_ROUTES: dict[tuple[str, str], str] = {
    ("anthropic_messages", "anthropic"): "https://api.deepseek.com/anthropic",
    ("openai_chat_completions", "openai"): "https://api.deepseek.com",
}
_SENSITIVE_KEY = re.compile(
    r"(?:api[_-]?key|authorization|auth[_-]?token|access[_-]?token|secret|password)",
    re.IGNORECASE,
)
_SENSITIVE_VALUE = re.compile(
    r"(?:\bBearer\s+\S+|(?:^|[^A-Za-z0-9])sk-[A-Za-z0-9_-]{8,})",
    re.IGNORECASE,
)


class HarnessProfileError(ValueError):
    pass


class PriceTableV1(EvalModel):
    currency: Literal["CNY"]
    model: Literal["deepseek-v4-flash"]
    input_cache_hit_cny_per_million: Decimal = Field(gt=0, allow_inf_nan=False)
    input_cache_miss_cny_per_million: Decimal = Field(gt=0, allow_inf_nan=False)
    output_cny_per_million: Decimal = Field(gt=0, allow_inf_nan=False)
    source_url: AnyHttpUrl
    retrieved_at: date

    @field_validator("source_url")
    @classmethod
    def validate_source_url(cls, value: AnyHttpUrl) -> AnyHttpUrl:
        if value.scheme != "https":
            raise ValueError("price source must use HTTPS")
        if str(value) != PRICE_SOURCE:
            raise ValueError("price source must match the frozen official source")
        return value

    @model_validator(mode="after")
    def validate_frozen_snapshot(self) -> "PriceTableV1":
        expected = {
            "input_cache_hit_cny_per_million": Decimal("0.02"),
            "input_cache_miss_cny_per_million": Decimal("1"),
            "output_cny_per_million": Decimal("2"),
        }
        for field_name, value in expected.items():
            if getattr(self, field_name) != value:
                raise ValueError(f"{field_name} does not match the frozen price snapshot")
        if self.retrieved_at != PRICE_RETRIEVED_AT:
            raise ValueError("retrieved_at does not match the frozen price snapshot")
        return self


class HarnessProfileV1(EvalModel):
    schema_version: Literal[1] = 1
    profile_id: Literal[
        "deepseek-v4-flash-anthropic-baseline-v1",
        "deepseek-v4-flash-openai-chat-control-v1",
        "deepseek-v4-flash-anthropic-minimal-v1",
        "deepseek-v4-flash-openai-chat-minimal-v1",
    ]
    provider: Literal["deepseek"]
    model: Literal["deepseek-v4-flash"]
    transport: Literal["anthropic_messages", "openai_chat_completions"]
    endpoint_family: Literal["anthropic", "openai"]
    base_url: (
        Literal[
            "https://api.deepseek.com/anthropic",
            "https://api.deepseek.com",
        ]
        | None
    ) = Field(default=None, exclude_if=lambda value: value is None)
    reasoning_effort: Literal["provider_default"]
    thinking_mode: Literal["enabled_default"]
    prompt_mode: Literal["current_product_v1", "minimal_v1"]
    context_policy: Literal["current_product_v1", "task_local_v1"]
    tool_result_policy: Literal["current_product_v1", "bounded_structured_v1"]
    parallel_read_tools: Literal[True]
    max_model_rounds: int = Field(ge=1, le=80)
    stagnation_policy: Literal["current_loop_guard_v1", "semantic_progress_v1"]
    verification_policy: Literal["required_targeted_then_full_v1"]
    repair_attempt_limit: int = Field(ge=0, le=1)
    dataset_split: Literal[DatasetSplit.DEVELOPMENT]
    development_case_count: Literal[10]
    repetitions: Literal[3]
    max_attempts: Literal[30]
    max_cost_cny_per_task: Decimal = Field(gt=0, allow_inf_nan=False)
    campaign_budget_cny: Decimal = Field(gt=0, allow_inf_nan=False)
    global_budget_cny: Decimal = Field(gt=0, allow_inf_nan=False)
    price_table: PriceTableV1

    @property
    def resolved_base_url(self) -> str:
        expected = OFFICIAL_DEEPSEEK_ROUTES.get((self.transport, self.endpoint_family))
        if expected is None:
            raise ValueError("profile route is not an official frozen DeepSeek route")
        if self.base_url is not None and self.base_url != expected:
            raise ValueError("profile base URL does not match its route")
        return expected

    @model_validator(mode="after")
    def validate_route_identity(self) -> "HarnessProfileV1":
        expected = OFFICIAL_DEEPSEEK_ROUTES.get((self.transport, self.endpoint_family))
        expected_profiles = {
            ("anthropic_messages", "anthropic", "current_product_v1"): (
                "deepseek-v4-flash-anthropic-baseline-v1"
            ),
            ("openai_chat_completions", "openai", "current_product_v1"): (
                "deepseek-v4-flash-openai-chat-control-v1"
            ),
            ("anthropic_messages", "anthropic", "minimal_v1"): (
                "deepseek-v4-flash-anthropic-minimal-v1"
            ),
            ("openai_chat_completions", "openai", "minimal_v1"): (
                "deepseek-v4-flash-openai-chat-minimal-v1"
            ),
        }
        expected_profile = expected_profiles.get(
            (self.transport, self.endpoint_family, self.prompt_mode)
        )
        minimal_policy = (
            self.prompt_mode == "minimal_v1"
            and self.context_policy == "task_local_v1"
            and self.tool_result_policy == "bounded_structured_v1"
            and self.stagnation_policy == "semantic_progress_v1"
        )
        current_policy = (
            self.prompt_mode == "current_product_v1"
            and self.context_policy == "current_product_v1"
            and self.tool_result_policy == "current_product_v1"
            and self.stagnation_policy == "current_loop_guard_v1"
        )
        if (
            expected is None
            or self.profile_id != expected_profile
            or (self.prompt_mode == "minimal_v1" and not minimal_policy)
            or (self.prompt_mode == "current_product_v1" and not current_policy)
            or (self.endpoint_family == "openai" and self.base_url is None)
            or (self.base_url is not None and self.base_url != expected)
        ):
            raise ValueError("profile identity must match an approved route")
        return self

    @model_validator(mode="after")
    def validate_budget_hierarchy(self) -> "HarnessProfileV1":
        if self.development_case_count * self.repetitions != self.max_attempts:
            raise ValueError("max_attempts must equal development_case_count * repetitions")
        if self.max_cost_cny_per_task > self.campaign_budget_cny:
            raise ValueError("per-task cost cap cannot exceed campaign budget")
        if self.campaign_budget_cny > self.global_budget_cny:
            raise ValueError("campaign budget cannot exceed global budget")
        return self


def load_harness_profile(path: Path) -> HarnessProfileV1:
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise HarnessProfileError("unable to read harness profile") from exc
    try:
        payload = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise HarnessProfileError("harness profile is not valid JSON") from exc
    if not isinstance(payload, dict):
        raise HarnessProfileError("harness profile must be a JSON object")
    if _contains_secret(payload):
        raise HarnessProfileError("harness profile contains secret material")
    profile = HarnessProfileV1.model_validate(payload)
    if raw != harness_profile_bytes(profile) + b"\n":
        raise HarnessProfileError("harness profile must use canonical JSON with one final newline")
    return profile


def harness_profile_bytes(profile: HarnessProfileV1) -> bytes:
    return canonical_json_bytes(profile.model_dump(mode="json", exclude_none=True))


def harness_profile_fingerprint(profile: HarnessProfileV1) -> str:
    return sha256_bytes(harness_profile_bytes(profile))


def _contains_secret(value: Any) -> bool:
    if isinstance(value, dict):
        return any(
            _SENSITIVE_KEY.search(str(key)) is not None or _contains_secret(child)
            for key, child in value.items()
        )
    if isinstance(value, list):
        return any(_contains_secret(child) for child in value)
    return isinstance(value, str) and _SENSITIVE_VALUE.search(value) is not None
