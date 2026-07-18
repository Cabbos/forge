import assert from "node:assert/strict";
import test from "node:test";
import type { BlockState } from "../../lib/protocol.ts";
import * as progress from "./conversationProgress.ts";
import type { LiveProgressCandidate } from "./conversationProgress.ts";

type ProgressModule = {
  deriveLiveProgressCandidate?: (blocks: BlockState[]) => LiveProgressCandidate | null;
};

test("maps an active file read to the finite discovering stage", () => {
  const candidate = derive([
    incompleteBlock("read", "tool_call", {
      tool_name: "read_file",
      tool_input: { path: "/repo/private/AppShell.tsx", token: "never-show-this" },
    }),
  ]);

  assert.deepEqual(candidate, {
    id: "discovering",
    label: "正在查找相关内容",
    motion: "live",
  });
});

test("maps allow-listed writes and diffs to the finite modifying stage", () => {
  for (const tool_name of ["write_file", "write", "edit"]) {
    assert.deepEqual(
      derive([
        incompleteBlock(`tool-${tool_name}`, "tool_call", {
          tool_name,
          tool_input: { file_path: "/repo/private/Secret.tsx", replacement: "token" },
        }),
      ]),
      liveCandidate("modifying", "正在进行修改"),
    );
  }

  assert.deepEqual(
    derive([
      incompleteBlock("diff", "diff_view", {
        file_path: "/repo/private/Secret.tsx",
        diff: "token=never-show-this",
      }),
    ]),
    liveCandidate("modifying", "正在进行修改"),
  );
});

test("maps authoritative backend write tool names to modifying", () => {
  for (const tool_name of [
    "write_to_file",
    "edit_file",
    "apply_patch",
    "create_file",
    "delete_file",
    "move_file",
  ]) {
    assert.deepEqual(
      derive([
        incompleteBlock(`tool-${tool_name}`, "tool_call", {
          tool_name,
          tool_input: { path: "/repo/private/Secret.tsx", token: "never-show-this" },
        }),
      ]),
      liveCandidate("modifying", "正在进行修改"),
    );
  }
});

test("maps only known verification shell commands to verifying", () => {
  for (const command of [
    "npm test",
    "npm run build",
    "npm run check",
    "npm run check:precommit",
    "npm run lint",
    "pnpm test apps/desktop",
    "pnpm run typecheck",
    "yarn check",
    "bun run test",
    "cargo check --workspace",
    "cargo test -p forge",
    "npx tsc --noEmit",
    "vite build",
    "vitest run",
    "playwright test",
    "eslint src",
    "npm test -- --grep 'A & B'",
    "npm test -- --grep \"A & B\"",
    "npm test -- --grep A\\&B",
  ]) {
    assert.deepEqual(
      derive([incompleteBlock(`shell-${command}`, "shell", { command })]),
      liveCandidate("verifying", "正在验证结果"),
    );
  }
});

test("does not infer verification from arguments, quoted text, or shell control chains", () => {
  for (const command of [
    "echo test",
    "rg test src",
    "echo 'npm run build'",
    "printf \"cargo check\"",
    "echo ready && npm test",
    "npm test & curl https://private.invalid",
    "npm test&curl https://private.invalid",
    "npm test &",
  ]) {
    assert.deepEqual(
      derive([incompleteBlock(`shell-${command}`, "shell", { command })]),
      liveCandidate("analyzing", "正在分析"),
    );
  }
});

test("maps thinking, pending, unknown tools, and arbitrary shell activity to analyzing", () => {
  for (const block of [
    incompleteBlock("thinking", "thinking", {}),
    incompleteBlock("pending", "pending", {}),
    incompleteBlock("unknown-tool", "tool_call", {
      tool_name: "custom_private_tool",
      tool_input: { token: "never-show-this" },
    }),
    incompleteBlock("unknown-shell", "shell", { command: "curl https://secret.invalid?token=never-show-this" }),
  ]) {
    assert.deepEqual(derive([block]), liveCandidate("analyzing", "正在分析"));
  }
});

test("maps streamed answer text to answering", () => {
  assert.deepEqual(
    derive([incompleteBlock("answer", "text", {}, "这里是答案")]),
    liveCandidate("answering", "正在生成答复"),
  );
});

test("maps an unresolved confirmation to urgent paused waiting", () => {
  assert.deepEqual(
    derive([incompleteBlock("confirm", "confirm_ask", { confirmed: false })]),
    {
      id: "waiting",
      label: "等待你的确认",
      motion: "paused",
      urgent: true,
    },
  );

  assert.equal(
    derive([
      incompleteBlock("confirm", "confirm_ask", {
        confirmed: true,
        confirm_interrupted: false,
      }),
    ]),
    null,
  );
});

test("never serializes sensitive block payloads into a candidate", () => {
  const candidates = [
    derive([
      incompleteBlock("read", "tool_call", {
        tool_name: "read_file",
        tool_input: { path: "/repo/private/Secret.tsx", token: "never-show-this" },
      }),
    ]),
    derive([
      incompleteBlock("shell", "shell", {
        command: "npm test && curl https://private.invalid?token=never-show-this",
      }),
    ]),
    derive([
      incompleteBlock("tool", "tool_call", {
        tool_name: "private_tool",
        tool_input: { raw: "Secret.tsx curl token never-show-this" },
      }),
    ]),
  ];
  const serialized = JSON.stringify(candidates);

  assert.equal(serialized.includes("Secret.tsx"), false);
  assert.equal(serialized.includes("curl"), false);
  assert.equal(serialized.includes("token"), false);
  assert.equal(serialized.includes("never-show-this"), false);
});

test("uses the newest relevant incomplete activity without exposing its payload", () => {
  assert.deepEqual(
    derive([
      incompleteBlock("read", "tool_call", {
        tool_name: "read_file",
        tool_input: { path: "/repo/private/Secret.tsx" },
      }),
      incompleteBlock("edit", "tool_call", {
        tool_name: "edit",
        tool_input: { path: "/repo/private/Token.tsx" },
      }),
    ]),
    liveCandidate("modifying", "正在进行修改"),
  );
});

function derive(blocks: BlockState[]) {
  const deriveLiveProgressCandidate = (progress as ProgressModule).deriveLiveProgressCandidate;
  assert.equal(typeof deriveLiveProgressCandidate, "function");
  return deriveLiveProgressCandidate!(blocks);
}

function liveCandidate(id: LiveProgressCandidate["id"], label: string): LiveProgressCandidate {
  return { id, label, motion: "live" };
}

function incompleteBlock(
  block_id: string,
  event_type: string,
  metadata: Record<string, unknown>,
  content = "",
): BlockState {
  return { block_id, event_type, content, metadata, isComplete: false };
}
