import { useRef, useEffect, useState } from "react";
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
  const [userScrolledUp, setUserScrolledUp] = useState(false);

  // Auto-scroll to bottom on new content, unless user scrolled up
  useEffect(() => {
    if (userScrolledUp) return;
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [blocks.length, blocks[blocks.length - 1]?.content, userScrolledUp]);

  const handleScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 60;
    setUserScrolledUp(!atBottom);
  };

  if (blocks.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ color: "#444" }}>
        <p className="text-sm">Send a message to begin.</p>
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      onScroll={handleScroll}
      className="flex-1 overflow-y-auto"
      style={{ padding: "28px 40px" }}
    >
      <div className="flex flex-col gap-5" style={{ maxWidth: "100%" }}>
        {blocks.map((block, i) => (
          <BlockRenderer key={`${block.block_id}-${i}`} block={block} />
        ))}
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
