import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type { WorkflowOverrideAction, WorkflowState } from "./types";

export async function getWorkflowState(sessionId: string): Promise<WorkflowState | null> {
  if (!hasTauriRuntime()) return null;
  return invoke("get_workflow_state", { sessionId });
}

export async function overrideWorkflowRoute(
  sessionId: string,
  action: WorkflowOverrideAction,
): Promise<WorkflowState> {
  return invoke("override_workflow_route", { sessionId, action });
}
