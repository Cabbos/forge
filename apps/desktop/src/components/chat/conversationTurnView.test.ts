import assert from "node:assert/strict";
import test from "node:test";
import type { BlockState, StreamEvent } from "../../lib/protocol.ts";
import { applyTranscriptEventToBlocks, eventToBlock } from "../../store/blocks.ts";
import type {
  ConversationTurn,
  LiveProgressCandidate,
  ProcessDigest,
  ProcessDigestItem,
  ProcessDigestKind,
  TurnTerminalSummary,
} from "./messageGrouping.ts";
import * as messageGrouping from "./messageGrouping.ts";

type TurnProjection = {
  userMessage: BlockState | null;
  finalAnswer: BlockState | null;
  terminalError: BlockState | null;
  interruptions: BlockState[];
  liveProgress: LiveProgressCandidate | null;
  terminalSummary: TurnTerminalSummary | null;
  processDigest: ProcessDigest;
};

type TurnProjectionModule = {
  deriveConversationTurnView?: (turn: ConversationTurn) => TurnProjection;
};

test("keeps streamed answer text visible with one answering stage", () => {
  const view = derive(conversationTurn([
    timedUser(1_000),
    block("thinking", "thinking", "private reasoning"),
    incompleteBlock("answer", "text", "正在输出结果"),
  ]));

  assert.equal(view.finalAnswer?.block_id, "answer");
  assert.deepEqual(view.liveProgress, {
    id: "answering",
    label: "正在生成答复",
    motion: "live",
  });
  assert.equal(view.terminalSummary, null);
});

test("derives an honest completed footer and safe compact process digest", () => {
  const view = derive(conversationTurn([
    timedUser(1_000, 13_250, "completed"),
    block("thinking", "thinking", "private chain of thought"),
    block("read", "tool_call", "", {
      tool_name: "read_file",
      tool_input: { path: "/repo/private/App.tsx", token: "never-show-this" },
    }),
    block("search", "tool_call", "secret payload", {
      tool_name: "search_content",
      tool_input: { path: "/repo/private", query: "password" },
    }),
    block("edit", "tool_call", "", {
      tool_name: "edit_file",
      tool_input: { path: "/repo/private/App.tsx", replacement: "secret payload" },
    }),
    block("diff", "diff_view", "secret diff", { file_path: "/repo/private/App.tsx" }),
    block("check", "shell", "ok", { command: "npm test", exit_code: 0 }),
    block("answer", "text", "页面已经整理完成。"),
  ]));

  assert.deepEqual(view.terminalSummary, {
    outcome: "completed",
    durationMs: 12_250,
    operationCount: 3,
  });
  assert.deepEqual(view.processDigest.items.map((item) => item.label), [
    "分析需求",
    "完成修改",
    "验证结果",
  ]);
  assert.equal(view.liveProgress, null);

  const publicDigest = view.processDigest.items.map(({ id, kind, label, outcome }) => ({
    id,
    kind,
    label,
    outcome,
  }));
  const serialized = JSON.stringify(publicDigest);
  for (const secret of ["App.tsx", "npm test", "secret payload", "never-show-this", "password"]) {
    assert.equal(serialized.includes(secret), false);
  }
  assert.equal(
    view.processDigest.items.flatMap((item) => item.evidence).some((item) => item.block_id === "diff"),
    true,
  );
});

test("preserves stopped and failed terminal outcomes", () => {
  const stopped = derive(conversationTurn([
    timedUser(1_000, 9_000, "stopped"),
    incompleteBlock("edit", "tool_call", "", { tool_name: "edit_file" }),
  ]));
  const failed = derive(conversationTurn([
    timedUser(1_000, 13_000, "failed"),
    block("edit", "tool_call", "", { tool_name: "edit_file", is_error: true }),
    block("error", "error", "private backend error", { code: "write_failed" }),
  ]));

  assert.deepEqual(stopped.terminalSummary, {
    outcome: "stopped",
    durationMs: 8_000,
    operationCount: 1,
  });
  assert.equal(stopped.processDigest.items.some((item) => item.outcome === "done"), false);
  assert.equal(stopped.processDigest.items.some((item) => item.outcome === "stopped"), true);

  assert.deepEqual(failed.terminalSummary, {
    outcome: "failed",
    durationMs: 12_000,
    operationCount: 1,
  });
  assert.equal(failed.processDigest.items.some((item) => item.outcome === "failed"), true);
});

test("uses a provisional completed footer for a legacy final answer without inventing duration", () => {
  const view = derive(conversationTurn([
    block("user", "user_message", "继续"),
    block("answer", "text", "已经完成。"),
  ]));

  assert.deepEqual(view.terminalSummary, {
    outcome: "completed",
    durationMs: null,
    operationCount: 0,
  });
});

test("omits an incomplete pre-answer operation from a provisional completed digest", () => {
  const view = derive(conversationTurn([
    block("user", "user_message", "继续"),
    incompleteBlock("legacy-edit", "tool_call", "", { tool_name: "edit_file" }),
    block("answer", "text", "已经完成。"),
  ]));

  assert.deepEqual(view.terminalSummary, {
    outcome: "completed",
    durationMs: null,
    operationCount: 1,
  });
  assert.deepEqual(view.processDigest.items, []);
});

test("later live activity overrides provisional completion", () => {
  const view = derive(conversationTurn([
    block("user", "user_message", "继续"),
    block("answer", "text", "第一段结果。"),
    incompleteBlock("later-edit", "tool_call", "", { tool_name: "apply_patch" }),
  ]));

  assert.equal(view.finalAnswer?.block_id, "answer");
  assert.equal(view.terminalSummary, null);
  assert.deepEqual(view.liveProgress, {
    id: "modifying",
    label: "正在进行修改",
    motion: "live",
  });
});

test("never relabels an incomplete group as done for an authoritative completed turn", () => {
  const view = derive(conversationTurn([
    timedUser(100, 500, "completed"),
    block("thinking", "thinking", "private"),
    incompleteBlock("running-edit", "tool_call", "", { tool_name: "edit_file" }),
    block("answer", "text", "完成。"),
  ]));

  assert.deepEqual(view.terminalSummary, {
    outcome: "completed",
    durationMs: 400,
    operationCount: 1,
  });
  assert.equal(view.processDigest.items.some((item) => item.outcome === "running"), false);
  assert.equal(
    view.processDigest.items.some((item) => item.kind === "modification" && item.outcome === "done"),
    false,
  );
});

test("retains an atomically rendered singleton Store diff after turn completion", () => {
  const diff = storeDiffBlock();
  assert.equal(diff.isComplete, false);

  const completedTurns = [
    conversationTurn([
      timedUser(100, 500, "completed"),
      diff,
      block("answer", "text", "图片修改已经完成。"),
    ]),
    conversationTurn([
      diff,
      block("answer", "text", "图片修改已经完成。"),
    ]),
  ];

  for (const turn of completedTurns) {
    const view = derive(turn);
    assert.equal(view.terminalSummary?.outcome, "completed");
    assert.equal(view.terminalSummary?.operationCount, 1);
    assert.deepEqual(view.processDigest.items.map((item) => ({
      kind: item.kind,
      label: item.label,
      outcome: item.outcome,
      evidence: item.evidence.map((evidence) => evidence.block_id),
    })), [{
      kind: "analysis",
      label: "分析需求",
      outcome: "done",
      evidence: ["singleton-diff"],
    }]);
  }
});

test("classifies a completed Store git_diff as inspection while retaining diff evidence", () => {
  const view = derive(conversationTurn([
    timedUser(100, 500, "completed"),
    ...toolBackedStoreDiff("git-diff-call", "git_diff"),
    block("answer", "text", "检查完成。"),
  ]));

  assert.equal(view.processDigest.items.some((item) => item.label === "完成修改"), false);
  assert.deepEqual(view.processDigest.items.map((item) => ({
    kind: item.kind,
    label: item.label,
    outcome: item.outcome,
  })), [{
    kind: "analysis",
    label: "分析需求",
    outcome: "done",
  }]);
  assert.equal(view.processDigest.operationCount, 1);
  assert.equal(
    view.processDigest.items
      .flatMap((item) => item.evidence)
      .some((evidence) => evidence.event_type === "diff_view"),
    true,
  );
});

test("classifies a completed Store write diff as one modification with retained evidence", () => {
  const view = derive(conversationTurn([
    timedUser(100, 500, "completed"),
    ...toolBackedStoreDiff("write-call", "write_to_file"),
    block("answer", "text", "修改完成。"),
  ]));

  assert.deepEqual(view.processDigest.items.map((item) => ({
    kind: item.kind,
    label: item.label,
    outcome: item.outcome,
  })), [{
    kind: "modification",
    label: "完成修改",
    outcome: "done",
  }]);
  assert.equal(view.processDigest.operationCount, 1);
  assert.equal(
    view.processDigest.items
      .flatMap((item) => item.evidence)
      .some((evidence) => evidence.event_type === "diff_view"),
    true,
  );
});

test("keeps singleton Store diff outcomes honest before completion and on interruption", () => {
  const diff = storeDiffBlock();
  const running = derive(conversationTurn([
    block("user", "user_message", "修改图片"),
    diff,
  ]));
  const stopped = derive(conversationTurn([
    timedUser(100, 500, "stopped"),
    diff,
  ]));
  const failed = derive(conversationTurn([
    timedUser(100, 500, "failed"),
    diff,
  ]));

  assert.equal(running.terminalSummary, null);
  assert.equal(running.processDigest.items[0]?.outcome, "running");
  assert.equal(stopped.processDigest.items[0]?.outcome, "stopped");
  assert.equal(failed.processDigest.items[0]?.outcome, "failed");
});

test("counts grouped meaningful operations before safe visible compaction", () => {
  const view = derive(conversationTurn([
    timedUser(1_000, 2_000, "completed"),
    block("thinking", "thinking", "private"),
    block("pending", "pending", "private"),
    block("read", "tool_call", "", { tool_name: "read_file" }),
    block("search", "tool_call", "", { tool_name: "grep" }),
    block("write", "tool_call", "", { tool_name: "write_to_file" }),
    block("diff", "diff_view", "diff", {}),
    block("verify", "shell", "ok", { command: "npm run check:precommit", exit_code: 0 }),
    block("confirm", "confirm_ask", "允许？", { confirmed: true }),
    block("usage", "provider_usage", "usage", { model: "test" }),
    block("delivery", "delivery_summary", "delivery", { summary: { next_action: "继续" } }),
    block("answer", "text", "完成。"),
  ]));

  assert.equal(view.processDigest.operationCount, 3);
  assert.equal(view.terminalSummary?.operationCount, 3);
  assert.deepEqual(view.processDigest.items.map((item) => item.kind), [
    "analysis",
    "modification",
    "verification",
  ]);
});

test("limits the public digest while retaining the latest important evidence", () => {
  const view = derive(conversationTurn([
    timedUser(0, 5_000, "failed"),
    block("read", "tool_call", "", { tool_name: "read_file", marker: "first-analysis" }),
    block("mod-1", "tool_call", "", { tool_name: "edit_file", marker: "old-modification" }),
    block("verify-1", "shell", "ok", { command: "npm test", marker: "old-verification" }),
    block("analysis-2", "tool_call", "", { tool_name: "search_content", marker: "later-analysis" }),
    block("mod-2", "tool_call", "", { tool_name: "apply_patch", marker: "latest-modification" }),
    block("verify-2", "shell", "ok", { command: "npm run build", marker: "latest-verification" }),
    block("error", "error", "private failure", { code: "unexpected" }),
  ]));

  assert.equal(view.processDigest.items.length, 4);
  assert.deepEqual(view.processDigest.items.map((item) => item.kind), [
    "analysis",
    "modification",
    "verification",
    "exception",
  ]);
  assert.equal(hasMarker(view.processDigest.items, "first-analysis"), true);
  assert.equal(hasMarker(view.processDigest.items, "latest-modification"), true);
  assert.equal(hasMarker(view.processDigest.items, "latest-verification"), true);
  assert.equal(view.processDigest.items.some((item) => item.outcome === "failed"), true);
  assert.equal(view.processDigest.operationCount, 6);
});

test("keeps usage and the latest delivery separate and intact", () => {
  const view = derive(conversationTurn([
    timedUser(0, 100, "completed"),
    block("usage-1", "provider_usage", "usage one", { model: "first" }),
    block("delivery-1", "delivery_summary", "delivery one", { summary: { value: 1 } }),
    block("usage-2", "provider_usage", "usage two", { model: "second" }),
    block("delivery-2", "delivery_summary", "delivery two", { summary: { value: 2 } }),
    block("answer", "text", "完成。"),
  ]));

  assert.deepEqual(view.processDigest.usage.map((item) => item.block_id), ["usage-1", "usage-2"]);
  assert.equal(view.processDigest.delivery?.block_id, "delivery-2");
});

test("classifies all canonical backend write names as modifications", () => {
  for (const toolName of [
    "write_to_file",
    "edit_file",
    "apply_patch",
    "create_file",
    "delete_file",
    "move_file",
    "write_file",
    "write",
    "edit",
  ]) {
    const view = derive(conversationTurn([
      timedUser(0, 100, "completed"),
      block(toolName, "tool_call", "", { tool_name: toolName }),
      block("answer", "text", "完成。"),
    ]));
    assert.deepEqual(view.processDigest.items.map((item) => item.kind), ["modification"]);
    assert.equal(view.processDigest.operationCount, 1);
  }
});

test("uses the hardened verification classifier instead of payload substring matching", () => {
  const view = derive(conversationTurn([
    timedUser(0, 100, "completed"),
    block("unsafe", "shell", "", { command: "echo ready && npm test" }),
    block("safe", "shell", "", { command: "npm test" }),
    block("answer", "text", "完成。"),
  ]));

  assert.deepEqual(view.processDigest.items.map((item) => item.kind), ["analysis", "verification"]);
  assert.equal(view.processDigest.operationCount, 1);
});

test("keeps messageGrouping compatibility exports available", () => {
  assert.equal(typeof messageGrouping.deriveConversationTurnView, "function");

  const kind: ProcessDigestKind = "analysis";
  const item: ProcessDigestItem = {
    id: "analysis",
    kind,
    label: "分析需求",
    outcome: "done",
    evidence: [],
  };
  assert.equal(item.kind, "analysis");
});

test("keeps internal context text out of the final answer slot", () => {
  const view = derive(conversationTurn([
    block("user", "user_message", "继续"),
    block("answer", "text", "这是用户应看到的结果。"),
    block("internal", "text", "Active Skills:\n- private-runtime-context"),
  ]));

  assert.equal(view.finalAnswer?.block_id, "answer");
});

test("promotes only unresolved confirmations into the interruption slot", () => {
  const view = derive(conversationTurn([
    block("user", "user_message", "修改设置"),
    incompleteBlock("pending-confirm", "confirm_ask", "允许修改设置？"),
    block("resolved-confirm", "confirm_ask", "允许读取？", { confirmed: true, answer: true }),
    block("interrupted-confirm", "confirm_ask", "恢复前的确认", {
      confirmed: true,
      answer: null,
      confirm_interrupted: true,
    }),
  ]));

  assert.deepEqual(view.interruptions.map((item) => item.block_id), ["pending-confirm"]);
  assert.equal(view.liveProgress?.id, "waiting");
});

function derive(turn: ConversationTurn): TurnProjection {
  const deriveConversationTurnView = (messageGrouping as TurnProjectionModule).deriveConversationTurnView;
  assert.equal(typeof deriveConversationTurnView, "function");
  return deriveConversationTurnView!(turn);
}

function conversationTurn(blocks: BlockState[]): ConversationTurn {
  return {
    key: "turn-user",
    startsWithUser: true,
    hasEvidence: true,
    items: blocks.map((value) => ({ kind: "block" as const, block: value, key: value.block_id })),
  };
}

function timedUser(
  startedAtMs: number,
  terminalAtMs?: number,
  outcome?: "completed" | "stopped" | "failed",
): BlockState {
  return block("user", "user_message", "整理这个页面", {
    turn_started_at_ms: startedAtMs,
    ...(terminalAtMs === undefined ? {} : { turn_terminal_at_ms: terminalAtMs }),
    ...(outcome === undefined ? {} : { turn_outcome: outcome }),
  });
}

function block(
  block_id: string,
  event_type: string,
  content: string,
  metadata: Record<string, unknown> = {},
): BlockState {
  return { block_id, event_type, content, metadata, isComplete: true };
}

function incompleteBlock(
  block_id: string,
  event_type: string,
  content: string,
  metadata: Record<string, unknown> = {},
): BlockState {
  return { block_id, event_type, content, metadata, isComplete: false };
}

function hasMarker(items: ProcessDigestItem[], marker: string) {
  return items.some((item) => item.evidence.some((block) => block.metadata.marker === marker));
}

function storeDiffBlock(): BlockState {
  const event: StreamEvent = {
    event_type: "diff_view",
    session_id: "session",
    block_id: "singleton-diff",
    file_path: "assets/logo.svg",
    old_content: "<svg><circle fill=\"red\" /></svg>",
    new_content: "<svg><circle fill=\"blue\" /></svg>",
  };
  const diff = eventToBlock(event);
  assert.ok(diff);
  return diff;
}

function toolBackedStoreDiff(blockId: string, toolName: string): BlockState[] {
  const events: StreamEvent[] = [
    {
      event_type: "tool_call_start",
      session_id: "session",
      block_id: blockId,
      tool_name: toolName,
      tool_input: { path: "src/App.tsx" },
    },
    {
      event_type: "diff_view",
      session_id: "session",
      block_id: blockId,
      file_path: "src/App.tsx",
      old_content: "before",
      new_content: "after",
    },
    {
      event_type: "tool_call_result",
      session_id: "session",
      block_id: blockId,
      result: "done",
      is_error: false,
      duration_ms: 12,
    },
  ];

  return events.reduce(applyTranscriptEventToBlocks, []);
}
