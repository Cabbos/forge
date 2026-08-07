"""Deterministic aggregation of H3 harness-contract evidence."""

from __future__ import annotations

from collections import Counter
from collections.abc import Sequence
from statistics import median
from typing import Any

from app.baseline_models import HarnessTelemetrySummary
from app.models import AgentTrace


def summarize_harness_telemetry(traces: Sequence[AgentTrace]) -> HarnessTelemetrySummary:
    """Summarize bounded context/tool/progress evidence without raw content."""

    tool_counts: list[int] = []
    repeated_calls = 0
    total_calls = 0
    context_bytes: list[int] = []
    context_tokens: list[int] = []
    duplicate_rates: list[float] = []
    conflict_rates: list[float] = []
    histogram: Counter[str] = Counter()
    tier_bytes: Counter[str] = Counter()
    tier_items: Counter[str] = Counter()
    redirects = 0
    stops = 0
    attempts_with_contract = 0
    repairs_within_bound = 0

    for trace in traces:
        if trace.repair_attempts_used <= 1:
            repairs_within_bound += 1
        event = _contract_event(trace)
        if event is None:
            continue
        attempts_with_contract += 1
        tool_results = _mapping(event.get("tool_results"))
        envelopes = tool_results.get("envelopes", [])
        names = [
            str(_mapping(envelope).get("tool", "unknown"))
            for envelope in envelopes
            if isinstance(envelope, dict)
        ]
        call_count = len(names)
        total_calls += call_count
        tool_counts.append(call_count)
        histogram[str(call_count)] += 1
        if call_count:
            duplicates = call_count - len(set(names))
            repeated_calls += duplicates

        manifest = _mapping(event.get("context_manifest"))
        context_bytes.append(_nonnegative_int(manifest.get("total_bytes")))
        context_tokens.append(_nonnegative_int(manifest.get("total_token_estimate")))
        items = manifest.get("items", [])
        item_count = len(items) if isinstance(items, list) else 0
        duplicate_paths = _grouped_item_count(manifest.get("duplicate_groups"))
        conflict_paths = _grouped_item_count(manifest.get("conflict_groups"))
        duplicate_rates.append(duplicate_paths / item_count if item_count else 0.0)
        conflict_rates.append(conflict_paths / item_count if item_count else 0.0)
        if isinstance(items, list):
            for item in items:
                item_map = _mapping(item)
                tier = str(item_map.get("tier", "unknown"))
                tier_bytes[tier] += _nonnegative_int(item_map.get("bytes"))
                tier_items[tier] += 1

        progress = _mapping(event.get("progress"))
        stagnation = _mapping(progress.get("stagnation"))
        redirects += int(stagnation.get("action") == "redirect")
        stops += int(stagnation.get("action") == "stop")

    attempt_count = len(traces)
    return HarnessTelemetrySummary(
        attempts_with_contract=attempts_with_contract,
        contract_coverage=(attempts_with_contract / attempt_count if attempt_count else 0.0),
        median_tool_calls=_median_or_zero(tool_counts),
        p95_tool_calls=_p95_or_zero(tool_counts),
        repeated_tool_call_rate=(repeated_calls / total_calls if total_calls else 0.0),
        tool_call_histogram=dict(sorted(histogram.items(), key=lambda item: int(item[0]))),
        median_context_bytes=_median_or_zero(context_bytes),
        p95_context_bytes=_p95_or_zero(context_bytes),
        median_context_tokens=_median_or_zero(context_tokens),
        p95_context_tokens=_p95_or_zero(context_tokens),
        context_duplicate_rate=_mean_or_zero(duplicate_rates),
        context_conflict_rate=_mean_or_zero(conflict_rates),
        context_bytes_by_tier=dict(sorted(tier_bytes.items())),
        context_items_by_tier=dict(sorted(tier_items.items())),
        stagnation_redirects=redirects,
        stagnation_stops=stops,
        single_repair_compliance_rate=(
            repairs_within_bound / attempt_count if attempt_count else 0.0
        ),
    )


def _contract_event(trace: AgentTrace) -> dict[str, Any] | None:
    for event in reversed(trace.raw_events):
        if isinstance(event, dict) and event.get("event_type") == "harness_contract_evidence":
            return event
    return None


def _mapping(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _grouped_item_count(value: Any) -> int:
    if not isinstance(value, list):
        return 0
    return sum(len(group) for group in value if isinstance(group, list))


def _nonnegative_int(value: Any) -> int:
    try:
        number = int(value)
    except (TypeError, ValueError):
        return 0
    return max(0, number)


def _median_or_zero(values: list[int]) -> float:
    return float(median(values)) if values else 0.0


def _p95_or_zero(values: list[int]) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = max(0, int(0.95 * len(ordered) + 0.999999) - 1)
    return float(ordered[index])


def _mean_or_zero(values: list[float]) -> float:
    return sum(values) / len(values) if values else 0.0
