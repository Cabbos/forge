import { useRef, useEffect } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { BlockState } from "@/lib/protocol";
import { ThinkingBlock } from "@/components/messages/ThinkingBlock";
import { TextBlock } from "@/components/messages/TextBlock";
import { ToolCallCard } from "@/components/messages/ToolCallCard";
import { UserMessage } from "@/components/messages/UserMessage";
import { ShellCard } from "@/components/messages/ShellCard";
import { DiffCard } from "@/components/messages/DiffCard";
import { ConfirmCard } from "@/components/messages/ConfirmCard";

interface MessageListProps { blocks: BlockState[] }

export function MessageList({ blocks }: MessageListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const prevLenRef = useRef(blocks.length);
  const prevContentRef = useRef("");
  const shouldAutoScroll = useRef(true);

  const virtualizer = useVirtualizer({
    count: blocks.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 80,
    overscan: 5,
  });

  const lastContent = blocks.length > 0 ? blocks[blocks.length - 1]?.content ?? "" : "";

  useEffect(() => {
    const len = blocks.length;
    if (len > prevLenRef.current || lastContent !== prevContentRef.current) {
      prevLenRef.current = len;
      prevContentRef.current = lastContent;
      if (shouldAutoScroll.current && scrollRef.current) {
        requestAnimationFrame(() => {
          scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
        });
      }
    }
  }, [blocks.length, lastContent]);

  const handleScroll = () => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    shouldAutoScroll.current = scrollHeight - scrollTop - clientHeight < 80;
  };

  if (blocks.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-center text-muted-foreground/50">
        <p className="text-sm">Send a message to begin.</p>
      </div>
    );
  }

  return (
    <div ref={scrollRef} onScroll={handleScroll} className="h-full overflow-auto">
      <div style={{ height: virtualizer.getTotalSize(), width: "100%", position: "relative" }}>
        {virtualizer.getVirtualItems().map((vi) => {
          const block = blocks[vi.index];
          return (
            <div
              key={`${block.block_id}-${vi.index}`}
              data-index={vi.index}
              ref={virtualizer.measureElement}
              style={{ position: "absolute", top: 0, left: 0, width: "100%", transform: `translateY(${vi.start}px)` }}
            >
              <BlockRenderer block={block} />
            </div>
          );
        })}
      </div>
    </div>
  );
}

function BlockRenderer({ block }: { block: BlockState }) {
  switch (block.event_type) {
    case "thinking": return <ThinkingBlock block={block} />;
    case "text": case "error": return <TextBlock block={block} />;
    case "tool_call": case "tool_call_result": return <ToolCallCard block={block} />;
    case "user_message": return <UserMessage block={block} />;
    case "shell": return <ShellCard block={block} />;
    case "diff_view": return <DiffCard block={block} />;
    case "confirm_ask": return <ConfirmCard block={block} />;
    default: return block.content ? <TextBlock block={block} /> : null;
  }
}
