import { useStore } from "@/store";
import { deriveComposerTurnState } from "./composerTurnState";

export function useComposerSessionState(sessionId: string) {
  const workflow = useStore((s) => s.workflowBySession.get(sessionId) ?? null);
  const session = useStore((s) => s.sessions.get(sessionId));
  const agentTurn = useStore((s) => s.agentTurnBySession.get(sessionId) ?? null);

  const isRunning = session?.status === "running";
  const isStreaming = session?.streaming ?? false;
  const hasPendingOutput = session?.blocks.some((block) => block.event_type === "pending") ?? false;
  const { composerState, isTurnInFlight } = deriveComposerTurnState({
    agentTurnStatus: agentTurn?.status,
    hasPendingOutput,
    isRunning,
    isStreaming,
  });

  return {
    composerState,
    isRunning,
    isTurnInFlight,
    workflow,
    workingDir: session?.workingDir,
  };
}
