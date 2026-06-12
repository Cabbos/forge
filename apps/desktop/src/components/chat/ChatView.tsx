import { useActiveBlocks, useStore } from "@/store";
import { AgentA2AInlineSummary } from "@/components/messages/AgentA2ATimeline";
import { MessageList } from "./MessageList";

export function ChatView() {
  const blocks = useActiveBlocks();
  const sessionId = useStore((s) => s.activeSessionId) ?? undefined;
  const agentA2A = useStore((s) => sessionId ? s.agentA2ABySession.get(sessionId) ?? null : null);

  return (
    <div className="flex flex-col flex-1 min-h-0">
      {agentA2A && agentA2A.tasks.length > 0 && (
        <div className="shrink-0 px-3 pt-2">
          <AgentA2AInlineSummary state={agentA2A} />
        </div>
      )}
      <MessageList blocks={blocks} sessionId={sessionId} />
    </div>
  );
}
