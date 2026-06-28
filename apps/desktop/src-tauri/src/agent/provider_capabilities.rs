const DEFAULT_PROVIDER: &str = "deepseek";

use crate::adapters::provider_registry::{get_provider_definition, normalize_provider_id};

pub(crate) fn normalize_provider(provider: Option<&str>) -> String {
    let raw = provider.unwrap_or(DEFAULT_PROVIDER).trim();
    normalize_provider_id(Some(raw))
        .map(str::to_string)
        .unwrap_or_else(|| raw.to_lowercase())
}

pub(crate) fn default_model(provider: &str) -> &'static str {
    get_provider_definition(&normalize_provider(Some(provider)))
        .or_else(|| get_provider_definition(DEFAULT_PROVIDER))
        .map(|definition| definition.default_model)
        .unwrap_or("deepseek-v4-flash[1m]")
}

pub(crate) fn context_window_tokens(provider: &str, model: &str) -> Option<u32> {
    let normalized = normalize_provider(Some(provider));
    match normalized.as_str() {
        "deepseek" if model.contains("[1m]") => Some(1_000_000),
        "deepseek" if model.contains("v4-pro") => Some(1_000_000),
        "deepseek" => Some(128_000),
        provider_id => get_provider_definition(provider_id)
            .and_then(|definition| definition.context_window_tokens),
    }
}

pub(crate) fn provider_label(provider: &str) -> &'static str {
    get_provider_definition(&normalize_provider(Some(provider)))
        .map(|definition| definition.label)
        .unwrap_or("provider")
}

pub(crate) fn missing_api_key_message(provider: &str) -> String {
    format!(
        "还没有配置 {} API Key。请打开设置，粘贴密钥后就可以开始发送。",
        provider_label(provider)
    )
}

pub(crate) fn is_context_overflow_error(_provider: &str, error_text: &str) -> bool {
    let error_text = error_text.to_lowercase();
    let rate_limit_only = [
        "429",
        "rate limit",
        "too many requests",
        "quota exceeded",
        "requests per",
    ]
    .iter()
    .any(|needle| error_text.contains(needle));
    let overflow = [
        "context length",
        "maximum context",
        "token limit",
        "too many tokens",
        "context_length_exceeded",
        "maximum context length",
        "prompt is too long",
        "input tokens",
        "context window",
    ]
    .iter()
    .any(|needle| error_text.contains(needle));

    overflow && !rate_limit_only
}

#[cfg(test)]
mod tests {
    use crate::adapters::provider_registry::{
        get_provider_definition, valid_provider_ids, ProviderDefinition,
    };

    use super::*;

    fn definition(id: &str) -> &'static ProviderDefinition {
        get_provider_definition(id).unwrap_or_else(|| panic!("missing provider definition: {id}"))
    }

    #[test]
    fn normalizes_provider_aliases() {
        assert_eq!(normalize_provider(Some("anthropic")), "anthropic");
        assert_eq!(normalize_provider(Some("claude")), "anthropic");
        assert_eq!(normalize_provider(Some("openai")), "openai");
        assert_eq!(normalize_provider(Some("gpt")), "openai");
        assert_eq!(normalize_provider(Some("openrouter")), "openrouter");
        assert_eq!(normalize_provider(Some("deepseek")), "deepseek");
        assert_eq!(normalize_provider(Some("")), "deepseek");
        assert_eq!(normalize_provider(None), "deepseek");
        assert_eq!(normalize_provider(Some("custom")), "custom");
    }

    #[test]
    fn normalizes_registry_provider_aliases() {
        assert_eq!(normalize_provider(Some("moonshot")), "kimi");
        assert_eq!(normalize_provider(Some("zhipu")), "glm");
        assert_eq!(normalize_provider(Some("z.ai")), "glm");
        assert_eq!(normalize_provider(Some("qwen")), "alibaba");
        assert_eq!(normalize_provider(Some("dashscope")), "alibaba");
        assert_eq!(normalize_provider(Some("grok")), "xai");
        assert_eq!(normalize_provider(Some("local")), "ollama");
        assert_eq!(normalize_provider(Some("lmstudio")), "ollama");
        assert_eq!(normalize_provider(Some("custom-openai")), "custom_openai");
        assert_eq!(
            normalize_provider(Some("custom-anthropic")),
            "custom_anthropic"
        );
    }

    #[test]
    fn provider_metadata_comes_from_registry_for_known_providers() {
        for provider in valid_provider_ids() {
            let definition = definition(provider);

            assert_eq!(
                default_model(provider),
                definition.default_model,
                "default model for {provider}"
            );
            assert_eq!(
                provider_label(provider),
                definition.label,
                "label for {provider}"
            );
            assert_eq!(
                context_window_tokens(provider, definition.default_model),
                definition.context_window_tokens,
                "context window for {provider}"
            );
        }
    }

    #[test]
    fn new_registry_provider_defaults_are_available() {
        assert_eq!(default_model("kimi"), "kimi-k2.7-code");
        assert_eq!(default_model("glm"), "glm-5.2");
        assert_eq!(default_model("alibaba"), "qwen3-coder-plus");
        assert_eq!(default_model("gemini"), "gemini-2.5-pro");
        assert_eq!(provider_label("minimax"), "MiniMax");
        assert_eq!(provider_label("custom_openai"), "Custom OpenAI-Compatible");
        assert_eq!(
            context_window_tokens("kimi", "kimi-k2.7-code"),
            Some(262_144)
        );
        assert_eq!(context_window_tokens("glm", "glm-5.2"), Some(1_000_000));
        assert_eq!(
            context_window_tokens("gemini", "gemini-2.5-pro"),
            Some(1_000_000)
        );
        assert_eq!(context_window_tokens("ollama", "llama3.1"), None);
    }

    #[test]
    fn deepseek_v4_flash_and_pro_one_million_context() {
        assert_eq!(
            context_window_tokens("deepseek", "deepseek-v4-flash[1m]"),
            Some(1_000_000)
        );
        assert_eq!(
            context_window_tokens("deepseek", "deepseek-v4-pro[1m]"),
            Some(1_000_000)
        );
        assert_eq!(
            context_window_tokens("deepseek", "deepseek-v4-pro"),
            Some(1_000_000)
        );
    }

    #[test]
    fn deepseek_other_models_default_to_128k() {
        assert_eq!(
            context_window_tokens("deepseek", "deepseek-chat"),
            Some(128_000)
        );
    }

    #[test]
    fn anthropic_and_openai_context_windows_are_defined() {
        assert_eq!(
            context_window_tokens("anthropic", "claude-sonnet-4-6"),
            Some(200_000)
        );
        assert_eq!(
            context_window_tokens("anthropic", "claude-opus-4-8"),
            Some(200_000)
        );
        assert_eq!(context_window_tokens("openai", "gpt-4o"), Some(128_000));
        assert_eq!(
            context_window_tokens("openai", "gpt-4o-mini"),
            Some(128_000)
        );
    }

    #[test]
    fn missing_key_message_is_chinese_and_contains_provider_label() {
        let message = missing_api_key_message("openrouter");

        assert!(message.contains("OpenRouter"));
        assert!(message.contains("还没有配置"));
        assert!(message.contains("密钥"));
    }

    #[test]
    fn overflow_classifier_matches_common_provider_errors() {
        assert!(is_context_overflow_error(
            "deepseek",
            "maximum context length exceeded: input tokens are too many"
        ));
        assert!(is_context_overflow_error(
            "openai",
            "context_length_exceeded: This model's maximum context length is 128000 tokens."
        ));
    }

    #[test]
    fn overflow_classifier_matches_anthropic_openrouter_and_generic_errors() {
        assert!(is_context_overflow_error(
            "anthropic",
            "prompt is too long for the model context window"
        ));
        assert!(is_context_overflow_error(
            "openrouter",
            "Token limit exceeded: too many tokens in request"
        ));
        assert!(is_context_overflow_error(
            "custom",
            "maximum context reached"
        ));
    }

    #[test]
    fn overflow_classifier_does_not_match_rate_limit_only() {
        assert!(!is_context_overflow_error(
            "openai",
            "429 rate limit exceeded, please try again later"
        ));
        assert!(!is_context_overflow_error(
            "deepseek",
            "Too Many Requests: rate limit"
        ));
    }
}
