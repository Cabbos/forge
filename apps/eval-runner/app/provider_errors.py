"""Secret-free, bounded provider-error taxonomy shared by eval reports."""

from __future__ import annotations

import re
from collections import Counter
from collections.abc import Iterable
from typing import Any

_SAFE_PROVIDER_ERROR_CODE = re.compile(r"^[a-z0-9][a-z0-9_.:-]{0,63}$")
_PROVIDER_ERROR_EVENT_TYPES = frozenset(
    {
        "provider_error",
        "provider_call_error",
        "provider_request_error",
        "api_error",
    }
)


def summarize_provider_errors(
    raw_events: Iterable[dict[str, Any]],
    trace_error: str | None = None,
) -> dict[str, int]:
    """Return bounded error codes without copying provider messages or secrets."""

    codes = _provider_error_codes(raw_events, trace_error)
    return dict(sorted(Counter(codes).items()))


def provider_error_codes(
    raw_events: Iterable[dict[str, Any]],
    trace_error: str | None = None,
) -> tuple[str, ...]:
    """Return one bounded code per provider error event, plus a safe text fallback."""

    return tuple(_provider_error_codes(raw_events, trace_error))


def _provider_error_codes(
    raw_events: Iterable[dict[str, Any]],
    trace_error: str | None,
) -> list[str]:
    codes: list[str] = []
    saw_provider_error_event = False
    for event in raw_events:
        event_type = str(event.get("event_type", "")).lower()
        if (
            event_type not in _PROVIDER_ERROR_EVENT_TYPES
            and event.get("provider_error") is not True
        ):
            continue
        saw_provider_error_event = True
        codes.append(_provider_error_event_code(event, event_type))
    if not saw_provider_error_event and trace_error and _looks_like_provider_error(trace_error):
        codes.append(_provider_error_text_code(trace_error))
    return codes


def _provider_error_event_code(event: dict[str, Any], event_type: str) -> str:
    code = str(event.get("code") or event.get("error_type") or "").strip().lower()
    if _SAFE_PROVIDER_ERROR_CODE.fullmatch(code):
        return f"code:{code}"
    error = event.get("error")
    if isinstance(error, str) and _looks_like_provider_error(error):
        return _provider_error_text_code(error)
    if event_type in _PROVIDER_ERROR_EVENT_TYPES:
        return event_type
    return "provider_error"


def _looks_like_provider_error(error: str) -> bool:
    normalized = error.lower()
    return any(
        token in normalized
        for token in (
            "provider",
            "api",
            "anthropic",
            "openai",
            "rate_limit",
            "rate limit",
            "http ",
            "timeout",
        )
    )


def _provider_error_text_code(error: str) -> str:
    normalized = error.lower()
    if "rate_limit" in normalized or "rate limit" in normalized or "429" in normalized:
        return "rate_limit"
    if "timeout" in normalized or "timed out" in normalized:
        return "timeout"
    if any(token in normalized for token in ("401", "403", "unauthorized", "forbidden")):
        return "authentication"
    if any(token in normalized for token in ("400", "404", "422", "bad request")):
        return "bad_request"
    return "provider_error"
