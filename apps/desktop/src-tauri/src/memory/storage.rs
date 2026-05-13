use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::memory::model::{
    MemoryListFilter, MemoryPatch, MemoryScope, MemoryStatus, SelectedContextMemory, WikiMemory,
};
use crate::memory::scoring::select_relevant_memories;

pub struct WikiMemoryStore {
    pub path: PathBuf,
    pub memories: RwLock<Vec<WikiMemory>>,
}

impl WikiMemoryStore {
    pub fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self::new(PathBuf::from(home).join(".tui-to-gui/wiki_memories.json"))
    }

    pub fn new(path: PathBuf) -> Self {
        let memories = fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<Vec<WikiMemory>>(&content).ok())
            .unwrap_or_default();

        Self {
            path,
            memories: RwLock::new(memories),
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
                    .map_or(true, |scope| &memory.scope == scope)
            })
            .filter(|memory| matches_project_filter(memory, filter.project_path.as_deref()))
            .cloned()
            .collect()
    }

    pub async fn upsert_candidate(
        &self,
        candidate: WikiMemory,
    ) -> Result<Option<WikiMemory>, String> {
        let mut memories = self.memories.write().unwrap_or_else(|err| err.into_inner());

        if memories.iter().any(|memory| {
            memory.status == MemoryStatus::Forgotten
                && memory.title == candidate.title
                && memory.body == candidate.body
        }) {
            return Ok(None);
        }

        if let Some(existing) = memories.iter_mut().find(|memory| {
            memory.status != MemoryStatus::Forgotten
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
            let updated = existing.clone();
            save_memories(&self.path, &memories)?;
            return Ok(Some(updated));
        }

        memories.push(candidate.clone());
        save_memories(&self.path, &memories)?;
        Ok(Some(candidate))
    }

    pub async fn update(&self, memory_id: &str, patch: MemoryPatch) -> Result<WikiMemory, String> {
        let mut memories = self.memories.write().unwrap_or_else(|err| err.into_inner());
        let memory = memories
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
        save_memories(&self.path, &memories)?;
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
        let candidates = self
            .list(MemoryListFilter {
                scope: None,
                project_path: project_path.map(str::to_string),
            })
            .await;
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
        let selected_ids = selected
            .iter()
            .map(|memory| memory.memory_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let now = now_string();
        let mut memories = self.memories.write().unwrap_or_else(|err| err.into_inner());

        for memory in memories.iter_mut() {
            if selected_ids.contains(memory.id.as_str()) {
                memory.use_count = memory.use_count.saturating_add(1);
                memory.last_used_at = Some(now.clone());
                memory.updated_at = now.clone();
            }
        }

        save_memories(&self.path, &memories)
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
        .map_or(false, |memory_project| {
            same_normalized_path(memory_project, project_path)
        })
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
            body: "Use the Living Wiki path".to_string(),
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
}
