pub mod anthropic;
pub mod base;
pub mod missing_key;
pub mod openai_compatible;
pub mod provider_registry;

use std::sync::Arc;

use base::{AiAdapter, ToolDef};
use provider_registry::{
    get_provider_definition, valid_provider_ids, ProviderTransport, RequestBodyPolicy,
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
    provider_id: &'static str,
    family: AdapterFamily,
    base_url: Option<String>,
    max_tokens: Option<u32>,
    thinking_budget_tokens: Option<u32>,
    thinking_disabled: bool,
}

pub fn build_adapter(
    provider: &str,
    api_key: &str,
    model: &str,
    api_base: Option<&str>,
    external_tools: Vec<ToolDef>,
) -> Result<Arc<dyn AiAdapter>, BuildAdapterError> {
    let route = resolve_adapter_route(provider, api_base)?;
    let base_url = route
        .base_url
        .as_deref()
        .ok_or_else(|| BuildAdapterError::MissingBaseUrl {
            provider: route.provider_id.to_string(),
        })?;

    match route.family {
        AdapterFamily::AnthropicCompatible => {
            let mut adapter = anthropic::AnthropicAdapter::new(api_key.to_string())
                .map_err(|error| BuildAdapterError::AdapterInit {
                    message: format!("API key error: {error}"),
                })?
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
            provider: route.provider_id.to_string(),
        })?;
    let mut adapter = openai_compatible::OpenAiCompatibleAdapter::new(api_key.to_string())
        .map_err(|error| BuildAdapterError::AdapterInit {
            message: format!("API key error: {error}"),
        })?
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
    let definition = get_provider_definition(provider).ok_or_else(|| {
        BuildAdapterError::UnsupportedProvider {
            provider: provider.to_string(),
            valid_provider_ids: valid_provider_ids()
                .iter()
                .map(|provider| (*provider).to_string())
                .collect(),
        }
    })?;
    let family = adapter_family(definition.transport).ok_or_else(|| {
        BuildAdapterError::UnsupportedTransport {
            provider: definition.id.to_string(),
            transport: transport_label(definition.transport),
        }
    })?;
    let base_url = api_base
        .map(str::to_string)
        .or_else(|| definition.default_base_url.map(str::to_string));
    if base_url.is_none() {
        return Err(BuildAdapterError::MissingBaseUrl {
            provider: definition.id.to_string(),
        });
    }

    Ok(AdapterRoute {
        provider_id: definition.id,
        family,
        base_url,
        max_tokens: definition.max_output_tokens_default,
        thinking_budget_tokens: match definition.request_body {
            RequestBodyPolicy::DeepSeekAnthropic {
                thinking_budget_tokens,
            } => thinking_budget_tokens,
            _ => None,
        },
        thinking_disabled: family == AdapterFamily::AnthropicCompatible
            && !definition.supports_thinking,
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
    use provider_registry::valid_provider_ids;

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
    fn build_adapter_applies_registry_max_tokens_to_openai_compatible_runtime() {
        let route = resolve_adapter_route("gemini", None).unwrap();
        let adapter =
            build_openai_compatible_adapter(&route, "test-key", "test-model", Vec::new()).unwrap();

        assert_eq!(adapter.max_tokens_for_test(), 65_536);
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
