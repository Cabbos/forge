use crate::adapters::provider_registry::{
    find_loaded_provider_profile, LoadedProviderProfile, ProviderTransport,
};
use crate::agent::provider_capabilities::{default_model, normalize_provider};
use crate::settings;

use super::types::{EvalHeadlessRequest, EvalProviderRoute, EvalProviderTransport};

pub(crate) const DEEPSEEK_ANTHROPIC_BASE_URL: &str = "https://api.deepseek.com/anthropic";
pub(crate) const DEEPSEEK_OPENAI_BASE_URL: &str = "https://api.deepseek.com";
pub(crate) const DEEPSEEK_EVAL_MODEL: &str = "deepseek-v4-flash";

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

/// Validate the caller-provided route against the closed eval-only contract.
///
/// The route is deliberately strict: accepting a new URL here would let a
/// paid campaign silently drift to a different provider or endpoint.  New
/// routes must be added as an explicit design change with new evidence.
pub(crate) fn validate_expected_provider_route(
    route: &EvalProviderRoute,
    provider: &str,
    model: &str,
) -> Result<(), String> {
    if provider.trim() != "deepseek" {
        return Err(format!(
            "eval provider route requires provider deepseek, got {provider}"
        ));
    }
    if model.trim() != DEEPSEEK_EVAL_MODEL {
        return Err(format!(
            "eval provider route requires model {DEEPSEEK_EVAL_MODEL}, got {model}"
        ));
    }

    let expected_base_url = match route.transport {
        EvalProviderTransport::AnthropicMessages => DEEPSEEK_ANTHROPIC_BASE_URL,
        EvalProviderTransport::OpenAiChatCompletions => DEEPSEEK_OPENAI_BASE_URL,
    };
    if route.base_url != expected_base_url {
        return Err(format!(
            "eval provider route does not match official {} endpoint: {}",
            route.transport.as_str(),
            route.base_url
        ));
    }
    Ok(())
}

/// Validate the effective profile loaded from the isolated eval HOME.
///
/// This keeps the typed request and the settings-derived adapter selection in
/// lockstep.  The helper taking an explicit profile slice is kept small and
/// deterministic so the route contract can be tested without touching the
/// user's desktop settings.
pub(crate) fn validate_configured_provider_route(
    route: &EvalProviderRoute,
    provider: &str,
    model: &str,
) -> Result<(), String> {
    let profiles = settings::load_configured_provider_profiles();
    validate_configured_provider_route_against(route, provider, model, &profiles)
}

fn validate_configured_provider_route_against(
    route: &EvalProviderRoute,
    provider: &str,
    model: &str,
    profiles: &[LoadedProviderProfile],
) -> Result<(), String> {
    let profile = find_loaded_provider_profile(profiles, provider).ok_or_else(|| {
        format!("eval provider route could not load configured provider profile {provider}")
    })?;
    if profile.id != "deepseek" {
        return Err(format!(
            "eval provider route resolved unexpected provider profile {}",
            profile.id
        ));
    }

    let expected_transport = match route.transport {
        EvalProviderTransport::AnthropicMessages => ProviderTransport::AnthropicMessages,
        EvalProviderTransport::OpenAiChatCompletions => ProviderTransport::OpenAiChatCompletions,
    };
    if profile.transport != expected_transport {
        return Err(format!(
            "eval provider route transport mismatch: expected {}, configured {}",
            route.transport.as_str(),
            provider_transport_name(profile.transport)
        ));
    }

    let expected_base_url = route.base_url.as_str();
    let configured_base_url = match profile.default_base_url.as_deref() {
        Some(value) => value,
        None if profile.transport == ProviderTransport::AnthropicMessages => {
            DEEPSEEK_ANTHROPIC_BASE_URL
        }
        None => {
            return Err(format!(
                "eval provider route configured profile {} has no base URL",
                profile.id
            ));
        }
    };
    if configured_base_url != expected_base_url {
        return Err(format!(
            "eval provider route base URL mismatch: expected {expected_base_url}, configured {configured_base_url}"
        ));
    }
    if profile.default_model != model {
        return Err(format!(
            "eval provider route model mismatch: expected {model}, configured {}",
            profile.default_model
        ));
    }
    Ok(())
}

fn provider_transport_name(transport: ProviderTransport) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::provider_registry::{
        load_provider_profiles, EnvVarList, ProviderProfileConfig,
    };
    fn route(transport: &str, base_url: &str) -> EvalProviderRoute {
        serde_json::from_value(serde_json::json!({
            "transport": transport,
            "base_url": base_url,
        }))
        .expect("route should deserialize")
    }

    fn configured_deepseek(transport: &str, base_url: &str) -> Vec<LoadedProviderProfile> {
        load_provider_profiles(&[ProviderProfileConfig {
            id: "deepseek".to_string(),
            label: Some("DeepSeek eval".to_string()),
            base_url: Some(base_url.to_string()),
            api_key_env: Some(EnvVarList::One("DEEPSEEK_API_KEY".to_string())),
            base_url_env: None,
            default_model: Some(DEEPSEEK_EVAL_MODEL.to_string()),
            transport: Some(transport.to_string()),
            supports_tools: Some(true),
            supports_streaming: Some(true),
            max_output_tokens_default: None,
            aliases: vec![],
        }])
        .expect("configured profile")
    }

    #[test]
    fn official_eval_routes_deserialize_and_validate() {
        let anthropic = route("anthropic_messages", DEEPSEEK_ANTHROPIC_BASE_URL);
        validate_expected_provider_route(&anthropic, "deepseek", DEEPSEEK_EVAL_MODEL)
            .expect("anthropic route");
        let profiles = configured_deepseek("anthropic_messages", DEEPSEEK_ANTHROPIC_BASE_URL);
        validate_configured_provider_route_against(
            &anthropic,
            "deepseek",
            DEEPSEEK_EVAL_MODEL,
            &profiles,
        )
        .expect("configured anthropic route");

        let openai = route("openai_chat_completions", DEEPSEEK_OPENAI_BASE_URL);
        validate_expected_provider_route(&openai, "deepseek", DEEPSEEK_EVAL_MODEL)
            .expect("openai route");
        let profiles = configured_deepseek("openai_chat_completions", DEEPSEEK_OPENAI_BASE_URL);
        validate_configured_provider_route_against(
            &openai,
            "deepseek",
            DEEPSEEK_EVAL_MODEL,
            &profiles,
        )
        .expect("configured openai route");
    }

    #[test]
    fn invalid_eval_routes_fail_closed() {
        let unsupported_transport =
            serde_json::from_value::<EvalProviderRoute>(serde_json::json!({
                "transport": "openai_responses",
                "base_url": DEEPSEEK_OPENAI_BASE_URL,
            }));
        assert!(unsupported_transport.is_err());

        for base_url in [
            "http://api.deepseek.com",
            "https://evil.example.test",
            "https://api.deepseek.com/v1",
        ] {
            let openai = route("openai_chat_completions", base_url);
            assert!(
                validate_expected_provider_route(&openai, "deepseek", DEEPSEEK_EVAL_MODEL).is_err()
            );
        }

        let openai = route("openai_chat_completions", DEEPSEEK_OPENAI_BASE_URL);
        assert!(
            validate_expected_provider_route(&openai, "anthropic", DEEPSEEK_EVAL_MODEL).is_err()
        );
        assert!(validate_expected_provider_route(&openai, "deepseek", "deepseek-chat").is_err());

        let profiles = configured_deepseek("anthropic_messages", DEEPSEEK_ANTHROPIC_BASE_URL);
        assert!(validate_configured_provider_route_against(
            &openai,
            "deepseek",
            DEEPSEEK_EVAL_MODEL,
            &profiles,
        )
        .is_err());

        let missing_base_url = load_provider_profiles(&[ProviderProfileConfig {
            id: "deepseek".to_string(),
            label: Some("DeepSeek eval".to_string()),
            base_url: None,
            api_key_env: Some(EnvVarList::One("DEEPSEEK_API_KEY".to_string())),
            base_url_env: None,
            default_model: Some(DEEPSEEK_EVAL_MODEL.to_string()),
            transport: Some("openai_chat_completions".to_string()),
            supports_tools: Some(true),
            supports_streaming: Some(true),
            max_output_tokens_default: None,
            aliases: vec![],
        }])
        .expect("configured profile without base URL");
        assert!(validate_configured_provider_route_against(
            &openai,
            "deepseek",
            DEEPSEEK_EVAL_MODEL,
            &missing_base_url,
        )
        .is_err());
    }

    #[test]
    fn ordinary_desktop_deepseek_default_remains_anthropic() {
        let profiles = load_provider_profiles(&[]).expect("built-in profiles");
        let profile = find_loaded_provider_profile(&profiles, "deepseek").expect("deepseek");
        assert_eq!(profile.transport, ProviderTransport::AnthropicMessages);
        assert_eq!(
            profile.default_base_url.as_deref(),
            Some(DEEPSEEK_ANTHROPIC_BASE_URL)
        );
    }

    #[test]
    fn configured_provider_route_reads_the_isolated_settings_profile() {
        let _guard = crate::eval_headless::tests::ENV_LOCK.blocking_lock();
        let previous_home = std::env::var("HOME").ok();
        let home = tempfile::tempdir().expect("home");
        let forge_dir = home.path().join(".forge");
        std::fs::create_dir_all(&forge_dir).expect("forge config directory");
        std::fs::write(
            forge_dir.join("config.json"),
            serde_json::json!({
                "providers": [{
                    "id": "deepseek",
                    "base_url": DEEPSEEK_OPENAI_BASE_URL,
                    "api_key_env": "DEEPSEEK_API_KEY",
                    "default_model": DEEPSEEK_EVAL_MODEL,
                    "transport": "openai_chat_completions"
                }]
            })
            .to_string(),
        )
        .expect("write isolated settings");
        std::env::set_var("HOME", home.path());

        let openai = route("openai_chat_completions", DEEPSEEK_OPENAI_BASE_URL);
        validate_configured_provider_route(&openai, "deepseek", DEEPSEEK_EVAL_MODEL)
            .expect("isolated candidate profile");

        let wrong_route = route("anthropic_messages", DEEPSEEK_ANTHROPIC_BASE_URL);
        assert!(
            validate_configured_provider_route(&wrong_route, "deepseek", DEEPSEEK_EVAL_MODEL)
                .is_err(),
            "the typed route must not accept a mismatched isolated profile"
        );

        match previous_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
    }
}
