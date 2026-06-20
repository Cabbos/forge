use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::adapters::provider_registry::{
    get_provider_definition, normalize_provider_id, valid_provider_ids,
};

/// Persisted user settings stored in ~/.forge/config.json
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
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
    let provider_id = normalize_settings_provider(raw_provider);
    let definition = get_provider_definition(&provider_id);
    let stored_key = stored_api_key(settings, &provider_id, raw_provider).map(str::to_string);

    let registry_api_key = definition.and_then(|definition| {
        first_provider_env_value(&env, &provider_id, definition.api_key_env)
    });
    let registry_base_url = definition.and_then(|definition| {
        first_provider_env_value(&env, &provider_id, definition.base_url_env)
    });

    let api_key = match provider_id.as_str() {
        "anthropic" => stored_key
            .or_else(|| claude_config.api_key())
            .or(registry_api_key)
            .unwrap_or_default(),
        _ => stored_key.or(registry_api_key).unwrap_or_default(),
    };

    let api_base = match provider_id.as_str() {
        "anthropic" => claude_config.api_base().or(registry_base_url),
        _ => registry_base_url,
    };

    let model = match provider_id.as_str() {
        "anthropic" => claude_config
            .model()
            .or_else(|| first_env_value(&env, provider_model_env_vars("anthropic"))),
        _ => first_env_value(&env, provider_model_env_vars(&provider_id)),
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
) -> Option<&'a str> {
    settings
        .get_api_key(provider_id)
        .or_else(|| settings.get_api_key(raw_provider))
        .or_else(|| {
            provider_alias_keys(provider_id)
                .iter()
                .find_map(|alias| settings.get_api_key(alias))
        })
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
        .filter(|key| {
            provider_id == "anthropic"
                || !matches!(
                    key,
                    &"ANTHROPIC_AUTH_TOKEN" | &"ANTHROPIC_API_KEY" | &"ANTHROPIC_BASE_URL"
                ) && (provider_id == "openai"
                    || !matches!(key, &"OPENAI_API_KEY" | &"OPENAI_BASE_URL"))
        })
        .find_map(|key| env(key).filter(|value| !value.trim().is_empty()))
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
        if key.trim().is_empty() {
            self.api_keys.remove(provider);
        } else {
            self.api_keys.insert(provider.to_string(), key.to_string());
        }
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
        // Always include known providers, even if not set
        for p in valid_provider_ids() {
            if !self.api_keys.contains_key(*p) {
                status.push(KeyStatus {
                    provider: p.to_string(),
                    set: false,
                    preview: String::new(),
                });
            }
        }
        status.sort_by(|a, b| a.provider.cmp(&b.provider));
        status
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeyStatus {
    pub provider: String,
    pub set: bool,
    pub preview: String,
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

    use crate::adapters::provider_registry::valid_provider_ids;

    use super::{detect_credentials_from_sources, mask_key, ClaudeSettings, Settings};

    #[test]
    fn detect_credentials_prefers_stored_provider_key_over_claude_and_env() {
        let settings = Settings {
            api_keys: HashMap::from([("anthropic".to_string(), "stored-key".to_string())]),
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
    fn mask_key_preserves_only_prefix_and_suffix() {
        assert_eq!(mask_key("short"), "••••");
        assert_eq!(
            mask_key("sk-1394f8913a224de4b8ee29f73d1d8ef5"),
            "sk-1••••8ef5"
        );
    }
}
