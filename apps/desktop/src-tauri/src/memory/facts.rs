//! Local memory facts store — user-managed CRUD for simple tagged facts.
//!
//! Persisted as JSON at `~/.forge/memory.json`.  No embeddings yet;
//! search is plain case-insensitive substring matching over text, tags,
//! profile_id, and source.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Schema ───────────────────────────────────────────────────────────────────

const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryFact {
    pub id: String,
    pub text: String,
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryFactFile {
    schema_version: u32,
    facts: Vec<MemoryFact>,
}

// ── Input / output helpers ───────────────────────────────────────────────────

/// Input for creating or updating a memory fact via IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertMemoryFactInput {
    /// When present the store updates the existing fact; otherwise creates.
    #[serde(default)]
    pub id: Option<String>,
    pub text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// Result payload for a successful upsert so the caller gets the final fact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertMemoryFactOutput {
    pub fact: MemoryFact,
    /// true when an existing fact was updated; false for a new creation.
    pub was_update: bool,
}

// ── Store ────────────────────────────────────────────────────────────────────

pub struct MemoryFactStore {
    path: PathBuf,
    facts: Mutex<Vec<MemoryFact>>,
    /// Human-readable message if the last load failed; cleared on successful save.
    load_error: Mutex<Option<String>>,
}

impl MemoryFactStore {
    // -- construction ----------------------------------------------------------

    /// Creates a store with the default path `~/.forge/memory.json`.
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".forge").join("memory.json")
    }

    pub fn new(path: PathBuf) -> Self {
        let (facts, load_error) = load_facts(&path);
        Self {
            path,
            facts: Mutex::new(facts),
            load_error: Mutex::new(load_error),
        }
    }

    /// Returns the last load error (if any) so diagnostics / UI can surface it.
    pub fn load_error(&self) -> Option<String> {
        self.load_error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    // -- queries ---------------------------------------------------------------

    /// List all facts, optionally filtered by a free-text query.
    ///
    /// An empty / whitespace-only query returns everything.  Matching is
    /// case-insensitive substring search over `text`, `tags`, `profile_id`,
    /// and `source`.
    pub fn list(&self, query: Option<&str>) -> Vec<MemoryFact> {
        let facts = self.facts.lock().unwrap_or_else(|e| e.into_inner());
        let q = normalize_query(query);
        if q.is_empty() {
            return facts.clone();
        }
        facts
            .iter()
            .filter(|f| fact_matches_query(f, &q))
            .cloned()
            .collect()
    }

    /// Look up a single fact by id.
    pub fn get(&self, id: &str) -> Option<MemoryFact> {
        let facts = self.facts.lock().unwrap_or_else(|e| e.into_inner());
        facts.iter().find(|f| f.id == id).cloned()
    }

    // -- mutations -------------------------------------------------------------

    /// Create or update a fact.
    ///
    /// - When `input.id` is `Some` and the id exists the existing fact is updated.
    /// - Otherwise a new fact is created with a fresh UUIDv4 id.
    ///
    /// Text is trimmed; empty text is rejected.  Tags are trimmed, empties
    /// dropped, and duplicates removed.  `created_at_ms` is preserved on
    /// update; `updated_at_ms` is always set to now.
    pub fn upsert(&self, input: UpsertMemoryFactInput) -> Result<UpsertMemoryFactOutput, String> {
        let text = input.text.trim().to_string();
        if text.is_empty() {
            return Err("Memory fact text must not be empty.".to_string());
        }

        let tags = normalize_tags(&input.tags);
        let now_ms = now_millis();

        let mut facts = self.facts.lock().unwrap_or_else(|e| e.into_inner());

        if let Some(ref id) = input.id {
            if let Some(existing) = facts.iter_mut().find(|f| f.id == *id) {
                // Update — created_at_ms is preserved in-place.
                existing.text = text;
                existing.tags = tags;
                existing.profile_id = input.profile_id.filter(|s| !s.trim().is_empty());
                existing.source = input.source.filter(|s| !s.trim().is_empty());
                existing.updated_at_ms = now_ms;

                let fact = existing.clone();
                drop(facts);
                self.save()?;
                return Ok(UpsertMemoryFactOutput {
                    fact,
                    was_update: true,
                });
            }
            // id provided but not found → fall through to create with that id
        }

        // Create
        let id = input
            .id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| new_fact_id());

        let fact = MemoryFact {
            id,
            text,
            tags,
            profile_id: input.profile_id.filter(|s| !s.trim().is_empty()),
            source: input.source.filter(|s| !s.trim().is_empty()),
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        };
        facts.push(fact.clone());
        drop(facts);
        self.save()?;
        Ok(UpsertMemoryFactOutput {
            fact,
            was_update: false,
        })
    }

    /// Delete a fact by id.  Returns `true` if it existed and was removed.
    pub fn delete(&self, id: &str) -> Result<bool, String> {
        let mut facts = self.facts.lock().unwrap_or_else(|e| e.into_inner());
        let len_before = facts.len();
        facts.retain(|f| f.id != id);
        let removed = facts.len() < len_before;
        drop(facts);
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    // -- persistence -----------------------------------------------------------

    fn save(&self) -> Result<(), String> {
        let facts = self.facts.lock().unwrap_or_else(|e| e.into_inner());
        let file = MemoryFactFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            facts: facts.clone(),
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

fn load_facts(path: &PathBuf) -> (Vec<MemoryFact>, Option<String>) {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return (Vec::new(), None);
        }
        Err(e) => return (Vec::new(), Some(format!("read error: {e}"))),
    };

    let file: MemoryFactFile = match serde_json::from_str(&raw) {
        Ok(f) => f,
        Err(e) => return (Vec::new(), Some(format!("corrupt JSON: {e}"))),
    };

    // Future-proof: accept any schema version (we don't reject higher versions).
    // Migration hooks can be added here later.

    (file.facts, None)
}

// ── Query helpers ────────────────────────────────────────────────────────────

fn normalize_query(query: Option<&str>) -> String {
    query.unwrap_or("").trim().to_lowercase()
}

fn fact_matches_query(fact: &MemoryFact, query: &str) -> bool {
    if fact.text.to_lowercase().contains(query) {
        return true;
    }
    if fact.tags.iter().any(|t| t.to_lowercase().contains(query)) {
        return true;
    }
    if let Some(ref pid) = fact.profile_id {
        if pid.to_lowercase().contains(query) {
            return true;
        }
    }
    if let Some(ref src) = fact.source {
        if src.to_lowercase().contains(query) {
            return true;
        }
    }
    false
}

fn normalize_tags(raw: &[String]) -> Vec<String> {
    let mut tags: Vec<String> = raw
        .iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn new_fact_id() -> String {
    // Time-ordered UUID v7 (v7 feature is enabled in Cargo.toml).
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
        std::env::temp_dir().join(format!("forge-memory-facts-{name}-{nanos}.json"))
    }

    fn sample_input(text: &str) -> UpsertMemoryFactInput {
        UpsertMemoryFactInput {
            id: None,
            text: text.to_string(),
            tags: vec![],
            profile_id: None,
            source: None,
        }
    }

    // ── Empty store ──────────────────────────────────────────────────────

    #[test]
    fn empty_store_lists_nothing() {
        let path = temp_path("empty");
        let store = MemoryFactStore::new(path);
        let facts = store.list(None);
        assert!(facts.is_empty());
    }

    #[test]
    fn empty_store_has_no_load_error() {
        let path = temp_path("no-error");
        let store = MemoryFactStore::new(path);
        assert_eq!(store.load_error(), None);
    }

    // ── Create / list ────────────────────────────────────────────────────

    #[test]
    fn create_and_list() {
        let path = temp_path("create-list");
        let store = MemoryFactStore::new(path);
        let out = store
            .upsert(UpsertMemoryFactInput {
                id: None,
                text: "hello world".to_string(),
                tags: vec!["rust".into(), "memory".into()],
                profile_id: Some("default".into()),
                source: Some("user".into()),
            })
            .expect("upsert");
        assert!(!out.was_update);
        assert_eq!(out.fact.text, "hello world");
        assert_eq!(out.fact.tags, vec!["memory", "rust"]);
        assert_eq!(out.fact.profile_id.as_deref(), Some("default"));
        assert_eq!(out.fact.source.as_deref(), Some("user"));
        assert!(out.fact.created_at_ms > 0);
        assert_eq!(out.fact.created_at_ms, out.fact.updated_at_ms);

        let all = store.list(None);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, out.fact.id);
    }

    #[test]
    fn create_rejects_empty_text() {
        let path = temp_path("empty-text");
        let store = MemoryFactStore::new(path);
        let err = store.upsert(sample_input("  ")).expect_err("empty");
        assert!(err.contains("not be empty"));
    }

    #[test]
    fn create_trims_text() {
        let path = temp_path("trim-text");
        let store = MemoryFactStore::new(path);
        let out = store.upsert(sample_input("  trimmed  ")).expect("upsert");
        assert_eq!(out.fact.text, "trimmed");
    }

    // ── Tags ─────────────────────────────────────────────────────────────

    #[test]
    fn tags_are_trimmed_and_deduped() {
        let path = temp_path("tags");
        let store = MemoryFactStore::new(path);
        let out = store
            .upsert(UpsertMemoryFactInput {
                id: None,
                text: "tag test".into(),
                tags: vec![
                    "  Rust  ".into(),
                    "rust".into(),
                    "".into(),
                    "CLI".into(),
                    "cli".into(),
                ],
                profile_id: None,
                source: None,
            })
            .expect("upsert");
        // Deduped (case-sensitive) + sorted: "CLI", "Rust", "cli", "rust"
        // Wait — dedup in normalize_tags uses sort + dedup.
        // "  Rust  " → "Rust", "rust" → "rust", "" → dropped, "CLI" → "CLI", "cli" → "cli"
        // After sort: ["CLI", "Rust", "cli", "rust"]
        // After dedup: ["CLI", "Rust", "cli", "rust"]
        // Since dedup only removes consecutive duplicates, "rust" and "Rust" are different.
        assert_eq!(out.fact.tags, vec!["CLI", "Rust", "cli", "rust"]);
        // Verify no empty strings
        assert!(out.fact.tags.iter().all(|t| !t.is_empty()));
    }

    // ── Search ───────────────────────────────────────────────────────────

    #[test]
    fn search_matches_text_case_insensitive() {
        let path = temp_path("search-text");
        let store = MemoryFactStore::new(path);
        store.upsert(sample_input("Hello World")).expect("upsert");
        store.upsert(sample_input("Goodbye")).expect("upsert");

        let results = store.list(Some("hello"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Hello World");
    }

    #[test]
    fn search_matches_tags() {
        let path = temp_path("search-tags");
        let store = MemoryFactStore::new(path);
        store
            .upsert(UpsertMemoryFactInput {
                id: None,
                text: "some fact".into(),
                tags: vec!["Rust".into(), "CLI".into()],
                profile_id: None,
                source: None,
            })
            .expect("upsert");

        let results = store.list(Some("rust"));
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_matches_profile_id() {
        let path = temp_path("search-profile");
        let store = MemoryFactStore::new(path);
        store
            .upsert(UpsertMemoryFactInput {
                id: None,
                text: "some fact".into(),
                tags: vec![],
                profile_id: Some("work-profile".into()),
                source: None,
            })
            .expect("upsert");

        let results = store.list(Some("work"));
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_matches_source() {
        let path = temp_path("search-source");
        let store = MemoryFactStore::new(path);
        store
            .upsert(UpsertMemoryFactInput {
                id: None,
                text: "some fact".into(),
                tags: vec![],
                profile_id: None,
                source: Some("cli-importer".into()),
            })
            .expect("upsert");

        let results = store.list(Some("import"));
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_empty_query_returns_all() {
        let path = temp_path("search-empty");
        let store = MemoryFactStore::new(path);
        store.upsert(sample_input("a")).expect("upsert");
        store.upsert(sample_input("b")).expect("upsert");

        assert_eq!(store.list(None).len(), 2);
        assert_eq!(store.list(Some("")).len(), 2);
        assert_eq!(store.list(Some("  ")).len(), 2);
    }

    // ── Update ───────────────────────────────────────────────────────────

    #[test]
    fn update_preserves_created_at_and_changes_updated_at() {
        let path = temp_path("update");
        let store = MemoryFactStore::new(path);
        let out1 = store.upsert(sample_input("v1")).expect("upsert");
        let created = out1.fact.created_at_ms;
        assert_eq!(out1.fact.created_at_ms, out1.fact.updated_at_ms);

        // Small sleep so timestamps differ on fast machines
        std::thread::sleep(std::time::Duration::from_millis(2));

        let out2 = store
            .upsert(UpsertMemoryFactInput {
                id: Some(out1.fact.id.clone()),
                text: "v2".into(),
                tags: vec![],
                profile_id: None,
                source: None,
            })
            .expect("upsert");
        assert!(out2.was_update);
        assert_eq!(out2.fact.text, "v2");
        assert_eq!(out2.fact.created_at_ms, created);
        assert!(
            out2.fact.updated_at_ms > created,
            "updated_at_ms {updated} should be > created_at_ms {created}",
            updated = out2.fact.updated_at_ms
        );
    }

    #[test]
    fn update_with_unknown_id_creates_new() {
        let path = temp_path("update-unknown");
        let store = MemoryFactStore::new(path);
        let out = store
            .upsert(UpsertMemoryFactInput {
                id: Some("nonexistent".into()),
                text: "new".into(),
                tags: vec![],
                profile_id: None,
                source: None,
            })
            .expect("upsert");
        assert!(!out.was_update);
        assert_eq!(out.fact.id, "nonexistent");
    }

    // ── Delete ───────────────────────────────────────────────────────────

    #[test]
    fn delete_removes_fact() {
        let path = temp_path("delete");
        let store = MemoryFactStore::new(path);
        let out = store.upsert(sample_input("to-delete")).expect("upsert");

        let removed = store.delete(&out.fact.id).expect("delete");
        assert!(removed);

        let all = store.list(None);
        assert!(all.is_empty());
    }

    #[test]
    fn delete_unknown_id_returns_false() {
        let path = temp_path("delete-unknown");
        let store = MemoryFactStore::new(path);
        let removed = store.delete("nonexistent").expect("delete");
        assert!(!removed);
    }

    // ── Persistence ──────────────────────────────────────────────────────

    #[test]
    fn facts_persist_across_store_reload() {
        let path = temp_path("persist");
        let store1 = MemoryFactStore::new(path.clone());
        store1.upsert(sample_input("persisted")).expect("upsert");

        let store2 = MemoryFactStore::new(path.clone());
        let all = store2.list(None);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].text, "persisted");

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    // ── Corrupt JSON ─────────────────────────────────────────────────────

    #[test]
    fn corrupt_json_loads_empty_and_reports_error() {
        let path = temp_path("corrupt");
        fs::write(&path, "not valid json {{{").expect("write corrupt");

        let store = MemoryFactStore::new(path.clone());
        assert!(store.list(None).is_empty());
        let err = store.load_error();
        assert!(err.is_some(), "should report load error");
        assert!(err.unwrap().contains("corrupt"));

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    // ── Atomic save leaves valid file ────────────────────────────────────

    #[test]
    fn save_does_not_leave_temp_file() {
        let path = temp_path("atomic");
        let store = MemoryFactStore::new(path.clone());
        store.upsert(sample_input("atomic")).expect("upsert");

        let tmp = path.with_extension("tmp");
        assert!(!tmp.exists(), "temp file should be gone after rename");

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn saved_file_is_valid_json() {
        let path = temp_path("valid-json");
        let store = MemoryFactStore::new(path.clone());
        store.upsert(sample_input("json")).expect("upsert");

        let raw = fs::read_to_string(&path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(parsed["schema_version"].as_u64(), Some(1));
        assert_eq!(parsed["facts"].as_array().unwrap().len(), 1);

        // Cleanup
        let _ = fs::remove_file(&path);
    }
}
