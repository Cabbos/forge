use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::memory::model::{
    MemoryListFilter, MemoryPatch, MemoryScope, MemoryStatus, SelectedContextMemory, WikiMemory,
};
use crate::memory::scoring::select_relevant_memories;

pub struct WikiMemoryStore {
    pub path: PathBuf,
    pub memories: RwLock<Vec<WikiMemory>>,
    mutation_lock: Mutex<()>,
    load_error: Option<String>,
}

impl WikiMemoryStore {
    pub fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self::new(PathBuf::from(home).join(".forge/wiki_memories.json"))
    }

    pub fn new(path: PathBuf) -> Self {
        let (memories, load_error) = load_memories(&path);

        Self {
            path,
            memories: RwLock::new(memories),
            mutation_lock: Mutex::new(()),
            load_error,
        }
    }

    pub async fn list(&self, filter: MemoryListFilter) -> Vec<WikiMemory> {
        let memories = self.memories.read().unwrap_or_else(|err| err.into_inner());
        memories
            .iter()
            .filter(|memory| {
                memory.status != MemoryStatus::Forgotten && memory.status != MemoryStatus::Archived
            })
            .filter(|memory| {
                filter
                    .scope
                    .as_ref()
                    .is_none_or(|scope| &memory.scope == scope)
            })
            .filter(|memory| matches_project_filter(memory, filter.project_path.as_deref()))
            .cloned()
            .collect()
    }

    pub async fn upsert_candidate(
        &self,
        candidate: WikiMemory,
    ) -> Result<Option<WikiMemory>, String> {
        let _mutation = self
            .mutation_lock
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let memories = self.memories.read().unwrap_or_else(|err| err.into_inner());
        let mut next_memories = memories.clone();
        drop(memories);

        if next_memories.iter().any(|memory| {
            memory.status == MemoryStatus::Forgotten
                && memory.category == candidate.category
                && memory.scope == candidate.scope
                && same_project_path(
                    memory.project_path.as_deref(),
                    candidate.project_path.as_deref(),
                )
                && memory.title == candidate.title
                && memory.body == candidate.body
        }) {
            return Ok(None);
        }

        let result = if let Some(existing) = next_memories.iter_mut().find(|memory| {
            memory.status == MemoryStatus::Candidate
                && memory.category == candidate.category
                && memory.scope == candidate.scope
                && same_project_path(
                    memory.project_path.as_deref(),
                    candidate.project_path.as_deref(),
                )
                && memory.title == candidate.title
        }) {
            existing.body = candidate.body;
            existing.confidence = existing.confidence.max(candidate.confidence);
            existing.updated_at = now_string();
            Some(existing.clone())
        } else {
            next_memories.push(candidate.clone());
            Some(candidate)
        };

        self.save_and_replace(next_memories)?;
        Ok(result)
    }

    pub async fn update(&self, memory_id: &str, patch: MemoryPatch) -> Result<WikiMemory, String> {
        let _mutation = self
            .mutation_lock
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let memories = self.memories.read().unwrap_or_else(|err| err.into_inner());
        let mut next_memories = memories.clone();
        drop(memories);

        let memory = next_memories
            .iter_mut()
            .find(|memory| memory.id == memory_id)
            .ok_or_else(|| format!("Memory not found: {memory_id}"))?;

        if let Some(title) = patch.title {
            memory.title = title;
        }
        if let Some(body) = patch.body {
            memory.body = body;
        }
        if let Some(status) = patch.status {
            memory.status = status;
        }
        if let Some(tags) = patch.tags {
            memory.tags = tags;
        }
        memory.updated_at = now_string();

        let updated = memory.clone();
        self.save_and_replace(next_memories)?;
        Ok(updated)
    }

    pub async fn pin(&self, memory_id: &str) -> Result<WikiMemory, String> {
        self.update(
            memory_id,
            MemoryPatch {
                status: Some(MemoryStatus::Pinned),
                ..MemoryPatch::default()
            },
        )
        .await
    }

    pub async fn forget(&self, memory_id: &str) -> Result<WikiMemory, String> {
        self.update(
            memory_id,
            MemoryPatch {
                status: Some(MemoryStatus::Forgotten),
                ..MemoryPatch::default()
            },
        )
        .await
    }

    pub async fn select(
        &self,
        message: &str,
        project_path: Option<&str>,
        limit: usize,
    ) -> Vec<SelectedContextMemory> {
        let mut candidates = self
            .list(MemoryListFilter {
                scope: None,
                project_path: project_path.map(str::to_string),
            })
            .await;
        if project_path.is_none() {
            candidates.retain(|memory| {
                memory.project_path.is_none() && memory.scope != MemoryScope::Project
            });
        }

        let selected = select_relevant_memories(&candidates, message, project_path, limit);
        if selected.is_empty() {
            return selected;
        }

        if let Err(err) = self.record_usage(&selected).await {
            log::warn!("Failed to save wiki memory usage: {err}");
        }

        selected
    }

    async fn record_usage(&self, selected: &[SelectedContextMemory]) -> Result<(), String> {
        let _mutation = self
            .mutation_lock
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let selected_ids = selected
            .iter()
            .map(|memory| memory.memory_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let now = now_string();
        let memories = self.memories.read().unwrap_or_else(|err| err.into_inner());
        let mut next_memories = memories.clone();
        drop(memories);

        for memory in next_memories.iter_mut() {
            if selected_ids.contains(memory.id.as_str()) {
                memory.use_count = memory.use_count.saturating_add(1);
                memory.last_used_at = Some(now.clone());
                memory.updated_at = now.clone();
            }
        }

        self.save_and_replace(next_memories)
    }

    fn save_and_replace(&self, next_memories: Vec<WikiMemory>) -> Result<(), String> {
        if let Some(load_error) = &self.load_error {
            return Err(format!(
                "Cannot save wiki memories because the existing memory file failed to load: {load_error}"
            ));
        }

        save_memories(&self.path, &next_memories)?;
        let mut memories = self.memories.write().unwrap_or_else(|err| err.into_inner());
        *memories = next_memories;
        Ok(())
    }
}

pub fn now_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn save_memories(path: &PathBuf, memories: &[WikiMemory]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create memory directory: {err}"))?;
    }

    let content = serde_json::to_string_pretty(memories)
        .map_err(|err| format!("Failed to serialize wiki memories: {err}"))?;
    fs::write(path, content).map_err(|err| format!("Failed to save wiki memories: {err}"))
}

fn load_memories(path: &PathBuf) -> (Vec<WikiMemory>, Option<String>) {
    if !path.exists() {
        return (Vec::new(), None);
    }

    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            let message = format!(
                "Failed to read wiki memories from {}: {err}",
                path.display()
            );
            log::warn!("{message}");
            return (Vec::new(), Some(message));
        }
    };

    match serde_json::from_str::<Vec<WikiMemory>>(&content) {
        Ok(memories) => (memories, None),
        Err(err) => {
            let message = format!(
                "Failed to parse wiki memories from {}: {err}",
                path.display()
            );
            log::warn!("{message}");
            (Vec::new(), Some(message))
        }
    }
}

fn matches_project_filter(memory: &WikiMemory, project_path: Option<&str>) -> bool {
    let Some(project_path) = project_path else {
        return true;
    };

    if memory.scope == MemoryScope::UserProfile && memory.project_path.is_none() {
        return true;
    }

    memory
        .project_path
        .as_deref()
        .is_some_and(|memory_project| same_normalized_path(memory_project, project_path))
}

fn same_project_path(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => same_normalized_path(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn same_normalized_path(left: &str, right: &str) -> bool {
    normalize_path(left) == normalize_path(right)
}

fn normalize_path(path: &str) -> String {
    path.trim().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::{now_string, WikiMemoryStore};
    use crate::memory::model::{
        MemoryCategory, MemoryListFilter, MemoryScope, MemoryStatus, WikiMemory,
    };

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "wiki-memory-storage-{name}-{}.json",
            uuid::Uuid::now_v7()
        ))
    }

    fn memory(
        id: &str,
        status: MemoryStatus,
        title: &str,
        project_path: Option<&str>,
    ) -> WikiMemory {
        let now = now_string();
        WikiMemory {
            id: id.to_string(),
            category: MemoryCategory::Decision,
            scope: MemoryScope::Project,
            status,
            title: title.to_string(),
            body: "Use the Project Records path".to_string(),
            project_path: project_path.map(str::to_string),
            source_session_id: Some("session-1".to_string()),
            source_message_ids: vec!["message-1".to_string()],
            confidence: 0.7,
            created_at: now.clone(),
            updated_at: now,
            last_used_at: None,
            use_count: 0,
            tags: vec!["forge".to_string()],
        }
    }

    #[tokio::test]
    async fn persists_upserted_memories_and_loads_them_again() {
        let path = temp_path("persist-load");
        let store = WikiMemoryStore::new(path.clone());
        let candidate = memory(
            "memory-1",
            MemoryStatus::Candidate,
            "Forge direction",
            Some("/tmp/forge"),
        );

        let saved = store
            .upsert_candidate(candidate)
            .await
            .expect("candidate should save")
            .expect("candidate should not be suppressed");

        let loaded_store = WikiMemoryStore::new(path.clone());
        let loaded = loaded_store.list(MemoryListFilter::default()).await;

        assert_eq!(loaded, vec![saved]);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn forget_marks_memory_and_hides_it_from_list() {
        let path = temp_path("forget-hides");
        let store = WikiMemoryStore::new(path.clone());
        let saved = store
            .upsert_candidate(memory(
                "memory-1",
                MemoryStatus::Accepted,
                "Forge direction",
                Some("/tmp/forge"),
            ))
            .await
            .expect("candidate should save")
            .expect("candidate should not be suppressed");

        let forgotten = store.forget(&saved.id).await.expect("forget should update");
        let visible = store.list(MemoryListFilter::default()).await;

        assert_eq!(forgotten.status, MemoryStatus::Forgotten);
        assert!(visible.is_empty());
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn does_not_recreate_same_forgotten_title_and_body() {
        let path = temp_path("forgotten-suppression");
        let store = WikiMemoryStore::new(path.clone());
        let mut original = memory(
            "memory-1",
            MemoryStatus::Accepted,
            "Forge direction",
            Some("/tmp/forge"),
        );
        original.body = "Keep context local".to_string();
        store
            .upsert_candidate(original.clone())
            .await
            .expect("candidate should save");
        store
            .forget(&original.id)
            .await
            .expect("forget should update");

        let mut repeated = memory(
            "memory-2",
            MemoryStatus::Candidate,
            &original.title,
            Some("/tmp/forge"),
        );
        repeated.body = original.body.clone();
        let suppressed = store
            .upsert_candidate(repeated)
            .await
            .expect("suppression should not error");

        assert!(suppressed.is_none());
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn candidate_upsert_does_not_overwrite_accepted_memory() {
        let path = temp_path("accepted-no-overwrite");
        let store = WikiMemoryStore::new(path.clone());
        let accepted = memory(
            "accepted-memory",
            MemoryStatus::Accepted,
            "Forge direction",
            Some("/tmp/forge"),
        );
        store.memories.write().unwrap().push(accepted.clone());

        let mut candidate = memory(
            "candidate-memory",
            MemoryStatus::Candidate,
            "Forge direction",
            Some("/tmp/forge"),
        );
        candidate.body = "New candidate body".to_string();
        store
            .upsert_candidate(candidate)
            .await
            .expect("candidate should save");

        let memories = store.list(MemoryListFilter::default()).await;
        let accepted_after = memories
            .iter()
            .find(|memory| memory.id == "accepted-memory")
            .expect("accepted memory should remain");
        assert_eq!(accepted_after.status, MemoryStatus::Accepted);
        assert_eq!(accepted_after.body, accepted.body);
        assert!(memories.iter().any(|memory| {
            memory.id == "candidate-memory"
                && memory.status == MemoryStatus::Candidate
                && memory.body == "New candidate body"
        }));
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn candidate_upsert_does_not_overwrite_pinned_memory() {
        let path = temp_path("pinned-no-overwrite");
        let store = WikiMemoryStore::new(path.clone());
        let pinned = memory(
            "pinned-memory",
            MemoryStatus::Pinned,
            "Forge direction",
            Some("/tmp/forge"),
        );
        store.memories.write().unwrap().push(pinned.clone());

        let mut candidate = memory(
            "candidate-memory",
            MemoryStatus::Candidate,
            "Forge direction",
            Some("/tmp/forge"),
        );
        candidate.body = "New candidate body".to_string();
        store
            .upsert_candidate(candidate)
            .await
            .expect("candidate should save");

        let memories = store.list(MemoryListFilter::default()).await;
        let pinned_after = memories
            .iter()
            .find(|memory| memory.id == "pinned-memory")
            .expect("pinned memory should remain");
        assert_eq!(pinned_after.status, MemoryStatus::Pinned);
        assert_eq!(pinned_after.body, pinned.body);
        assert!(memories.iter().any(|memory| {
            memory.id == "candidate-memory"
                && memory.status == MemoryStatus::Candidate
                && memory.body == "New candidate body"
        }));
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn malformed_json_blocks_save_without_overwriting_original_file() {
        let path = temp_path("malformed-json");
        std::fs::write(&path, "{not valid json").expect("write corrupt fixture");

        let store = WikiMemoryStore::new(path.clone());
        let result = store
            .upsert_candidate(memory(
                "memory-1",
                MemoryStatus::Candidate,
                "Forge direction",
                Some("/tmp/forge"),
            ))
            .await;

        assert!(result
            .expect_err("corrupt load should block save")
            .contains("load"));
        assert_eq!(
            std::fs::read_to_string(&path).expect("corrupt file should remain"),
            "{not valid json"
        );
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn failed_update_save_keeps_in_memory_state_unchanged() {
        let path = std::env::temp_dir();
        let store = WikiMemoryStore::new(path);
        let original = memory(
            "memory-1",
            MemoryStatus::Accepted,
            "Forge direction",
            Some("/tmp/forge"),
        );
        store.memories.write().unwrap().push(original.clone());

        let result = store
            .forget(&original.id)
            .await
            .expect_err("directory path should fail save");
        let memories = store.memories.read().unwrap();

        assert!(result.contains("save"));
        assert_eq!(memories[0].status, MemoryStatus::Accepted);
    }

    #[tokio::test]
    async fn pathless_selection_excludes_project_memories() {
        let path = temp_path("pathless-select");
        let store = WikiMemoryStore::new(path.clone());
        let project_memory = memory(
            "project-memory",
            MemoryStatus::Accepted,
            "Forge direction",
            Some("/tmp/forge"),
        );
        let mut profile_memory = memory(
            "profile-memory",
            MemoryStatus::Accepted,
            "Forge preference",
            None,
        );
        profile_memory.scope = MemoryScope::UserProfile;
        profile_memory.category = MemoryCategory::Preference;

        store
            .upsert_candidate(project_memory)
            .await
            .expect("project memory should save");
        store
            .upsert_candidate(profile_memory)
            .await
            .expect("profile memory should save");

        let selected = store.select("Forge direction preference", None, 10).await;

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].memory_id, "profile-memory");
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn forgotten_suppression_is_scoped_to_matching_project() {
        let path = temp_path("forgotten-suppression-project");
        let store = WikiMemoryStore::new(path.clone());
        let mut original = memory(
            "memory-1",
            MemoryStatus::Accepted,
            "Forge direction",
            Some("/tmp/forge-a"),
        );
        original.body = "Keep context local".to_string();
        store
            .upsert_candidate(original.clone())
            .await
            .expect("candidate should save");
        store
            .forget(&original.id)
            .await
            .expect("forget should update");

        let mut other_project = memory(
            "memory-2",
            MemoryStatus::Candidate,
            &original.title,
            Some("/tmp/forge-b"),
        );
        other_project.body = original.body.clone();
        let saved = store
            .upsert_candidate(other_project)
            .await
            .expect("other project should not error");

        assert!(saved.is_some());
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_upserts_preserve_all_memories() {
        let path = temp_path("concurrent-upserts");
        let store = Arc::new(WikiMemoryStore::new(path.clone()));
        let mut handles = Vec::new();

        for index in 0..24 {
            let store = Arc::clone(&store);
            handles.push(tokio::spawn(async move {
                store
                    .upsert_candidate(memory(
                        &format!("memory-{index}"),
                        MemoryStatus::Candidate,
                        &format!("Forge direction {index}"),
                        Some("/tmp/forge"),
                    ))
                    .await
                    .expect("candidate should save");
            }));
        }

        for handle in handles {
            handle.await.expect("upsert task should complete");
        }

        let loaded = WikiMemoryStore::new(path.clone())
            .list(MemoryListFilter::default())
            .await;

        assert_eq!(loaded.len(), 24);
        let _ = std::fs::remove_file(path);
    }
}
