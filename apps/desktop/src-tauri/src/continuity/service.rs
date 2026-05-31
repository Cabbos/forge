use std::path::Path;

use super::{form_experiences_from_reflection, ContinuityEvent, ContinuityStore, ExperienceMemory};

pub struct ContinuityService {
    store: ContinuityStore,
}

impl ContinuityService {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        Ok(Self {
            store: ContinuityStore::open(path)?,
        })
    }

    pub fn record_event(&self, project_path: &str, event: &ContinuityEvent) -> Result<(), String> {
        self.store.record_event(project_path, event)
    }

    pub fn form_experiences_for_session(
        &self,
        project_path: &str,
        session_id: &str,
        now_ms: u64,
    ) -> Result<Vec<ExperienceMemory>, String> {
        let events = self
            .store
            .list_events_for_session(project_path, session_id)?;
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
        self.store.upsert_experiences(&candidates)
    }

    pub fn list_experiences_for_project(
        &self,
        project_path: &str,
    ) -> Result<Vec<ExperienceMemory>, String> {
        self.store.list_experiences_for_project(project_path)
    }

    pub fn search_experiences_for_project(
        &self,
        project_path: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ExperienceMemory>, String> {
        self.store
            .search_experiences_for_project(project_path, query, limit)
    }
}
