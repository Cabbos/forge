import assert from "node:assert/strict";
import test from "node:test";
import type { BlockState } from "../../lib/protocol.ts";
import { deriveConversationProcessTarget } from "./conversationProcessTarget.ts";
import type { ProcessDigestItem } from "./conversationTurnView.ts";

test("creates one concrete file target from read and diff evidence", () => {
  const readTarget = deriveConversationProcessTarget(item([
    block("tool_call", { tool_name: "read_file", tool_input: { path: "/repo/src/AppShell.tsx" } }),
  ]));
  const diffTarget = deriveConversationProcessTarget(item([
    block("diff_view", { file_path: "/repo/src/AppShell.tsx" }),
  ]));

  assert.deepEqual(readTarget, {
    accessibleLabel: "在工作面板打开 AppShell.tsx",
    tab: {
      kind: "file",
      id: "file:/repo/src/AppShell.tsx",
      label: "AppShell.tsx",
      path: "/repo/src/AppShell.tsx",
    },
  });
  assert.deepEqual(diffTarget, readTarget);
});

test("does not treat a search directory or shell command as a file target", () => {
  assert.equal(deriveConversationProcessTarget(item([
    block("tool_call", { tool_name: "search_content", tool_input: { path: "/repo/src" } }),
  ])), null);
  assert.equal(deriveConversationProcessTarget(item([
    block("shell", { command: "npm run build" }),
  ])), null);
});

function item(evidence: BlockState[]): ProcessDigestItem {
  return {
    id: "process",
    kind: "operation",
    label: "已执行",
    outcome: "done",
    durationMs: null,
    evidence,
  };
}

function block(eventType: BlockState["event_type"], metadata: Record<string, unknown>): BlockState {
  return {
    event_type: eventType,
    block_id: "block",
    content: "",
    metadata,
    isComplete: true,
  };
}
