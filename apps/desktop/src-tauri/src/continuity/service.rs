use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use super::{
    form_experiences_from_reflection, ContinuityEvent, ContinuityStore, ExperienceMemory,
    ExperienceStatus,
};

pub struct ContinuityService {
    stores: Mutex<HashMap<String, Arc<ContinuityStore>>>,
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
        }
    }

    pub fn open(_path: impl AsRef<Path>) -> Result<Self, String> {
        Ok(Self::new())
    }

    fn store_for_project(&self, project_path: &str) -> Result<Arc<ContinuityStore>, String> {
        let mut stores = self.stores.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(store) = stores.get(project_path) {
            return Ok(store.clone());
        }
        let db_path = std::path::PathBuf::from(project_path)
            .join(".forge")
            .join("continuity.db");
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
        let events = store.list_events_for_session(project_path, session_id)?;
        let mut candidates = Vec::new();
        for event in events {
            if let ContinuityEvent::Reflection(reflection) = event {
                candidates.extend(form_experiences_from_reflection(
                    &reflection,
                    Some(project_path),
                    now_ms,
                ));
            }
        }
        store.upsert_experiences(&candidates)
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
