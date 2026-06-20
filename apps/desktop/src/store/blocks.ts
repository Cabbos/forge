import type { BlockState, DeliverySummary, StreamEvent } from "../lib/protocol";

export const SESSION_RESTORED_TOOL_INTERRUPTION_MESSAGE =
  "Tool call interrupted by session restore before it returned.";

const SESSION_RESTORED_TOOL_INTERRUPTION_REASON = "session_restored";

export function transcriptEventsToBlocks(events: StreamEvent[]): BlockState[] {
  let blocks: BlockState[] = [];
  for (const event of events) {
    blocks = applyTranscriptEventToBlocks(blocks, event);
  }
  return blocks.filter((block) => block.event_type !== "pending");
}

export function closeInterruptedConfirmBlocks(
  blocks: BlockState[],
  reason: "session_stopped" | "session_restored",
): BlockState[] {
  return blocks.map((block) => {
    if (block.event_type !== "confirm_ask") return block;
    if (block.metadata.confirmed === true || block.metadata.confirm_interrupted === true) {
      return block;
    }
    return {
      ...block,
      isComplete: true,
      metadata: {
        ...block.metadata,
        confirmed: true,
        answer: null,
        confirm_interrupted: true,
        confirm_interrupted_reason: reason,
      },
    };
  });
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

  if (event_type === "file_io") {
    return applyFileIoToBlocks(blocks, event);
  }

  if (event_type === "context_compacted" || event_type === "context_compact_skipped") {
    return applyCompactResultToBlocks(blocks, event);
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
          ...interruptedToolResultMetadata(event.result, event.is_error),
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
          ...interruptedToolResultMetadata(event.result, event.is_error),
        },
      },
    ];
  }

  // Phase 1.6: dedupe tool_call_start — if a block with the same block_id
  // already exists (e.g. from a prior transcript load), update its metadata
  // instead of appending a duplicate. This keeps the block list clean when
  // startup replays active tool-call descriptors that were already in the
  // transcript.
  if (event_type === "tool_call_start") {
    const next = [...blocks];
    const existingIdx = next.findIndex((block) => block.block_id === event.block_id);
    if (existingIdx >= 0) {
      next[existingIdx] = {
        ...next[existingIdx],
        event_type: "tool_call",
        metadata: {
          ...next[existingIdx].metadata,
          tool_name: event.tool_name,
          tool_input: event.tool_input,
        },
      };
      return next;
    }
    const block = eventToBlock(event);
    return block ? [...next, block] : next;
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

  // Phase 1.5: dedupe replayed confirm_ask — if a confirm_ask block with the
  // same block_id already exists (e.g. from a previous transcript replay or
  // live session), replace it instead of appending a duplicate. This keeps the
  // block list clean when startup restores a session that already had its
  // transcript loaded.
  if (event_type === "confirm_ask" && (event as { replayed_interrupted?: boolean }).replayed_interrupted) {
    const next = [...blocks];
    const existingIdx = next.findIndex((block) => block.block_id === event.block_id);
    const block = eventToBlock(event);
    if (block) {
      if (existingIdx >= 0) {
        next[existingIdx] = block;
      } else {
        next.push(block);
      }
    }
    return next;
  }

  const block = eventToBlock(event);
  return block ? [...blocks, block] : blocks;
}

export function applyCompactResultToBlocks(
  blocks: BlockState[],
  event: Extract<StreamEvent, { event_type: "context_compacted" }> | Extract<StreamEvent, { event_type: "context_compact_skipped" }>,
): BlockState[] {
  const next = [...blocks];
  const existingIdx = next.findIndex((block) =>
    block.block_id === event.block_id && block.event_type === "context_compact_start"
  );
  if (existingIdx >= 0) {
    if (event.event_type === "context_compacted") {
      next[existingIdx] = {
        ...next[existingIdx],
        event_type: "context_compacted",
        content: event.summary,
        isComplete: true,
        metadata: {
          retained_messages: event.retained_messages,
          compacted_messages: event.compacted_messages,
          estimated_tokens_before: event.estimated_tokens_before,
          estimated_tokens_after: event.estimated_tokens_after,
        },
      };
    } else {
      next[existingIdx] = {
        ...next[existingIdx],
        event_type: "context_compact_skipped",
        content: compactSkipMessage(event.reason),
        isComplete: true,
        metadata: {
          reason: event.reason,
          retained_messages: event.retained_messages,
        },
      };
    }
    return next;
  }
  const block = eventToBlock(event);
  return block ? [...next, block] : next;
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

export function applyFileIoToBlocks(
  blocks: BlockState[],
  event: Extract<StreamEvent, { event_type: "file_io" }>,
): BlockState[] {
  const next = [...blocks];
  const existingIdx = next.findIndex((block) =>
    block.block_id === event.block_id &&
    (block.event_type === "tool_call" ||
      block.event_type === "tool_call_result" ||
      block.event_type === "shell")
  );
  if (existingIdx < 0) return blocks;

  const existingEvents = Array.isArray(next[existingIdx].metadata.file_io_events)
    ? next[existingIdx].metadata.file_io_events
    : [];
  next[existingIdx] = {
    ...next[existingIdx],
    metadata: {
      ...next[existingIdx].metadata,
      file_io_events: [
        ...existingEvents,
        {
          path: event.path,
          operation: event.operation,
          ...(event.source ? { source: event.source } : {}),
        },
      ],
    },
  };
  return next;
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
          ...interruptedToolResultMetadata(event.result, event.is_error),
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
      if (event.replayed_interrupted) {
        return {
          ...base,
          event_type: "confirm_ask",
          content: event.question,
          isComplete: true,
          metadata: {
            kind: event.kind,
            boundary: event.boundary ?? null,
            confirmed: true,
            answer: null,
            confirm_interrupted: true,
            confirm_interrupted_reason: "session_restored",
          },
        };
      }
      return {
        ...base,
        event_type: "confirm_ask",
        content: event.question,
        metadata: {
          kind: event.kind,
          boundary: event.boundary ?? null,
        },
      };
    case "context_compact_start":
      return {
        ...base,
        event_type: "context_compact_start",
        content: "",
        metadata: {},
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
    case "provider_usage": {
      const providerId = event.provider_id?.trim() || null;
      const model = event.model?.trim() || null;
      const source = event.source?.trim() || null;
      const metadata = {
        provider_id: providerId,
        model,
        source,
        reason: event.reason,
        input_tokens: event.input_tokens,
        output_tokens: event.output_tokens,
        cache_read_tokens: event.cache_read_tokens ?? null,
        cache_creation_tokens: event.cache_creation_tokens ?? null,
        reasoning_tokens: event.reasoning_tokens ?? null,
        estimated_cost_micros: event.estimated_cost_micros,
        pricing_source: event.pricing_source ?? null,
        input_tokens_unknown: !isFiniteNumber(event.input_tokens),
        output_tokens_unknown: !isFiniteNumber(event.output_tokens),
        cost_unknown: !isFiniteNumber(event.estimated_cost_micros),
      };
      return {
        ...base,
        block_id: event.block_id?.trim() || providerUsageBlockId(event),
        event_type: "provider_usage",
        content: providerUsageContent({
          providerId,
          model,
          source,
          reason: event.reason,
          inputTokens: event.input_tokens,
          outputTokens: event.output_tokens,
          estimatedCostMicros: event.estimated_cost_micros,
        }),
        metadata,
        isComplete: true,
      };
    }
    default:
      return null;
  }
}

export function interruptedToolResultMetadata(result: string, isError: boolean): Record<string, unknown> {
  if (!isError || result !== SESSION_RESTORED_TOOL_INTERRUPTION_MESSAGE) {
    return {};
  }
  return {
    tool_interrupted: true,
    tool_interrupted_reason: SESSION_RESTORED_TOOL_INTERRUPTION_REASON,
  };
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

function providerUsageBlockId(event: Extract<StreamEvent, { event_type: "provider_usage" }>) {
  return [
    "provider_usage",
    event.session_id,
    event.provider_id?.trim() || "unknown-provider",
    event.model?.trim() || "unknown-model",
    event.source?.trim() || "unknown-source",
    event.reason,
    numberOrUnknown(event.input_tokens),
    numberOrUnknown(event.output_tokens),
    costOrUnknown(event.estimated_cost_micros),
  ].join(":");
}

function providerUsageContent({
  model,
  providerId,
  source,
  reason,
  inputTokens,
  outputTokens,
  estimatedCostMicros,
}: {
  providerId: string | null;
  model: string | null;
  source: string | null;
  reason: string;
  inputTokens: number | null;
  outputTokens: number | null;
  estimatedCostMicros: number | null;
}) {
  const parts = [
    `provider ${providerId || "unknown provider"}`,
    `model ${model || "unknown model"}`,
    `input ${numberOrUnknown(inputTokens)}`,
    `output ${numberOrUnknown(outputTokens)}`,
    `cost ${costOrUnknown(estimatedCostMicros)}`,
  ];
  if (source) parts.push(`source ${source}`);
  const reasonLabel = providerUsageReasonLabel(reason);
  if (reasonLabel) parts.push(reasonLabel);
  return `模型用量 · ${parts.join(" / ")}`;
}

function providerUsageReasonLabel(reason: string): string | null {
  if (reason === "provider_omitted") return "provider omitted";
  if (reason === "pricing_unknown") return "pricing unknown";
  return null;
}

function numberOrUnknown(value: number | null | undefined): string {
  return isFiniteNumber(value) ? String(value) : "unknown";
}

function costOrUnknown(value: number | null | undefined): string {
  return isFiniteNumber(value) ? `${value} micros` : "unknown";
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
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
