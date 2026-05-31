import { useCallback, useEffect, useRef, useState } from "react";
import type { WheelEvent } from "react";
import { ArrowDown } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { ForgeControlButton } from "@/components/primitives/control-button";
import { ToolActivityGroup } from "@/components/messages/ToolActivityGroup";
import { StartReadinessCard } from "@/components/session/StartReadinessCard";
import { MemoizedBlockRenderer } from "@/components/chat/BlockRenderer";
import { groupConversationTurns, groupProcessBlocks } from "@/components/chat/messageGrouping";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

interface MessageListProps { blocks: BlockState[]; sessionId?: string }

const BOTTOM_LOCK_THRESHOLD = 96;

export function MessageList({ blocks, sessionId }: MessageListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const laneRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);
  const scrollRafRef = useRef<number | null>(null);
  const autoScrollRafRef = useRef<number | null>(null);
  const [userScrolledUp, setUserScrolledUp] = useState(false);
  const lastBlock = blocks[blocks.length - 1];
  const messageItems = groupProcessBlocks(blocks);
  const conversationTurns = groupConversationTurns(messageItems);

  useGSAP(() => {
    if (prefersReducedMotion()) return;

    const lane = laneRef.current;
    if (!lane) return;

    const messageBlocks = gsap.utils.toArray<HTMLElement>("[data-testid='message-block']", lane);
    const latest = messageBlocks[messageBlocks.length - 1];
    if (!latest || latest.dataset.forgeMotionSeen === "true") return;

    latest.dataset.forgeMotionSeen = "true";
    gsap.fromTo(
      latest,
      { autoAlpha: 0, y: 8, scale: 0.996 },
      {
        autoAlpha: 1,
        y: 0,
        scale: 1,
        duration: forgeMotion.message.duration,
        ease: forgeMotion.message.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
  }, { scope: laneRef, dependencies: [blocks.length] });

  const setScrolledUpIfChanged = useCallback((next: boolean) => {
    setUserScrolledUp((current) => (current === next ? current : next));
  }, []);

  const cancelAutoScroll = useCallback(() => {
    if (autoScrollRafRef.current === null) return;
    cancelAnimationFrame(autoScrollRafRef.current);
    autoScrollRafRef.current = null;
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
    const el = scrollRef.current;
    if (el && el.scrollHeight - el.scrollTop - el.clientHeight > BOTTOM_LOCK_THRESHOLD) {
      cancelAutoScroll();
    }
    if (scrollRafRef.current !== null) return;
    scrollRafRef.current = requestAnimationFrame(() => {
      scrollRafRef.current = null;
      updateStickiness();
    });
  }, [cancelAutoScroll, updateStickiness]);

  const handleWheel = useCallback((event: WheelEvent<HTMLDivElement>) => {
    if (event.deltaY < 0) {
      cancelAutoScroll();
      stickToBottomRef.current = false;
      setScrolledUpIfChanged(true);
    }
  }, [cancelAutoScroll, setScrolledUpIfChanged]);

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
        <div data-testid="message-lane" data-surface="conversation" ref={laneRef} className="forge-conversation-lane forge-operating-lane">
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
        <div data-testid="message-lane" data-surface="conversation" ref={laneRef} className="forge-conversation-lane forge-operating-lane forge-message-lane flex flex-col">
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
        <ForgeControlButton
          data-testid="scroll-to-bottom"
          aria-label="回到底部"
          title="回到底部"
          onClick={scrollToBottom}
          className="forge-scroll-to-bottom absolute z-10 flex size-7 items-center justify-center text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
        >
          <ArrowDown className="size-3.5" />
        </ForgeControlButton>
      )}
    </div>
  );
}
