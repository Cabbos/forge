import { useStore } from "@/store";
import { ChatView } from "@/components/chat/ChatView";
import { InputBar } from "./InputBar";

interface SessionViewProps {
  sessionId: string;
}

export function SessionView({ sessionId }: SessionViewProps) {
  const session = useStore((s) => s.sessions.get(sessionId));
  const isAgent = session?.agentType === "claude" || session?.agentType === "codex" || session?.agentType === "hermes";

  return (
    <div className="flex flex-col h-full min-h-0">
      <ChatView />
      {isAgent && <AgentContext model={session?.model} />}
      <InputBar sessionId={sessionId} />
    </div>
  );
}

function AgentContext({ model }: { model?: string }) {
  if (!model) return null;
  const display = model.replace("claude-", "").replace("deepseek-", "").replace("-20251001", "");
  return (
    <div className="flex items-center justify-center gap-2 py-1 text-[11px] text-muted-foreground/50">
      <span className="size-1 rounded-full bg-emerald-400/60" />
      {display}
    </div>
  );
}
