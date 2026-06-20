import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  DEFAULT_PROVIDER_ID,
  PROVIDERS,
  formatContextWindow,
  getDefaultModel,
  getModelContextWindow,
  getModelLabel,
  getProviderDefinition,
  getProviderForModel,
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
  kimi: "kimi-k2.5",
  glm: "glm-4.5",
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
    assert.equal(normalizeProviderId("unknown-provider"), "deepseek");
  });

  it("resolves model ownership for defaults and registry fallback models", () => {
    assert.equal(getProviderForModel("kimi-k2"), "kimi");
    assert.equal(getProviderForModel("moonshot-v1-32k"), "kimi");
    assert.equal(getProviderForModel("glm-4.5-air"), "glm");
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
    assert.equal(modelBelongsToProvider("moonshot", "kimi-k2.5"), true);
    assert.equal(modelBelongsToProvider("zhipu", "glm-4.5-air"), true);
    assert.equal(modelBelongsToProvider("qwen", "qwen-max"), true);
    assert.equal(modelBelongsToProvider("openai", "kimi-k2.5"), false);

    assert.equal(getModelContextWindow("deepseek-v4-flash[1m]"), 1_000_000);
    assert.equal(getModelContextWindow("deepseek-chat"), 128_000);
    assert.equal(getModelContextWindow("claude-sonnet-4-6"), 200_000);
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
});
