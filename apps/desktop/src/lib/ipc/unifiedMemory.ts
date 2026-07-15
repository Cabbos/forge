import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type {
  UnifiedMemoryAction,
  UnifiedMemoryActionError,
  UnifiedMemoryActionResult,
  UnifiedMemoryListFilter,
  UnifiedMemoryRecord,
} from "./types";

export async function listUnifiedMemories(
  sessionId?: string,
  workingDir?: string | null,
  query?: string,
  filter?: UnifiedMemoryListFilter,
): Promise<UnifiedMemoryRecord[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("list_unified_memories", {
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
    query: query ?? null,
    filter: filter ?? null,
  });
}

export async function applyUnifiedMemoryAction(
  action: UnifiedMemoryAction,
  sessionId?: string,
  workingDir?: string | null,
): Promise<UnifiedMemoryActionResult | null> {
  if (!hasTauriRuntime()) return null;
  return invoke("apply_unified_memory_action", {
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
    action,
  });
}

export function unifiedMemoryActionErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (isUnifiedMemoryActionError(error)) return error.message;
  return String(error);
}

function isUnifiedMemoryActionError(error: unknown): error is UnifiedMemoryActionError {
  return Boolean(
    error &&
      typeof error === "object" &&
      "message" in error &&
      typeof (error as { message?: unknown }).message === "string" &&
      "kind" in error,
  );
}
