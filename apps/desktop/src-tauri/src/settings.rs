use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::adapters::provider_registry::{
    find_loaded_provider_profile, get_provider_definition, load_provider_profiles,
    normalize_provider_id, EnvVarList, LoadedProviderProfile, ProviderProfileConfig,
    ProviderProfileLoadError, ProviderProfileSource, ProviderTransport,
};

/// Persisted user settings stored in ~/.forge/config.json
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
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

/// Detect credentials for a given provider by reading local config files + env vars.
///
/// Priority (highest first):
/// 1. Our stored API key in ~/.forge/config.json
/// 2. Anthropic-only Claude Code config (`apiKey` / `apiBase` / `model`)
/// 3. Registry-defined provider API key and base URL env vars
/// 4. Provider-specific model env vars; `ANTHROPIC_MODEL` only applies to Anthropic
pub fn detect_credentials(provider: &str) -> Credentials {
    // 1. Check our own stored keys
    let settings = Settings::load();

    // 2. Try Claude Code config
    let claude_config = read_claude_settings();

    detect_credentials_from_sources(provider, &settings, &claude_config, |key| {
        std::env::var(key).ok()
    })
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
    let raw_provider = provider.trim();
    let profiles = settings.provider_profiles_or_builtin();
    let loaded_profile = find_loaded_provider_profile(&profiles, raw_provider);
    let provider_id = loaded_profile
        .map(|profile| profile.id.clone())
        .unwrap_or_else(|| normalize_settings_provider(raw_provider));
    let definition = get_provider_definition(&provider_id);
    let loaded_aliases = loaded_profile
        .map(|profile| profile.aliases.as_slice())
        .unwrap_or(&[]);
    let stored_key =
        stored_api_key(settings, &provider_id, raw_provider, loaded_aliases).map(str::to_string);

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

fn stored_api_key<'a>(
    settings: &'a Settings,
    provider_id: &str,
    raw_provider: &str,
    loaded_aliases: &[String],
) -> Option<&'a str> {
    settings
        .get_api_key(provider_id)
        .or_else(|| settings.get_api_key(raw_provider))
        .or_else(|| {
            loaded_aliases
                .iter()
                .find_map(|alias| settings.get_api_key(alias))
        })
        .or_else(|| {
            provider_alias_keys(provider_id)
                .iter()
                .find_map(|alias| settings.get_api_key(alias))
        })
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
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, json).map_err(|e| format!("Failed to write config: {e}"))?;
        Ok(())
    }

    pub fn set_api_key(&mut self, provider: &str, key: &str) -> Result<(), String> {
        let provider_id = provider.trim().to_ascii_lowercase();
        if key.trim().is_empty() {
            self.api_keys.remove(provider);
        } else {
            self.api_keys.insert(provider.to_string(), key.to_string());
        }
        self.provider_probe_evidence.remove(&provider_id);
        self.save()
    }

    pub fn get_api_key(&self, provider: &str) -> Option<&str> {
        self.api_keys.get(provider).map(|s| s.as_str())
    }

    pub fn key_status(&self) -> Vec<KeyStatus> {
        let mut status = Vec::new();
        for (provider, key) in &self.api_keys {
            status.push(KeyStatus {
                provider: provider.clone(),
                set: !key.is_empty(),
                preview: mask_key(key),
            });
        }
        // Always include known and configured providers, even if not set.
        for profile in self.provider_profiles_or_builtin() {
            if !self.api_keys.contains_key(&profile.id) {
                status.push(KeyStatus {
                    provider: profile.id,
                    set: false,
                    preview: String::new(),
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

        load_provider_profiles(&[config.clone()]).map_err(|error| format!("{error:?}"))?;
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
    pub set: bool,
    pub preview: String,
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
    pub models: Vec<ProviderCatalogModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CachedProviderProbeEvidence {
    pub source: ProviderProbeEvidenceSource,
    pub status: ProviderProbeEvidenceStatus,
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::adapters::provider_registry::{
        valid_provider_ids, EnvVarList, ProviderProfileConfig,
    };
    use crate::settings::ProviderProfileInput;

    use super::{
        detect_credentials_from_sources, mask_key, CachedProviderProbeCheck,
        CachedProviderProbeEvidence, ClaudeSettings, ProviderCatalogModel,
        ProviderModelCatalogSource, ProviderProbeEvidenceCheckStatus, ProviderProbeEvidenceSource,
        ProviderProbeEvidenceStatus, Settings,
    };

    #[test]
    fn detect_credentials_prefers_stored_provider_key_over_claude_and_env() {
        let settings = Settings {
            api_keys: HashMap::from([("anthropic".to_string(), "stored-key".to_string())]),
            ..Default::default()
        };
        let claude = ClaudeSettings {
            api_key: Some("claude-key".to_string()),
            api_base: Some("https://claude.example".to_string()),
            model: Some("claude-model".to_string()),
            ..Default::default()
        };

        let credentials =
            detect_credentials_from_sources("anthropic", &settings, &claude, |key| match key {
                "ANTHROPIC_API_KEY" => Some("env-key".to_string()),
                "ANTHROPIC_BASE_URL" => Some("https://env.example".to_string()),
                "ANTHROPIC_MODEL" => Some("env-model".to_string()),
                _ => None,
            });

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
        let settings = Settings {
            api_keys: HashMap::from([("kimi".to_string(), "stored-kimi-key".to_string())]),
            ..Default::default()
        };
        let claude = ClaudeSettings::default();

        let credentials =
            detect_credentials_from_sources("moonshot", &settings, &claude, |key| match key {
                "KIMI_API_KEY" => Some("env-kimi-key".to_string()),
                "MOONSHOT_API_KEY" => Some("env-moonshot-key".to_string()),
                "KIMI_BASE_URL" => Some("https://kimi.example".to_string()),
                _ => None,
            });

        assert_eq!(credentials.api_key, "stored-kimi-key");
        assert_eq!(
            credentials.api_base.as_deref(),
            Some("https://kimi.example")
        );
    }

    #[test]
    fn detect_credentials_falls_back_to_alias_saved_keys_for_canonical_providers() {
        let settings = Settings {
            api_keys: HashMap::from([
                ("moonshot".to_string(), "stored-moonshot-key".to_string()),
                ("qwen".to_string(), "stored-qwen-key".to_string()),
            ]),
            ..Default::default()
        };
        let claude = ClaudeSettings::default();

        let kimi = detect_credentials_from_sources("kimi", &settings, &claude, |key| match key {
            "KIMI_API_KEY" => Some("env-kimi-key".to_string()),
            _ => None,
        });
        let alibaba =
            detect_credentials_from_sources("alibaba", &settings, &claude, |key| match key {
                "ALIBABA_API_KEY" => Some("env-alibaba-key".to_string()),
                _ => None,
            });

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
        let settings = Settings {
            api_keys: HashMap::from([("kimi".to_string(), "stored-kimi-key".to_string())]),
            ..Default::default()
        };
        let status = settings.key_status();
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
        assert!(status
            .iter()
            .any(|entry| entry.provider == "kimi" && entry.set));
    }

    #[test]
    fn detect_credentials_uses_configured_user_provider_profile() {
        let settings = Settings {
            api_keys: HashMap::from([("nvidia".to_string(), "stored-nvidia-key".to_string())]),
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

        let credentials =
            detect_credentials_from_sources("nim", &settings, &claude, |key| match key {
                "NVIDIA_API_KEY" => Some("env-nvidia-key".to_string()),
                "NVIDIA_BASE_URL" => Some("https://env.nvidia.example/v1".to_string()),
                _ => None,
            });

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
            .key_status()
            .iter()
            .any(|status| status.provider == "nvidia" && status.set));
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
        assert!(settings
            .key_status()
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
