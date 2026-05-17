import { memo, useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { WheelEvent } from "react";
import { ArrowDown } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { ThinkingBlock } from "@/components/messages/ThinkingBlock";
import { TextBlock } from "@/components/messages/TextBlock";
import { ToolCallCard } from "@/components/messages/ToolCallCard";
import { UserMessage } from "@/components/messages/UserMessage";
import { ShellCard } from "@/components/messages/ShellCard";
import { DiffCard } from "@/components/messages/DiffCard";
import { ConfirmCard } from "@/components/messages/ConfirmCard";
import { PendingBlock } from "@/components/messages/PendingBlock";
import { ContextCompactCard } from "@/components/messages/ContextCompactCard";
import { DeliverySummaryCard } from "@/components/messages/DeliverySummaryCard";
import { MissingApiKeyCard } from "@/components/messages/MissingApiKeyCard";
import { StartReadinessCard } from "@/components/session/StartReadinessCard";

interface MessageListProps { blocks: BlockState[]; sessionId?: string }

const BOTTOM_LOCK_THRESHOLD = 96;

export function MessageList({ blocks, sessionId }: MessageListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);
  const scrollRafRef = useRef<number | null>(null);
  const [userScrolledUp, setUserScrolledUp] = useState(false);
  const lastBlock = blocks[blocks.length - 1];

  const setScrolledUpIfChanged = useCallback((next: boolean) => {
    setUserScrolledUp((current) => (current === next ? current : next));
  }, []);

  const updateStickiness = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    const isAtBottom = distanceFromBottom <= BOTTOM_LOCK_THRESHOLD;
    stickToBottomRef.current = isAtBottom;
    setScrolledUpIfChanged(!isAtBottom);
  }, [setScrolledUpIfChanged]);

  useLayoutEffect(() => {
    if (!stickToBottomRef.current) return;
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    setScrolledUpIfChanged(false);
  }, [blocks.length, lastBlock?.content, lastBlock?.isComplete, setScrolledUpIfChanged]);

  useEffect(() => {
    return () => {
      if (scrollRafRef.current !== null) {
        cancelAnimationFrame(scrollRafRef.current);
      }
    };
  }, []);

  const handleScroll = useCallback(() => {
    if (scrollRafRef.current !== null) return;
    scrollRafRef.current = requestAnimationFrame(() => {
      scrollRafRef.current = null;
      updateStickiness();
    });
  }, [updateStickiness]);

  const handleWheel = useCallback((event: WheelEvent<HTMLDivElement>) => {
    if (event.deltaY < 0) {
      stickToBottomRef.current = false;
      setScrolledUpIfChanged(true);
    }
  }, [setScrolledUpIfChanged]);

  const scrollToBottom = () => {
    const el = scrollRef.current;
    if (el) {
      stickToBottomRef.current = true;
      el.scrollTop = el.scrollHeight;
      setScrolledUpIfChanged(false);
    }
  };

  if (blocks.length === 0) {
    return (
      <div data-testid="conversation-scroll" className="forge-conversation-scroll flex-1 min-h-0 overflow-y-auto">
        <div data-testid="message-lane" className="forge-conversation-lane">
          <StartReadinessCard sessionId={sessionId} />
        </div>
      </div>
    );
  }

  return (
    <div className="relative flex-1 min-h-0">
      <div
        data-testid="conversation-scroll"
        ref={scrollRef}
        onScroll={handleScroll}
        onWheel={handleWheel}
        className="forge-conversation-scroll h-full overflow-y-auto"
        style={{
          scrollbarGutter: "stable",
          overflowAnchor: userScrolledUp ? "auto" : "none",
        }}
      >
        <div data-testid="message-lane" className="forge-conversation-lane forge-message-lane flex flex-col">
          {blocks.map((block, i) => (
            <div data-testid="message-block" className="forge-message-block" key={block.block_id || `${block.event_type}-${i}`}>
              <MemoizedBlockRenderer block={block} sessionId={sessionId} />
            </div>
          ))}
        </div>
      </div>
      {userScrolledUp && (
        <button
          type="button"
          data-testid="scroll-to-bottom"
          aria-label="回到底部"
          title="回到底部"
          onClick={scrollToBottom}
          className="forge-scroll-to-bottom forge-control-surface absolute left-1/2 z-10 flex size-7 -translate-x-1/2 items-center justify-center text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
        >
          <ArrowDown className="size-3.5" />
        </button>
      )}
    </div>
  );
}

function BlockRenderer({ block, sessionId }: { block: BlockState; sessionId?: string }) {
  if (block.event_type === "text" && isInternalContextBlock(block.content.trim())) return null;

  switch (block.event_type) {
    case "thinking": return <ThinkingBlock block={block} />;
    case "text": return <TextBlock block={block} sessionId={sessionId} />;
    case "error":
      return block.metadata?.code === "missing_api_key"
        ? <MissingApiKeyCard block={block} />
        : <TextBlock block={block} sessionId={sessionId} />;
    case "tool_call": case "tool_call_result": return <ToolCallCard block={block} />;
    case "user_message": return <UserMessage block={block} />;
    case "shell": return <ShellCard block={block} />;
    case "diff_view": return <DiffCard block={block} />;
    case "confirm_ask": return <ConfirmCard block={block} sessionId={sessionId} />;
    case "context_compacted": return <ContextCompactCard block={block} />;
    case "delivery_summary": return <DeliverySummaryCard block={block} />;
    case "pending": return <PendingBlock />;
    default: return block.content ? <TextBlock block={block} sessionId={sessionId} /> : null;
  }
}

const MemoizedBlockRenderer = memo(BlockRenderer);
MemoizedBlockRenderer.displayName = "MemoizedBlockRenderer";

function isInternalContextBlock(content: string) {
  return (
    content.startsWith("Active Skills:") ||
    content.startsWith("已启用插件:") ||
    content.startsWith("## Active Skills")
  );
}
