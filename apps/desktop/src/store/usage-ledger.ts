import type {
  BlockState,
  ContextUsageState,
  ProviderUsageReason,
  SessionUsageLedgerState,
  StreamEvent,
} from "../lib/protocol";

type ProviderUsageEvent = Extract<StreamEvent, { event_type: "provider_usage" }>;
type LegacyUsageEvent = Extract<StreamEvent, { event_type: "usage" }>;

export interface UsageProjectionFromBlocks {
  usageLedger: SessionUsageLedgerState;
  contextUsage: ContextUsageState | null;
  costUsd: number;
}

export function applyProviderUsageToLedger(
  _previous: SessionUsageLedgerState | null | undefined,
  event: ProviderUsageEvent,
  now = Date.now(),
): SessionUsageLedgerState {
  const inputTokens = sanitizeCount(event.input_tokens);
  const outputTokens = sanitizeCount(event.output_tokens);
  const estimatedCostMicros = sanitizeCount(event.estimated_cost_micros);
  const costUsd = estimatedCostMicros === null ? null : estimatedCostMicros / 1_000_000;

  return {
    providerId: sanitizeText(event.provider_id),
    model: sanitizeText(event.model),
    source: sanitizeText(event.source),
    reason: event.reason,
    inputTokens,
    outputTokens,
    cacheReadTokens: sanitizeCount(event.cache_read_tokens ?? null),
    cacheCreationTokens: sanitizeCount(event.cache_creation_tokens ?? null),
    reasoningTokens: sanitizeCount(event.reasoning_tokens ?? null),
    estimatedCostMicros,
    pricingSource: sanitizeText(event.pricing_source),
    costUsd,
    hasUnknownInputTokens: inputTokens === null,
    hasUnknownOutputTokens: outputTokens === null,
    hasUnknownCost: costUsd === null,
    lastEventType: "provider_usage",
    lastProviderUsageBlockId: sanitizeText(event.block_id),
    legacyDuplicateIgnored: false,
    updatedAt: now,
  };
}

export function applyLegacyUsageToLedger(
  previous: SessionUsageLedgerState | null | undefined,
  event: LegacyUsageEvent,
  now = Date.now(),
): SessionUsageLedgerState {
  const inputTokens = sanitizeCount(event.input_tokens);
  const outputTokens = sanitizeCount(event.output_tokens);
  const costUsd = sanitizeCost(event.estimated_cost_usd);
  const estimatedCostMicros = costUsd === null ? null : Math.max(0, Math.round(costUsd * 1_000_000));

  if (previous?.lastEventType === "provider_usage") {
    const isDuplicate =
      previous.inputTokens === inputTokens
      && previous.outputTokens === outputTokens
      && sameUsageCost(previous.costUsd, costUsd);

    if (isDuplicate) {
      return {
        ...previous,
        legacyDuplicateIgnored: true,
        updatedAt: now,
      };
    }
  }

  return {
    providerId: previous?.providerId ?? null,
    model: previous?.model ?? null,
    source: previous?.source ?? null,
    reason: "legacy_usage",
    inputTokens,
    outputTokens,
    cacheReadTokens: previous?.cacheReadTokens ?? null,
    cacheCreationTokens: previous?.cacheCreationTokens ?? null,
    reasoningTokens: previous?.reasoningTokens ?? null,
    estimatedCostMicros,
    pricingSource: previous?.pricingSource ?? null,
    costUsd,
    hasUnknownInputTokens: inputTokens === null,
    hasUnknownOutputTokens: outputTokens === null,
    hasUnknownCost: costUsd === null,
    lastEventType: "usage",
    lastProviderUsageBlockId: previous?.lastProviderUsageBlockId ?? null,
    legacyDuplicateIgnored: false,
    updatedAt: now,
  };
}

export function contextUsageFromLedger(
  ledger: SessionUsageLedgerState | null | undefined,
  contextWindowTokens: number | null | undefined,
  previous?: ContextUsageState | null,
  now = Date.now(),
): ContextUsageState | null {
  const usedTokens = sanitizeCount(ledger?.inputTokens ?? null);
  if (usedTokens === null) return previous ?? null;

  const safeWindow = sanitizeCount(contextWindowTokens ?? null);
  const percentUsed = safeWindow && safeWindow > 0
    ? clampPercent(Math.round((usedTokens / safeWindow) * 100))
    : null;

  return {
    usedTokens,
    contextWindowTokens: safeWindow,
    percentUsed,
    source: "provider_usage",
    lastUpdatedAt: now,
    lastCompactedAt: previous?.lastCompactedAt ?? null,
    compactedFromTokens: previous?.compactedFromTokens ?? null,
    compactedToTokens: previous?.compactedToTokens ?? null,
  };
}

export function usageProjectionFromProviderUsageBlocks(
  blocks: BlockState[],
  contextWindowTokens: number | null | undefined,
  previousContext?: ContextUsageState | null,
  now = Date.now(),
): UsageProjectionFromBlocks | null {
  let usageLedger: SessionUsageLedgerState | null = null;
  let contextUsage = previousContext ?? null;
  let costUsd = 0;
  const seenBlockIds = new Set<string>();

  for (const block of blocks) {
    const event = providerUsageEventFromBlock(block);
    if (!event) continue;
    const blockId = event.block_id ?? "";
    if (blockId && seenBlockIds.has(blockId)) continue;
    if (blockId) seenBlockIds.add(blockId);

    usageLedger = applyProviderUsageToLedger(usageLedger, event, now);
    if (usageLedger.costUsd !== null) {
      costUsd += usageLedger.costUsd;
    }
    contextUsage = contextUsageFromLedger(usageLedger, contextWindowTokens, contextUsage, now);
  }

  return usageLedger ? { usageLedger, contextUsage, costUsd } : null;
}

function sanitizeCount(value: number | null | undefined): number | null {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.max(0, Math.round(value))
    : null;
}

function sanitizeCost(value: number | null | undefined): number | null {
  return typeof value === "number" && Number.isFinite(value) ? Math.max(0, value) : null;
}

function sanitizeText(value: string | null | undefined): string | null {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

export function sameUsageCost(a: number | null, b: number | null): boolean {
  if (a === null || b === null) return a === b;
  return Math.round(a * 1_000_000) === Math.round(b * 1_000_000);
}

function clampPercent(value: number): number {
  return Math.max(0, Math.min(100, value));
}

function providerUsageEventFromBlock(block: BlockState): ProviderUsageEvent | null {
  if (block.event_type !== "provider_usage") return null;
  const metadata = block.metadata ?? {};
  const reason = providerUsageReason(metadata.reason);
  if (!reason) return null;
  return {
    event_type: "provider_usage",
    session_id: "",
    block_id: block.block_id,
    provider_id: stringOrNull(metadata.provider_id),
    model: stringOrNull(metadata.model),
    source: stringOrNull(metadata.source),
    reason,
    input_tokens: numberOrNull(metadata.input_tokens),
    output_tokens: numberOrNull(metadata.output_tokens),
    cache_read_tokens: numberOrNull(metadata.cache_read_tokens),
    cache_creation_tokens: numberOrNull(metadata.cache_creation_tokens),
    reasoning_tokens: numberOrNull(metadata.reasoning_tokens),
    estimated_cost_micros: numberOrNull(metadata.estimated_cost_micros),
    pricing_source: stringOrNull(metadata.pricing_source),
  };
}

function providerUsageReason(value: unknown): ProviderUsageReason | null {
  return value === "provider_reported" || value === "provider_omitted" || value === "pricing_unknown"
    ? value
    : null;
}

function numberOrNull(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function stringOrNull(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}
