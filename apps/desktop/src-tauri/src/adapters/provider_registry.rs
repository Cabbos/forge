#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderTransport {
    AnthropicMessages,
    OpenAiChatCompletions,
    OpenAiResponses,
    NativeGemini,
    BedrockConverse,
    CustomOpenAiCompatible,
    CustomAnthropicCompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessagePreparationPolicy {
    AnthropicCompatible,
    OpenAiCompatible,
    GeminiOpenAiCompatible,
    Passthrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestBodyPolicy {
    StandardAnthropic,
    StandardOpenAi,
    DeepSeekAnthropic { thinking_budget_tokens: Option<u32> },
    KimiAnthropic,
    GlmAnthropic,
    MinimaxAnthropic,
    OpenRouterExtensions,
    CustomCompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopLevelRequestPolicy {
    None,
    KimiReasoningEffort,
    OpenRouterProviderHints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TemperaturePolicy {
    UserConfigurable,
    Omit,
    Fixed(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelCatalogPolicy {
    None,
    StaticFallback,
    HttpModelsEndpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HealthCheckSupport {
    None,
    HttpModelsEndpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VisionSupport {
    Supported,
    Unsupported,
    ModelDependent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolMessageSupport {
    AnthropicBlocks,
    OpenAiToolRole,
    PlainTextOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderDefinition {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) default_model: &'static str,
    pub(crate) default_base_url: Option<&'static str>,
    pub(crate) api_key_env: &'static [&'static str],
    pub(crate) base_url_env: &'static [&'static str],
    pub(crate) transport: ProviderTransport,
    pub(crate) supports_streaming: bool,
    pub(crate) supports_tools: bool,
    pub(crate) supports_thinking: bool,
    pub(crate) supports_usage: bool,
    pub(crate) context_window_tokens: Option<u32>,
    pub(crate) message_preparation: MessagePreparationPolicy,
    pub(crate) request_body: RequestBodyPolicy,
    pub(crate) top_level_request: TopLevelRequestPolicy,
    pub(crate) temperature: TemperaturePolicy,
    pub(crate) model_catalog: ModelCatalogPolicy,
    pub(crate) health_check: HealthCheckSupport,
    pub(crate) vision: VisionSupport,
    pub(crate) tool_messages: ToolMessageSupport,
    pub(crate) model_fallbacks: &'static [&'static str],
    pub(crate) max_output_tokens_default: Option<u32>,
}

const VALID_PROVIDER_IDS: &[&str] = &[
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

const PROVIDER_DEFINITIONS: &[ProviderDefinition] = &[
    ProviderDefinition {
        id: "deepseek",
        label: "DeepSeek",
        default_model: "deepseek-v4-flash[1m]",
        default_base_url: Some("https://api.deepseek.com/anthropic"),
        api_key_env: &["DEEPSEEK_API_KEY"],
        base_url_env: &["DEEPSEEK_BASE_URL"],
        transport: ProviderTransport::AnthropicMessages,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: true,
        supports_usage: true,
        context_window_tokens: Some(1_000_000),
        message_preparation: MessagePreparationPolicy::AnthropicCompatible,
        request_body: RequestBodyPolicy::DeepSeekAnthropic {
            thinking_budget_tokens: Some(16_000),
        },
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::Omit,
        model_catalog: ModelCatalogPolicy::StaticFallback,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::Unsupported,
        tool_messages: ToolMessageSupport::AnthropicBlocks,
        model_fallbacks: &["deepseek-v4-flash[1m]", "deepseek-v4-pro", "deepseek-chat"],
        max_output_tokens_default: Some(384_000),
    },
    ProviderDefinition {
        id: "anthropic",
        label: "Anthropic",
        default_model: "claude-sonnet-4-6",
        default_base_url: Some("https://api.anthropic.com"),
        api_key_env: &["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY"],
        base_url_env: &["ANTHROPIC_BASE_URL"],
        transport: ProviderTransport::AnthropicMessages,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: true,
        supports_usage: true,
        context_window_tokens: Some(200_000),
        message_preparation: MessagePreparationPolicy::AnthropicCompatible,
        request_body: RequestBodyPolicy::StandardAnthropic,
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::Supported,
        tool_messages: ToolMessageSupport::AnthropicBlocks,
        model_fallbacks: &["claude-sonnet-4-6", "claude-opus-4-8"],
        max_output_tokens_default: None,
    },
    ProviderDefinition {
        id: "kimi",
        label: "Kimi / Moonshot",
        default_model: "kimi-k2.5",
        default_base_url: Some("https://api.moonshot.cn/anthropic"),
        api_key_env: &["KIMI_API_KEY", "MOONSHOT_API_KEY"],
        base_url_env: &["KIMI_BASE_URL", "MOONSHOT_BASE_URL"],
        transport: ProviderTransport::AnthropicMessages,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: true,
        supports_usage: true,
        context_window_tokens: Some(128_000),
        message_preparation: MessagePreparationPolicy::AnthropicCompatible,
        request_body: RequestBodyPolicy::KimiAnthropic,
        top_level_request: TopLevelRequestPolicy::KimiReasoningEffort,
        temperature: TemperaturePolicy::Omit,
        model_catalog: ModelCatalogPolicy::StaticFallback,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::ModelDependent,
        tool_messages: ToolMessageSupport::AnthropicBlocks,
        model_fallbacks: &["kimi-k2.5", "kimi-k2", "moonshot-v1-32k"],
        max_output_tokens_default: Some(32_768),
    },
    ProviderDefinition {
        id: "glm",
        label: "GLM / Zhipu",
        default_model: "glm-4.5",
        default_base_url: Some("https://open.bigmodel.cn/api/anthropic"),
        api_key_env: &["GLM_API_KEY", "ZHIPU_API_KEY"],
        base_url_env: &["GLM_BASE_URL", "ZHIPU_BASE_URL"],
        transport: ProviderTransport::AnthropicMessages,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: true,
        supports_usage: true,
        context_window_tokens: Some(128_000),
        message_preparation: MessagePreparationPolicy::AnthropicCompatible,
        request_body: RequestBodyPolicy::GlmAnthropic,
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::StaticFallback,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::ModelDependent,
        tool_messages: ToolMessageSupport::AnthropicBlocks,
        model_fallbacks: &["glm-4.5", "glm-4.5-air", "glm-4-plus"],
        max_output_tokens_default: Some(32_768),
    },
    ProviderDefinition {
        id: "alibaba",
        label: "Alibaba / Qwen",
        default_model: "qwen3-coder-plus",
        default_base_url: Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
        api_key_env: &["ALIBABA_API_KEY", "DASHSCOPE_API_KEY"],
        base_url_env: &["ALIBABA_BASE_URL", "DASHSCOPE_BASE_URL"],
        transport: ProviderTransport::OpenAiChatCompletions,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: true,
        supports_usage: true,
        context_window_tokens: Some(128_000),
        message_preparation: MessagePreparationPolicy::OpenAiCompatible,
        request_body: RequestBodyPolicy::StandardOpenAi,
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::ModelDependent,
        tool_messages: ToolMessageSupport::OpenAiToolRole,
        model_fallbacks: &["qwen3-coder-plus", "qwen-max", "qwen-plus"],
        max_output_tokens_default: Some(32_768),
    },
    ProviderDefinition {
        id: "minimax",
        label: "MiniMax",
        default_model: "MiniMax-M2.7",
        default_base_url: Some("https://api.minimax.io/anthropic"),
        api_key_env: &["MINIMAX_API_KEY", "MINIMAX_CN_API_KEY"],
        base_url_env: &["MINIMAX_BASE_URL", "MINIMAX_CN_BASE_URL"],
        transport: ProviderTransport::AnthropicMessages,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: true,
        supports_usage: true,
        context_window_tokens: Some(128_000),
        message_preparation: MessagePreparationPolicy::AnthropicCompatible,
        request_body: RequestBodyPolicy::MinimaxAnthropic,
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::StaticFallback,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::ModelDependent,
        tool_messages: ToolMessageSupport::AnthropicBlocks,
        model_fallbacks: &["MiniMax-M2.7", "MiniMax-M1"],
        max_output_tokens_default: Some(32_768),
    },
    ProviderDefinition {
        id: "openai",
        label: "OpenAI",
        default_model: "gpt-4o",
        default_base_url: Some("https://api.openai.com/v1"),
        api_key_env: &["OPENAI_API_KEY"],
        base_url_env: &["OPENAI_BASE_URL"],
        transport: ProviderTransport::OpenAiChatCompletions,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: false,
        supports_usage: true,
        context_window_tokens: Some(128_000),
        message_preparation: MessagePreparationPolicy::OpenAiCompatible,
        request_body: RequestBodyPolicy::StandardOpenAi,
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::Supported,
        tool_messages: ToolMessageSupport::OpenAiToolRole,
        model_fallbacks: &["gpt-4o", "gpt-4o-mini"],
        max_output_tokens_default: None,
    },
    ProviderDefinition {
        id: "openrouter",
        label: "OpenRouter",
        default_model: "openai/gpt-4o-mini",
        default_base_url: Some("https://openrouter.ai/api/v1"),
        api_key_env: &["OPENROUTER_API_KEY"],
        base_url_env: &["OPENROUTER_BASE_URL"],
        transport: ProviderTransport::OpenAiChatCompletions,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: true,
        supports_usage: true,
        context_window_tokens: Some(128_000),
        message_preparation: MessagePreparationPolicy::OpenAiCompatible,
        request_body: RequestBodyPolicy::OpenRouterExtensions,
        top_level_request: TopLevelRequestPolicy::OpenRouterProviderHints,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::ModelDependent,
        tool_messages: ToolMessageSupport::OpenAiToolRole,
        model_fallbacks: &[
            "openai/gpt-4o-mini",
            "anthropic/claude-sonnet-4",
            "google/gemini-2.5-pro",
        ],
        max_output_tokens_default: None,
    },
    ProviderDefinition {
        id: "gemini",
        label: "Gemini",
        default_model: "gemini-2.5-pro",
        default_base_url: Some("https://generativelanguage.googleapis.com/v1beta/openai"),
        api_key_env: &["GEMINI_API_KEY"],
        base_url_env: &["GEMINI_BASE_URL"],
        transport: ProviderTransport::OpenAiChatCompletions,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: true,
        supports_usage: true,
        context_window_tokens: Some(1_000_000),
        message_preparation: MessagePreparationPolicy::GeminiOpenAiCompatible,
        request_body: RequestBodyPolicy::StandardOpenAi,
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::Supported,
        tool_messages: ToolMessageSupport::OpenAiToolRole,
        model_fallbacks: &["gemini-2.5-pro", "gemini-2.5-flash"],
        max_output_tokens_default: Some(65_536),
    },
    ProviderDefinition {
        id: "xai",
        label: "xAI",
        default_model: "grok-4",
        default_base_url: Some("https://api.x.ai/v1"),
        api_key_env: &["XAI_API_KEY"],
        base_url_env: &["XAI_BASE_URL"],
        transport: ProviderTransport::OpenAiChatCompletions,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: true,
        supports_usage: true,
        context_window_tokens: Some(128_000),
        message_preparation: MessagePreparationPolicy::OpenAiCompatible,
        request_body: RequestBodyPolicy::StandardOpenAi,
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::ModelDependent,
        tool_messages: ToolMessageSupport::OpenAiToolRole,
        model_fallbacks: &["grok-4", "grok-3"],
        max_output_tokens_default: Some(32_768),
    },
    ProviderDefinition {
        id: "groq",
        label: "Groq",
        default_model: "llama-3.3-70b-versatile",
        default_base_url: Some("https://api.groq.com/openai/v1"),
        api_key_env: &["GROQ_API_KEY"],
        base_url_env: &["GROQ_BASE_URL"],
        transport: ProviderTransport::OpenAiChatCompletions,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: false,
        supports_usage: true,
        context_window_tokens: Some(128_000),
        message_preparation: MessagePreparationPolicy::OpenAiCompatible,
        request_body: RequestBodyPolicy::StandardOpenAi,
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::ModelDependent,
        tool_messages: ToolMessageSupport::OpenAiToolRole,
        model_fallbacks: &["llama-3.3-70b-versatile", "openai/gpt-oss-120b"],
        max_output_tokens_default: Some(32_768),
    },
    ProviderDefinition {
        id: "mistral",
        label: "Mistral",
        default_model: "mistral-large-latest",
        default_base_url: Some("https://api.mistral.ai/v1"),
        api_key_env: &["MISTRAL_API_KEY"],
        base_url_env: &["MISTRAL_BASE_URL"],
        transport: ProviderTransport::OpenAiChatCompletions,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: false,
        supports_usage: true,
        context_window_tokens: Some(128_000),
        message_preparation: MessagePreparationPolicy::OpenAiCompatible,
        request_body: RequestBodyPolicy::StandardOpenAi,
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::ModelDependent,
        tool_messages: ToolMessageSupport::OpenAiToolRole,
        model_fallbacks: &["mistral-large-latest", "codestral-latest"],
        max_output_tokens_default: Some(32_768),
    },
    ProviderDefinition {
        id: "ollama",
        label: "Ollama",
        default_model: "llama3.1",
        default_base_url: Some("http://localhost:11434"),
        api_key_env: &[],
        base_url_env: &["OLLAMA_BASE_URL"],
        transport: ProviderTransport::CustomAnthropicCompatible,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: false,
        supports_usage: false,
        context_window_tokens: None,
        message_preparation: MessagePreparationPolicy::AnthropicCompatible,
        request_body: RequestBodyPolicy::CustomCompatible,
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
        health_check: HealthCheckSupport::HttpModelsEndpoint,
        vision: VisionSupport::ModelDependent,
        tool_messages: ToolMessageSupport::AnthropicBlocks,
        model_fallbacks: &["llama3.1", "qwen2.5-coder", "gpt-oss"],
        max_output_tokens_default: None,
    },
    ProviderDefinition {
        id: "custom_openai",
        label: "Custom OpenAI-Compatible",
        default_model: "custom-model",
        default_base_url: None,
        api_key_env: &["FORGE_CUSTOM_OPENAI_API_KEY", "OPENAI_API_KEY"],
        base_url_env: &["FORGE_CUSTOM_OPENAI_BASE_URL", "OPENAI_BASE_URL"],
        transport: ProviderTransport::CustomOpenAiCompatible,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: false,
        supports_usage: false,
        context_window_tokens: None,
        message_preparation: MessagePreparationPolicy::OpenAiCompatible,
        request_body: RequestBodyPolicy::CustomCompatible,
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::None,
        health_check: HealthCheckSupport::None,
        vision: VisionSupport::ModelDependent,
        tool_messages: ToolMessageSupport::OpenAiToolRole,
        model_fallbacks: &[],
        max_output_tokens_default: None,
    },
    ProviderDefinition {
        id: "custom_anthropic",
        label: "Custom Anthropic-Compatible",
        default_model: "custom-model",
        default_base_url: None,
        api_key_env: &["FORGE_CUSTOM_ANTHROPIC_API_KEY", "ANTHROPIC_API_KEY"],
        base_url_env: &["FORGE_CUSTOM_ANTHROPIC_BASE_URL", "ANTHROPIC_BASE_URL"],
        transport: ProviderTransport::CustomAnthropicCompatible,
        supports_streaming: true,
        supports_tools: true,
        supports_thinking: false,
        supports_usage: false,
        context_window_tokens: None,
        message_preparation: MessagePreparationPolicy::AnthropicCompatible,
        request_body: RequestBodyPolicy::CustomCompatible,
        top_level_request: TopLevelRequestPolicy::None,
        temperature: TemperaturePolicy::UserConfigurable,
        model_catalog: ModelCatalogPolicy::None,
        health_check: HealthCheckSupport::None,
        vision: VisionSupport::ModelDependent,
        tool_messages: ToolMessageSupport::AnthropicBlocks,
        model_fallbacks: &[],
        max_output_tokens_default: None,
    },
];

pub(crate) fn all_provider_definitions() -> &'static [ProviderDefinition] {
    PROVIDER_DEFINITIONS
}

pub(crate) fn valid_provider_ids() -> &'static [&'static str] {
    VALID_PROVIDER_IDS
}

pub(crate) fn normalize_provider_id(id_or_alias: Option<&str>) -> Option<&'static str> {
    let raw = id_or_alias.unwrap_or("").trim();
    if raw.is_empty() {
        return Some("deepseek");
    }

    match raw.to_ascii_lowercase().as_str() {
        "deepseek" => Some("deepseek"),
        "anthropic" | "claude" => Some("anthropic"),
        "kimi" | "moonshot" => Some("kimi"),
        "glm" | "zhipu" | "z.ai" | "zai" | "z-ai" => Some("glm"),
        "alibaba" | "qwen" | "dashscope" => Some("alibaba"),
        "minimax" => Some("minimax"),
        "openai" | "gpt" => Some("openai"),
        "openrouter" => Some("openrouter"),
        "gemini" | "google" => Some("gemini"),
        "xai" | "grok" | "x.ai" => Some("xai"),
        "groq" => Some("groq"),
        "mistral" => Some("mistral"),
        "ollama" | "local" | "vllm" | "lmstudio" | "llama.cpp" => Some("ollama"),
        "custom_openai" | "custom-openai" | "openai_compatible" => Some("custom_openai"),
        "custom_anthropic" | "custom-anthropic" | "anthropic_compatible" => {
            Some("custom_anthropic")
        }
        _ => None,
    }
}

pub(crate) fn get_provider_definition(id_or_alias: &str) -> Option<&'static ProviderDefinition> {
    let normalized = normalize_provider_id(Some(id_or_alias))?;
    PROVIDER_DEFINITIONS
        .iter()
        .find(|definition| definition.id == normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ExpectedProvider {
        id: &'static str,
        label: &'static str,
        default_model: &'static str,
        default_base_url: Option<&'static str>,
        api_key_env: &'static [&'static str],
        base_url_env: &'static [&'static str],
        transport: ProviderTransport,
        supports_streaming: bool,
        supports_tools: bool,
        supports_thinking: bool,
        supports_usage: bool,
        context_window_tokens: Option<u32>,
        message_preparation: MessagePreparationPolicy,
        request_body: RequestBodyPolicy,
        top_level_request: TopLevelRequestPolicy,
        temperature: TemperaturePolicy,
        model_catalog: ModelCatalogPolicy,
        health_check: HealthCheckSupport,
        vision: VisionSupport,
        tool_messages: ToolMessageSupport,
        model_fallbacks: &'static [&'static str],
        max_output_tokens_default: Option<u32>,
    }

    const EXPECTED_PROVIDERS: &[ExpectedProvider] = &[
        ExpectedProvider {
            id: "deepseek",
            label: "DeepSeek",
            default_model: "deepseek-v4-flash[1m]",
            default_base_url: Some("https://api.deepseek.com/anthropic"),
            api_key_env: &["DEEPSEEK_API_KEY"],
            base_url_env: &["DEEPSEEK_BASE_URL"],
            transport: ProviderTransport::AnthropicMessages,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: true,
            supports_usage: true,
            context_window_tokens: Some(1_000_000),
            message_preparation: MessagePreparationPolicy::AnthropicCompatible,
            request_body: RequestBodyPolicy::DeepSeekAnthropic {
                thinking_budget_tokens: Some(16_000),
            },
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::Omit,
            model_catalog: ModelCatalogPolicy::StaticFallback,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::Unsupported,
            tool_messages: ToolMessageSupport::AnthropicBlocks,
            model_fallbacks: &["deepseek-v4-flash[1m]", "deepseek-v4-pro", "deepseek-chat"],
            max_output_tokens_default: Some(384_000),
        },
        ExpectedProvider {
            id: "anthropic",
            label: "Anthropic",
            default_model: "claude-sonnet-4-6",
            default_base_url: Some("https://api.anthropic.com"),
            api_key_env: &["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY"],
            base_url_env: &["ANTHROPIC_BASE_URL"],
            transport: ProviderTransport::AnthropicMessages,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: true,
            supports_usage: true,
            context_window_tokens: Some(200_000),
            message_preparation: MessagePreparationPolicy::AnthropicCompatible,
            request_body: RequestBodyPolicy::StandardAnthropic,
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::Supported,
            tool_messages: ToolMessageSupport::AnthropicBlocks,
            model_fallbacks: &["claude-sonnet-4-6", "claude-opus-4-8"],
            max_output_tokens_default: None,
        },
        ExpectedProvider {
            id: "kimi",
            label: "Kimi / Moonshot",
            default_model: "kimi-k2.5",
            default_base_url: Some("https://api.moonshot.cn/anthropic"),
            api_key_env: &["KIMI_API_KEY", "MOONSHOT_API_KEY"],
            base_url_env: &["KIMI_BASE_URL", "MOONSHOT_BASE_URL"],
            transport: ProviderTransport::AnthropicMessages,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: true,
            supports_usage: true,
            context_window_tokens: Some(128_000),
            message_preparation: MessagePreparationPolicy::AnthropicCompatible,
            request_body: RequestBodyPolicy::KimiAnthropic,
            top_level_request: TopLevelRequestPolicy::KimiReasoningEffort,
            temperature: TemperaturePolicy::Omit,
            model_catalog: ModelCatalogPolicy::StaticFallback,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::ModelDependent,
            tool_messages: ToolMessageSupport::AnthropicBlocks,
            model_fallbacks: &["kimi-k2.5", "kimi-k2", "moonshot-v1-32k"],
            max_output_tokens_default: Some(32_768),
        },
        ExpectedProvider {
            id: "glm",
            label: "GLM / Zhipu",
            default_model: "glm-4.5",
            default_base_url: Some("https://open.bigmodel.cn/api/anthropic"),
            api_key_env: &["GLM_API_KEY", "ZHIPU_API_KEY"],
            base_url_env: &["GLM_BASE_URL", "ZHIPU_BASE_URL"],
            transport: ProviderTransport::AnthropicMessages,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: true,
            supports_usage: true,
            context_window_tokens: Some(128_000),
            message_preparation: MessagePreparationPolicy::AnthropicCompatible,
            request_body: RequestBodyPolicy::GlmAnthropic,
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::StaticFallback,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::ModelDependent,
            tool_messages: ToolMessageSupport::AnthropicBlocks,
            model_fallbacks: &["glm-4.5", "glm-4.5-air", "glm-4-plus"],
            max_output_tokens_default: Some(32_768),
        },
        ExpectedProvider {
            id: "alibaba",
            label: "Alibaba / Qwen",
            default_model: "qwen3-coder-plus",
            default_base_url: Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
            api_key_env: &["ALIBABA_API_KEY", "DASHSCOPE_API_KEY"],
            base_url_env: &["ALIBABA_BASE_URL", "DASHSCOPE_BASE_URL"],
            transport: ProviderTransport::OpenAiChatCompletions,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: true,
            supports_usage: true,
            context_window_tokens: Some(128_000),
            message_preparation: MessagePreparationPolicy::OpenAiCompatible,
            request_body: RequestBodyPolicy::StandardOpenAi,
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::ModelDependent,
            tool_messages: ToolMessageSupport::OpenAiToolRole,
            model_fallbacks: &["qwen3-coder-plus", "qwen-max", "qwen-plus"],
            max_output_tokens_default: Some(32_768),
        },
        ExpectedProvider {
            id: "minimax",
            label: "MiniMax",
            default_model: "MiniMax-M2.7",
            default_base_url: Some("https://api.minimax.io/anthropic"),
            api_key_env: &["MINIMAX_API_KEY", "MINIMAX_CN_API_KEY"],
            base_url_env: &["MINIMAX_BASE_URL", "MINIMAX_CN_BASE_URL"],
            transport: ProviderTransport::AnthropicMessages,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: true,
            supports_usage: true,
            context_window_tokens: Some(128_000),
            message_preparation: MessagePreparationPolicy::AnthropicCompatible,
            request_body: RequestBodyPolicy::MinimaxAnthropic,
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::StaticFallback,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::ModelDependent,
            tool_messages: ToolMessageSupport::AnthropicBlocks,
            model_fallbacks: &["MiniMax-M2.7", "MiniMax-M1"],
            max_output_tokens_default: Some(32_768),
        },
        ExpectedProvider {
            id: "openai",
            label: "OpenAI",
            default_model: "gpt-4o",
            default_base_url: Some("https://api.openai.com/v1"),
            api_key_env: &["OPENAI_API_KEY"],
            base_url_env: &["OPENAI_BASE_URL"],
            transport: ProviderTransport::OpenAiChatCompletions,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: false,
            supports_usage: true,
            context_window_tokens: Some(128_000),
            message_preparation: MessagePreparationPolicy::OpenAiCompatible,
            request_body: RequestBodyPolicy::StandardOpenAi,
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::Supported,
            tool_messages: ToolMessageSupport::OpenAiToolRole,
            model_fallbacks: &["gpt-4o", "gpt-4o-mini"],
            max_output_tokens_default: None,
        },
        ExpectedProvider {
            id: "openrouter",
            label: "OpenRouter",
            default_model: "openai/gpt-4o-mini",
            default_base_url: Some("https://openrouter.ai/api/v1"),
            api_key_env: &["OPENROUTER_API_KEY"],
            base_url_env: &["OPENROUTER_BASE_URL"],
            transport: ProviderTransport::OpenAiChatCompletions,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: true,
            supports_usage: true,
            context_window_tokens: Some(128_000),
            message_preparation: MessagePreparationPolicy::OpenAiCompatible,
            request_body: RequestBodyPolicy::OpenRouterExtensions,
            top_level_request: TopLevelRequestPolicy::OpenRouterProviderHints,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::ModelDependent,
            tool_messages: ToolMessageSupport::OpenAiToolRole,
            model_fallbacks: &[
                "openai/gpt-4o-mini",
                "anthropic/claude-sonnet-4",
                "google/gemini-2.5-pro",
            ],
            max_output_tokens_default: None,
        },
        ExpectedProvider {
            id: "gemini",
            label: "Gemini",
            default_model: "gemini-2.5-pro",
            default_base_url: Some("https://generativelanguage.googleapis.com/v1beta/openai"),
            api_key_env: &["GEMINI_API_KEY"],
            base_url_env: &["GEMINI_BASE_URL"],
            transport: ProviderTransport::OpenAiChatCompletions,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: true,
            supports_usage: true,
            context_window_tokens: Some(1_000_000),
            message_preparation: MessagePreparationPolicy::GeminiOpenAiCompatible,
            request_body: RequestBodyPolicy::StandardOpenAi,
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::Supported,
            tool_messages: ToolMessageSupport::OpenAiToolRole,
            model_fallbacks: &["gemini-2.5-pro", "gemini-2.5-flash"],
            max_output_tokens_default: Some(65_536),
        },
        ExpectedProvider {
            id: "xai",
            label: "xAI",
            default_model: "grok-4",
            default_base_url: Some("https://api.x.ai/v1"),
            api_key_env: &["XAI_API_KEY"],
            base_url_env: &["XAI_BASE_URL"],
            transport: ProviderTransport::OpenAiChatCompletions,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: true,
            supports_usage: true,
            context_window_tokens: Some(128_000),
            message_preparation: MessagePreparationPolicy::OpenAiCompatible,
            request_body: RequestBodyPolicy::StandardOpenAi,
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::ModelDependent,
            tool_messages: ToolMessageSupport::OpenAiToolRole,
            model_fallbacks: &["grok-4", "grok-3"],
            max_output_tokens_default: Some(32_768),
        },
        ExpectedProvider {
            id: "groq",
            label: "Groq",
            default_model: "llama-3.3-70b-versatile",
            default_base_url: Some("https://api.groq.com/openai/v1"),
            api_key_env: &["GROQ_API_KEY"],
            base_url_env: &["GROQ_BASE_URL"],
            transport: ProviderTransport::OpenAiChatCompletions,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: false,
            supports_usage: true,
            context_window_tokens: Some(128_000),
            message_preparation: MessagePreparationPolicy::OpenAiCompatible,
            request_body: RequestBodyPolicy::StandardOpenAi,
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::ModelDependent,
            tool_messages: ToolMessageSupport::OpenAiToolRole,
            model_fallbacks: &["llama-3.3-70b-versatile", "openai/gpt-oss-120b"],
            max_output_tokens_default: Some(32_768),
        },
        ExpectedProvider {
            id: "mistral",
            label: "Mistral",
            default_model: "mistral-large-latest",
            default_base_url: Some("https://api.mistral.ai/v1"),
            api_key_env: &["MISTRAL_API_KEY"],
            base_url_env: &["MISTRAL_BASE_URL"],
            transport: ProviderTransport::OpenAiChatCompletions,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: false,
            supports_usage: true,
            context_window_tokens: Some(128_000),
            message_preparation: MessagePreparationPolicy::OpenAiCompatible,
            request_body: RequestBodyPolicy::StandardOpenAi,
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::ModelDependent,
            tool_messages: ToolMessageSupport::OpenAiToolRole,
            model_fallbacks: &["mistral-large-latest", "codestral-latest"],
            max_output_tokens_default: Some(32_768),
        },
        ExpectedProvider {
            id: "ollama",
            label: "Ollama",
            default_model: "llama3.1",
            default_base_url: Some("http://localhost:11434"),
            api_key_env: &[],
            base_url_env: &["OLLAMA_BASE_URL"],
            transport: ProviderTransport::CustomAnthropicCompatible,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: false,
            supports_usage: false,
            context_window_tokens: None,
            message_preparation: MessagePreparationPolicy::AnthropicCompatible,
            request_body: RequestBodyPolicy::CustomCompatible,
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::HttpModelsEndpoint,
            health_check: HealthCheckSupport::HttpModelsEndpoint,
            vision: VisionSupport::ModelDependent,
            tool_messages: ToolMessageSupport::AnthropicBlocks,
            model_fallbacks: &["llama3.1", "qwen2.5-coder", "gpt-oss"],
            max_output_tokens_default: None,
        },
        ExpectedProvider {
            id: "custom_openai",
            label: "Custom OpenAI-Compatible",
            default_model: "custom-model",
            default_base_url: None,
            api_key_env: &["FORGE_CUSTOM_OPENAI_API_KEY", "OPENAI_API_KEY"],
            base_url_env: &["FORGE_CUSTOM_OPENAI_BASE_URL", "OPENAI_BASE_URL"],
            transport: ProviderTransport::CustomOpenAiCompatible,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: false,
            supports_usage: false,
            context_window_tokens: None,
            message_preparation: MessagePreparationPolicy::OpenAiCompatible,
            request_body: RequestBodyPolicy::CustomCompatible,
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::None,
            health_check: HealthCheckSupport::None,
            vision: VisionSupport::ModelDependent,
            tool_messages: ToolMessageSupport::OpenAiToolRole,
            model_fallbacks: &[],
            max_output_tokens_default: None,
        },
        ExpectedProvider {
            id: "custom_anthropic",
            label: "Custom Anthropic-Compatible",
            default_model: "custom-model",
            default_base_url: None,
            api_key_env: &["FORGE_CUSTOM_ANTHROPIC_API_KEY", "ANTHROPIC_API_KEY"],
            base_url_env: &["FORGE_CUSTOM_ANTHROPIC_BASE_URL", "ANTHROPIC_BASE_URL"],
            transport: ProviderTransport::CustomAnthropicCompatible,
            supports_streaming: true,
            supports_tools: true,
            supports_thinking: false,
            supports_usage: false,
            context_window_tokens: None,
            message_preparation: MessagePreparationPolicy::AnthropicCompatible,
            request_body: RequestBodyPolicy::CustomCompatible,
            top_level_request: TopLevelRequestPolicy::None,
            temperature: TemperaturePolicy::UserConfigurable,
            model_catalog: ModelCatalogPolicy::None,
            health_check: HealthCheckSupport::None,
            vision: VisionSupport::ModelDependent,
            tool_messages: ToolMessageSupport::AnthropicBlocks,
            model_fallbacks: &[],
            max_output_tokens_default: None,
        },
    ];

    fn definition(id: &str) -> &'static ProviderDefinition {
        get_provider_definition(id).unwrap_or_else(|| panic!("missing provider definition: {id}"))
    }

    #[test]
    fn provider_registry_exposes_stable_builtin_ids() {
        assert_eq!(
            valid_provider_ids(),
            &[
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
            ]
        );
        assert_eq!(all_provider_definitions().len(), valid_provider_ids().len());
        assert_eq!(valid_provider_ids().len(), EXPECTED_PROVIDERS.len());
    }

    #[test]
    fn provider_registry_normalizes_required_aliases() {
        assert_eq!(normalize_provider_id(None), Some("deepseek"));
        assert_eq!(normalize_provider_id(Some("")), Some("deepseek"));
        assert_eq!(normalize_provider_id(Some("  CLAUDE  ")), Some("anthropic"));
        assert_eq!(normalize_provider_id(Some("gpt")), Some("openai"));
        assert_eq!(normalize_provider_id(Some("zhipu")), Some("glm"));
        assert_eq!(normalize_provider_id(Some("z.ai")), Some("glm"));
        assert_eq!(normalize_provider_id(Some("moonshot")), Some("kimi"));
        assert_eq!(normalize_provider_id(Some("qwen")), Some("alibaba"));
        assert_eq!(normalize_provider_id(Some("dashscope")), Some("alibaba"));
        assert_eq!(normalize_provider_id(Some("grok")), Some("xai"));
        assert_eq!(normalize_provider_id(Some("x.ai")), Some("xai"));
        assert_eq!(normalize_provider_id(Some("local")), Some("ollama"));
        assert_eq!(normalize_provider_id(Some("vllm")), Some("ollama"));
        assert_eq!(normalize_provider_id(Some("lmstudio")), Some("ollama"));
        assert_eq!(normalize_provider_id(Some("llama.cpp")), Some("ollama"));
        assert_eq!(normalize_provider_id(Some("nvidia")), None);
        assert_eq!(normalize_provider_id(Some("nim")), None);
        assert_eq!(normalize_provider_id(Some("unknown")), None);
    }

    #[test]
    fn provider_registry_returns_definitions_by_id_or_alias() {
        assert_eq!(get_provider_definition("claude").unwrap().id, "anthropic");
        assert_eq!(get_provider_definition("gpt").unwrap().id, "openai");
        assert_eq!(get_provider_definition("moonshot").unwrap().id, "kimi");
        assert_eq!(get_provider_definition("qwen").unwrap().id, "alibaba");
        assert!(get_provider_definition("nvidia").is_none());
        assert!(get_provider_definition("unknown").is_none());
    }

    #[test]
    fn every_task_one_provider_definition_is_pinned() {
        for expected in EXPECTED_PROVIDERS {
            let actual = definition(expected.id);

            assert_eq!(actual.id, expected.id, "id for {}", expected.id);
            assert_eq!(actual.label, expected.label, "label for {}", expected.id);
            assert_eq!(
                actual.default_model, expected.default_model,
                "default model for {}",
                expected.id
            );
            assert_eq!(
                actual.default_base_url, expected.default_base_url,
                "default base URL for {}",
                expected.id
            );
            assert_eq!(
                actual.api_key_env, expected.api_key_env,
                "API key env vars for {}",
                expected.id
            );
            assert_eq!(
                actual.base_url_env, expected.base_url_env,
                "base URL env vars for {}",
                expected.id
            );
            assert_eq!(
                actual.transport, expected.transport,
                "transport for {}",
                expected.id
            );
            assert_eq!(
                actual.supports_streaming, expected.supports_streaming,
                "streaming support for {}",
                expected.id
            );
            assert_eq!(
                actual.supports_tools, expected.supports_tools,
                "tools support for {}",
                expected.id
            );
            assert_eq!(
                actual.supports_thinking, expected.supports_thinking,
                "thinking support for {}",
                expected.id
            );
            assert_eq!(
                actual.supports_usage, expected.supports_usage,
                "usage support for {}",
                expected.id
            );
            assert_eq!(
                actual.context_window_tokens, expected.context_window_tokens,
                "context window for {}",
                expected.id
            );
            assert_eq!(
                actual.message_preparation, expected.message_preparation,
                "message preparation policy for {}",
                expected.id
            );
            assert_eq!(
                actual.request_body, expected.request_body,
                "request body policy for {}",
                expected.id
            );
            assert_eq!(
                actual.top_level_request, expected.top_level_request,
                "top-level request policy for {}",
                expected.id
            );
            assert_eq!(
                actual.temperature, expected.temperature,
                "temperature policy for {}",
                expected.id
            );
            assert_eq!(
                actual.model_catalog, expected.model_catalog,
                "model catalog policy for {}",
                expected.id
            );
            assert_eq!(
                actual.health_check, expected.health_check,
                "health check support for {}",
                expected.id
            );
            assert_eq!(
                actual.vision, expected.vision,
                "vision support for {}",
                expected.id
            );
            assert_eq!(
                actual.tool_messages, expected.tool_messages,
                "tool message support for {}",
                expected.id
            );
            assert_eq!(
                actual.model_fallbacks, expected.model_fallbacks,
                "fallback models for {}",
                expected.id
            );
            assert_eq!(
                actual.max_output_tokens_default, expected.max_output_tokens_default,
                "max output token default for {}",
                expected.id
            );
        }
    }
}
