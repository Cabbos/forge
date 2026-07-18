import type { BlockState } from "../../lib/protocol.ts";
import {
  readConversationTurnTiming,
  type ConversationTurnOutcome,
  type ConversationTurnTiming,
} from "../../lib/conversationTurnTiming.ts";
import type { ConversationTurn, MessageItem } from "./messageGrouping.ts";
import {
  deriveLiveProgressCandidate,
  progressCandidateForBlock,
  waitingProgressCandidate,
  type LiveProgressCandidate,
} from "./conversationProgress.ts";

export {
  deriveLiveProgressCandidate,
  type LiveProgressCandidate,
} from "./conversationProgress.ts";

export type ProcessDigestKind = "analysis" | "modification" | "verification" | "exception";

type ProcessDigestOutcome = "running" | "done" | "stopped" | "failed";

export interface ProcessDigestItem {
  id: string;
  kind: ProcessDigestKind;
  label: string;
  outcome: ProcessDigestOutcome;
  evidence: BlockState[];
}

export interface ProcessDigest {
  items: ProcessDigestItem[];
  operationCount: number;
  usage: BlockState[];
  delivery: BlockState | null;
}

export interface TurnTerminalSummary {
  outcome: ConversationTurnOutcome;
  durationMs: number | null;
  operationCount: number;
}

export interface ConversationTurnView {
  key: string;
  userMessage: BlockState | null;
  finalAnswer: BlockState | null;
  terminalError: BlockState | null;
  interruptions: BlockState[];
  liveProgress: LiveProgressCandidate | null;
  terminalSummary: TurnTerminalSummary | null;
  processDigest: ProcessDigest;
}

interface DigestGroup {
  id: string;
  kind: ProcessDigestKind;
  label: string;
  outcome: ProcessDigestOutcome;
  evidence: BlockState[];
  operation: boolean;
  operationKey: string | null;
}

interface DigestClassification {
  kind: ProcessDigestKind;
  operation: boolean;
}

export function deriveConversationTurnView(turn: ConversationTurn): ConversationTurnView {
  const blocks = flattenMessageItems(turn.items);
  const userMessage = blocks.find((block) => block.event_type === "user_message") ?? null;
  const finalAnswer = findLast(
    blocks,
    (block) => block.event_type === "text"
      && Boolean(block.content.trim())
      && !isInternalContextContent(block.content.trim()),
  );
  const errors = blocks.filter((block) => block.event_type === "error");
  const terminalError = finalAnswer ? null : errors[errors.length - 1] ?? null;
  const interruptions = blocks.filter(isUnresolvedInterruption);
  const timing = readConversationTurnTiming(userMessage);
  const provisionalCompletion = isProvisionalCompletion(blocks, finalAnswer);
  const effectiveTerminalOutcome = timing.outcome
    ?? (provisionalCompletion ? "completed" : null);
  const processDigest = deriveProcessDigest(blocks, effectiveTerminalOutcome);
  const terminalSummary = deriveTerminalSummary(
    timing,
    provisionalCompletion,
    processDigest,
  );

  return {
    key: turn.key,
    userMessage,
    finalAnswer,
    terminalError,
    interruptions,
    liveProgress: terminalSummary
      ? null
      : interruptions.length > 0
        ? waitingProgressCandidate()
        : deriveLiveProgressCandidate(blocks),
    terminalSummary,
    processDigest,
  };
}

function flattenMessageItems(items: MessageItem[]) {
  return items.flatMap((item) => item.kind === "block" ? [item.block] : item.blocks);
}

function isUnresolvedInterruption(block: BlockState) {
  if (block.event_type !== "confirm_ask") return false;
  return block.metadata.confirmed !== true && block.metadata.confirm_interrupted !== true;
}

function deriveProcessDigest(
  blocks: BlockState[],
  terminalOutcome: ConversationTurnOutcome | null,
): ProcessDigest {
  const toolNames = buildToolNameIndex(blocks);
  const groups: DigestGroup[] = [];
  const usage: BlockState[] = [];
  let delivery: BlockState | null = null;

  for (const block of blocks) {
    if (block.event_type === "provider_usage") {
      usage.push(block);
      continue;
    }
    if (block.event_type === "delivery_summary") {
      delivery = block;
      continue;
    }

    const group = digestGroupForBlock(block, toolNames);
    if (group) groups.push(group);
  }

  const normalizedGroups = mergeDigestGroups(groups);
  const operationCount = normalizedGroups.filter((group) => group.operation).length;
  const terminalizedGroups = normalizedGroups.flatMap((group) => {
    const outcome = terminalizedOutcome(group, terminalOutcome);
    return outcome === null ? [] : [{ ...group, outcome }];
  });

  return {
    items: compactDigestGroups(terminalizedGroups).map(itemFromGroup),
    operationCount,
    usage,
    delivery,
  };
}

function digestGroupForBlock(
  block: BlockState,
  toolNames: Map<string, string>,
): DigestGroup | null {
  switch (block.event_type) {
    case "thinking":
    case "pending":
    case "context_compact_start":
    case "context_compacted":
    case "context_compact_skipped":
      return digestGroup(block, { kind: "analysis", operation: false });
    case "tool_call":
    case "tool_call_result":
    case "shell":
    case "diff_view":
    case "error":
      return digestGroup(block, classificationForBlock(block, toolNames));
    default:
      return null;
  }
}

function digestGroup(
  block: BlockState,
  classification: DigestClassification,
): DigestGroup {
  const operationKey = block.event_type === "tool_call" || block.event_type === "tool_call_result"
    ? `tool-${block.block_id}`
    : null;
  return {
    id: `${classification.kind}-${block.block_id}`,
    kind: classification.kind,
    label: labelForKind(classification.kind),
    outcome: blockOutcome(block),
    evidence: [block],
    operation: classification.operation,
    operationKey,
  };
}

function classificationForBlock(
  block: BlockState,
  toolNames: Map<string, string>,
): DigestClassification {
  if (block.event_type === "error") {
    if (typeof block.metadata.tool_name !== "string" && typeof block.metadata.command !== "string") {
      return { kind: "exception", operation: false };
    }
  }

  if (block.event_type === "diff_view") {
    return classificationForDiffBlock(block, toolNames);
  }

  let projection = block;
  if (block.event_type === "tool_call_result" || block.event_type === "error") {
    const toolName = toolNameForBlock(block, toolNames);
    if (toolName) {
      projection = {
        ...block,
        event_type: "tool_call",
        metadata: { ...block.metadata, tool_name: toolName },
      };
    } else if (typeof block.metadata.command === "string") {
      projection = { ...block, event_type: "shell" };
    }
  }

  const stage = progressCandidateForBlock(projection).id;
  if (stage === "discovering") return { kind: "analysis", operation: true };
  if (stage === "modifying") return { kind: "modification", operation: true };
  if (stage === "verifying") return { kind: "verification", operation: true };
  return {
    kind: block.event_type === "error" ? "exception" : "analysis",
    operation: false,
  };
}

function classificationForDiffBlock(
  block: BlockState,
  toolNames: Map<string, string>,
): DigestClassification {
  const toolName = toolNameForBlock(block, toolNames);
  if (!toolName) return { kind: "analysis", operation: true };

  const stage = progressCandidateForBlock({
    ...block,
    event_type: "tool_call",
    metadata: { ...block.metadata, tool_name: toolName },
  }).id;

  if (stage === "modifying") {
    return { kind: "modification", operation: false };
  }
  if (stage === "discovering") {
    return { kind: "analysis", operation: false };
  }
  return { kind: "analysis", operation: true };
}

function buildToolNameIndex(blocks: BlockState[]) {
  const names = new Map<string, string>();
  for (const block of blocks) {
    if (
      block.event_type === "tool_call"
      && block.block_id
      && typeof block.metadata.tool_name === "string"
    ) {
      names.set(block.block_id, block.metadata.tool_name);
    }
  }

  for (let index = 1; index < blocks.length; index += 1) {
    const block = blocks[index];
    if (block.event_type !== "diff_view" || names.has(block.block_id)) continue;

    const previous = blocks[index - 1];
    if (previous.event_type !== "tool_call" && previous.event_type !== "tool_call_result") continue;
    const toolName = toolNameForBlock(previous, names);
    if (!toolName) continue;
    const projection = previous.event_type === "tool_call"
      ? previous
      : {
          ...previous,
          event_type: "tool_call",
          metadata: { ...previous.metadata, tool_name: toolName },
        };
    if (progressCandidateForBlock(projection).id === "modifying") {
      names.set(block.block_id, toolName);
    }
  }
  return names;
}

function toolNameForBlock(block: BlockState, toolNames: Map<string, string>) {
  return typeof block.metadata.tool_name === "string"
    ? block.metadata.tool_name
    : toolNames.get(block.block_id) ?? null;
}

function mergeDigestGroups(groups: DigestGroup[]): DigestGroup[] {
  const merged: DigestGroup[] = [];
  for (const group of groups) {
    const previous = merged[merged.length - 1];
    if (previous?.operationKey && previous.operationKey === group.operationKey) {
      merged[merged.length - 1] = mergeSameOperation(previous, group);
      continue;
    }
    if (previous && previous.kind === group.kind && previous.outcome === group.outcome) {
      merged[merged.length - 1] = {
        ...previous,
        evidence: [...previous.evidence, ...group.evidence],
        operation: previous.operation || group.operation,
      };
      continue;
    }
    merged.push(group);
  }
  return merged;
}

function mergeSameOperation(previous: DigestGroup, current: DigestGroup): DigestGroup {
  const outcome = previous.outcome === "failed" || current.outcome === "failed"
    ? "failed"
    : current.outcome;
  return {
    ...previous,
    kind: current.kind,
    label: labelForKind(current.kind),
    outcome,
    evidence: [...previous.evidence, ...current.evidence],
    operation: previous.operation || current.operation,
  };
}

function compactDigestGroups(groups: DigestGroup[]): DigestGroup[] {
  const compacted: DigestGroup[] = [];
  const indexByKind = new Map<ProcessDigestKind, number>();

  for (const group of groups) {
    const existingIndex = indexByKind.get(group.kind);
    if (existingIndex === undefined) {
      indexByKind.set(group.kind, compacted.length);
      compacted.push(group);
      continue;
    }

    const existing = compacted[existingIndex];
    compacted[existingIndex] = {
      ...existing,
      outcome: strongerOutcome(existing.outcome, group.outcome),
      evidence: [...existing.evidence, ...group.evidence],
      operation: existing.operation || group.operation,
    };
  }

  return compacted.slice(0, 4);
}

function itemFromGroup(group: DigestGroup): ProcessDigestItem {
  return {
    id: group.id,
    kind: group.kind,
    label: group.label,
    outcome: group.outcome,
    evidence: group.evidence,
  };
}

function terminalizedOutcome(
  group: DigestGroup,
  terminalOutcome: ConversationTurnOutcome | null,
): ProcessDigestOutcome | null {
  const { outcome } = group;
  if (outcome !== "running" || terminalOutcome === null) return outcome;
  if (terminalOutcome === "completed") {
    return isSafelyCompletableDiffGroup(group) ? "done" : null;
  }
  return terminalOutcome === "stopped" ? "stopped" : "failed";
}

function isSafelyCompletableDiffGroup(group: DigestGroup) {
  return group.evidence.length === 1
    && group.evidence[0].event_type === "diff_view";
}

function strongerOutcome(
  left: ProcessDigestOutcome,
  right: ProcessDigestOutcome,
): ProcessDigestOutcome {
  const strength: Record<ProcessDigestOutcome, number> = {
    done: 0,
    running: 1,
    stopped: 2,
    failed: 3,
  };
  return strength[right] > strength[left] ? right : left;
}

function deriveTerminalSummary(
  timing: ConversationTurnTiming,
  provisionalCompletion: boolean,
  digest: ProcessDigest,
): TurnTerminalSummary | null {
  if (timing.outcome) {
    return {
      outcome: timing.outcome,
      durationMs: timing.durationMs,
      operationCount: digest.operationCount,
    };
  }

  if (!provisionalCompletion) return null;

  return {
    outcome: "completed",
    durationMs: null,
    operationCount: digest.operationCount,
  };
}

function isProvisionalCompletion(
  blocks: BlockState[],
  finalAnswer: BlockState | null,
) {
  if (!finalAnswer?.isComplete) return false;
  return !blocks.some(isUnresolvedInterruption)
    && !hasProcessActivityAfter(blocks, finalAnswer);
}

function hasProcessActivityAfter(blocks: BlockState[], finalAnswer: BlockState) {
  const answerIndex = blocks.lastIndexOf(finalAnswer);
  return answerIndex >= 0 && blocks.slice(answerIndex + 1).some(isProcessActivity);
}

function isProcessActivity(block: BlockState) {
  return block.event_type === "thinking"
    || block.event_type === "pending"
    || block.event_type === "tool_call"
    || block.event_type === "tool_call_result"
    || block.event_type === "shell"
    || block.event_type === "diff_view"
    || block.event_type === "confirm_ask"
    || block.event_type === "error"
    || block.event_type === "context_compact_start"
    || block.event_type === "context_compacted"
    || block.event_type === "context_compact_skipped";
}

function labelForKind(kind: ProcessDigestKind) {
  switch (kind) {
    case "analysis":
      return "分析需求";
    case "modification":
      return "完成修改";
    case "verification":
      return "验证结果";
    case "exception":
      return "处理异常";
  }
}

function isInternalContextContent(content: string) {
  return content.startsWith("Active Skills:")
    || content.startsWith("已启用插件:")
    || content.startsWith("## Active Skills");
}

function blockOutcome(block: BlockState): ProcessDigestOutcome {
  return blockFailed(block) ? "failed" : block.isComplete ? "done" : "running";
}

function blockFailed(block: BlockState) {
  if (block.event_type === "error" || block.metadata.is_error === true) return true;
  if (block.metadata.success === false) return true;
  if (block.metadata.status === "failed" || block.metadata.status === "error") return true;
  const exitCode = block.metadata.exit_code;
  if (typeof exitCode === "number" && Number.isFinite(exitCode) && exitCode !== 0) return true;
  const error = block.metadata.error;
  return error !== undefined && error !== null && error !== false && error !== "";
}

function findLast<T>(values: T[], predicate: (value: T) => boolean): T | null {
  for (let index = values.length - 1; index >= 0; index -= 1) {
    if (predicate(values[index])) return values[index];
  }
  return null;
}
