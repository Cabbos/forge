use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const KNOWN_PROVIDERS: &[&str] = &["deepseek", "anthropic", "openai", "openrouter"];

/// Persisted user settings stored in ~/.forge/config.json
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
/// 2. Claude Code's ~/.claude/settings.json (apiKey / apiBase / model)
/// 3. ANTHROPIC_AUTH_TOKEN env var (set by Claude Code)
/// 4. ANTHROPIC_API_KEY / OPENAI_API_KEY env vars
/// 5. ANTHROPIC_BASE_URL env var
pub fn detect_credentials(provider: &str) -> Credentials {
    // 1. Check our own stored keys
    let settings = Settings::load();
    let stored_key = settings.get_api_key(provider).map(|s| s.to_string());

    // 2. Try Claude Code config
    let claude_config = read_claude_settings();

    // 3. Check process env vars (may be empty in Tauri GUI)
    let env_auth_token = std::env::var("ANTHROPIC_AUTH_TOKEN").ok();
    let env_anthropic_key = std::env::var("ANTHROPIC_API_KEY").ok();
    let env_deepseek_key = std::env::var("DEEPSEEK_API_KEY").ok();
    let env_openai_key = std::env::var("OPENAI_API_KEY").ok();
    let env_openrouter_key = std::env::var("OPENROUTER_API_KEY").ok();
    let env_anthropic_base_url = std::env::var("ANTHROPIC_BASE_URL").ok();
    let env_deepseek_base_url = std::env::var("DEEPSEEK_BASE_URL").ok();
    let env_openai_base_url = std::env::var("OPENAI_BASE_URL").ok();
    let env_openrouter_base_url = std::env::var("OPENROUTER_BASE_URL").ok();
    let env_model = std::env::var("ANTHROPIC_MODEL").ok();

    let api_key = match provider {
        "anthropic" | "claude" | "hermes" => stored_key
            .or_else(|| claude_config.api_key())
            .or(env_auth_token)
            .or(env_anthropic_key)
            .unwrap_or_default(),
        "openai" | "codex" => stored_key.or(env_openai_key).unwrap_or_default(),
        "openrouter" => stored_key.or(env_openrouter_key).unwrap_or_default(),
        "deepseek" => stored_key.or(env_deepseek_key).unwrap_or_default(),
        _ => stored_key.unwrap_or_default(),
    };

    let api_base = match provider {
        "anthropic" | "claude" | "hermes" => claude_config
            .api_base()
            .map(|s| s.to_string())
            .or(env_anthropic_base_url),
        "deepseek" => env_deepseek_base_url,
        "openai" | "codex" => env_openai_base_url,
        "openrouter" => env_openrouter_base_url,
        _ => None,
    };

    // Model: Claude config (including nested env) first, then process env var
    let model = claude_config.model().or(env_model);

    Credentials {
        api_key,
        api_base,
        model,
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

impl Default for ClaudeSettings {
    fn default() -> Self {
        Self {
            api_key: None,
            api_base: None,
            api_key_camel: None,
            api_base_camel: None,
            model: None,
            env: None,
        }
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
        for p in KNOWN_PROVIDERS {
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

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_keys: HashMap::new(),
        }
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
