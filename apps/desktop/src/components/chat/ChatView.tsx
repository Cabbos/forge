import { useCallback } from "react";
import { useActiveBlocks } from "@/store";
import { MessageList } from "./MessageList";
import { Copy } from "lucide-react";

export function ChatView() {
  const blocks = useActiveBlocks();

  const copyConversation = useCallback(() => {
    const text = blocks
      .filter(b => b.content)
      .map(b => `${b.event_type === "user_message" ? "You" : "AI"}: ${b.content}`)
      .join("\n\n");
    navigator.clipboard.writeText(text).catch(() => {});
  }, [blocks]);

  return (
    <div className="flex-1 min-h-0">
      {blocks.length > 0 && (
        <div className="flex justify-end px-6 pt-2">
          <button onClick={copyConversation} className="flex items-center gap-1.5 text-[11px] text-muted-foreground/40 hover:text-muted-foreground transition-colors">
            <Copy className="size-3" />
            Copy
          </button>
        </div>
      )}
      <div className="max-w-3xl mx-auto px-6 py-4 h-full">
        <MessageList blocks={blocks} />
      </div>
    </div>
  );
}
