import { useStore } from "@/store";
import { ChatView } from "@/components/chat/ChatView";
import { InputBar } from "./InputBar";

interface SessionViewProps { sessionId: string }

export function SessionView({ sessionId }: SessionViewProps) {
  const session = useStore((s) => s.sessions.get(sessionId));

  return (
    <div className="flex-1 min-h-0 flex flex-col" style={{ background: "#0D0D0D" }}>
      {/* Status indicator — fixed height */}
      <div className="flex items-center justify-center gap-2 py-1.5 flex-shrink-0" style={{ borderBottom: "1px solid #141414" }}>
        <span className="h-1.5 w-1.5 rounded-full" style={{ background: session?.streaming ? "#D4A853" : "#4A9E6B" }} />
        <span className="text-[10px]" style={{ color: "#666" }}>
          {session?.streaming ? "正在处理你的请求" : "准备好了，可以直接描述目标"}
        </span>
      </div>

      {/* Chat scroll area — takes all remaining space */}
      <ChatView />

      {/* Input — fixed at bottom */}
      <InputBar sessionId={sessionId} />
    </div>
  );
}
