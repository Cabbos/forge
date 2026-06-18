import { describe, it } from "node:test";
import assert from "node:assert";
import { register } from "node:module";
import type { SessionState, StreamEvent } from "../lib/protocol.ts";
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
  it("persists provider_usage as a block without updating legacy cost or context usage", () => {
    const { state, dispatch } = createHarness();

    dispatch({
      event_type: "provider_usage",
      session_id: "session-1",
      model: "claude-sonnet",
      source: "anthropic",
      reason: "provider_reported",
      input_tokens: 100,
      output_tokens: 25,
      estimated_cost_micros: 1234,
    });

    const session = state.sessions.get("session-1")!;
    assert.strictEqual(session.costUsd, 0);
    assert.strictEqual(session.contextUsage, null);
    assert.strictEqual(session.blocks.length, 1);
    assert.strictEqual(session.blocks[0].event_type, "provider_usage");
    assert.strictEqual(session.blocks[0].metadata.estimated_cost_micros, 1234);
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
    assert.strictEqual(session.blocks.length, 1);
    assert.strictEqual(session.blocks[0].event_type, "provider_usage");
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
