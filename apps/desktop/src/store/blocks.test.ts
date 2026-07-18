import { describe, it } from "node:test";
import assert from "node:assert";
import {
  applyConfirmResponseToBlocks,
  applyPermissionDecisionToBlocks,
  applyTranscriptEventToBlocks,
  closeInterruptedConfirmBlocks,
  eventToBlock,
  SESSION_RESTORED_TOOL_INTERRUPTION_MESSAGE,
  transcriptEventsToBlocks,
} from "./blocks.ts";
import type { PermissionLedgerEvent, StreamEvent } from "../lib/protocol.ts";
import type { SubagentRuntimePayload } from "../lib/protocol.ts";
import { readConversationTurnTiming } from "../lib/conversationTurnTiming.ts";
import {
  applyLoopRuntimeUpdate,
  applySubagentRuntimeEvent,
  runtimeFactSourcesForSubagentTasks,
} from "./runtime-projections.ts";
import { runtimeFactsForSubagentTask } from "../lib/loopRuntime.ts";
import type {
  LoopRuntimeByTask,
  SubagentRuntimeByTask,
} from "./types.ts";

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

  it("provider_usage returns a completed known-usage block with structured metadata", () => {
    const event: StreamEvent = {
      event_type: "provider_usage",
      session_id: "s1",
      provider_id: "anthropic",
      model: "claude-sonnet",
      source: "anthropic",
      reason: "provider_reported",
      input_tokens: 1200,
      output_tokens: 300,
      cache_read_tokens: null,
      cache_creation_tokens: null,
      reasoning_tokens: null,
      estimated_cost_micros: 4567,
      pricing_source: "forge_static_pricing_2026_06_20",
    };
    const block = eventToBlock(event);
    assert.ok(block);
    assert.strictEqual(block!.event_type, "provider_usage");
    assert.strictEqual(block!.isComplete, true);
    assert.match(block!.content, /claude-sonnet/);
    assert.match(block!.content, /input 1200/);
    assert.match(block!.content, /output 300/);
    assert.match(block!.content, /cost 4567 micros/);
    assert.deepStrictEqual(block!.metadata, {
      provider_id: "anthropic",
      model: "claude-sonnet",
      source: "anthropic",
      reason: "provider_reported",
      input_tokens: 1200,
      output_tokens: 300,
      cache_read_tokens: null,
      cache_creation_tokens: null,
      reasoning_tokens: null,
      estimated_cost_micros: 4567,
      pricing_source: "forge_static_pricing_2026_06_20",
      input_tokens_unknown: false,
      output_tokens_unknown: false,
      cost_unknown: false,
    });
  });

  it("provider_usage labels provider-omitted unknown usage without dropping metadata", () => {
    const event: StreamEvent = {
      event_type: "provider_usage",
      session_id: "s1",
      model: "claude-sonnet",
      source: "anthropic",
      reason: "provider_omitted",
      input_tokens: null,
      output_tokens: null,
      estimated_cost_micros: null,
    };
    const block = eventToBlock(event);
    assert.ok(block);
    assert.strictEqual(block!.event_type, "provider_usage");
    assert.match(block!.content, /provider omitted/);
    assert.match(block!.content, /input unknown/);
    assert.match(block!.content, /output unknown/);
    assert.match(block!.content, /cost unknown/);
    assert.strictEqual(block!.metadata.reason, "provider_omitted");
    assert.strictEqual(block!.metadata.input_tokens_unknown, true);
    assert.strictEqual(block!.metadata.output_tokens_unknown, true);
    assert.strictEqual(block!.metadata.cost_unknown, true);
  });

  it("provider_usage labels pricing-unknown cost while preserving known tokens", () => {
    const event: StreamEvent = {
      event_type: "provider_usage",
      session_id: "s1",
      model: "mystery-model",
      source: "openai_compatible",
      reason: "pricing_unknown",
      input_tokens: 44,
      output_tokens: 12,
      estimated_cost_micros: null,
    };
    const block = eventToBlock(event);
    assert.ok(block);
    assert.strictEqual(block!.event_type, "provider_usage");
    assert.match(block!.content, /pricing unknown/);
    assert.match(block!.content, /input 44/);
    assert.match(block!.content, /output 12/);
    assert.match(block!.content, /cost unknown/);
    assert.strictEqual(block!.metadata.input_tokens, 44);
    assert.strictEqual(block!.metadata.output_tokens, 12);
    assert.strictEqual(block!.metadata.estimated_cost_micros, null);
    assert.strictEqual(block!.metadata.cost_unknown, true);
  });

  it("provider_usage uses backend block IDs so identical usage payloads stay distinct", () => {
    const first = {
      event_type: "provider_usage",
      session_id: "s1",
      block_id: "usage-block-1",
      model: "claude-sonnet",
      source: "anthropic",
      reason: "provider_omitted",
      input_tokens: null,
      output_tokens: null,
      estimated_cost_micros: null,
    } as StreamEvent;
    const second = {
      ...first,
      block_id: "usage-block-2",
    } as StreamEvent;

    const firstBlock = eventToBlock(first);
    const secondBlock = eventToBlock(second);

    assert.ok(firstBlock);
    assert.ok(secondBlock);
    assert.strictEqual(firstBlock!.block_id, "usage-block-1");
    assert.strictEqual(secondBlock!.block_id, "usage-block-2");
    assert.notStrictEqual(firstBlock!.block_id, secondBlock!.block_id);
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

describe("applyTranscriptEventToBlocks provider_usage", () => {
  it("appends provider_usage as a standalone completed direct-session block", () => {
    const blocks = applyTranscriptEventToBlocks([], {
      event_type: "provider_usage",
      session_id: "s1",
      model: "claude-sonnet",
      source: "anthropic",
      reason: "provider_reported",
      input_tokens: 100,
      output_tokens: 50,
      estimated_cost_micros: 123,
    });

    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "provider_usage");
    assert.strictEqual(blocks[0].isComplete, true);
    assert.strictEqual(blocks[0].metadata.model, "claude-sonnet");
  });
});

describe("transcript conversation turn timing", () => {
  it("restores replayed user start and completed terminal timestamps", () => {
    let blocks = applyTranscriptEventToBlocks([], replayedEvent({
      event_type: "user_message",
      session_id: "s1",
      block_id: "user-1",
      content: "Ship it",
    }, 1_000));

    blocks = applyTranscriptEventToBlocks(blocks, replayedEvent({
      event_type: "agent_turn_updated",
      session_id: "s1",
      state: testAgentTurnProjection("running_tools"),
    }, 2_000));
    assert.deepStrictEqual(readConversationTurnTiming(blocks[0]), {
      startedAtMs: 1_000,
      terminalAtMs: null,
      outcome: null,
      durationMs: null,
    });

    blocks = applyTranscriptEventToBlocks(blocks, replayedEvent({
      event_type: "agent_turn_updated",
      session_id: "s1",
      state: testAgentTurnProjection("completed"),
    }, 13_000));
    assert.deepStrictEqual(readConversationTurnTiming(blocks[0]), {
      startedAtMs: 1_000,
      terminalAtMs: 13_000,
      outcome: "completed",
      durationMs: 12_000,
    });
  });

  it("uses replayed session_stopped as the terminal fallback", () => {
    const blocks = transcriptEventsToBlocks([
      replayedEvent({
        event_type: "user_message",
        session_id: "s1",
        block_id: "user-1",
        content: "Check then stop",
      }, 1_000),
      replayedEvent({
        event_type: "session_stopped",
        session_id: "s1",
        reason: "user_requested",
      }, 9_000),
    ]);

    assert.deepStrictEqual(readConversationTurnTiming(blocks[0]), {
      startedAtMs: 1_000,
      terminalAtMs: 9_000,
      outcome: "stopped",
      durationMs: 8_000,
    });
  });

  it("keeps legacy raw transcript events honest when timing is unavailable", () => {
    const initial = applyTranscriptEventToBlocks([], {
      event_type: "user_message",
      session_id: "s1",
      block_id: "user-legacy",
      content: "Legacy turn",
    });
    const blocks = applyTranscriptEventToBlocks(initial, {
      event_type: "agent_turn_updated",
      session_id: "s1",
      state: testAgentTurnProjection("completed"),
    });

    assert.strictEqual(blocks, initial);
    assert.deepStrictEqual(blocks[0].metadata, {});
    assert.deepStrictEqual(readConversationTurnTiming(blocks[0]), {
      startedAtMs: null,
      terminalAtMs: null,
      outcome: null,
      durationMs: null,
    });
  });

  it("does not overwrite an already terminal replayed outcome", () => {
    const blocks = transcriptEventsToBlocks([
      replayedEvent({
        event_type: "user_message",
        session_id: "s1",
        block_id: "user-1",
        content: "Ship it",
      }, 1_000),
      replayedEvent({
        event_type: "agent_turn_updated",
        session_id: "s1",
        state: testAgentTurnProjection("completed"),
      }, 2_000),
      replayedEvent({
        event_type: "agent_turn_updated",
        session_id: "s1",
        state: testAgentTurnProjection("failed"),
      }, 3_000),
    ]);

    assert.deepStrictEqual(readConversationTurnTiming(blocks[0]), {
      startedAtMs: 1_000,
      terminalAtMs: 2_000,
      outcome: "completed",
      durationMs: 1_000,
    });
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

describe("replayed confirm_ask", () => {
  it("eventToBlock sets interrupted metadata when replayed_interrupted is true", () => {
    const event: StreamEvent = {
      event_type: "confirm_ask",
      session_id: "s1",
      block_id: "confirm-1",
      question: "Allow write?",
      kind: "file_write",
      replayed_interrupted: true,
    };
    const block = eventToBlock(event);
    assert.ok(block);
    assert.strictEqual(block!.event_type, "confirm_ask");
    assert.strictEqual(block!.isComplete, true);
    assert.strictEqual(block!.metadata.confirmed, true);
    assert.strictEqual(block!.metadata.answer, null);
    assert.strictEqual(block!.metadata.confirm_interrupted, true);
    assert.strictEqual(block!.metadata.confirm_interrupted_reason, "session_restored");
  });

  it("eventToBlock creates normal block when replayed_interrupted is false/omitted", () => {
    const event: StreamEvent = {
      event_type: "confirm_ask",
      session_id: "s1",
      block_id: "confirm-1",
      question: "Allow write?",
      kind: "file_write",
    };
    const block = eventToBlock(event);
    assert.ok(block);
    assert.strictEqual(block!.event_type, "confirm_ask");
    assert.strictEqual(block!.isComplete, false);
    assert.strictEqual(block!.metadata.confirmed, undefined);
  });

  it("applyTranscriptEventToBlocks replaces existing confirm_ask with replayed one", () => {
    const normal: StreamEvent = {
      event_type: "confirm_ask",
      session_id: "s1",
      block_id: "confirm-1",
      question: "Allow write?",
      kind: "file_write",
    };
    let blocks = applyTranscriptEventToBlocks([], normal);
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].isComplete, false);

    const replay: StreamEvent = {
      event_type: "confirm_ask",
      session_id: "s1",
      block_id: "confirm-1",
      question: "Allow write?",
      kind: "file_write",
      replayed_interrupted: true,
    };
    blocks = applyTranscriptEventToBlocks(blocks, replay);
    // Should have replaced, not appended
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].isComplete, true);
    assert.strictEqual(blocks[0].metadata.confirm_interrupted, true);
    assert.strictEqual(blocks[0].metadata.confirm_interrupted_reason, "session_restored");
  });

  it("applyTranscriptEventToBlocks appends replayed confirm when no existing block matches", () => {
    const replay: StreamEvent = {
      event_type: "confirm_ask",
      session_id: "s1",
      block_id: "confirm-2",
      question: "Allow delete?",
      kind: "file_delete",
      replayed_interrupted: true,
    };
    const blocks = applyTranscriptEventToBlocks([], replay);
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].block_id, "confirm-2");
    assert.strictEqual(blocks[0].isComplete, true);
    assert.strictEqual(blocks[0].metadata.confirm_interrupted, true);
  });
});

describe("confirm_response replay", () => {
  it("applyTranscriptEventToBlocks marks an existing confirmation as approved", () => {
    let blocks = applyTranscriptEventToBlocks([], {
      event_type: "confirm_ask",
      session_id: "s1",
      block_id: "confirm-1",
      question: "Allow write?",
      kind: "file_write",
      boundary: testWriteBoundary(),
    });

    blocks = applyTranscriptEventToBlocks(blocks, {
      event_type: "confirm_response",
      session_id: "s1",
      block_id: "confirm-1",
      approved: true,
      responded_at_ms: 1,
      reason: "user_response",
      replayed: false,
    });

    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "confirm_ask");
    assert.strictEqual(blocks[0].isComplete, true);
    assert.strictEqual(blocks[0].metadata.confirmed, true);
    assert.strictEqual(blocks[0].metadata.answer, true);
    assert.strictEqual(blocks[0].metadata.confirm_interrupted, undefined);
  });

  it("applyTranscriptEventToBlocks marks an existing confirmation as declined", () => {
    let blocks = applyTranscriptEventToBlocks([], {
      event_type: "confirm_ask",
      session_id: "s1",
      block_id: "confirm-2",
      question: "Allow delete?",
      kind: "file_delete",
    });

    blocks = applyTranscriptEventToBlocks(blocks, {
      event_type: "confirm_response",
      session_id: "s1",
      block_id: "confirm-2",
      approved: false,
      responded_at_ms: 2,
      reason: "user_response",
      replayed: false,
    });

    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].isComplete, true);
    assert.strictEqual(blocks[0].metadata.confirmed, true);
    assert.strictEqual(blocks[0].metadata.answer, false);
    assert.strictEqual(blocks[0].metadata.confirm_interrupted, undefined);
  });

  it("applyTranscriptEventToBlocks creates a completed interrupted confirmation from an orphan response", () => {
    const boundary = testWriteBoundary();
    const blocks = applyTranscriptEventToBlocks([], {
      event_type: "confirm_response",
      session_id: "s1",
      block_id: "confirm-restored",
      approved: null,
      responded_at_ms: 3,
      reason: "session_restored",
      replayed: true,
      question: "Allow restored write?",
      kind: "write_file",
      boundary,
    });

    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "confirm_ask");
    assert.strictEqual(blocks[0].content, "Allow restored write?");
    assert.strictEqual(blocks[0].isComplete, true);
    assert.strictEqual(blocks[0].metadata.kind, "write_file");
    assert.deepStrictEqual(blocks[0].metadata.boundary, boundary);
    assert.strictEqual(blocks[0].metadata.confirmed, true);
    assert.strictEqual(blocks[0].metadata.answer, null);
    assert.strictEqual(blocks[0].metadata.confirm_interrupted, true);
    assert.strictEqual(blocks[0].metadata.confirm_interrupted_reason, "session_restored");
  });

  it("keeps backend permission evidence on confirm ask and response blocks", () => {
    const evidence: PermissionLedgerEvent = {
      kind: "manual_required",
      workspace_path: "/workspace/forge",
      session_id: "s1",
      risk_tier: "caution",
      affected_files: ["src/main.rs"],
      operation: "write_to_file",
      permission_mode: "manual_confirm",
      reason: "manual_confirm_requires_user_response",
    };
    const askEvent: StreamEvent = {
      event_type: "confirm_ask",
      session_id: "s1",
      block_id: "confirm-evidence",
      question: "Allow write?",
      kind: "file_write",
      boundary: null,
      permission_evidence: evidence,
    };
    const responseEvent: Extract<StreamEvent, { event_type: "confirm_response" }> = {
      event_type: "confirm_response",
      session_id: "s1",
      block_id: "confirm-evidence",
      question: "Allow write?",
      kind: "file_write",
      boundary: null,
      approved: true,
      responded_at_ms: 123,
      reason: "user_response",
      permission_evidence: {
        ...evidence,
        kind: "user_approved",
        reason: "user_response",
      },
    };

    const askBlock = eventToBlock(askEvent);
    assert.ok(askBlock);
    assert.deepStrictEqual(askBlock!.metadata.permission_evidence, evidence);

    const blocks = applyConfirmResponseToBlocks([askBlock!], responseEvent);

    assert.strictEqual(blocks[0].metadata.confirmed, true);
    assert.strictEqual(blocks[0].metadata.answer, true);
    assert.deepStrictEqual(blocks[0].metadata.permission_evidence, {
      ...evidence,
      kind: "user_approved",
      reason: "user_response",
    });
  });
});

describe("permission_decision replay", () => {
  const evidence: PermissionLedgerEvent = {
    kind: "auto_approved",
    workspace_path: "/workspace/forge",
    session_id: "s1",
    risk_tier: "normal",
    affected_files: [],
    operation: "read_file",
    permission_mode: "full_access",
    reason: "full_access_current_project",
  };

  const permissionDecision: Extract<StreamEvent, { event_type: "permission_decision" }> = {
    event_type: "permission_decision",
    session_id: "s1",
    block_id: "tool-1",
    evidence,
  };

  it("merges permission evidence into an existing tool block", () => {
    const blocks = applyPermissionDecisionToBlocks([
      {
        block_id: "tool-1",
        event_type: "tool_call",
        content: "",
        isComplete: false,
        metadata: { tool_name: "read_file", tool_input: { path: "README.md" } },
      },
    ], permissionDecision);

    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "tool_call");
    assert.deepStrictEqual(blocks[0].metadata.permission_evidence, evidence);
  });

  it("keeps out-of-order permission decisions internal until tool_start upgrades them", () => {
    let blocks = applyTranscriptEventToBlocks([], permissionDecision);
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "permission_decision");
    assert.strictEqual(blocks[0].content, "");

    blocks = applyTranscriptEventToBlocks(blocks, {
      event_type: "tool_call_start",
      session_id: "s1",
      block_id: "tool-1",
      tool_name: "read_file",
      tool_input: { path: "README.md" },
    });

    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "tool_call");
    assert.deepStrictEqual(blocks[0].metadata.permission_evidence, evidence);
    assert.strictEqual(blocks[0].metadata.tool_name, "read_file");
  });

  it("does not create a standalone rendered block from eventToBlock", () => {
    assert.strictEqual(eventToBlock(permissionDecision), null);
  });
});

describe("tool_call_start deduplication (Phase 1.6)", () => {
  it("tool_call_start creates a new block when no existing block matches", () => {
    const startEvent: StreamEvent = {
      event_type: "tool_call_start",
      session_id: "s1",
      block_id: "tool-1",
      tool_name: "write_to_file",
      tool_input: { path: "file.txt" },
    };
    const blocks = applyTranscriptEventToBlocks([], startEvent);
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].block_id, "tool-1");
    assert.strictEqual(blocks[0].event_type, "tool_call");
    assert.strictEqual(blocks[0].metadata.tool_name, "write_to_file");
    assert.deepStrictEqual(blocks[0].metadata.tool_input, { path: "file.txt" });
  });

  it("tool_call_start updates existing block with same block_id instead of appending", () => {
    const first: StreamEvent = {
      event_type: "tool_call_start",
      session_id: "s1",
      block_id: "tool-1",
      tool_name: "write_to_file",
      tool_input: { path: "old.txt" },
    };
    let blocks = applyTranscriptEventToBlocks([], first);
    assert.strictEqual(blocks.length, 1);

    const duplicate: StreamEvent = {
      event_type: "tool_call_start",
      session_id: "s1",
      block_id: "tool-1",
      tool_name: "write_to_file",
      tool_input: { path: "new.txt" },
    };
    blocks = applyTranscriptEventToBlocks(blocks, duplicate);
    // Should update, not append
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "tool_call");
    assert.strictEqual(blocks[0].metadata.tool_name, "write_to_file");
    assert.deepStrictEqual(blocks[0].metadata.tool_input, { path: "new.txt" });
  });

  it("tool_call_start then interrupted tool_call_result updates one block", () => {
    const startEvent: StreamEvent = {
      event_type: "tool_call_start",
      session_id: "s1",
      block_id: "tool-1",
      tool_name: "run_shell",
      tool_input: { command: "npm test" },
    };
    let blocks = applyTranscriptEventToBlocks([], startEvent);
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].isComplete, false);
    assert.strictEqual(blocks[0].metadata.tool_name, "run_shell");

    const resultEvent: StreamEvent = {
      event_type: "tool_call_result",
      session_id: "s1",
      block_id: "tool-1",
      result: SESSION_RESTORED_TOOL_INTERRUPTION_MESSAGE,
      is_error: true,
      duration_ms: 5000,
    };
    blocks = applyTranscriptEventToBlocks(blocks, resultEvent);
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].isComplete, true);
    assert.strictEqual(blocks[0].metadata.is_error, true);
    assert.strictEqual(blocks[0].metadata.duration_ms, 5000);
    assert.strictEqual(blocks[0].metadata.tool_interrupted, true);
    assert.strictEqual(blocks[0].metadata.tool_interrupted_reason, "session_restored");
    // tool_name should be preserved from the start event
    assert.strictEqual(blocks[0].metadata.tool_name, "run_shell");
    assert.deepStrictEqual(blocks[0].metadata.tool_input, { command: "npm test" });
  });

  it("tool_call_result without prior start still creates a completed tool block", () => {
    const resultEvent: StreamEvent = {
      event_type: "tool_call_result",
      session_id: "s1",
      block_id: "tool-orphan",
      result: "Some result",
      is_error: false,
      duration_ms: 100,
    };
    const blocks = applyTranscriptEventToBlocks([], resultEvent);
    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].block_id, "tool-orphan");
    assert.strictEqual(blocks[0].event_type, "tool_call");
    assert.strictEqual(blocks[0].isComplete, true);
    assert.strictEqual(blocks[0].metadata.tool_name, "Tool");
    assert.strictEqual(blocks[0].metadata.tool_interrupted, undefined);
  });

  it("eventToBlock marks restore-interrupted tool results with metadata", () => {
    const resultEvent: StreamEvent = {
      event_type: "tool_call_result",
      session_id: "s1",
      block_id: "tool-interrupted",
      result: SESSION_RESTORED_TOOL_INTERRUPTION_MESSAGE,
      is_error: true,
      duration_ms: 250,
    };
    const block = eventToBlock(resultEvent);
    assert.ok(block);
    assert.strictEqual(block!.metadata.tool_interrupted, true);
    assert.strictEqual(block!.metadata.tool_interrupted_reason, "session_restored");
  });
});

describe("file_io transcript projection", () => {
  it("attaches file IO metadata to an existing tool block", () => {
    let blocks = applyTranscriptEventToBlocks([], {
      event_type: "tool_call_start",
      session_id: "s1",
      block_id: "tool-file-1",
      tool_name: "read_file",
      tool_input: { path: "src/main.rs" },
    });

    blocks = applyTranscriptEventToBlocks(blocks, {
      event_type: "file_io",
      session_id: "s1",
      block_id: "tool-file-1",
      path: "/workspace/src/main.rs",
      operation: "read",
      source: "executor",
    });

    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "tool_call");
    assert.deepStrictEqual(blocks[0].metadata.file_io_events, [
      {
        path: "/workspace/src/main.rs",
        operation: "read",
        source: "executor",
      },
    ]);
  });

  it("ignores file IO events without creating standalone blocks", () => {
    const blocks = applyTranscriptEventToBlocks([], {
      event_type: "file_io",
      session_id: "s1",
      block_id: "missing-tool",
      path: "/workspace/src/main.rs",
      operation: "read",
      source: "executor",
    });

    assert.deepStrictEqual(blocks, []);
  });

  it("attaches post-shell delta metadata to the matching shell block", () => {
    let blocks = applyTranscriptEventToBlocks([], {
      event_type: "shell_start",
      session_id: "s1",
      block_id: "shell-1",
      command: "printf 'hello' > shell.txt",
    });

    blocks = applyTranscriptEventToBlocks(blocks, {
      event_type: "file_io",
      session_id: "s1",
      block_id: "shell-1",
      path: "shell.txt",
      operation: "created",
      source: "post_shell_delta",
    });

    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].event_type, "shell");
    assert.deepStrictEqual(blocks[0].metadata.file_io_events, [
      {
        path: "shell.txt",
        operation: "created",
        source: "post_shell_delta",
      },
    ]);
  });

  it("does not attach a post-shell delta to a mismatched shell block id", () => {
    const blocks = applyTranscriptEventToBlocks([
      {
        block_id: "shell-1",
        event_type: "shell",
        content: "",
        isComplete: true,
        metadata: {
          command: "printf 'hello' > shell.txt",
          exit_code: 0,
        },
      },
    ], {
      event_type: "file_io",
      session_id: "s1",
      block_id: "shell-2",
      path: "shell.txt",
      operation: "created",
      source: "post_shell_delta",
    });

    assert.strictEqual(blocks.length, 1);
    assert.strictEqual(blocks[0].block_id, "shell-1");
    assert.equal(blocks[0].metadata.file_io_events, undefined);
  });
});

describe("subagent runtime projections", () => {
  it("stores subagent runtime events outside transcript blocks", () => {
    const blocks = applyTranscriptEventToBlocks([], {
      event_type: "subagent_runtime_event",
      session_id: "s1",
      loop_task_id: "loop-1",
      task_id: "task-1",
      event: { type: "started", role: "implementer" },
    });
    let runtimeByTask: SubagentRuntimeByTask = new Map();

    runtimeByTask = applySubagentRuntimeEvent(runtimeByTask, {
      event_type: "subagent_runtime_event",
      session_id: "s1",
      loop_task_id: "loop-1",
      task_id: "task-1",
      event: { type: "started", role: "implementer" },
    });

    assert.strictEqual(blocks.length, 0);
    assert.deepStrictEqual(
      runtimeByTask.get("s1:task-1"),
      {
        session_id: "s1",
        loop_task_id: "loop-1",
        task_id: "task-1",
        latest_event: { type: "started", role: "implementer" },
        status: "started",
        role: "implementer",
      },
    );
  });

  it("preserves status text when status payload omits message", () => {
    let runtimeByTask: SubagentRuntimeByTask = new Map();
    runtimeByTask = applySubagentRuntimeEvent(runtimeByTask, {
      event_type: "subagent_runtime_event",
      session_id: "s1",
      task_id: "task-1",
      event: { type: "status", status: "running", message: "Reading files" },
    });

    runtimeByTask = applySubagentRuntimeEvent(runtimeByTask, {
      event_type: "subagent_runtime_event",
      session_id: "s1",
      task_id: "task-1",
      event: { type: "status", status: "running" },
    });

    assert.strictEqual(runtimeByTask.get("s1:task-1")!.message, "Reading files");

    runtimeByTask = applySubagentRuntimeEvent(runtimeByTask, {
      event_type: "subagent_runtime_event",
      session_id: "s1",
      task_id: "task-1",
      event: { type: "status", status: "running", message: null },
    });

    assert.strictEqual(runtimeByTask.get("s1:task-1")!.message, "Reading files");

    runtimeByTask = applySubagentRuntimeEvent(runtimeByTask, {
      event_type: "subagent_runtime_event",
      session_id: "s1",
      task_id: "task-1",
      event: { type: "status", status: "running", message: "Running checks" },
    });

    assert.strictEqual(runtimeByTask.get("s1:task-1")!.message, "Running checks");
  });

  it("preserves meaningful state across telemetry-only subagent events", () => {
    let runtimeByTask: SubagentRuntimeByTask = new Map([
      [
        "s1:task-1",
        {
          session_id: "s1",
          loop_task_id: "loop-1",
          task_id: "task-1",
          latest_event: { type: "failed", reason: "needs review" },
          status: "running",
          role: "implementer",
          message: "Applying patch",
          reason: "needs review",
        },
      ],
    ]);

    runtimeByTask = applySubagentRuntimeEvent(runtimeByTask, {
      event_type: "subagent_runtime_event",
      session_id: "s1",
      loop_task_id: "loop-1",
      task_id: "task-1",
      event: { type: "file_io", path: "src/App.tsx", operation: "write" },
    });

    assert.deepStrictEqual(runtimeByTask.get("s1:task-1"), {
      session_id: "s1",
      loop_task_id: "loop-1",
      task_id: "task-1",
      latest_event: { type: "file_io", path: "src/App.tsx", operation: "write" },
      latest_file_io_event: { type: "file_io", path: "src/App.tsx", operation: "write" },
      status: "running",
      role: "implementer",
      message: "Applying patch",
      reason: "needs review",
    });

    runtimeByTask = applySubagentRuntimeEvent(runtimeByTask, {
      event_type: "subagent_runtime_event",
      session_id: "s1",
      loop_task_id: "loop-1",
      task_id: "task-1",
      event: {
        type: "usage_recorded",
        model: "test-model",
        input_tokens: null,
        output_tokens: null,
        estimated_cost_micros: null,
      },
    });

    assert.deepStrictEqual(runtimeByTask.get("s1:task-1"), {
      session_id: "s1",
      loop_task_id: "loop-1",
      task_id: "task-1",
      latest_event: {
        type: "usage_recorded",
        model: "test-model",
        input_tokens: null,
        output_tokens: null,
        estimated_cost_micros: null,
      },
      latest_usage_event: {
        type: "usage_recorded",
        model: "test-model",
        input_tokens: null,
        output_tokens: null,
        estimated_cost_micros: null,
      },
      latest_file_io_event: { type: "file_io", path: "src/App.tsx", operation: "write" },
      status: "running",
      role: "implementer",
      message: "Applying patch",
      reason: "needs review",
    });
  });

  it("retains usage and file facts after later subagent runtime events", () => {
    let runtimeByTask: SubagentRuntimeByTask = new Map();
    runtimeByTask = applySubagentRuntimeEvent(runtimeByTask, {
      event_type: "subagent_runtime_event",
      session_id: "s1",
      loop_task_id: "loop-1",
      task_id: "task-1",
      event: {
        type: "usage_recorded",
        model: "claude-sonnet",
        input_tokens: 40,
        output_tokens: 8,
        estimated_cost_micros: 120,
        source: "anthropic",
        reason: "provider_reported",
      },
    });
    runtimeByTask = applySubagentRuntimeEvent(runtimeByTask, {
      event_type: "subagent_runtime_event",
      session_id: "s1",
      loop_task_id: "loop-1",
      task_id: "task-1",
      event: { type: "file_io", path: "src/App.tsx", operation: "write" },
    });
    runtimeByTask = applySubagentRuntimeEvent(runtimeByTask, {
      event_type: "subagent_runtime_event",
      session_id: "s1",
      loop_task_id: "loop-1",
      task_id: "task-1",
      event: { type: "ended", status: "completed" },
    });

    const entry = runtimeByTask.get("s1:task-1") as
      | (NonNullable<ReturnType<SubagentRuntimeByTask["get"]>> & {
          latest_usage_event?: SubagentRuntimePayload;
          latest_file_io_event?: SubagentRuntimePayload;
        })
      | undefined;

    assert.ok(entry);
    assert.strictEqual(entry.latest_event.type, "ended");
    assert.strictEqual(entry.latest_usage_event?.type, "usage_recorded");
    assert.strictEqual(entry.latest_file_io_event?.type, "file_io");

    const sources = runtimeFactSourcesForSubagentTasks({
      entries: runtimeByTask,
      taskIds: new Set(["task-1"]),
      sessionId: "s1",
    });
    const facts = runtimeFactsForSubagentTask(sources, "task-1");

    assert.deepStrictEqual(facts.map((fact) => fact.kind), ["usage", "file_io"]);
    assert.strictEqual(facts[0].model, "claude-sonnet");
    assert.strictEqual(facts[0].inputTokens, 40);
    assert.strictEqual(facts[1].detail, "src/App.tsx");
  });

  it("stores gateway loop runtime updates before session lookup", () => {
    const blocks = applyTranscriptEventToBlocks([], {
      event_type: "loop_runtime_updated",
      session_id: "gateway",
      loop_task_id: "loop-1",
      task: testLoopTaskRecord(),
    });
    let runtimeByTask: LoopRuntimeByTask = new Map();

    runtimeByTask = applyLoopRuntimeUpdate(runtimeByTask, {
      event_type: "loop_runtime_updated",
      session_id: "gateway",
      loop_task_id: "loop-1",
      task: testLoopTaskRecord(),
    });

    assert.strictEqual(blocks.length, 0);
    assert.deepStrictEqual(
      runtimeByTask.get("gateway:loop-1"),
      {
        session_id: "gateway",
        loop_task_id: "loop-1",
        task: testLoopTaskRecord(),
      },
    );
  });
});

function replayedEvent<T extends StreamEvent>(
  event: T,
  recordedAtMs: number,
): T & { recorded_at_ms: number } {
  return { ...event, recorded_at_ms: recordedAtMs };
}

function testAgentTurnProjection(
  status: Extract<StreamEvent, { event_type: "agent_turn_updated" }>["state"]["status"],
): Extract<StreamEvent, { event_type: "agent_turn_updated" }>["state"] {
  return {
    session_id: "s1",
    status,
    step_label: status,
    workspace_path: "/workspace",
    compact_count: 0,
    verification_status: "not_needed",
    model_rounds: 1,
    tool_call_count: 0,
    failed_tool_count: status === "failed" ? 1 : 0,
    estimated_context_tokens: null,
    stop_reason: null,
    compact_saved_tokens: 0,
  };
}

function testLoopTaskRecord() {
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

function testWriteBoundary() {
  return {
    title: "Write src/main.ts",
    target_label: "src/main.ts",
    workspace_name: "workspace",
    workspace_path: "/workspace",
    operation: "write_file",
    affected_files: ["/workspace/src/main.ts"],
    command: null,
    impact: "Updates a source file.",
    risk: "normal" as const,
    recovery: "Revert the file if needed.",
    checkpoint_status: "ready" as const,
    warning: null,
  };
}
