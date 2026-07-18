import assert from "node:assert/strict";
import test from "node:test";
import type { BlockState } from "../../lib/protocol.ts";
import * as turnView from "./conversationTurnView.ts";

type ProgressModule = {
  deriveLiveProgressCandidate?: (blocks: BlockState[]) => { id: string; label: string } | null;
};

test("derives a specific safe label for an active file read", () => {
  const deriveLiveProgressCandidate = (turnView as ProgressModule).deriveLiveProgressCandidate;
  assert.equal(typeof deriveLiveProgressCandidate, "function");

  const candidate = deriveLiveProgressCandidate!([
    incompleteBlock("read", "tool_call", {
      tool_name: "read_file",
      tool_input: { path: "/Users/demo/project/src/AppShell.tsx" },
    }),
  ]);

  assert.deepEqual(candidate, {
    id: "read:AppShell.tsx",
    label: "正在查看 AppShell.tsx",
  });
});

test("classifies allow-listed search and edit tools without exposing their payload", () => {
  const deriveLiveProgressCandidate = (turnView as ProgressModule).deriveLiveProgressCandidate!;

  assert.deepEqual(
    deriveLiveProgressCandidate([
      incompleteBlock("search", "tool_call", {
        tool_name: "search_content",
        tool_input: { path: "/repo/src/components", query: "password=never-show-this" },
      }),
    ]),
    { id: "search:components", label: "正在查找 components" },
  );
  assert.deepEqual(
    deriveLiveProgressCandidate([
      incompleteBlock("edit", "tool_call", {
        tool_name: "edit",
        tool_input: { file_path: "/repo/src/AppShell.tsx", replacement: "secret body" },
      }),
    ]),
    { id: "edit:AppShell.tsx", label: "正在调整 AppShell.tsx" },
  );
});

test("classifies verification commands without rendering the command", () => {
  const deriveLiveProgressCandidate = (turnView as ProgressModule).deriveLiveProgressCandidate!;

  assert.deepEqual(
    deriveLiveProgressCandidate([
      incompleteBlock("build", "shell", { command: "npm run build -- --token never-show-this" }),
    ]),
    { id: "verify:build", label: "正在验证构建" },
  );
  assert.deepEqual(
    deriveLiveProgressCandidate([
      incompleteBlock("test", "shell", { command: "pnpm test apps/desktop" }),
    ]),
    { id: "verify:test", label: "正在运行测试" },
  );
  assert.deepEqual(
    deriveLiveProgressCandidate([
      incompleteBlock("check", "shell", { command: "npm run typecheck" }),
    ]),
    { id: "verify:type", label: "正在检查类型" },
  );
  assert.deepEqual(
    deriveLiveProgressCandidate([
      incompleteBlock("lint", "shell", { command: "npm run lint" }),
    ]),
    { id: "verify:lint", label: "正在检查代码" },
  );
  assert.deepEqual(
    deriveLiveProgressCandidate([
      incompleteBlock("check", "shell", { command: "npm run check" }),
    ]),
    { id: "verify:check", label: "正在验证结果" },
  );
});

test("redacts sensitive names and bounds every visible object label", () => {
  const deriveLiveProgressCandidate = (turnView as ProgressModule).deriveLiveProgressCandidate!;
  const secret = deriveLiveProgressCandidate([
    incompleteBlock("secret", "tool_call", {
      tool_name: "read_file",
      tool_input: { path: "/repo/config/.env.production", token: "never-show-this" },
    }),
  ]);
  const url = deriveLiveProgressCandidate([
    incompleteBlock("url", "tool_call", {
      tool_name: "read_file",
      tool_input: { path: "https://localhost/src/App.tsx?token=never-show-this" },
    }),
  ]);
  const longName = deriveLiveProgressCandidate([
    incompleteBlock("long", "tool_call", {
      tool_name: "edit",
      tool_input: { path: "/repo/ThisFileNameIsDeliberatelyLongEnoughToOverflowTheProgressRow.tsx" },
    }),
  ]);

  assert.deepEqual(secret, { id: "read", label: "正在查看相关内容" });
  assert.deepEqual(url, { id: "read:App.tsx", label: "正在查看 App.tsx" });
  assert.ok((longName?.label.length ?? 100) <= 52);
  assert.equal(longName?.label.includes("ThisFileNameIsDeliberately"), true);
  assert.equal(longName?.label.endsWith(".tsx"), true);
  assert.equal(JSON.stringify([secret, url, longName]).includes("never-show-this"), false);
});

function incompleteBlock(
  block_id: string,
  event_type: string,
  metadata: Record<string, unknown>,
): BlockState {
  return { block_id, event_type, content: "", metadata, isComplete: false };
}
