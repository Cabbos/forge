import { describe, it } from "node:test";
import assert from "node:assert";
import { deriveToolCounts } from "../components/messages/processActivity.ts";
import type { BlockState } from "../lib/protocol.ts";

/**
 * Minimal BlockState factory for pure-helper tests.
 * Only block_id, event_type, and relevant metadata fields matter to deriveToolCounts.
 */
function block(
  block_id: string,
  event_type: string,
  metadata: Record<string, unknown> = {},
): BlockState {
  return {
    block_id,
    event_type,
    content: "",
    isComplete: true,
    metadata,
  };
}

describe("deriveToolCounts", () => {
  it("returns zeroed counts for empty input", () => {
    const result = deriveToolCounts([]);
    assert.strictEqual(result.totalTools, 0);
    assert.strictEqual(result.failedTools, 0);
    assert.strictEqual(result.shellCommands, 0);
    assert.strictEqual(result.shellChecks, 0);
    assert.strictEqual(result.failedShells, 0);
    assert.strictEqual(result.topTool, null);
    assert.deepStrictEqual(result.perTool, {});
  });

  it("counts unique tool_call blocks by tool_name", () => {
    const blocks = [
      block("b1", "tool_call", { tool_name: "read_file" }),
      block("b2", "tool_call", { tool_name: "write_file" }),
      block("b3", "tool_call", { tool_name: "read_file" }),
    ];
    const result = deriveToolCounts(blocks);
    assert.strictEqual(result.totalTools, 3);
    assert.strictEqual(result.failedTools, 0);
    assert.strictEqual(result.perTool["read_file"], 2);
    assert.strictEqual(result.perTool["write_file"], 1);
    assert.strictEqual(result.topTool, "read_file");
  });

  it("counts orphan tool_call_result blocks as tool calls", () => {
    const blocks = [
      block("b1", "tool_call_result", { tool_name: "Tool", is_error: true }),
      block("b2", "tool_call", { tool_name: "read_file" }),
    ];
    const result = deriveToolCounts(blocks);
    assert.strictEqual(result.totalTools, 2);
    assert.strictEqual(result.failedTools, 1);
    assert.strictEqual(result.perTool["Tool"], 1);
    assert.strictEqual(result.perTool["read_file"], 1);
  });

  it("deduplicates by block_id so replay / update does not overcount", () => {
    const blocks = [
      block("b1", "tool_call", { tool_name: "grep" }),
      block("b1", "tool_call", { tool_name: "grep" }),
      block("b2", "tool_call", { tool_name: "read_file" }),
      block("b2", "shell", { command: "ls" }),
    ];
    const result = deriveToolCounts(blocks);
    // b1 counted once, b2 counted once (first event_type wins: tool_call)
    assert.strictEqual(result.totalTools, 2);
    assert.strictEqual(result.perTool["grep"], 1);
    assert.strictEqual(result.perTool["read_file"], 1);
  });

  it("counts failed tool calls via metadata.is_error", () => {
    const blocks = [
      block("b1", "tool_call", { tool_name: "write_file", is_error: true }),
      block("b2", "tool_call", { tool_name: "read_file" }),
      block("b3", "tool_call", { tool_name: "bash", is_error: true }),
    ];
    const result = deriveToolCounts(blocks);
    assert.strictEqual(result.totalTools, 3);
    assert.strictEqual(result.failedTools, 2);
  });

  it("classifies shell blocks into checks and commands", () => {
    const blocks = [
      block("s1", "shell", { command: "npm run build" }),
      block("s2", "shell", { command: "cargo test" }),
      block("s3", "shell", { command: "cargo check" }),
      block("s4", "shell", { command: "npm run lint" }),
      block("s5", "shell", { command: "ls -la" }),
      block("s6", "shell", { command: "cat file.txt" }),
    ];
    const result = deriveToolCounts(blocks);
    assert.strictEqual(result.shellChecks, 4); // build, test, check, lint
    assert.strictEqual(result.shellCommands, 2); // ls, cat
    assert.strictEqual(result.failedShells, 0);
  });

  it("detects failed shells via non-zero exit_code", () => {
    const blocks = [
      block("s1", "shell", { command: "ls", exit_code: 0 }),
      block("s2", "shell", { command: "make", exit_code: 2 }),
      block("s3", "shell", { command: "npm test", exit_code: 1 }),
    ];
    const result = deriveToolCounts(blocks);
    // make does not match build/test/check/lint → command, exit_code=2 → failed
    // npm test matches "test" → check, exit_code=1 → failed
    assert.strictEqual(result.failedShells, 2);
    assert.strictEqual(result.shellCommands, 2); // ls, make
    assert.strictEqual(result.shellChecks, 1); // npm test
  });

  it("shell with missing exit_code is not counted as failed", () => {
    const blocks = [
      block("s1", "shell", { command: "ls" }),
    ];
    const result = deriveToolCounts(blocks);
    assert.strictEqual(result.failedShells, 0);
  });

  it("ignores non-tool, non-shell blocks", () => {
    const blocks = [
      block("b1", "user_message", {}),
      block("b2", "text", {}),
      block("b3", "confirm_ask", {}),
      block("b4", "tool_call", { tool_name: "read_file" }),
    ];
    const result = deriveToolCounts(blocks);
    assert.strictEqual(result.totalTools, 1);
  });

  it("reports topTool from most frequently used tool", () => {
    const blocks = [
      block("b1", "tool_call", { tool_name: "read_file" }),
      block("b2", "tool_call", { tool_name: "read_file" }),
      block("b3", "tool_call", { tool_name: "read_file" }),
      block("b4", "tool_call", { tool_name: "write_file" }),
      block("b5", "tool_call", { tool_name: "grep" }),
    ];
    const result = deriveToolCounts(blocks);
    assert.strictEqual(result.topTool, "read_file");
    assert.strictEqual(result.perTool["read_file"], 3);
    assert.strictEqual(result.perTool["write_file"], 1);
    assert.strictEqual(result.perTool["grep"], 1);
  });

  it("returns null topTool when no tool calls present", () => {
    const blocks = [
      block("s1", "shell", { command: "ls" }),
    ];
    const result = deriveToolCounts(blocks);
    assert.strictEqual(result.topTool, null);
  });

  it("handles missing tool_name gracefully as 'unknown'", () => {
    const blocks = [
      block("b1", "tool_call", {}),
    ];
    const result = deriveToolCounts(blocks);
    assert.strictEqual(result.totalTools, 1);
    assert.strictEqual(result.perTool["unknown"], 1);
  });

  it("handles missing command in shell gracefully", () => {
    const blocks = [
      block("s1", "shell", {}),
    ];
    const result = deriveToolCounts(blocks);
    assert.strictEqual(result.shellCommands, 1); // empty string doesn't match check pattern
    assert.strictEqual(result.shellChecks, 0);
  });
});
