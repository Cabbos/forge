export type ProviderId = "deepseek" | "anthropic" | "openai" | "openrouter";

export interface ModelOption {
  id: string;
  name: string;
  description?: string;
  contextWindowTokens?: number;
}

export interface ProviderDefinition {
  id: ProviderId;
  label: string;
  shortLabel: string;
  keyPlaceholder: string;
  defaultModel: string;
  models: ModelOption[];
}

export const PROVIDERS: ProviderDefinition[] = [
  {
    id: "deepseek",
    label: "DeepSeek",
    shortLabel: "DeepSeek",
    keyPlaceholder: "sk-...",
    defaultModel: "deepseek-v4-flash[1m]",
    models: [
      { id: "deepseek-v4-flash[1m]", name: "DeepSeek V4 Flash 1M", description: "日常任务", contextWindowTokens: 1_000_000 },
      { id: "deepseek-v4-pro[1m]", name: "DeepSeek V4 Pro 1M", description: "复杂任务，支持 1M 上下文", contextWindowTokens: 1_000_000 },
    ],
  },
  {
    id: "anthropic",
    label: "Anthropic",
    shortLabel: "Claude",
    keyPlaceholder: "sk-ant-...",
    defaultModel: "claude-sonnet-4-6",
    models: [
      { id: "claude-sonnet-4-6", name: "Claude Sonnet 4.6", description: "主力编码" },
      { id: "claude-opus-4-7", name: "Claude Opus 4.7", description: "深度推理" },
      { id: "claude-haiku-4-5-20251001", name: "Claude Haiku 4.5", description: "轻量快速" },
    ],
  },
  {
    id: "openai",
    label: "OpenAI",
    shortLabel: "GPT",
    keyPlaceholder: "sk-...",
    defaultModel: "gpt-4o",
    models: [
      { id: "gpt-4o", name: "GPT-4o", description: "通用任务" },
      { id: "gpt-4o-mini", name: "GPT-4o Mini", description: "轻量快速" },
      { id: "gpt-4.1", name: "GPT-4.1", description: "编码与长上下文" },
    ],
  },
  {
    id: "openrouter",
    label: "OpenRouter",
    shortLabel: "OR",
    keyPlaceholder: "sk-or-...",
    defaultModel: "openai/gpt-4o-mini",
    models: [
      { id: "openai/gpt-4o-mini", name: "GPT-4o Mini", description: "OpenRouter" },
      { id: "anthropic/claude-sonnet-4", name: "Claude Sonnet", description: "OpenRouter" },
      { id: "google/gemini-2.5-pro", name: "Gemini 2.5 Pro", description: "OpenRouter" },
    ],
  },
];

export const DEFAULT_PROVIDER_ID: ProviderId = "deepseek";

export function normalizeProviderId(provider?: string | null): ProviderId {
  return PROVIDERS.some((item) => item.id === provider)
    ? (provider as ProviderId)
    : DEFAULT_PROVIDER_ID;
}

export function getProviderDefinition(provider?: string | null): ProviderDefinition {
  const id = normalizeProviderId(provider);
  return PROVIDERS.find((item) => item.id === id) ?? PROVIDERS[0];
}

export function getProviderLabel(provider?: string | null): string {
  return getProviderDefinition(provider).label;
}

export function getDefaultModel(provider?: string | null): string {
  return getProviderDefinition(provider).defaultModel;
}

export function modelBelongsToProvider(provider: string | null | undefined, model?: string | null): boolean {
  if (!model) return false;
  return getProviderDefinition(provider).models.some((item) => item.id === model);
}

export function getProviderForModel(model?: string | null): ProviderId | null {
  if (!model) return null;
  return PROVIDERS.find((provider) => provider.models.some((item) => item.id === model))?.id ?? null;
}

export function getModelLabel(model?: string | null): string {
  if (!model) return "未选择模型";
  for (const provider of PROVIDERS) {
    const found = provider.models.find((item) => item.id === model);
    if (found) return found.name;
  }
  return model;
}

export function getModelContextWindow(model?: string | null): number | null {
  if (!model) return null;
  for (const provider of PROVIDERS) {
    const found = provider.models.find((item) => item.id === model);
    if (found?.contextWindowTokens) return found.contextWindowTokens;
  }
  return null;
}

export function formatContextWindow(tokens?: number | null): string {
  if (!tokens) return "";
  if (tokens >= 1_000_000) return `${Math.round(tokens / 1_000_000)}M`;
  if (tokens >= 1_000) return `${Math.round(tokens / 1_000)}K`;
  return String(tokens);
}

export function getProviderModelLabel(provider?: string | null, model?: string | null): string {
  const providerDef = getProviderDefinition(provider);
  return `${providerDef.shortLabel} · ${getModelLabel(model || providerDef.defaultModel)}`;
}
