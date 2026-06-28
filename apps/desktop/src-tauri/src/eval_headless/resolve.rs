use crate::agent::provider_capabilities::{default_model, normalize_provider};

use super::types::EvalHeadlessRequest;

pub(crate) fn resolve_prompt(request: &EvalHeadlessRequest) -> Result<String, String> {
    let prompt = request
        .task
        .as_ref()
        .and_then(|task| task.prompt.as_deref())
        .filter(|prompt| !prompt.trim().is_empty())
        .unwrap_or(&request.prompt)
        .trim()
        .to_string();
    if prompt.is_empty() {
        return Err("Forge eval request did not include a prompt.".to_string());
    }
    Ok(prompt)
}

pub(crate) fn resolve_agent_provider(display_provider: Option<&str>) -> String {
    let env_provider = std::env::var("FORGE_HEADLESS_PROVIDER")
        .or_else(|_| std::env::var("FORGE_EVAL_AI_PROVIDER"))
        .ok();
    let provider_hint = env_provider
        .as_deref()
        .or_else(|| display_provider.filter(|provider| provider != &"forge"));
    normalize_provider(provider_hint)
}

pub(crate) fn resolve_agent_model(
    display_model: Option<&str>,
    credential_model: Option<&str>,
    provider: &str,
) -> String {
    if let Some(model) = std::env::var("FORGE_HEADLESS_MODEL")
        .or_else(|_| std::env::var("FORGE_EVAL_AI_MODEL"))
        .ok()
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
    {
        return model;
    }

    credential_model
        .filter(|model| model_matches_provider(provider, model))
        .map(str::to_string)
        .or_else(|| {
            display_model
                .filter(|model| model != &"local-forge")
                .filter(|model| model_matches_provider(provider, model))
                .map(str::to_string)
        })
        .unwrap_or_else(|| default_headless_model(provider).to_string())
}

pub(crate) fn model_matches_provider(provider: &str, model: &str) -> bool {
    let model = model.trim().to_lowercase();
    if model.is_empty() {
        return false;
    }

    match provider {
        "deepseek" => model.starts_with("deepseek-"),
        "anthropic" => model.starts_with("claude"),
        "openai" => model.starts_with("gpt-") || model.starts_with("o1") || model.starts_with("o3"),
        "openrouter" => true,
        _ => true,
    }
}

pub(crate) fn default_headless_model(provider: &str) -> &'static str {
    match provider {
        "deepseek" => "deepseek-v4-flash",
        _ => default_model(provider),
    }
}

pub(crate) fn final_answer_from_events(events: &[crate::protocol::events::StreamEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            crate::protocol::events::StreamEvent::TextChunk { content, .. } => {
                Some(content.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Resolve provider and model from an optional profile, falling back to the
/// supplied defaults when no profile is selected or the profile has no
/// overrides.
pub(crate) fn resolve_profile_defaults(
    profile_id: Option<&str>,
    default_provider: &str,
    default_model: &str,
) -> (String, String) {
    let Some(pid) = profile_id else {
        return (default_provider.to_string(), default_model.to_string());
    };
    let store = crate::profile::ProfileStore::new(crate::profile::ProfileStore::default_path());
    let Some(profile) = store.get(pid) else {
        crate::app_log!(
            "WARN",
            "Profile '{}' not found, using defaults for provider/model",
            pid
        );
        return (default_provider.to_string(), default_model.to_string());
    };
    let provider = profile
        .default_provider
        .unwrap_or_else(|| default_provider.to_string());
    let model = profile
        .default_model
        .unwrap_or_else(|| default_model.to_string());
    (provider, model)
}
