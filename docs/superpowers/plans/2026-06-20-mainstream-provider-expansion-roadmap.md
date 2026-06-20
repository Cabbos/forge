# Mainstream Provider Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand Forge Desktop from the current DeepSeek/Anthropic/OpenAI/OpenRouter set into a mainstream model provider runtime that can support Claude, DeepSeek, Kimi/Moonshot, GLM/Zhipu, OpenAI, Gemini, xAI/Grok, Groq, Mistral, Ollama/local, and compatible gateways without rewriting the agent loop for each provider.

**Architecture:** Keep Forge's internal agent contract Anthropic-style: ordered messages, tool-use/tool-result adjacency, streaming text/tool deltas, usage facts, thinking/reasoning facts, and provider capability metadata. Route each provider through the most faithful transport: Anthropic Messages-compatible first when officially supported, OpenAI Responses or Chat Completions-compatible where that is the provider's contract, and native adapters only for features that cannot be represented safely through a compatibility endpoint.

**Tech Stack:** Rust/Tauri backend in `apps/desktop/src-tauri`, React/TypeScript frontend in `apps/desktop/src`, existing `AnthropicAdapter`, `OpenAiCompatibleAdapter`, provider settings UI, GitNexus impact analysis, Vitest/Rust tests, and `scripts/acceptance.sh --dry-run`.

---

## Current State

Forge already has the right adapter split, but provider knowledge is scattered:

- `apps/desktop/src-tauri/src/adapters/mod.rs` routes `deepseek` and `anthropic` into `AnthropicAdapter`, and `openai` and `openrouter` into `OpenAiCompatibleAdapter`.
- `apps/desktop/src-tauri/src/adapters/base.rs` already normalizes chat around Anthropic-like content blocks and has `repair_tool_result_adjacency(...)`, which is exactly the internal contract we should preserve.
- `apps/desktop/src-tauri/src/agent/provider_capabilities.rs` owns provider normalization, default models, labels, and context windows.
- `apps/desktop/src-tauri/src/settings.rs` owns known providers and API key/base URL detection.
- `apps/desktop/src/lib/providers.ts` mirrors provider/model metadata for the UI.

The risk is not lack of adapters. The risk is drift: backend routing, frontend options, credential detection, model defaults, pricing, usage flags, and tests can disagree.

## Hermes Reference Pattern

Local Hermes was inspected at `/Users/cabbos/.hermes/hermes-agent`. The key finding is that Hermes does not scale provider support by adding one hardcoded adapter per vendor. It combines:

- `hermes_cli/providers.py`: a provider identity overlay with transport, auth type, env vars, aliases, aggregator flags, and base URL overrides.
- `providers/base.py`: a declarative `ProviderProfile` with hooks for message preparation, extra request body, top-level kwargs, model fetching, max token defaults, vision support, and provider quirks.
- `plugins/model-providers/<name>/`: self-registering provider profile plugins. Bundled profiles include `anthropic`, `deepseek`, `kimi-coding`, `zai`/GLM, `gemini`, `minimax`, `alibaba`, `openrouter`, `xai`, `nvidia`, `bedrock`, `custom`, `huggingface`, `novita`, `gmi`, `xiaomi`, `stepfun`, `azure-foundry`, `openai-codex`, and others.
- `hermes_cli/runtime_provider.py`: runtime resolution that chooses `chat_completions`, `anthropic_messages`, `codex_responses`, `bedrock_converse`, or local/custom modes based on provider, URL, model family, config, and credentials.
- `agent/transports/chat_completions.py`: one OpenAI-compatible transport that delegates provider-specific request quirks back to the provider profile rather than branching in every call path.

Hermes' provider strength is therefore a profile system, not a giant adapter switch. Forge should adopt this shape in Rust: static built-in profiles first, then user-defined profiles, then optional remote catalog/cache later.

## External Facts Checked

- Anthropic's official TypeScript SDK targets Anthropic's Messages API and streaming/tooling surface, but Forge's provider runtime is currently Rust, so the practical implementation is REST via `reqwest`, not the npm SDK: https://platform.claude.com/docs/en/cli-sdks-libraries/sdks/typescript
- OpenAI recommends the Responses API for new text generation and reasoning work, while Chat Completions still exists for compatibility: https://developers.openai.com/api/docs/guides/text
- Gemini documents OpenAI-compatible access by changing API key/base URL, while also saying direct Gemini API is recommended for new Gemini-only integrations: https://ai.google.dev/gemini-api/docs/openai
- DeepSeek documents OpenAI/Anthropic-compatible API formats, which supports the current DeepSeek-through-Anthropic route: https://api-docs.deepseek.com/
- xAI documents OpenAI REST compatibility with `/v1/chat/completions`: https://docs.x.ai/developers/rest-api-reference/inference/chat
- Groq documents OpenAI compatibility and base URL `https://api.groq.com/openai/v1`: https://console.groq.com/docs/openai
- Mistral documents Chat Completion APIs following OpenAI-like request structure for migration: https://docs.mistral.ai/resources/migration-guides
- Ollama documents both OpenAI compatibility and Anthropic Messages compatibility for local models: https://docs.ollama.com/api/openai-compatibility and https://docs.ollama.com/api/anthropic-compatibility
- Kimi/Moonshot documents OpenAI-compatible migration and tool calls, and also documents Claude Code usage through `ANTHROPIC_BASE_URL=https://api.moonshot.cn/anthropic`: https://platform.moonshot.cn/docs/guide/migrating-from-openai-to-kimi and https://platform.moonshot.cn/docs/guide/agent-support
- GLM/Zhipu documents OpenAI-compatible usage with `https://open.bigmodel.cn/api/coding/paas/v4`, and also documents Anthropic protocol usage with `https://open.bigmodel.cn/api/anthropic`: https://docs.bigmodel.cn/cn/guide/develop/openai/introduction and https://docs.bigmodel.cn/cn/guide/develop/goose

## Product Decision

We should support mainstream providers, but we should not force every provider through the Anthropic SDK/API shape.

The right product contract is:

1. Forge's internal agent loop stays Anthropic-style because that maps naturally to tool-use adjacency, event replay, and durable runtime facts.
2. Provider routing is capability-based.
3. Anthropic-compatible transport is preferred only when the provider officially supports it or when the user configures a custom Anthropic-compatible endpoint.
4. OpenAI-compatible transport remains first-class for OpenAI, OpenRouter, xAI, Groq, Mistral-compatible usage, Gemini OpenAI-compatible mode, local OpenAI-compatible servers, and many gateways.
5. Native adapters are allowed only when compatibility endpoints lose critical features such as thinking/reasoning blocks, structured tool deltas, usage accounting, or model-specific controls.

This lets us say, truthfully: Forge supports mainstream providers through a stable agent-loop contract, not through a brittle one-SDK-fits-all assumption.

## Provider Routing Matrix

| Provider | Initial route | Base URL / shape | MVP stance |
| --- | --- | --- | --- |
| Anthropic / Claude | `AnthropicAdapter` | Anthropic Messages | First-class |
| DeepSeek | `AnthropicAdapter` | `https://api.deepseek.com/anthropic` | First-class, current default |
| Kimi / Moonshot | `AnthropicAdapter` for coding-agent preset; OpenAI-compatible fallback | `https://api.moonshot.cn/anthropic` or Moonshot OpenAI-compatible endpoint | First-class China preset after fixture tests |
| GLM / Zhipu | `AnthropicAdapter` for coding-agent preset; OpenAI-compatible fallback | `https://open.bigmodel.cn/api/anthropic` or `https://open.bigmodel.cn/api/coding/paas/v4` | First-class China preset after fixture tests |
| Alibaba / Qwen / DashScope | OpenAI-compatible preset | DashScope compatible-mode endpoint | First-class China preset after Kimi/GLM |
| MiniMax | Anthropic-compatible first, OpenAI-compatible for M-series where needed | MiniMax Anthropic/OpenAI-compatible endpoints | First-class China preset after fixture tests |
| OpenAI | `OpenAiCompatibleAdapter` now, evaluate Responses adapter next | `https://api.openai.com/v1` | First-class |
| OpenRouter | `OpenAiCompatibleAdapter` | `https://openrouter.ai/api/v1` | First-class gateway |
| Gemini | OpenAI-compatible preset first, native Gemini later if needed | Gemini OpenAI-compatible endpoint | Named preset after fixture tests |
| xAI / Grok | OpenAI-compatible preset | `https://api.x.ai/v1` | Named preset after fixture tests |
| Groq | OpenAI-compatible preset | `https://api.groq.com/openai/v1` | Named preset after fixture tests |
| Mistral | OpenAI-compatible or native Mistral preset | Mistral chat completion shape | Named preset after fixture tests |
| Ollama local | Anthropic-compatible or OpenAI-compatible custom preset | `http://localhost:11434` | Local preset, no cloud claim |
| NVIDIA NIM | OpenAI-compatible preset | NVIDIA NIM endpoint | Named preset after mainstream MVP |
| Hugging Face / Novita / GMI / Together / Fireworks / Cerebras / Perplexity / LM Studio / vLLM | Custom or aggregator-style OpenAI-compatible provider | User-provided or preset base URL/model | Covered by custom provider before named UI polish |
| Bedrock / Azure Foundry / OpenAI Codex / GitHub Copilot | Separate transport/auth families | AWS SDK, Azure mixed endpoints, Responses/OAuth/external process | Explicitly out of MVP unless separately scoped |

## Non-Goals

- Do not claim every mainstream provider works through Anthropic's SDK.
- Do not move Rust provider calls into a Node sidecar just to use `@anthropic-ai/sdk`.
- Do not auto-enable providers without API key, base URL, model, streaming, tool-call, and usage fixture coverage.
- Do not make billing-grade cost claims when provider usage/pricing is unknown.
- Do not change the core agent loop semantics for provider-specific quirks.

## Task 1: Provider Registry Contract

**Status (2026-06-20): Completed for the registry contract slice.** This landed the static built-in provider registry, Hermes-style policy fields, aliases, and table-driven stability tests. It intentionally does not yet wire adapter routing, settings credential detection, frontend catalogs, or user-defined profile loading.

**Evidence:** implementer subagent landed the registry; spec review requested removal of an extra NVIDIA built-in and stronger provider-surface tests; spec re-review approved; quality review approved; controller verification passed `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_registry --lib`, `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --lib`, and scoped `rustfmt --edition 2021 --check` on the touched Rust files. Full `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check` still reports unrelated pre-existing formatting drift in `loop_runtime/headless.rs` and `loop_runtime/runner.rs`.

- [x] Run GitNexus impact before editing symbols:

```text
impact(target: "build_adapter", direction: "upstream", repo: "forge")
impact(target: "normalize_provider", direction: "upstream", repo: "forge")
impact(target: "default_model", direction: "upstream", repo: "forge")
impact(target: "detect_credentials_from_sources", direction: "upstream", repo: "forge")
```

Note: GitNexus reported the index as stale/incomplete and did not resolve those symbol names. File-level impact on `apps/desktop/src-tauri/src/adapters/mod.rs`, `apps/desktop/src-tauri/src/agent/provider_capabilities.rs`, and `apps/desktop/src-tauri/src/settings.rs` returned LOW risk, 0 direct callers, and 0 affected processes. Treat that as a limited-confidence gate until the GitNexus Swift parser dependency issue is repaired.

- [x] Add a backend provider registry module at `apps/desktop/src-tauri/src/adapters/provider_registry.rs`.
- [x] Move provider constants out of ad hoc match arms into explicit definitions:

```rust
pub enum ProviderTransport {
    AnthropicMessages,
    OpenAiChatCompletions,
    OpenAiResponses,
    NativeGemini,
    BedrockConverse,
    CustomOpenAiCompatible,
    CustomAnthropicCompatible,
}

pub struct ProviderDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub default_model: &'static str,
    pub default_base_url: Option<&'static str>,
    pub api_key_env: &'static [&'static str],
    pub base_url_env: &'static [&'static str],
    pub transport: ProviderTransport,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_thinking: bool,
    pub supports_usage: bool,
    pub context_window_tokens: Option<u32>,
}
```

- [x] Make the registry shape profile-oriented, inspired by Hermes `ProviderProfile`, so provider-specific quirks live on the provider definition rather than inside every adapter.
- [x] Add hooks or enum-backed policy fields for:
  - message preparation
  - extra request body
  - top-level request kwargs
  - fixed/omitted temperature
  - max token defaults
  - model catalog fallback
  - health-check support
  - vision/tool-message support
- [x] Add registry definitions for `deepseek`, `anthropic`, `kimi`, `glm`, `alibaba`, `minimax`, `openai`, `openrouter`, `gemini`, `xai`, `groq`, `mistral`, `ollama`, `custom_openai`, and `custom_anthropic`.
- [x] Preserve existing aliases: `claude -> anthropic`, `gpt -> openai`, empty provider -> `deepseek`.
- [x] Add unit tests that prove provider IDs, aliases, defaults, transport choices, and context windows are stable.

Expected result:

```text
cargo test provider_registry
```

passes and no provider metadata is duplicated in adapter routing.

## Task 1.5: Hermes-Style Provider Profile Loading

**Status (2026-06-20): Completed for the config-only profile-loading slice.** This adds safe, data-only user profile loading on top of the static registry. It supports built-in profile overrides and user-defined profiles such as `nvidia` without reintroducing NVIDIA as a built-in provider. It intentionally does not wire adapter routing, settings persistence, frontend catalogs, credential detection, or runtime calls yet.

**Evidence:** TDD red pass failed on missing `ProviderProfileConfig`, `EnvVarList`, `load_provider_profiles`, and related API. Green pass then succeeded with `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_profile_loading --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_registry --lib`, and `rustfmt --edition 2021 --check apps/desktop/src-tauri/src/adapters/provider_registry.rs`.

- [x] Add a second profile source after built-ins: user-defined provider profiles from Forge config, equivalent in spirit to Hermes' `$HERMES_HOME/plugins/model-providers/<name>/`.
- [x] MVP shape can be config-only, not executable plugins:

```toml
[[providers]]
id = "my-local-llm"
label = "My Local LLM"
transport = "openai_chat_completions"
base_url = "http://127.0.0.1:1234/v1"
api_key_env = "MY_LOCAL_LLM_API_KEY"
default_model = "local-model"
supports_tools = true
supports_streaming = true
```

- [x] Let user-defined profiles override labels, base URLs, key env vars, default model, transport, and max token defaults, but not arbitrary Rust behavior.
- [x] Keep executable provider plugins out of MVP. They are powerful, but they add code-loading and security questions that Forge does not need yet.
- [x] Add import/migration tests for Hermes-like config names and aliases: `glm`, `zhipu`, `kimi`, `moonshot`, `qwen`, `dashscope`, `minimax`, `nvidia`, `ollama`, `lmstudio`, `vllm`.

Expected result:

```text
cargo test provider_profile_loading
```

proves Forge can grow provider coverage through data/config before code.

## Task 2: Adapter Routing by Capability

**Status (2026-06-20): Completed for registry-backed adapter routing.** `build_adapter(...)` now resolves provider metadata from `provider_registry` before choosing an adapter family, preserves existing DeepSeek/Anthropic/OpenAI/OpenRouter behavior, routes the new registry providers by capability, and returns a typed unsupported-provider error with valid provider IDs. This still does not implement settings/env detection, frontend catalogs, probes, native Gemini, OpenAI Responses, runtime profile file loading, or real provider-call fixture coverage.

**Evidence:** TDD red pass failed on missing `AdapterFamily`, `resolve_adapter_route`, and `BuildAdapterError`. Green pass succeeded with `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml build_adapter --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_registry --lib`, `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --lib`, and scoped `rustfmt --edition 2021 --check` on the touched Rust files.

- [x] Refactor `apps/desktop/src-tauri/src/adapters/mod.rs::build_adapter(...)` to resolve a `ProviderDefinition` first.
- [x] Keep current behavior for existing providers:
  - `deepseek` still uses `AnthropicAdapter` with DeepSeek Anthropic base URL.
  - `anthropic` still uses `AnthropicAdapter`.
  - `kimi` uses Anthropic-compatible transport for coding-agent behavior first, with an OpenAI-compatible fallback if a user selects that route.
  - `glm` uses Anthropic-compatible transport for coding-agent behavior first, with an OpenAI-compatible fallback if a user selects that route.
  - `alibaba`/Qwen uses OpenAI-compatible transport.
  - `minimax` uses Anthropic-compatible transport by default, with explicit OpenAI-compatible profile only when the selected endpoint/model requires it.
  - `openai` still uses `OpenAiCompatibleAdapter` until an OpenAI Responses adapter is intentionally added.
  - `openrouter` still uses `OpenAiCompatibleAdapter`.
- [x] Add provider routes for:
  - `gemini` through OpenAI-compatible transport.
  - `xai` through OpenAI-compatible transport.
  - `groq` through OpenAI-compatible transport.
  - `mistral` through OpenAI-compatible transport.
  - `ollama` through the registry default Anthropic-compatible transport for now.
  - `custom_openai` and `custom_anthropic`.
- [x] Return a typed unsupported-provider error that includes the valid provider IDs.
- [x] Add golden request tests for each transport route.

Expected result:

```text
cargo test build_adapter provider_registry
```

shows existing providers unchanged and new providers routed by registry.

## Task 3: Credentials, Base URLs, and Model Defaults

**Status (2026-06-20): Completed for registry-backed credentials and provider metadata.** Settings now uses the provider registry as the authoritative known-provider list and reads API key/base URL env vars from registry metadata. Provider model env detection is provider-specific, so `ANTHROPIC_MODEL` no longer leaks into OpenAI/Kimi/GLM/etc. Anthropic/Claude config behavior is preserved for Anthropic only. Provider normalization, default models, labels, and context windows now resolve through the registry while preserving DeepSeek v4-pro and `[1m]` one-million-token behavior.

**Evidence:** TDD red pass failed on mainstream env fallback, provider-specific model detection, key-status provider coverage, and registry-backed provider-capability metadata. Green pass succeeded with `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml settings::tests::detect_credentials --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_capabilities --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_registry --lib`, `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --lib`, and scoped `rustfmt --edition 2021 --check` on the touched Rust files.

- [x] Update `apps/desktop/src-tauri/src/settings.rs` so `KNOWN_PROVIDERS` is registry-driven or generated from one authoritative list.
- [x] Add provider-specific env detection:

```text
GEMINI_API_KEY
GEMINI_BASE_URL
XAI_API_KEY
XAI_BASE_URL
GROQ_API_KEY
GROQ_BASE_URL
MISTRAL_API_KEY
MISTRAL_BASE_URL
ALIBABA_API_KEY
DASHSCOPE_API_KEY
ALIBABA_BASE_URL
MINIMAX_API_KEY
MINIMAX_BASE_URL
MINIMAX_CN_API_KEY
MINIMAX_CN_BASE_URL
MOONSHOT_API_KEY
MOONSHOT_BASE_URL
KIMI_API_KEY
KIMI_BASE_URL
ZHIPU_API_KEY
ZHIPU_BASE_URL
GLM_API_KEY
GLM_BASE_URL
OLLAMA_BASE_URL
NVIDIA_API_KEY
NVIDIA_BASE_URL
FORGE_CUSTOM_OPENAI_API_KEY
FORGE_CUSTOM_OPENAI_BASE_URL
FORGE_CUSTOM_ANTHROPIC_API_KEY
FORGE_CUSTOM_ANTHROPIC_BASE_URL
```

- [x] Stop treating `ANTHROPIC_MODEL` as a universal model override. Add provider-specific model envs for built-in providers, including `OPENAI_MODEL`, `KIMI_MODEL`, `MOONSHOT_MODEL`, `GLM_MODEL`, `ZHIPU_MODEL`, `ALIBABA_MODEL`, `QWEN_MODEL`, `MINIMAX_MODEL`, `GEMINI_MODEL`, `XAI_MODEL`, `GROQ_MODEL`, `MISTRAL_MODEL`, `DEEPSEEK_MODEL`, `OPENROUTER_MODEL`, `OLLAMA_MODEL`, `FORGE_CUSTOM_OPENAI_MODEL`, and `FORGE_CUSTOM_ANTHROPIC_MODEL`. `NVIDIA_MODEL` remains future profile-driven coverage because NVIDIA is not a built-in provider.
- [x] Add tests for stored key priority, env fallback, base URL fallback, and provider-specific model detection.

Expected result:

```text
cargo test settings::tests::detect_credentials
```

proves each new provider can be discovered without breaking existing DeepSeek/Anthropic/OpenAI/OpenRouter behavior.

## Task 4: Frontend Provider Catalog

- [ ] Update `apps/desktop/src/lib/providers.ts` to include the same provider IDs and default model names as the Rust registry.
- [ ] Add custom provider support without requiring hardcoded model lists.
- [ ] Update settings/model picker UI surfaces that read `PROVIDERS`:
  - `apps/desktop/src/lib/components/settings/SettingsProviderRows.tsx`
  - `apps/desktop/src/lib/components/settings/SettingsDialogModel.ts`
  - `apps/desktop/src/lib/components/composer/ComposerModelMenu.tsx`
  - `apps/desktop/src/lib/components/composer/useComposerModelMenu.ts`
- [ ] Add UI tests or component tests that provider labels, default models, and context windows render without truncation.
- [ ] Keep the default provider as `deepseek` unless the user explicitly changes it.

Expected result:

```text
npm --workspace apps/desktop test -- providers
```

or the repo-equivalent frontend test command passes.

## Task 5: Streaming, Tools, and Reasoning Fixtures

- [ ] Add fixture tests for OpenAI-compatible streaming deltas:
  - text delta
  - tool call start
  - tool call argument delta
  - usage payload
  - reasoning or reasoning-like payload when present
- [ ] Add fixture tests for Anthropic-compatible streaming deltas:
  - text block
  - thinking block
  - tool_use block
  - usage payload
- [ ] Add a provider conformance test table:

```rust
struct ProviderConformance {
    provider: &'static str,
    streaming_fixture: &'static str,
    expects_tools: bool,
    expects_usage: bool,
    expects_reasoning: bool,
}
```

- [ ] Mark unknown fields as unknown/null rather than dropping them silently.
- [ ] Ensure `compact_summary(...)` remains tool-free for every provider.

Expected result:

```text
cargo test adapters::openai_compatible adapters::anthropic provider_conformance
```

proves mainstream providers fit the agent loop contract before any live API call is trusted.

## Task 6: Cost and Usage Facts

- [ ] Extend usage metadata so every provider records:
  - provider ID
  - model ID
  - prompt/input tokens
  - completion/output tokens
  - cache tokens when available
  - reasoning tokens when available
  - cost estimate when known
  - `usage_unknown` and `cost_unknown` flags when not known
- [ ] Keep pricing tables source-stamped and optional.
- [ ] Do not block agent execution when pricing is missing.
- [ ] Add tests proving unknown usage/cost is explicit in emitted runtime facts.

Expected result:

```text
cargo test usage cost pricing
```

passes without pretending all providers have equal telemetry quality.

## Task 7: Compatibility Probe UX

- [ ] Add a non-destructive provider probe command that can validate:
  - key present
  - base URL reachable
  - model accepted
  - streaming accepted
  - tool schema accepted
- [ ] Keep probe execution user-triggered. Do not probe paid APIs automatically on app startup.
- [ ] Surface probe result in Settings diagnostics with provider-specific error messages.
- [ ] Add acceptance coverage for a mocked successful probe and a mocked unsupported-tool response.

Expected result:

```text
scripts/acceptance.sh --dry-run
```

advertises the provider probe coverage when the product surface is added.

## Task 8: Documentation and Obsidian Narrative

- [ ] Update `README.md`, `apps/desktop/README.md`, and `CHANGELOG.md` only when the provider expansion becomes user-visible.
- [ ] Add an Obsidian narrative at `/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Mainstream Provider Expansion Plan.md`.
- [ ] The Obsidian note must explain:
  - current provider architecture
  - why Forge uses an internal Anthropic-style agent contract
  - why provider transport is capability-based
  - why not every provider should go through Anthropic SDK
  - evidence and tests
  - what is not claimed
  - interview-ready explanation

Expected result:

```text
rg "Mainstream Provider Expansion" docs/superpowers/plans /Users/cabbos/cabbosAI/code-cli/Forge/03\\ Roadmap
```

finds both the engineering source-of-truth plan and the narrative note.

## Task 9: Implementation Discipline

- [ ] Use subagent-driven-development for implementation:
  - implementer
  - spec reviewer
  - quality reviewer
  - fix pass
  - commit gate
- [ ] Before editing existing Rust or TypeScript symbols, run GitNexus impact and report risk.
- [ ] Before every commit, run:

```text
detect_changes(scope: "all", repo: "forge")
```

- [ ] Commit in small slices:
  - provider registry
  - adapter routing
  - credentials/settings
  - frontend catalog
  - fixture/conformance tests
  - docs/Obsidian sync
- [ ] Do not touch unrelated untracked roadmap files.

## MVP Definition

The MVP is complete when:

- Existing providers behave exactly as before.
- New mainstream providers can be represented by registry definitions.
- At least Kimi, GLM, Alibaba/Qwen, MiniMax, Gemini, xAI, Groq, Mistral, Ollama, `custom_openai`, and `custom_anthropic` have routing and fixture coverage.
- The UI can show provider choices and custom model/base URL paths without hardcoded drift from backend defaults.
- Streaming, tools, usage, unknown-cost flags, and summary compaction are covered by tests.
- Documentation clearly says Forge is Anthropic-style internally and capability-routed externally.

## Recommended Sequencing

1. Build the registry without changing behavior.
2. Add the Hermes-style profile fields and config-defined profile loading.
3. Route existing providers through the registry.
4. Add custom OpenAI-compatible and custom Anthropic-compatible providers.
5. Add China coding-provider presets: Kimi, GLM, Alibaba/Qwen, MiniMax.
6. Add international named presets: Gemini, xAI, Groq, Mistral, NVIDIA.
7. Add fixture conformance tests.
8. Add frontend catalog and settings UX.
9. Add compatibility probes.
10. Publish docs and Obsidian narrative.

This sequence gives us mainstream coverage fast through custom compatibility endpoints, then turns the most important providers into polished first-class presets.
