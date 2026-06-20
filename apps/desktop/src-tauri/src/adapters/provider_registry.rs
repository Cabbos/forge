use serde::{Deserialize, Serialize};

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

impl ProviderTransport {
    fn from_config_name(value: &str) -> Option<Self> {
        match normalize_profile_key(value).as_str() {
            "anthropic" | "anthropic_messages" | "claude" => Some(Self::AnthropicMessages),
            "openai" | "openai_chat_completions" | "chat_completions" => {
                Some(Self::OpenAiChatCompletions)
            }
            "openai_responses" | "responses" => Some(Self::OpenAiResponses),
            "native_gemini" | "gemini" => Some(Self::NativeGemini),
            "bedrock" | "bedrock_converse" => Some(Self::BedrockConverse),
            "custom_openai" | "custom_openai_compatible" | "openai_compatible" => {
                Some(Self::CustomOpenAiCompatible)
            }
            "custom_anthropic" | "custom_anthropic_compatible" | "anthropic_compatible" => {
                Some(Self::CustomAnthropicCompatible)
            }
            _ => None,
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub(crate) enum EnvVarList {
    One(String),
    Many(Vec<String>),
}

impl EnvVarList {
    fn to_vec(&self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value.clone()],
            Self::Many(values) => values.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct ProviderProfileConfig {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) label: Option<String>,
    #[serde(default, alias = "default_base_url")]
    pub(crate) base_url: Option<String>,
    #[serde(default)]
    pub(crate) api_key_env: Option<EnvVarList>,
    #[serde(default)]
    pub(crate) base_url_env: Option<EnvVarList>,
    #[serde(default)]
    pub(crate) default_model: Option<String>,
    #[serde(default)]
    pub(crate) transport: Option<String>,
    #[serde(default)]
    pub(crate) supports_tools: Option<bool>,
    #[serde(default)]
    pub(crate) supports_streaming: Option<bool>,
    #[serde(default)]
    pub(crate) max_output_tokens_default: Option<u32>,
    #[serde(default)]
    pub(crate) aliases: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderProfileSource {
    BuiltIn,
    UserOverride,
    UserDefined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoadedProviderProfile {
    pub(crate) id: String,
    pub(crate) aliases: Vec<String>,
    pub(crate) label: String,
    pub(crate) default_model: String,
    pub(crate) default_base_url: Option<String>,
    pub(crate) api_key_env: Vec<String>,
    pub(crate) base_url_env: Vec<String>,
    pub(crate) transport: ProviderTransport,
    pub(crate) supports_streaming: bool,
    pub(crate) supports_tools: bool,
    pub(crate) max_output_tokens_default: Option<u32>,
    pub(crate) source: ProviderProfileSource,
}

impl LoadedProviderProfile {
    fn from_builtin(definition: &ProviderDefinition) -> Self {
        Self {
            id: definition.id.to_string(),
            aliases: vec![],
            label: definition.label.to_string(),
            default_model: definition.default_model.to_string(),
            default_base_url: definition.default_base_url.map(str::to_string),
            api_key_env: definition
                .api_key_env
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            base_url_env: definition
                .base_url_env
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            transport: definition.transport,
            supports_streaming: definition.supports_streaming,
            supports_tools: definition.supports_tools,
            max_output_tokens_default: definition.max_output_tokens_default,
            source: ProviderProfileSource::BuiltIn,
        }
    }

    fn from_user_config(
        id: String,
        source_alias: String,
        config: &ProviderProfileConfig,
    ) -> Result<Self, ProviderProfileLoadError> {
        let mut profile = Self {
            id,
            aliases: vec![],
            label: config
                .label
                .clone()
                .unwrap_or_else(|| config.id.trim().to_string()),
            default_model: config
                .default_model
                .clone()
                .unwrap_or_else(|| "custom-model".to_string()),
            default_base_url: config.base_url.clone(),
            api_key_env: config
                .api_key_env
                .as_ref()
                .map(EnvVarList::to_vec)
                .unwrap_or_default(),
            base_url_env: config
                .base_url_env
                .as_ref()
                .map(EnvVarList::to_vec)
                .unwrap_or_default(),
            transport: parse_transport(config)?
                .unwrap_or(ProviderTransport::CustomOpenAiCompatible),
            supports_streaming: config.supports_streaming.unwrap_or(true),
            supports_tools: config.supports_tools.unwrap_or(true),
            max_output_tokens_default: config.max_output_tokens_default,
            source: ProviderProfileSource::UserDefined,
        };
        profile.add_alias(source_alias);
        for alias in &config.aliases {
            profile.add_alias(normalize_profile_key(alias));
        }
        Ok(profile)
    }

    fn apply_user_override(
        &mut self,
        source_alias: String,
        config: &ProviderProfileConfig,
    ) -> Result<(), ProviderProfileLoadError> {
        if let Some(label) = &config.label {
            self.label = label.clone();
        }
        if let Some(base_url) = &config.base_url {
            self.default_base_url = Some(base_url.clone());
        }
        if let Some(api_key_env) = &config.api_key_env {
            self.api_key_env = api_key_env.to_vec();
        }
        if let Some(base_url_env) = &config.base_url_env {
            self.base_url_env = base_url_env.to_vec();
        }
        if let Some(default_model) = &config.default_model {
            self.default_model = default_model.clone();
        }
        if let Some(transport) = parse_transport(config)? {
            self.transport = transport;
        }
        if let Some(supports_tools) = config.supports_tools {
            self.supports_tools = supports_tools;
        }
        if let Some(supports_streaming) = config.supports_streaming {
            self.supports_streaming = supports_streaming;
        }
        if let Some(max_output_tokens_default) = config.max_output_tokens_default {
            self.max_output_tokens_default = Some(max_output_tokens_default);
        }
        self.source = ProviderProfileSource::UserOverride;
        self.add_alias(source_alias);
        for alias in &config.aliases {
            self.add_alias(normalize_profile_key(alias));
        }
        Ok(())
    }

    fn add_alias(&mut self, alias: String) {
        if alias.is_empty() || alias == self.id || self.aliases.iter().any(|value| value == &alias)
        {
            return;
        }
        self.aliases.push(alias);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProviderProfileLoadError {
    MissingId,
    UnsupportedTransport { id: String, transport: String },
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

pub(crate) fn load_provider_profiles(
    configs: &[ProviderProfileConfig],
) -> Result<Vec<LoadedProviderProfile>, ProviderProfileLoadError> {
    let mut profiles = PROVIDER_DEFINITIONS
        .iter()
        .map(LoadedProviderProfile::from_builtin)
        .collect::<Vec<_>>();

    for config in configs {
        let source_alias = normalize_profile_key(&config.id);
        if source_alias.is_empty() {
            return Err(ProviderProfileLoadError::MissingId);
        }
        let id = normalize_provider_id(Some(&source_alias))
            .map(str::to_string)
            .unwrap_or_else(|| source_alias.clone());

        if let Some(existing) = profiles.iter_mut().find(|profile| profile.id == id) {
            existing.apply_user_override(source_alias, config)?;
        } else {
            profiles.push(LoadedProviderProfile::from_user_config(
                id,
                source_alias,
                config,
            )?);
        }
    }

    Ok(profiles)
}

pub(crate) fn find_loaded_provider_profile<'a>(
    profiles: &'a [LoadedProviderProfile],
    id_or_alias: &str,
) -> Option<&'a LoadedProviderProfile> {
    let key = normalize_profile_key(id_or_alias);
    if key.is_empty() {
        return profiles.iter().find(|profile| profile.id == "deepseek");
    }

    if let Some(normalized) = normalize_provider_id(Some(&key)) {
        if let Some(profile) = profiles.iter().find(|profile| profile.id == normalized) {
            return Some(profile);
        }
    }

    profiles
        .iter()
        .find(|profile| profile.id == key || profile.aliases.iter().any(|alias| alias == &key))
}

fn parse_transport(
    config: &ProviderProfileConfig,
) -> Result<Option<ProviderTransport>, ProviderProfileLoadError> {
    let Some(transport) = &config.transport else {
        return Ok(None);
    };
    ProviderTransport::from_config_name(transport)
        .map(Some)
        .ok_or_else(|| ProviderProfileLoadError::UnsupportedTransport {
            id: config.id.trim().to_string(),
            transport: transport.clone(),
        })
}

fn normalize_profile_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
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

    fn profile_config(id: &str) -> ProviderProfileConfig {
        ProviderProfileConfig {
            id: id.to_string(),
            label: Some(format!("Loaded {id}")),
            base_url: Some(format!("https://{id}.example.test/v1")),
            api_key_env: Some(EnvVarList::One(format!(
                "{}_API_KEY",
                id.replace(['-', '.'], "_").to_ascii_uppercase()
            ))),
            base_url_env: Some(EnvVarList::Many(vec![format!(
                "{}_BASE_URL",
                id.replace(['-', '.'], "_").to_ascii_uppercase()
            )])),
            default_model: Some(format!("{id}-model")),
            transport: Some("openai_chat_completions".to_string()),
            supports_tools: Some(false),
            supports_streaming: Some(false),
            max_output_tokens_default: Some(4096),
            aliases: vec![],
        }
    }

    #[test]
    fn provider_profile_loading_accepts_hermes_like_config_names() {
        let cases = [
            ("glm", "glm"),
            ("zhipu", "glm"),
            ("kimi", "kimi"),
            ("moonshot", "kimi"),
            ("qwen", "alibaba"),
            ("dashscope", "alibaba"),
            ("minimax", "minimax"),
            ("ollama", "ollama"),
            ("lmstudio", "ollama"),
            ("vllm", "ollama"),
        ];

        for (config_id, expected_id) in cases {
            let profiles = load_provider_profiles(&[profile_config(config_id)]).unwrap();
            let profile = find_loaded_provider_profile(&profiles, config_id)
                .unwrap_or_else(|| panic!("missing loaded profile for {config_id}"));

            assert_eq!(
                profile.id, expected_id,
                "canonical profile id for {config_id}"
            );
            assert_eq!(
                profile.label,
                format!("Loaded {config_id}"),
                "profile label override for {config_id}"
            );
            assert_eq!(
                profile.source,
                ProviderProfileSource::UserOverride,
                "profile source for {config_id}"
            );
        }
    }

    #[test]
    fn provider_profile_loading_overrides_only_safe_profile_fields() {
        let mut config = profile_config("moonshot");
        config.label = Some("Moonshot Private Endpoint".to_string());
        config.base_url = Some("https://moonshot.example.test/anthropic".to_string());
        config.api_key_env = Some(EnvVarList::Many(vec![
            "PRIVATE_KIMI_API_KEY".to_string(),
            "MOONSHOT_API_KEY".to_string(),
        ]));
        config.base_url_env = Some(EnvVarList::One("PRIVATE_KIMI_BASE_URL".to_string()));
        config.default_model = Some("kimi-private-coder".to_string());
        config.transport = Some("anthropic_messages".to_string());
        config.supports_tools = Some(true);
        config.supports_streaming = Some(true);
        config.max_output_tokens_default = Some(65_536);

        let profiles = load_provider_profiles(&[config]).unwrap();
        let profile = find_loaded_provider_profile(&profiles, "kimi").unwrap();

        assert_eq!(profile.id, "kimi");
        assert_eq!(profile.source, ProviderProfileSource::UserOverride);
        assert_eq!(profile.label, "Moonshot Private Endpoint");
        assert_eq!(
            profile.default_base_url.as_deref(),
            Some("https://moonshot.example.test/anthropic")
        );
        assert_eq!(
            profile.api_key_env,
            vec![
                "PRIVATE_KIMI_API_KEY".to_string(),
                "MOONSHOT_API_KEY".to_string()
            ]
        );
        assert_eq!(
            profile.base_url_env,
            vec!["PRIVATE_KIMI_BASE_URL".to_string()]
        );
        assert_eq!(profile.default_model, "kimi-private-coder");
        assert_eq!(profile.transport, ProviderTransport::AnthropicMessages);
        assert!(profile.supports_tools);
        assert!(profile.supports_streaming);
        assert_eq!(profile.max_output_tokens_default, Some(65_536));
    }

    #[test]
    fn provider_profile_loading_allows_nvidia_as_user_profile_not_builtin() {
        let mut config = profile_config("nvidia");
        config.label = Some("NVIDIA NIM".to_string());
        config.base_url = Some("https://integrate.api.nvidia.com/v1".to_string());
        config.default_model = Some("nvidia/llama-3.1-nemotron".to_string());
        config.aliases = vec!["nim".to_string()];

        let profiles = load_provider_profiles(&[config]).unwrap();
        let profile = find_loaded_provider_profile(&profiles, "nim").unwrap();

        assert_eq!(profile.id, "nvidia");
        assert_eq!(profile.source, ProviderProfileSource::UserDefined);
        assert_eq!(profile.label, "NVIDIA NIM");
        assert_eq!(
            profile.default_base_url.as_deref(),
            Some("https://integrate.api.nvidia.com/v1")
        );
        assert_eq!(profile.default_model, "nvidia/llama-3.1-nemotron");
        assert!(!valid_provider_ids().contains(&"nvidia"));
        assert!(get_provider_definition("nvidia").is_none());
    }

    #[test]
    fn provider_profile_loading_user_aliases_do_not_shadow_builtin_aliases() {
        let mut config = profile_config("my-provider");
        config.aliases = vec![
            "moonshot".to_string(),
            "claude".to_string(),
            "qwen".to_string(),
            "local".to_string(),
        ];

        let profiles = load_provider_profiles(&[config]).unwrap();

        assert_eq!(
            find_loaded_provider_profile(&profiles, "my-provider")
                .unwrap()
                .id,
            "my-provider"
        );
        assert_eq!(
            find_loaded_provider_profile(&profiles, "moonshot")
                .unwrap()
                .id,
            "kimi"
        );
        assert_eq!(
            find_loaded_provider_profile(&profiles, "claude")
                .unwrap()
                .id,
            "anthropic"
        );
        assert_eq!(
            find_loaded_provider_profile(&profiles, "qwen").unwrap().id,
            "alibaba"
        );
        assert_eq!(
            find_loaded_provider_profile(&profiles, "local").unwrap().id,
            "ollama"
        );
    }

    #[test]
    fn provider_profile_loading_rejects_unknown_transport_names() {
        let mut config = profile_config("my-provider");
        config.transport = Some("run_this_rust_hook".to_string());

        let error = load_provider_profiles(&[config]).unwrap_err();

        assert_eq!(
            error,
            ProviderProfileLoadError::UnsupportedTransport {
                id: "my-provider".to_string(),
                transport: "run_this_rust_hook".to_string(),
            }
        );
    }
}
