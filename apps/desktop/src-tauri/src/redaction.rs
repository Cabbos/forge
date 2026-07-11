use regex::{Captures, Regex};
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, LazyLock, RwLock};

const REDACTED: &str = "[redacted]";

static URL_SECRET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(https?://[^\s?#]+)[?#][^\s]*").expect("valid URL redaction regex")
});
static AUTH_HEADER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)(\bauthorization\s*:\s*)(?:bearer\s+)?[^\r\n,;]+")
        .expect("valid authorization redaction regex")
});
static SECRET_ASSIGNMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?im)(\b(?:x-api-key|api[-_ ]?key|access[-_ ]?token|token|secret|password)\s*[:=]\s*)[^\s,;]+",
    )
    .expect("valid secret assignment redaction regex")
});
static BEARER_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\bbearer\s+)[A-Za-z0-9._~+/=-]{8,}")
        .expect("valid bearer token redaction regex")
});
static PREFIXED_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:sk-|nvapi-|xai-|gsk_)[A-Za-z0-9._-]{8,}\b")
        .expect("valid prefixed token redaction regex")
});
static LONG_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Za-z0-9][A-Za-z0-9._-]{31,}\b").expect("valid long token redaction regex")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedactionError;

impl fmt::Display for RedactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("redaction failed")
    }
}

impl std::error::Error for RedactionError {}

pub struct PersistentLogRedactor {
    secrets: RwLock<HashSet<String>>,
    #[cfg(test)]
    fail_for_test: std::sync::atomic::AtomicBool,
}

impl Default for PersistentLogRedactor {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistentLogRedactor {
    pub fn new() -> Self {
        Self {
            secrets: RwLock::new(HashSet::new()),
            #[cfg(test)]
            fail_for_test: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn register_secret(&self, secret: &str) {
        if secret.trim().is_empty() {
            return;
        }
        self.secrets
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .insert(secret.to_string());
    }

    pub fn redact_text(&self, text: &str) -> Result<String, RedactionError> {
        self.check_available()?;

        let mut redacted = text.to_string();
        let secrets = self
            .secrets
            .read()
            .unwrap_or_else(|error| error.into_inner());
        for secret in secrets.iter() {
            redacted = redacted.replace(secret, REDACTED);
        }
        drop(secrets);

        redacted = URL_SECRET_RE.replace_all(&redacted, "$1").into_owned();
        redacted = AUTH_HEADER_RE
            .replace_all(&redacted, |captures: &Captures<'_>| {
                format!("{}{}", &captures[1], REDACTED)
            })
            .into_owned();
        redacted = BEARER_TOKEN_RE
            .replace_all(&redacted, |captures: &Captures<'_>| {
                format!("{}{}", &captures[1], REDACTED)
            })
            .into_owned();
        redacted = SECRET_ASSIGNMENT_RE
            .replace_all(&redacted, |captures: &Captures<'_>| {
                format!("{}{}", &captures[1], REDACTED)
            })
            .into_owned();
        redacted = PREFIXED_TOKEN_RE
            .replace_all(&redacted, REDACTED)
            .into_owned();
        redacted = LONG_TOKEN_RE.replace_all(&redacted, REDACTED).into_owned();
        Ok(redacted)
    }

    pub fn redact_json(&self, value: &Value) -> Result<Value, RedactionError> {
        self.check_available()?;
        self.redact_json_value(value)
    }

    fn redact_json_value(&self, value: &Value) -> Result<Value, RedactionError> {
        match value {
            Value::Object(object) => {
                let mut redacted = serde_json::Map::with_capacity(object.len());
                for (key, value) in object {
                    let value = if is_sensitive_key(key) {
                        Value::String(REDACTED.to_string())
                    } else {
                        self.redact_json_value(value)?
                    };
                    redacted.insert(key.clone(), value);
                }
                Ok(Value::Object(redacted))
            }
            Value::Array(values) => values
                .iter()
                .map(|value| self.redact_json_value(value))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            Value::String(text) => self.redact_text(text).map(Value::String),
            other => Ok(other.clone()),
        }
    }

    fn check_available(&self) -> Result<(), RedactionError> {
        #[cfg(test)]
        if self
            .fail_for_test
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err(RedactionError);
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn set_fail_for_test(&self, fail: bool) {
        self.fail_for_test
            .store(fail, std::sync::atomic::Ordering::Relaxed);
    }
}

fn is_sensitive_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().replace(['-', ' '], "_").as_str(),
        "api_key"
            | "authorization"
            | "environment"
            | "hidden_context"
            | "messages"
            | "password"
            | "request_body"
            | "secret"
            | "system_prompt"
            | "token"
            | "access_token"
            | "x_api_key"
    )
}

static GLOBAL_REDACTOR: LazyLock<Arc<PersistentLogRedactor>> =
    LazyLock::new(|| Arc::new(PersistentLogRedactor::new()));

pub fn global_redactor() -> Arc<PersistentLogRedactor> {
    Arc::clone(&GLOBAL_REDACTOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "forge-secret-9d7f";

    #[test]
    fn redacts_sensitive_headers_and_registered_free_text_secret() {
        let redactor = PersistentLogRedactor::new();
        redactor.register_secret(SECRET);

        for input in [
            format!("Authorization: Bearer {SECRET}"),
            format!("x-api-key: {SECRET}"),
            format!("provider rejected raw credential {SECRET}"),
        ] {
            let redacted = redactor.redact_text(&input).expect("redact text");
            assert!(!redacted.contains(SECRET), "{redacted}");
            assert!(redacted.contains("[redacted]"), "{redacted}");
        }
    }

    #[test]
    fn redacts_sensitive_json_keys_recursively() {
        let redactor = PersistentLogRedactor::new();
        let input = serde_json::json!({
            "api_key": SECRET,
            "nested": {
                "token": SECRET,
                "password": SECRET,
                "request_body": {"safe": "no"},
                "messages": [{"role": "user", "content": SECRET}],
                "system_prompt": SECRET,
                "hidden_context": SECRET,
                "environment": {"FORGE_KEY": SECRET}
            },
            "safe": "visible"
        });

        let redacted = redactor.redact_json(&input).expect("redact json");
        let serialized = serde_json::to_string(&redacted).expect("serialize");
        assert!(!serialized.contains(SECRET), "{serialized}");
        assert_eq!(redacted["api_key"], "[redacted]");
        assert_eq!(redacted["nested"]["messages"], "[redacted]");
        assert_eq!(redacted["safe"], "visible");
    }

    #[test]
    fn drops_url_query_and_fragment_values() {
        let redactor = PersistentLogRedactor::new();
        let input = format!(
            "request failed at https://api.example.test/v1/models?api_key={SECRET}#token={SECRET}"
        );

        let redacted = redactor.redact_text(&input).expect("redact url");
        assert_eq!(
            redacted,
            "request failed at https://api.example.test/v1/models"
        );
    }

    #[test]
    fn redacts_common_unregistered_token_shapes() {
        let redactor = PersistentLogRedactor::new();
        let input = "Bearer abcdefghijklmnop secret=provider-token sk-abcdefghijklmnop ABCDEFGHIJKLMNOPQRSTUVWXYZ123456";

        let redacted = redactor.redact_text(input).expect("redact token shapes");

        assert!(!redacted.contains("abcdefghijklmnop"));
        assert!(!redacted.contains("provider-token"));
        assert!(!redacted.contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ123456"));
    }
}
