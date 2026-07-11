use crate::credential_store::{canonical_provider_reference_key, CredentialRef, CredentialStore};
use serde_json::{Map, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CredentialMigrationPaths {
    pub settings: PathBuf,
    pub profiles: PathBuf,
}

impl CredentialMigrationPaths {
    pub fn default_paths() -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let forge = home.join(".forge");
        Self {
            settings: forge.join("config.json"),
            profiles: forge.join("profiles.json"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("credential migration failed: {stage}")]
pub struct CredentialMigrationError {
    stage: &'static str,
}

impl CredentialMigrationError {
    fn at(stage: &'static str) -> Self {
        Self { stage }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CredentialMigrationReport {
    pub settings_migrated: bool,
    pub profiles_migrated: bool,
}

pub fn migrate_legacy_credentials(
    paths: &CredentialMigrationPaths,
    store: &dyn CredentialStore,
) -> Result<CredentialMigrationReport, CredentialMigrationError> {
    let settings_migrated = migrate_settings_file(&paths.settings, store)?;
    let profiles_migrated = migrate_profiles_file(&paths.profiles, store)?;
    Ok(CredentialMigrationReport {
        settings_migrated,
        profiles_migrated,
    })
}

fn migrate_settings_file(
    path: &Path,
    store: &dyn CredentialStore,
) -> Result<bool, CredentialMigrationError> {
    migrate_file(
        path,
        |document| {
            let Some(object) = document.as_object_mut() else {
                return Err(CredentialMigrationError::at("parse settings"));
            };
            let Some(legacy) = object.remove("api_keys") else {
                return Ok(None);
            };
            let legacy = legacy
                .as_object()
                .ok_or_else(|| CredentialMigrationError::at("parse settings"))?;
            let mut credentials = Vec::new();
            let references = reference_object(object, "credential_refs")?;
            for (provider, value) in legacy {
                let Some(secret) = value.as_str().filter(|secret| !secret.trim().is_empty()) else {
                    continue;
                };
                let reference = CredentialRef::provider(provider);
                references.insert(
                    canonical_provider_reference_key(provider),
                    serde_json::to_value(&reference)
                        .map_err(|_| CredentialMigrationError::at("serialize reference"))?,
                );
                credentials.push((reference, secret.to_string()));
            }
            Ok(Some(credentials))
        },
        store,
    )
}

fn migrate_profiles_file(
    path: &Path,
    store: &dyn CredentialStore,
) -> Result<bool, CredentialMigrationError> {
    migrate_file(
        path,
        |document| {
            let Some(object) = document.as_object_mut() else {
                return Err(CredentialMigrationError::at("parse profiles"));
            };
            let profiles = object
                .get_mut("profiles")
                .and_then(Value::as_array_mut)
                .ok_or_else(|| CredentialMigrationError::at("parse profiles"))?;
            let mut found_legacy = false;
            let mut credentials = Vec::new();
            for profile in profiles {
                let Some(profile) = profile.as_object_mut() else {
                    return Err(CredentialMigrationError::at("parse profiles"));
                };
                let Some(legacy) = profile.remove("api_key_overrides") else {
                    continue;
                };
                found_legacy = true;
                let profile_id = profile
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| CredentialMigrationError::at("parse profiles"))?
                    .to_string();
                let legacy = legacy
                    .as_object()
                    .ok_or_else(|| CredentialMigrationError::at("parse profiles"))?;
                let references = reference_object(profile, "credential_overrides")?;
                for (provider, value) in legacy {
                    let Some(secret) = value.as_str().filter(|secret| !secret.trim().is_empty())
                    else {
                        continue;
                    };
                    let reference = CredentialRef::profile(&profile_id, provider);
                    references.insert(
                        canonical_provider_reference_key(provider),
                        serde_json::to_value(&reference)
                            .map_err(|_| CredentialMigrationError::at("serialize reference"))?,
                    );
                    credentials.push((reference, secret.to_string()));
                }
            }
            Ok(found_legacy.then_some(credentials))
        },
        store,
    )
}

fn reference_object<'a>(
    parent: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>, CredentialMigrationError> {
    let value = parent
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    value
        .as_object_mut()
        .ok_or_else(|| CredentialMigrationError::at("parse references"))
}

fn migrate_file<F>(
    path: &Path,
    transform: F,
    store: &dyn CredentialStore,
) -> Result<bool, CredentialMigrationError>
where
    F: FnOnce(&mut Value) -> Result<Option<Vec<(CredentialRef, String)>>, CredentialMigrationError>,
{
    let original = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(CredentialMigrationError::at("read source")),
    };
    let mut document: Value = serde_json::from_slice(&original)
        .map_err(|_| CredentialMigrationError::at("parse source"))?;
    let Some(credentials) = transform(&mut document)? else {
        return Ok(false);
    };

    for (reference, secret) in &credentials {
        store
            .put(reference, secret)
            .map_err(|_| CredentialMigrationError::at("store write"))?;
    }
    for (reference, expected) in &credentials {
        let actual = store
            .get(reference)
            .map_err(|_| CredentialMigrationError::at("store read"))?;
        if actual.as_deref() != Some(expected.as_str()) {
            return Err(CredentialMigrationError::at("readback mismatch"));
        }
    }

    let migrated = serde_json::to_vec_pretty(&document)
        .map_err(|_| CredentialMigrationError::at("serialize document"))?;
    atomic_replace(path, &migrated)?;

    let verified = fs::read(path).map_err(|_| CredentialMigrationError::at("verify read"))?;
    let verified_document: Value = serde_json::from_slice(&verified)
        .map_err(|_| CredentialMigrationError::at("verify parse"))?;
    let contains_secret = credentials
        .iter()
        .any(|(_, secret)| value_contains_text(&verified_document, secret));
    if contains_legacy_secret_key(&verified_document) || contains_secret {
        atomic_replace(path, &original)?;
        return Err(CredentialMigrationError::at("verify plaintext removal"));
    }

    let redactor = crate::redaction::global_redactor();
    for (_, secret) in credentials {
        redactor.register_secret(&secret);
    }
    Ok(true)
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), CredentialMigrationError> {
    let metadata =
        fs::metadata(path).map_err(|_| CredentialMigrationError::at("source metadata"))?;
    let temporary = path.with_extension("credential-migration.tmp");
    let result = (|| {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| CredentialMigrationError::at("write temp"))?;
        file.set_permissions(metadata.permissions())
            .map_err(|_| CredentialMigrationError::at("temp permissions"))?;
        file.write_all(bytes)
            .map_err(|_| CredentialMigrationError::at("write temp"))?;
        file.sync_all()
            .map_err(|_| CredentialMigrationError::at("sync temp"))?;
        drop(file);
        fs::rename(&temporary, path).map_err(|_| CredentialMigrationError::at("rename"))?;
        if let Some(parent) = path.parent() {
            if let Ok(directory) = fs::File::open(parent) {
                directory
                    .sync_all()
                    .map_err(|_| CredentialMigrationError::at("sync directory"))?;
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn contains_legacy_secret_key(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            matches!(key.as_str(), "api_keys" | "api_key_overrides")
                || contains_legacy_secret_key(value)
        }),
        Value::Array(values) => values.iter().any(contains_legacy_secret_key),
        _ => false,
    }
}

fn value_contains_text(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => text.contains(needle),
        Value::Array(values) => values
            .iter()
            .any(|value| value_contains_text(value, needle)),
        Value::Object(object) => object
            .values()
            .any(|value| value_contains_text(value, needle)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credential_store::{
        CredentialRef, CredentialStore, CredentialStoreError, MemoryCredentialStore,
    };
    use serde_json::{json, Value};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_paths(name: &str) -> CredentialMigrationPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("forge-credential-migration-{name}-{nanos}"));
        CredentialMigrationPaths {
            settings: root.join("config.json"),
            profiles: root.join("profiles.json"),
        }
    }

    fn write_json(path: &PathBuf, value: &Value) -> Vec<u8> {
        fs::create_dir_all(path.parent().expect("parent")).expect("create fixture dir");
        let bytes = serde_json::to_vec_pretty(value).expect("serialize fixture");
        fs::write(path, &bytes).expect("write fixture");
        bytes
    }

    fn read_json(path: &PathBuf) -> Value {
        serde_json::from_slice(&fs::read(path).expect("read migrated file"))
            .expect("parse migrated file")
    }

    fn cleanup(paths: &CredentialMigrationPaths) {
        if let Some(root) = paths.settings.parent() {
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn legacy_settings_keys_migrate_to_verified_references_without_plaintext() {
        let paths = temp_paths("settings");
        write_json(
            &paths.settings,
            &json!({
                "api_keys": {
                    "openai": "forge-settings-secret",
                    "moonshot": "forge-alias-secret"
                },
                "providers": []
            }),
        );
        let store = MemoryCredentialStore::default();

        migrate_legacy_credentials(&paths, &store).expect("migrate settings");

        let migrated = read_json(&paths.settings);
        assert!(migrated.get("api_keys").is_none());
        assert_eq!(
            migrated.pointer("/credential_refs/openai/account"),
            Some(&json!("provider:openai"))
        );
        assert_eq!(
            migrated.pointer("/credential_refs/kimi/account"),
            Some(&json!("provider:kimi"))
        );
        assert!(migrated.pointer("/credential_refs/moonshot").is_none());
        assert_eq!(
            store
                .get(&CredentialRef::provider("openai"))
                .expect("read credential")
                .as_deref(),
            Some("forge-settings-secret")
        );
        assert!(!fs::read_to_string(&paths.settings)
            .expect("settings text")
            .contains("forge-settings-secret"));
        assert!(!fs::read_to_string(&paths.settings)
            .expect("settings text")
            .contains("forge-alias-secret"));
        cleanup(&paths);
    }

    #[test]
    fn legacy_profile_overrides_migrate_to_verified_references_without_plaintext() {
        let paths = temp_paths("profiles");
        write_json(
            &paths.profiles,
            &json!({
                "schema_version": 1,
                "profiles": [{
                    "id": "work",
                    "name": "Work",
                    "api_key_overrides": {"anthropic": "forge-profile-secret"},
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                }]
            }),
        );
        let store = MemoryCredentialStore::default();

        migrate_legacy_credentials(&paths, &store).expect("migrate profiles");

        let migrated = read_json(&paths.profiles);
        assert!(migrated.pointer("/profiles/0/api_key_overrides").is_none());
        assert_eq!(
            migrated.pointer("/profiles/0/credential_overrides/anthropic/account"),
            Some(&json!("profile:work:provider:anthropic"))
        );
        assert_eq!(
            store
                .get(&CredentialRef::profile("work", "anthropic"))
                .expect("read credential")
                .as_deref(),
            Some("forge-profile-secret")
        );
        assert!(!fs::read_to_string(&paths.profiles)
            .expect("profiles text")
            .contains("forge-profile-secret"));
        cleanup(&paths);
    }

    #[test]
    fn migration_is_idempotent_after_partial_file_completion() {
        let paths = temp_paths("partial");
        let settings = json!({
            "credential_refs": {
                "openai": {"service": CredentialRef::SERVICE, "account": "provider:openai"}
            }
        });
        let settings_bytes = write_json(&paths.settings, &settings);
        write_json(
            &paths.profiles,
            &json!({
                "schema_version": 1,
                "profiles": [{
                    "id": "work",
                    "name": "Work",
                    "api_key_overrides": {"openai": "forge-partial-secret"},
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                }]
            }),
        );
        let store = MemoryCredentialStore::default();

        migrate_legacy_credentials(&paths, &store).expect("resume partial migration");
        migrate_legacy_credentials(&paths, &store).expect("repeat completed migration");

        assert_eq!(
            fs::read(&paths.settings).expect("settings bytes"),
            settings_bytes
        );
        assert!(read_json(&paths.profiles)
            .pointer("/profiles/0/api_key_overrides")
            .is_none());
        cleanup(&paths);
    }

    #[derive(Default)]
    struct FailingPutStore;

    impl CredentialStore for FailingPutStore {
        fn put(
            &self,
            _reference: &CredentialRef,
            _secret: &str,
        ) -> Result<(), CredentialStoreError> {
            Err(CredentialStoreError::Backend { operation: "put" })
        }

        fn get(&self, _reference: &CredentialRef) -> Result<Option<String>, CredentialStoreError> {
            Ok(None)
        }

        fn delete(&self, _reference: &CredentialRef) -> Result<(), CredentialStoreError> {
            Ok(())
        }
    }

    #[test]
    fn store_write_failure_leaves_original_json_byte_for_byte() {
        let paths = temp_paths("put-failure");
        let original = write_json(
            &paths.settings,
            &json!({"api_keys": {"openai": "forge-write-failure-secret"}}),
        );

        let error = migrate_legacy_credentials(&paths, &FailingPutStore)
            .expect_err("store write must fail migration");

        assert_eq!(
            error.to_string(),
            "credential migration failed: store write"
        );
        assert_eq!(fs::read(&paths.settings).expect("settings bytes"), original);
        cleanup(&paths);
    }

    #[derive(Default)]
    struct MismatchStore {
        values: Mutex<Vec<(CredentialRef, String)>>,
    }

    impl CredentialStore for MismatchStore {
        fn put(&self, reference: &CredentialRef, secret: &str) -> Result<(), CredentialStoreError> {
            self.values
                .lock()
                .expect("values")
                .push((reference.clone(), secret.to_string()));
            Ok(())
        }

        fn get(&self, _reference: &CredentialRef) -> Result<Option<String>, CredentialStoreError> {
            Ok(Some("different-secret".to_string()))
        }

        fn delete(&self, _reference: &CredentialRef) -> Result<(), CredentialStoreError> {
            Ok(())
        }
    }

    #[test]
    fn store_readback_mismatch_leaves_original_json_byte_for_byte() {
        let paths = temp_paths("readback-mismatch");
        let original = write_json(
            &paths.profiles,
            &json!({
                "schema_version": 1,
                "profiles": [{
                    "id": "work",
                    "name": "Work",
                    "api_key_overrides": {"openai": "forge-readback-secret"},
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                }]
            }),
        );

        let error = migrate_legacy_credentials(&paths, &MismatchStore::default())
            .expect_err("readback mismatch must fail migration");

        assert_eq!(
            error.to_string(),
            "credential migration failed: readback mismatch"
        );
        assert_eq!(fs::read(&paths.profiles).expect("profile bytes"), original);
        cleanup(&paths);
    }
}
