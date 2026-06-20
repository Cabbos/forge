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

export type ProviderId = (typeof PROVIDER_IDS)[number];

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
    defaultModel: "kimi-k2.5",
    models: [
      { id: "kimi-k2.5", name: "Kimi K2.5", description: "Moonshot coding preset", contextWindowTokens: 128_000 },
      { id: "kimi-k2", name: "Kimi K2", description: "Moonshot fallback", contextWindowTokens: 128_000 },
      { id: "moonshot-v1-32k", name: "Moonshot V1 32K", description: "OpenAI-compatible fallback", contextWindowTokens: 128_000 },
    ],
  },
  {
    id: "glm",
    label: "GLM / Zhipu",
    shortLabel: "GLM",
    keyPlaceholder: "sk-...",
    defaultModel: "glm-4.5",
    models: [
      { id: "glm-4.5", name: "GLM 4.5", description: "Zhipu coding preset", contextWindowTokens: 128_000 },
      { id: "glm-4.5-air", name: "GLM 4.5 Air", description: "轻量快速", contextWindowTokens: 128_000 },
      { id: "glm-4-plus", name: "GLM 4 Plus", description: "历史兼容", contextWindowTokens: 128_000 },
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

export function normalizeProviderId(provider?: string | null): ProviderId {
  const normalized = provider?.trim().toLowerCase();
  if (!normalized) return DEFAULT_PROVIDER_ID;
  if (PROVIDER_ID_SET.has(normalized as ProviderId)) return normalized as ProviderId;
  return PROVIDER_ALIASES[normalized] ?? DEFAULT_PROVIDER_ID;
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
  const cleanModel = model?.trim();
  if (!cleanModel) return false;
  const providerDef = getProviderDefinition(provider);
  if (CUSTOM_PROVIDER_IDS.has(providerDef.id)) return true;
  return providerDef.models.some((item) => item.id === cleanModel);
}

export function getProviderForModel(model?: string | null): ProviderId | null {
  const cleanModel = model?.trim();
  if (!cleanModel) return null;
  return PROVIDERS.find((provider) =>
    provider.models.some((item) => item.id === cleanModel)
  )?.id ?? null;
}

export function getModelLabel(model?: string | null): string {
  const cleanModel = model?.trim();
  if (!cleanModel) return "未选择模型";
  for (const provider of PROVIDERS) {
    const found = provider.models.find((item) => item.id === cleanModel);
    if (found) return found.name;
  }
  return cleanModel;
}

export function getModelContextWindow(model?: string | null): number | null {
  const cleanModel = model?.trim();
  if (!cleanModel) return null;
  for (const provider of PROVIDERS) {
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

export function getProviderModelLabel(provider?: string | null, model?: string | null): string {
  const providerDef = getProviderDefinition(provider);
  return `${providerDef.shortLabel} · ${getModelLabel(model || providerDef.defaultModel)}`;
}
