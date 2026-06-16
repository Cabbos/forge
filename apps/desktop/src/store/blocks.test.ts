import { describe, it } from "node:test";
import assert from "node:assert";
import {
  applyTranscriptEventToBlocks,
  closeInterruptedConfirmBlocks,
  eventToBlock,
} from "./blocks.ts";
import type { StreamEvent } from "../lib/protocol.ts";

describe("eventToBlock", () => {
  it("context_compact_start returns a running block", () => {
    const event: StreamEvent = {
      event_type: "context_compact_start",
      session_id: "s1",
      block_id: "b1",
    };
    const block = eventToBlock(event);
    assert.ok(block);
    assert.strictEqual(block!.event_type, "context_compact_start");
    assert.strictEqual(block!.block_id, "b1");
    assert.strictEqual(block!.isComplete, false);
  });

  it("context_compacted returns a completed block", () => {
    const event: StreamEvent = {
      event_type: "context_compacted",
      session_id: "s1",
      block_id: "b1",
      summary: "summary text",
      retained_messages: 10,
      compacted_messages: 20,
      estimated_tokens_before: 1000,
      estimated_tokens_after: 500,
    };
    const block = eventToBlock(event);
    assert.ok(block);
    assert.strictEqual(block!.event_type, "context_compacted");
    assert.strictEqual(block!.isComplete, true);
    assert.strictEqual(block!.metadata.retained_messages, 10);
  });

  it("context_compact_skipped returns a completed block", () => {
    const event: StreamEvent = {
      event_type: "context_compact_skipped",
      session_id: "s1",
      block_id: "b1",
      reason: "history_too_short",
      retained_messages: 5,
    };
    const block = eventToBlock(event);
    assert.ok(block);
    assert.strictEqual(block!.event_type, "context_compact_skipped");
    assert.strictEqual(block!.isComplete, true);
  });
});

describe("applyTranscriptEventToBlocks compact lifecycle", () => {
  it("context_compact_start creates a running block", () => {
    const startEvent: StreamEvent = {
      event_type: "context_compact_start",
      session_id: "s1",
      block_id: "compact-1",
    };
    const blocks = applyTranscriptEventToBlocks([], startEvent);
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "context_compact_start");
    assert.strictEqual(blocks[0].isComplete, false);
  });

  it("context_compacted updates the same block_id from start to completed", () => {
    const startEvent: StreamEvent = {
      event_type: "context_compact_start",
      session_id: "s1",
      block_id: "compact-1",
    };
    const compactedEvent: StreamEvent = {
      event_type: "context_compacted",
      session_id: "s1",
      block_id: "compact-1",
      summary: "summary",
      retained_messages: 10,
      compacted_messages: 20,
      estimated_tokens_before: 1000,
      estimated_tokens_after: 500,
    };
    let blocks = applyTranscriptEventToBlocks([], startEvent);
    blocks = applyTranscriptEventToBlocks(blocks, compactedEvent);
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "context_compacted");
    assert.strictEqual(blocks[0].isComplete, true);
    assert.strictEqual(blocks[0].content, "summary");
  });

  it("context_compact_skipped updates the same block_id from start to skipped", () => {
    const startEvent: StreamEvent = {
      event_type: "context_compact_start",
      session_id: "s1",
      block_id: "compact-1",
    };
    const skippedEvent: StreamEvent = {
      event_type: "context_compact_skipped",
      session_id: "s1",
      block_id: "compact-1",
      reason: "history_too_short",
      retained_messages: 5,
    };
    let blocks = applyTranscriptEventToBlocks([], startEvent);
    blocks = applyTranscriptEventToBlocks(blocks, skippedEvent);
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "context_compact_skipped");
    assert.strictEqual(blocks[0].isComplete, true);
  });

  it("context_compacted without a prior start still creates a block", () => {
    const compactedEvent: StreamEvent = {
      event_type: "context_compacted",
      session_id: "s1",
      block_id: "compact-1",
      summary: "summary",
      retained_messages: 10,
      compacted_messages: 20,
      estimated_tokens_before: 1000,
      estimated_tokens_after: 500,
    };
    const blocks = applyTranscriptEventToBlocks([], compactedEvent);
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "context_compacted");
  });
});

describe("closeInterruptedConfirmBlocks", () => {
  it("marks pending confirmation blocks as interrupted", () => {
    const blocks = closeInterruptedConfirmBlocks([
      {
        block_id: "confirm-1",
        event_type: "confirm_ask",
        content: "Continue?",
        isComplete: false,
        metadata: { kind: "shell_cmd" },
      },
    ], "session_restored");

    assert.strictEqual(blocks[0].isComplete, true);
    assert.strictEqual(blocks[0].metadata.confirmed, true);
    assert.strictEqual(blocks[0].metadata.answer, null);
    assert.strictEqual(blocks[0].metadata.confirm_interrupted, true);
    assert.strictEqual(blocks[0].metadata.confirm_interrupted_reason, "session_restored");
  });

  it("keeps already resolved confirmation blocks unchanged", () => {
    const original = {
      block_id: "confirm-1",
      event_type: "confirm_ask",
      content: "Continue?",
      isComplete: true,
      metadata: { kind: "shell_cmd", confirmed: true, answer: true },
    };

    const blocks = closeInterruptedConfirmBlocks([original], "session_stopped");

    assert.deepStrictEqual(blocks[0], original);
  });
});
