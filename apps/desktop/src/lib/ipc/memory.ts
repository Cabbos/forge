import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type {
  MemoryPatch,
  MemoryScope,
  SelectedContextMemory,
  WikiMemory,
} from "./types";

export async function listMemories(
  scope?: MemoryScope,
  projectPath?: string,
  sessionId?: string | null,
): Promise<WikiMemory[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("list_memories", {
    scope: scope ?? null,
    projectPath: projectPath ?? null,
    sessionId: sessionId ?? null,
  });
}

export async function updateMemory(
  memoryId: string,
  patch: MemoryPatch,
  sessionId?: string,
): Promise<WikiMemory> {
  return invoke("update_memory", { memoryId, patch, sessionId: sessionId ?? null });
}

export async function forgetMemory(memoryId: string, sessionId?: string): Promise<WikiMemory> {
  return invoke("forget_memory", { memoryId, sessionId: sessionId ?? null });
}

export async function pinMemory(memoryId: string, sessionId?: string): Promise<WikiMemory> {
  return invoke("pin_memory", { memoryId, sessionId: sessionId ?? null });
}

export async function selectContextMemories(
  message: string,
  projectPath?: string,
  sessionId?: string | null,
): Promise<SelectedContextMemory[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("select_context_memories", {
    message,
    projectPath: projectPath ?? null,
    sessionId: sessionId ?? null,
  });
}
