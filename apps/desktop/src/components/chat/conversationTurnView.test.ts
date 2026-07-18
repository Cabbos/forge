import assert from "node:assert/strict";
import test from "node:test";
import type { BlockState } from "../../lib/protocol.ts";
import type { ConversationTurn } from "./messageGrouping.ts";
import * as messageGrouping from "./messageGrouping.ts";

type TurnProjectionModule = {
  deriveConversationTurnView?: (turn: ConversationTurn) => {
    userMessage: BlockState | null;
    finalAnswer: BlockState | null;
    terminalError: BlockState | null;
    interruptions: BlockState[];
    liveProgress: { id: string; label: string } | null;
    processDigest: {
      items: Array<{ kind: string; label: string; evidence: BlockState[] }>;
      operationCount: number;
      usage: BlockState[];
      delivery: BlockState | null;
    };
  };
};

test("derives a result-first view from a mixed completed turn", () => {
  const deriveConversationTurnView = (messageGrouping as TurnProjectionModule).deriveConversationTurnView;
  assert.equal(typeof deriveConversationTurnView, "function");

  const turn = conversationTurn([
    block("user", "user_message", "整理这个页面"),
    block("thinking", "thinking", "private reasoning"),
    block("read", "tool_call", "", { tool_name: "read_file", tool_input: { path: "/repo/AppShell.tsx" } }),
    block("read", "tool_call_result", "file content", { tool_name: "read_file", duration_ms: 18 }),
    block("check", "shell", "ok", { command: "npm run build", exit_code: 0, duration_ms: 302 }),
    block("diff", "diff_view", "new source", { file_path: "/repo/AppShell.tsx" }),
    block("confirm", "confirm_ask", "允许修改？", { confirmed: true, answer: true }),
    block("usage", "provider_usage", "usage", { model: "forge-test" }),
    block("delivery", "delivery_summary", "本轮交付", { summary: { next_action: "检查这版" } }),
    block("answer", "text", "页面已经整理完成。"),
  ]);

  const view = deriveConversationTurnView!(turn);

  assert.equal(view.userMessage?.block_id, "user");
  assert.equal(view.finalAnswer?.block_id, "answer");
  assert.equal(view.terminalError, null);
  assert.deepEqual(view.interruptions, []);
  assert.equal(view.liveProgress, null);
  assert.deepEqual(view.processDigest.items.map((item) => item.kind), [
    "understanding",
    "operation",
    "verification",
    "operation",
    "exception",
  ]);
  assert.deepEqual(view.processDigest.items.map((item) => item.label), [
    "已理解任务",
    "已查看 AppShell.tsx",
    "已验证构建",
    "已更新 AppShell.tsx",
    "已处理确认",
  ]);
  assert.equal(view.processDigest.operationCount, 4);
  assert.equal(view.processDigest.usage.length, 1);
  assert.equal(view.processDigest.delivery?.block_id, "delivery");
});

test("keeps internal context text out of the final answer slot", () => {
  const deriveConversationTurnView = (messageGrouping as TurnProjectionModule).deriveConversationTurnView!;
  const turn = conversationTurn([
    block("user", "user_message", "继续"),
    block("answer", "text", "这是用户应看到的结果。"),
    block("internal", "text", "Active Skills:\n- private-runtime-context"),
  ]);

  const view = deriveConversationTurnView(turn);

  assert.equal(view.finalAnswer?.block_id, "answer");
});

test("promotes only unresolved confirmations into the interruption slot", () => {
  const deriveConversationTurnView = (messageGrouping as TurnProjectionModule).deriveConversationTurnView!;
  const turn = conversationTurn([
    block("user", "user_message", "修改设置"),
    incompleteBlock("pending-confirm", "confirm_ask", "允许修改设置？"),
    block("resolved-confirm", "confirm_ask", "允许读取？", { confirmed: true, answer: true }),
    block("interrupted-confirm", "confirm_ask", "恢复前的确认", {
      confirmed: true,
      answer: null,
      confirm_interrupted: true,
    }),
  ]);

  const view = deriveConversationTurnView(turn);

  assert.deepEqual(view.interruptions.map((item) => item.block_id), ["pending-confirm"]);
});

test("promotes a terminal error only when the turn has no final answer", () => {
  const deriveConversationTurnView = (messageGrouping as TurnProjectionModule).deriveConversationTurnView!;
  const failed = deriveConversationTurnView(conversationTurn([
    block("user", "user_message", "运行检查"),
    block("error", "error", "构建没有完成", { code: "build_failed" }),
  ]));
  const recovered = deriveConversationTurnView(conversationTurn([
    block("user", "user_message", "运行检查"),
    block("error", "error", "第一次检查失败", { code: "build_failed" }),
    block("answer", "text", "已修复并重新检查通过。"),
  ]));

  assert.equal(failed.terminalError?.block_id, "error");
  assert.equal(recovered.terminalError, null);
  assert.equal(
    recovered.processDigest.items[recovered.processDigest.items.length - 1]?.kind,
    "exception",
  );
});

test("exposes one live candidate until answer text begins", () => {
  const deriveConversationTurnView = (messageGrouping as TurnProjectionModule).deriveConversationTurnView!;
  const running = deriveConversationTurnView(conversationTurn([
    block("user", "user_message", "整理页面"),
    incompleteBlock("thinking", "thinking", ""),
  ]));
  const answering = deriveConversationTurnView(conversationTurn([
    block("user", "user_message", "整理页面"),
    block("thinking", "thinking", "private"),
    incompleteBlock("answer", "text", "正在输出结果"),
  ]));

  assert.deepEqual(running.liveProgress, { id: "understanding", label: "正在理解任务" });
  assert.equal(answering.finalAnswer?.block_id, "answer");
  assert.equal(answering.liveProgress, null);
});

test("keeps a preparing-result label between text start and visible answer content", () => {
  const deriveConversationTurnView = (messageGrouping as TurnProjectionModule).deriveConversationTurnView!;
  const preparing = deriveConversationTurnView(conversationTurn([
    block("user", "user_message", "整理页面"),
    block("thinking", "thinking", "private"),
    incompleteBlock("answer", "text", ""),
  ]));

  assert.deepEqual(preparing.liveProgress, { id: "answer:preparing", label: "正在整理回答" });
  assert.equal(preparing.finalAnswer, null);
});

test("keeps grouped evidence ordered and restored completed turns collapsed", () => {
  const deriveConversationTurnView = (messageGrouping as TurnProjectionModule).deriveConversationTurnView!;
  const restored = deriveConversationTurnView(conversationTurn([
    block("user", "user_message", "读取配置"),
    block("read", "tool_call", "", { tool_name: "read_file" }),
    block("read", "tool_call_result", "配置内容", { tool_name: "read_file" }),
    block("answer", "text", "配置读取完成。"),
  ]));

  assert.equal(restored.liveProgress, null);
  assert.equal(restored.processDigest.operationCount, 1);
  assert.deepEqual(
    restored.processDigest.items[0]?.evidence.map((item) => item.event_type),
    ["tool_call", "tool_call_result"],
  );
});

function conversationTurn(blocks: BlockState[]): ConversationTurn {
  return {
    key: "turn-user",
    startsWithUser: true,
    hasEvidence: true,
    items: blocks.map((value) => ({ kind: "block" as const, block: value, key: value.block_id })),
  };
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
