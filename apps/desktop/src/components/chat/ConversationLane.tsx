import type { Ref } from "react";
import { StartReadinessCard } from "@/components/session/StartReadinessCard";
import type { ConversationTurn as ConversationTurnModel } from "@/components/chat/messageGrouping";
import { ConversationTurn } from "@/components/chat/ConversationTurn";

interface ConversationLaneProps {
  conversationTurns: ConversationTurnModel[];
  laneRef: Ref<HTMLDivElement>;
  sessionId?: string;
  empty?: boolean;
}

export function ConversationLane({ conversationTurns, laneRef, sessionId, empty = false }: ConversationLaneProps) {
  return (
    <div
      data-testid="message-lane"
      data-surface="conversation"
      ref={laneRef}
      className={empty
        ? "forge-conversation-lane forge-operating-lane"
        : "forge-conversation-lane forge-operating-lane forge-message-lane flex flex-col"}
    >
      {empty ? (
        <StartReadinessCard sessionId={sessionId} />
      ) : (
        conversationTurns.map((turn) => (
          <ConversationTurn key={turn.key} turn={turn} sessionId={sessionId} />
        ))
      )}
    </div>
  );
}
