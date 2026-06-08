import { useActiveBlocks, useStore } from "@/store";
import { MessageList } from "./MessageList";

export function ChatView() {
  const blocks = useActiveBlocks();
  const sessionId = useStore((s) => s.activeSessionId) ?? undefined;
  return <MessageList blocks={blocks} sessionId={sessionId} />;
}
