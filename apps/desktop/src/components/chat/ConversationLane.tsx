import type { Ref } from "react";
import { ToolActivityGroup } from "@/components/messages/ToolActivityGroup";
import { StartReadinessCard } from "@/components/session/StartReadinessCard";
import { MemoizedBlockRenderer } from "@/components/chat/BlockRenderer";
import type { ConversationTurn, MessageItem } from "@/components/chat/messageGrouping";

interface ConversationLaneProps {
  conversationTurns: ConversationTurn[];
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
          <section
            key={turn.key}
            data-testid="conversation-turn"
            data-turn-shape={turn.hasEvidence ? "with-evidence" : "direct"}
            data-turn-start={turn.startsWithUser ? "user" : "system"}
            className="forge-conversation-turn"
          >
            {turn.items.map((item) => (
              <div
                data-testid="message-block"
                data-block-role={getMessageBlockRole(item)}
                className="forge-message-block"
                key={item.key}
              >
                {item.kind === "tool_group"
                  ? <ToolActivityGroup blocks={item.blocks} />
                  : <MemoizedBlockRenderer block={item.block} sessionId={sessionId} />}
              </div>
            ))}
          </section>
        ))
      )}
    </div>
  );
}

function getMessageBlockRole(item: MessageItem) {
  if (item.kind === "tool_group") return "trace";

  switch (item.block.event_type) {
    case "user_message":
      return "user";
    case "text":
      return "assistant";
    case "thinking":
    case "pending":
    case "tool_call":
    case "tool_call_result":
    case "shell":
    case "context_compacted":
      return "trace";
    case "diff_view":
    case "confirm_ask":
    case "delivery_summary":
    case "error":
      return "artifact";
    default:
      return item.block.content ? "assistant" : "trace";
  }
}
