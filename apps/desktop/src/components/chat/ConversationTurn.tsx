import { MemoizedBlockRenderer } from "@/components/chat/BlockRenderer";
import type { ConversationTurn as RawConversationTurn } from "@/components/chat/messageGrouping";
import { deriveConversationTurnView } from "@/components/chat/conversationTurnView";
import { TurnProgress } from "@/components/chat/TurnProgress";
import { ConversationProcessDisclosure } from "@/components/chat/ConversationProcessDisclosure";

export function ConversationTurn({ turn, sessionId }: { turn: RawConversationTurn; sessionId?: string }) {
  const view = deriveConversationTurnView(turn);
  const primaryResult = view.finalAnswer ?? view.terminalError;
  const showTerminalFooter = Boolean(view.terminalSummary);
  const hasAssistantRail = Boolean(
    view.liveProgress
      || view.interruptions.length > 0
      || primaryResult
      || showTerminalFooter,
  );

  if (
    !view.userMessage
    && !view.liveProgress
    && view.interruptions.length === 0
    && !primaryResult
    && !showTerminalFooter
  ) return null;

  return (
    <section
      data-testid="conversation-turn"
      data-turn-shape={view.processDigest.items.length > 0 ? "with-evidence" : "direct"}
      data-turn-start={view.userMessage ? "user" : "system"}
      data-turn-rail={hasAssistantRail ? "assistant" : "user"}
      className="forge-conversation-turn forge-result-first-turn"
    >
      {view.userMessage && (
        <PrimaryBlock block={view.userMessage} role="user" sessionId={sessionId} />
      )}

      <TurnProgress candidate={view.liveProgress} />

      {view.interruptions.map((block) => (
        <PrimaryBlock key={block.block_id} block={block} role="artifact" sessionId={sessionId} />
      ))}

      {primaryResult && (
        <PrimaryBlock
          block={primaryResult}
          role={view.finalAnswer ? "assistant" : "artifact"}
          sessionId={sessionId}
        />
      )}

      {view.terminalSummary && (
        <ConversationProcessDisclosure
          digest={view.processDigest}
          terminal={view.terminalSummary}
          sessionId={sessionId}
        />
      )}
    </section>
  );
}

function PrimaryBlock({
  block,
  role,
  sessionId,
}: {
  block: NonNullable<ReturnType<typeof deriveConversationTurnView>["userMessage"]>;
  role: "user" | "assistant" | "artifact";
  sessionId?: string;
}) {
  return (
    <div
      data-testid="message-block"
      data-block-role={role}
      className="forge-message-block"
    >
      <MemoizedBlockRenderer block={block} sessionId={sessionId} />
    </div>
  );
}
