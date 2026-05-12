import { invoke } from "@tauri-apps/api/core";

export interface SessionCreated { session_id: string }

export interface SessionInfo {
  id: string; provider: string; model: string;
  status: string; created_at: string;
}

export async function createSession(workingDir: string, apiKey: string, model: string): Promise<SessionCreated> {
  return invoke<SessionCreated>("create_session", {
    workingDir, apiKey: apiKey || "", model,
  });
}

export async function sendInput(sessionId: string, text: string): Promise<void> {
  return invoke("send_input", { sessionId, text });
}

export async function killSession(sessionId: string): Promise<void> {
  return invoke("kill_session", { sessionId });
}

export async function listSessions(): Promise<SessionInfo[]> {
  return invoke("list_sessions");
}

export async function confirmResponse(blockId: string, approved: boolean): Promise<void> {
  return invoke("confirm_response", { blockId, approved });
}

export interface KeyStatus { provider: string; set: boolean; preview: string }

export async function getApiKeyStatus(): Promise<KeyStatus[]> {
  return invoke("get_api_key_status");
}

export async function setApiKey(provider: string, key: string): Promise<void> {
  return invoke("set_api_key", { provider, key });
}

// Plugin exports (kept for existing UI components)
export interface PluginEntry {
  id: string; name: string; description: string;
  plugin_type: string; agent: string; category: string;
  status: unknown; config_schema?: unknown; current_config?: unknown;
  homepage?: string; author?: string;
}
export async function listPlugins(agent: string): Promise<PluginEntry[]> { return invoke("list_plugins", { agent }); }
export async function discoverPlugins(agent: string): Promise<PluginEntry[]> { return invoke("discover_plugins", { agent }); }
export async function installPlugin(pluginId: string, agent: string, config?: unknown): Promise<void> { return invoke("install_plugin", { pluginId, agent, config: config ?? null }); }
export async function uninstallPlugin(pluginId: string, agent: string): Promise<void> { return invoke("uninstall_plugin", { pluginId, agent }); }
export async function togglePlugin(pluginId: string, agent: string, enabled: boolean): Promise<void> { return invoke("toggle_plugin", { pluginId, agent, enabled }); }

// Capability IPC
export interface CapabilityInfo {
  id: string;
  name: string;
  description: string;
  kind: string;
  source: string;
  version: string;
  enabled: boolean;
}

export async function listCapabilities(): Promise<CapabilityInfo[]> {
  return invoke("list_capabilities");
}

export async function toggleCapability(id: string, enabled: boolean): Promise<void> {
  return invoke("toggle_capability", { capabilityId: id, enabled });
}

/** Search workspace files for @ autocomplete */
export async function searchWorkspaceFiles(query: string): Promise<string[]> {
  return invoke("search_workspace_files", { query });
}

/** Install a skill from GitHub (owner/repo) */
export async function installSkill(repo: string): Promise<CapabilityInfo> {
  return invoke("install_skill", { repo });
}
