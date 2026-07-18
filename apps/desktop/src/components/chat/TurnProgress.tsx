import type { LiveProgressCandidate } from "./conversationTurnView.ts";
import { useStableProgressLabel } from "./useStableProgressLabel.ts";
import "../../styles/conversation-turn.css";

export function TurnProgress({ candidate }: { candidate: LiveProgressCandidate | null }) {
  const visible = useStableProgressLabel(candidate);
  if (!visible) return null;

  return (
    <div
      data-testid="conversation-progress"
      data-progress-id={visible.id}
      data-progress-motion={visible.motion}
      className="forge-turn-progress"
      role="status"
      aria-live="polite"
      aria-atomic="true"
    >
      <span aria-hidden="true" className="forge-turn-progress-dot" />
      <span key={visible.id} className="forge-turn-progress-label">{visible.label}</span>
      <span aria-hidden="true" className="forge-turn-progress-track">
        <span className="forge-turn-progress-trace" />
      </span>
    </div>
  );
}
