# Usage And Context Ledger Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Forge use one coherent usage/context ledger so provider token usage, context-window status, cost display, and auto-compact hints cannot disagree.

**Architecture:** Treat `provider_usage` as the canonical runtime usage fact and keep legacy `usage` as a compatibility fallback only. Add a focused frontend usage projection layer that updates session cost, context usage, and message blocks from the same event semantics. Keep Rust event emission backward-compatible until the frontend proves duplicate legacy events are ignored.

**Tech Stack:** Tauri Rust backend, React/TypeScript desktop frontend, Zustand store, IndexedDB session persistence, Node test runner, Playwright e2e, Cargo tests, GitNexus impact/detect_changes.

---

## Scope Check

This plan covers one subsystem: usage and context accounting across the desktop runtime. It intentionally does not change provider routing, pricing tables, auto-compact algorithms, or billing-grade claims beyond making current usage facts internally consistent.

## Source Evidence

- `apps/desktop/src-tauri/src/protocol/events.rs` emits both legacy `usage` and canonical `provider_usage` events.
- `apps/desktop/src-tauri/src/adapters/anthropic.rs` emits `provider_usage` first, then legacy `usage` only when input/output/cost are all known.
- `apps/desktop/src/store/event-dispatch.ts` currently updates `contextUsage` and `costUsd` from legacy `usage`, while `provider_usage` only becomes a visible block.
- `apps/desktop/src/components/session/contextUsageView.ts` owns the composer context usage label and title.
- `apps/desktop/src/components/messages/ProviderUsageCard.tsx` renders `provider_usage` metadata in the conversation lane.
- `apps/desktop/src-tauri/src/loop_runtime/budget.rs` already has `UsageEvent` and `LoopUsageLedger` for loop-runtime/subagent accounting; use its semantics as reference, not as a frontend dependency.

## Product Semantics

Use these definitions everywhere:

| Term | Meaning | UI Surface |
| --- | --- | --- |
| Provider usage | Tokens/cost reported or omitted by a model provider for one model call. | Provider usage trace row |
| Context used | Best available estimate of input/context tokens currently loaded for the session. | Composer context label |
| Context remaining | `context_window_tokens - context_used`, clamped to zero. | Composer label `余 ...` |
| Auto-compact threshold | Internal threshold where Forge should compact before the provider limit. | Composer tooltip only |
| Cost | Provider usage estimated cost, non-billing-grade unless pricing source is known. | Session state and usage trace |
| Unknown usage | Provider omitted usage or pricing cannot be estimated. | `unknown`, no fake totals |

## Acceptance Contract

- **A1 Canonical Source:** A `provider_usage` event updates session usage ledger, context usage, and visible provider usage trace without requiring a legacy `usage` event.
- **A2 Compatibility:** When a canonical `provider_usage` event and its legacy `usage` companion both arrive, `costUsd` and `contextUsage` are not double-counted.
- **A3 True Remaining Context:** The composer short label shows true context-window remaining. Example: `411 / 1M · 余 999.5K`, not `余 967K`.
- **A4 Auto-Compact Separation:** Auto-compact threshold and distance remain visible in the tooltip, never in the short `余` label.
- **A5 Unknown Safety:** Unknown token or cost fields stay unknown; Forge does not invent cost or token totals.
- **A6 Persistence:** Hydrated sessions keep enough usage state to render the same composer label after reload.
- **A7 User-Visible Docs:** README, desktop README, and CHANGELOG describe the usage/context display semantics after the change.
- **A8 Verification:** Focused Node tests, Playwright e2e, Rust compatibility tests, `npm run build:desktop`, `npm --prefix apps/desktop run check:backend`, `git diff --check`, and GitNexus `detect_changes` pass.

## File Structure

- Create: `apps/desktop/src/store/usage-ledger.ts`
  - Responsibility: pure TypeScript projection helpers for canonical provider usage, legacy usage fallback, cost conversion, context usage derivation, and duplicate suppression.
- Create: `apps/desktop/src/store/usage-ledger.test.mjs`
  - Responsibility: fast TDD coverage for provider usage projection, legacy compatibility, unknown usage, and context remaining inputs.
- Modify: `apps/desktop/src/lib/protocol.ts`
  - Responsibility: add a persisted `SessionUsageLedgerState` type and `usageLedger?: SessionUsageLedgerState | null` to `SessionState`.
- Modify: `apps/desktop/src/store/types.ts`
  - Responsibility: mirror persisted session usage ledger shape.
- Modify: `apps/desktop/src/store/persistence.ts`
  - Responsibility: persist and hydrate `usageLedger`.
- Modify: `apps/desktop/src/store/session-utils.ts`
  - Responsibility: keep `buildContextUsage(...)` but move provider/legacy usage decisions into `usage-ledger.ts`.
- Modify: `apps/desktop/src/store/event-dispatch.ts`
  - Responsibility: use `usage-ledger.ts` for `provider_usage`, legacy `usage`, and `context_compacted` events.
- Modify: `apps/desktop/src/store/event-dispatch.test.ts`
  - Responsibility: prove dispatcher state updates from `provider_usage` and ignores duplicate legacy `usage`.
- Modify: `apps/desktop/src/components/session/contextUsageView.ts`
  - Responsibility: keep display semantics explicit: short label is true remaining; title includes auto-compact threshold.
- Modify: `apps/desktop/src/components/session/contextUsageView.test.mjs`
  - Responsibility: add edge cases for unknown usage and zero remaining.
- Modify: `apps/desktop/src/components/messages/ProviderUsageCard.tsx`
  - Responsibility: render known/unknown usage fields consistently with ledger labels.
- Modify: `apps/desktop/e2e/composer.spec.ts`
  - Responsibility: prove `provider_usage` alone updates the composer context label.
- Modify: `apps/desktop/e2e/messages.spec.ts`
  - Responsibility: prove provider usage trace remains visible and no assistant prose leak returns.
- Modify: `apps/desktop/src-tauri/src/adapters/anthropic.rs`
  - Responsibility: keep canonical-before-legacy emission order and document legacy compatibility.
- Modify: `apps/desktop/src-tauri/src/protocol/events.rs`
  - Responsibility: document `usage` as legacy compatibility and `provider_usage` as canonical usage fact.
- Modify: `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`
  - Responsibility: document user-visible runtime display semantics.

## Task 1: Establish The Usage Ledger Contract

**Files:**
- Create: `apps/desktop/src/store/usage-ledger.test.mjs`
- Create: `apps/desktop/src/store/usage-ledger.ts`
- Modify: `apps/desktop/src/lib/protocol.ts`

- [ ] **Step 1: Run impact analysis**

Run these before editing symbols:

```text
impact({ repo: "forge", target: "createOutputEventDispatcher", file_path: "apps/desktop/src/store/event-dispatch.ts", direction: "upstream" })
impact({ repo: "forge", target: "buildContextUsage", file_path: "apps/desktop/src/store/session-utils.ts", direction: "upstream" })
impact({ repo: "forge", target: "buildComposerContextUsageView", file_path: "apps/desktop/src/components/session/contextUsageView.ts", direction: "upstream" })
impact({ repo: "forge", target: "emit_usage_events_for_provider", file_path: "apps/desktop/src-tauri/src/adapters/anthropic.rs", direction: "upstream" })
```

If the MCP server does not expose `impact`, run:

```text
context({ repo: "forge", name: "createOutputEventDispatcher", file_path: "apps/desktop/src/store/event-dispatch.ts" })
context({ repo: "forge", name: "buildContextUsage", file_path: "apps/desktop/src/store/session-utils.ts" })
context({ repo: "forge", name: "buildComposerContextUsageView", file_path: "apps/desktop/src/components/session/contextUsageView.ts" })
context({ repo: "forge", name: "emit_usage_events_for_provider", file_path: "apps/desktop/src-tauri/src/adapters/anthropic.rs" })
```

Record whether GitNexus returned LOW/MEDIUM/HIGH/CRITICAL or an unresolved-symbol limitation. Stop and report before edits if any resolved impact is HIGH or CRITICAL.

- [ ] **Step 2: Write the failing usage-ledger tests**

Create `apps/desktop/src/store/usage-ledger.test.mjs` with:

```javascript
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
    assert.equal(ledger.lastEventType, "usage");
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
});
```

- [ ] **Step 3: Run the test to verify RED**

Run:

```bash
node --test apps/desktop/src/store/usage-ledger.test.mjs
```

Expected: FAIL because `apps/desktop/src/store/usage-ledger.ts` does not exist or exports are missing.

- [ ] **Step 4: Add the session usage ledger type**

Modify `apps/desktop/src/lib/protocol.ts` near `ContextUsageState`:

```ts
export interface SessionUsageLedgerState {
  providerId: string | null;
  model: string | null;
  source: string | null;
  reason: ProviderUsageReason | "legacy_usage";
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheCreationTokens: number | null;
  reasoningTokens: number | null;
  estimatedCostMicros: number | null;
  pricingSource: string | null;
  costUsd: number | null;
  hasUnknownInputTokens: boolean;
  hasUnknownOutputTokens: boolean;
  hasUnknownCost: boolean;
  lastEventType: "provider_usage" | "usage";
  lastProviderUsageBlockId: string | null;
  legacyDuplicateIgnored: boolean;
  updatedAt: number;
}
```

Add to `SessionState`:

```ts
usageLedger?: SessionUsageLedgerState | null;
```

- [ ] **Step 5: Implement the pure projection helper**

Create `apps/desktop/src/store/usage-ledger.ts`:

```ts
import type { ContextUsageState, SessionUsageLedgerState, StreamEvent } from "@/lib/protocol";

type ProviderUsageEvent = Extract<StreamEvent, { event_type: "provider_usage" }>;
type LegacyUsageEvent = Extract<StreamEvent, { event_type: "usage" }>;

export function applyProviderUsageToLedger(
  previous: SessionUsageLedgerState | null | undefined,
  event: ProviderUsageEvent,
  now = Date.now(),
): SessionUsageLedgerState {
  const inputTokens = finiteNumberOrNull(event.input_tokens);
  const outputTokens = finiteNumberOrNull(event.output_tokens);
  const estimatedCostMicros = finiteNumberOrNull(event.estimated_cost_micros);
  return {
    providerId: trimmedOrNull(event.provider_id),
    model: trimmedOrNull(event.model),
    source: trimmedOrNull(event.source),
    reason: event.reason,
    inputTokens,
    outputTokens,
    cacheReadTokens: finiteNumberOrNull(event.cache_read_tokens),
    cacheCreationTokens: finiteNumberOrNull(event.cache_creation_tokens),
    reasoningTokens: finiteNumberOrNull(event.reasoning_tokens),
    estimatedCostMicros,
    pricingSource: trimmedOrNull(event.pricing_source),
    costUsd: estimatedCostMicros === null ? null : estimatedCostMicros / 1_000_000,
    hasUnknownInputTokens: inputTokens === null,
    hasUnknownOutputTokens: outputTokens === null,
    hasUnknownCost: estimatedCostMicros === null,
    lastEventType: "provider_usage",
    lastProviderUsageBlockId: trimmedOrNull(event.block_id),
    legacyDuplicateIgnored: previous?.legacyDuplicateIgnored ?? false,
    updatedAt: now,
  };
}

export function applyLegacyUsageToLedger(
  previous: SessionUsageLedgerState | null | undefined,
  event: LegacyUsageEvent,
  now = Date.now(),
): SessionUsageLedgerState {
  if (isDuplicateLegacyUsage(previous, event)) {
    return {
      ...previous!,
      legacyDuplicateIgnored: true,
      updatedAt: now,
    };
  }

  return {
    providerId: previous?.providerId ?? null,
    model: previous?.model ?? null,
    source: previous?.source ?? null,
    reason: "legacy_usage",
    inputTokens: Math.max(0, Math.round(event.input_tokens)),
    outputTokens: Math.max(0, Math.round(event.output_tokens)),
    cacheReadTokens: previous?.cacheReadTokens ?? null,
    cacheCreationTokens: previous?.cacheCreationTokens ?? null,
    reasoningTokens: previous?.reasoningTokens ?? null,
    estimatedCostMicros: Math.max(0, Math.round(event.estimated_cost_usd * 1_000_000)),
    pricingSource: previous?.pricingSource ?? null,
    costUsd: Math.max(0, event.estimated_cost_usd),
    hasUnknownInputTokens: false,
    hasUnknownOutputTokens: false,
    hasUnknownCost: false,
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
  if (!ledger || ledger.inputTokens === null) return previous ?? null;
  const safeWindow = finiteNumberOrNull(contextWindowTokens);
  const percentUsed = safeWindow && safeWindow > 0
    ? Math.min(100, Math.round((ledger.inputTokens / safeWindow) * 100))
    : null;
  return {
    usedTokens: ledger.inputTokens,
    contextWindowTokens: safeWindow,
    percentUsed,
    source: "provider_usage",
    lastUpdatedAt: now,
    lastCompactedAt: previous?.lastCompactedAt ?? null,
    compactedFromTokens: previous?.compactedFromTokens ?? null,
    compactedToTokens: previous?.compactedToTokens ?? null,
  };
}

function isDuplicateLegacyUsage(
  previous: SessionUsageLedgerState | null | undefined,
  event: LegacyUsageEvent,
) {
  if (!previous || previous.lastEventType !== "provider_usage") return false;
  if (previous.inputTokens !== event.input_tokens) return false;
  if (previous.outputTokens !== event.output_tokens) return false;
  if (previous.costUsd === null) return false;
  return Math.abs(previous.costUsd - event.estimated_cost_usd) < 0.0000005;
}

function finiteNumberOrNull(value: number | null | undefined) {
  return typeof value === "number" && Number.isFinite(value) ? Math.max(0, Math.round(value)) : null;
}

function trimmedOrNull(value: string | null | undefined) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}
```

- [ ] **Step 6: Run the usage-ledger test to verify GREEN**

Run:

```bash
node --test apps/desktop/src/store/usage-ledger.test.mjs
```

Expected: PASS.

## Task 2: Wire The Ledger Into Session Dispatch And Persistence

**Files:**
- Modify: `apps/desktop/src/store/types.ts`
- Modify: `apps/desktop/src/store/persistence.ts`
- Modify: `apps/desktop/src/store/session-utils.ts`
- Modify: `apps/desktop/src/store/event-dispatch.ts`
- Modify: `apps/desktop/src/store/event-dispatch.test.ts`

- [ ] **Step 1: Write failing dispatcher tests**

Extend `apps/desktop/src/store/event-dispatch.test.ts` in the existing usage describe block with:

```ts
it("updates context usage and cost from provider_usage without legacy usage", () => {
  const { state, dispatch } = createHarness();

  dispatch({
    event_type: "provider_usage",
    session_id: "session-1",
    block_id: "usage-provider-only",
    provider_id: "deepseek",
    model: "deepseek-v4-flash[1m]",
    source: "anthropic",
    reason: "provider_reported",
    input_tokens: 411,
    output_tokens: 137,
    estimated_cost_micros: 96,
  });

  const session = state.sessions.get("session-1")!;
  assert.strictEqual(session.usageLedger?.inputTokens, 411);
  assert.strictEqual(session.usageLedger?.estimatedCostMicros, 96);
  assert.strictEqual(session.costUsd, 0.000096);
  assert.strictEqual(session.contextUsage?.usedTokens, 411);
  assert.strictEqual(session.blocks.at(-1)?.event_type, "provider_usage");
});

it("does not double count legacy usage after provider_usage companion", () => {
  const { state, dispatch } = createHarness();

  dispatch({
    event_type: "provider_usage",
    session_id: "session-1",
    block_id: "usage-provider-first",
    provider_id: "deepseek",
    model: "deepseek-v4-flash[1m]",
    source: "anthropic",
    reason: "provider_reported",
    input_tokens: 411,
    output_tokens: 137,
    estimated_cost_micros: 96,
  });
  dispatch({
    event_type: "usage",
    session_id: "session-1",
    input_tokens: 411,
    output_tokens: 137,
    estimated_cost_usd: 0.000096,
  });

  const session = state.sessions.get("session-1")!;
  assert.strictEqual(session.costUsd, 0.000096);
  assert.strictEqual(session.contextUsage?.usedTokens, 411);
  assert.strictEqual(session.usageLedger?.legacyDuplicateIgnored, true);
});
```

- [ ] **Step 2: Run dispatcher tests to verify RED**

Run:

```bash
node --test apps/desktop/src/store/event-dispatch.test.ts
```

Expected: FAIL because `provider_usage` does not yet update `contextUsage`, `usageLedger`, or `costUsd`.

- [ ] **Step 3: Persist the ledger shape**

Modify `apps/desktop/src/store/types.ts` and `apps/desktop/src/store/persistence.ts`:

```ts
usageLedger?: PersistedSession["usageLedger"];
```

Persist:

```ts
usageLedger: s.usageLedger ?? null,
```

Hydrate:

```ts
usageLedger: s.usageLedger ?? null,
```

Ensure new sessions start with:

```ts
usageLedger: null,
```

- [ ] **Step 4: Route provider and legacy events through the ledger helper**

Modify `apps/desktop/src/store/event-dispatch.ts`.

For `event_type === "provider_usage"`:

```ts
const providerEvent = event as Extract<StreamEvent, { event_type: "provider_usage" }>;
const contextWindowTokens = session.contextWindowTokens ?? getModelContextWindow(session.model);
const usageLedger = applyProviderUsageToLedger(session.usageLedger, providerEvent);
const contextUsage = contextUsageFromLedger(usageLedger, contextWindowTokens, session.contextUsage);
const newBlock = eventToBlock(event);
if (newBlock) blocks.push(newBlock);
sessions.set(session_id, touchSession(session, {
  blocks,
  usageLedger,
  contextUsage,
  costUsd: usageLedger.costUsd ?? session.costUsd,
}));
set({ sessions });
persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
persistBlocks(session_id, blocks);
return;
```

For legacy `usage`:

```ts
const usageEvent = event as Extract<StreamEvent, { event_type: "usage" }>;
const contextWindowTokens = session.contextWindowTokens ?? getModelContextWindow(session.model);
const usageLedger = applyLegacyUsageToLedger(session.usageLedger, usageEvent);
const contextUsage = contextUsageFromLedger(usageLedger, contextWindowTokens, session.contextUsage);
sessions.set(session_id, {
  ...session,
  costUsd: usageLedger.costUsd ?? session.costUsd,
  usageLedger,
  contextUsage,
  blocks,
  updatedAt: Date.now(),
});
set({ sessions });
persistSessions(sessions, get().workflowBySession, get().deliverySummaryBySession);
return;
```

- [ ] **Step 5: Preserve compaction metadata**

When handling `context_compacted`, keep the existing local-estimate `buildContextUsage(...)` behavior and do not clear `usageLedger`.

Expected state after compaction:

```ts
contextUsage.source === "local_estimate"
usageLedger.lastEventType === "provider_usage" || usageLedger.lastEventType === "usage"
```

- [ ] **Step 6: Run dispatcher tests to verify GREEN**

Run:

```bash
node --test apps/desktop/src/store/event-dispatch.test.ts
```

Expected: PASS.

## Task 3: Keep Backend Event Contract Backward-Compatible

**Files:**
- Modify: `apps/desktop/src-tauri/src/adapters/anthropic.rs`
- Modify: `apps/desktop/src-tauri/src/protocol/events.rs`

- [ ] **Step 1: Add or extend Rust compatibility tests**

In `apps/desktop/src-tauri/src/adapters/anthropic.rs`, add a test near the existing usage event tests:

```rust
#[test]
fn emit_usage_events_keeps_provider_usage_before_legacy_usage() {
    let emitter = CaptureEmitter::default();
    emit_usage_events_for_provider(
        &emitter,
        "session-1",
        "deepseek",
        "anthropic",
        "deepseek-v4-flash[1m]",
        Some(ProviderTokenUsage {
            input_tokens: 411,
            output_tokens: 137,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            reasoning_tokens: None,
        }),
    );

    let events = emitter.events();
    assert_eq!(events[0].event_type(), "provider_usage");
    assert_eq!(events[1].event_type(), "usage");
}
```

If this exact helper does not exist in the file, reuse the local capture emitter pattern already used by nearby usage tests and assert the same event order.

- [ ] **Step 2: Run the Rust test to verify current behavior**

Run:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml emit_usage_events_keeps_provider_usage_before_legacy_usage
```

Expected: PASS. This task documents compatibility; it should not require behavior changes.

- [ ] **Step 3: Document canonical vs legacy events in Rust protocol**

Add comments in `apps/desktop/src-tauri/src/protocol/events.rs`:

```rust
// Legacy compatibility event for older frontend projections.
// New UI/state code should prefer ProviderUsage because it preserves provider,
// model, unknown-token, cache, reasoning, pricing-source, and reason fields.
#[serde(rename = "usage")]
Usage { ... }

// Canonical provider usage fact for one model call.
#[serde(rename = "provider_usage")]
ProviderUsage { ... }
```

- [ ] **Step 4: Run backend checks for this slice**

Run:

```bash
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_usage
```

Expected: PASS.

## Task 4: Align UI Consumers With The Ledger Semantics

**Files:**
- Modify: `apps/desktop/src/components/session/contextUsageView.ts`
- Modify: `apps/desktop/src/components/session/contextUsageView.test.mjs`
- Modify: `apps/desktop/src/components/messages/ProviderUsageCard.tsx`
- Modify: `apps/desktop/e2e/composer.spec.ts`
- Modify: `apps/desktop/e2e/messages.spec.ts`

- [ ] **Step 1: Add context view-model edge tests**

Extend `apps/desktop/src/components/session/contextUsageView.test.mjs`:

```javascript
it("labels unknown provider usage without showing fake remaining context", () => {
  const view = buildComposerContextUsageView({
    fallbackContextWindowTokens: 1_000_000,
    isCompacting: false,
    isStreaming: false,
    usage: {
      usedTokens: null,
      contextWindowTokens: 1_000_000,
      percentUsed: null,
      source: "provider_usage",
      lastUpdatedAt: 1,
    },
  });
  assert.equal(view.label, "");
  assert.equal(view.title, "压缩当前上下文");
});

it("clamps exhausted context remaining to zero", () => {
  const view = buildComposerContextUsageView({
    fallbackContextWindowTokens: null,
    isCompacting: false,
    isStreaming: false,
    usage: {
      usedTokens: 1_005_000,
      contextWindowTokens: 1_000_000,
      percentUsed: 100,
      source: "provider_usage",
      lastUpdatedAt: 1,
    },
  });
  assert.equal(view.label, "1M / 1M · 余 0");
  assert.match(view.title, /剩余上下文约 0 tokens/);
});
```

- [ ] **Step 2: Run context view-model tests**

Run:

```bash
node --test apps/desktop/src/components/session/contextUsageView.test.mjs
```

Expected: PASS. If it fails, update only `formatComposerContextUsage(...)` and `formatComposerContextUsageTitle(...)` so `usedTokens: null` returns an empty label and the default `压缩当前上下文` title.

- [ ] **Step 3: Add provider_usage-only composer e2e**

Extend `apps/desktop/e2e/composer.spec.ts`:

```ts
test("composer updates context usage from provider_usage without legacy usage", async ({ page }) => {
  const sessionId = crypto.randomUUID();
  await page.addInitScript((sessionId) => {
    // @ts-expect-error mock
    window.__mockSessionId = sessionId;
  }, sessionId);

  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await expect(page.locator("textarea")).toBeVisible();
  await page.waitForFunction(() => {
    // @ts-expect-error Tauri listener registry installed by setup()
    return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
  });

  await simulateStream(page, sessionId, [
    {
      event_type: "session_started",
      session_id: sessionId,
      agent_type: "deepseek",
      model: "deepseek-v4-flash[1m]",
      context_window_tokens: 1_000_000,
    },
    {
      event_type: "provider_usage",
      session_id: sessionId,
      block_id: "usage-composer-provider-only",
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
    },
  ], 1);

  await expect(page.getByTestId("composer-context-usage")).toContainText("411 / 1M · 余 999.5K");
  await expect(page.getByTestId("composer-context-usage")).toHaveAttribute(
    "title",
    /自动压缩阈值 967K/,
  );
});
```

- [ ] **Step 4: Run focused e2e**

Run:

```bash
npm --prefix apps/desktop run test:e2e -- e2e/composer.spec.ts -g "provider_usage without legacy usage"
```

Expected: PASS.

- [ ] **Step 5: Keep provider usage trace rendering stable**

Run:

```bash
npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g "provider usage"
```

Expected: PASS, with one `provider-usage-card` and zero assistant prose leaks containing `模型用量 · provider`.

## Task 5: Documentation And Acceptance Visibility

**Files:**
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`
- Modify: `scripts/acceptance.sh`

- [ ] **Step 1: Update user-visible docs**

Add this wording to the desktop runtime or usage section in `README.md` and `apps/desktop/README.md`:

```markdown
Forge separates model usage from context-window status. Provider usage rows show the model call's reported tokens/cost, while the composer context indicator shows estimated context used and true remaining context. Auto-compact threshold distance appears in the tooltip so it is not confused with provider context remaining.
```

- [ ] **Step 2: Update CHANGELOG**

Add:

```markdown
- Fixed usage/context accounting so provider usage events update the composer context indicator without relying on legacy usage events, and the `余` label now means true context remaining rather than auto-compact threshold distance.
```

- [ ] **Step 3: Update acceptance dry-run labels**

In `scripts/acceptance.sh --dry-run`, ensure the advertised desktop checks include:

```text
composer context usage from provider_usage
provider usage trace rendering
legacy usage duplicate suppression
```

- [ ] **Step 4: Run dry-run acceptance**

Run:

```bash
scripts/acceptance.sh --dry-run
```

Expected: output mentions the three usage/context checks above.

## Task 6: Full Verification And Change Detection

**Files:**
- No new source files beyond previous tasks.

- [ ] **Step 1: Run fast frontend tests**

Run:

```bash
node --test apps/desktop/src/store/usage-ledger.test.mjs
node --test apps/desktop/src/store/event-dispatch.test.ts
node --test apps/desktop/src/components/session/contextUsageView.test.mjs
```

Expected: PASS.

- [ ] **Step 2: Run focused e2e**

Run:

```bash
npm --prefix apps/desktop run test:e2e -- e2e/composer.spec.ts -g "context usage"
npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts -g "provider usage"
```

Expected: PASS.

- [ ] **Step 3: Run backend verification**

Run:

```bash
npm --prefix apps/desktop run check:backend
```

Expected: cargo fmt, clippy, Rust unit tests, and Rust integration tests pass.

- [ ] **Step 4: Run desktop build**

Run:

```bash
npm run build:desktop
```

Expected: TypeScript and Vite build pass.

- [ ] **Step 5: Run whitespace and GitNexus checks**

Run:

```bash
git diff --check
```

Then:

```text
detect_changes({ repo: "forge", scope: "all" })
```

Expected: no whitespace errors; GitNexus reports no HIGH/CRITICAL unexpected affected processes. If the working tree contains unrelated prior changes, record that limitation and inspect the scoped diff for the usage/context files.

## Execution Order

1. Task 1: pure ledger contract.
2. Task 2: dispatcher and persistence.
3. Task 3: backend compatibility documentation/test.
4. Task 4: UI and e2e consumers.
5. Task 5: docs and acceptance labels.
6. Task 6: full verification.

This order keeps the riskiest semantic change in pure tests first, then wires it into the running app.

## Self-Review

- Spec coverage: A1 through A8 each map to at least one task and test command.
- Placeholder scan: no task depends on unspecified deferred work; each test step includes concrete code and command.
- Type consistency: `SessionUsageLedgerState`, `usageLedger`, `provider_usage`, `usage`, and `ContextUsageState` names match the existing TypeScript/Rust protocol names.
