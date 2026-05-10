import { useActiveBlocks } from "@/store";
import { MessageList } from "./MessageList";

export function ChatView() {
  const blocks = useActiveBlocks();
  return <MessageList blocks={blocks} />;
}
