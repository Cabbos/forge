//! User profile store — named profiles with optional provider/model/workspace
//! defaults and API key overrides.
//!
//! Persisted as JSON at `~/.forge/profiles.json`.  Supports create, update,
//! delete, and active-profile selection.  Atomic save: write temp then rename.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Schema ───────────────────────────────────────────────────────────────────

const CURRENT_SCHEMA_VERSION: u32 = 1;

/// A named user profile with optional provider/model/workspace defaults and
/// API key overrides.  `api_key_overrides` is a map of provider → key string
/// for future runtime use; the UI MUST NOT expose raw key values unless
/// masking is implemented.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgeProfile {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_overrides: Option<HashMap<String, String>>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

/// On-disk representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileFile {
    schema_version: u32,
    profiles: Vec<ForgeProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_profile_id: Option<String>,
}

// ── Input / output helpers ───────────────────────────────────────────────────

/// Input for creating or updating a profile via IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertProfileInput {
    /// When present the store updates the existing profile; otherwise creates.
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub default_provider: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub default_workspace: Option<String>,
    /// If present, replaces api_key_overrides entirely.  Omit to leave
    /// existing overrides untouched on update.
    #[serde(default)]
    pub api_key_overrides: Option<HashMap<String, String>>,
}

/// Payload returned by list_profiles IPC so the UI can render profiles and
/// active selection in one call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileListPayload {
    pub profiles: Vec<ForgeProfile>,
    pub active_profile_id: Option<String>,
}

// ── Store ────────────────────────────────────────────────────────────────────

const DEFAULT_PROFILE_ID: &str = "default";

pub struct ProfileStore {
    path: PathBuf,
    profiles: Mutex<Vec<ForgeProfile>>,
    active_profile_id: Mutex<Option<String>>,
    load_error: Mutex<Option<String>>,
}

impl ProfileStore {
    // -- construction ----------------------------------------------------------

    /// Returns the default path `~/.forge/profiles.json`.
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".forge").join("profiles.json")
    }

    /// Creates a store, loading from `path` if it exists.  Seeds a default
    /// profile when the file is missing or contains zero profiles.
    pub fn new(path: PathBuf) -> Self {
        let (profiles, active_profile_id, load_error) = load_profiles(&path);
        let store = Self {
            path,
            profiles: Mutex::new(profiles),
            active_profile_id: Mutex::new(active_profile_id),
            load_error: Mutex::new(load_error),
        };
        // Seed default profile if empty — must happen after construction so
        // the Mutexes are available.
        let is_empty = store
            .profiles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty();
        if is_empty {
            // Capture load error before seed (save() clears it).
            let load_err = store.load_error();
            let _ = store.seed_default();
            // Restore load error so diagnostics can surface corrupt-load.
            if let Some(err) = load_err {
                if let Ok(mut le) = store.load_error.lock() {
                    *le = Some(err);
                }
            }
        }
        store
    }

    /// Returns the last load error (if any) so diagnostics / UI can surface it.
    pub fn load_error(&self) -> Option<String> {
        self.load_error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    // -- queries ---------------------------------------------------------------

    /// List all profiles.
    pub fn list_profiles(&self) -> Vec<ForgeProfile> {
        self.profiles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Returns the active profile id, if set.
    pub fn active_profile_id(&self) -> Option<String> {
        self.active_profile_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Returns the active profile, if set and it exists.
    pub fn get_active_profile(&self) -> Option<ForgeProfile> {
        let active_id = self.active_profile_id();
        let Some(ref id) = active_id else {
            return None;
        };
        let profiles = self.profiles.lock().unwrap_or_else(|e| e.into_inner());
        profiles.iter().find(|p| p.id == *id).cloned()
    }

    /// Builds the combined list payload for IPC.
    pub fn list_payload(&self) -> ProfileListPayload {
        ProfileListPayload {
            profiles: self.list_profiles(),
            active_profile_id: self.active_profile_id(),
        }
    }

    /// Look up a single profile by id.
    pub fn get(&self, id: &str) -> Option<ForgeProfile> {
        let profiles = self.profiles.lock().unwrap_or_else(|e| e.into_inner());
        profiles.iter().find(|p| p.id == id).cloned()
    }

    // -- mutations -------------------------------------------------------------

    /// Create or update a profile.
    ///
    /// - When `input.id` is `Some` and the id exists the existing profile is
    ///   updated.
    /// - When `input.id` is `Some` but the id does not exist a new profile is
    ///   created with that id.
    /// - Otherwise a new profile is created with a fresh UUIDv7 id.
    ///
    /// Name is trimmed; empty name is rejected.  `created_at_ms` is preserved
    /// on update; `updated_at_ms` is always set to now.
    pub fn upsert(&self, input: UpsertProfileInput) -> Result<ForgeProfile, String> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err("Profile name must not be empty.".to_string());
        }

        let def_provider = optional_trimmed(input.default_provider.as_deref());
        let def_model = optional_trimmed(input.default_model.as_deref());
        let def_workspace = optional_trimmed(input.default_workspace.as_deref());

        // Normalize api_key_overrides: trim keys, drop empty values. Omitted
        // overrides preserve existing values on update because the Settings UI
        // does not expose raw keys.
        let has_api_key_overrides_patch = input.api_key_overrides.is_some();
        let api_key_overrides = input.api_key_overrides.map(|m| {
            m.into_iter()
                .filter_map(|(k, v)| {
                    let key = k.trim();
                    let value = v.trim();
                    if key.is_empty() || value.is_empty() {
                        None
                    } else {
                        Some((key.to_string(), value.to_string()))
                    }
                })
                .collect::<HashMap<String, String>>()
        });
        let api_key_overrides = if api_key_overrides.as_ref().is_some_and(|m| m.is_empty()) {
            None
        } else {
            api_key_overrides
        };

        let now_ms = now_millis();

        let mut profiles = self.profiles.lock().unwrap_or_else(|e| e.into_inner());

        if let Some(ref id) = input.id {
            if let Some(existing) = profiles.iter_mut().find(|p| p.id == *id) {
                // Update — preserve created_at_ms.
                existing.name = name;
                existing.default_provider = def_provider;
                existing.default_model = def_model;
                existing.default_workspace = def_workspace;
                if has_api_key_overrides_patch {
                    existing.api_key_overrides = api_key_overrides;
                }
                existing.updated_at_ms = now_ms;

                let profile = existing.clone();
                drop(profiles);
                self.save()?;
                return Ok(profile);
            }
        }

        // Create
        let id = input
            .id
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| new_profile_id());

        let profile = ForgeProfile {
            id,
            name,
            default_provider: def_provider,
            default_model: def_model,
            default_workspace: def_workspace,
            api_key_overrides,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        };
        profiles.push(profile.clone());
        drop(profiles);
        self.save()?;
        Ok(profile)
    }

    /// Delete a profile by id.  Returns `true` if it existed and was removed.
    ///
    /// Rejects deletion of the active profile and the default profile (id ==
    /// "default") to keep at least one profile always available.
    pub fn delete(&self, id: &str) -> Result<bool, String> {
        if id == DEFAULT_PROFILE_ID {
            return Err("Cannot delete the default profile.".to_string());
        }

        {
            let active = self
                .active_profile_id
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if active.as_deref() == Some(id) {
                return Err(
                    "Cannot delete the active profile. Select a different profile first."
                        .to_string(),
                );
            }
        }

        let mut profiles = self.profiles.lock().unwrap_or_else(|e| e.into_inner());
        let len_before = profiles.len();
        profiles.retain(|p| p.id != id);
        let removed = profiles.len() < len_before;
        drop(profiles);
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Set the active profile.
    pub fn set_active(&self, id: &str) -> Result<(), String> {
        // Verify the profile exists.
        {
            let profiles = self.profiles.lock().unwrap_or_else(|e| e.into_inner());
            if !profiles.iter().any(|p| p.id == id) {
                return Err(format!("Profile '{id}' not found."));
            }
        }

        {
            let mut active = self
                .active_profile_id
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *active = Some(id.to_string());
        }
        self.save()?;
        Ok(())
    }

    // -- persistence -----------------------------------------------------------

    /// Seed a default profile.  Called when the store is initialized with an
    /// empty profile list.
    fn seed_default(&self) -> Result<(), String> {
        let now_ms = now_millis();
        let default = ForgeProfile {
            id: DEFAULT_PROFILE_ID.to_string(),
            name: "Default".to_string(),
            default_provider: None,
            default_model: None,
            default_workspace: None,
            api_key_overrides: None,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        };

        {
            let mut profiles = self.profiles.lock().unwrap_or_else(|e| e.into_inner());
            profiles.push(default);
        }

        {
            let mut active = self
                .active_profile_id
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *active = Some(DEFAULT_PROFILE_ID.to_string());
        }

        self.save()?;
        Ok(())
    }

    fn save(&self) -> Result<(), String> {
        let profiles = self.profiles.lock().unwrap_or_else(|e| e.into_inner());
        let active_profile_id = self
            .active_profile_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        let file = ProfileFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            profiles: profiles.clone(),
            active_profile_id,
        };
        let json = serde_json::to_string_pretty(&file).map_err(|e| format!("serialize: {e}"))?;

        // Atomic-ish: write to temp then rename.
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
        }
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, json.as_bytes()).map_err(|e| format!("write temp: {e}"))?;
        fs::rename(&tmp, &self.path).map_err(|e| format!("rename: {e}"))?;

        // Clear any stale load error on successful save.
        if let Ok(mut err) = self.load_error.lock() {
            *err = None;
        }

        Ok(())
    }
}

// ── File I/O ─────────────────────────────────────────────────────────────────

fn load_profiles(path: &PathBuf) -> (Vec<ForgeProfile>, Option<String>, Option<String>) {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return (Vec::new(), None, None);
        }
        Err(e) => return (Vec::new(), None, Some(format!("read error: {e}"))),
    };

    let file: ProfileFile = match serde_json::from_str(&raw) {
        Ok(f) => f,
        Err(e) => {
            return (Vec::new(), None, Some(format!("corrupt JSON: {e}")));
        }
    };

    (file.profiles, file.active_profile_id, None)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn optional_trimmed(s: Option<&str>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn new_profile_id() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("forge-profiles-{name}-{nanos}.json"))
    }

    fn profile_input(name: &str) -> UpsertProfileInput {
        UpsertProfileInput {
            id: None,
            name: name.to_string(),
            default_provider: None,
            default_model: None,
            default_workspace: None,
            api_key_overrides: None,
        }
    }

    fn assert_cleanup(path: &PathBuf) {
        let _ = fs::remove_file(path);
        let tmp = path.with_extension("tmp");
        let _ = fs::remove_file(&tmp);
    }

    // ── Default seed ──────────────────────────────────────────────────────

    #[test]
    fn empty_store_seeds_default_profile() {
        let path = temp_path("seed");
        let store = ProfileStore::new(path.clone());
        let profiles = store.list_profiles();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, "default");
        assert_eq!(profiles[0].name, "Default");
        assert_eq!(store.active_profile_id().as_deref(), Some("default"));

        assert_cleanup(&path);
    }

    #[test]
    fn seed_default_sets_active() {
        let path = temp_path("seed-active");
        let store = ProfileStore::new(path.clone());
        assert_eq!(store.active_profile_id().as_deref(), Some("default"));
        assert_cleanup(&path);
    }

    // ── Create / list ────────────────────────────────────────────────────

    #[test]
    fn create_and_list_profile() {
        let path = temp_path("create-list");
        let store = ProfileStore::new(path.clone());

        let out = store
            .upsert(UpsertProfileInput {
                id: None,
                name: "Work".to_string(),
                default_provider: Some("deepseek".into()),
                default_model: Some("deepseek-chat".into()),
                default_workspace: Some("/home/user/projects".into()),
                api_key_overrides: None,
            })
            .expect("upsert");

        assert_eq!(out.name, "Work");
        assert_eq!(out.default_provider.as_deref(), Some("deepseek"));
        assert_eq!(out.default_model.as_deref(), Some("deepseek-chat"));
        assert_eq!(
            out.default_workspace.as_deref(),
            Some("/home/user/projects")
        );
        assert!(out.created_at_ms > 0);
        assert_eq!(out.created_at_ms, out.updated_at_ms);

        let all = store.list_profiles();
        // default + work
        assert_eq!(all.len(), 2);

        assert_cleanup(&path);
    }

    #[test]
    fn create_rejects_empty_name() {
        let path = temp_path("empty-name");
        let store = ProfileStore::new(path.clone());
        let err = store.upsert(profile_input("  ")).expect_err("empty name");
        assert!(err.contains("not be empty"));
        assert_cleanup(&path);
    }

    #[test]
    fn create_trims_name() {
        let path = temp_path("trim-name");
        let store = ProfileStore::new(path.clone());
        let out = store.upsert(profile_input("  Trimmed  ")).expect("upsert");
        assert_eq!(out.name, "Trimmed");
        assert_cleanup(&path);
    }

    #[test]
    fn create_trims_optional_fields() {
        let path = temp_path("trim-fields");
        let store = ProfileStore::new(path.clone());
        let out = store
            .upsert(UpsertProfileInput {
                id: None,
                name: "T".into(),
                default_provider: Some("  deepseek  ".into()),
                default_model: Some("  ".into()),
                default_workspace: Some("  /tmp  ".into()),
                api_key_overrides: None,
            })
            .expect("upsert");
        assert_eq!(out.default_provider.as_deref(), Some("deepseek"));
        assert_eq!(out.default_model, None); // whitespace-only trimmed to None
        assert_eq!(out.default_workspace.as_deref(), Some("/tmp"));
        assert_cleanup(&path);
    }

    // ── List payload ──────────────────────────────────────────────────────

    #[test]
    fn list_payload_includes_profiles_and_active() {
        let path = temp_path("payload");
        let store = ProfileStore::new(path.clone());

        // Default is active
        let payload = store.list_payload();
        assert_eq!(payload.profiles.len(), 1);
        assert_eq!(payload.active_profile_id.as_deref(), Some("default"));

        assert_cleanup(&path);
    }

    // ── Update ───────────────────────────────────────────────────────────

    #[test]
    fn update_preserves_created_at_and_changes_updated_at() {
        let path = temp_path("update");
        let store = ProfileStore::new(path.clone());
        let out1 = store.upsert(profile_input("v1")).expect("upsert");
        let created = out1.created_at_ms;
        assert_eq!(out1.created_at_ms, out1.updated_at_ms);

        std::thread::sleep(std::time::Duration::from_millis(2));

        let out2 = store
            .upsert(UpsertProfileInput {
                id: Some(out1.id.clone()),
                name: "v2".to_string(),
                default_provider: Some("anthropic".into()),
                default_model: Some("claude-opus-4-8".into()),
                default_workspace: None,
                api_key_overrides: None,
            })
            .expect("upsert");

        assert_eq!(out2.name, "v2");
        assert_eq!(out2.default_provider.as_deref(), Some("anthropic"));
        assert_eq!(out2.created_at_ms, created);
        assert!(
            out2.updated_at_ms > created,
            "updated_at_ms {} should be > created_at_ms {}",
            out2.updated_at_ms,
            created
        );

        assert_cleanup(&path);
    }

    #[test]
    fn update_with_unknown_id_creates_new() {
        let path = temp_path("update-unknown");
        let store = ProfileStore::new(path.clone());
        let out = store
            .upsert(UpsertProfileInput {
                id: Some("my-custom-id".into()),
                name: "Custom".into(),
                default_provider: None,
                default_model: None,
                default_workspace: None,
                api_key_overrides: None,
            })
            .expect("upsert");
        assert_eq!(out.id, "my-custom-id");
        assert_eq!(out.name, "Custom");

        let all = store.list_profiles();
        // default + custom
        assert_eq!(all.len(), 2);

        assert_cleanup(&path);
    }

    // ── Set active ───────────────────────────────────────────────────────

    #[test]
    fn set_active_profile() {
        let path = temp_path("set-active");
        let store = ProfileStore::new(path.clone());

        let p = store.upsert(profile_input("Work")).expect("upsert");
        store.set_active(&p.id).expect("set active");
        assert_eq!(store.active_profile_id().as_deref(), Some(p.id.as_str()));

        let active = store.get_active_profile().expect("active profile");
        assert_eq!(active.id, p.id);
        assert_eq!(active.name, "Work");

        assert_cleanup(&path);
    }

    #[test]
    fn set_active_unknown_profile_errors() {
        let path = temp_path("active-unknown");
        let store = ProfileStore::new(path.clone());
        let err = store.set_active("nonexistent").expect_err("unknown");
        assert!(err.contains("not found"));
        assert_cleanup(&path);
    }

    // ── Delete ───────────────────────────────────────────────────────────

    #[test]
    fn delete_inactive_profile() {
        let path = temp_path("delete-inactive");
        let store = ProfileStore::new(path.clone());
        let p = store.upsert(profile_input("Extra")).expect("upsert");

        let removed = store.delete(&p.id).expect("delete");
        assert!(removed);

        let all = store.list_profiles();
        assert_eq!(all.len(), 1); // only default remains
        assert_eq!(all[0].id, "default");

        assert_cleanup(&path);
    }

    #[test]
    fn delete_nonexistent_returns_false() {
        let path = temp_path("delete-nonexistent");
        let store = ProfileStore::new(path.clone());
        let removed = store.delete("nonexistent").expect("delete");
        assert!(!removed);
        assert_cleanup(&path);
    }

    #[test]
    fn delete_active_profile_rejected() {
        let path = temp_path("delete-active");
        let store = ProfileStore::new(path.clone());
        // Create a non-default profile and make it active.
        let p = store.upsert(profile_input("ActiveOne")).expect("upsert");
        store.set_active(&p.id).expect("set active");
        let err = store.delete(&p.id).expect_err("delete active");
        assert!(err.contains("active profile"));
        assert_cleanup(&path);
    }

    #[test]
    fn delete_default_profile_rejected_even_when_inactive() {
        let path = temp_path("delete-default-inactive");
        let store = ProfileStore::new(path.clone());
        let p = store.upsert(profile_input("Work")).expect("upsert");

        // Switch active to 'Work' so 'default' is inactive
        store.set_active(&p.id).expect("set active to work");

        // Still can't delete 'default' because it's the default seed id
        let err = store.delete("default").expect_err("delete default");
        assert!(err.contains("default profile"));

        assert_cleanup(&path);
    }

    // ── API key overrides ────────────────────────────────────────────────

    #[test]
    fn api_key_overrides_are_stored() {
        let path = temp_path("api-keys");
        let store = ProfileStore::new(path.clone());

        let mut overrides = HashMap::new();
        overrides.insert("deepseek".to_string(), "sk-test-key".to_string());

        let out = store
            .upsert(UpsertProfileInput {
                id: None,
                name: "WithKeys".into(),
                default_provider: None,
                default_model: None,
                default_workspace: None,
                api_key_overrides: Some(overrides),
            })
            .expect("upsert");

        let keys = out.api_key_overrides.expect("api_key_overrides");
        assert_eq!(
            keys.get("deepseek").map(String::as_str),
            Some("sk-test-key")
        );

        assert_cleanup(&path);
    }

    #[test]
    fn api_key_overrides_empty_vals_are_dropped() {
        let path = temp_path("api-keys-empty");
        let store = ProfileStore::new(path.clone());

        let mut overrides = HashMap::new();
        overrides.insert("deepseek".to_string(), "  ".to_string());

        let out = store
            .upsert(UpsertProfileInput {
                id: None,
                name: "NoKeys".into(),
                default_provider: None,
                default_model: None,
                default_workspace: None,
                api_key_overrides: Some(overrides),
            })
            .expect("upsert");

        assert_eq!(out.api_key_overrides, None);

        assert_cleanup(&path);
    }

    #[test]
    fn update_without_api_key_overrides_preserves_existing_keys() {
        let path = temp_path("api-keys-preserve");
        let store = ProfileStore::new(path.clone());

        let mut overrides = HashMap::new();
        overrides.insert(" deepseek ".to_string(), " sk-test-key ".to_string());

        let created = store
            .upsert(UpsertProfileInput {
                id: None,
                name: "WithKeys".into(),
                default_provider: None,
                default_model: None,
                default_workspace: None,
                api_key_overrides: Some(overrides),
            })
            .expect("create");

        let updated = store
            .upsert(UpsertProfileInput {
                id: Some(created.id.clone()),
                name: "Renamed".into(),
                default_provider: Some("deepseek".into()),
                default_model: None,
                default_workspace: None,
                api_key_overrides: None,
            })
            .expect("update");

        let keys = updated.api_key_overrides.expect("keys preserved");
        assert_eq!(
            keys.get("deepseek").map(String::as_str),
            Some("sk-test-key")
        );

        assert_cleanup(&path);
    }

    // ── Persistence ──────────────────────────────────────────────────────

    #[test]
    fn profiles_persist_across_store_reload() {
        let path = temp_path("persist");
        let store1 = ProfileStore::new(path.clone());
        let p = store1.upsert(profile_input("Persisted")).expect("upsert");
        store1.set_active(&p.id).expect("set active");

        let store2 = ProfileStore::new(path.clone());
        let all = store2.list_profiles();
        assert_eq!(all.len(), 2); // default + Persisted
        assert!(all.iter().any(|x| x.name == "Persisted"));
        assert_eq!(store2.active_profile_id().as_deref(), Some(p.id.as_str()));

        assert_cleanup(&path);
    }

    // ── Corrupt JSON ─────────────────────────────────────────────────────

    #[test]
    fn corrupt_json_loads_empty_and_reports_error() {
        let path = temp_path("corrupt");
        fs::write(&path, "not valid json {{{").expect("write corrupt");

        let store = ProfileStore::new(path.clone());
        // Should have seeded a default because corrupt → empty → seed
        let all = store.list_profiles();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "default");
        let err = store.load_error();
        assert!(err.is_some(), "should report load error");
        assert!(err.unwrap().contains("corrupt"));

        assert_cleanup(&path);
    }

    // ── Atomic save ──────────────────────────────────────────────────────

    #[test]
    fn save_does_not_leave_temp_file() {
        let path = temp_path("atomic");
        let store = ProfileStore::new(path.clone());
        store.upsert(profile_input("Atomic")).expect("upsert");

        let tmp = path.with_extension("tmp");
        assert!(!tmp.exists(), "temp file should be gone after rename");

        assert_cleanup(&path);
    }

    #[test]
    fn saved_file_is_valid_json() {
        let path = temp_path("valid-json");
        let store = ProfileStore::new(path.clone());
        store.upsert(profile_input("Json")).expect("upsert");

        let raw = fs::read_to_string(&path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(parsed["schema_version"].as_u64(), Some(1));
        assert_eq!(parsed["profiles"].as_array().unwrap().len(), 2); // default + Json
        assert_eq!(parsed["active_profile_id"].as_str(), Some("default"));

        assert_cleanup(&path);
    }

    #[test]
    fn saved_active_profile_id_roundtrips() {
        let path = temp_path("active-roundtrip");
        let store = ProfileStore::new(path.clone());
        let p = store.upsert(profile_input("Active")).expect("upsert");
        store.set_active(&p.id).expect("set active");

        // Reload
        let store2 = ProfileStore::new(path.clone());
        assert_eq!(store2.active_profile_id().as_deref(), Some(p.id.as_str()));

        assert_cleanup(&path);
    }
}
