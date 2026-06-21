use serde::Serialize;
use std::collections::BTreeSet;

use crate::adapters::provider_registry::{
    find_loaded_provider_profile, LoadedProviderProfile, ModelCatalogPolicy, ProviderTransport,
};
use crate::settings::{Credentials, ProviderCatalogModel, ProviderModelCatalogSource, Settings};

const MODEL_CATALOG_TIMEOUT_SECS: u64 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderModelCatalogStatus {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProviderModelCatalogItem {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProviderModelCatalogResult {
    pub(crate) provider: String,
    pub(crate) provider_label: String,
    pub(crate) base_url: Option<String>,
    pub(crate) source: ProviderModelCatalogSource,
    pub(crate) status: ProviderModelCatalogStatus,
    pub(crate) recorded_at_ms: Option<u64>,
    pub(crate) models: Vec<ProviderModelCatalogItem>,
    pub(crate) message: String,
    pub(crate) remediation: Option<String>,
}

pub(crate) async fn list_provider_models(provider: &str) -> ProviderModelCatalogResult {
    let credentials = crate::settings::detect_credentials(provider);
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(MODEL_CATALOG_TIMEOUT_SECS))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let mut result = list_provider_models_with_credentials_and_profiles(
        provider,
        credentials,
        client,
        &crate::settings::load_configured_provider_profiles(),
    )
    .await;
    if result.status == ProviderModelCatalogStatus::Available {
        let models = result
            .models
            .iter()
            .map(|model| ProviderCatalogModel {
                id: model.id.clone(),
                name: model.name.clone(),
                context_window_tokens: None,
            })
            .collect();
        let mut settings = Settings::load();
        if let Err(error) = settings.record_provider_model_catalog(
            &result.provider,
            result.base_url.clone(),
            result.source,
            models,
        ) {
            result.remediation = Some(format!(
                "Models were fetched, but Forge could not save the catalog cache: {error}"
            ));
        }
    }
    result
}

pub(crate) async fn list_provider_models_with_credentials_and_profiles(
    provider: &str,
    credentials: Credentials,
    client: reqwest::Client,
    profiles: &[LoadedProviderProfile],
) -> ProviderModelCatalogResult {
    let Some(profile) = find_loaded_provider_profile(profiles, provider) else {
        return unavailable(
            provider,
            provider,
            None,
            ProviderModelCatalogSource::Unsupported,
            Vec::new(),
            &format!("Unsupported provider: {provider}."),
            Some("Choose a configured provider before refreshing models.".to_string()),
        );
    };
    let provider_id = profile.id.clone();
    let provider_label = profile.label.clone();
    if profile.model_catalog == ModelCatalogPolicy::StaticFallback {
        return static_fallback_catalog(profile);
    }

    let base_url = credentials
        .api_base
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| profile.default_base_url.clone());
    let base_url_label = base_url.as_deref().map(safe_base_url_label);
    let key_required = !profile.api_key_env.is_empty();
    let key_present = !credentials.api_key.trim().is_empty() || !key_required;

    if !key_present {
        return unavailable(
            &provider_id,
            &provider_label,
            base_url_label,
            ProviderModelCatalogSource::LiveEndpoint,
            Vec::new(),
            &format!("{provider_label} API key is missing."),
            Some(format!(
                "Add a {provider_label} API key, then refresh models again."
            )),
        );
    }

    let Some(base_url) = base_url else {
        return unavailable(
            &provider_id,
            &provider_label,
            None,
            ProviderModelCatalogSource::LiveEndpoint,
            Vec::new(),
            &format!("{provider_label} base URL is missing."),
            Some(format!(
                "Configure a base URL for {provider_label}, then refresh models again."
            )),
        );
    };

    let request = match model_catalog_request(&client, profile, &base_url, &credentials.api_key) {
        Ok(request) => request,
        Err(message) => {
            return unavailable(
                &provider_id,
                &provider_label,
                Some(safe_base_url_label(&base_url)),
                ProviderModelCatalogSource::LiveEndpoint,
                Vec::new(),
                &message,
                Some(
                    "This transport does not expose a compatible model-list endpoint yet."
                        .to_string(),
                ),
            );
        }
    };

    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            return unavailable(
                &provider_id,
                &provider_label,
                Some(safe_base_url_label(&base_url)),
                ProviderModelCatalogSource::LiveEndpoint,
                Vec::new(),
                &format!("{} model catalog unreachable.", provider_label),
                Some(sanitize_text(&error.to_string(), &credentials.api_key)),
            );
        }
    };
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return unavailable(
            &provider_id,
            &provider_label,
            Some(safe_base_url_label(&base_url)),
            ProviderModelCatalogSource::LiveEndpoint,
            Vec::new(),
            &format!(
                "{} model catalog returned HTTP {}.",
                provider_label,
                status.as_u16()
            ),
            Some(sanitize_text(
                &extract_provider_error_message(&body),
                &credentials.api_key,
            )),
        );
    }

    let models = parse_openai_compatible_models(&body);
    if models.is_empty() {
        return unavailable(
            &provider_id,
            &provider_label,
            Some(safe_base_url_label(&base_url)),
            ProviderModelCatalogSource::LiveEndpoint,
            Vec::new(),
            &format!("{provider_label} returned no model IDs."),
            Some(
                "Check that the endpoint implements an OpenAI-compatible /models response."
                    .to_string(),
            ),
        );
    }

    ProviderModelCatalogResult {
        provider: provider_id,
        provider_label: provider_label.clone(),
        base_url: Some(safe_base_url_label(&base_url)),
        source: ProviderModelCatalogSource::LiveEndpoint,
        status: ProviderModelCatalogStatus::Available,
        recorded_at_ms: Some(current_epoch_millis()),
        models,
        message: format!(
            "{} returned {} models.",
            provider_label,
            parse_model_count(&body)
        ),
        remediation: None,
    }
}

fn static_fallback_catalog(profile: &LoadedProviderProfile) -> ProviderModelCatalogResult {
    let models = fallback_model_items(&profile.model_fallbacks);
    if models.is_empty() {
        return unavailable(
            &profile.id,
            &profile.label,
            profile.default_base_url.as_deref().map(safe_base_url_label),
            ProviderModelCatalogSource::StaticFallback,
            Vec::new(),
            &format!(
                "{} has no model catalog fallback configured.",
                profile.label
            ),
            Some(
                "Configure a default model or use a provider with a live model catalog endpoint."
                    .to_string(),
            ),
        );
    }

    ProviderModelCatalogResult {
        provider: profile.id.clone(),
        provider_label: profile.label.clone(),
        base_url: profile.default_base_url.as_deref().map(safe_base_url_label),
        source: ProviderModelCatalogSource::StaticFallback,
        status: ProviderModelCatalogStatus::Available,
        recorded_at_ms: Some(current_epoch_millis()),
        models,
        message: format!("{} uses Forge's static model catalog.", profile.label),
        remediation: None,
    }
}

fn fallback_model_items(models: &[String]) -> Vec<ProviderModelCatalogItem> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for model in models {
        let id = model.trim();
        if id.is_empty() || !seen.insert(id.to_string()) {
            continue;
        }
        result.push(ProviderModelCatalogItem {
            id: id.to_string(),
            name: id.to_string(),
        });
    }
    result
}

fn model_catalog_request(
    client: &reqwest::Client,
    profile: &LoadedProviderProfile,
    base_url: &str,
    api_key: &str,
) -> Result<reqwest::RequestBuilder, String> {
    let base_url = base_url.trim().trim_end_matches('/');
    if base_url.is_empty() {
        return Err("Base URL is empty.".to_string());
    }
    match profile.transport {
        ProviderTransport::OpenAiChatCompletions | ProviderTransport::CustomOpenAiCompatible => Ok(
            with_bearer_auth_header(client.get(format!("{base_url}/models")), api_key),
        ),
        ProviderTransport::AnthropicMessages
        | ProviderTransport::CustomAnthropicCompatible
        | ProviderTransport::OpenAiResponses
        | ProviderTransport::NativeGemini
        | ProviderTransport::BedrockConverse => Err(format!(
            "{} model catalog refresh is not supported for this transport yet.",
            profile.label
        )),
    }
}

fn parse_openai_compatible_models(body: &str) -> Vec<ProviderModelCatalogItem> {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    let mut ids = BTreeSet::new();
    for entry in parsed
        .get("data")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
    {
        let Some(id) = entry.get("id").and_then(|value| value.as_str()) else {
            continue;
        };
        let id = id.trim();
        if !id.is_empty() {
            ids.insert(id.to_string());
        }
    }
    ids.into_iter()
        .map(|id| ProviderModelCatalogItem {
            name: id.clone(),
            id,
        })
        .collect()
}

fn parse_model_count(body: &str) -> usize {
    parse_openai_compatible_models(body).len()
}

fn with_bearer_auth_header(
    request: reqwest::RequestBuilder,
    api_key: &str,
) -> reqwest::RequestBuilder {
    if api_key.trim().is_empty() {
        request
    } else {
        request.header("authorization", format!("Bearer {api_key}"))
    }
}

fn unavailable(
    provider: &str,
    provider_label: &str,
    base_url: Option<String>,
    source: ProviderModelCatalogSource,
    models: Vec<ProviderModelCatalogItem>,
    message: &str,
    remediation: Option<String>,
) -> ProviderModelCatalogResult {
    ProviderModelCatalogResult {
        provider: provider.to_string(),
        provider_label: provider_label.to_string(),
        base_url,
        source,
        status: ProviderModelCatalogStatus::Unavailable,
        recorded_at_ms: None,
        models,
        message: message.to_string(),
        remediation,
    }
}

fn current_epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn safe_base_url_label(base_url: &str) -> String {
    let trimmed = base_url.trim();
    let Ok(mut url) = reqwest::Url::parse(trimmed) else {
        return sanitize_url_like_text(trimmed);
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    let path = url
        .path_segments()
        .map(|segments| {
            segments
                .map(|segment| {
                    if looks_secret_like(segment) {
                        "[redacted]"
                    } else {
                        segment
                    }
                })
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default();
    url.set_path(&path);
    url.to_string().trim_end_matches('/').to_string()
}

fn sanitize_url_like_text(value: &str) -> String {
    value
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .split('/')
        .map(|segment| {
            if looks_secret_like(segment) {
                "[redacted]"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn sanitize_text(value: &str, api_key: &str) -> String {
    let mut sanitized = value.to_string();
    if !api_key.trim().is_empty() {
        sanitized = sanitized.replace(api_key, "[redacted]");
    }
    sanitized
}

fn looks_secret_like(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value.len() >= 16
        && (lower.starts_with("sk-")
            || lower.starts_with("nvapi-")
            || lower.starts_with("xai-")
            || lower.starts_with("gsk_")
            || lower.contains("token")
            || lower.contains("secret"))
}

fn extract_provider_error_message(body: &str) -> String {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.trim().to_string();
    };
    parsed
        .pointer("/error/message")
        .or_else(|| parsed.pointer("/message"))
        .and_then(|value| value.as_str())
        .unwrap_or_else(|| body.trim())
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::provider_registry::{
        load_provider_profiles, EnvVarList, ProviderProfileConfig,
    };
    use crate::settings::Credentials;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::sync::mpsc;
    use std::time::Duration;

    #[tokio::test]
    async fn model_catalog_fetches_openai_compatible_models() {
        let (base_url, request_rx) = spawn_json_response_server(&json!({
            "object": "list",
            "data": [
                { "id": "zeta-model", "object": "model" },
                { "id": "alpha-model", "object": "model" },
                { "object": "model" },
                { "id": "" }
            ]
        }));

        let profiles = load_provider_profiles(&[]).expect("built-in profiles load");
        let result = list_provider_models_with_credentials_and_profiles(
            "openai",
            Credentials {
                api_key: "test-key".to_string(),
                api_base: Some(base_url),
                model: None,
            },
            reqwest::Client::new(),
            &profiles,
        )
        .await;

        assert_eq!(result.status, ProviderModelCatalogStatus::Available);
        assert_eq!(result.source, ProviderModelCatalogSource::LiveEndpoint);
        assert!(
            result.recorded_at_ms.is_some_and(|value| value > 0),
            "live model catalog evidence must include when it was recorded"
        );
        assert_eq!(result.provider, "openai");
        assert_eq!(
            result
                .models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            ["alpha-model", "zeta-model",]
        );
        assert_eq!(result.message, "OpenAI returned 2 models.");

        let request = request_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("captured models request");
        assert_eq!(request["path"], "/models");
        assert_eq!(request["authorization"], "Bearer test-key");
    }

    #[tokio::test]
    async fn model_catalog_allows_no_auth_local_openai_profiles() {
        let (base_url, request_rx) = spawn_json_response_server(&json!({
            "data": [{ "id": "local-model" }]
        }));
        let profiles = load_provider_profiles(&[ProviderProfileConfig {
            id: "local-openai".to_string(),
            label: Some("Local OpenAI-Compatible".to_string()),
            base_url: Some(base_url.clone()),
            api_key_env: Some(EnvVarList::Many(vec![])),
            base_url_env: None,
            default_model: Some("local-model".to_string()),
            transport: Some("openai_chat_completions".to_string()),
            supports_tools: Some(true),
            supports_streaming: Some(true),
            max_output_tokens_default: None,
            aliases: vec!["local-lab".to_string()],
        }])
        .expect("profiles load");

        let result = list_provider_models_with_credentials_and_profiles(
            "local-lab",
            Credentials {
                api_key: String::new(),
                api_base: None,
                model: None,
            },
            reqwest::Client::new(),
            &profiles,
        )
        .await;

        assert_eq!(result.status, ProviderModelCatalogStatus::Available);
        assert_eq!(result.source, ProviderModelCatalogSource::LiveEndpoint);
        assert_eq!(result.provider, "local-openai");
        assert_eq!(result.models[0].id, "local-model");

        let request = request_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("captured local models request");
        assert_eq!(request["path"], "/models");
        assert_eq!(request["authorization"], "");
    }

    #[tokio::test]
    async fn model_catalog_uses_static_fallbacks_without_network_or_key() {
        let profiles = load_provider_profiles(&[]).expect("built-in profiles load");
        let result = list_provider_models_with_credentials_and_profiles(
            "deepseek",
            Credentials {
                api_key: String::new(),
                api_base: None,
                model: None,
            },
            reqwest::Client::new(),
            &profiles,
        )
        .await;

        assert_eq!(result.status, ProviderModelCatalogStatus::Available);
        assert_eq!(result.source, ProviderModelCatalogSource::StaticFallback);
        assert!(
            result.recorded_at_ms.is_some_and(|value| value > 0),
            "static fallback catalog evidence must include when it was recorded"
        );
        assert_eq!(result.provider, "deepseek");
        assert_eq!(result.provider_label, "DeepSeek");
        assert_eq!(
            result
                .models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            ["deepseek-v4-flash[1m]", "deepseek-v4-pro", "deepseek-chat"]
        );
        assert_eq!(
            result.message,
            "DeepSeek uses Forge's static model catalog."
        );
        assert_eq!(result.remediation, None);
    }

    #[tokio::test]
    async fn model_catalog_static_fallbacks_respect_provider_aliases() {
        let profiles = load_provider_profiles(&[]).expect("built-in profiles load");
        let result = list_provider_models_with_credentials_and_profiles(
            "moonshot",
            Credentials {
                api_key: String::new(),
                api_base: None,
                model: None,
            },
            reqwest::Client::new(),
            &profiles,
        )
        .await;

        assert_eq!(result.status, ProviderModelCatalogStatus::Available);
        assert_eq!(result.source, ProviderModelCatalogSource::StaticFallback);
        assert_eq!(result.provider, "kimi");
        assert_eq!(result.provider_label, "Kimi / Moonshot");
        assert_eq!(
            result
                .models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            ["kimi-k2.7-code", "kimi-k2.5", "kimi-k2"]
        );
    }

    fn spawn_json_response_server(
        response_body: &serde_json::Value,
    ) -> (String, mpsc::Receiver<serde_json::Value>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
        let (tx, rx) = mpsc::channel();
        let response_body = response_body.to_string();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let request = read_http_request(&mut stream);
            tx.send(request).expect("send request");
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            )
            .expect("write response");
        });

        (base_url, rx)
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> serde_json::Value {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 4096];
        loop {
            let read = stream.read(&mut chunk).expect("read request");
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&buffer);
        let mut lines = request.lines();
        let first_line = lines.next().unwrap_or_default();
        let path = first_line.split_whitespace().nth(1).unwrap_or_default();
        let authorization = request
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("authorization")
                    .then(|| value.trim().to_string())
            })
            .unwrap_or_default();
        json!({
            "path": path,
            "authorization": authorization,
        })
    }
}
