use serde::Serialize;
use serde_json::json;

use crate::adapters::provider_registry::{
    find_loaded_provider_profile, LoadedProviderProfile, ProviderTransport,
};
use crate::settings::Credentials;
use crate::settings::{
    CachedProviderProbeCheck, CachedProviderProbeEvidence, ProviderProbeEvidenceCheckStatus,
    ProviderProbeEvidenceSource, ProviderProbeEvidenceStatus, Settings,
};

const PROBE_TIMEOUT_SECS: u64 = 20;
const PROBE_TOOL_NAME: &str = "forge_probe";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderProbeStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderProbeCheckStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProviderProbeCheck {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) status: ProviderProbeCheckStatus,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProviderProbeResult {
    pub(crate) provider: String,
    pub(crate) provider_label: String,
    pub(crate) model: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) status: ProviderProbeStatus,
    pub(crate) checks: Vec<ProviderProbeCheck>,
    pub(crate) message: String,
    pub(crate) remediation: Option<String>,
}

pub(crate) async fn probe_provider_with_credentials(
    provider: &str,
    credentials: Credentials,
    client: reqwest::Client,
) -> ProviderProbeResult {
    probe_provider_with_credentials_and_profiles(
        provider,
        credentials,
        client,
        &crate::settings::load_configured_provider_profiles(),
    )
    .await
}

async fn probe_provider_with_credentials_and_profiles(
    provider: &str,
    credentials: Credentials,
    client: reqwest::Client,
    profiles: &[LoadedProviderProfile],
) -> ProviderProbeResult {
    let Some(profile) = find_loaded_provider_profile(profiles, provider) else {
        return unsupported_provider_result(provider, profiles);
    };
    let provider_id = profile.id.clone();
    let provider_label = profile.label.clone();
    let model = credentials
        .model
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| profile.default_model.clone());
    let base_url = credentials
        .api_base
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| profile.default_base_url.clone());
    let redaction_values = redaction_values(&credentials.api_key, base_url.as_deref());
    let base_url_label = base_url.as_deref().map(safe_base_url_label);
    let key_required = !profile.api_key_env.is_empty();
    let key_present = !credentials.api_key.trim().is_empty() || !key_required;

    if !key_present {
        return ProviderProbeResult {
            provider: provider_id,
            provider_label: provider_label.clone(),
            model: Some(model),
            base_url: base_url_label,
            status: ProviderProbeStatus::Failed,
            checks: checks([
                check_failed("key_present", "Key present", "API key is missing."),
                check_failed(
                    "base_url_reachable",
                    "Base URL reachable",
                    "Not run because the API key is missing.",
                ),
                check_failed(
                    "model_accepted",
                    "Model accepted",
                    "Not run because the API key is missing.",
                ),
                check_failed(
                    "streaming_accepted",
                    "Streaming accepted",
                    "Not run because the API key is missing.",
                ),
                check_failed(
                    "tool_schema_accepted",
                    "Tool schema accepted",
                    "Not run because the API key is missing.",
                ),
            ]),
            message: format!("{provider_label} API key is missing."),
            remediation: Some(format!(
                "Add a {provider_label} API key, then run the probe again."
            )),
        };
    }

    let Some(base_url) = base_url else {
        return ProviderProbeResult {
            provider: provider_id,
            provider_label: provider_label.clone(),
            model: Some(model),
            base_url: None,
            status: ProviderProbeStatus::Failed,
            checks: checks([
                check_passed("key_present", "Key present", "API key is present."),
                check_failed(
                    "base_url_reachable",
                    "Base URL reachable",
                    "No base URL is configured for this provider.",
                ),
                check_failed(
                    "model_accepted",
                    "Model accepted",
                    "Not run because the base URL is missing.",
                ),
                check_failed(
                    "streaming_accepted",
                    "Streaming accepted",
                    "Not run because the base URL is missing.",
                ),
                check_failed(
                    "tool_schema_accepted",
                    "Tool schema accepted",
                    "Not run because the base URL is missing.",
                ),
            ]),
            message: format!("{provider_label} base URL is missing."),
            remediation: Some(format!(
                "Configure a base URL for {provider_label}, then run the probe again."
            )),
        };
    };

    let request = match probe_request(&client, profile, &base_url, &credentials.api_key, &model) {
        Ok(request) => request,
        Err(message) => {
            let message = sanitize_text(&message, &redaction_values);
            return ProviderProbeResult {
                provider: provider_id,
                provider_label: provider_label.clone(),
                model: Some(model),
                base_url: Some(safe_base_url_label(&base_url)),
                status: ProviderProbeStatus::Failed,
                checks: checks([
                    check_passed("key_present", "Key present", "API key is present."),
                    check_failed("base_url_reachable", "Base URL reachable", &message),
                    check_failed("model_accepted", "Model accepted", "Not run."),
                    check_failed("streaming_accepted", "Streaming accepted", "Not run."),
                    check_failed("tool_schema_accepted", "Tool schema accepted", "Not run."),
                ]),
                message: format!("{provider_label} probe could not start."),
                remediation: Some(message),
            };
        }
    };

    let response = request.send().await;
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            let message = network_error_message(&error, &redaction_values);
            return ProviderProbeResult {
                provider: provider_id,
                provider_label: provider_label.clone(),
                model: Some(model),
                base_url: Some(safe_base_url_label(&base_url)),
                status: ProviderProbeStatus::Failed,
                checks: checks([
                    check_passed("key_present", "Key present", "API key is present."),
                    check_failed("base_url_reachable", "Base URL reachable", &message),
                    check_failed(
                        "model_accepted",
                        "Model accepted",
                        "Not run because the base URL was unreachable.",
                    ),
                    check_failed(
                        "streaming_accepted",
                        "Streaming accepted",
                        "Not run because the base URL was unreachable.",
                    ),
                    check_failed(
                        "tool_schema_accepted",
                        "Tool schema accepted",
                        "Not run because the base URL was unreachable.",
                    ),
                ]),
                message: format!("{provider_label} base URL unreachable."),
                remediation: Some(format!(
                    "Check the {provider_label} base URL and network connection."
                )),
            };
        }
    };

    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response.text().await.unwrap_or_default();
    if status.is_success() {
        if let Err(message) =
            validate_streaming_response(profile.transport, content_type.as_deref(), &body)
        {
            return successful_http_without_streaming_result(
                profile,
                Some(model),
                &base_url,
                &sanitize_text(&message, &redaction_values),
            );
        }

        return ProviderProbeResult {
            provider: provider_id,
            provider_label: provider_label.clone(),
            model: Some(model),
            base_url: Some(safe_base_url_label(&base_url)),
            status: ProviderProbeStatus::Passed,
            checks: checks([
                check_passed("key_present", "Key present", "API key is present."),
                check_passed(
                    "base_url_reachable",
                    "Base URL reachable",
                    "Provider endpoint accepted the probe request.",
                ),
                check_passed("model_accepted", "Model accepted", "Model was accepted."),
                check_passed(
                    "streaming_accepted",
                    "Streaming accepted",
                    "Streaming request was accepted.",
                ),
                check_passed(
                    "tool_schema_accepted",
                    "Tool schema accepted",
                    "No-op tool schema was accepted.",
                ),
            ]),
            message: format!("{provider_label} probe passed."),
            remediation: None,
        };
    }

    failed_http_result(
        profile,
        Some(model),
        &base_url,
        status.as_u16(),
        &extract_provider_error_message(&body),
        &redaction_values,
    )
}

pub(crate) async fn probe_provider(
    provider: &str,
    resolver: &crate::settings::CredentialResolver,
    profile: Option<&crate::profile::ForgeProfile>,
) -> Result<ProviderProbeResult, crate::settings::CredentialResolutionError> {
    let credentials = resolver.resolve(provider, profile)?;
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(PROBE_TIMEOUT_SECS))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let mut result = probe_provider_with_credentials(provider, credentials, client).await;
    let mut settings = Settings::load();
    if let Err(error) =
        settings.record_provider_probe_evidence(&result.provider, cached_probe_evidence(&result))
    {
        result.remediation = Some(format!(
            "Probe ran, but Forge could not save the probe evidence: {error}"
        ));
    }
    Ok(result)
}

fn cached_probe_evidence(result: &ProviderProbeResult) -> CachedProviderProbeEvidence {
    CachedProviderProbeEvidence {
        source: ProviderProbeEvidenceSource::ManualProbe,
        status: match result.status {
            ProviderProbeStatus::Passed => ProviderProbeEvidenceStatus::Passed,
            ProviderProbeStatus::Failed => ProviderProbeEvidenceStatus::Failed,
        },
        recorded_at_ms: Some(current_epoch_millis()),
        model: result.model.clone(),
        base_url: result.base_url.clone(),
        checks: result
            .checks
            .iter()
            .map(|check| CachedProviderProbeCheck {
                id: check.id.clone(),
                label: check.label.clone(),
                status: match check.status {
                    ProviderProbeCheckStatus::Passed => ProviderProbeEvidenceCheckStatus::Passed,
                    ProviderProbeCheckStatus::Failed => ProviderProbeEvidenceCheckStatus::Failed,
                },
            })
            .collect(),
    }
}

fn current_epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn probe_request(
    client: &reqwest::Client,
    profile: &LoadedProviderProfile,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Result<reqwest::RequestBuilder, String> {
    let base_url = base_url.trim().trim_end_matches('/');
    if base_url.is_empty() {
        return Err("Base URL is empty.".to_string());
    }

    match profile.transport {
        ProviderTransport::AnthropicMessages | ProviderTransport::CustomAnthropicCompatible => {
            let request =
                with_anthropic_auth_header(client.post(format!("{base_url}/v1/messages")), api_key)
                    .header("anthropic-version", "2023-06-01")
                    .header("content-type", "application/json")
                    .json(&json!({
                        "model": model,
                        "max_tokens": 16,
                        "stream": true,
                        "messages": [
                            {
                                "role": "user",
                                "content": "Reply with ok."
                            }
                        ],
                        "tools": [
                            {
                                "name": PROBE_TOOL_NAME,
                                "description": "No-op compatibility probe.",
                                "input_schema": noop_tool_schema()
                            }
                        ]
                    }));
            Ok(request)
        }
        ProviderTransport::OpenAiChatCompletions | ProviderTransport::CustomOpenAiCompatible => {
            let request = with_bearer_auth_header(
                client.post(format!("{base_url}/chat/completions")),
                api_key,
            )
            .header("content-type", "application/json")
            .json(&json!({
                "model": model,
                "messages": [
                    {
                        "role": "user",
                        "content": "Reply with ok."
                    }
                ],
                "stream": true,
                "max_tokens": 16,
                "tools": [
                    {
                        "type": "function",
                        "function": {
                            "name": PROBE_TOOL_NAME,
                            "description": "No-op compatibility probe.",
                            "parameters": noop_tool_schema()
                        }
                    }
                ],
                "tool_choice": "none"
            }));
            Ok(request)
        }
        ProviderTransport::OpenAiResponses
        | ProviderTransport::NativeGemini
        | ProviderTransport::BedrockConverse => Err(format!(
            "{} uses a transport Forge cannot probe yet.",
            profile.label
        )),
    }
}

fn with_anthropic_auth_header(
    request: reqwest::RequestBuilder,
    api_key: &str,
) -> reqwest::RequestBuilder {
    if api_key.trim().is_empty() {
        request
    } else {
        request.header("x-api-key", api_key)
    }
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

fn failed_http_result(
    profile: &LoadedProviderProfile,
    model: Option<String>,
    base_url: &str,
    status: u16,
    provider_message: &str,
    redaction_values: &[String],
) -> ProviderProbeResult {
    let message = if provider_message.trim().is_empty() {
        format!("HTTP {status}")
    } else {
        provider_message.trim().to_string()
    };
    let message = sanitize_text(&message, redaction_values);
    let base_checks = [
        check_passed("key_present", "Key present", "API key is present."),
        check_passed(
            "base_url_reachable",
            "Base URL reachable",
            "Provider endpoint returned an HTTP response.",
        ),
    ];

    if looks_like_tool_schema_error(&message) {
        return ProviderProbeResult {
            provider: profile.id.clone(),
            provider_label: profile.label.clone(),
            model,
            base_url: Some(safe_base_url_label(base_url)),
            status: ProviderProbeStatus::Failed,
            checks: checks([
                base_checks[0].clone(),
                base_checks[1].clone(),
                check_passed(
                    "model_accepted",
                    "Model accepted",
                    "Model was accepted before tool validation.",
                ),
                check_passed(
                    "streaming_accepted",
                    "Streaming accepted",
                    "Streaming request was accepted before tool validation.",
                ),
                check_failed(
                    "tool_schema_accepted",
                    "Tool schema accepted",
                    &format!("Provider rejected the no-op tool schema: {message}"),
                ),
            ]),
            message: format!("{} tool schema unsupported.", profile.label),
            remediation: Some(format!(
                "Use a {} model or endpoint that accepts tool/function schemas.",
                profile.label
            )),
        };
    }

    if looks_like_streaming_error(&message) {
        return ProviderProbeResult {
            provider: profile.id.clone(),
            provider_label: profile.label.clone(),
            model,
            base_url: Some(safe_base_url_label(base_url)),
            status: ProviderProbeStatus::Failed,
            checks: checks([
                base_checks[0].clone(),
                base_checks[1].clone(),
                check_passed("model_accepted", "Model accepted", "Model was accepted."),
                check_failed(
                    "streaming_accepted",
                    "Streaming accepted",
                    &format!("Provider rejected streaming: {message}"),
                ),
                check_failed(
                    "tool_schema_accepted",
                    "Tool schema accepted",
                    "Not run because streaming was rejected.",
                ),
            ]),
            message: format!("{} streaming unsupported.", profile.label),
            remediation: Some(format!(
                "Use a {} endpoint that accepts streaming chat requests.",
                profile.label
            )),
        };
    }

    if looks_like_model_error(&message) {
        return ProviderProbeResult {
            provider: profile.id.clone(),
            provider_label: profile.label.clone(),
            model,
            base_url: Some(safe_base_url_label(base_url)),
            status: ProviderProbeStatus::Failed,
            checks: checks([
                base_checks[0].clone(),
                base_checks[1].clone(),
                check_failed(
                    "model_accepted",
                    "Model accepted",
                    &format!("Provider rejected the configured model: {message}"),
                ),
                check_failed(
                    "streaming_accepted",
                    "Streaming accepted",
                    "Not run because the model was rejected.",
                ),
                check_failed(
                    "tool_schema_accepted",
                    "Tool schema accepted",
                    "Not run because the model was rejected.",
                ),
            ]),
            message: format!("{} model rejected.", profile.label),
            remediation: Some(format!(
                "Select a model available to your {} account or endpoint.",
                profile.label
            )),
        };
    }

    let auth_message = if looks_like_auth_error(status, &message) {
        format!("{} rejected the configured API key.", profile.label)
    } else {
        format!("{} probe failed.", profile.label)
    };
    ProviderProbeResult {
        provider: profile.id.clone(),
        provider_label: profile.label.clone(),
        model,
        base_url: Some(safe_base_url_label(base_url)),
        status: ProviderProbeStatus::Failed,
        checks: checks([
            base_checks[0].clone(),
            base_checks[1].clone(),
            check_failed(
                "model_accepted",
                "Model accepted",
                &format!("Probe stopped with HTTP {status}: {message}"),
            ),
            check_failed(
                "streaming_accepted",
                "Streaming accepted",
                "Not confirmed because the provider rejected the request.",
            ),
            check_failed(
                "tool_schema_accepted",
                "Tool schema accepted",
                "Not confirmed because the provider rejected the request.",
            ),
        ]),
        message: auth_message,
        remediation: Some(message),
    }
}

fn successful_http_without_streaming_result(
    profile: &LoadedProviderProfile,
    model: Option<String>,
    base_url: &str,
    reason: &str,
) -> ProviderProbeResult {
    ProviderProbeResult {
        provider: profile.id.clone(),
        provider_label: profile.label.clone(),
        model,
        base_url: Some(safe_base_url_label(base_url)),
        status: ProviderProbeStatus::Failed,
        checks: checks([
            check_passed("key_present", "Key present", "API key is present."),
            check_passed(
                "base_url_reachable",
                "Base URL reachable",
                "Provider endpoint returned HTTP success.",
            ),
            check_passed(
                "model_accepted",
                "Model accepted",
                "Provider accepted the configured model in the probe request.",
            ),
            check_failed("streaming_accepted", "Streaming accepted", reason),
            check_failed(
                "tool_schema_accepted",
                "Tool schema accepted",
                "Not confirmed because the streaming response shape was not confirmed.",
            ),
        ]),
        message: format!("{} streaming response was not confirmed.", profile.label),
        remediation: Some(format!(
            "Check that the {} endpoint supports streaming responses for this model.",
            profile.label
        )),
    }
}

fn unsupported_provider_result(
    provider: &str,
    profiles: &[LoadedProviderProfile],
) -> ProviderProbeResult {
    ProviderProbeResult {
        provider: provider.to_string(),
        provider_label: provider.to_string(),
        model: None,
        base_url: None,
        status: ProviderProbeStatus::Failed,
        checks: checks([
            check_failed(
                "key_present",
                "Key present",
                "Provider is not known to Forge.",
            ),
            check_failed("base_url_reachable", "Base URL reachable", "Not run."),
            check_failed("model_accepted", "Model accepted", "Not run."),
            check_failed("streaming_accepted", "Streaming accepted", "Not run."),
            check_failed("tool_schema_accepted", "Tool schema accepted", "Not run."),
        ]),
        message: format!("Unsupported provider: {provider}."),
        remediation: Some(format!(
            "Choose one of: {}.",
            profiles
                .iter()
                .map(|profile| profile.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn check_passed(id: &str, label: &str, message: &str) -> ProviderProbeCheck {
    ProviderProbeCheck {
        id: id.to_string(),
        label: label.to_string(),
        status: ProviderProbeCheckStatus::Passed,
        message: message.to_string(),
    }
}

fn check_failed(id: &str, label: &str, message: &str) -> ProviderProbeCheck {
    ProviderProbeCheck {
        id: id.to_string(),
        label: label.to_string(),
        status: ProviderProbeCheckStatus::Failed,
        message: message.to_string(),
    }
}

fn checks<const N: usize>(checks: [ProviderProbeCheck; N]) -> Vec<ProviderProbeCheck> {
    checks.into_iter().collect()
}

fn noop_tool_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {},
        "required": [],
        "additionalProperties": false
    })
}

fn extract_provider_error_message(body: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return compact_message(body);
    };
    value
        .pointer("/error/message")
        .or_else(|| value.pointer("/error"))
        .or_else(|| value.pointer("/message"))
        .and_then(|value| value.as_str())
        .map(compact_message)
        .unwrap_or_else(|| compact_message(body))
}

fn compact_message(message: &str) -> String {
    let trimmed = message.trim();
    let compact = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    let compact = sanitize_text(&compact, &[]);
    if compact.chars().count() <= 240 {
        compact
    } else {
        let preview = compact.chars().take(240).collect::<String>();
        format!("{preview}...")
    }
}

fn network_error_message(error: &reqwest::Error, redaction_values: &[String]) -> String {
    let message = if error.is_timeout() {
        "Timed out connecting to the provider base URL.".to_string()
    } else if error.is_connect() {
        "Could not connect to the provider base URL.".to_string()
    } else {
        format!("Provider request failed before an HTTP response: {error}")
    };
    sanitize_text(&message, redaction_values)
}

fn safe_base_url_label(base_url: &str) -> String {
    let trimmed = base_url.trim();
    let Ok(mut url) = reqwest::Url::parse(trimmed) else {
        return sanitize_malformed_base_url_label(trimmed);
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    sanitize_text(url.to_string().trim_end_matches('/'), &[])
}

fn sanitize_malformed_base_url_label(value: &str) -> String {
    let stripped = value
        .split('#')
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/');
    let sanitized = sanitize_text(stripped, &[]);
    if sanitized.is_empty()
        || sanitized.contains('@')
        || sanitized.to_ascii_lowercase().contains("api_key")
        || sanitized.to_ascii_lowercase().contains("token")
        || sanitized.to_ascii_lowercase().contains("secret")
        || sanitized.to_ascii_lowercase().contains("password")
    {
        "[invalid base URL]".to_string()
    } else {
        sanitized
    }
}

fn redaction_values(api_key: &str, base_url: Option<&str>) -> Vec<String> {
    let mut values = Vec::new();
    let key = api_key.trim();
    if key.len() >= 4 {
        values.push(key.to_string());
    }
    if let Some(base_url) = base_url {
        if let Ok(url) = reqwest::Url::parse(base_url) {
            if !url.username().is_empty() {
                values.push(url.username().to_string());
            }
            if let Some(password) = url.password() {
                values.push(password.to_string());
            }
            for (_, value) in url.query_pairs() {
                let value = value.trim();
                if value.len() >= 4 {
                    values.push(value.to_string());
                }
            }
        }
    }
    values
}

fn sanitize_text(input: &str, redaction_values: &[String]) -> String {
    let redactor = crate::redaction::PersistentLogRedactor::new();
    for value in redaction_values {
        if value.len() >= 4 {
            redactor.register_secret(value);
        }
    }
    redactor
        .redact_text(input)
        .unwrap_or_else(|_| "[redacted]".to_string())
}

fn validate_streaming_response(
    transport: ProviderTransport,
    content_type: Option<&str>,
    body: &str,
) -> Result<(), String> {
    let content_type_is_sse = content_type
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains("text/event-stream");
    if body_has_expected_sse_event(transport, body) {
        return Ok(());
    }
    if body.trim().is_empty() {
        return Err(
            "Provider returned HTTP success without a streaming response body.".to_string(),
        );
    }
    if content_type_is_sse {
        return Err(
            "Provider returned event-stream content without a recognizable streaming SSE event."
                .to_string(),
        );
    }
    Err("Provider returned HTTP success without a recognizable streaming SSE response.".to_string())
}

fn body_has_expected_sse_event(transport: ProviderTransport, body: &str) -> bool {
    for line in body.lines() {
        let Some(data) = line.trim().strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            return matches!(
                transport,
                ProviderTransport::OpenAiChatCompletions
                    | ProviderTransport::CustomOpenAiCompatible
            );
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
            continue;
        };
        match transport {
            ProviderTransport::AnthropicMessages | ProviderTransport::CustomAnthropicCompatible => {
                if value.get("type").and_then(|value| value.as_str()).is_some() {
                    return true;
                }
            }
            ProviderTransport::OpenAiChatCompletions
            | ProviderTransport::CustomOpenAiCompatible => {
                if value
                    .get("choices")
                    .and_then(|value| value.as_array())
                    .is_some()
                {
                    return true;
                }
            }
            ProviderTransport::OpenAiResponses
            | ProviderTransport::NativeGemini
            | ProviderTransport::BedrockConverse => {}
        }
    }
    false
}

fn looks_like_tool_schema_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    (lower.contains("tool") || lower.contains("function"))
        && (lower.contains("not support")
            || lower.contains("unsupported")
            || lower.contains("schema")
            || lower.contains("tool_choice"))
}

fn looks_like_streaming_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("stream") && (lower.contains("not support") || lower.contains("unsupported"))
}

fn looks_like_model_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("model")
        && (lower.contains("not found")
            || lower.contains("does not exist")
            || lower.contains("invalid")
            || lower.contains("not support")
            || lower.contains("unsupported"))
}

fn looks_like_auth_error(status: u16, message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    matches!(status, 401 | 403)
        || lower.contains("api key")
        || lower.contains("authentication")
        || lower.contains("unauthorized")
        || lower.contains("invalid_api_key")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::provider_registry::load_provider_profiles;
    use crate::settings::Credentials;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::sync::mpsc;
    use std::time::Duration;

    #[tokio::test]
    async fn provider_probe_fails_missing_key_without_network() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
        listener
            .set_nonblocking(true)
            .expect("make test listener nonblocking");
        let base_url = format!("http://{}", listener.local_addr().expect("local addr"));

        let result = probe_builtin_provider_with_credentials(
            "openai",
            Credentials {
                api_key: String::new(),
                api_base: Some(base_url),
                model: Some("probe-model".to_string()),
            },
            reqwest::Client::new(),
        )
        .await;

        assert_eq!(result.status, ProviderProbeStatus::Failed);
        assert_check(&result, "key_present", ProviderProbeCheckStatus::Failed);
        assert_eq!(result.message, "OpenAI API key is missing.");
        assert!(
            listener.accept().is_err(),
            "probe should not call network without a key"
        );
    }

    #[tokio::test]
    async fn provider_probe_success_validates_openai_compatible_contract() {
        let (base_url, request_rx) = spawn_probe_response_server(
            200,
            "text/event-stream",
            "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}\n\ndata: [DONE]\n\n",
        );

        let result = probe_builtin_provider_with_credentials(
            "openai",
            Credentials {
                api_key: "test-key".to_string(),
                api_base: Some(base_url),
                model: Some("probe-model".to_string()),
            },
            reqwest::Client::new(),
        )
        .await;

        assert_eq!(result.status, ProviderProbeStatus::Passed);
        assert_eq!(result.provider, "openai");
        assert_eq!(result.model.as_deref(), Some("probe-model"));
        assert_eq!(result.message, "OpenAI probe passed.");
        assert_check(&result, "key_present", ProviderProbeCheckStatus::Passed);
        assert_check(
            &result,
            "base_url_reachable",
            ProviderProbeCheckStatus::Passed,
        );
        assert_check(&result, "model_accepted", ProviderProbeCheckStatus::Passed);
        assert_check(
            &result,
            "streaming_accepted",
            ProviderProbeCheckStatus::Passed,
        );
        assert_check(
            &result,
            "tool_schema_accepted",
            ProviderProbeCheckStatus::Passed,
        );

        let request = request_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("captured probe request");
        assert_eq!(request["path"], "/chat/completions");
        assert_eq!(request["authorization"], "Bearer test-key");
        assert_eq!(request["body"]["model"], "probe-model");
        assert_eq!(request["body"]["stream"], true);
        assert_eq!(
            request["body"]["tools"][0]["function"]["name"],
            "forge_probe"
        );
        assert_eq!(request["body"]["tool_choice"], "none");

        let serialized = serde_json::to_string(&result).expect("serialize result");
        assert!(
            !serialized.contains("test-key"),
            "probe result must not leak API keys"
        );
    }

    #[test]
    fn cached_provider_probe_evidence_records_capture_time() {
        let result = ProviderProbeResult {
            provider: "openai".to_string(),
            provider_label: "OpenAI".to_string(),
            model: Some("gpt-4o".to_string()),
            base_url: Some("https://api.openai.com/v1".to_string()),
            status: ProviderProbeStatus::Passed,
            checks: vec![ProviderProbeCheck {
                id: "streaming_accepted".to_string(),
                label: "Streaming accepted".to_string(),
                status: ProviderProbeCheckStatus::Passed,
                message: "Streaming request was accepted.".to_string(),
            }],
            message: "OpenAI probe passed.".to_string(),
            remediation: None,
        };

        let evidence = cached_probe_evidence(&result);

        assert!(
            evidence.recorded_at_ms.is_some_and(|value| value > 0),
            "cached manual probe evidence must include when it was recorded"
        );
    }

    #[tokio::test]
    async fn provider_probe_allows_no_auth_ollama_without_key_header() {
        let (base_url, request_rx) = spawn_probe_response_server(
            200,
            "text/event-stream",
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\"}}\n\n",
        );

        let result = probe_builtin_provider_with_credentials(
            "ollama",
            Credentials {
                api_key: String::new(),
                api_base: Some(base_url),
                model: Some("qwen2.5-coder".to_string()),
            },
            reqwest::Client::new(),
        )
        .await;

        assert_eq!(result.status, ProviderProbeStatus::Passed);
        assert_eq!(result.provider, "ollama");
        assert_check(&result, "key_present", ProviderProbeCheckStatus::Passed);

        let request = request_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("captured probe request");
        assert_eq!(request["path"], "/v1/messages");
        assert_eq!(request["authorization"], "");
        assert_eq!(request["x_api_key"], "");
        assert_eq!(request["body"]["model"], "qwen2.5-coder");
        assert_eq!(request["body"]["tools"][0]["name"], "forge_probe");
    }

    #[tokio::test]
    async fn provider_probe_reports_unsupported_tool_schema() {
        let (base_url, _request_rx) = spawn_probe_response_server(
            400,
            "application/json",
            &json!({
                "error": {
                    "message": "This model does not support tools or function schemas."
                }
            })
            .to_string(),
        );

        let result = probe_builtin_provider_with_credentials(
            "openai",
            Credentials {
                api_key: "test-key".to_string(),
                api_base: Some(base_url),
                model: Some("probe-model".to_string()),
            },
            reqwest::Client::new(),
        )
        .await;

        assert_eq!(result.status, ProviderProbeStatus::Failed);
        assert_eq!(result.message, "OpenAI tool schema unsupported.");
        assert_check(&result, "key_present", ProviderProbeCheckStatus::Passed);
        assert_check(
            &result,
            "base_url_reachable",
            ProviderProbeCheckStatus::Passed,
        );
        assert_check(&result, "model_accepted", ProviderProbeCheckStatus::Passed);
        assert_check(
            &result,
            "streaming_accepted",
            ProviderProbeCheckStatus::Passed,
        );
        let tool_check = result
            .checks
            .iter()
            .find(|check| check.id == "tool_schema_accepted")
            .expect("tool schema check");
        assert_eq!(tool_check.status, ProviderProbeCheckStatus::Failed);
        assert_eq!(
            tool_check.message,
            "Provider rejected the no-op tool schema: This model does not support tools or function schemas."
        );
    }

    #[tokio::test]
    async fn provider_probe_rejects_200_json_without_streaming_shape() {
        let (base_url, _request_rx) = spawn_probe_response_server(200, "application/json", "{}");

        let result = probe_builtin_provider_with_credentials(
            "openai",
            Credentials {
                api_key: "test-key".to_string(),
                api_base: Some(base_url),
                model: Some("probe-model".to_string()),
            },
            reqwest::Client::new(),
        )
        .await;

        assert_eq!(result.status, ProviderProbeStatus::Failed);
        assert_check(&result, "key_present", ProviderProbeCheckStatus::Passed);
        assert_check(
            &result,
            "base_url_reachable",
            ProviderProbeCheckStatus::Passed,
        );
        assert_check(&result, "model_accepted", ProviderProbeCheckStatus::Passed);
        assert_check(
            &result,
            "streaming_accepted",
            ProviderProbeCheckStatus::Failed,
        );
        assert_check(
            &result,
            "tool_schema_accepted",
            ProviderProbeCheckStatus::Failed,
        );
        assert_eq!(
            result.message,
            "OpenAI streaming response was not confirmed."
        );
    }

    #[tokio::test]
    async fn provider_probe_rejects_event_stream_without_expected_sse_event() {
        let (base_url, _request_rx) =
            spawn_probe_response_server(200, "text/event-stream", "still warming up\n");

        let result = probe_builtin_provider_with_credentials(
            "openai",
            Credentials {
                api_key: "test-key".to_string(),
                api_base: Some(base_url),
                model: Some("probe-model".to_string()),
            },
            reqwest::Client::new(),
        )
        .await;

        assert_eq!(result.status, ProviderProbeStatus::Failed);
        assert_check(&result, "key_present", ProviderProbeCheckStatus::Passed);
        assert_check(
            &result,
            "base_url_reachable",
            ProviderProbeCheckStatus::Passed,
        );
        assert_check(&result, "model_accepted", ProviderProbeCheckStatus::Passed);
        assert_check(
            &result,
            "streaming_accepted",
            ProviderProbeCheckStatus::Failed,
        );
        assert_check(
            &result,
            "tool_schema_accepted",
            ProviderProbeCheckStatus::Failed,
        );
        assert_eq!(
            result.message,
            "OpenAI streaming response was not confirmed."
        );
    }

    #[tokio::test]
    async fn provider_probe_redacts_echoed_key_and_secret_base_url() {
        let secret = "sk-secret-review-key-1234567890";
        let (base_url, _request_rx) = spawn_probe_response_server(
            401,
            "application/json",
            &json!({
                "error": {
                    "message": format!("Invalid API key {secret}; Authorization: Bearer {secret}")
                }
            })
            .to_string(),
        );
        let secret_url = format!("{base_url}?api_key={secret}#token={secret}");

        let result = probe_builtin_provider_with_credentials(
            "openai",
            Credentials {
                api_key: secret.to_string(),
                api_base: Some(secret_url),
                model: Some("probe-model".to_string()),
            },
            reqwest::Client::new(),
        )
        .await;

        let serialized = serde_json::to_string(&result).expect("serialize result");
        assert!(!serialized.contains(secret), "{serialized}");
        assert!(serialized.contains("[redacted]"), "{serialized}");
        assert_eq!(result.base_url.as_deref(), Some(base_url.as_str()));
    }

    #[tokio::test]
    async fn provider_probe_sanitizes_malformed_secret_base_url() {
        let secret = "sk-secret-query-abcdef1234567890";
        let result = probe_builtin_provider_with_credentials(
            "custom_openai",
            Credentials {
                api_key: "test-key".to_string(),
                api_base: Some(format!(
                    "not a url?api_key={secret}@example.com#token={secret}"
                )),
                model: Some("probe-model".to_string()),
            },
            reqwest::Client::new(),
        )
        .await;

        let serialized = serde_json::to_string(&result).expect("serialize result");
        assert!(!serialized.contains(secret), "{serialized}");
        assert_ne!(
            result.base_url.as_deref(),
            Some("not a url?api_key=sk-secret-query-abcdef1234567890@example.com#token=sk-secret-query-abcdef1234567890")
        );
    }

    #[tokio::test]
    async fn provider_probe_sanitizes_path_secret_base_url() {
        let secret = "sk-secret-path-abcdef1234567890";
        let (base_url, _request_rx) = spawn_probe_response_server(
            200,
            "text/event-stream",
            "data: {\"choices\":[{\"delta\":{}}]}\n\ndata: [DONE]\n\n",
        );
        let secret_url = format!("{base_url}/{secret}");

        let result = probe_builtin_provider_with_credentials(
            "openai",
            Credentials {
                api_key: "test-key".to_string(),
                api_base: Some(secret_url),
                model: Some("probe-model".to_string()),
            },
            reqwest::Client::new(),
        )
        .await;

        let serialized = serde_json::to_string(&result).expect("serialize result");
        assert!(!serialized.contains(secret), "{serialized}");
        let expected_base_url = format!("{base_url}/[redacted]");
        assert_eq!(result.base_url.as_deref(), Some(expected_base_url.as_str()));
    }

    fn assert_check(result: &ProviderProbeResult, id: &str, expected: ProviderProbeCheckStatus) {
        let check = result
            .checks
            .iter()
            .find(|check| check.id == id)
            .unwrap_or_else(|| panic!("missing check {id}"));
        assert_eq!(check.status, expected, "{id}: {}", check.message);
    }

    async fn probe_builtin_provider_with_credentials(
        provider: &str,
        credentials: Credentials,
        client: reqwest::Client,
    ) -> ProviderProbeResult {
        let profiles = load_provider_profiles(&[]).expect("built-in profiles load");
        probe_provider_with_credentials_and_profiles(provider, credentials, client, &profiles).await
    }

    fn spawn_probe_response_server(
        status: u16,
        content_type: &'static str,
        response_body: &str,
    ) -> (String, mpsc::Receiver<serde_json::Value>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
        let (tx, rx) = mpsc::channel();
        let response_body = response_body.to_string();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let request = read_http_request(&mut stream);
            tx.send(request).expect("send request");

            let reason = if status == 200 { "OK" } else { "Bad Request" };
            write!(
                stream,
                "HTTP/1.1 {status} {reason}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
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
        let header_end = loop {
            let read = stream.read(&mut chunk).expect("read request");
            assert!(read > 0, "connection closed before headers");
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(index) = find_subslice(&buffer, b"\r\n\r\n") {
                break index + 4;
            }
        };

        let headers = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        while buffer.len() < header_end + content_length {
            let read = stream.read(&mut chunk).expect("read request body");
            assert!(read > 0, "connection closed before body");
            buffer.extend_from_slice(&chunk[..read]);
        }
        let body = &buffer[header_end..header_end + content_length];
        let body: serde_json::Value = serde_json::from_slice(body).expect("json body");
        let request_line = headers.lines().next().unwrap_or_default();
        let path = request_line.split_whitespace().nth(1).unwrap_or_default();
        let authorization = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("authorization")
                    .then(|| value.trim().to_string())
            })
            .unwrap_or_default();
        let x_api_key = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("x-api-key")
                    .then(|| value.trim().to_string())
            })
            .unwrap_or_default();

        json!({
            "path": path,
            "authorization": authorization,
            "x_api_key": x_api_key,
            "body": body,
        })
    }

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }
}
