import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core.ts";

export interface KeyStatus {
  provider: string;
  configured: boolean;
  source: string;
  status: "available" | "missing" | "unavailable" | "not_configured";
  error?: string | null;
}

export interface ProviderCatalogEntry {
  id: string;
  label: string;
  default_model: string;
  context_window_tokens: number | null;
  models: ProviderCatalogModelEntry[];
  aliases: string[];
  requires_api_key: boolean;
  supports_streaming: boolean;
  supports_tools: boolean;
  source: ProviderProfileSource;
  base_url: string | null;
  transport: ProviderTransportName;
  api_key_env: string[];
  base_url_env: string[];
  model_catalog_source: ProviderModelCatalogSource | null;
  model_catalog_recorded_at_ms: number | null;
  probe_evidence: ProviderProbeEvidence | null;
}

export interface ProviderCatalogModelEntry {
  id: string;
  name: string;
  context_window_tokens?: number | null;
}

export type ProviderProfileSource = "built_in" | "user_override" | "user_defined";
export type ProviderTransportName =
  | "anthropic_messages"
  | "openai_chat_completions"
  | "openai_responses"
  | "native_gemini"
  | "bedrock_converse"
  | "custom_openai_compatible"
  | "custom_anthropic_compatible";

export interface ProviderProfileInput {
  id: string;
  label: string;
  transport: ProviderTransportName;
  base_url: string | null;
  api_key_env: string[];
  base_url_env: string[];
  default_model: string;
  aliases: string[];
  supports_tools: boolean;
  supports_streaming: boolean;
}

export type ProviderProbeStatus = "passed" | "failed";
export type ProviderProbeCheckStatus = "passed" | "failed";
export type ProviderProbeEvidenceSource = "manual_probe";
export type ProviderModelCatalogStatus = "available" | "unavailable";
export type ProviderModelCatalogSource = "live_endpoint" | "static_fallback" | "unsupported";

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

export interface ProviderProbeEvidence {
  source: ProviderProbeEvidenceSource;
  status: ProviderProbeStatus;
  recorded_at_ms: number | null;
  model: string | null;
  base_url: string | null;
  checks: ProviderProbeEvidenceCheck[];
}

export interface ProviderProbeEvidenceCheck {
  id: string;
  label: string;
  status: ProviderProbeCheckStatus;
}

export interface ProviderModelCatalogItem {
  id: string;
  name: string;
}

export interface ProviderModelCatalogResult {
  provider: string;
  provider_label: string;
  base_url: string | null;
  source: ProviderModelCatalogSource;
  status: ProviderModelCatalogStatus;
  recorded_at_ms: number | null;
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

export async function upsertProviderProfile(input: ProviderProfileInput): Promise<ProviderCatalogEntry> {
  if (!hasTauriRuntime()) {
    throw new Error("Provider profile editing is not available outside Tauri runtime");
  }
  return invoke("upsert_provider_profile", { input });
}

export async function deleteProviderProfile(provider: string): Promise<void> {
  if (!hasTauriRuntime()) {
    throw new Error("Provider profile editing is not available outside Tauri runtime");
  }
  return invoke("delete_provider_profile", { provider });
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
