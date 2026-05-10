import { invoke } from "@tauri-apps/api/core";

export interface SessionCreated {
  session_id: string;
}

export interface SessionInfo {
  id: string;
  tool_type: string;
  status: string;
  created_at: string;
}

export async function createSession(
  toolType: string,
  workingDir: string,
  toolPath?: string,
  model?: string
): Promise<SessionCreated> {
  return invoke<SessionCreated>("create_session", {
    toolType,
    workingDir,
    toolPath: toolPath ?? null,
    model: model ?? null,
  });
}

export async function sendInput(sessionId: string, text: string): Promise<void> {
  return invoke("send_input", { sessionId, text });
}

export async function sendSignal(
  sessionId: string,
  signal: "interrupt" | "terminate"
): Promise<void> {
  return invoke("send_signal", { sessionId, signal });
}

export async function resizeSession(
  sessionId: string,
  cols: number,
  rows: number
): Promise<void> {
  return invoke("resize_session", { sessionId, cols, rows });
}

export async function killSession(sessionId: string): Promise<void> {
  return invoke("kill_session", { sessionId });
}

export async function listSessions(): Promise<SessionInfo[]> {
  return invoke("list_sessions");
}

// ── Plugin Manager ──

export interface PluginEntry {
  id: string;
  name: string;
  description: string;
  plugin_type: "mcp_server" | "hook" | "skill" | "extension";
  agent: "claude" | "codex" | "hermes";
  category: string;
  status: "not_installed" | { installed: { enabled: boolean } } | "installing" | { error: { message: string } };
  config_schema?: unknown;
  current_config?: unknown;
  homepage?: string;
  author?: string;
}

export async function listPlugins(agent: string): Promise<PluginEntry[]> {
  return invoke("list_plugins", { agent });
}

export async function discoverPlugins(agent: string): Promise<PluginEntry[]> {
  return invoke("discover_plugins", { agent });
}

export async function installPlugin(
  pluginId: string,
  agent: string,
  config?: unknown
): Promise<void> {
  return invoke("install_plugin", { pluginId, agent, config: config ?? null });
}

export async function uninstallPlugin(
  pluginId: string,
  agent: string
): Promise<void> {
  return invoke("uninstall_plugin", { pluginId, agent });
}

export async function togglePlugin(
  pluginId: string,
  agent: string,
  enabled: boolean
): Promise<void> {
  return invoke("toggle_plugin", { pluginId, agent, enabled });
}

export async function confirmResponse(
  blockId: string,
  approved: boolean
): Promise<void> {
  return invoke("confirm_response", { blockId, approved });
}

export interface KeyStatus {
  provider: string;
  set: boolean;
  preview: string;
}

export async function getApiKeyStatus(): Promise<KeyStatus[]> {
  return invoke("get_api_key_status");
}

export async function setApiKey(provider: string, key: string): Promise<void> {
  return invoke("set_api_key", { provider, key });
}
