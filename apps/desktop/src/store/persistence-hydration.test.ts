import assert from "node:assert/strict";
import { beforeEach, describe, it } from "node:test";
import { register } from "node:module";
import type { SessionState, SessionUsageLedgerState } from "../lib/protocol.ts";
import type { AppStore } from "./types";

declare global {
  interface Window {
    __TAURI__?: unknown;
  }
  // eslint-disable-next-line no-var
  var __forgeTestIdb: Map<string, unknown> | undefined;
  // eslint-disable-next-line no-var
  var __forgeTauriInvoke: ((command: string, args?: Record<string, unknown>) => Promise<unknown>) | undefined;
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
export async function invoke(command, args) {
  if (globalThis.__forgeTauriInvoke) {
    return globalThis.__forgeTauriInvoke(command, args);
  }
  throw new Error("Unexpected Tauri invoke in store persistence test: " + command);
}
`;

const tauriDialogStub = `
export async function open() {
  return null;
}
`;

const queryClientStub = `
export const queryClient = {
  async fetchQuery(options) {
    return options.queryFn();
  },
  clear() {},
  invalidateQueries() {
    return Promise.resolve();
  },
};
`;

register(
  `data:text/javascript,${encodeURIComponent(`
    const stubs = new Map([
      ["idb-keyval", ${JSON.stringify(`data:text/javascript,${encodeURIComponent(idbKeyvalStub)}`)}],
      ["@tauri-apps/api/core", ${JSON.stringify(`data:text/javascript,${encodeURIComponent(tauriCoreStub)}`)}],
      ["@tauri-apps/plugin-dialog", ${JSON.stringify(`data:text/javascript,${encodeURIComponent(tauriDialogStub)}`)}],
      ["../lib/query-client", ${JSON.stringify(`data:text/javascript,${encodeURIComponent(queryClientStub)}`)}],
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
    globalThis.__forgeTauriInvoke = undefined;
    delete globalThis.window.__TAURI__;
  });

  it("persists usageLedger and cumulative cost in the session payload", async () => {
    const usageLedger = testUsageLedger();
    const sessions = new Map<string, SessionState>([
      ["session-1", testSession({ usageLedger })],
    ]);

    await persistSessions(sessions, new Map(), new Map());

    const persisted = testIdb().get(PERSIST_KEY) as Array<{
      costUsd?: number | null;
      usageLedger?: SessionUsageLedgerState | null;
    }>;
    assert.equal(persisted.length, 1);
    assert.deepEqual(persisted[0].usageLedger, usageLedger);
    assert.equal(persisted[0].costUsd, 0.000096);
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

  it("hydrates cumulative cost from the persisted session when usage blocks are unavailable", async () => {
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
        costUsd: 0.000146,
        status: "running",
        workflowState: null,
        deliverySummary: null,
      },
    ]);
    const state = createStoreState();
    const hydrate = createHydrateAction(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    await hydrate();

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.costUsd, 0.000146);
    assert.deepEqual(session.usageLedger, usageLedger);
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

  it("hydrates compacted context usage from restored blocks when session context metadata is missing", async () => {
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
    ]);
    const state = createStoreState();
    const hydrate = createHydrateAction(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    await hydrate();

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.usageLedger?.inputTokens, 142_000);
    assert.strictEqual(session.costUsd, 0.002);
    assert.strictEqual(session.contextUsage?.usedTokens, 32_000);
    assert.strictEqual(session.contextUsage?.source, "local_estimate");
    assert.strictEqual(session.contextUsage?.compactedFromTokens, 142_000);
    assert.strictEqual(session.contextUsage?.compactedToTokens, 32_000);
  });

  it("hydrates compacted context usage when provider usage blocks have been pruned", async () => {
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
    ]);
    const state = createStoreState();
    const hydrate = createHydrateAction(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    await hydrate();

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.usageLedger, null);
    assert.strictEqual(session.costUsd, 0);
    assert.strictEqual(session.contextUsage?.usedTokens, 32_000);
    assert.strictEqual(session.contextUsage?.source, "local_estimate");
    assert.strictEqual(session.contextUsage?.compactedFromTokens, 142_000);
    assert.strictEqual(session.contextUsage?.compactedToTokens, 32_000);
  });

  it("hydrates compacted context usage over stale persisted context metadata", async () => {
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
        contextUsage: {
          usedTokens: 142_000,
          contextWindowTokens: 1_000_000,
          percentUsed: 14,
          source: "provider_usage",
          lastUpdatedAt: 111,
          lastCompactedAt: null,
          compactedFromTokens: null,
          compactedToTokens: null,
        },
        usageLedger: null,
        status: "running",
        workflowState: null,
        deliverySummary: null,
      },
    ]);
    testIdb().set(BLOCKS_PREFIX + "session-1", [
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
        block_id: "compact-after-stale-metadata",
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
    ]);
    const state = createStoreState();
    const hydrate = createHydrateAction(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    await hydrate();

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.contextUsage?.usedTokens, 32_000);
    assert.strictEqual(session.contextUsage?.source, "local_estimate");
    assert.strictEqual(session.contextUsage?.compactedFromTokens, 142_000);
    assert.strictEqual(session.contextUsage?.compactedToTokens, 32_000);
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

  it("merges updateBlock metadata with the current block metadata", () => {
    const state = createStoreState();
    state.sessions.set("session-1", testSession({
      blocks: [
        {
          block_id: "confirm-1",
          event_type: "confirm_ask",
          content: "Allow write?",
          isComplete: true,
          metadata: {
            kind: "file_write",
            confirmed: true,
            answer: false,
            responded_at_ms: 123,
            confirm_response_reason: "user_response",
          },
        },
      ],
    }));
    const actions = createSessionActions(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    actions.updateBlock("session-1", "confirm-1", {
      metadata: {
        confirmed: true,
        answer: true,
      },
    });

    const block = state.sessions.get("session-1")!.blocks[0];
    assert.deepEqual(block.metadata, {
      kind: "file_write",
      confirmed: true,
      answer: true,
      responded_at_ms: 123,
      confirm_response_reason: "user_response",
    });
  });

  it("hydrates resolved confirmations from Tauri transcript events", async () => {
    globalThis.window.__TAURI__ = {};
    globalThis.__forgeTauriInvoke = async (command, args) => {
      switch (command) {
        case "load_app_metadata":
          return {
            workspaces: [
              {
                id: "/workspace",
                name: "workspace",
                path: "/workspace",
                lastOpenedAt: 20,
              },
            ],
            activeWorkspaceId: "/workspace",
            activeSessionId: "session-1",
            selectedProvider: "deepseek",
            selectedModel: "deepseek-v4-flash[1m]",
          };
        case "list_sessions":
          return [
            {
              id: "session-1",
              provider: "deepseek",
              model: "deepseek-v4-flash[1m]",
              status: "running",
              created_at: "2026-06-27T00:00:00.000Z",
              working_dir: "/workspace",
              created_at_ms: 10,
              updated_at_ms: 20,
              context_window_tokens: 1_000_000,
            },
          ];
        case "load_session_transcript":
          assert.deepEqual(args, { sessionId: "session-1" });
          return [
            {
              event_type: "confirm_ask",
              session_id: "session-1",
              block_id: "confirm-1",
              question: "Allow write?",
              kind: "file_write",
              boundary: null,
            },
            {
              event_type: "confirm_response",
              session_id: "session-1",
              block_id: "confirm-1",
              approved: false,
              responded_at_ms: 123,
              reason: "user_response",
            },
          ];
        case "save_app_metadata":
          return null;
        default:
          throw new Error(`Unexpected Tauri invoke: ${command}`);
      }
    };
    const state = createStoreState();
    const hydrate = createHydrateAction(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    await hydrate();

    const session = state.sessions.get("session-1");
    assert.ok(session);
    const block = session.blocks[0];
    assert.strictEqual(block.event_type, "confirm_ask");
    assert.strictEqual(block.isComplete, true);
    assert.strictEqual(block.metadata.confirmed, true);
    assert.strictEqual(block.metadata.answer, false);
    assert.strictEqual(block.metadata.responded_at_ms, 123);
    assert.strictEqual(block.metadata.confirm_response_reason, "user_response");
    assert.strictEqual(block.metadata.confirm_interrupted, undefined);
    assert.strictEqual(state.activeSessionId, "session-1");
  });

  it("hydrates legacy usage from Tauri transcript events without rendering a usage block", async () => {
    globalThis.window.__TAURI__ = {};
    globalThis.__forgeTauriInvoke = async (command, args) => {
      switch (command) {
        case "load_app_metadata":
          return {
            workspaces: [
              {
                id: "/workspace",
                name: "workspace",
                path: "/workspace",
                lastOpenedAt: 20,
              },
            ],
            activeWorkspaceId: "/workspace",
            activeSessionId: "session-1",
            selectedProvider: "deepseek",
            selectedModel: "deepseek-v4-flash[1m]",
          };
        case "list_sessions":
          return [
            {
              id: "session-1",
              provider: "deepseek",
              model: "deepseek-v4-flash[1m]",
              status: "running",
              created_at: "2026-06-27T00:00:00.000Z",
              working_dir: "/workspace",
              created_at_ms: 10,
              updated_at_ms: 20,
              context_window_tokens: 1_000_000,
            },
          ];
        case "load_session_transcript":
          assert.deepEqual(args, { sessionId: "session-1" });
          return [
            {
              event_type: "usage",
              session_id: "session-1",
              input_tokens: 142_000,
              output_tokens: 800,
              estimated_cost_usd: 0.002,
            },
          ];
        case "save_app_metadata":
          return null;
        default:
          throw new Error(`Unexpected Tauri invoke: ${command}`);
      }
    };
    const state = createStoreState();
    const hydrate = createHydrateAction(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    await hydrate();

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.blocks.length, 0);
    assert.strictEqual(session.costUsd, 0.002);
    assert.strictEqual(session.usageLedger?.lastEventType, "usage");
    assert.strictEqual(session.usageLedger?.inputTokens, 142_000);
    assert.strictEqual(session.contextUsage?.usedTokens, 142_000);
    assert.strictEqual(session.contextUsage?.contextWindowTokens, 1_000_000);
    assert.strictEqual(session.contextUsage?.source, "provider_usage");
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
