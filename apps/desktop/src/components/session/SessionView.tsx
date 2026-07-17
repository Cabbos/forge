import { ChatView } from "@/components/chat/ChatView";
import { InputBar } from "./InputBar";
import { useStore } from "@/store";

interface SessionViewProps { sessionId: string }

export function SessionView({ sessionId }: SessionViewProps) {
  const theme = useStore((state) => state.theme);

  return (
    <div
      data-conversation-theme={theme}
      className="forge-session-operating-surface flex-1 min-h-0 flex flex-col bg-background"
    >
      {/* Chat scroll area — takes all remaining space */}
      <ChatView />

      {/* Input — fixed at bottom */}
      <InputBar sessionId={sessionId} />
    </div>
  );
}
