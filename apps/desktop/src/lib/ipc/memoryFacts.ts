import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type { MemoryFact, UpsertMemoryFactInput, UpsertMemoryFactOutput } from "./types";

export async function listMemoryFacts(
  query?: string,
  profileId?: string | null,
): Promise<MemoryFact[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("list_memory_facts", {
    query: query ?? null,
    profileId: profileId ?? null,
  });
}

export async function upsertMemoryFact(
  input: UpsertMemoryFactInput,
): Promise<UpsertMemoryFactOutput> {
  return invoke("upsert_memory_fact", { input });
}

export async function deleteMemoryFact(id: string): Promise<boolean> {
  return invoke("delete_memory_fact", { id });
}
