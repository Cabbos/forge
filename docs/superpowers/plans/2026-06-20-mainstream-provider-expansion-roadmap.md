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

**Status (2026-06-20): Completed for the frontend catalog slice.** `apps/desktop/src/lib/providers.ts` now mirrors the Rust built-in provider ID set and default models for DeepSeek, Anthropic, Kimi/Moonshot, GLM/Zhipu, Alibaba/Qwen, MiniMax, OpenAI, OpenRouter, Gemini, xAI, Groq, Mistral, Ollama, `custom_openai`, and `custom_anthropic`. NVIDIA remains excluded from built-ins. Provider aliases, model ownership, context-window helpers, and custom-provider model display are covered by node tests. Browser-level truncation/render validation remains future UI polish.

**Evidence:** TDD red pass failed while the frontend catalog still had only four providers. Green pass succeeded with `node --test src/lib/providers.test.ts` and the adjacent profile-default regression `node --test src/hooks/sessionProfileDefaults.test.ts` from `apps/desktop`. At the time, `npm run build` reached `tsc` but was blocked by an unrelated pre-existing error in `src/lib/backgroundTaskStatus.ts` where `tasks.map(summarizeLoopTaskRecord)` passed the array index into an options parameter; that blocker was later fixed in `f5ec87cf`.

- [x] Update `apps/desktop/src/lib/providers.ts` to include the same provider IDs and default model names as the Rust registry.
- [x] Add custom provider support without requiring hardcoded model lists.
- [x] Update settings/model picker UI surfaces that read `PROVIDERS`:
  - `apps/desktop/src/components/settings/SettingsProviderRows.tsx`
  - `apps/desktop/src/components/settings/SettingsDialogModel.ts`
  - `apps/desktop/src/components/session/ComposerModelMenu.tsx`
  - `apps/desktop/src/components/session/useComposerModelMenu.ts`
- [ ] Add UI tests or component tests that provider labels, default models, and context windows render without truncation.
- [x] Keep the default provider as `deepseek` unless the user explicitly changes it.

Note: no React component edits were required in this slice because Settings and Composer already derive provider rows and menu options from `PROVIDERS`; updating the catalog and helper semantics updates those surfaces.

Expected result:

```text
npm --workspace apps/desktop test -- providers
```

or the repo-equivalent frontend test command passes. The repo-equivalent command for this slice is:

```text
cd apps/desktop && node --test src/lib/providers.test.ts
```

## Task 5: Streaming, Tools, and Reasoning Fixtures

**Status (2026-06-20): Completed for fixture/conformance coverage.** Existing OpenAI-compatible and Anthropic-compatible parsers already satisfied the mainstream streaming contract, so this slice added fixture and conformance tests without production parser changes. The tests now pin text deltas, tool-call starts, split tool argument deltas, usage payloads, OpenAI-style reasoning payloads, Anthropic thinking blocks, explicit unknown usage/null fields, and compact-summary tool-free behavior across representative provider families. This does not add live provider calls or Task 6 cost/usage schema expansion.

**Evidence:** Verification passed `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml adapters::openai_compatible --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml adapters::anthropic --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_conformance --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml compact_summary --lib`, `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --lib`, and scoped `rustfmt --edition 2021 --check` on touched Rust files.

- [x] Add fixture tests for OpenAI-compatible streaming deltas:
  - text delta
  - tool call start
  - tool call argument delta
  - usage payload
  - reasoning or reasoning-like payload when present
- [x] Add fixture tests for Anthropic-compatible streaming deltas:
  - text block
  - thinking block
  - tool_use block
  - usage payload
- [x] Add a provider conformance test table:

```rust
struct ProviderConformance {
    provider: &'static str,
    streaming_fixture: &'static str,
    expects_tools: bool,
    expects_usage: bool,
    expects_reasoning: bool,
}
```

- [x] Mark unknown fields as unknown/null rather than dropping them silently.
- [x] Ensure `compact_summary(...)` remains tool-free for every provider.

Expected result:

```text
cargo test adapters::openai_compatible adapters::anthropic provider_conformance
```

proves mainstream providers fit the agent loop contract before any live API call is trusted.

## Task 6: Cost and Usage Facts

**Status (2026-06-20): Completed for runtime usage/cost fact expansion.** Provider usage events, subagent usage facts, and loop usage ledgers now carry canonical `provider_id` separately from legacy transport `source`, model ID, input/output tokens, optional cache-read/cache-creation/reasoning tokens, optional cost estimate, and optional `pricing_source`. Unknown token/cost behavior remains explicit through existing null fields and `has_unknown_*` ledger flags. Static pricing remains optional and source-stamped; missing pricing emits known tokens with `pricing_unknown` rather than blocking execution. This does not certify live provider billing accuracy or complete provider-specific pricing tables.

**Evidence:** TDD red pass failed on missing provider/cached/reasoning/pricing fields in Rust usage contracts and frontend metadata preservation. Green pass succeeded with `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml usage --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml budget --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml adapters::anthropic --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml adapters::openai_compatible --lib`, and `node --test src/store/blocks.test.ts src/lib/loopRuntime.test.ts` from `apps/desktop`. Final check/rustfmt evidence is tracked in the Task 6 implementer handoff.

- [x] Extend usage metadata so every provider records:
  - provider ID
  - model ID
  - prompt/input tokens
  - completion/output tokens
  - cache tokens when available
  - reasoning tokens when available
  - cost estimate when known
  - explicit unknown token/cost facts through null fields and existing unknown flags
- [x] Keep pricing tables source-stamped and optional.
- [x] Do not block agent execution when pricing is missing.
- [x] Add tests proving unknown usage/cost is explicit in emitted runtime facts.

Expected result:

```text
cargo test usage cost pricing
```

passes without pretending all providers have equal telemetry quality.

## Task 7: Compatibility Probe UX

**Status (2026-06-20): Completed for the manual Settings probe slice.** Forge now exposes a user-triggered `probe_provider` IPC command and a Settings provider-row action. The probe checks local key presence before network, sends one minimal streaming request with a no-op tool schema through the provider's registry transport family, returns structured check results without API key leakage, and classifies unsupported tool/schema responses into provider-specific diagnostics. It does not run on startup or automatically probe paid APIs.

**Evidence:** TDD red pass failed on missing provider probe types/API, then on missing Settings probe button. Green pass succeeded with `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_probe --lib`, `node --test apps/desktop/src/lib/ipc/apiKeys.test.ts`, and `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "provider probe"`. `scripts/acceptance.sh --dry-run` now advertises Desktop Phase 7 provider probe coverage.

- [x] Add a non-destructive provider probe command that can validate:
  - key present
  - base URL reachable
  - model accepted
  - streaming accepted
  - tool schema accepted
- [x] Keep probe execution user-triggered. Do not probe paid APIs automatically on app startup.
- [x] Surface probe result in Settings diagnostics with provider-specific error messages.
- [x] Add acceptance coverage for a mocked successful probe and a mocked unsupported-tool response.

Expected result:

```text
scripts/acceptance.sh --dry-run
```

advertises the provider probe coverage when the product surface is added.

## Task 8: Documentation and Obsidian Narrative

**Status (2026-06-20): Completed for the provider-expansion narrative slice.** The user-visible Settings probe was documented in `README.md`, `apps/desktop/README.md`, and `CHANGELOG.md` when Task 7 made provider compatibility probing visible. The Obsidian note now mirrors the engineering roadmap as an architecture narrative for interview/backing use: current state, Hermes comparison, internal Anthropic-style contract, capability-based provider routing, evidence, and explicit non-claims.

**Evidence:** `rg "Mainstream Provider Expansion" docs/superpowers/plans /Users/cabbos/cabbosAI/code-cli/Forge/03\ Roadmap` finds both this source-of-truth roadmap and the Obsidian narrative. Task 7 verification also confirmed `scripts/acceptance.sh --dry-run` advertises the provider probe acceptance coverage.

- [x] Update `README.md`, `apps/desktop/README.md`, and `CHANGELOG.md` only when the provider expansion becomes user-visible.
- [x] Add an Obsidian narrative at `/Users/cabbos/cabbosAI/code-cli/Forge/03 Roadmap/Mainstream Provider Expansion Plan.md`.
- [x] The Obsidian note must explain:
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

**Status (2026-06-20): Completed with a documented GitNexus gate adjustment.** Implementation was run as small subagent-driven slices with implementer, spec reviewer, quality reviewer, fix passes, controller verification, and commit gates. GitNexus impact was run before editing existing Rust/TypeScript symbols, but the index was stale/incomplete for several TypeScript/Rust symbols and often fell back to LOW-confidence file-level results. The original `detect_changes(scope: "all")` example was narrowed to staged-scope commit gates because the worktree contains unrelated user-owned dirty files and untracked eval roadmaps that must not be touched; a final `detect_changes(scope: "compare", base_ref: "main")` audit was run at closure and reported LOW risk with 0 affected processes across the broad branch diff.

- [x] Use subagent-driven-development for implementation:
  - implementer
  - spec reviewer
  - quality reviewer
  - fix pass
  - commit gate
- [x] Before editing existing Rust or TypeScript symbols, run GitNexus impact and report risk.
- [x] Before every commit, run a GitNexus detect-changes gate on the commit scope:

```text
detect_changes(scope: "staged", repo: "forge")
```

- [x] Do not claim the literal all-scope gate was run before every commit; use final all/compare audit as completion verification because unrelated dirty files remain in the worktree.
- [x] Commit in small slices:
  - provider registry
  - config-defined profile loading
  - adapter routing
  - credentials/settings
  - frontend catalog
  - fixture/conformance tests
  - usage/cost facts
  - compatibility probe
  - docs/Obsidian sync
- [x] Do not touch unrelated untracked roadmap files.

## Final Completion Snapshot

**Status (2026-06-20): Provider expansion MVP implemented and documented.** This roadmap update is the final docs closure for the mainstream provider expansion slice.

Completed commits:

- `fcea8d7b feat(provider): add mainstream provider registry`
- `6580c764 feat(provider): load config provider profiles`
- `e587c149 feat(provider): route adapters by registry`
- `4b60920f feat(provider): detect registry credentials`
- `7ad6a83e feat(provider): sync frontend catalog`
- `1a753498 test(provider): add streaming conformance fixtures`
- `818a6890 feat(provider): record usage cost facts`
- `03c4b727 feat(provider): add manual compatibility probe`

Post-MVP usability commits:

- `66410350 feat(provider): wire config profiles into runtime`
- `8ec4ab0e feat(provider): surface config profiles in catalog`
- `351c7804 feat(provider): honor dynamic catalog profile defaults`
- `f5ec87cf fix(desktop): unblock production build`

Known caveats:

- Live provider certification is not claimed; compatibility probing is manual and user-triggered.
- OpenAI Responses, native Gemini, Bedrock, Azure Foundry, OpenAI Codex, and executable provider plugins remain out of MVP.
- Pricing is source-stamped and optional; missing cost remains explicit unknown/null, not zero.
- Dynamic model fetching now has a manual Settings path for OpenAI-compatible `/models`. Successful refreshes are persisted into a provider-catalog cache and become selectable in Composer, but Forge still does not auto-change default models and native Anthropic/Gemini/Bedrock model catalog endpoints are not claimed.

## 2026-06-20 Post-MVP Usability Slice: Config Profiles and No-Auth Local Providers

**Status (2026-06-20): Implemented as the first "fully usable provider" hardening slice after MVP closure.** Config-defined provider profiles now participate in the real runtime path instead of only passing registry loading tests. `~/.forge/config.json` can add data-only provider profiles with id, label, transport, base URL, API-key/base-URL env vars, default model, aliases, and streaming/tool capability flags. These profiles are used by credential detection, key-status rows, adapter routing, default model/base URL resolution, and manual Settings probes.

No-auth local providers are also unblocked. Profiles with an empty `api_key_env` list, including built-in `ollama`, no longer fall into `MissingKeyAdapter` just because the API key is empty. Anthropic-compatible and OpenAI-compatible adapters can now construct no-auth clients for those profiles, and request/probe code skips empty `x-api-key` / `Authorization` headers.

**Evidence:** Focused verification passed `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml settings::tests --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml adapters::tests::build_adapter --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_probe --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_profile_loading --lib`, and `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --lib`.

**At that point not claimed:** live endpoint certification, executable provider plugins, in-app provider-profile editing, native OpenAI Responses/Gemini/Bedrock transports, or billing-grade pricing.

## 2026-06-20 Post-MVP Usability Slice: Dynamic Frontend Provider Catalog

**Status (2026-06-20): Implemented as the second "fully usable provider" hardening slice after MVP closure.** Config-defined provider profiles now flow through a backend `get_provider_catalog` command and merge into the frontend provider catalog. Settings provider rows and the Composer model menu can display profiles such as `nvidia` or `local-openai` from `~/.forge/config.json`, including their label, aliases, default model, context window when known, and no-auth key placeholder behavior.

This closes the practical drift where the backend could route/probe a config provider, but the desktop UI still only displayed the static built-in list. The UI still keeps built-in provider metadata as a fallback for non-Tauri/test contexts, and configured profiles remain data-only: no executable provider plugins were claimed in this slice.

**Evidence:** Focused verification passed `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml settings::tests --lib`, `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --lib`, `cd apps/desktop && node --test src/hooks/sessionProfileDefaults.test.ts src/lib/providers.test.ts src/lib/ipc/apiKeys.test.ts`, `rustfmt --edition 2021 --check` on the touched Rust catalog files, and `git diff --check` on touched provider/catalog files. The earlier production-build blocker in `apps/desktop/src/lib/backgroundTaskStatus.ts` was fixed in `f5ec87cf`, and `npm --prefix apps/desktop run build` now passes.

**At that point not claimed:** live provider certification, automatic default-model mutation from refreshed catalogs, in-app provider-profile editing, executable provider plugins, native OpenAI Responses/Gemini/Bedrock transports, or browser-level truncation/render validation for every provider row.

## 2026-06-20 Post-MVP Usability Slice: Profile Defaults Use Dynamic Catalog

**Status (2026-06-20): Implemented as the third "fully usable provider" hardening slice after MVP closure.** Active profile defaults now use the dynamic provider catalog when they update the visible Composer selection or create a new desktop session. A profile that sets `default_provider: "nim"` resolves through the configured alias to `nvidia` and uses the configured default model, and a profile that only sets `default_model: "local-model"` can infer the matching configured provider from the catalog.

This closes the follow-on drift after the dynamic catalog UI slice: config providers were visible, but profile application still used static helper defaults and could pair a configured provider with `custom-model` or with the previous provider's model. The fix keeps profile defaults aligned with the same catalog that backs Settings rows and the Composer model menu.

**Evidence:** TDD red pass failed in `cd apps/desktop && node --test src/hooks/sessionProfileDefaults.test.ts` on configured provider alias/default-model resolution for both Composer and new-session defaults. Green pass succeeded for that test file plus `cd apps/desktop && node --test src/lib/providers.test.ts src/lib/ipc/apiKeys.test.ts`, and the desktop production build now passes after `f5ec87cf`.

**At that point not claimed:** visual provider-profile editing, live provider certification, automatic default-model mutation from refreshed catalogs, native model catalog endpoints, or browser-level validation of profile form/provider dropdown ergonomics.

## 2026-06-20 Post-MVP Usability Slice: Manual Model Catalog Refresh

**Status (2026-06-20): Implemented as the fourth "fully usable provider" hardening slice after MVP closure, then extended with a provider-catalog cache.** Settings provider rows now expose a user-triggered model catalog refresh for OpenAI-compatible providers. The backend `list_provider_models` command resolves the same registry/config profile metadata used by routing, reads credential/base URL state through Settings detection, calls `{base_url}/models`, deduplicates returned model ids, saves successful results into `provider_model_catalogs` in the local Forge config, and reports an explicit available/unavailable result with remediation text. Config-defined no-auth local profiles are supported by skipping the bearer header when no key is required.

This closes another practical usability gap: a custom provider profile can be visible and routable, but users still need a way to inspect what the endpoint actually exposes and then select one of those models. Settings displays the fetched model ids and source base URL; `get_provider_catalog` now carries cached model ids into the frontend catalog, so the Composer model menu can display and select refreshed models. Composer defaults and profile config are not automatically mutated.

**Evidence:** TDD red pass first failed on the missing Rust model catalog API, then failed again on missing cached catalog fields and frontend merge semantics. Green verification covered `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_model_catalog --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml settings::tests::provider_catalog --lib`, `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --lib`, `cd apps/desktop && node --test src/lib/ipc/apiKeys.test.ts src/lib/providers.test.ts src/hooks/sessionProfileDefaults.test.ts`, `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "refreshes a mocked provider model catalog"`, `npm --prefix apps/desktop run build`, and scoped `rustfmt --edition 2021 --check` on the touched Rust files.

**Still not claimed:** automatic Composer/default-model mutation, startup auto-probing, native Anthropic/Gemini/Bedrock model catalog endpoints, live certification of every provider, or billing-grade provider catalog/pricing metadata.

## 2026-06-20 Post-MVP Usability Slice: Settings Custom Provider Profile Editor

**Status (2026-06-20): Implemented as the fifth "fully usable provider" hardening slice after MVP closure.** Settings now includes a visual custom Provider profile editor for data-only OpenAI-compatible and Anthropic-compatible endpoints. Users can create profiles with id, label, transport, base URL, API-key/base-URL env vars, default model, aliases, and streaming/tool capability flags; they can also delete editable user-defined profiles from provider rows. Saved profiles are persisted through the same Forge config path as hand-written `providers`, then flow into key-status rows, the backend provider catalog, frontend provider merge logic, manual probes, cached model selection, and Composer.

This closes the usability gap where custom providers were technically supported but still required editing `~/.forge/config.json` by hand. The first editor slice intentionally stays data-only and conservative: it does not load executable plugins, does not certify live provider endpoints, does not auto-select refreshed models as defaults, and does not add native OpenAI Responses/Gemini/Bedrock transports.

**Evidence:** TDD red pass failed on missing `ProviderProfileInput`, settings upsert/delete application, IPC wrappers, Settings form controls, and e2e mock commands. Green verification covered `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml provider_profile_input_can_be_upserted_and_deleted --lib`, `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml settings::tests --lib`, `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --lib`, `cd apps/desktop && node --test src/lib/ipc/apiKeys.test.ts src/lib/providers.test.ts src/hooks/sessionProfileDefaults.test.ts`, `npm --prefix apps/desktop run test:e2e -- e2e/acceptance.spec.ts -g "settings models (creates and deletes a custom provider profile|refreshes a mocked provider model catalog)"`, `npm --prefix apps/desktop run build`, `scripts/acceptance.sh --dry-run`, and scoped `rustfmt --edition 2021` on touched Rust settings files.

**Still not claimed:** full arbitrary provider-plugin editing, live endpoint certification for every vendor, automatic default-model mutation after `/models`, native model catalog endpoints for Anthropic/Gemini/Bedrock, advanced provider-specific quirk hooks in the UI, or executable Hermes-style provider plugins.

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
