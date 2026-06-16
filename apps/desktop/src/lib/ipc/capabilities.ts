import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type { CapabilityInfo, PluginEntry } from "./types";

export async function listCapabilities(): Promise<CapabilityInfo[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("list_capabilities");
}

export async function toggleCapability(id: string, enabled: boolean): Promise<void> {
  return invoke("toggle_capability", { capabilityId: id, enabled });
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
  config?: unknown,
): Promise<void> {
  return invoke("install_plugin", { pluginId, agent, config: config ?? null });
}

export async function uninstallPlugin(pluginId: string, agent: string): Promise<void> {
  return invoke("uninstall_plugin", { pluginId, agent });
}

export async function togglePlugin(
  pluginId: string,
  agent: string,
  enabled: boolean,
): Promise<void> {
  return invoke("toggle_plugin", { pluginId, agent, enabled });
}
