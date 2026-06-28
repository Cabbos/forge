import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type {
  ContinuityExperience,
  ContinuityExperienceStatus,
} from "./types";

export async function listContinuityExperiences(
  sessionId?: string,
  workingDir?: string | null,
): Promise<ContinuityExperience[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("list_continuity_experiences", {
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
  });
}

export async function searchContinuityExperiences(
  query: string,
  sessionId?: string,
  workingDir?: string | null,
  limit?: number,
): Promise<ContinuityExperience[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("search_continuity_experiences", {
    query,
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
    limit: limit ?? null,
  });
}

export async function updateContinuityExperienceStatus(
  experienceId: string,
  status: ContinuityExperienceStatus,
  sessionId?: string,
  workingDir?: string | null,
): Promise<ContinuityExperience> {
  return invoke("update_continuity_experience_status", {
    experienceId,
    status,
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
  });
}
