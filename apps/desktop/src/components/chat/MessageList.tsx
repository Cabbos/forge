import { useRef, useEffect, useState, useCallback } from "react";
import { ArrowDown } from "lucide-react";
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

  useEffect(() => {
    if (userScrolledUp) return;
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [blocks.length, blocks[blocks.length - 1]?.content, userScrolledUp]);

  const handleScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    setUserScrolledUp(el.scrollHeight - el.scrollTop - el.clientHeight > 60);
  }, []);

  const scrollToBottom = () => {
    const el = scrollRef.current;
    if (el) { el.scrollTop = el.scrollHeight; setUserScrolledUp(false); }
  };

  if (blocks.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ color: "#333" }}>
        <p className="text-sm">Send a message to begin.</p>
      </div>
    );
  }

  return (
    <div className="relative flex-1">
      <div ref={scrollRef} onScroll={handleScroll} className="h-full overflow-y-auto" style={{ padding: "28px 48px" }}>
        <div className="flex flex-col" style={{ maxWidth: "780px", margin: "0 auto" }}>
          {blocks.map((block, i) => (
            <BlockRenderer key={`${block.block_id}-${i}`} block={block} />
          ))}
        </div>
      </div>
      {userScrolledUp && (
        <button onClick={scrollToBottom}
          className="absolute bottom-4 left-1/2 -translate-x-1/2 p-2 rounded-full shadow-lg transition-all z-10"
          style={{ background: "#1c1c1c", border: "1px solid #2a2a2a" }}>
          <ArrowDown className="size-4" style={{ color: "#D4A853" }} />
        </button>
      )}
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
