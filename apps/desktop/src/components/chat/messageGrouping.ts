import type { BlockState } from "@/lib/protocol";

export { deriveConversationTurnView } from "./conversationTurnView.ts";
export type {
  ConversationTurnView,
  LiveProgressCandidate,
  ProcessDigest,
  ProcessDigestItem,
  ProcessDigestKind,
} from "./conversationTurnView.ts";

export type MessageItem =
  | { kind: "block"; block: BlockState; key: string }
  | { kind: "tool_group"; blocks: BlockState[]; key: string };

export interface ConversationTurn {
  key: string;
  items: MessageItem[];
  hasEvidence: boolean;
  startsWithUser: boolean;
}

export function groupProcessBlocks(blocks: BlockState[]): MessageItem[] {
  const items: MessageItem[] = [];
  let i = 0;

  while (i < blocks.length) {
    const block = blocks[i];
    if (!isProcessEvidenceBlock(block)) {
      items.push({ kind: "block", block, key: block.block_id || `${block.event_type}-${i}` });
      i += 1;
      continue;
    }

    const group: BlockState[] = [];
    while (i < blocks.length && isProcessEvidenceBlock(blocks[i])) {
      group.push(blocks[i]);
      i += 1;
    }

    if (group.length >= 2) {
      items.push({ kind: "tool_group", blocks: group, key: `tool-group-${group[0].block_id}` });
    } else {
      const single = group[0];
      items.push({ kind: "block", block: single, key: single.block_id || `${single.event_type}-${i}` });
    }
  }

  return items;
}

export function groupConversationTurns(items: MessageItem[]): ConversationTurn[] {
  const turns: ConversationTurn[] = [];
  let current: ConversationTurn | null = null;

  for (const item of items) {
    const startsTurn = isUserMessageItem(item) || current === null;
    if (startsTurn) {
      if (current && current.items.length > 0) turns.push(current);
      current = {
        key: `turn-${item.key}`,
        items: [],
        hasEvidence: false,
        startsWithUser: isUserMessageItem(item),
      };
    }

    const activeTurn = current!;
    activeTurn.items.push(item);
    activeTurn.hasEvidence = activeTurn.hasEvidence || isEvidenceItem(item);
  }

  if (current && current.items.length > 0) turns.push(current);
  return turns;
}

export function isInternalContextBlock(content: string) {
  return (
    content.startsWith("Active Skills:") ||
    content.startsWith("已启用插件:") ||
    content.startsWith("## Active Skills")
  );
}

function isUserMessageItem(item: MessageItem) {
  return item.kind === "block" && item.block.event_type === "user_message";
}

function isEvidenceItem(item: MessageItem) {
  if (item.kind === "tool_group") return true;
  return (
    item.block.event_type === "tool_call" ||
    item.block.event_type === "tool_call_result" ||
    item.block.event_type === "shell" ||
    item.block.event_type === "provider_usage" ||
    item.block.event_type === "diff_view"
  );
}

function isProcessEvidenceBlock(block: BlockState) {
  return block.event_type === "tool_call" || block.event_type === "tool_call_result" || block.event_type === "shell";
}
