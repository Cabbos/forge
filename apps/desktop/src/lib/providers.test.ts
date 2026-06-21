import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  DEFAULT_PROVIDER_ID,
  PROVIDERS,
  formatContextWindow,
  getDefaultModel,
  deriveProviderEvidenceSummary,
  getModelContextWindow,
  getModelLabel,
  getProviderDefinition,
  getProviderForModel,
  mergeProviderCatalog,
  getProviderModelLabel,
  modelBelongsToProvider,
  normalizeProviderId,
} from "./providers.ts";

const RUST_BUILTIN_PROVIDER_IDS = [
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
];

const RUST_DEFAULT_MODELS = {
  deepseek: "deepseek-v4-flash[1m]",
  anthropic: "claude-sonnet-4-6",
  kimi: "kimi-k2.7-code",
  glm: "glm-5.2",
  alibaba: "qwen3-coder-plus",
  minimax: "MiniMax-M2.7",
  openai: "gpt-4o",
  openrouter: "openai/gpt-4o-mini",
  gemini: "gemini-2.5-pro",
  xai: "grok-4",
  groq: "llama-3.3-70b-versatile",
  mistral: "mistral-large-latest",
  ollama: "llama3.1",
  custom_openai: "custom-model",
  custom_anthropic: "custom-model",
} as const;

describe("frontend provider catalog", () => {
  it("matches the Rust built-in provider id set without adding NVIDIA", () => {
    assert.deepEqual(PROVIDERS.map((provider) => provider.id), RUST_BUILTIN_PROVIDER_IDS);
    assert.equal(PROVIDERS.map((provider) => String(provider.id)).includes("nvidia"), false);
    assert.equal(DEFAULT_PROVIDER_ID, "deepseek");
  });

  it("keeps default models aligned with the Rust registry", () => {
    for (const [provider, model] of Object.entries(RUST_DEFAULT_MODELS)) {
      assert.equal(getDefaultModel(provider), model, provider);
      assert.equal(getProviderDefinition(provider).defaultModel, model, provider);
      assert.equal(
        getProviderDefinition(provider).models.some((option) => option.id === model),
        true,
        `${provider} default model must be present in model options`,
      );
    }
  });

  it("normalizes registry aliases used by settings and profile defaults", () => {
    assert.equal(normalizeProviderId("claude"), "anthropic");
    assert.equal(normalizeProviderId("gpt"), "openai");
    assert.equal(normalizeProviderId("moonshot"), "kimi");
    assert.equal(normalizeProviderId("zhipu"), "glm");
    assert.equal(normalizeProviderId("z.ai"), "glm");
    assert.equal(normalizeProviderId("qwen"), "alibaba");
    assert.equal(normalizeProviderId("dashscope"), "alibaba");
    assert.equal(normalizeProviderId("grok"), "xai");
    assert.equal(normalizeProviderId("x.ai"), "xai");
    assert.equal(normalizeProviderId("local"), "ollama");
    assert.equal(normalizeProviderId("lmstudio"), "ollama");
    assert.equal(normalizeProviderId("vllm"), "ollama");
    assert.equal(normalizeProviderId("llama.cpp"), "ollama");
    assert.equal(normalizeProviderId("custom-openai"), "custom_openai");
    assert.equal(normalizeProviderId("custom-anthropic"), "custom_anthropic");
    assert.equal(normalizeProviderId(""), "deepseek");
    assert.equal(normalizeProviderId("unknown-provider"), "unknown-provider");
  });

  it("resolves model ownership for defaults and registry fallback models", () => {
    assert.equal(getProviderForModel("kimi-k2.7-code"), "kimi");
    assert.equal(getProviderForModel("kimi-k2.5"), "kimi");
    assert.equal(getProviderForModel("glm-5.2"), "glm");
    assert.equal(getProviderForModel("glm-4.7"), "glm");
    assert.equal(getProviderForModel("qwen-plus"), "alibaba");
    assert.equal(getProviderForModel("MiniMax-M1"), "minimax");
    assert.equal(getProviderForModel("gemini-2.5-flash"), "gemini");
    assert.equal(getProviderForModel("grok-3"), "xai");
    assert.equal(getProviderForModel("openai/gpt-oss-120b"), "groq");
    assert.equal(getProviderForModel("codestral-latest"), "mistral");
    assert.equal(getProviderForModel("qwen2.5-coder"), "ollama");
    assert.equal(getProviderForModel("nvidia/llama-3.1-nemotron"), null);
  });

  it("reports model membership and context windows consistently", () => {
    assert.equal(modelBelongsToProvider("moonshot", "kimi-k2.7-code"), true);
    assert.equal(modelBelongsToProvider("zhipu", "glm-5.2"), true);
    assert.equal(modelBelongsToProvider("qwen", "qwen-max"), true);
    assert.equal(modelBelongsToProvider("openai", "kimi-k2.7-code"), false);

    assert.equal(getModelContextWindow("deepseek-v4-flash[1m]"), 1_000_000);
    assert.equal(getModelContextWindow("deepseek-chat"), 128_000);
    assert.equal(getModelContextWindow("claude-sonnet-4-6"), 200_000);
    assert.equal(getModelContextWindow("kimi-k2.7-code"), 262_144);
    assert.equal(getModelContextWindow("glm-5.2"), 1_000_000);
    assert.equal(getModelContextWindow("gemini-2.5-pro"), 1_000_000);
    assert.equal(getModelContextWindow("llama3.1"), null);
    assert.equal(formatContextWindow(getModelContextWindow("gemini-2.5-pro")), "1M");
  });

  it("keeps custom providers flexible without hardcoded model lists", () => {
    assert.equal(modelBelongsToProvider("custom_openai", "my-private-model"), true);
    assert.equal(modelBelongsToProvider("custom-anthropic", "claude-proxy-model"), true);
    assert.equal(getProviderForModel("my-private-model"), null);
    assert.equal(getModelLabel("my-private-model"), "my-private-model");
    assert.equal(
      getProviderModelLabel("custom_openai", "my-private-model"),
      "Custom OpenAI · my-private-model",
    );
  });

  it("merges configured provider profiles into the frontend catalog", () => {
    const catalog = mergeProviderCatalog([
      {
        id: "nvidia",
        label: "NVIDIA NIM",
        default_model: "nvidia/llama-3.1-nemotron",
        context_window_tokens: null,
        aliases: ["nim"],
        requires_api_key: true,
        supports_streaming: true,
        supports_tools: true,
      },
      {
        id: "local-openai",
        label: "Local OpenAI",
        default_model: "local-model",
        context_window_tokens: null,
        aliases: [],
        requires_api_key: false,
        supports_streaming: true,
        supports_tools: true,
      },
    ]);

    assert.equal(catalog.some((provider) => provider.id === "nvidia"), true);
    assert.equal(catalog.some((provider) => provider.id === "local-openai"), true);
    assert.equal(getProviderDefinition("nvidia", catalog).label, "NVIDIA NIM");
    assert.equal(getDefaultModel("local-openai", catalog), "local-model");
    assert.equal(modelBelongsToProvider("nvidia", "nvidia/llama-3.1-nemotron", catalog), true);
    assert.equal(modelBelongsToProvider("local-openai", "anything-local", catalog), true);
    assert.equal(normalizeProviderId("nvidia", catalog), "nvidia");
    assert.equal(normalizeProviderId("unknown-provider"), "unknown-provider");
  });

  it("merges cached provider model catalogs into selectable model options", () => {
    const catalog = mergeProviderCatalog([
      {
        id: "nvidia",
        label: "NVIDIA NIM",
        default_model: "nvidia/llama-3.1-nemotron",
        context_window_tokens: null,
        aliases: ["nim"],
        requires_api_key: true,
        supports_streaming: true,
        supports_tools: true,
        model_catalog_source: "static_fallback",
        probe_evidence: {
          source: "manual_probe",
          status: "passed",
          model: "nvidia/llama-3.1-nemotron",
          base_url: "https://integrate.api.nvidia.com/v1",
          checks: [
            { id: "tool_schema_accepted", label: "Tool schema accepted", status: "passed" },
          ],
        },
        models: [
          {
            id: "nvidia/llama-3.1-nemotron",
            name: "NVIDIA Nemotron",
            context_window_tokens: null,
          },
          {
            id: "nvidia/llama-3.3-70b",
            name: "NVIDIA Llama 3.3 70B",
            context_window_tokens: 128_000,
          },
        ],
      },
    ]);

    const nvidia = getProviderDefinition("nim", catalog);
    assert.equal(nvidia.modelCatalogSource, "static_fallback");
    assert.equal(nvidia.probeEvidence?.source, "manual_probe");
    assert.equal(nvidia.probeEvidence?.status, "passed");
    assert.equal(nvidia.probeEvidence?.checks[0]?.id, "tool_schema_accepted");
    assert.equal(nvidia.models.map((model) => model.id).includes("nvidia/llama-3.3-70b"), true);
    assert.equal(getProviderForModel("nvidia/llama-3.3-70b", catalog), "nvidia");
    assert.equal(getModelLabel("nvidia/llama-3.3-70b", catalog), "NVIDIA Llama 3.3 70B");
    assert.equal(getModelContextWindow("nvidia/llama-3.3-70b", catalog), 128_000);
    assert.equal(modelBelongsToProvider("nvidia", "nvidia/llama-3.3-70b", catalog), true);
  });

  it("derives provider evidence summaries from manual probe and catalog source", () => {
    const liveReady = deriveProviderEvidenceSummary({
      id: "nvidia",
      label: "NVIDIA NIM",
      shortLabel: "NVIDIA",
      keyPlaceholder: "sk-...",
      defaultModel: "nvidia/llama-3.1-nemotron",
      models: [{ id: "nvidia/llama-3.1-nemotron", name: "NVIDIA Nemotron" }],
      requiresApiKey: true,
      modelCatalogSource: "live_endpoint",
      probeEvidence: {
        source: "manual_probe",
        status: "passed",
        recorded_at_ms: 1_717_891_200_000,
        model: "nvidia/llama-3.1-nemotron",
        base_url: "https://integrate.api.nvidia.com/v1",
        checks: [
          { id: "streaming_accepted", label: "Streaming accepted", status: "passed" },
          { id: "tool_schema_accepted", label: "Tool schema accepted", status: "passed" },
        ],
      },
    });
    assert.equal(liveReady.tone, "ready");
    assert.equal(liveReady.label, "证据较强");
    assert.equal(liveReady.detail, "手动检测通过 · 检测 2024-06-09 · 目录 Live /models");

    const staticFallback = deriveProviderEvidenceSummary({
      id: "kimi",
      label: "Kimi / Moonshot",
      shortLabel: "Kimi",
      keyPlaceholder: "sk-...",
      defaultModel: "kimi-k2.7-code",
      models: [{ id: "kimi-k2.7-code", name: "Kimi K2.7 Code" }],
      requiresApiKey: true,
      modelCatalogSource: "static_fallback",
    });
    assert.equal(staticFallback.tone, "warning");
    assert.equal(staticFallback.label, "需要手动检测");
    assert.equal(staticFallback.detail, "尚未手动检测 · 目录 static fallback");

    const failedProbe = deriveProviderEvidenceSummary({
      id: "openai",
      label: "OpenAI",
      shortLabel: "GPT",
      keyPlaceholder: "sk-...",
      defaultModel: "gpt-4o",
      models: [{ id: "gpt-4o", name: "GPT-4o" }],
      requiresApiKey: true,
      probeEvidence: {
        source: "manual_probe",
        status: "failed",
        recorded_at_ms: null,
        model: "gpt-4o",
        base_url: "https://api.openai.com/v1",
        checks: [{ id: "key_present", label: "Key present", status: "failed" }],
      },
    });
    assert.equal(failedProbe.tone, "blocked");
    assert.equal(failedProbe.label, "检测失败");
    assert.equal(failedProbe.detail, "手动检测失败 · 检测时间未知 · 目录未验证");
  });
});
