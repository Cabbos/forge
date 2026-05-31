import type { AgentTurnStatus } from "@/lib/protocol";

export type ComposerSurfaceState = "running" | "busy" | "paused";

interface DeriveComposerTurnStateInput {
  agentTurnStatus?: AgentTurnStatus;
  hasPendingOutput: boolean;
  isRunning: boolean;
  isStreaming: boolean;
}

export function deriveComposerTurnState({
  agentTurnStatus,
  hasPendingOutput,
  isRunning,
  isStreaming,
}: DeriveComposerTurnStateInput) {
  const isTurnInFlight = isRunning && (
    isActiveAgentTurn(agentTurnStatus) ||
    hasPendingOutput ||
    (!isTerminalAgentTurn(agentTurnStatus) && isStreaming)
  );

  const composerState: ComposerSurfaceState = isTurnInFlight ? "running" : isRunning ? "busy" : "paused";

  return {
    composerState,
    isTurnInFlight,
  };
}

function isActiveAgentTurn(status: AgentTurnStatus | undefined) {
  return status === "started" ||
    status === "gathering_context" ||
    status === "calling_model" ||
    status === "running_tools" ||
    status === "verifying";
}

function isTerminalAgentTurn(status: AgentTurnStatus | undefined) {
  return status === "completed" || status === "failed" || status === "cancelled";
}
