import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type { McpContextSources } from "./types";

export async function listMcpContextSources(
  sessionId?: string,
): Promise<McpContextSources> {
  if (!hasTauriRuntime()) return { resources: [], prompts: [] };
  return invoke("list_mcp_context_sources", { sessionId: sessionId ?? null });
}
