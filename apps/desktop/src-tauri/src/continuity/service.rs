use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;

use super::{
    form_experiences_from_reflection, ContinuityEvent, ContinuityStore, ExperienceMemory,
    ExperienceStatus,
};

pub struct ContinuityService {
    stores: Mutex<HashMap<String, Arc<ContinuityStore>>>,
    database_path_override: Option<PathBuf>,
}

impl Default for ContinuityService {
    fn default() -> Self {
        Self::new()
    }
}

impl ContinuityService {
    pub fn new() -> Self {
        Self {
            stores: Mutex::new(HashMap::new()),
            database_path_override: None,
        }
    }

    pub(crate) fn new_with_database_path(database_path: PathBuf) -> Self {
        Self {
            stores: Mutex::new(HashMap::new()),
            database_path_override: Some(database_path),
        }
    }

    pub fn open(_path: impl AsRef<Path>) -> Result<Self, String> {
        Ok(Self::new())
    }

    pub fn initialize_project(&self, project_path: &str) -> Result<(), String> {
        self.store_for_project(project_path).map(|_| ())
    }

    fn store_for_project(&self, project_path: &str) -> Result<Arc<ContinuityStore>, String> {
        let mut stores = self.stores.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(store) = stores.get(project_path) {
            return Ok(store.clone());
        }
        let db_path = self.database_path_override.clone().unwrap_or_else(|| {
            std::path::PathBuf::from(project_path)
                .join(".forge")
                .join("continuity.db")
        });
        let store = Arc::new(ContinuityStore::open(&db_path)?);
        stores.insert(project_path.to_string(), store.clone());
        Ok(store)
    }

    pub fn record_event(&self, project_path: &str, event: &ContinuityEvent) -> Result<(), String> {
        self.store_for_project(project_path)?
            .record_event(project_path, event)
    }

    pub fn form_experiences_for_session(
        &self,
        project_path: &str,
        session_id: &str,
        now_ms: u64,
    ) -> Result<Vec<ExperienceMemory>, String> {
        let store = self.store_for_project(project_path)?;
        let reflections = store.list_unformed_reflections_for_session(project_path, session_id)?;
        let mut candidates = Vec::new();
        for reflection in &reflections {
            candidates.extend(form_experiences_from_reflection(
                reflection,
                Some(project_path),
                now_ms,
            ));
        }
        let inserted = store.upsert_experiences(&candidates)?;
        store.mark_reflections_formed(project_path, &reflections)?;
        Ok(inserted)
    }

    pub fn list_experiences_for_project(
        &self,
        project_path: &str,
    ) -> Result<Vec<ExperienceMemory>, String> {
        self.store_for_project(project_path)?
            .list_experiences_for_project(project_path)
    }

    pub fn search_experiences_for_project(
        &self,
        project_path: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ExperienceMemory>, String> {
        self.store_for_project(project_path)?
            .search_experiences_for_project(project_path, query, limit)
    }

    pub fn recall_experiences_for_project(
        &self,
        project_path: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ExperienceMemory>, String> {
        self.store_for_project(project_path)?
            .recall_experiences_for_project(project_path, query, limit)
    }

    pub fn update_experience_status(
        &self,
        project_path: &str,
        experience_id: &str,
        status: ExperienceStatus,
        review_session_id: Option<&str>,
        now_ms: u64,
    ) -> Result<ExperienceMemory, String> {
        let store = self.store_for_project(project_path)?;
        // First get the current experience to know the old status
        let current_experiences = store.list_experiences_for_project(project_path)?;
        let old_status = current_experiences
            .iter()
            .find(|e| e.id == experience_id)
            .map(|e| e.status.clone())
            .unwrap_or(ExperienceStatus::Candidate);

        let updated =
            store.update_experience_status(project_path, experience_id, status.clone(), now_ms)?;
        let event_session_id = review_session_id
            .map(str::trim)
            .filter(|session_id| !session_id.is_empty())
            .map(str::to_string)
            .or_else(|| updated.source_session_id.clone())
            .unwrap_or_else(|| "manual-review".to_string());

        // Record the status change as a continuity event for feedback loop
        if let Err(error) = store.record_event(
            project_path,
            &ContinuityEvent::ExperienceStatusChanged {
                experience_id: experience_id.to_string(),
                old_status,
                new_status: status,
                session_id: event_session_id,
                project_path: Some(project_path.to_string()),
                timestamp_ms: now_ms,
            },
        ) {
            crate::app_log!(
                "WARN",
                "[continuity] failed to record status change event: {}",
                error
            );
        }

        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::{ContinuityEvent, ContinuityService};

    #[test]
    fn isolated_database_path_does_not_write_continuity_state_into_project() {
        let root = tempfile::tempdir().expect("temp root");
        let workspace = root.path().join("workspace");
        let database_path = root.path().join("runtime-state").join("continuity.db");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let service = ContinuityService::new_with_database_path(database_path.clone());
        let project_path = workspace.to_string_lossy().to_string();

        service
            .record_event(
                &project_path,
                &ContinuityEvent::UserMessage {
                    session_id: "session-1".to_string(),
                    content: "remember this".to_string(),
                    timestamp_ms: 1,
                },
            )
            .expect("record isolated continuity event");

        assert!(database_path.exists());
        assert!(!workspace.join(".forge").join("continuity.db").exists());
    }

    #[test]
    fn initialize_project_prepares_schema_before_the_agent_can_validate_it() {
        let root = tempfile::tempdir().expect("temp root");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let service = ContinuityService::new();
        let project_path = workspace.to_string_lossy().to_string();

        service
            .initialize_project(&project_path)
            .expect("initialize continuity project");

        let database_path = workspace.join(".forge").join("continuity.db");
        let conn = rusqlite::Connection::open(database_path).expect("open continuity database");
        let event_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM continuity_events", [], |row| {
                row.get(0)
            })
            .expect("query continuity events");
        assert_eq!(event_count, 0);
    }
}
