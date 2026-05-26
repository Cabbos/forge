import { useCallback, useEffect, useRef, useState } from "react";
import type { WheelEvent } from "react";
import { ArrowDown } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { ToolActivityGroup } from "@/components/messages/ToolActivityGroup";
import { StartReadinessCard } from "@/components/session/StartReadinessCard";
import { MemoizedBlockRenderer } from "@/components/chat/BlockRenderer";
import { groupConversationTurns, groupProcessBlocks } from "@/components/chat/messageGrouping";

interface MessageListProps { blocks: BlockState[]; sessionId?: string }

const BOTTOM_LOCK_THRESHOLD = 96;

export function MessageList({ blocks, sessionId }: MessageListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);
  const scrollRafRef = useRef<number | null>(null);
  const autoScrollRafRef = useRef<number | null>(null);
  const [userScrolledUp, setUserScrolledUp] = useState(false);
  const lastBlock = blocks[blocks.length - 1];
  const messageItems = groupProcessBlocks(blocks);
  const conversationTurns = groupConversationTurns(messageItems);

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

  useEffect(() => {
    if (!stickToBottomRef.current) return;
    if (autoScrollRafRef.current !== null) {
      cancelAnimationFrame(autoScrollRafRef.current);
    }
    autoScrollRafRef.current = requestAnimationFrame(() => {
      autoScrollRafRef.current = null;
      const el = scrollRef.current;
      if (!el) return;
      el.scrollTop = el.scrollHeight;
      setScrolledUpIfChanged(false);
    });
    return () => {
      if (autoScrollRafRef.current !== null) {
        cancelAnimationFrame(autoScrollRafRef.current);
        autoScrollRafRef.current = null;
      }
    };
  }, [blocks.length, lastBlock?.content, lastBlock?.isComplete, setScrolledUpIfChanged]);

  useEffect(() => {
    return () => {
      if (scrollRafRef.current !== null) {
        cancelAnimationFrame(scrollRafRef.current);
      }
      if (autoScrollRafRef.current !== null) {
        cancelAnimationFrame(autoScrollRafRef.current);
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
          {conversationTurns.map((turn) => (
            <section
              key={turn.key}
              data-testid="conversation-turn"
              data-turn-shape={turn.hasEvidence ? "with-evidence" : "direct"}
              data-turn-start={turn.startsWithUser ? "user" : "system"}
              className="forge-conversation-turn"
            >
              {turn.items.map((item) => (
                <div data-testid="message-block" className="forge-message-block" key={item.key}>
                  {item.kind === "tool_group"
                    ? <ToolActivityGroup blocks={item.blocks} />
                    : <MemoizedBlockRenderer block={item.block} sessionId={sessionId} />}
                </div>
              ))}
            </section>
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
          className="forge-scroll-to-bottom forge-control-surface absolute z-10 flex size-7 items-center justify-center text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
        >
          <ArrowDown className="size-3.5" />
        </button>
      )}
    </div>
  );
}
