import { invoke } from "@tauri-apps/api/core";
import type { WorkspaceReview } from "./types";

export async function getWorkspaceReview(
  sessionId?: string,
  workingDir?: string | null,
): Promise<WorkspaceReview> {
  return invoke<WorkspaceReview>("get_workspace_review", {
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
  });
}
