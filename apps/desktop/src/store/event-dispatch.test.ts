import { describe, it } from "node:test";
import assert from "node:assert";
import type { StreamEvent } from "../lib/protocol";
import { createOutputEventDispatcher } from "./event-dispatch";
import type { AppStore } from "./types";

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

function createDispatcherState(): AppStore {
  return {
    sessions: new Map(),
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
