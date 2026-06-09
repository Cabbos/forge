const DEFAULT_PROVIDER: &str = "deepseek";

pub(crate) fn normalize_provider(provider: Option<&str>) -> String {
    match provider
        .unwrap_or(DEFAULT_PROVIDER)
        .trim()
        .to_lowercase()
        .as_str()
    {
        "anthropic" | "claude" => "anthropic".to_string(),
        "openai" | "gpt" => "openai".to_string(),
        "openrouter" => "openrouter".to_string(),
        "deepseek" | "" => "deepseek".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn default_model(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "claude-sonnet-4-6",
        "openai" => "gpt-4o",
        "openrouter" => "openai/gpt-4o-mini",
        _ => "deepseek-v4-flash[1m]",
    }
}

pub(crate) fn context_window_tokens(provider: &str, model: &str) -> Option<u32> {
    match provider {
        "deepseek" if model.contains("[1m]") => Some(1_000_000),
        "deepseek" if model.contains("v4-pro") => Some(1_000_000),
        "deepseek" => Some(128_000),
        "anthropic" => Some(200_000),
        "openai" => Some(128_000),
        _ => None,
    }
}

pub(crate) fn provider_label(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "Anthropic",
        "openai" => "OpenAI",
        "openrouter" => "OpenRouter",
        "deepseek" => "DeepSeek",
        _ => "provider",
    }
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
    use super::*;

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
