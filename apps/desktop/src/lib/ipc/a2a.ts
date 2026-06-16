import { invoke } from "@tauri-apps/api/core";
import type { AgentA2ASessionState, AgentA2AStatesPayload } from "./types";

export async function getAgentA2AState(sessionId: string): Promise<AgentA2ASessionState | null> {
  return invoke<AgentA2ASessionState | null>("get_agent_a2a_state", { sessionId });
}

export async function listAgentA2AStates(): Promise<AgentA2AStatesPayload> {
  return invoke<AgentA2AStatesPayload>("list_agent_a2a_states");
}
