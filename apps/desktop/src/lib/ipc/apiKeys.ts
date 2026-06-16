import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core.ts";

export interface KeyStatus {
  provider: string;
  set: boolean;
  preview: string;
}

export async function getApiKeyStatus(): Promise<KeyStatus[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("get_api_key_status");
}

export async function setApiKey(provider: string, key: string): Promise<void> {
  return invoke("set_api_key", { provider, key });
}
