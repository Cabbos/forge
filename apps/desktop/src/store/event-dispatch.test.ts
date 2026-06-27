import { describe, it } from "node:test";
import assert from "node:assert";
import { register } from "node:module";
import type { SessionState, SessionUsageLedgerState, StreamEvent } from "../lib/protocol.ts";
import type { AppStore } from "./types";

register(
  `data:text/javascript,${encodeURIComponent(`
    export async function resolve(specifier, context, nextResolve) {
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

Object.assign(globalThis, { window: { __TAURI__: {} } });

const { createOutputEventDispatcher } = await import("./event-dispatch.ts");

describe("createOutputEventDispatcher runtime projection events", () => {
  it("handles runtime events before session lookup", () => {
    const state = createDispatcherState();
    const dispatch = createOutputEventDispatcher(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    dispatch({
      event_type: "subagent_runtime_event",
      session_id: "missing-session",
      loop_task_id: "loop-1",
      task_id: "task-1",
      event: { type: "status", status: "running", message: "Working" },
    });

    dispatch({
      event_type: "loop_runtime_updated",
      session_id: "gateway",
      loop_task_id: "loop-1",
      task: testLoopTaskRecord(),
    });

    assert.strictEqual(state.sessions.size, 0);
    assert.deepStrictEqual(
      state.subagentRuntimeByTask.get("missing-session:task-1"),
      {
        session_id: "missing-session",
        loop_task_id: "loop-1",
        task_id: "task-1",
        latest_event: { type: "status", status: "running", message: "Working" },
        status: "running",
        message: "Working",
      },
    );
    assert.deepStrictEqual(
      state.loopRuntimeByTask.get("gateway:loop-1"),
      {
        session_id: "gateway",
        loop_task_id: "loop-1",
        task: testLoopTaskRecord(),
      },
    );
  });
});

describe("createOutputEventDispatcher live file_io events", () => {
  it("attaches live file_io metadata to an existing tool block", () => {
    const { state, dispatch } = createHarness();

    dispatch({
      event_type: "tool_call_start",
      session_id: "session-1",
      block_id: "tool-1",
      tool_name: "read_file",
      tool_input: { path: "src/main.rs" },
    });
    dispatch({
      event_type: "file_io",
      session_id: "session-1",
      block_id: "tool-1",
      path: "/workspace/src/main.rs",
      operation: "read",
      source: "executor",
    });

    const block = state.sessions.get("session-1")!.blocks[0];
    assert.strictEqual(block.event_type, "tool_call");
    assert.deepStrictEqual(block.metadata.file_io_events, [
      {
        path: "/workspace/src/main.rs",
        operation: "read",
        source: "executor",
      },
    ]);
  });

  it("ignores orphan live file_io events without creating blocks", () => {
    const { state, dispatch } = createHarness();

    dispatch({
      event_type: "file_io",
      session_id: "session-1",
      block_id: "missing-tool",
      path: "/workspace/src/main.rs",
      operation: "read",
      source: "executor",
    });

    assert.deepStrictEqual(state.sessions.get("session-1")!.blocks, []);
  });

  it("attaches live file_io metadata to an existing shell block", () => {
    const { state, dispatch } = createHarness();

    dispatch({
      event_type: "shell_start",
      session_id: "session-1",
      block_id: "shell-1",
      command: "ls src",
    });
    dispatch({
      event_type: "file_io",
      session_id: "session-1",
      block_id: "shell-1",
      path: "/workspace/src",
      operation: "list",
      source: "executor",
    });

    const block = state.sessions.get("session-1")!.blocks[0];
    assert.strictEqual(block.event_type, "shell");
    assert.deepStrictEqual(block.metadata.file_io_events, [
      {
        path: "/workspace/src",
        operation: "list",
        source: "executor",
      },
    ]);
  });

  it("preserves file_io metadata when the tool result arrives", () => {
    const { state, dispatch } = createHarness();

    dispatch({
      event_type: "tool_call_start",
      session_id: "session-1",
      block_id: "tool-1",
      tool_name: "read_file",
      tool_input: { path: "src/main.rs" },
    });
    dispatch({
      event_type: "file_io",
      session_id: "session-1",
      block_id: "tool-1",
      path: "/workspace/src/main.rs",
      operation: "read",
      source: "executor",
    });
    dispatch({
      event_type: "tool_call_result",
      session_id: "session-1",
      block_id: "tool-1",
      result: "hello",
      is_error: false,
      duration_ms: 12,
    });

    const block = state.sessions.get("session-1")!.blocks[0];
    assert.strictEqual(block.content, "hello");
    assert.strictEqual(block.isComplete, true);
    assert.deepStrictEqual(block.metadata.file_io_events, [
      {
        path: "/workspace/src/main.rs",
        operation: "read",
        source: "executor",
      },
    ]);
  });
});

describe("createOutputEventDispatcher live provider_usage events", () => {
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
    assert.strictEqual(session.usageLedger?.outputTokens, 137);
    assert.strictEqual(session.usageLedger?.estimatedCostMicros, 96);
    assert.strictEqual(session.costUsd, 0.000096);
    assert.strictEqual(session.contextUsage?.usedTokens, 411);
    assert.strictEqual(session.contextUsage?.contextWindowTokens, 1000);
    assert.strictEqual(session.blocks.length, 1);
    assert.strictEqual(session.blocks[0].event_type, "provider_usage");
    assert.strictEqual(session.blocks[session.blocks.length - 1]?.event_type, "provider_usage");
    assert.strictEqual(session.blocks[0].metadata.estimated_cost_micros, 96);
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

  it("treats near-matching legacy usage after provider_usage as a duplicate companion", () => {
    const { state, dispatch } = createHarness();

    dispatch({
      event_type: "provider_usage",
      session_id: "session-1",
      block_id: "usage-provider-tolerance",
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
      estimated_cost_usd: 0.0000964,
    });

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.costUsd, 0.000096);
    assert.strictEqual(session.usageLedger?.legacyDuplicateIgnored, true);
  });

  it("keeps legacy usage as fallback when no provider_usage exists", () => {
    const { state, dispatch } = createHarness();

    dispatch({
      event_type: "usage",
      session_id: "session-1",
      input_tokens: 142_000,
      output_tokens: 800,
      estimated_cost_usd: 0.002,
    });

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.costUsd, 0.002);
    assert.strictEqual(session.contextUsage?.usedTokens, 142_000);
    assert.strictEqual(session.usageLedger?.lastEventType, "usage");
    assert.strictEqual(session.usageLedger?.estimatedCostMicros, 2000);
  });

  it("does not double-count provider_usage after legacy usage updates cost and context", () => {
    const { state, dispatch } = createHarness();

    dispatch({
      event_type: "usage",
      session_id: "session-1",
      input_tokens: 100,
      output_tokens: 25,
      estimated_cost_usd: 0.001,
    });
    dispatch({
      event_type: "provider_usage",
      session_id: "session-1",
      block_id: "usage-provider-after-legacy",
      provider_id: "anthropic",
      model: "claude-sonnet",
      source: "anthropic",
      reason: "provider_reported",
      input_tokens: 100,
      output_tokens: 25,
      estimated_cost_micros: 1000,
    });

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.costUsd, 0.001);
    assert.strictEqual(session.contextUsage?.usedTokens, 100);
    assert.strictEqual(session.usageLedger?.lastEventType, "provider_usage");
    assert.strictEqual(session.usageLedger?.estimatedCostMicros, 1000);
    assert.strictEqual(session.blocks.length, 1);
    assert.strictEqual(session.blocks[0].event_type, "provider_usage");
  });

  it("keeps usage ledger when context compaction applies a local estimate", () => {
    const { state, dispatch } = createHarness();

    dispatch({
      event_type: "provider_usage",
      session_id: "session-1",
      block_id: "usage-before-compact",
      provider_id: "deepseek",
      model: "deepseek-v4-flash[1m]",
      source: "anthropic",
      reason: "provider_reported",
      input_tokens: 411,
      output_tokens: 137,
      estimated_cost_micros: 96,
    });
    dispatch({
      event_type: "context_compacted",
      session_id: "session-1",
      block_id: "compact-1",
      summary: "Compacted context",
      retained_messages: 2,
      compacted_messages: 3,
      estimated_tokens_before: 411,
      estimated_tokens_after: 128,
    });

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.contextUsage?.source, "local_estimate");
    assert.strictEqual(session.contextUsage?.usedTokens, 128);
    assert.strictEqual(session.usageLedger?.lastEventType, "provider_usage");
    assert.strictEqual(session.usageLedger?.inputTokens, 411);
  });

  it("keeps compacted local estimate after provider usage replay and legacy companion", () => {
    const { state, dispatch } = createHarness();
    const providerUsageEvent = {
      event_type: "provider_usage" as const,
      session_id: "session-1",
      block_id: "usage-before-compact-legacy",
      provider_id: "deepseek",
      model: "deepseek-v4-flash[1m]",
      source: "anthropic",
      reason: "provider_reported" as const,
      input_tokens: 411,
      output_tokens: 137,
      estimated_cost_micros: 96,
    };

    dispatch(providerUsageEvent);
    dispatch({
      event_type: "context_compacted",
      session_id: "session-1",
      block_id: "compact-before-legacy",
      summary: "Compacted context",
      retained_messages: 2,
      compacted_messages: 3,
      estimated_tokens_before: 411,
      estimated_tokens_after: 128,
    });
    dispatch(providerUsageEvent);
    dispatch({
      event_type: "usage",
      session_id: "session-1",
      input_tokens: 411,
      output_tokens: 137,
      estimated_cost_usd: 0.000096,
    });

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.costUsd, 0.000096);
    assert.strictEqual(session.contextUsage?.source, "local_estimate");
    assert.strictEqual(session.contextUsage?.usedTokens, 128);
    assert.strictEqual(session.usageLedger?.lastEventType, "provider_usage");
    assert.strictEqual(session.usageLedger?.inputTokens, 411);
    assert.strictEqual(session.usageLedger?.lastProviderUsageBlockId, "usage-before-compact-legacy");
    assert.strictEqual(session.usageLedger?.legacyDuplicateIgnored, true);
    assert.strictEqual(
      session.blocks.filter((block) => block.event_type === "provider_usage").length,
      1,
    );
  });

  it("ignores replayed provider_usage with the same block id", () => {
    const { state, dispatch } = createHarness();
    const event = {
      event_type: "provider_usage" as const,
      session_id: "session-1",
      block_id: "usage-replayed-provider",
      provider_id: "deepseek",
      model: "deepseek-v4-flash[1m]",
      source: "anthropic",
      reason: "provider_reported" as const,
      input_tokens: 411,
      output_tokens: 137,
      estimated_cost_micros: 96,
    };

    dispatch(event);
    dispatch(event);

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.costUsd, 0.000096);
    assert.strictEqual(session.blocks.length, 1);
    assert.strictEqual(session.blocks[0].block_id, "usage-replayed-provider");
  });

  it("restores a replayed provider_usage block when ledger has the block id but blocks are missing", () => {
    const usageLedger = testUsageLedger({ lastProviderUsageBlockId: "usage-missing-visible-block" });
    const state = createDispatcherState([
      [
        "session-1",
        {
          id: "session-1",
          agentType: "codex",
          model: "test-model",
          workingDir: "/workspace",
          workspaceId: "/workspace",
          createdAt: 1,
          updatedAt: 1,
          contextWindowTokens: 1000,
          status: "running",
          streaming: false,
          blocks: [],
          costUsd: 0.000096,
          contextUsage: null,
          usageLedger,
        },
      ],
    ]);
    const dispatch = createOutputEventDispatcher(
      (partial) => Object.assign(state, partial),
      () => state,
    );

    dispatch({
      event_type: "provider_usage",
      session_id: "session-1",
      block_id: "usage-missing-visible-block",
      provider_id: "deepseek",
      model: "deepseek-v4-flash[1m]",
      source: "anthropic",
      reason: "provider_reported",
      input_tokens: 411,
      output_tokens: 137,
      estimated_cost_micros: 96,
    });

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.costUsd, 0.000096);
    assert.strictEqual(session.blocks.length, 1);
    assert.strictEqual(session.blocks[0].block_id, "usage-missing-visible-block");
    assert.strictEqual(session.blocks[0].event_type, "provider_usage");
  });
});

describe("createOutputEventDispatcher health alerts", () => {
  it("clears stale same-session health alerts when fresh output arrives", () => {
    const { state, dispatch } = createHarness();
    state.healthAlerts = [
      {
        alert_id: "session-stale-session-1",
        session_id: "session-1",
        level: "warn",
        title: "会话无响应",
        message: "No recent events.",
      },
      {
        alert_id: "session-stale-session-2",
        session_id: "session-2",
        level: "warn",
        title: "会话无响应",
        message: "No recent events.",
      },
      {
        alert_id: "missing-api-key:session-1",
        session_id: "session-1",
        level: "critical",
        title: "缺少模型密钥",
        message: "Missing key.",
      },
    ];

    dispatch({
      event_type: "text_start",
      session_id: "session-1",
      block_id: "fresh-text",
    });

    assert.deepStrictEqual(
      state.healthAlerts.map((alert) => alert.alert_id),
      ["session-stale-session-2", "missing-api-key:session-1"],
    );
  });
});

describe("createOutputEventDispatcher replayed confirmations", () => {
  it("restored interrupted pending confirmation replaces the existing confirm block after hydration", () => {
    const { state, dispatch } = createHarness([
      {
        block_id: "confirm-1",
        event_type: "confirm_ask",
        content: "Allow write?",
        isComplete: false,
        metadata: {
          kind: "write_file",
          boundary: testWriteBoundary(),
        },
      },
    ]);

    dispatch({
      event_type: "confirm_ask",
      session_id: "session-1",
      block_id: "confirm-1",
      question: "Allow write?",
      kind: "write_file",
      boundary: testWriteBoundary(),
      replayed_interrupted: true,
    });

    const confirmBlocks = state.sessions
      .get("session-1")!
      .blocks.filter((block) => block.event_type === "confirm_ask");
    assert.strictEqual(confirmBlocks.length, 1);
    assert.strictEqual(confirmBlocks[0].metadata.confirmed, true);
    assert.strictEqual(confirmBlocks[0].metadata.answer, null);
    assert.strictEqual(confirmBlocks[0].metadata.confirm_interrupted, true);
    assert.strictEqual(confirmBlocks[0].metadata.confirm_interrupted_reason, "session_restored");
    assert.strictEqual(confirmBlocks[0].isComplete, true);
  });

  it("confirm_response resolves the existing live confirmation block", () => {
    const { state, dispatch } = createHarness();

    dispatch({
      event_type: "confirm_ask",
      session_id: "session-1",
      block_id: "confirm-live",
      question: "Allow write?",
      kind: "write_file",
      boundary: testWriteBoundary(),
    });
    dispatch({
      event_type: "confirm_response",
      session_id: "session-1",
      block_id: "confirm-live",
      approved: false,
      responded_at_ms: 1,
      reason: "user_response",
      replayed: false,
    });

    const confirmBlocks = state.sessions
      .get("session-1")!
      .blocks.filter((block) => block.event_type === "confirm_ask");
    assert.strictEqual(confirmBlocks.length, 1);
    assert.strictEqual(confirmBlocks[0].isComplete, true);
    assert.strictEqual(confirmBlocks[0].metadata.confirmed, true);
    assert.strictEqual(confirmBlocks[0].metadata.answer, false);
    assert.strictEqual(confirmBlocks[0].metadata.confirm_interrupted, undefined);
  });
});

function createHarness(blocks: SessionState["blocks"] = []) {
  const state = createDispatcherState([
    [
      "session-1",
      {
        id: "session-1",
        agentType: "codex",
        model: "test-model",
        workingDir: "/workspace",
        workspaceId: "/workspace",
        createdAt: 1,
        updatedAt: 1,
        contextWindowTokens: 1000,
        status: "running",
        streaming: false,
        blocks,
        costUsd: 0,
        contextUsage: null,
        usageLedger: null,
      },
    ],
  ]);
  const dispatch = createOutputEventDispatcher(
    (partial) => Object.assign(state, partial),
    () => state,
  );
  return { state, dispatch };
}

function createDispatcherState(
  sessions: Iterable<readonly [string, SessionState]> = [],
): AppStore {
  return {
    sessions: new Map(sessions),
    activeWorkspaceId: "/workspace",
    workflowBySession: new Map(),
    deliverySummaryBySession: new Map(),
    selectedContextBySession: new Map(),
    agentTurnBySession: new Map(),
    agentA2ABySession: new Map(),
    subagentRuntimeByTask: new Map(),
    loopRuntimeByTask: new Map(),
    healthAlerts: [],
  } as unknown as AppStore;
}

function testLoopTaskRecord(): Extract<StreamEvent, { event_type: "loop_runtime_updated" }>["task"] {
  return {
    id: "loop-1",
    goal: "Ship runtime protocol",
    status: "running",
    owner: { kind: "gateway" },
    policy: {},
    budget: {},
    completion_contract: {},
    created_at_ms: 1,
    updated_at_ms: 10,
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

function testWriteBoundary() {
  return {
    title: "Write src/main.ts",
    target_label: "src/main.ts",
    workspace_name: "workspace",
    workspace_path: "/workspace",
    operation: "write_file",
    affected_files: ["/workspace/src/main.ts"],
    command: null,
    impact: "Updates a source file.",
    risk: "normal" as const,
    recovery: "Revert the file if needed.",
    checkpoint_status: "ready" as const,
    warning: null,
  };
}
