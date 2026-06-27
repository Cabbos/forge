import assert from "node:assert/strict";
import { beforeEach, describe, it } from "node:test";
import { register } from "node:module";
import type { SessionState, SessionUsageLedgerState } from "../lib/protocol.ts";
import type { AppStore } from "./types";

declare global {
  // eslint-disable-next-line no-var
  var __forgeTestIdb: Map<string, unknown> | undefined;
}

const idbKeyvalStub = `
function store() {
  if (!globalThis.__forgeTestIdb) globalThis.__forgeTestIdb = new Map();
  return globalThis.__forgeTestIdb;
}
export async function get(key) {
  return store().get(key);
}
export async function set(key, value) {
  store().set(key, value);
}
export async function del(key) {
  store().delete(key);
}
`;

const tauriCoreStub = `
export async function invoke(command) {
  throw new Error("Unexpected Tauri invoke in store persistence test: " + command);
}
`;

const tauriDialogStub = `
export async function open() {
  return null;
}
`;

register(
  `data:text/javascript,${encodeURIComponent(`
    const stubs = new Map([
      ["idb-keyval", ${JSON.stringify(`data:text/javascript,${encodeURIComponent(idbKeyvalStub)}`)}],
      ["@tauri-apps/api/core", ${JSON.stringify(`data:text/javascript,${encodeURIComponent(tauriCoreStub)}`)}],
      ["@tauri-apps/plugin-dialog", ${JSON.stringify(`data:text/javascript,${encodeURIComponent(tauriDialogStub)}`)}],
    ]);

    export async function resolve(specifier, context, nextResolve) {
      const stub = stubs.get(specifier);
      if (stub) return { url: stub, shortCircuit: true };
      try {
        return await nextResolve(specifier, context);
      } catch (error) {
        if (
          error?.code === "ERR_MODULE_NOT_FOUND" &&
          (specifier.startsWith("./") || specifier.startsWith("../")) &&
          !specifier.endsWith(".ts")
        ) {
          return nextResolve(specifier + ".ts", context);
        }
        throw error;
      }
    }
  `)}`,
  import.meta.url,
);

Object.assign(globalThis, {
  window: {
    localStorage: {
      getItem: () => null,
      setItem: () => {},
      removeItem: () => {},
    },
  },
});

const {
  ACTIVE_SESSION_KEY,
  BLOCKS_PREFIX,
  PERSIST_KEY,
  persistSessions,
} = await import("./persistence.ts");
const { createHydrateAction } = await import("./hydration.ts");
const { createSessionActions } = await import("./session-actions.ts");

describe("store usageLedger persistence and hydration", () => {
  beforeEach(() => {
    globalThis.__forgeTestIdb = new Map();
  });

  it("persists usageLedger in the session payload", async () => {
    const usageLedger = testUsageLedger();
    const sessions = new Map<string, SessionState>([
      ["session-1", testSession({ usageLedger })],
    ]);

    await persistSessions(sessions, new Map(), new Map());

    const persisted = testIdb().get(PERSIST_KEY) as Array<{ usageLedger?: SessionUsageLedgerState | null }>;
    assert.equal(persisted.length, 1);
    assert.deepEqual(persisted[0].usageLedger, usageLedger);
  });

  it("hydrates usageLedger from a persisted session", async () => {
    const usageLedger = testUsageLedger();
    testIdb().set(PERSIST_KEY, [
      {
        id: "session-1",
        agentType: "codex",
        model: "deepseek-v4-flash[1m]",
        workingDir: "/workspace",
        workspaceId: "/workspace",
        createdAt: 10,
        updatedAt: 20,
        contextWindowTokens: 1_000_000,
        contextUsage: null,
        usageLedger,
        status: "running",
        workflowState: null,
        deliverySummary: null,
      },
    ]);
    testIdb().set(ACTIVE_SESSION_KEY, "session-1");
    const state = createStoreState();
    const hydrate = createHydrateAction(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    await hydrate();

    assert.deepEqual(state.sessions.get("session-1")?.usageLedger, usageLedger);
  });

  it("restores cost from provider_usage blocks while keeping a persisted usageLedger", async () => {
    const usageLedger = testUsageLedger({ inputTokens: 777 });
    testIdb().set(PERSIST_KEY, [
      {
        id: "session-1",
        agentType: "codex",
        model: "deepseek-v4-flash[1m]",
        workingDir: "/workspace",
        workspaceId: "/workspace",
        createdAt: 10,
        updatedAt: 20,
        contextWindowTokens: 1_000_000,
        contextUsage: null,
        usageLedger,
        status: "running",
        workflowState: null,
        deliverySummary: null,
      },
    ]);
    testIdb().set(BLOCKS_PREFIX + "session-1", [
      {
        block_id: "usage-from-block-with-persisted-ledger",
        event_type: "provider_usage",
        content: "provider usage",
        isComplete: true,
        metadata: {
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
      },
    ]);
    const state = createStoreState();
    const hydrate = createHydrateAction(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    await hydrate();

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.costUsd, 0.000096);
    assert.deepEqual(session.usageLedger, usageLedger);
  });

  it("reconstructs usageLedger, context usage, and cost from restored provider_usage blocks", async () => {
    testIdb().set(PERSIST_KEY, [
      {
        id: "session-1",
        agentType: "codex",
        model: "deepseek-v4-flash[1m]",
        workingDir: "/workspace",
        workspaceId: "/workspace",
        createdAt: 10,
        updatedAt: 20,
        contextWindowTokens: 1_000_000,
        contextUsage: null,
        usageLedger: null,
        status: "running",
        workflowState: null,
        deliverySummary: null,
      },
    ]);
    testIdb().set(BLOCKS_PREFIX + "session-1", [
      {
        block_id: "usage-from-block",
        event_type: "provider_usage",
        content: "provider usage",
        isComplete: true,
        metadata: {
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
      },
    ]);
    const state = createStoreState();
    const hydrate = createHydrateAction(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    await hydrate();

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.usageLedger?.lastEventType, "provider_usage");
    assert.strictEqual(session.usageLedger?.lastProviderUsageBlockId, "usage-from-block");
    assert.strictEqual(session.usageLedger?.inputTokens, 411);
    assert.strictEqual(session.usageLedger?.estimatedCostMicros, 96);
    assert.strictEqual(session.contextUsage?.usedTokens, 411);
    assert.strictEqual(session.contextUsage?.contextWindowTokens, 1_000_000);
    assert.strictEqual(session.costUsd, 0.000096);
  });

  it("hydrates usage ledger and composer context label from provider usage blocks after reload", async () => {
    testIdb().set(PERSIST_KEY, [
      {
        id: "session-1",
        agentType: "codex",
        model: "deepseek-v4-flash[1m]",
        workingDir: "/workspace",
        workspaceId: "/workspace",
        createdAt: 10,
        updatedAt: 20,
        contextWindowTokens: 1_000_000,
        contextUsage: null,
        usageLedger: null,
        status: "running",
        workflowState: null,
        deliverySummary: null,
      },
    ]);
    testIdb().set(BLOCKS_PREFIX + "session-1", [
      {
        block_id: "usage-after-first-turn",
        event_type: "provider_usage",
        content: "provider usage",
        isComplete: true,
        metadata: {
          provider_id: "deepseek",
          model: "deepseek-v4-flash[1m]",
          source: "anthropic",
          reason: "provider_reported",
          input_tokens: 100,
          output_tokens: 40,
          cache_read_tokens: null,
          cache_creation_tokens: null,
          reasoning_tokens: null,
          estimated_cost_micros: 50,
          pricing_source: "forge_static_pricing_2026_06_20",
        },
      },
      {
        block_id: "usage-after-second-turn",
        event_type: "provider_usage",
        content: "provider usage",
        isComplete: true,
        metadata: {
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
      },
    ]);
    const state = createStoreState();
    const hydrate = createHydrateAction(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    await hydrate();

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.usageLedger?.inputTokens, 411);
    assert.strictEqual(session.usageLedger?.outputTokens, 137);
    assert.strictEqual(session.usageLedger?.estimatedCostMicros, 96);
    assert.strictEqual(session.usageLedger?.costUsd, 0.000096);
    assert.strictEqual(session.costUsd, 0.000146);
    assert.strictEqual(session.contextUsage?.usedTokens, 411);
    assert.strictEqual(session.contextUsage?.contextWindowTokens, 1_000_000);
    assert.strictEqual(session.contextUsage?.source, "provider_usage");
    assert.strictEqual(session.contextUsage?.percentUsed, 0);

    const composerContextLabelInput = {
      fallbackContextWindowTokens: session.contextWindowTokens,
      isCompacting: false,
      isStreaming: session.streaming,
      usage: session.contextUsage,
    };
    assert.strictEqual(composerContextLabelInput.usage?.usedTokens, session.usageLedger?.inputTokens);
    assert.strictEqual(
      composerContextLabelInput.usage?.contextWindowTokens,
      composerContextLabelInput.fallbackContextWindowTokens,
    );
    assert.strictEqual(composerContextLabelInput.usage?.source, "provider_usage");
    assert.strictEqual(composerContextLabelInput.isStreaming, false);
  });

  it("initializes a new session with a null usageLedger", () => {
    const state = createStoreState();
    const actions = createSessionActions(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    actions.addSession("session-1", "codex", "deepseek-v4-flash[1m]", "/workspace");

    assert.strictEqual(state.sessions.get("session-1")?.usageLedger, null);
  });
});

function createStoreState(): AppStore {
  return {
    sessions: new Map(),
    activeSessionId: null,
    hydrated: false,
    workspaces: new Map(),
    activeWorkspaceId: null,
    memories: [],
    selectedContextBySession: new Map(),
    forgeWikiContextBySession: new Map(),
    mcpContextBySession: new Map(),
    mcpContextStatusBySession: new Map(),
    forgeWikiProposalsBySession: new Map(),
    workflowBySession: new Map(),
    agentTurnBySession: new Map(),
    firstLoopDraftBySession: new Map(),
    deliverySummaryBySession: new Map(),
    agentA2ABySession: new Map(),
    subagentRuntimeByTask: new Map(),
    loopRuntimeByTask: new Map(),
    recoveryNotices: [],
    healthAlerts: [],
    selectedProvider: "deepseek",
    selectedModel: "deepseek-v4-flash[1m]",
    theme: "light",
  } as unknown as AppStore;
}

function testIdb(): Map<string, unknown> {
  if (!globalThis.__forgeTestIdb) {
    globalThis.__forgeTestIdb = new Map();
  }
  return globalThis.__forgeTestIdb;
}

function testSession(overrides: Partial<SessionState> = {}): SessionState {
  return {
    id: "session-1",
    agentType: "codex",
    model: "deepseek-v4-flash[1m]",
    workingDir: "/workspace",
    workspaceId: "/workspace",
    createdAt: 10,
    updatedAt: 20,
    contextWindowTokens: 1_000_000,
    status: "running",
    streaming: false,
    blocks: [],
    costUsd: 0.000096,
    contextUsage: null,
    usageLedger: null,
    ...overrides,
  };
}

function testUsageLedger(overrides: Partial<SessionUsageLedgerState> = {}): SessionUsageLedgerState {
  return {
    providerId: "deepseek",
    model: "deepseek-v4-flash[1m]",
    source: "anthropic",
    reason: "provider_reported",
    inputTokens: 411,
    outputTokens: 137,
    cacheReadTokens: null,
    cacheCreationTokens: null,
    reasoningTokens: null,
    estimatedCostMicros: 96,
    pricingSource: "forge_static_pricing_2026_06_20",
    costUsd: 0.000096,
    hasUnknownInputTokens: false,
    hasUnknownOutputTokens: false,
    hasUnknownCost: false,
    lastEventType: "provider_usage",
    lastProviderUsageBlockId: "usage-1",
    legacyDuplicateIgnored: false,
    updatedAt: 123,
    ...overrides,
  };
}
