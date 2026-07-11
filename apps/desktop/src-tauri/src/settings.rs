use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::adapters::provider_registry::{
    find_loaded_provider_profile, get_provider_definition, load_provider_profiles,
    normalize_provider_id, EnvVarList, LoadedProviderProfile, ProviderProfileConfig,
    ProviderProfileLoadError, ProviderProfileSource, ProviderTransport,
};
use crate::credential_store::{CredentialRef, CredentialStore};
use crate::profile::ForgeProfile;
use crate::redaction::{global_redactor, PersistentLogRedactor};

/// Persisted user settings stored in ~/.forge/config.json
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub credential_refs: HashMap<String, CredentialRef>,
    #[serde(default)]
    pub(crate) providers: Vec<ProviderProfileConfig>,
    #[serde(default)]
    pub(crate) provider_model_catalogs: HashMap<String, CachedProviderModelCatalog>,
    #[serde(default)]
    pub(crate) provider_probe_evidence: HashMap<String, CachedProviderProbeEvidence>,
}

/// Auto-detected credentials from Claude Code config + env vars
#[derive(Debug, Clone)]
pub struct Credentials {
    pub api_key: String,
    pub api_base: Option<String>,
    pub model: Option<String>,
}

/// Raw Claude Code settings.json structure (only the fields we care about)
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct ClaudeSettings {
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    api_base: Option<String>,
    #[serde(default)]
    #[serde(rename = "apiKey")]
    api_key_camel: Option<String>,
    #[serde(default)]
    #[serde(rename = "apiBase")]
    api_base_camel: Option<String>,
    #[serde(default)]
    model: Option<String>,
    /// Nested env vars (set by Claude Code at launch)
    #[serde(default)]
    env: Option<HashMap<String, String>>,
}

impl ClaudeSettings {
    fn api_key(&self) -> Option<String> {
        // Nested env vars FIRST — they're set by Claude Code at runtime
        self.env_var("ANTHROPIC_AUTH_TOKEN")
            .or_else(|| self.env_var("ANTHROPIC_API_KEY"))
            .or_else(|| self.api_key.clone())
            .or_else(|| self.api_key_camel.clone())
    }
    fn api_base(&self) -> Option<String> {
        self.env_var("ANTHROPIC_BASE_URL")
            .or_else(|| self.api_base.clone())
            .or_else(|| self.api_base_camel.clone())
    }
    fn model(&self) -> Option<String> {
        self.env_var("ANTHROPIC_MODEL")
            .or_else(|| self.model.clone())
    }
    fn env_var(&self, key: &str) -> Option<String> {
        self.env.as_ref()?.get(key).cloned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CredentialResolutionError {
    #[error(
        "System credential store is unavailable. Open Settings and save the provider key again."
    )]
    StoreUnavailable,
    #[error("A saved credential reference has no matching secret. Open Settings and save the provider key again.")]
    MissingReferencedSecret,
}

pub struct CredentialResolver {
    store: Arc<dyn CredentialStore>,
    redactor: Arc<PersistentLogRedactor>,
}

impl CredentialResolver {
    pub fn new(store: Arc<dyn CredentialStore>) -> Self {
        Self {
            store,
            redactor: global_redactor(),
        }
    }

    pub fn resolve(
        &self,
        provider: &str,
        profile: Option<&ForgeProfile>,
    ) -> Result<Credentials, CredentialResolutionError> {
        let settings = Settings::load();
        let claude_config = read_claude_settings();
        self.resolve_from_sources(provider, profile, &settings, &claude_config, |key| {
            std::env::var(key).ok()
        })
    }

    fn resolve_from_sources<F>(
        &self,
        provider: &str,
        profile: Option<&ForgeProfile>,
        settings: &Settings,
        claude_config: &ClaudeSettings,
        env: F,
    ) -> Result<Credentials, CredentialResolutionError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let reference = stored_credential_ref(settings, profile, provider);
        let stored_key = match reference {
            Some(reference) => self
                .store
                .get(reference)
                .map_err(|_| CredentialResolutionError::StoreUnavailable)?
                .ok_or(CredentialResolutionError::MissingReferencedSecret)?
                .into(),
            None => None,
        };
        let credentials = detect_credentials_with_stored_key_from_sources(
            provider,
            settings,
            claude_config,
            stored_key,
            env,
        );
        if !credentials.api_key.trim().is_empty() {
            self.redactor.register_secret(&credentials.api_key);
        }
        Ok(credentials)
    }
}

pub(crate) fn load_configured_provider_profiles() -> Vec<LoadedProviderProfile> {
    Settings::load().provider_profiles_or_builtin()
}

pub(crate) fn provider_requires_api_key(provider: &str) -> bool {
    Settings::load().provider_requires_api_key(provider)
}

fn detect_credentials_from_sources<F>(
    provider: &str,
    settings: &Settings,
    claude_config: &ClaudeSettings,
    env: F,
) -> Credentials
where
    F: Fn(&str) -> Option<String>,
{
    detect_credentials_with_stored_key_from_sources(provider, settings, claude_config, None, env)
}

fn detect_credentials_with_stored_key_from_sources<F>(
    provider: &str,
    settings: &Settings,
    claude_config: &ClaudeSettings,
    stored_key: Option<String>,
    env: F,
) -> Credentials
where
    F: Fn(&str) -> Option<String>,
{
    let raw_provider = provider.trim();
    let profiles = settings.provider_profiles_or_builtin();
    let loaded_profile = find_loaded_provider_profile(&profiles, raw_provider);
    let provider_id = loaded_profile
        .map(|profile| profile.id.clone())
        .unwrap_or_else(|| normalize_settings_provider(raw_provider));
    let definition = get_provider_definition(&provider_id);
    let registry_api_key = loaded_profile
        .and_then(|profile| first_profile_env_value(&env, &provider_id, &profile.api_key_env))
        .or_else(|| {
            definition.and_then(|definition| {
                first_provider_env_value(&env, &provider_id, definition.api_key_env)
            })
        });
    let registry_base_url = loaded_profile
        .and_then(|profile| first_profile_env_value(&env, &provider_id, &profile.base_url_env))
        .or_else(|| {
            definition.and_then(|definition| {
                first_provider_env_value(&env, &provider_id, definition.base_url_env)
            })
        });
    let profile_base_url = loaded_profile.and_then(|profile| profile.default_base_url.clone());
    let configured_default_model = loaded_profile
        .filter(|profile| profile.source != ProviderProfileSource::BuiltIn)
        .map(|profile| profile.default_model.clone());

    let api_key = match provider_id.as_str() {
        "anthropic" => stored_key
            .or_else(|| claude_config.api_key())
            .or(registry_api_key)
            .unwrap_or_default(),
        _ => stored_key.or(registry_api_key).unwrap_or_default(),
    };

    let api_base = match provider_id.as_str() {
        "anthropic" => claude_config
            .api_base()
            .or(registry_base_url)
            .or(profile_base_url),
        _ => registry_base_url.or(profile_base_url),
    };

    let model = match provider_id.as_str() {
        "anthropic" => claude_config
            .model()
            .or_else(|| first_env_value(&env, provider_model_env_vars("anthropic")))
            .or(configured_default_model),
        _ => first_env_value(&env, provider_model_env_vars(&provider_id))
            .or(configured_default_model),
    };

    Credentials {
        api_key,
        api_base,
        model,
    }
}

fn normalize_settings_provider(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        "hermes" => "anthropic".to_string(),
        "codex" => "openai".to_string(),
        other => normalize_provider_id(Some(other))
            .map(str::to_string)
            .unwrap_or_else(|| other.to_string()),
    }
}

fn stored_credential_ref<'a>(
    settings: &'a Settings,
    profile: Option<&'a ForgeProfile>,
    provider: &str,
) -> Option<&'a CredentialRef> {
    let raw_provider = provider.trim();
    let profiles = settings.provider_profiles_or_builtin();
    let loaded_profile = find_loaded_provider_profile(&profiles, raw_provider);
    let provider_id = loaded_profile
        .map(|profile| profile.id.clone())
        .unwrap_or_else(|| normalize_settings_provider(raw_provider));
    let loaded_aliases = loaded_profile
        .map(|profile| profile.aliases.as_slice())
        .unwrap_or(&[]);
    let find = |references: &'a HashMap<String, CredentialRef>| {
        references
            .get(&provider_id)
            .or_else(|| references.get(raw_provider))
            .or_else(|| {
                loaded_aliases
                    .iter()
                    .find_map(|alias| references.get(alias))
            })
            .or_else(|| {
                provider_alias_keys(&provider_id)
                    .iter()
                    .find_map(|alias| references.get(*alias))
            })
    };
    profile
        .and_then(|profile| find(&profile.credential_overrides))
        .or_else(|| find(&settings.credential_refs))
}

fn credential_store_recovery_message() -> String {
    "System credential store is unavailable. Open Settings and save the provider key again."
        .to_string()
}

fn rollback_credential(
    store: &dyn CredentialStore,
    reference: &CredentialRef,
    previous: Option<&str>,
) {
    if let Some(previous) = previous {
        let _ = store.put(reference, previous);
    } else {
        let _ = store.delete(reference);
    }
}

fn first_profile_env_value<F>(env: &F, provider_id: &str, keys: &[String]) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    keys.iter()
        .map(String::as_str)
        .filter(|key| provider_env_allowed(provider_id, key))
        .find_map(|key| env(key).filter(|value| !value.trim().is_empty()))
}

fn provider_alias_keys(provider_id: &str) -> &'static [&'static str] {
    match provider_id {
        "anthropic" => &["claude", "hermes"],
        "openai" => &["gpt", "codex"],
        "kimi" => &["moonshot"],
        "glm" => &["zhipu", "z.ai", "zai", "z-ai"],
        "alibaba" => &["qwen", "dashscope"],
        "gemini" => &["google"],
        "xai" => &["grok", "x.ai"],
        "ollama" => &["local", "vllm", "lmstudio", "llama.cpp"],
        "custom_openai" => &["custom-openai", "openai_compatible"],
        "custom_anthropic" => &["custom-anthropic", "anthropic_compatible"],
        _ => &[],
    }
}

fn first_provider_env_value<F>(env: &F, provider_id: &str, keys: &[&str]) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    keys.iter()
        .copied()
        .filter(|key| provider_env_allowed(provider_id, key))
        .find_map(|key| env(key).filter(|value| !value.trim().is_empty()))
}

fn provider_env_allowed(provider_id: &str, key: &str) -> bool {
    provider_id == "anthropic"
        || !matches!(
            key,
            "ANTHROPIC_AUTH_TOKEN" | "ANTHROPIC_API_KEY" | "ANTHROPIC_BASE_URL"
        ) && (provider_id == "openai" || !matches!(key, "OPENAI_API_KEY" | "OPENAI_BASE_URL"))
}

fn first_env_value<F>(env: &F, keys: &[&str]) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    keys.iter()
        .copied()
        .find_map(|key| env(key).filter(|value| !value.trim().is_empty()))
}

fn provider_model_env_vars(provider_id: &str) -> &'static [&'static str] {
    match provider_id {
        "deepseek" => &["DEEPSEEK_MODEL"],
        "anthropic" => &["ANTHROPIC_MODEL"],
        "kimi" => &["KIMI_MODEL", "MOONSHOT_MODEL"],
        "glm" => &["GLM_MODEL", "ZHIPU_MODEL"],
        "alibaba" => &["ALIBABA_MODEL", "QWEN_MODEL"],
        "minimax" => &["MINIMAX_MODEL"],
        "openai" => &["OPENAI_MODEL"],
        "openrouter" => &["OPENROUTER_MODEL"],
        "gemini" => &["GEMINI_MODEL"],
        "xai" => &["XAI_MODEL"],
        "groq" => &["GROQ_MODEL"],
        "mistral" => &["MISTRAL_MODEL"],
        "ollama" => &["OLLAMA_MODEL"],
        "custom_openai" => &["FORGE_CUSTOM_OPENAI_MODEL"],
        "custom_anthropic" => &["FORGE_CUSTOM_ANTHROPIC_MODEL"],
        _ => &[],
    }
}

fn provider_profile_source_name(source: ProviderProfileSource) -> &'static str {
    match source {
        ProviderProfileSource::BuiltIn => "built_in",
        ProviderProfileSource::UserOverride => "user_override",
        ProviderProfileSource::UserDefined => "user_defined",
    }
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

fn normalize_profile_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}

fn normalize_profile_input_id(value: &str) -> Result<String, String> {
    let key = normalize_profile_key(value);
    if key.is_empty() {
        return Err("Provider id is required.".to_string());
    }
    Ok(normalize_provider_id(Some(&key))
        .map(str::to_string)
        .unwrap_or(key))
}

fn clean_string_list(values: Vec<String>) -> Vec<String> {
    let mut cleaned = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() || cleaned.iter().any(|item: &String| item == value) {
            continue;
        }
        cleaned.push(value.to_string());
    }
    cleaned
}

fn default_true() -> bool {
    true
}

fn read_claude_settings() -> ClaudeSettings {
    let path = home_dir().join(".claude").join("settings.json");
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        ClaudeSettings::default()
    }
}

impl Settings {
    fn path() -> PathBuf {
        home_dir().join(".forge").join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn save(&self) -> Result<(), String> {
        self.save_to_path(&Self::path())
    }

    fn save_to_path(&self, path: &PathBuf) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, json).map_err(|e| format!("Failed to write config temp: {e}"))?;
        fs::rename(&temporary, path).map_err(|e| format!("Failed to replace config: {e}"))?;
        Ok(())
    }

    pub fn set_api_key(
        &mut self,
        store: &dyn CredentialStore,
        provider: &str,
        key: &str,
    ) -> Result<(), String> {
        self.set_api_key_at_path(store, provider, key, &Self::path())
    }

    fn set_api_key_at_path(
        &mut self,
        store: &dyn CredentialStore,
        provider: &str,
        key: &str,
        path: &PathBuf,
    ) -> Result<(), String> {
        let provider_id = normalize_settings_provider(provider);
        if provider_id.is_empty() {
            return Err("Provider is required.".to_string());
        }
        let matching_keys = self
            .credential_refs
            .keys()
            .filter(|key| normalize_settings_provider(key) == provider_id)
            .cloned()
            .collect::<Vec<_>>();
        let reference = matching_keys
            .iter()
            .find_map(|key| self.credential_refs.get(key))
            .cloned()
            .unwrap_or_else(|| CredentialRef::provider(&provider_id));
        let previous = store
            .get(&reference)
            .map_err(|_| credential_store_recovery_message())?;

        if key.trim().is_empty() {
            store
                .delete(&reference)
                .map_err(|_| credential_store_recovery_message())?;
            self.credential_refs
                .retain(|key, _| normalize_settings_provider(key) != provider_id);
        } else {
            store
                .put(&reference, key)
                .map_err(|_| credential_store_recovery_message())?;
            let verified = store
                .get(&reference)
                .map_err(|_| credential_store_recovery_message())?;
            if verified.as_deref() != Some(key) {
                rollback_credential(store, &reference, previous.as_deref());
                return Err(
                    "Credential store verification failed. Save the provider key again."
                        .to_string(),
                );
            }
            self.credential_refs
                .retain(|key, _| normalize_settings_provider(key) != provider_id);
            self.credential_refs
                .insert(provider_id.clone(), reference.clone());
        }
        self.provider_probe_evidence.remove(&provider_id);
        if let Err(error) = self.save_to_path(path) {
            rollback_credential(store, &reference, previous.as_deref());
            return Err(error);
        }
        if !key.trim().is_empty() {
            global_redactor().register_secret(key);
        }
        Ok(())
    }

    pub fn key_status(&self, store: &dyn CredentialStore) -> Vec<KeyStatus> {
        let mut status = Vec::new();
        for (provider, reference) in &self.credential_refs {
            let (credential_status, error) = match store.get(reference) {
                Ok(Some(_)) => ("available".to_string(), None),
                Ok(None) => (
                    "missing".to_string(),
                    Some("Saved credential is missing. Save the provider key again.".to_string()),
                ),
                Err(_) => (
                    "unavailable".to_string(),
                    Some(credential_store_recovery_message()),
                ),
            };
            status.push(KeyStatus {
                provider: provider.clone(),
                configured: true,
                source: "system_store".to_string(),
                status: credential_status,
                error,
            });
        }
        // Always include known and configured providers, even if not set.
        for profile in self.provider_profiles_or_builtin() {
            if !self.credential_refs.contains_key(&profile.id) {
                status.push(KeyStatus {
                    provider: profile.id,
                    configured: false,
                    source: "none".to_string(),
                    status: "not_configured".to_string(),
                    error: None,
                });
            }
        }
        status.sort_by(|a, b| a.provider.cmp(&b.provider));
        status
    }

    pub(crate) fn provider_profiles(
        &self,
    ) -> Result<Vec<LoadedProviderProfile>, ProviderProfileLoadError> {
        load_provider_profiles(&self.providers)
    }

    pub(crate) fn provider_catalog(
        &self,
    ) -> Result<Vec<ProviderCatalogEntry>, ProviderProfileLoadError> {
        Ok(self
            .provider_profiles()?
            .into_iter()
            .map(|profile| {
                let provider_id = profile.id.clone();
                let context_window_tokens = get_provider_definition(&profile.id)
                    .and_then(|definition| definition.context_window_tokens);
                let cached_model_catalog = self.provider_model_catalogs.get(&provider_id);
                let probe_evidence = self.provider_probe_evidence.get(&provider_id).cloned();
                ProviderCatalogEntry {
                    id: profile.id,
                    label: profile.label,
                    default_model: profile.default_model,
                    context_window_tokens,
                    aliases: profile.aliases,
                    requires_api_key: !profile.api_key_env.is_empty(),
                    supports_streaming: profile.supports_streaming,
                    supports_tools: profile.supports_tools,
                    source: provider_profile_source_name(profile.source).to_string(),
                    base_url: profile.default_base_url,
                    transport: provider_transport_name(profile.transport).to_string(),
                    api_key_env: profile.api_key_env,
                    base_url_env: profile.base_url_env,
                    model_catalog_source: cached_model_catalog.and_then(|catalog| catalog.source),
                    model_catalog_recorded_at_ms: cached_model_catalog
                        .and_then(|catalog| catalog.recorded_at_ms),
                    probe_evidence,
                    models: cached_model_catalog
                        .map(|catalog| catalog.models.clone())
                        .unwrap_or_default(),
                }
            })
            .collect())
    }

    pub fn upsert_provider_profile(
        &mut self,
        input: ProviderProfileInput,
    ) -> Result<ProviderCatalogEntry, String> {
        let id = normalize_profile_input_id(&input.id)?;
        self.apply_provider_profile_input(input)?;
        self.save()?;
        self.provider_catalog()
            .map_err(|error| format!("{error:?}"))?
            .into_iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| "Provider profile was saved but could not be read back.".to_string())
    }

    pub fn delete_provider_profile(&mut self, provider: &str) -> Result<(), String> {
        self.apply_delete_provider_profile(provider)?;
        self.save()
    }

    fn apply_provider_profile_input(&mut self, input: ProviderProfileInput) -> Result<(), String> {
        let id = normalize_profile_input_id(&input.id)?;
        let default_model = input.default_model.trim();
        if default_model.is_empty() {
            return Err("Default model is required.".to_string());
        }

        let config = ProviderProfileConfig {
            id: id.clone(),
            label: Some(input.label.trim().to_string()).filter(|value| !value.is_empty()),
            base_url: input
                .base_url
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            api_key_env: Some(EnvVarList::Many(clean_string_list(input.api_key_env))),
            base_url_env: Some(EnvVarList::Many(clean_string_list(input.base_url_env))),
            default_model: Some(default_model.to_string()),
            transport: Some(input.transport.trim().to_string()),
            supports_tools: Some(input.supports_tools),
            supports_streaming: Some(input.supports_streaming),
            max_output_tokens_default: None,
            aliases: clean_string_list(input.aliases),
        };

        load_provider_profiles(std::slice::from_ref(&config))
            .map_err(|error| format!("{error:?}"))?;
        self.providers
            .retain(|profile| normalize_profile_key(&profile.id) != id);
        self.providers.push(config);
        self.provider_probe_evidence.remove(&id);
        Ok(())
    }

    fn apply_delete_provider_profile(&mut self, provider: &str) -> Result<(), String> {
        let id = normalize_profile_input_id(provider)?;
        let before = self.providers.len();
        self.providers
            .retain(|profile| normalize_profile_key(&profile.id) != id);
        self.provider_model_catalogs.remove(&id);
        self.provider_probe_evidence.remove(&id);
        if self.providers.len() == before {
            return Err("No editable provider profile exists for that provider.".to_string());
        }
        Ok(())
    }

    pub(crate) fn record_provider_probe_evidence(
        &mut self,
        provider: &str,
        evidence: CachedProviderProbeEvidence,
    ) -> Result<(), String> {
        self.apply_provider_probe_evidence(provider, evidence)?;
        self.save()
    }

    fn apply_provider_probe_evidence(
        &mut self,
        provider: &str,
        evidence: CachedProviderProbeEvidence,
    ) -> Result<(), String> {
        let provider = provider.trim().to_ascii_lowercase();
        if provider.is_empty() {
            return Err("Provider is required to store probe evidence.".to_string());
        }
        self.provider_probe_evidence.insert(provider, evidence);
        Ok(())
    }

    pub(crate) fn record_provider_model_catalog(
        &mut self,
        provider: &str,
        base_url: Option<String>,
        source: ProviderModelCatalogSource,
        models: Vec<ProviderCatalogModel>,
    ) -> Result<(), String> {
        self.apply_provider_model_catalog(provider, base_url, source, models)?;
        self.save()
    }

    fn apply_provider_model_catalog(
        &mut self,
        provider: &str,
        base_url: Option<String>,
        source: ProviderModelCatalogSource,
        models: Vec<ProviderCatalogModel>,
    ) -> Result<(), String> {
        let provider = provider.trim().to_ascii_lowercase();
        if provider.is_empty() {
            return Err("Provider is required to store a model catalog.".to_string());
        }

        let mut cleaned_models = Vec::new();
        for model in models {
            let id = model.id.trim();
            if id.is_empty()
                || cleaned_models
                    .iter()
                    .any(|item: &ProviderCatalogModel| item.id == id)
            {
                continue;
            }
            cleaned_models.push(ProviderCatalogModel {
                id: id.to_string(),
                name: model.name.trim().to_string(),
                context_window_tokens: model.context_window_tokens,
            });
        }

        if cleaned_models.is_empty() {
            self.provider_model_catalogs.remove(&provider);
        } else {
            self.provider_model_catalogs.insert(
                provider,
                CachedProviderModelCatalog {
                    base_url,
                    source: Some(source),
                    recorded_at_ms: Some(current_epoch_millis()),
                    models: cleaned_models,
                },
            );
        }
        Ok(())
    }

    fn provider_profiles_or_builtin(&self) -> Vec<LoadedProviderProfile> {
        self.provider_profiles()
            .unwrap_or_else(|_| load_provider_profiles(&[]).expect("built-in providers load"))
    }

    fn provider_requires_api_key(&self, provider: &str) -> bool {
        let profiles = self.provider_profiles_or_builtin();
        find_loaded_provider_profile(&profiles, provider)
            .map(|profile| !profile.api_key_env.is_empty())
            .unwrap_or(true)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeyStatus {
    pub provider: String,
    pub configured: bool,
    pub source: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderCatalogEntry {
    pub id: String,
    pub label: String,
    pub default_model: String,
    pub context_window_tokens: Option<u32>,
    pub aliases: Vec<String>,
    pub requires_api_key: bool,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub source: String,
    pub base_url: Option<String>,
    pub transport: String,
    pub api_key_env: Vec<String>,
    pub base_url_env: Vec<String>,
    pub model_catalog_source: Option<ProviderModelCatalogSource>,
    pub model_catalog_recorded_at_ms: Option<u64>,
    pub probe_evidence: Option<CachedProviderProbeEvidence>,
    pub models: Vec<ProviderCatalogModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderProfileInput {
    pub id: String,
    pub label: String,
    pub transport: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_env: Vec<String>,
    #[serde(default)]
    pub base_url_env: Vec<String>,
    pub default_model: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default = "default_true")]
    pub supports_tools: bool,
    #[serde(default = "default_true")]
    pub supports_streaming: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CachedProviderModelCatalog {
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub source: Option<ProviderModelCatalogSource>,
    #[serde(default)]
    pub recorded_at_ms: Option<u64>,
    #[serde(default)]
    pub models: Vec<ProviderCatalogModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CachedProviderProbeEvidence {
    pub source: ProviderProbeEvidenceSource,
    pub status: ProviderProbeEvidenceStatus,
    #[serde(default)]
    pub recorded_at_ms: Option<u64>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub checks: Vec<CachedProviderProbeCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CachedProviderProbeCheck {
    pub id: String,
    pub label: String,
    pub status: ProviderProbeEvidenceCheckStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProbeEvidenceSource {
    ManualProbe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProbeEvidenceStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProbeEvidenceCheckStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderModelCatalogSource {
    LiveEndpoint,
    StaticFallback,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderCatalogModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub context_window_tokens: Option<u32>,
}

pub fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "••••".to_string();
    }
    let prefix: String = key.chars().take(4).collect();
    let suffix: String = key
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}••••{}", prefix, suffix)
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn current_epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::adapters::provider_registry::{
        valid_provider_ids, EnvVarList, ProviderProfileConfig,
    };
    use crate::credential_store::{
        CredentialRef, CredentialStore, MemoryCredentialStore, UnavailableCredentialStore,
    };
    use crate::settings::ProviderProfileInput;

    use super::{
        detect_credentials_from_sources, detect_credentials_with_stored_key_from_sources, mask_key,
        CachedProviderProbeCheck, CachedProviderProbeEvidence, ClaudeSettings, CredentialResolver,
        ForgeProfile, ProviderCatalogModel, ProviderModelCatalogSource,
        ProviderProbeEvidenceCheckStatus, ProviderProbeEvidenceSource, ProviderProbeEvidenceStatus,
        Settings,
    };

    #[test]
    fn settings_save_never_serializes_api_keys() {
        let settings: Settings = serde_json::from_value(serde_json::json!({
            "api_keys": {"openai": "forge-settings-serialization-secret"},
            "credential_refs": {
                "openai": {"service": CredentialRef::SERVICE, "account": "provider:openai"}
            }
        }))
        .expect("deserialize settings");

        let serialized = serde_json::to_string(&settings).expect("serialize settings");

        assert!(!serialized.contains("api_keys"));
        assert!(!serialized.contains("forge-settings-serialization-secret"));
    }

    fn temp_settings_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("forge-settings-{name}-{nanos}.json"))
    }

    #[test]
    fn setting_key_creates_reference_and_keychain_item() {
        let path = temp_settings_path("create-reference");
        let store = MemoryCredentialStore::default();
        let mut settings = Settings::default();

        settings
            .set_api_key_at_path(&store, "openai", "forge-write-only-secret", &path)
            .expect("set key");

        let reference = settings
            .credential_refs
            .get("openai")
            .expect("credential reference");
        assert_eq!(
            store.get(reference).expect("read key").as_deref(),
            Some("forge-write-only-secret")
        );
        let persisted = fs::read_to_string(&path).expect("read settings");
        assert!(persisted.contains("credential_refs"));
        assert!(!persisted.contains("forge-write-only-secret"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn deleting_key_removes_reference_and_keychain_item() {
        let path = temp_settings_path("delete-reference");
        let store = MemoryCredentialStore::default();
        let mut settings = Settings::default();
        settings
            .set_api_key_at_path(&store, "openai", "forge-delete-secret", &path)
            .expect("set key");
        let reference = settings
            .credential_refs
            .get("openai")
            .cloned()
            .expect("credential reference");

        settings
            .set_api_key_at_path(&store, "openai", "", &path)
            .expect("delete key");

        assert!(!settings.credential_refs.contains_key("openai"));
        assert_eq!(store.get(&reference).expect("read deleted key"), None);
        let persisted = fs::read_to_string(&path).expect("read settings");
        assert!(!persisted.contains("forge-delete-secret"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn deleting_canonical_key_removes_legacy_alias_reference() {
        let path = temp_settings_path("delete-alias-reference");
        let store = MemoryCredentialStore::default();
        let reference = CredentialRef::provider("moonshot");
        store
            .put(&reference, "forge-alias-delete-secret")
            .expect("put alias key");
        let mut settings = Settings {
            credential_refs: HashMap::from([("moonshot".to_string(), reference.clone())]),
            ..Settings::default()
        };

        settings
            .set_api_key_at_path(&store, "kimi", "", &path)
            .expect("delete canonical key");

        assert!(settings.credential_refs.is_empty());
        assert_eq!(store.get(&reference).expect("read deleted alias"), None);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn key_status_reports_configured_without_returning_secret() {
        let store = MemoryCredentialStore::default();
        let reference = CredentialRef::provider("openai");
        store
            .put(&reference, "forge-status-secret")
            .expect("put key");
        let settings = Settings {
            credential_refs: HashMap::from([("openai".to_string(), reference)]),
            ..Settings::default()
        };

        let status = settings.key_status(&store);
        let serialized = serde_json::to_string(&status).expect("serialize status");

        assert!(status.iter().any(|item| {
            item.provider == "openai" && item.configured && item.status == "available"
        }));
        assert!(!serialized.contains("forge-status-secret"));
        assert!(!serialized.contains("preview"));
    }

    #[test]
    fn unavailable_store_prevents_provider_start_with_recovery_message() {
        let reference = CredentialRef::provider("openai");
        let settings = Settings {
            credential_refs: HashMap::from([("openai".to_string(), reference)]),
            ..Settings::default()
        };
        let resolver = CredentialResolver::new(Arc::new(UnavailableCredentialStore::new(
            "test_unavailable",
        )));

        let error = resolver
            .resolve_from_sources(
                "openai",
                None,
                &settings,
                &ClaudeSettings::default(),
                |_| None,
            )
            .expect_err("unavailable store must block resolution");

        assert_eq!(
            error.to_string(),
            "System credential store is unavailable. Open Settings and save the provider key again."
        );
    }

    #[test]
    fn profile_credential_reference_precedes_provider_reference() {
        let store = Arc::new(MemoryCredentialStore::default());
        let provider_reference = CredentialRef::provider("openai");
        let profile_reference = CredentialRef::profile("work", "openai");
        store
            .put(&provider_reference, "provider-secret")
            .expect("put provider secret");
        store
            .put(&profile_reference, "profile-secret")
            .expect("put profile secret");
        let settings = Settings {
            credential_refs: HashMap::from([("openai".to_string(), provider_reference)]),
            ..Settings::default()
        };
        let profile = ForgeProfile {
            id: "work".to_string(),
            name: "Work".to_string(),
            default_provider: Some("openai".to_string()),
            default_model: None,
            default_workspace: None,
            credential_overrides: HashMap::from([("openai".to_string(), profile_reference)]),
            created_at_ms: 1,
            updated_at_ms: 1,
        };
        let resolver = CredentialResolver::new(store);

        let credentials = resolver
            .resolve_from_sources(
                "openai",
                Some(&profile),
                &settings,
                &ClaudeSettings::default(),
                |_| None,
            )
            .expect("resolve profile credential");

        assert_eq!(credentials.api_key, "profile-secret");
    }

    #[test]
    fn detect_credentials_prefers_stored_provider_key_over_claude_and_env() {
        let settings = Settings::default();
        let claude = ClaudeSettings {
            api_key: Some("claude-key".to_string()),
            api_base: Some("https://claude.example".to_string()),
            model: Some("claude-model".to_string()),
            ..Default::default()
        };

        let credentials = detect_credentials_with_stored_key_from_sources(
            "anthropic",
            &settings,
            &claude,
            Some("stored-key".to_string()),
            |key| match key {
                "ANTHROPIC_API_KEY" => Some("env-key".to_string()),
                "ANTHROPIC_BASE_URL" => Some("https://env.example".to_string()),
                "ANTHROPIC_MODEL" => Some("env-model".to_string()),
                _ => None,
            },
        );

        assert_eq!(credentials.api_key, "stored-key");
        assert_eq!(
            credentials.api_base.as_deref(),
            Some("https://claude.example")
        );
        assert_eq!(credentials.model.as_deref(), Some("claude-model"));
    }

    #[test]
    fn detect_credentials_uses_claude_nested_env_before_top_level_fields() {
        let settings = Settings::default();
        let claude = ClaudeSettings {
            api_key: Some("top-level-key".to_string()),
            api_base: Some("https://top-level.example".to_string()),
            model: Some("top-level-model".to_string()),
            env: Some(HashMap::from([
                (
                    "ANTHROPIC_AUTH_TOKEN".to_string(),
                    "nested-token".to_string(),
                ),
                (
                    "ANTHROPIC_BASE_URL".to_string(),
                    "https://nested.example".to_string(),
                ),
                ("ANTHROPIC_MODEL".to_string(), "nested-model".to_string()),
            ])),
            ..Default::default()
        };

        let credentials =
            detect_credentials_from_sources("anthropic", &settings, &claude, |key| match key {
                "ANTHROPIC_AUTH_TOKEN" => Some("process-token".to_string()),
                "ANTHROPIC_BASE_URL" => Some("https://process.example".to_string()),
                "ANTHROPIC_MODEL" => Some("process-model".to_string()),
                _ => None,
            });

        assert_eq!(credentials.api_key, "nested-token");
        assert_eq!(
            credentials.api_base.as_deref(),
            Some("https://nested.example")
        );
        assert_eq!(credentials.model.as_deref(), Some("nested-model"));
    }

    #[test]
    fn detect_credentials_keeps_provider_envs_isolated() {
        let settings = Settings::default();
        let claude = ClaudeSettings::default();

        let deepseek =
            detect_credentials_from_sources("deepseek", &settings, &claude, |key| match key {
                "ANTHROPIC_API_KEY" => Some("wrong-provider-key".to_string()),
                "DEEPSEEK_API_KEY" => Some("deepseek-key".to_string()),
                "DEEPSEEK_BASE_URL" => Some("https://deepseek.example".to_string()),
                _ => None,
            });
        let openai =
            detect_credentials_from_sources("openai", &settings, &claude, |key| match key {
                "DEEPSEEK_API_KEY" => Some("wrong-provider-key".to_string()),
                "OPENAI_API_KEY" => Some("openai-key".to_string()),
                "OPENAI_BASE_URL" => Some("https://openai.example".to_string()),
                _ => None,
            });

        assert_eq!(deepseek.api_key, "deepseek-key");
        assert_eq!(
            deepseek.api_base.as_deref(),
            Some("https://deepseek.example")
        );
        assert_eq!(openai.api_key, "openai-key");
        assert_eq!(openai.api_base.as_deref(), Some("https://openai.example"));
    }

    #[test]
    fn detect_credentials_uses_registry_env_vars_for_mainstream_providers() {
        let settings = Settings::default();
        let claude = ClaudeSettings::default();
        let cases = [
            (
                "gemini",
                "GEMINI_API_KEY",
                "gemini-key",
                "GEMINI_BASE_URL",
                "https://gemini.example",
            ),
            (
                "xai",
                "XAI_API_KEY",
                "xai-key",
                "XAI_BASE_URL",
                "https://xai.example",
            ),
            (
                "groq",
                "GROQ_API_KEY",
                "groq-key",
                "GROQ_BASE_URL",
                "https://groq.example",
            ),
            (
                "mistral",
                "MISTRAL_API_KEY",
                "mistral-key",
                "MISTRAL_BASE_URL",
                "https://mistral.example",
            ),
            (
                "alibaba",
                "DASHSCOPE_API_KEY",
                "dashscope-key",
                "ALIBABA_BASE_URL",
                "https://alibaba.example",
            ),
            (
                "minimax",
                "MINIMAX_CN_API_KEY",
                "minimax-cn-key",
                "MINIMAX_CN_BASE_URL",
                "https://minimax.example",
            ),
            (
                "kimi",
                "MOONSHOT_API_KEY",
                "moonshot-key",
                "MOONSHOT_BASE_URL",
                "https://moonshot.example",
            ),
            (
                "glm",
                "ZHIPU_API_KEY",
                "zhipu-key",
                "ZHIPU_BASE_URL",
                "https://zhipu.example",
            ),
            (
                "ollama",
                "",
                "",
                "OLLAMA_BASE_URL",
                "http://localhost:11434",
            ),
            (
                "custom_openai",
                "FORGE_CUSTOM_OPENAI_API_KEY",
                "custom-openai-key",
                "FORGE_CUSTOM_OPENAI_BASE_URL",
                "https://custom-openai.example",
            ),
            (
                "custom_anthropic",
                "FORGE_CUSTOM_ANTHROPIC_API_KEY",
                "custom-anthropic-key",
                "FORGE_CUSTOM_ANTHROPIC_BASE_URL",
                "https://custom-anthropic.example",
            ),
        ];

        for (provider, key_env, key_value, base_env, base_value) in cases {
            let credentials =
                detect_credentials_from_sources(provider, &settings, &claude, |key| {
                    if key == key_env {
                        Some(key_value.to_string())
                    } else if key == base_env {
                        Some(base_value.to_string())
                    } else {
                        None
                    }
                });

            assert_eq!(
                credentials.api_key, key_value,
                "API key fallback for {provider}"
            );
            assert_eq!(
                credentials.api_base.as_deref(),
                Some(base_value),
                "base URL fallback for {provider}"
            );
        }
    }

    #[test]
    fn detect_credentials_prefers_stored_key_over_registry_env_key() {
        let settings = Settings::default();
        let claude = ClaudeSettings::default();

        let credentials = detect_credentials_with_stored_key_from_sources(
            "moonshot",
            &settings,
            &claude,
            Some("stored-kimi-key".to_string()),
            |key| match key {
                "KIMI_API_KEY" => Some("env-kimi-key".to_string()),
                "MOONSHOT_API_KEY" => Some("env-moonshot-key".to_string()),
                "KIMI_BASE_URL" => Some("https://kimi.example".to_string()),
                _ => None,
            },
        );

        assert_eq!(credentials.api_key, "stored-kimi-key");
        assert_eq!(
            credentials.api_base.as_deref(),
            Some("https://kimi.example")
        );
    }

    #[test]
    fn detect_credentials_falls_back_to_alias_saved_keys_for_canonical_providers() {
        let store = Arc::new(MemoryCredentialStore::default());
        let moonshot = CredentialRef::provider("moonshot");
        let qwen = CredentialRef::provider("qwen");
        store
            .put(&moonshot, "stored-moonshot-key")
            .expect("store moonshot");
        store.put(&qwen, "stored-qwen-key").expect("store qwen");
        let settings = Settings {
            credential_refs: HashMap::from([
                ("moonshot".to_string(), moonshot),
                ("qwen".to_string(), qwen),
            ]),
            ..Default::default()
        };
        let claude = ClaudeSettings::default();
        let resolver = CredentialResolver::new(store);

        let kimi = resolver
            .resolve_from_sources("kimi", None, &settings, &claude, |key| match key {
                "KIMI_API_KEY" => Some("env-kimi-key".to_string()),
                _ => None,
            })
            .expect("resolve kimi alias");
        let alibaba = resolver
            .resolve_from_sources("alibaba", None, &settings, &claude, |key| match key {
                "ALIBABA_API_KEY" => Some("env-alibaba-key".to_string()),
                _ => None,
            })
            .expect("resolve alibaba alias");

        assert_eq!(kimi.api_key, "stored-moonshot-key");
        assert_eq!(alibaba.api_key, "stored-qwen-key");
    }

    #[test]
    fn detect_credentials_uses_provider_specific_model_envs_only() {
        let settings = Settings::default();
        let claude = ClaudeSettings::default();

        let openai =
            detect_credentials_from_sources("openai", &settings, &claude, |key| match key {
                "OPENAI_MODEL" => Some("gpt-4o-mini".to_string()),
                "ANTHROPIC_MODEL" => Some("claude-should-not-leak".to_string()),
                _ => None,
            });
        let kimi =
            detect_credentials_from_sources("moonshot", &settings, &claude, |key| match key {
                "KIMI_MODEL" => Some("kimi-k2".to_string()),
                "MOONSHOT_MODEL" => Some("moonshot-v1-32k".to_string()),
                "ANTHROPIC_MODEL" => Some("claude-should-not-leak".to_string()),
                _ => None,
            });
        let glm = detect_credentials_from_sources("zhipu", &settings, &claude, |key| match key {
            "ZHIPU_MODEL" => Some("glm-zhipu-model".to_string()),
            "ANTHROPIC_MODEL" => Some("claude-should-not-leak".to_string()),
            _ => None,
        });
        let anthropic =
            detect_credentials_from_sources("anthropic", &settings, &claude, |key| match key {
                "ANTHROPIC_MODEL" => Some("claude-sonnet-env".to_string()),
                "OPENAI_MODEL" => Some("gpt-should-not-leak".to_string()),
                _ => None,
            });

        assert_eq!(openai.model.as_deref(), Some("gpt-4o-mini"));
        assert_eq!(kimi.model.as_deref(), Some("kimi-k2"));
        assert_eq!(glm.model.as_deref(), Some("glm-zhipu-model"));
        assert_eq!(anthropic.model.as_deref(), Some("claude-sonnet-env"));
    }

    #[test]
    fn detect_credentials_keeps_anthropic_process_envs_scoped_to_anthropic() {
        let settings = Settings::default();
        let claude = ClaudeSettings::default();

        let custom_anthropic = detect_credentials_from_sources(
            "custom_anthropic",
            &settings,
            &claude,
            |key| match key {
                "ANTHROPIC_AUTH_TOKEN" => Some("anthropic-token".to_string()),
                "ANTHROPIC_API_KEY" => Some("anthropic-key".to_string()),
                "ANTHROPIC_BASE_URL" => Some("https://anthropic.example".to_string()),
                "ANTHROPIC_MODEL" => Some("claude-env-model".to_string()),
                _ => None,
            },
        );
        let anthropic =
            detect_credentials_from_sources("anthropic", &settings, &claude, |key| match key {
                "ANTHROPIC_AUTH_TOKEN" => Some("anthropic-token".to_string()),
                "ANTHROPIC_BASE_URL" => Some("https://anthropic.example".to_string()),
                "ANTHROPIC_MODEL" => Some("claude-env-model".to_string()),
                _ => None,
            });

        assert_eq!(custom_anthropic.api_key, "");
        assert_eq!(custom_anthropic.api_base, None);
        assert_eq!(custom_anthropic.model, None);
        assert_eq!(anthropic.api_key, "anthropic-token");
        assert_eq!(
            anthropic.api_base.as_deref(),
            Some("https://anthropic.example")
        );
        assert_eq!(anthropic.model.as_deref(), Some("claude-env-model"));
    }

    #[test]
    fn detect_credentials_keeps_openai_process_envs_out_of_custom_openai() {
        let settings = Settings::default();
        let claude = ClaudeSettings::default();

        let custom_openai =
            detect_credentials_from_sources("custom_openai", &settings, &claude, |key| match key {
                "OPENAI_API_KEY" => Some("openai-key".to_string()),
                "OPENAI_BASE_URL" => Some("https://openai.example".to_string()),
                "OPENAI_MODEL" => Some("gpt-env-model".to_string()),
                _ => None,
            });
        let openai =
            detect_credentials_from_sources("openai", &settings, &claude, |key| match key {
                "OPENAI_API_KEY" => Some("openai-key".to_string()),
                "OPENAI_BASE_URL" => Some("https://openai.example".to_string()),
                "OPENAI_MODEL" => Some("gpt-env-model".to_string()),
                _ => None,
            });

        assert_eq!(custom_openai.api_key, "");
        assert_eq!(custom_openai.api_base, None);
        assert_eq!(custom_openai.model, None);
        assert_eq!(openai.api_key, "openai-key");
        assert_eq!(openai.api_base.as_deref(), Some("https://openai.example"));
        assert_eq!(openai.model.as_deref(), Some("gpt-env-model"));
    }

    #[test]
    fn detect_credentials_key_status_includes_registry_known_providers() {
        let store = MemoryCredentialStore::default();
        let kimi_ref = CredentialRef::provider("kimi");
        store.put(&kimi_ref, "stored-kimi-key").expect("store kimi");
        let settings = Settings {
            credential_refs: HashMap::from([("kimi".to_string(), kimi_ref)]),
            ..Default::default()
        };
        let status = settings.key_status(&store);
        let providers = status
            .iter()
            .map(|entry| entry.provider.as_str())
            .collect::<Vec<_>>();

        for provider in valid_provider_ids() {
            assert!(
                providers.contains(provider),
                "key_status should include {provider}"
            );
        }
        assert!(!providers.contains(&"nvidia"));
        assert!(status.iter().any(|entry| entry.provider == "kimi"
            && entry.configured
            && entry.status == "available"));
    }

    #[test]
    fn detect_credentials_uses_configured_user_provider_profile() {
        let store = MemoryCredentialStore::default();
        let nvidia_ref = CredentialRef::provider("nvidia");
        store
            .put(&nvidia_ref, "stored-nvidia-key")
            .expect("store nvidia");
        let settings = Settings {
            credential_refs: HashMap::from([("nvidia".to_string(), nvidia_ref)]),
            providers: vec![ProviderProfileConfig {
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
            }],
            ..Default::default()
        };
        let claude = ClaudeSettings::default();

        let credentials = detect_credentials_with_stored_key_from_sources(
            "nim",
            &settings,
            &claude,
            Some("stored-nvidia-key".to_string()),
            |key| match key {
                "NVIDIA_API_KEY" => Some("env-nvidia-key".to_string()),
                "NVIDIA_BASE_URL" => Some("https://env.nvidia.example/v1".to_string()),
                _ => None,
            },
        );

        assert_eq!(credentials.api_key, "stored-nvidia-key");
        assert_eq!(
            credentials.api_base.as_deref(),
            Some("https://env.nvidia.example/v1")
        );
        assert_eq!(
            credentials.model.as_deref(),
            Some("nvidia/llama-3.1-nemotron")
        );
        assert!(settings.provider_requires_api_key("nim"));
        assert!(settings
            .key_status(&store)
            .iter()
            .any(|status| status.provider == "nvidia" && status.configured));
    }

    #[test]
    fn provider_requires_api_key_respects_no_auth_profiles() {
        let settings = Settings {
            providers: vec![ProviderProfileConfig {
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
            }],
            ..Default::default()
        };

        let credentials = detect_credentials_from_sources(
            "local-openai",
            &settings,
            &ClaudeSettings::default(),
            |_| None,
        );

        assert_eq!(credentials.api_key, "");
        assert_eq!(
            credentials.api_base.as_deref(),
            Some("http://127.0.0.1:1234/v1")
        );
        assert_eq!(credentials.model.as_deref(), Some("local-model"));
        assert!(!settings.provider_requires_api_key("local-openai"));
        assert!(!settings.provider_requires_api_key("ollama"));
    }

    #[test]
    fn provider_catalog_includes_configured_profiles_for_frontend() {
        let settings = Settings {
            providers: vec![ProviderProfileConfig {
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
            }],
            ..Default::default()
        };

        let catalog = settings.provider_catalog().expect("provider catalog");
        let nvidia = catalog
            .iter()
            .find(|entry| entry.id == "nvidia")
            .expect("nvidia catalog entry");

        assert_eq!(nvidia.label, "NVIDIA NIM");
        assert_eq!(nvidia.default_model, "nvidia/llama-3.1-nemotron");
        assert!(nvidia.requires_api_key);
        assert!(nvidia.supports_streaming);
        assert!(nvidia.supports_tools);
        assert_eq!(nvidia.aliases, vec!["nim".to_string()]);
        assert!(catalog.iter().any(|entry| entry.id == "deepseek"));
    }

    #[test]
    fn provider_catalog_includes_cached_model_catalogs() {
        let mut settings = Settings {
            providers: vec![ProviderProfileConfig {
                id: "nvidia".to_string(),
                label: Some("NVIDIA NIM".to_string()),
                base_url: Some("https://integrate.api.nvidia.com/v1".to_string()),
                api_key_env: Some(EnvVarList::One("NVIDIA_API_KEY".to_string())),
                base_url_env: None,
                default_model: Some("nvidia/llama-3.1-nemotron".to_string()),
                transport: Some("openai_chat_completions".to_string()),
                supports_tools: Some(true),
                supports_streaming: Some(true),
                max_output_tokens_default: None,
                aliases: vec!["nim".to_string()],
            }],
            ..Default::default()
        };
        settings
            .apply_provider_model_catalog(
                "nvidia",
                Some("https://integrate.api.nvidia.com/v1".to_string()),
                ProviderModelCatalogSource::LiveEndpoint,
                vec![
                    ProviderCatalogModel {
                        id: "nvidia/llama-3.1-nemotron".to_string(),
                        name: "NVIDIA Nemotron".to_string(),
                        context_window_tokens: None,
                    },
                    ProviderCatalogModel {
                        id: "nvidia/llama-3.3-70b".to_string(),
                        name: "NVIDIA Llama 3.3 70B".to_string(),
                        context_window_tokens: Some(128_000),
                    },
                ],
            )
            .expect("model catalog cache applies");

        let catalog = settings.provider_catalog().expect("provider catalog");
        let nvidia = catalog
            .iter()
            .find(|entry| entry.id == "nvidia")
            .expect("nvidia catalog entry");

        assert_eq!(nvidia.models.len(), 2);
        assert_eq!(nvidia.models[0].id, "nvidia/llama-3.1-nemotron");
        assert_eq!(nvidia.models[0].name, "NVIDIA Nemotron");
        assert_eq!(nvidia.models[1].context_window_tokens, Some(128_000));
        assert_eq!(
            nvidia.model_catalog_source,
            Some(ProviderModelCatalogSource::LiveEndpoint)
        );
        assert!(
            nvidia
                .model_catalog_recorded_at_ms
                .is_some_and(|value| value > 0),
            "cached model catalog evidence must include when it was recorded"
        );
    }

    #[test]
    fn provider_catalog_includes_cached_probe_evidence() {
        let mut settings = Settings::default();
        settings
            .apply_provider_probe_evidence(
                "openai",
                CachedProviderProbeEvidence {
                    source: ProviderProbeEvidenceSource::ManualProbe,
                    status: ProviderProbeEvidenceStatus::Passed,
                    recorded_at_ms: Some(1_717_891_200_000),
                    model: Some("gpt-4o".to_string()),
                    base_url: Some("https://api.openai.com/v1".to_string()),
                    checks: vec![CachedProviderProbeCheck {
                        id: "tool_schema_accepted".to_string(),
                        label: "Tool schema accepted".to_string(),
                        status: ProviderProbeEvidenceCheckStatus::Passed,
                    }],
                },
            )
            .expect("probe evidence applies");

        let catalog = settings.provider_catalog().expect("provider catalog");
        let openai = catalog
            .iter()
            .find(|entry| entry.id == "openai")
            .expect("openai catalog entry");
        let evidence = openai
            .probe_evidence
            .as_ref()
            .expect("probe evidence is projected");

        assert_eq!(evidence.source, ProviderProbeEvidenceSource::ManualProbe);
        assert_eq!(evidence.status, ProviderProbeEvidenceStatus::Passed);
        assert_eq!(evidence.recorded_at_ms, Some(1_717_891_200_000));
        assert_eq!(evidence.model.as_deref(), Some("gpt-4o"));
        assert_eq!(
            evidence.base_url.as_deref(),
            Some("https://api.openai.com/v1")
        );
        assert_eq!(evidence.checks.len(), 1);
        assert_eq!(
            evidence.checks[0].status,
            ProviderProbeEvidenceCheckStatus::Passed
        );
    }

    #[test]
    fn provider_profile_input_can_be_upserted_and_deleted() {
        let mut settings = Settings::default();
        settings
            .apply_provider_profile_input(ProviderProfileInput {
                id: "local-openai".to_string(),
                label: "Local OpenAI".to_string(),
                transport: "openai_chat_completions".to_string(),
                base_url: Some("http://127.0.0.1:1234/v1".to_string()),
                api_key_env: vec![],
                base_url_env: vec!["LOCAL_OPENAI_BASE_URL".to_string()],
                default_model: "local-model".to_string(),
                aliases: vec!["local-lab".to_string()],
                supports_tools: true,
                supports_streaming: true,
            })
            .expect("profile input applies");

        let catalog = settings.provider_catalog().expect("provider catalog");
        let local = catalog
            .iter()
            .find(|entry| entry.id == "local-openai")
            .expect("local provider catalog entry");
        assert_eq!(local.label, "Local OpenAI");
        assert_eq!(local.source, "user_defined");
        assert_eq!(local.transport, "openai_chat_completions");
        assert_eq!(local.base_url.as_deref(), Some("http://127.0.0.1:1234/v1"));
        assert!(!local.requires_api_key);
        assert_eq!(local.default_model, "local-model");
        assert_eq!(
            local.base_url_env,
            vec!["LOCAL_OPENAI_BASE_URL".to_string()]
        );
        let store = MemoryCredentialStore::default();
        assert!(settings
            .key_status(&store)
            .iter()
            .any(|status| status.provider == "local-openai"));

        settings
            .apply_delete_provider_profile("local-openai")
            .expect("profile deletes");
        let catalog = settings.provider_catalog().expect("provider catalog");
        assert!(!catalog.iter().any(|entry| entry.id == "local-openai"));
    }

    #[test]
    fn mask_key_preserves_only_prefix_and_suffix() {
        assert_eq!(mask_key("short"), "••••");
        assert_eq!(
            mask_key("sk-1394f8913a224de4b8ee29f73d1d8ef5"),
            "sk-1••••8ef5"
        );
    }
}
