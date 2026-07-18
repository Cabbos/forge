import type { AgentTurnStatus, BlockState } from "./protocol.ts";

export type ConversationTurnOutcome = "completed" | "stopped" | "failed";

export interface ConversationTurnTiming {
  startedAtMs: number | null;
  terminalAtMs: number | null;
  outcome: ConversationTurnOutcome | null;
  durationMs: number | null;
}

const STARTED_AT_KEY = "turn_started_at_ms";
const TERMINAL_AT_KEY = "turn_terminal_at_ms";
const OUTCOME_KEY = "turn_outcome";

export function startConversationTurnMetadata(now: number): Record<string, unknown> {
  return {
    [STARTED_AT_KEY]: finiteTimestamp(now) ?? 0,
  };
}

export function turnOutcomeForAgentStatus(
  status: AgentTurnStatus,
): ConversationTurnOutcome | null {
  switch (status) {
    case "completed":
      return "completed";
    case "failed":
      return "failed";
    case "cancelled":
      return "stopped";
    default:
      return null;
  }
}

export function markLatestConversationTurnTerminal(
  blocks: BlockState[],
  outcome: ConversationTurnOutcome,
  now: number,
): BlockState[] {
  let latestUserIndex = -1;
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    if (blocks[index].event_type === "user_message") {
      latestUserIndex = index;
      break;
    }
  }
  if (latestUserIndex < 0) return blocks;

  const latestUserBlock = blocks[latestUserIndex];
  if (readConversationTurnTiming(latestUserBlock).outcome !== null) return blocks;

  const startedAtMs = finiteTimestamp(latestUserBlock.metadata[STARTED_AT_KEY]);
  const observedAtMs = finiteTimestamp(now);
  const terminalAtMs = observedAtMs === null
    ? startedAtMs
    : startedAtMs === null
      ? observedAtMs
      : Math.max(startedAtMs, observedAtMs);
  const nextBlocks = [...blocks];
  nextBlocks[latestUserIndex] = {
    ...latestUserBlock,
    metadata: {
      ...latestUserBlock.metadata,
      [TERMINAL_AT_KEY]: terminalAtMs,
      [OUTCOME_KEY]: outcome,
    },
  };
  return nextBlocks;
}

export function readConversationTurnTiming(
  block: BlockState | null,
): ConversationTurnTiming {
  const startedAtMs = finiteTimestamp(block?.metadata[STARTED_AT_KEY]);
  const terminalAtMs = finiteTimestamp(block?.metadata[TERMINAL_AT_KEY]);
  const outcome = conversationTurnOutcome(block?.metadata[OUTCOME_KEY]);
  return {
    startedAtMs,
    terminalAtMs,
    outcome,
    durationMs: startedAtMs === null || terminalAtMs === null
      ? null
      : Math.max(0, terminalAtMs - startedAtMs),
  };
}

function finiteTimestamp(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 0
    ? value
    : null;
}

function conversationTurnOutcome(value: unknown): ConversationTurnOutcome | null {
  return value === "completed" || value === "stopped" || value === "failed"
    ? value
    : null;
}
