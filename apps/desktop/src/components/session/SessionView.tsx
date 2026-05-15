import { useActiveBlocks, useStore } from "@/store";
import { ChatView } from "@/components/chat/ChatView";
import { InputBar } from "./InputBar";
import { TaskProgressPopover } from "./TaskProgressPopover";
import { FirstLoopProgressStrip } from "./FirstLoopProgressStrip";

interface SessionViewProps { sessionId: string }

export function SessionView({ sessionId }: SessionViewProps) {
  const session = useStore((s) => s.sessions.get(sessionId));
  const blocks = useActiveBlocks();
  const firstLoopDraft = useStore((s) => s.firstLoopDraftBySession.get(sessionId) ?? null);

  return (
    <div className="flex-1 min-h-0 flex flex-col bg-background">
      {/* Status indicator — fixed height */}
      <div className="flex items-center justify-center gap-2 border-b border-border py-1.5 flex-shrink-0">
        <TaskProgressPopover session={session} />
      </div>
      <FirstLoopProgressStrip blocks={blocks} draft={firstLoopDraft} />

      {/* Chat scroll area — takes all remaining space */}
      <ChatView />

      {/* Input — fixed at bottom */}
      <InputBar sessionId={sessionId} />
    </div>
  );
}
