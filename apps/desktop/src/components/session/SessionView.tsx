import { ChatView } from "@/components/chat/ChatView";
import { InputBar } from "./InputBar";

interface SessionViewProps { sessionId: string }

export function SessionView({ sessionId }: SessionViewProps) {
  return (
    <div className="flex-1 min-h-0 flex flex-col bg-background">
      {/* Chat scroll area — takes all remaining space */}
      <ChatView />

      {/* Input — fixed at bottom */}
      <InputBar sessionId={sessionId} />
    </div>
  );
}
