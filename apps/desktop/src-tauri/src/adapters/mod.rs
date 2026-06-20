pub mod anthropic;
pub mod base;
pub mod missing_key;
pub mod openai_compatible;
pub mod provider_registry;

use std::sync::Arc;

use base::{AiAdapter, ToolDef};

/// DeepSeek Anthropic-compatible API (recommended by DeepSeek docs)
const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/anthropic";
const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

pub fn build_adapter(
    provider: &str,
    api_key: &str,
    model: &str,
    api_base: Option<&str>,
    external_tools: Vec<ToolDef>,
) -> Result<Arc<dyn AiAdapter>, String> {
    match provider {
        "deepseek" => {
            let adapter = anthropic::AnthropicAdapter::new(api_key.to_string())
                .map_err(|e| format!("API key error: {e}"))?
                .with_base_url(api_base.unwrap_or(DEEPSEEK_BASE_URL))
                .with_model(model)
                .with_external_tools(external_tools)
                .with_max_tokens(384_000)
                .with_thinking_budget_tokens(16_000);
            Ok(Arc::new(adapter))
        }
        "anthropic" => {
            let adapter = anthropic::AnthropicAdapter::new(api_key.to_string())
                .map_err(|e| format!("API key error: {e}"))?
                .with_base_url(api_base.unwrap_or("https://api.anthropic.com"))
                .with_model(model)
                .with_external_tools(external_tools);
            Ok(Arc::new(adapter))
        }
        "openai" => {
            let adapter = openai_compatible::OpenAiCompatibleAdapter::new(api_key.to_string())
                .map_err(|e| format!("API key error: {e}"))?
                .with_base_url(api_base.unwrap_or(OPENAI_BASE_URL))
                .with_model(model)
                .with_external_tools(external_tools);
            Ok(Arc::new(adapter))
        }
        "openrouter" => {
            let adapter = openai_compatible::OpenAiCompatibleAdapter::new(api_key.to_string())
                .map_err(|e| format!("API key error: {e}"))?
                .with_base_url(api_base.unwrap_or(OPENROUTER_BASE_URL))
                .with_model(model)
                .with_external_tools(external_tools);
            Ok(Arc::new(adapter))
        }
        other => Err(format!("Unsupported provider: {other}")),
    }
}
