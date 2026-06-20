export const PROVIDER_IDS = [
  "deepseek",
  "anthropic",
  "kimi",
  "glm",
  "alibaba",
  "minimax",
  "openai",
  "openrouter",
  "gemini",
  "xai",
  "groq",
  "mistral",
  "ollama",
  "custom_openai",
  "custom_anthropic",
] as const;

export type BuiltInProviderId = (typeof PROVIDER_IDS)[number];
export type ProviderId = BuiltInProviderId | (string & {});

export interface ProviderCatalogEntry {
  id: string;
  label: string;
  default_model: string;
  context_window_tokens?: number | null;
  models?: ProviderCatalogModelEntry[];
  aliases?: string[];
  requires_api_key?: boolean;
  supports_streaming?: boolean;
  supports_tools?: boolean;
  source?: ProviderProfileSource;
  base_url?: string | null;
  transport?: ProviderTransportName;
  api_key_env?: string[];
  base_url_env?: string[];
  model_catalog_source?: ProviderModelCatalogSource | null;
}

export interface ProviderCatalogModelEntry {
  id: string;
  name: string;
  context_window_tokens?: number | null;
}

export type ProviderProfileSource = "built_in" | "user_override" | "user_defined";
export type ProviderModelCatalogSource = "live_endpoint" | "static_fallback" | "unsupported";
export type ProviderTransportName =
  | "anthropic_messages"
  | "openai_chat_completions"
  | "openai_responses"
  | "native_gemini"
  | "bedrock_converse"
  | "custom_openai_compatible"
  | "custom_anthropic_compatible";

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
  aliases?: string[];
  requiresApiKey?: boolean;
  customModels?: boolean;
  source?: ProviderProfileSource;
  baseUrl?: string | null;
  transport?: ProviderTransportName;
  apiKeyEnv?: string[];
  baseUrlEnv?: string[];
  modelCatalogSource?: ProviderModelCatalogSource | null;
  supportsStreaming?: boolean;
  supportsTools?: boolean;
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
      { id: "deepseek-v4-pro", name: "DeepSeek V4 Pro", description: "复杂任务，支持 1M 上下文", contextWindowTokens: 1_000_000 },
      { id: "deepseek-chat", name: "DeepSeek Chat", description: "通用对话", contextWindowTokens: 128_000 },
    ],
  },
  {
    id: "anthropic",
    label: "Anthropic",
    shortLabel: "Claude",
    keyPlaceholder: "sk-ant-...",
    defaultModel: "claude-sonnet-4-6",
    models: [
      { id: "claude-sonnet-4-6", name: "Claude Sonnet 4.6", description: "主力编码", contextWindowTokens: 200_000 },
      { id: "claude-opus-4-8", name: "Claude Opus 4.8", description: "深度推理", contextWindowTokens: 200_000 },
      { id: "claude-opus-4-7", name: "Claude Opus 4.7", description: "历史配置兼容", contextWindowTokens: 200_000 },
      { id: "claude-haiku-4-5-20251001", name: "Claude Haiku 4.5", description: "轻量快速", contextWindowTokens: 200_000 },
    ],
  },
  {
    id: "kimi",
    label: "Kimi / Moonshot",
    shortLabel: "Kimi",
    keyPlaceholder: "sk-...",
    defaultModel: "kimi-k2.7-code",
    models: [
      { id: "kimi-k2.7-code", name: "Kimi K2.7 Code", description: "Moonshot coding preset", contextWindowTokens: 262_144 },
      { id: "kimi-k2.5", name: "Kimi K2.5", description: "Moonshot fallback", contextWindowTokens: 262_144 },
      { id: "kimi-k2", name: "Kimi K2", description: "legacy fallback", contextWindowTokens: 128_000 },
    ],
  },
  {
    id: "glm",
    label: "GLM / Zhipu",
    shortLabel: "GLM",
    keyPlaceholder: "sk-...",
    defaultModel: "glm-5.2",
    models: [
      { id: "glm-5.2", name: "GLM 5.2", description: "Zhipu coding preset", contextWindowTokens: 1_000_000 },
      { id: "glm-5.1", name: "GLM 5.1", description: "long-horizon fallback", contextWindowTokens: 200_000 },
      { id: "glm-5", name: "GLM 5", description: "agentic fallback", contextWindowTokens: 200_000 },
      { id: "glm-4.7", name: "GLM 4.7", description: "legacy coding fallback", contextWindowTokens: 200_000 },
    ],
  },
  {
    id: "alibaba",
    label: "Alibaba / Qwen",
    shortLabel: "Qwen",
    keyPlaceholder: "sk-...",
    defaultModel: "qwen3-coder-plus",
    models: [
      { id: "qwen3-coder-plus", name: "Qwen3 Coder Plus", description: "DashScope compatible mode", contextWindowTokens: 128_000 },
      { id: "qwen-max", name: "Qwen Max", description: "通用高能力", contextWindowTokens: 128_000 },
      { id: "qwen-plus", name: "Qwen Plus", description: "通用任务", contextWindowTokens: 128_000 },
    ],
  },
  {
    id: "minimax",
    label: "MiniMax",
    shortLabel: "MiniMax",
    keyPlaceholder: "sk-...",
    defaultModel: "MiniMax-M2.7",
    models: [
      { id: "MiniMax-M2.7", name: "MiniMax M2.7", description: "Anthropic-compatible preset", contextWindowTokens: 128_000 },
      { id: "MiniMax-M1", name: "MiniMax M1", description: "fallback", contextWindowTokens: 128_000 },
    ],
  },
  {
    id: "openai",
    label: "OpenAI",
    shortLabel: "GPT",
    keyPlaceholder: "sk-...",
    defaultModel: "gpt-4o",
    models: [
      { id: "gpt-4o", name: "GPT-4o", description: "通用任务", contextWindowTokens: 128_000 },
      { id: "gpt-4o-mini", name: "GPT-4o Mini", description: "轻量快速", contextWindowTokens: 128_000 },
      { id: "gpt-4.1", name: "GPT-4.1", description: "历史配置兼容", contextWindowTokens: 128_000 },
    ],
  },
  {
    id: "openrouter",
    label: "OpenRouter",
    shortLabel: "OR",
    keyPlaceholder: "sk-or-...",
    defaultModel: "openai/gpt-4o-mini",
    models: [
      { id: "openai/gpt-4o-mini", name: "GPT-4o Mini", description: "OpenRouter", contextWindowTokens: 128_000 },
      { id: "anthropic/claude-sonnet-4", name: "Claude Sonnet", description: "OpenRouter", contextWindowTokens: 128_000 },
      { id: "google/gemini-2.5-pro", name: "Gemini 2.5 Pro", description: "OpenRouter", contextWindowTokens: 128_000 },
    ],
  },
  {
    id: "gemini",
    label: "Gemini",
    shortLabel: "Gemini",
    keyPlaceholder: "AIza...",
    defaultModel: "gemini-2.5-pro",
    models: [
      { id: "gemini-2.5-pro", name: "Gemini 2.5 Pro", description: "OpenAI-compatible route", contextWindowTokens: 1_000_000 },
      { id: "gemini-2.5-flash", name: "Gemini 2.5 Flash", description: "轻量快速", contextWindowTokens: 1_000_000 },
    ],
  },
  {
    id: "xai",
    label: "xAI",
    shortLabel: "Grok",
    keyPlaceholder: "xai-...",
    defaultModel: "grok-4",
    models: [
      { id: "grok-4", name: "Grok 4", description: "xAI preset", contextWindowTokens: 128_000 },
      { id: "grok-3", name: "Grok 3", description: "fallback", contextWindowTokens: 128_000 },
    ],
  },
  {
    id: "groq",
    label: "Groq",
    shortLabel: "Groq",
    keyPlaceholder: "gsk_...",
    defaultModel: "llama-3.3-70b-versatile",
    models: [
      { id: "llama-3.3-70b-versatile", name: "Llama 3.3 70B Versatile", description: "Groq preset", contextWindowTokens: 128_000 },
      { id: "openai/gpt-oss-120b", name: "GPT OSS 120B", description: "fallback", contextWindowTokens: 128_000 },
    ],
  },
  {
    id: "mistral",
    label: "Mistral",
    shortLabel: "Mistral",
    keyPlaceholder: "sk-...",
    defaultModel: "mistral-large-latest",
    models: [
      { id: "mistral-large-latest", name: "Mistral Large", description: "Mistral preset", contextWindowTokens: 128_000 },
      { id: "codestral-latest", name: "Codestral", description: "coding fallback", contextWindowTokens: 128_000 },
    ],
  },
  {
    id: "ollama",
    label: "Ollama",
    shortLabel: "Local",
    keyPlaceholder: "not required",
    defaultModel: "llama3.1",
    models: [
      { id: "llama3.1", name: "Llama 3.1", description: "local default" },
      { id: "qwen2.5-coder", name: "Qwen2.5 Coder", description: "local coding fallback" },
      { id: "gpt-oss", name: "GPT OSS", description: "local fallback" },
    ],
  },
  {
    id: "custom_openai",
    label: "Custom OpenAI-Compatible",
    shortLabel: "Custom OpenAI",
    keyPlaceholder: "sk-...",
    defaultModel: "custom-model",
    models: [
      { id: "custom-model", name: "Custom Model", description: "user supplied" },
    ],
  },
  {
    id: "custom_anthropic",
    label: "Custom Anthropic-Compatible",
    shortLabel: "Custom Claude",
    keyPlaceholder: "sk-...",
    defaultModel: "custom-model",
    models: [
      { id: "custom-model", name: "Custom Model", description: "user supplied" },
    ],
  },
];

export const DEFAULT_PROVIDER_ID: ProviderId = "deepseek";

const PROVIDER_ID_SET = new Set<ProviderId>(PROVIDER_IDS);

const CUSTOM_PROVIDER_IDS = new Set<ProviderId>([
  "custom_openai",
  "custom_anthropic",
]);

const PROVIDER_ALIASES: Record<string, ProviderId> = {
  deepseek: "deepseek",
  anthropic: "anthropic",
  claude: "anthropic",
  kimi: "kimi",
  moonshot: "kimi",
  glm: "glm",
  zhipu: "glm",
  "z.ai": "glm",
  zai: "glm",
  "z-ai": "glm",
  alibaba: "alibaba",
  qwen: "alibaba",
  dashscope: "alibaba",
  minimax: "minimax",
  openai: "openai",
  gpt: "openai",
  openrouter: "openrouter",
  gemini: "gemini",
  google: "gemini",
  xai: "xai",
  grok: "xai",
  "x.ai": "xai",
  groq: "groq",
  mistral: "mistral",
  ollama: "ollama",
  local: "ollama",
  vllm: "ollama",
  lmstudio: "ollama",
  "llama.cpp": "ollama",
  custom_openai: "custom_openai",
  "custom-openai": "custom_openai",
  openai_compatible: "custom_openai",
  custom_anthropic: "custom_anthropic",
  "custom-anthropic": "custom_anthropic",
  anthropic_compatible: "custom_anthropic",
};

export function mergeProviderCatalog(
  entries: ProviderCatalogEntry[] = [],
  base: ProviderDefinition[] = PROVIDERS,
): ProviderDefinition[] {
  const merged = base.map((provider) => ({
    ...provider,
    models: provider.models.map((model) => ({ ...model })),
    aliases: [...(provider.aliases ?? [])],
  }));
  const indexById = new Map(merged.map((provider, index) => [provider.id, index]));

  for (const entry of entries) {
    const id = entry.id.trim().toLowerCase();
    const defaultModel = entry.default_model.trim();
    if (!id || !defaultModel) continue;
    const catalogModels = providerCatalogModelOptions(entry);
    const modelOption: ModelOption = {
      id: defaultModel,
      name: catalogModels.find((model) => model.id === defaultModel)?.name || defaultModel,
      description: entry.supports_tools === false ? "configured profile" : "configured provider",
      contextWindowTokens: entry.context_window_tokens ?? undefined,
    };
    const existingIndex = indexById.get(id);
    if (existingIndex !== undefined) {
      const existing = merged[existingIndex];
      merged[existingIndex] = {
        ...existing,
        label: entry.label || existing.label,
        shortLabel: existing.shortLabel || entry.label || id,
        defaultModel,
        aliases: [...new Set([...(existing.aliases ?? []), ...(entry.aliases ?? [])])],
        requiresApiKey: entry.requires_api_key ?? existing.requiresApiKey,
        customModels: existing.customModels,
        source: entry.source ?? existing.source,
        baseUrl: entry.base_url ?? existing.baseUrl,
        transport: entry.transport ?? existing.transport,
        apiKeyEnv: entry.api_key_env ?? existing.apiKeyEnv,
        baseUrlEnv: entry.base_url_env ?? existing.baseUrlEnv,
        modelCatalogSource: entry.model_catalog_source ?? existing.modelCatalogSource ?? null,
        supportsStreaming: entry.supports_streaming ?? existing.supportsStreaming,
        supportsTools: entry.supports_tools ?? existing.supportsTools,
        models: mergeModelOptions(existing.models, modelOption, catalogModels),
      };
      continue;
    }

    indexById.set(id, merged.length);
    merged.push({
      id,
      label: entry.label || id,
      shortLabel: entry.label || id,
      keyPlaceholder: entry.requires_api_key === false ? "not required" : "sk-...",
      defaultModel,
      models: mergeModelOptions([], modelOption, catalogModels),
      aliases: entry.aliases ?? [],
      requiresApiKey: entry.requires_api_key ?? true,
      customModels: true,
      source: entry.source,
      baseUrl: entry.base_url,
      transport: entry.transport,
      apiKeyEnv: entry.api_key_env,
      baseUrlEnv: entry.base_url_env,
      modelCatalogSource: entry.model_catalog_source ?? null,
      supportsStreaming: entry.supports_streaming,
      supportsTools: entry.supports_tools,
    });
  }

  return merged;
}

function providerCatalogModelOptions(entry: ProviderCatalogEntry): ModelOption[] {
  const options: ModelOption[] = [];
  for (const model of entry.models ?? []) {
    const id = model.id.trim();
    if (!id) continue;
    const name = model.name.trim() || id;
    options.push({
      id,
      name,
      description: "refreshed catalog",
      contextWindowTokens: model.context_window_tokens ?? entry.context_window_tokens ?? undefined,
    });
  }
  return options;
}

function mergeModelOptions(
  existingModels: ModelOption[],
  defaultModel: ModelOption,
  catalogModels: ModelOption[],
): ModelOption[] {
  const merged = existingModels.map((model) => ({ ...model }));
  const seen = new Set(merged.map((model) => model.id));
  if (!seen.has(defaultModel.id)) {
    merged.unshift({ ...defaultModel });
    seen.add(defaultModel.id);
  }
  for (const model of catalogModels) {
    if (seen.has(model.id)) continue;
    merged.push({ ...model });
    seen.add(model.id);
  }
  return merged;
}

function providerAliasMap(catalog: ProviderDefinition[]): Record<string, ProviderId> {
  const aliases: Record<string, ProviderId> = { ...PROVIDER_ALIASES };
  for (const provider of catalog) {
    aliases[String(provider.id).toLowerCase()] = provider.id;
    for (const alias of provider.aliases ?? []) {
      aliases[alias.trim().toLowerCase()] = provider.id;
    }
  }
  return aliases;
}

export function normalizeProviderId(
  provider?: string | null,
  catalog: ProviderDefinition[] = PROVIDERS,
): ProviderId {
  const normalized = provider?.trim().toLowerCase();
  if (!normalized) return DEFAULT_PROVIDER_ID;
  if (PROVIDER_ID_SET.has(normalized as ProviderId)) return normalized as ProviderId;
  return providerAliasMap(catalog)[normalized] ?? normalized;
}

function fallbackProviderDefinition(provider: ProviderId): ProviderDefinition {
  const id = String(provider || DEFAULT_PROVIDER_ID);
  return {
    id,
    label: id,
    shortLabel: id,
    keyPlaceholder: "sk-...",
    defaultModel: "custom-model",
    models: [{ id: "custom-model", name: "Custom Model", description: "configured provider" }],
    customModels: true,
  };
}

export function getProviderDefinition(
  provider?: string | null,
  catalog: ProviderDefinition[] = PROVIDERS,
): ProviderDefinition {
  const id = normalizeProviderId(provider, catalog);
  return catalog.find((item) => item.id === id) ?? fallbackProviderDefinition(id);
}

export function getProviderLabel(
  provider?: string | null,
  catalog: ProviderDefinition[] = PROVIDERS,
): string {
  return getProviderDefinition(provider, catalog).label;
}

export function getDefaultModel(
  provider?: string | null,
  catalog: ProviderDefinition[] = PROVIDERS,
): string {
  return getProviderDefinition(provider, catalog).defaultModel;
}

export function modelBelongsToProvider(
  provider: string | null | undefined,
  model?: string | null,
  catalog: ProviderDefinition[] = PROVIDERS,
): boolean {
  const cleanModel = model?.trim();
  if (!cleanModel) return false;
  const providerDef = getProviderDefinition(provider, catalog);
  if (CUSTOM_PROVIDER_IDS.has(providerDef.id) || providerDef.customModels) return true;
  return providerDef.models.some((item) => item.id === cleanModel);
}

export function getProviderForModel(
  model?: string | null,
  catalog: ProviderDefinition[] = PROVIDERS,
): ProviderId | null {
  const cleanModel = model?.trim();
  if (!cleanModel) return null;
  return catalog.find((provider) =>
    provider.models.some((item) => item.id === cleanModel)
  )?.id ?? null;
}

export function getModelLabel(model?: string | null, catalog: ProviderDefinition[] = PROVIDERS): string {
  const cleanModel = model?.trim();
  if (!cleanModel) return "未选择模型";
  for (const provider of catalog) {
    const found = provider.models.find((item) => item.id === cleanModel);
    if (found) return found.name;
  }
  return cleanModel;
}

export function getModelContextWindow(
  model?: string | null,
  catalog: ProviderDefinition[] = PROVIDERS,
): number | null {
  const cleanModel = model?.trim();
  if (!cleanModel) return null;
  for (const provider of catalog) {
    const found = provider.models.find((item) => item.id === cleanModel);
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

export function getProviderModelLabel(
  provider?: string | null,
  model?: string | null,
  catalog: ProviderDefinition[] = PROVIDERS,
): string {
  const providerDef = getProviderDefinition(provider, catalog);
  return `${providerDef.shortLabel} · ${getModelLabel(model || providerDef.defaultModel, catalog)}`;
}
