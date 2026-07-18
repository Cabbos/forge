import type { BlockState } from "../../lib/protocol.ts";
import type { ConversationTurn, MessageItem } from "./messageGrouping.ts";

export type ProcessDigestKind = "understanding" | "operation" | "verification" | "exception";

export interface ProcessDigestItem {
  id: string;
  kind: ProcessDigestKind;
  label: string;
  outcome: "running" | "done" | "failed";
  durationMs: number | null;
  evidence: BlockState[];
}

export interface ProcessDigest {
  items: ProcessDigestItem[];
  operationCount: number;
  usage: BlockState[];
  delivery: BlockState | null;
}

export interface LiveProgressCandidate {
  id: string;
  label: string;
}

export interface ConversationTurnView {
  key: string;
  userMessage: BlockState | null;
  finalAnswer: BlockState | null;
  terminalError: BlockState | null;
  interruptions: BlockState[];
  liveProgress: LiveProgressCandidate | null;
  processDigest: ProcessDigest;
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

  return {
    key: turn.key,
    userMessage,
    finalAnswer,
    terminalError,
    interruptions,
    liveProgress: finalAnswer || terminalError ? null : deriveInitialLiveProgress(blocks),
    processDigest: deriveProcessDigest(blocks, finalAnswer, terminalError),
  };
}

function flattenMessageItems(items: MessageItem[]) {
  return items.flatMap((item) => item.kind === "block" ? [item.block] : item.blocks);
}

function isUnresolvedInterruption(block: BlockState) {
  if (block.event_type !== "confirm_ask") return false;
  return block.metadata.confirmed !== true && block.metadata.confirm_interrupted !== true;
}

function deriveInitialLiveProgress(blocks: BlockState[]): LiveProgressCandidate | null {
  const running = findLast(blocks, (block) => !block.isComplete && isProgressBlock(block));
  if (!running) return null;
  if (running.event_type === "thinking" || running.event_type === "pending") {
    return { id: "understanding", label: "正在理解任务" };
  }
  return { id: running.block_id || running.event_type, label: "正在执行操作" };
}

function deriveProcessDigest(
  blocks: BlockState[],
  finalAnswer: BlockState | null,
  terminalError: BlockState | null,
): ProcessDigest {
  const items: ProcessDigestItem[] = [];
  const groupedOperations = new Map<string, ProcessDigestItem>();
  let hasUnderstanding = false;
  let delivery: BlockState | null = null;
  const usage: BlockState[] = [];

  for (const block of blocks) {
    switch (block.event_type) {
      case "thinking":
      case "pending":
        if (!hasUnderstanding) {
          items.push(digestItem(block, "understanding", "已理解任务"));
          hasUnderstanding = true;
        }
        break;
      case "tool_call":
      case "tool_call_result": {
        const groupId = `tool-${block.block_id || items.length}`;
        const existing = groupedOperations.get(groupId);
        if (existing) {
          existing.evidence.push(block);
          existing.outcome = block.metadata.is_error === true ? "failed" : block.isComplete ? "done" : "running";
          existing.durationMs = finiteNumber(block.metadata.duration_ms) ?? existing.durationMs;
        } else {
          const item = digestItem(block, "operation", "执行操作", groupId);
          items.push(item);
          groupedOperations.set(groupId, item);
        }
        break;
      }
      case "shell":
        items.push(digestItem(
          block,
          isVerificationCommand(block.metadata.command) ? "verification" : "operation",
          isVerificationCommand(block.metadata.command) ? "已验证结果" : "已执行命令",
        ));
        break;
      case "diff_view":
        items.push(digestItem(block, "operation", "已更新文件"));
        break;
      case "confirm_ask":
        if (!isUnresolvedInterruption(block)) items.push(digestItem(block, "exception", "已处理确认"));
        break;
      case "error":
        if (finalAnswer || block !== terminalError) items.push(digestItem(block, "exception", "已处理异常"));
        break;
      case "context_compact_start":
      case "context_compacted":
      case "context_compact_skipped":
        items.push(digestItem(block, "exception", "已整理上下文"));
        break;
      case "provider_usage":
        usage.push(block);
        break;
      case "delivery_summary":
        delivery = block;
        break;
    }
  }

  return {
    items,
    operationCount: items.filter((item) => item.kind !== "understanding").length,
    usage,
    delivery,
  };
}

function digestItem(
  block: BlockState,
  kind: ProcessDigestKind,
  label: string,
  id = `${kind}-${block.block_id}`,
): ProcessDigestItem {
  return {
    id,
    kind,
    label,
    outcome: blockFailed(block) ? "failed" : block.isComplete ? "done" : "running",
    durationMs: finiteNumber(block.metadata.duration_ms),
    evidence: [block],
  };
}

function isInternalContextContent(content: string) {
  return content.startsWith("Active Skills:")
    || content.startsWith("已启用插件:")
    || content.startsWith("## Active Skills");
}

function isProgressBlock(block: BlockState) {
  return block.event_type === "thinking"
    || block.event_type === "pending"
    || block.event_type === "tool_call"
    || block.event_type === "shell";
}

function isVerificationCommand(value: unknown) {
  return typeof value === "string" && /(?:^|\s|:)(build|test|check|lint|typecheck)(?:\s|$|:)/i.test(value);
}

function blockFailed(block: BlockState) {
  if (block.metadata.is_error === true) return true;
  const exitCode = finiteNumber(block.metadata.exit_code);
  return exitCode !== null && exitCode !== 0;
}

function finiteNumber(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function findLast<T>(values: T[], predicate: (value: T) => boolean): T | null {
  for (let index = values.length - 1; index >= 0; index -= 1) {
    if (predicate(values[index])) return values[index];
  }
  return null;
}
