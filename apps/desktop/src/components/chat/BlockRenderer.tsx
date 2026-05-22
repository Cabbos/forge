import { memo } from "react";
import type { BlockState } from "@/lib/protocol";
import { ConfirmCard } from "@/components/messages/ConfirmCard";
import { ContextCompactCard } from "@/components/messages/ContextCompactCard";
import { DeliverySummaryCard } from "@/components/messages/DeliverySummaryCard";
import { DiffCard } from "@/components/messages/DiffCard";
import { ErrorCard } from "@/components/messages/ErrorCard";
import { MissingApiKeyCard } from "@/components/messages/MissingApiKeyCard";
import { PendingBlock } from "@/components/messages/PendingBlock";
import { ShellCard } from "@/components/messages/ShellCard";
import { TextBlock } from "@/components/messages/TextBlock";
import { ThinkingBlock } from "@/components/messages/ThinkingBlock";
import { ToolCallCard } from "@/components/messages/ToolCallCard";
import { UserMessage } from "@/components/messages/UserMessage";
import { isInternalContextBlock } from "@/components/chat/messageGrouping";

function BlockRenderer({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  if (block.event_type === "text" && isInternalContextBlock(block.content.trim())) return null;

  switch (block.event_type) {
    case "thinking": return <ThinkingBlock block={block} />;
    case "text": return <TextBlock block={block} sessionId={sessionId} />;
    case "error":
      return block.metadata?.code === "missing_api_key"
        ? <MissingApiKeyCard block={block} />
        : <ErrorCard block={block} />;
    case "tool_call": case "tool_call_result": return <ToolCallCard block={block} />;
    case "user_message": return <UserMessage block={block} />;
    case "shell": return <ShellCard block={block} />;
    case "diff_view": return <DiffCard block={block} sessionId={sessionId} />;
    case "confirm_ask": return <ConfirmCard block={block} sessionId={sessionId} />;
    case "context_compacted": return <ContextCompactCard block={block} />;
    case "delivery_summary": return <DeliverySummaryCard block={block} sessionId={sessionId} />;
    case "pending": return <PendingBlock />;
    default: return block.content ? <TextBlock block={block} sessionId={sessionId} /> : null;
  }
}

export const MemoizedBlockRenderer = memo(BlockRenderer);
MemoizedBlockRenderer.displayName = "MemoizedBlockRenderer";
