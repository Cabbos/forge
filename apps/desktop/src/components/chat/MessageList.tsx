import { useRef } from "react";
import { ArrowDown } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { ForgeControlButton } from "@/components/primitives/control-button";
import { ConversationLane } from "@/components/chat/ConversationLane";
import { groupConversationTurns, groupProcessBlocks } from "@/components/chat/messageGrouping";
import { useConversationScroll } from "@/components/chat/useConversationScroll";
import { useMessageEntryMotion } from "@/components/chat/useMessageEntryMotion";

interface MessageListProps { blocks: BlockState[]; sessionId?: string }

export function MessageList({ blocks, sessionId }: MessageListProps) {
  const laneRef = useRef<HTMLDivElement>(null);
  const lastBlock = blocks[blocks.length - 1];
  const messageItems = groupProcessBlocks(blocks);
  const conversationTurns = groupConversationTurns(messageItems);
  const {
    scrollRef,
    userScrolledUp,
    handleScroll,
    handleWheel,
    scrollToBottom,
    scrollStyle,
  } = useConversationScroll({ blockCount: blocks.length, lastBlock });

  useMessageEntryMotion(laneRef, blocks.length);

  if (blocks.length === 0) {
    return (
      <div data-testid="conversation-scroll" className="forge-conversation-scroll flex-1 min-h-0 overflow-y-auto">
        <ConversationLane empty conversationTurns={conversationTurns} laneRef={laneRef} sessionId={sessionId} />
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
        style={scrollStyle}
      >
        <ConversationLane conversationTurns={conversationTurns} laneRef={laneRef} sessionId={sessionId} />
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
