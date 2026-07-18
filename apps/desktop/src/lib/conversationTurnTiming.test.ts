import assert from "node:assert/strict";
import test from "node:test";
import type { BlockState } from "./protocol.ts";
import {
  markLatestConversationTurnTerminal,
  readConversationTurnTiming,
  startConversationTurnMetadata,
  turnOutcomeForAgentStatus,
} from "./conversationTurnTiming.ts";

test("stamps and closes only the latest open user turn", () => {
  const first = userBlock("first", 1_000, {
    turn_terminal_at_ms: 2_000,
    turn_outcome: "completed",
  });
  const second = userBlock("second", 5_000);
  const blocks = markLatestConversationTurnTerminal([first, second], "completed", 17_250);

  assert.deepEqual(readConversationTurnTiming(blocks[0]), {
    startedAtMs: 1_000,
    terminalAtMs: 2_000,
    outcome: "completed",
    durationMs: 1_000,
  });
  assert.deepEqual(readConversationTurnTiming(blocks[1]), {
    startedAtMs: 5_000,
    terminalAtMs: 17_250,
    outcome: "completed",
    durationMs: 12_250,
  });
});

test("terminal outcome is idempotent and guards clock skew", () => {
  const stopped = markLatestConversationTurnTerminal(
    [userBlock("user", 5_000)],
    "stopped",
    4_000,
  );
  const repeated = markLatestConversationTurnTerminal(stopped, "failed", 9_000);

  assert.equal(repeated, stopped);
  assert.deepEqual(readConversationTurnTiming(stopped[0]), {
    startedAtMs: 5_000,
    terminalAtMs: 5_000,
    outcome: "stopped",
    durationMs: 0,
  });
});

test("legacy turns remain honest when timing is unavailable", () => {
  const legacy: BlockState = {
    block_id: "legacy",
    event_type: "user_message",
    content: "旧对话",
    isComplete: true,
    metadata: {},
  };

  assert.deepEqual(readConversationTurnTiming(legacy), {
    startedAtMs: null,
    terminalAtMs: null,
    outcome: null,
    durationMs: null,
  });
  assert.deepEqual(startConversationTurnMetadata(123), {
    turn_started_at_ms: 123,
  });
});

test("maps only authoritative terminal agent states", () => {
  assert.equal(turnOutcomeForAgentStatus("completed"), "completed");
  assert.equal(turnOutcomeForAgentStatus("failed"), "failed");
  assert.equal(turnOutcomeForAgentStatus("cancelled"), "stopped");
  assert.equal(turnOutcomeForAgentStatus("verifying"), null);
});

function userBlock(
  blockId: string,
  startedAtMs: number,
  extra: Record<string, unknown> = {},
): BlockState {
  return {
    block_id: blockId,
    event_type: "user_message",
    content: blockId,
    isComplete: true,
    metadata: {
      ...startConversationTurnMetadata(startedAtMs),
      ...extra,
    },
  };
}
