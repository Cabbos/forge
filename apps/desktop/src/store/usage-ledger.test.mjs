import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { describe, it } from "node:test";
import ts from "typescript";

async function importUsageLedger() {
  const source = await readFile(new URL("./usage-ledger.ts", import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2020,
    },
    fileName: "usage-ledger.ts",
  });
  return import(`data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`);
}

const {
  applyProviderUsageToLedger,
  applyLegacyUsageToLedger,
  contextUsageFromLedger,
  usageProjectionFromProviderUsageBlocks,
} = await importUsageLedger();

const providerUsage = {
  event_type: "provider_usage",
  session_id: "s1",
  block_id: "usage-1",
  provider_id: "deepseek",
  model: "deepseek-v4-flash[1m]",
  source: "anthropic",
  reason: "provider_reported",
  input_tokens: 411,
  output_tokens: 137,
  cache_read_tokens: null,
  cache_creation_tokens: null,
  reasoning_tokens: null,
  estimated_cost_micros: 96,
  pricing_source: "forge_static_pricing_2026_06_20",
};

describe("usage ledger projection", () => {
  it("uses provider_usage as canonical cost and context source", () => {
    const ledger = applyProviderUsageToLedger(null, providerUsage);
    assert.equal(ledger.providerId, "deepseek");
    assert.equal(ledger.model, "deepseek-v4-flash[1m]");
    assert.equal(ledger.inputTokens, 411);
    assert.equal(ledger.outputTokens, 137);
    assert.equal(ledger.estimatedCostMicros, 96);
    assert.equal(ledger.costUsd, 0.000096);
    assert.equal(ledger.lastEventType, "provider_usage");

    const context = contextUsageFromLedger(ledger, 1_000_000, null);
    assert.equal(context.usedTokens, 411);
    assert.equal(context.contextWindowTokens, 1_000_000);
    assert.equal(context.source, "provider_usage");
  });

  it("ignores duplicate legacy usage after matching canonical provider_usage", () => {
    const canonical = applyProviderUsageToLedger(null, providerUsage);
    const afterLegacy = applyLegacyUsageToLedger(canonical, {
      event_type: "usage",
      session_id: "s1",
      input_tokens: 411,
      output_tokens: 137,
      estimated_cost_usd: 0.000096,
    });
    assert.equal(afterLegacy.inputTokens, 411);
    assert.equal(afterLegacy.outputTokens, 137);
    assert.equal(afterLegacy.costUsd, 0.000096);
    assert.equal(afterLegacy.legacyDuplicateIgnored, true);
  });

  it("uses non-duplicate legacy usage after canonical provider_usage as fallback update", () => {
    const canonical = applyProviderUsageToLedger(null, providerUsage);
    const afterLegacy = applyLegacyUsageToLedger(canonical, {
      event_type: "usage",
      session_id: "s1",
      input_tokens: 500,
      output_tokens: 200,
      estimated_cost_usd: 0.0002,
    });
    assert.equal(afterLegacy.inputTokens, 500);
    assert.equal(afterLegacy.outputTokens, 200);
    assert.equal(afterLegacy.costUsd, 0.0002);
    assert.equal(afterLegacy.estimatedCostMicros, 200);
    assert.equal(afterLegacy.lastEventType, "usage");
    assert.equal(afterLegacy.legacyDuplicateIgnored, false);
  });

  it("keeps legacy usage as fallback when no canonical event exists", () => {
    const ledger = applyLegacyUsageToLedger(null, {
      event_type: "usage",
      session_id: "s1",
      input_tokens: 142_000,
      output_tokens: 800,
      estimated_cost_usd: 0.002,
    });
    assert.equal(ledger.inputTokens, 142_000);
    assert.equal(ledger.outputTokens, 800);
    assert.equal(ledger.costUsd, 0.002);
    assert.equal(ledger.estimatedCostMicros, 2000);
    assert.equal(ledger.lastEventType, "usage");
  });

  it("keeps invalid legacy cost unknown in micros and dollars", () => {
    const ledger = applyLegacyUsageToLedger(null, {
      event_type: "usage",
      session_id: "s1",
      input_tokens: 142_000,
      output_tokens: 800,
      estimated_cost_usd: Number.NaN,
    });
    assert.equal(ledger.costUsd, null);
    assert.equal(ledger.estimatedCostMicros, null);
    assert.equal(ledger.hasUnknownCost, true);
  });

  it("does not invent unknown provider usage", () => {
    const ledger = applyProviderUsageToLedger(null, {
      ...providerUsage,
      input_tokens: null,
      output_tokens: null,
      estimated_cost_micros: null,
      reason: "provider_omitted",
    });
    assert.equal(ledger.inputTokens, null);
    assert.equal(ledger.outputTokens, null);
    assert.equal(ledger.estimatedCostMicros, null);
    assert.equal(ledger.costUsd, null);
    assert.equal(ledger.hasUnknownInputTokens, true);
    assert.equal(ledger.hasUnknownOutputTokens, true);
    assert.equal(ledger.hasUnknownCost, true);
  });

  it("does not carry stale provider metadata when omitted by a later provider event", () => {
    const first = applyProviderUsageToLedger(null, providerUsage);
    const second = applyProviderUsageToLedger(first, {
      ...providerUsage,
      block_id: null,
      provider_id: null,
      model: null,
      source: null,
      pricing_source: null,
    });
    assert.equal(second.providerId, null);
    assert.equal(second.model, null);
    assert.equal(second.source, null);
    assert.equal(second.pricingSource, null);
    assert.equal(second.lastProviderUsageBlockId, null);
  });

  it("returns previous context usage when ledger input tokens are unknown", () => {
    const previous = {
      usedTokens: 100,
      contextWindowTokens: 1_000,
      percentUsed: 10,
      source: "local_estimate",
      lastUpdatedAt: 123,
    };
    const ledger = applyProviderUsageToLedger(null, {
      ...providerUsage,
      input_tokens: null,
      output_tokens: null,
      estimated_cost_micros: null,
    });
    assert.strictEqual(contextUsageFromLedger(ledger, 1_000, previous), previous);
  });

  it("preserves compact metadata when projecting context usage", () => {
    const previous = {
      usedTokens: 32_000,
      contextWindowTokens: 1_000_000,
      percentUsed: 3,
      source: "local_estimate",
      lastUpdatedAt: 123,
      lastCompactedAt: 456,
      compactedFromTokens: 142_000,
      compactedToTokens: 32_000,
    };
    const ledger = applyProviderUsageToLedger(null, providerUsage);
    const context = contextUsageFromLedger(ledger, 1_000_000, previous, 789);
    assert.equal(context.lastUpdatedAt, 789);
    assert.equal(context.lastCompactedAt, 456);
    assert.equal(context.compactedFromTokens, 142_000);
    assert.equal(context.compactedToTokens, 32_000);
  });

  it("rounds and clamps percent while sanitizing context windows", () => {
    const ledger = applyProviderUsageToLedger(null, {
      ...providerUsage,
      input_tokens: 1_500,
    });
    const overWindow = contextUsageFromLedger(ledger, 1_000, null);
    const invalidWindow = contextUsageFromLedger(ledger, Number.NaN, null);
    const roundedWindow = contextUsageFromLedger(ledger, 3333.6, null);

    assert.equal(overWindow.percentUsed, 100);
    assert.equal(invalidWindow.contextWindowTokens, null);
    assert.equal(invalidWindow.percentUsed, null);
    assert.equal(roundedWindow.contextWindowTokens, 3334);
    assert.equal(roundedWindow.percentUsed, 45);
  });

  it("replays compacted context blocks as the latest local estimate", () => {
    const projection = usageProjectionFromProviderUsageBlocks([
      {
        block_id: "usage-before-compact",
        event_type: "provider_usage",
        content: "provider usage",
        isComplete: true,
        metadata: {
          provider_id: "deepseek",
          model: "deepseek-v4-flash[1m]",
          source: "anthropic",
          reason: "provider_reported",
          input_tokens: 142_000,
          output_tokens: 800,
          cache_read_tokens: null,
          cache_creation_tokens: null,
          reasoning_tokens: null,
          estimated_cost_micros: 2000,
          pricing_source: "forge_static_pricing_2026_06_20",
        },
      },
      {
        block_id: "compact-after-usage",
        event_type: "context_compacted",
        content: "Compacted context",
        isComplete: true,
        metadata: {
          retained_messages: 2,
          compacted_messages: 3,
          estimated_tokens_before: 142_000,
          estimated_tokens_after: 32_000,
        },
      },
    ], 1_000_000, null, 999);

    assert.equal(projection.contextUsage.usedTokens, 32_000);
    assert.equal(projection.contextUsage.source, "local_estimate");
    assert.equal(projection.contextUsage.lastCompactedAt, 999);
    assert.equal(projection.contextUsage.compactedFromTokens, 142_000);
    assert.equal(projection.contextUsage.compactedToTokens, 32_000);
    assert.equal(projection.usageLedger.inputTokens, 142_000);
    assert.equal(projection.costUsd, 0.002);
  });

  it("replays compacted context blocks even when provider usage blocks were pruned", () => {
    const projection = usageProjectionFromProviderUsageBlocks([
      {
        block_id: "compact-only",
        event_type: "context_compacted",
        content: "Compacted context",
        isComplete: true,
        metadata: {
          retained_messages: 2,
          compacted_messages: 3,
          estimated_tokens_before: 142_000,
          estimated_tokens_after: 32_000,
        },
      },
    ], 1_000_000, null, 999);

    assert.equal(projection.usageLedger, null);
    assert.equal(projection.costUsd, 0);
    assert.equal(projection.contextUsage.usedTokens, 32_000);
    assert.equal(projection.contextUsage.source, "local_estimate");
  });
});
