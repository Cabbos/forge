import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core.ts";

export interface KeyStatus {
  provider: string;
  set: boolean;
  preview: string;
}

export interface ProviderCatalogEntry {
  id: string;
  label: string;
  default_model: string;
  context_window_tokens: number | null;
  aliases: string[];
  requires_api_key: boolean;
  supports_streaming: boolean;
  supports_tools: boolean;
}

export type ProviderProbeStatus = "passed" | "failed";
export type ProviderProbeCheckStatus = "passed" | "failed";
export type ProviderModelCatalogStatus = "available" | "unavailable";

export interface ProviderProbeCheck {
  id: string;
  label: string;
  status: ProviderProbeCheckStatus;
  message: string;
}

export interface ProviderProbeResult {
  provider: string;
  provider_label: string;
  model: string | null;
  base_url: string | null;
  status: ProviderProbeStatus;
  checks: ProviderProbeCheck[];
  message: string;
  remediation: string | null;
}

export interface ProviderModelCatalogItem {
  id: string;
  name: string;
}

export interface ProviderModelCatalogResult {
  provider: string;
  provider_label: string;
  base_url: string | null;
  status: ProviderModelCatalogStatus;
  models: ProviderModelCatalogItem[];
  message: string;
  remediation: string | null;
}

export async function getApiKeyStatus(): Promise<KeyStatus[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("get_api_key_status");
}

export async function getProviderCatalog(): Promise<ProviderCatalogEntry[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("get_provider_catalog");
}

export async function setApiKey(provider: string, key: string): Promise<void> {
  return invoke("set_api_key", { provider, key });
}

export async function probeProvider(provider: string): Promise<ProviderProbeResult> {
  if (!hasTauriRuntime()) {
    throw new Error("Provider probe is not available outside Tauri runtime");
  }
  return invoke("probe_provider", { provider });
}

export async function listProviderModels(provider: string): Promise<ProviderModelCatalogResult> {
  if (!hasTauriRuntime()) {
    throw new Error("Provider model catalog is not available outside Tauri runtime");
  }
  return invoke("list_provider_models", { provider });
}
