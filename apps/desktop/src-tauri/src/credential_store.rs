use crate::adapters::provider_registry::normalize_provider_id;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

#[cfg(test)]
use std::collections::HashMap;

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CredentialRef {
    pub service: String,
    pub account: String,
}

impl CredentialRef {
    pub const SERVICE: &'static str = "com.forge.desktop.provider";

    pub fn provider(provider: &str) -> Self {
        Self {
            service: Self::SERVICE.to_string(),
            account: format!("provider:{}", canonical_provider_account(provider)),
        }
    }

    pub fn profile(profile_id: &str, provider: &str) -> Self {
        Self {
            service: Self::SERVICE.to_string(),
            account: format!(
                "profile:{}:provider:{}",
                profile_id.trim(),
                canonical_provider_account(provider)
            ),
        }
    }
}

impl fmt::Debug for CredentialRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialRef")
            .field("service", &self.service)
            .field("account", &"[redacted-reference]")
            .finish()
    }
}

fn canonical_provider_account(provider: &str) -> String {
    normalize_provider_id(Some(provider.trim()))
        .map(str::to_string)
        .unwrap_or_else(|| provider.trim().to_ascii_lowercase())
}

pub(crate) fn canonical_provider_reference_key(provider: &str) -> String {
    canonical_provider_account(provider)
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CredentialStoreError {
    #[error("credential store unavailable: {category}")]
    Unavailable { category: String },
    #[error("credential store operation failed: {operation}")]
    Backend { operation: &'static str },
}

pub trait CredentialStore: Send + Sync {
    fn put(&self, reference: &CredentialRef, secret: &str) -> Result<(), CredentialStoreError>;
    fn get(&self, reference: &CredentialRef) -> Result<Option<String>, CredentialStoreError>;
    fn delete(&self, reference: &CredentialRef) -> Result<(), CredentialStoreError>;
}

pub struct UnavailableCredentialStore {
    category: String,
}

impl UnavailableCredentialStore {
    pub fn new(category: impl Into<String>) -> Self {
        Self {
            category: category.into(),
        }
    }

    fn error(&self) -> CredentialStoreError {
        CredentialStoreError::Unavailable {
            category: self.category.clone(),
        }
    }
}

impl CredentialStore for UnavailableCredentialStore {
    fn put(&self, _reference: &CredentialRef, _secret: &str) -> Result<(), CredentialStoreError> {
        Err(self.error())
    }

    fn get(&self, _reference: &CredentialRef) -> Result<Option<String>, CredentialStoreError> {
        Err(self.error())
    }

    fn delete(&self, _reference: &CredentialRef) -> Result<(), CredentialStoreError> {
        Err(self.error())
    }
}

#[cfg(target_os = "macos")]
#[derive(Default)]
pub struct KeychainCredentialStore;

#[cfg(target_os = "macos")]
impl KeychainCredentialStore {
    fn entry(reference: &CredentialRef) -> Result<keyring::Entry, CredentialStoreError> {
        keyring::Entry::new(&reference.service, &reference.account).map_err(|_| {
            CredentialStoreError::Backend {
                operation: "open_entry",
            }
        })
    }
}

#[cfg(target_os = "macos")]
impl CredentialStore for KeychainCredentialStore {
    fn put(&self, reference: &CredentialRef, secret: &str) -> Result<(), CredentialStoreError> {
        Self::entry(reference)?
            .set_password(secret)
            .map_err(|_| CredentialStoreError::Backend { operation: "put" })
    }

    fn get(&self, reference: &CredentialRef) -> Result<Option<String>, CredentialStoreError> {
        match Self::entry(reference)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(CredentialStoreError::Backend { operation: "get" }),
        }
    }

    fn delete(&self, reference: &CredentialRef) -> Result<(), CredentialStoreError> {
        match Self::entry(reference)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(CredentialStoreError::Backend {
                operation: "delete",
            }),
        }
    }
}

pub fn system_credential_store() -> Arc<dyn CredentialStore> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(KeychainCredentialStore)
    }

    #[cfg(not(target_os = "macos"))]
    {
        Arc::new(UnavailableCredentialStore::new("unsupported_platform"))
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct MemoryCredentialStore {
    secrets: parking_lot::RwLock<HashMap<CredentialRef, String>>,
}

#[cfg(test)]
impl CredentialStore for MemoryCredentialStore {
    fn put(&self, reference: &CredentialRef, secret: &str) -> Result<(), CredentialStoreError> {
        self.secrets
            .write()
            .insert(reference.clone(), secret.to_string());
        Ok(())
    }

    fn get(&self, reference: &CredentialRef) -> Result<Option<String>, CredentialStoreError> {
        Ok(self.secrets.read().get(reference).cloned())
    }

    fn delete(&self, reference: &CredentialRef) -> Result<(), CredentialStoreError> {
        self.secrets.write().remove(reference);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_create_read_replace_delete() {
        let store = MemoryCredentialStore::default();
        let reference = CredentialRef::provider("openai");

        store.put(&reference, "forge-secret-first").expect("put");
        assert_eq!(
            store.get(&reference).expect("get").as_deref(),
            Some("forge-secret-first")
        );

        store
            .put(&reference, "forge-secret-replaced")
            .expect("replace");
        assert_eq!(
            store.get(&reference).expect("get replaced").as_deref(),
            Some("forge-secret-replaced")
        );

        store.delete(&reference).expect("delete");
        assert_eq!(store.get(&reference).expect("get missing"), None);
    }

    #[test]
    fn credential_ref_uses_canonical_provider_account() {
        assert_eq!(
            CredentialRef::provider("  CLAUDE  ").account,
            "provider:anthropic"
        );
        assert_eq!(
            CredentialRef::profile("work", "moonshot").account,
            "profile:work:provider:kimi"
        );
    }

    #[test]
    fn missing_credential_returns_none() {
        let store = MemoryCredentialStore::default();
        assert_eq!(
            store
                .get(&CredentialRef::provider("deepseek"))
                .expect("missing get"),
            None
        );
    }

    #[test]
    fn delete_missing_credential_is_idempotent() {
        let store = MemoryCredentialStore::default();
        let reference = CredentialRef::provider("deepseek");
        store.delete(&reference).expect("first delete");
        store.delete(&reference).expect("second delete");
    }

    #[test]
    fn unavailable_store_fails_create_read_and_delete() {
        let store = UnavailableCredentialStore::new("unsupported_platform");
        let reference = CredentialRef::provider("openai");

        assert!(matches!(
            store.put(&reference, "forge-secret-9d7f"),
            Err(CredentialStoreError::Unavailable { .. })
        ));
        assert!(matches!(
            store.get(&reference),
            Err(CredentialStoreError::Unavailable { .. })
        ));
        assert!(matches!(
            store.delete(&reference),
            Err(CredentialStoreError::Unavailable { .. })
        ));
    }

    #[test]
    fn credential_ref_debug_never_contains_secret() {
        let reference = CredentialRef::provider("forge-secret-9d7f");
        let rendered = format!("{reference:?}");

        assert!(!rendered.contains("forge-secret-9d7f"));
    }
}
