import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core.ts";

export interface KeyStatus {
  provider: string;
  set: boolean;
  preview: string;
}

export type ProviderProbeStatus = "passed" | "failed";
export type ProviderProbeCheckStatus = "passed" | "failed";

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

export async function getApiKeyStatus(): Promise<KeyStatus[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("get_api_key_status");
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
