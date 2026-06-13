import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type { MemoryFact, UpsertMemoryFactInput, UpsertMemoryFactOutput } from "./types";

export async function listMemoryFacts(query?: string): Promise<MemoryFact[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("list_memory_facts", { query: query ?? null });
}

export async function upsertMemoryFact(
  input: UpsertMemoryFactInput,
): Promise<UpsertMemoryFactOutput> {
  return invoke("upsert_memory_fact", { input });
}

export async function deleteMemoryFact(id: string): Promise<boolean> {
  return invoke("delete_memory_fact", { id });
}
