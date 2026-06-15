import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type {
  CapabilityInfo,
  EcosystemItem,
  McpEcosystemItemConfig,
  ToolInventoryEntry,
} from "./types";

export async function listEcosystemItems(): Promise<EcosystemItem[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("list_ecosystem_items");
}

export async function setEcosystemEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke("set_ecosystem_enabled", { id, enabled });
}

export async function getToolInventory(): Promise<ToolInventoryEntry[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("get_tool_inventory");
}

/**
 * Configure an ecosystem item.
 *
 * MCP server items support `.forge/mcp.json` write-back. Other item kinds
 * still return a clear unsupported error until their config models stabilize.
 */
export async function configureEcosystemItem(
  id: string,
  config: McpEcosystemItemConfig,
): Promise<void> {
  return invoke("configure_ecosystem_item", { id, config });
}

/** Search workspace files for @ autocomplete */
export async function searchWorkspaceFiles(
  query: string,
  sessionId?: string,
  workingDir?: string | null,
): Promise<string[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("search_workspace_files", {
    query,
    sessionId: sessionId ?? null,
    workingDir: workingDir ?? null,
  });
}

/** Install a skill from GitHub (owner/repo) */
export async function installSkill(repo: string): Promise<CapabilityInfo> {
  return invoke("install_skill", { repo });
}
