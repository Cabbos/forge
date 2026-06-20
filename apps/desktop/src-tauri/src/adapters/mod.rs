pub mod anthropic;
pub mod base;
pub mod missing_key;
pub mod openai_compatible;
pub mod provider_registry;

use std::sync::Arc;

use base::{AiAdapter, ToolDef};
use provider_registry::{
    find_loaded_provider_profile, get_provider_definition, load_provider_profiles,
    LoadedProviderProfile, ProviderTransport, RequestBodyPolicy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildAdapterError {
    UnsupportedProvider {
        provider: String,
        valid_provider_ids: Vec<String>,
    },
    UnsupportedTransport {
        provider: String,
        transport: &'static str,
    },
    MissingBaseUrl {
        provider: String,
    },
    AdapterInit {
        message: String,
    },
}

impl std::fmt::Display for BuildAdapterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedProvider {
                provider,
                valid_provider_ids,
            } => write!(
                formatter,
                "Unsupported provider: {provider}. Valid providers: {}",
                valid_provider_ids.join(", ")
            ),
            Self::UnsupportedTransport {
                provider,
                transport,
            } => write!(
                formatter,
                "Unsupported transport for provider {provider}: {transport}"
            ),
            Self::MissingBaseUrl { provider } => {
                write!(formatter, "Missing base URL for provider: {provider}")
            }
            Self::AdapterInit { message } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for BuildAdapterError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdapterFamily {
    AnthropicCompatible,
    OpenAiCompatible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdapterRoute {
    provider_id: String,
    family: AdapterFamily,
    base_url: Option<String>,
    max_tokens: Option<u32>,
    thinking_budget_tokens: Option<u32>,
    thinking_disabled: bool,
    api_key_required: bool,
}

pub fn build_adapter(
    provider: &str,
    api_key: &str,
    model: &str,
    api_base: Option<&str>,
    external_tools: Vec<ToolDef>,
) -> Result<Arc<dyn AiAdapter>, BuildAdapterError> {
    let profiles = load_provider_profiles(&[]).map_err(|error| BuildAdapterError::AdapterInit {
        message: format!("Provider profile load error: {error:?}"),
    })?;
    build_adapter_with_profiles(
        provider,
        api_key,
        model,
        api_base,
        &profiles,
        external_tools,
    )
}

pub(crate) fn build_adapter_with_profiles(
    provider: &str,
    api_key: &str,
    model: &str,
    api_base: Option<&str>,
    profiles: &[LoadedProviderProfile],
    external_tools: Vec<ToolDef>,
) -> Result<Arc<dyn AiAdapter>, BuildAdapterError> {
    let route = resolve_adapter_route_with_profiles(provider, api_base, profiles)?;
    let base_url = route
        .base_url
        .as_deref()
        .ok_or_else(|| BuildAdapterError::MissingBaseUrl {
            provider: route.provider_id.clone(),
        })?;

    match route.family {
        AdapterFamily::AnthropicCompatible => {
            let adapter = if route.api_key_required {
                anthropic::AnthropicAdapter::new(api_key.to_string())
            } else {
                anthropic::AnthropicAdapter::new_allowing_empty_api_key(api_key.to_string())
            };
            let mut adapter = adapter
                .map_err(|error| BuildAdapterError::AdapterInit {
                    message: format!("API key error: {error}"),
                })?
                .with_provider_id(&route.provider_id)
                .with_base_url(base_url)
                .with_model(model)
                .with_external_tools(external_tools);
            if let Some(max_tokens) = route.max_tokens {
                adapter = adapter.with_max_tokens(max_tokens);
            }
            if let Some(thinking_budget_tokens) = route.thinking_budget_tokens {
                adapter = adapter.with_thinking_budget_tokens(thinking_budget_tokens);
            }
            if route.thinking_disabled {
                adapter = adapter.with_thinking_disabled();
            }
            Ok(Arc::new(adapter))
        }
        AdapterFamily::OpenAiCompatible => {
            let adapter = build_openai_compatible_adapter(&route, api_key, model, external_tools)?;
            Ok(Arc::new(adapter))
        }
    }
}

fn build_openai_compatible_adapter(
    route: &AdapterRoute,
    api_key: &str,
    model: &str,
    external_tools: Vec<ToolDef>,
) -> Result<openai_compatible::OpenAiCompatibleAdapter, BuildAdapterError> {
    let base_url = route
        .base_url
        .as_deref()
        .ok_or_else(|| BuildAdapterError::MissingBaseUrl {
            provider: route.provider_id.clone(),
        })?;
    let adapter = if route.api_key_required {
        openai_compatible::OpenAiCompatibleAdapter::new(api_key.to_string())
    } else {
        openai_compatible::OpenAiCompatibleAdapter::new_allowing_empty_api_key(api_key.to_string())
    };
    let mut adapter = adapter
        .map_err(|error| BuildAdapterError::AdapterInit {
            message: format!("API key error: {error}"),
        })?
        .with_provider_id(&route.provider_id)
        .with_base_url(base_url)
        .with_model(model)
        .with_external_tools(external_tools);
    if let Some(max_tokens) = route.max_tokens {
        adapter = adapter.with_max_tokens(max_tokens);
    }
    Ok(adapter)
}

fn resolve_adapter_route(
    provider: &str,
    api_base: Option<&str>,
) -> Result<AdapterRoute, BuildAdapterError> {
    let profiles = load_provider_profiles(&[]).map_err(|error| BuildAdapterError::AdapterInit {
        message: format!("Provider profile load error: {error:?}"),
    })?;
    resolve_adapter_route_with_profiles(provider, api_base, &profiles)
}

fn resolve_adapter_route_with_profiles(
    provider: &str,
    api_base: Option<&str>,
    profiles: &[LoadedProviderProfile],
) -> Result<AdapterRoute, BuildAdapterError> {
    let profile = find_loaded_provider_profile(profiles, provider).ok_or_else(|| {
        BuildAdapterError::UnsupportedProvider {
            provider: provider.to_string(),
            valid_provider_ids: profiles.iter().map(|profile| profile.id.clone()).collect(),
        }
    })?;
    let family = adapter_family(profile.transport).ok_or_else(|| {
        BuildAdapterError::UnsupportedTransport {
            provider: profile.id.clone(),
            transport: transport_label(profile.transport),
        }
    })?;
    let base_url = api_base
        .map(str::to_string)
        .or_else(|| profile.default_base_url.clone());
    if base_url.is_none() {
        return Err(BuildAdapterError::MissingBaseUrl {
            provider: profile.id.clone(),
        });
    }
    let definition = get_provider_definition(&profile.id);

    Ok(AdapterRoute {
        provider_id: profile.id.clone(),
        family,
        base_url,
        max_tokens: profile.max_output_tokens_default,
        thinking_budget_tokens: definition.and_then(|definition| match definition.request_body {
            RequestBodyPolicy::DeepSeekAnthropic {
                thinking_budget_tokens,
            } => thinking_budget_tokens,
            _ => None,
        }),
        thinking_disabled: family == AdapterFamily::AnthropicCompatible
            && definition.is_some_and(|definition| !definition.supports_thinking),
        api_key_required: !profile.api_key_env.is_empty(),
    })
}

fn adapter_family(transport: ProviderTransport) -> Option<AdapterFamily> {
    match transport {
        ProviderTransport::AnthropicMessages | ProviderTransport::CustomAnthropicCompatible => {
            Some(AdapterFamily::AnthropicCompatible)
        }
        ProviderTransport::OpenAiChatCompletions | ProviderTransport::CustomOpenAiCompatible => {
            Some(AdapterFamily::OpenAiCompatible)
        }
        ProviderTransport::OpenAiResponses
        | ProviderTransport::NativeGemini
        | ProviderTransport::BedrockConverse => None,
    }
}

fn transport_label(transport: ProviderTransport) -> &'static str {
    match transport {
        ProviderTransport::AnthropicMessages => "anthropic_messages",
        ProviderTransport::OpenAiChatCompletions => "openai_chat_completions",
        ProviderTransport::OpenAiResponses => "openai_responses",
        ProviderTransport::NativeGemini => "native_gemini",
        ProviderTransport::BedrockConverse => "bedrock_converse",
        ProviderTransport::CustomOpenAiCompatible => "custom_openai_compatible",
        ProviderTransport::CustomAnthropicCompatible => "custom_anthropic_compatible",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use provider_registry::{
        load_provider_profiles, valid_provider_ids, EnvVarList, ProviderProfileConfig,
    };

    #[test]
    fn build_adapter_routes_registry_providers_by_capability() {
        let cases = [
            (
                "deepseek",
                "deepseek",
                AdapterFamily::AnthropicCompatible,
                Some("https://api.deepseek.com/anthropic"),
                Some(384_000),
                Some(16_000),
                false,
            ),
            (
                "anthropic",
                "anthropic",
                AdapterFamily::AnthropicCompatible,
                Some("https://api.anthropic.com"),
                None,
                None,
                false,
            ),
            (
                "kimi",
                "kimi",
                AdapterFamily::AnthropicCompatible,
                Some("https://api.moonshot.cn/anthropic"),
                Some(32_768),
                None,
                false,
            ),
            (
                "moonshot",
                "kimi",
                AdapterFamily::AnthropicCompatible,
                Some("https://api.moonshot.cn/anthropic"),
                Some(32_768),
                None,
                false,
            ),
            (
                "glm",
                "glm",
                AdapterFamily::AnthropicCompatible,
                Some("https://open.bigmodel.cn/api/anthropic"),
                Some(32_768),
                None,
                false,
            ),
            (
                "minimax",
                "minimax",
                AdapterFamily::AnthropicCompatible,
                Some("https://api.minimax.io/anthropic"),
                Some(32_768),
                None,
                false,
            ),
            (
                "ollama",
                "ollama",
                AdapterFamily::AnthropicCompatible,
                Some("http://localhost:11434"),
                None,
                None,
                true,
            ),
            (
                "custom_anthropic",
                "custom_anthropic",
                AdapterFamily::AnthropicCompatible,
                Some("http://127.0.0.1:9000/anthropic"),
                None,
                None,
                true,
            ),
            (
                "openai",
                "openai",
                AdapterFamily::OpenAiCompatible,
                Some("https://api.openai.com/v1"),
                None,
                None,
                false,
            ),
            (
                "openrouter",
                "openrouter",
                AdapterFamily::OpenAiCompatible,
                Some("https://openrouter.ai/api/v1"),
                None,
                None,
                false,
            ),
            (
                "alibaba",
                "alibaba",
                AdapterFamily::OpenAiCompatible,
                Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
                Some(32_768),
                None,
                false,
            ),
            (
                "qwen",
                "alibaba",
                AdapterFamily::OpenAiCompatible,
                Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
                Some(32_768),
                None,
                false,
            ),
            (
                "gemini",
                "gemini",
                AdapterFamily::OpenAiCompatible,
                Some("https://generativelanguage.googleapis.com/v1beta/openai"),
                Some(65_536),
                None,
                false,
            ),
            (
                "xai",
                "xai",
                AdapterFamily::OpenAiCompatible,
                Some("https://api.x.ai/v1"),
                Some(32_768),
                None,
                false,
            ),
            (
                "groq",
                "groq",
                AdapterFamily::OpenAiCompatible,
                Some("https://api.groq.com/openai/v1"),
                Some(32_768),
                None,
                false,
            ),
            (
                "mistral",
                "mistral",
                AdapterFamily::OpenAiCompatible,
                Some("https://api.mistral.ai/v1"),
                Some(32_768),
                None,
                false,
            ),
            (
                "custom_openai",
                "custom_openai",
                AdapterFamily::OpenAiCompatible,
                Some("http://127.0.0.1:9000/v1"),
                None,
                None,
                false,
            ),
        ];

        for (
            provider,
            expected_provider_id,
            expected_family,
            expected_base_url,
            expected_max_tokens,
            expected_thinking_budget,
            expected_thinking_disabled,
        ) in cases
        {
            let api_base = match provider {
                "custom_anthropic" => Some("http://127.0.0.1:9000/anthropic"),
                "custom_openai" => Some("http://127.0.0.1:9000/v1"),
                _ => None,
            };
            let route = resolve_adapter_route(provider, api_base).unwrap();

            assert_eq!(
                route.provider_id, expected_provider_id,
                "provider id for {provider}"
            );
            assert_eq!(
                route.family, expected_family,
                "adapter family for {provider}"
            );
            assert_eq!(
                route.base_url.as_deref(),
                expected_base_url,
                "base URL for {provider}"
            );
            assert_eq!(
                route.max_tokens, expected_max_tokens,
                "max token default for {provider}"
            );
            assert_eq!(
                route.thinking_budget_tokens, expected_thinking_budget,
                "thinking budget for {provider}"
            );
            assert_eq!(
                route.thinking_disabled, expected_thinking_disabled,
                "thinking disabled flag for {provider}"
            );
        }
    }

    #[test]
    fn build_adapter_builds_every_registry_provider_without_network() {
        for provider in valid_provider_ids() {
            let api_base = match *provider {
                "custom_anthropic" => Some("http://127.0.0.1:9000/anthropic"),
                "custom_openai" => Some("http://127.0.0.1:9000/v1"),
                _ => None,
            };
            let adapter = build_adapter(provider, "test-key", "test-model", api_base, Vec::new())
                .unwrap_or_else(|error| panic!("failed to build provider {provider}: {error}"));

            assert_eq!(adapter.model_id(), "test-model", "model for {provider}");
        }
    }

    #[test]
    fn build_adapter_routes_user_defined_provider_profiles() {
        let profiles = load_provider_profiles(&[ProviderProfileConfig {
            id: "nvidia".to_string(),
            label: Some("NVIDIA NIM".to_string()),
            base_url: Some("https://integrate.api.nvidia.com/v1".to_string()),
            api_key_env: Some(EnvVarList::One("NVIDIA_API_KEY".to_string())),
            base_url_env: Some(EnvVarList::One("NVIDIA_BASE_URL".to_string())),
            default_model: Some("nvidia/llama-3.1-nemotron".to_string()),
            transport: Some("openai_chat_completions".to_string()),
            supports_tools: Some(true),
            supports_streaming: Some(true),
            max_output_tokens_default: Some(16_384),
            aliases: vec!["nim".to_string()],
        }])
        .unwrap();

        let route = resolve_adapter_route_with_profiles("nim", None, &profiles).unwrap();
        assert_eq!(route.provider_id, "nvidia");
        assert_eq!(route.family, AdapterFamily::OpenAiCompatible);
        assert_eq!(
            route.base_url.as_deref(),
            Some("https://integrate.api.nvidia.com/v1")
        );
        assert_eq!(route.max_tokens, Some(16_384));
        assert!(route.api_key_required);

        let adapter = build_adapter_with_profiles(
            "nim",
            "test-key",
            "nvidia/llama-3.1-nemotron",
            None,
            &profiles,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(adapter.model_id(), "nvidia/llama-3.1-nemotron");
    }

    #[test]
    fn build_adapter_allows_no_auth_user_defined_local_profiles() {
        let profiles = load_provider_profiles(&[ProviderProfileConfig {
            id: "local-openai".to_string(),
            label: Some("Local OpenAI".to_string()),
            base_url: Some("http://127.0.0.1:1234/v1".to_string()),
            api_key_env: Some(EnvVarList::Many(vec![])),
            base_url_env: None,
            default_model: Some("local-model".to_string()),
            transport: Some("openai_chat_completions".to_string()),
            supports_tools: Some(true),
            supports_streaming: Some(true),
            max_output_tokens_default: None,
            aliases: vec![],
        }])
        .unwrap();

        let route = resolve_adapter_route_with_profiles("local-openai", None, &profiles).unwrap();
        assert_eq!(route.provider_id, "local-openai");
        assert_eq!(route.family, AdapterFamily::OpenAiCompatible);
        assert!(!route.api_key_required);

        let adapter = build_adapter_with_profiles(
            "local-openai",
            "",
            "local-model",
            None,
            &profiles,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(adapter.model_id(), "local-model");
    }

    #[test]
    fn build_adapter_applies_registry_max_tokens_to_openai_compatible_runtime() {
        let route = resolve_adapter_route("gemini", None).unwrap();
        let adapter =
            build_openai_compatible_adapter(&route, "test-key", "test-model", Vec::new()).unwrap();

        assert_eq!(adapter.max_tokens_for_test(), 65_536);
    }

    #[test]
    fn build_adapter_carries_canonical_provider_id_into_openai_compatible_runtime() {
        let route = resolve_adapter_route("openai", None).unwrap();
        let adapter =
            build_openai_compatible_adapter(&route, "test-key", "test-model", Vec::new()).unwrap();

        assert_eq!(adapter.provider_id_for_test(), "openai");
    }

    #[test]
    fn build_adapter_returns_typed_unsupported_provider_error() {
        let error =
            match build_adapter("not-a-provider", "test-key", "test-model", None, Vec::new()) {
                Ok(_) => panic!("unsupported provider unexpectedly built an adapter"),
                Err(error) => error,
            };

        assert_eq!(
            error,
            BuildAdapterError::UnsupportedProvider {
                provider: "not-a-provider".to_string(),
                valid_provider_ids: valid_provider_ids()
                    .iter()
                    .map(|provider| (*provider).to_string())
                    .collect(),
            }
        );
    }
}

#[cfg(test)]
mod provider_conformance;
