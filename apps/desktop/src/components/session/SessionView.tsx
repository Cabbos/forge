import { useStore } from "@/store";
import { ChatView } from "@/components/chat/ChatView";
import { InputBar } from "./InputBar";

interface SessionViewProps { sessionId: string }

export function SessionView({ sessionId }: SessionViewProps) {
  const session = useStore((s) => s.sessions.get(sessionId));

  return (
    <div className="flex-1 min-h-0 flex flex-col" style={{ background: "#0D0D0D" }}>
      {/* Model indicator — fixed height */}
      <div className="flex items-center justify-center py-1 flex-shrink-0" style={{ borderBottom: "1px solid #141414" }}>
        <span className="text-[10px] font-mono" style={{ color: "#555" }}>
          🐋 {session?.model || "DeepSeek"}
        </span>
      </div>

      {/* Chat scroll area — takes all remaining space */}
      <ChatView />

      {/* Input — fixed at bottom */}
      <InputBar sessionId={sessionId} />
    </div>
  );
}
