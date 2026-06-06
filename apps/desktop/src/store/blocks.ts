import type { BlockState, DeliverySummary, StreamEvent } from "../lib/protocol";

export function transcriptEventsToBlocks(events: StreamEvent[]): BlockState[] {
  let blocks: BlockState[] = [];
  for (const event of events) {
    blocks = applyTranscriptEventToBlocks(blocks, event);
  }
  return blocks.filter((block) => block.event_type !== "pending");
}

export function applyTranscriptEventToBlocks(blocks: BlockState[], event: StreamEvent): BlockState[] {
  const event_type = event.event_type;

  if (event_type === "delivery_summary" && isSameAsLastDeliveryBlock(blocks, event.summary)) {
    return blocks;
  }

  if (event_type === "error") {
    return [
      ...blocks,
      {
        block_id: event.block_id,
        event_type: "error",
        content: event.message,
        metadata: { code: event.code },
        isComplete: true,
      },
    ];
  }

  if (event_type === "shell_start") {
    return applyShellStartToBlocks(blocks, event);
  }

  if (event_type === "tool_call_result") {
    const next = [...blocks];
    const existingIdx = findToolResultTargetBlockIndex(next, event.block_id);
    if (existingIdx >= 0) {
      next[existingIdx] = {
        ...next[existingIdx],
        content: event.result,
        isComplete: true,
        metadata: {
          ...next[existingIdx].metadata,
          is_error: event.is_error,
          duration_ms: event.duration_ms,
        },
      };
      return next;
    }
    return [
      ...next,
      {
        block_id: event.block_id,
        event_type: "tool_call",
        content: event.result,
        isComplete: true,
        metadata: {
          is_error: event.is_error,
          duration_ms: event.duration_ms,
          tool_name: "Tool",
        },
      },
    ];
  }

  if (event_type === "thinking_chunk" || event_type === "text_chunk" || event_type === "shell_output") {
    const next = [...blocks];
    const existingIdx = event_type === "shell_output"
      ? findShellTargetBlockIndex(next, event.block_id)
      : next.findIndex((block) => block.block_id === event.block_id);
    const blockType = event_type === "thinking_chunk" ? "thinking" : event_type === "shell_output" ? "shell" : "text";
    if (existingIdx >= 0) {
      next[existingIdx] = {
        ...next[existingIdx],
        content: next[existingIdx].content + event.content,
      };
      return next;
    }
    return [
      ...next,
      {
        block_id: event.block_id,
        event_type: blockType,
        content: event.content,
        isComplete: false,
        metadata: {},
      },
    ];
  }

  if (event_type === "thinking_end" || event_type === "text_end" || event_type === "shell_end" || event_type === "tool_call_end") {
    const next = [...blocks];
    const existingIdx = event_type === "shell_end"
      ? findShellTargetBlockIndex(next, event.block_id)
      : next.findIndex((block) => block.block_id === event.block_id);
    if (existingIdx >= 0) {
      if (event_type !== "tool_call_end") {
        next[existingIdx] = { ...next[existingIdx], isComplete: true };
      }
      if (event_type === "shell_end") {
        next[existingIdx] = {
          ...next[existingIdx],
          metadata: { ...next[existingIdx].metadata, exit_code: event.exit_code },
        };
      }
    }
    return next;
  }

  const block = eventToBlock(event);
  return block ? [...blocks, block] : blocks;
}

export function applyShellStartToBlocks(
  blocks: BlockState[],
  event: Extract<StreamEvent, { event_type: "shell_start" }>,
): BlockState[] {
  const next = [...blocks];
  const existingShellIdx = next.findIndex((block) =>
    block.block_id === event.block_id && block.event_type === "shell"
  );
  if (existingShellIdx >= 0) {
    next[existingShellIdx] = {
      ...next[existingShellIdx],
      isComplete: false,
      metadata: {
        ...next[existingShellIdx].metadata,
        command: event.command,
      },
    };
    return next;
  }

  const existingToolIdx = next.findIndex((block) =>
    block.block_id === event.block_id &&
    (block.event_type === "tool_call" || block.event_type === "tool_call_result")
  );
  if (existingToolIdx >= 0) {
    next[existingToolIdx] = {
      ...next[existingToolIdx],
      event_type: "shell",
      content: "",
      isComplete: false,
      metadata: {
        ...next[existingToolIdx].metadata,
        command: event.command,
      },
    };
    return next;
  }

  const block = eventToBlock(event);
  return block ? [...next, block] : next;
}

export function findShellTargetBlockIndex(blocks: BlockState[], blockId: string) {
  const shellIdx = blocks.findIndex((block) =>
    block.block_id === blockId && block.event_type === "shell"
  );
  if (shellIdx >= 0) return shellIdx;
  return blocks.findIndex((block) => block.block_id === blockId);
}

export function findToolResultTargetBlockIndex(blocks: BlockState[], blockId: string) {
  const shellIdx = blocks.findIndex((block) =>
    block.block_id === blockId && block.event_type === "shell"
  );
  if (shellIdx >= 0) return shellIdx;
  return blocks.findIndex((block) =>
    block.block_id === blockId &&
    (block.event_type === "tool_call" || block.event_type === "thinking")
  );
}

export function latestDeliverySummaryFromBlocks(blocks: BlockState[]): DeliverySummary | null {
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    const block = blocks[index];
    if (block?.event_type !== "delivery_summary") continue;
    return parsePersistedDeliverySummary(block.metadata?.summary);
  }
  return null;
}

function lastNonPendingBlock(blocks: BlockState[]): BlockState | null {
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    const block = blocks[index];
    if (block?.event_type !== "pending") return block;
  }
  return null;
}

export function isSameAsLastDeliveryBlock(blocks: BlockState[], summary: DeliverySummary): boolean {
  const lastBlock = lastNonPendingBlock(blocks);
  if (lastBlock?.event_type !== "delivery_summary") return false;
  return deliverySummariesEqual(parsePersistedDeliverySummary(lastBlock.metadata?.summary), summary);
}

function parsePersistedDeliverySummary(value: unknown): DeliverySummary | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const record = value as Partial<Record<keyof DeliverySummary, unknown>>;
  const previewLabel = stringValue(record.preview_label);
  const checkpointLabel = stringValue(record.checkpoint_label);
  const nextAction = stringValue(record.next_action);
  if (!previewLabel || !checkpointLabel || !nextAction) return null;
  return {
    project_path: stringValue(record.project_path),
    preview_label: previewLabel,
    checkpoint_label: checkpointLabel,
    next_action: nextAction,
    verification_label: stringValue(record.verification_label),
    verification_status: stringValue(record.verification_status),
    verification_command: stringValue(record.verification_command),
    record_label: stringValue(record.record_label),
    record_status: stringValue(record.record_status),
    record_target_pages: Array.isArray(record.record_target_pages)
      ? record.record_target_pages.filter((item): item is string => typeof item === "string" && item.trim().length > 0)
      : [],
  };
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

export function eventToBlock(event: StreamEvent): BlockState | null {
  const base = {
    block_id: "block_id" in event ? (event as { block_id: string }).block_id : "",
    isComplete: false,
    metadata: {} as Record<string, unknown>,
  };

  switch (event.event_type) {
    case "user_message":
      return {
        ...base,
        event_type: "user_message",
        content: event.content,
        isComplete: true,
      };
    case "thinking_start":
      return { ...base, event_type: "thinking", content: "", metadata: {} };
    case "text_start":
      return { ...base, event_type: "text", content: "" };
    case "tool_call_start":
      return {
        ...base,
        event_type: "tool_call",
        content: "",
        metadata: {
          tool_name: event.tool_name,
          tool_input: event.tool_input,
        },
      };
    case "tool_call_result":
      return {
        ...base,
        event_type: "tool_call_result",
        content: event.result,
        metadata: {
          is_error: event.is_error,
          duration_ms: event.duration_ms,
        },
      };
    case "diff_view":
      return {
        ...base,
        event_type: "diff_view",
        content: event.new_content,
        metadata: {
          file_path: event.file_path,
          old_content: event.old_content,
        },
      };
    case "shell_start":
      return {
        ...base,
        event_type: "shell",
        content: "",
        metadata: { command: event.command },
      };
    case "confirm_ask":
      return {
        ...base,
        event_type: "confirm_ask",
        content: event.question,
        metadata: {
          kind: event.kind,
          boundary: event.boundary ?? null,
        },
      };
    case "context_compacted":
      return {
        ...base,
        event_type: "context_compacted",
        content: event.summary,
        metadata: {
          retained_messages: event.retained_messages,
          compacted_messages: event.compacted_messages,
          estimated_tokens_before: event.estimated_tokens_before,
          estimated_tokens_after: event.estimated_tokens_after,
        },
        isComplete: true,
      };
    case "context_compact_skipped":
      return {
        ...base,
        event_type: "context_compact_skipped",
        content: compactSkipMessage(event.reason),
        metadata: {
          reason: event.reason,
          retained_messages: event.retained_messages,
        },
        isComplete: true,
      };
    case "delivery_summary":
      return {
        ...base,
        event_type: "delivery_summary",
        content: "本轮交付",
        metadata: {
          summary: event.summary,
        },
        isComplete: true,
      };
    default:
      return null;
  }
}

function compactSkipMessage(reason: string) {
  switch (reason) {
    case "history_too_short":
    case "too_few_messages_to_compact":
      return "当前历史还不够长，暂时无需压缩。";
    case "no_safe_retention_boundary":
      return "当前历史里没有安全的压缩边界，Forge 已保留原上下文。";
    default:
      return "当前上下文暂时无需压缩。";
  }
}

function deliverySummariesEqual(left: DeliverySummary | null, right: DeliverySummary | null) {
  if (!left || !right) return false;
  return (
    (left.project_path ?? null) === (right.project_path ?? null) &&
    left.preview_label === right.preview_label &&
    left.checkpoint_label === right.checkpoint_label &&
    left.next_action === right.next_action &&
    (left.verification_label ?? null) === (right.verification_label ?? null) &&
    (left.verification_status ?? null) === (right.verification_status ?? null) &&
    (left.verification_command ?? null) === (right.verification_command ?? null) &&
    (left.record_label ?? null) === (right.record_label ?? null) &&
    (left.record_status ?? null) === (right.record_status ?? null) &&
    stringArraysEqual(left.record_target_pages ?? [], right.record_target_pages ?? [])
  );
}

function stringArraysEqual(left: string[], right: string[]) {
  if (left.length !== right.length) return false;
  return left.every((value, index) => value === right[index]);
}
