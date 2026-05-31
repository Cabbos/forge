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
        let experiences = self.store.list_experiences_for_project(project_path)?;
        Ok(search_experiences(experiences, query, limit))
    }
}

fn search_experiences(
    experiences: Vec<ExperienceMemory>,
    query: &str,
    limit: usize,
) -> Vec<ExperienceMemory> {
    if limit == 0 {
        return Vec::new();
    }

    let terms = query_terms(query);
    if terms.is_empty() {
        return Vec::new();
    }

    let mut scored = experiences
        .into_iter()
        .filter_map(|experience| {
            let score = relevance_score(&experience, &terms);
            (score > 0).then_some((experience, score))
        })
        .collect::<Vec<_>>();

    scored.sort_by(|(left, left_score), (right, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| {
                right
                    .confidence
                    .partial_cmp(&left.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| right.updated_at_ms.cmp(&left.updated_at_ms))
            .then_with(|| left.id.cmp(&right.id))
    });

    scored
        .into_iter()
        .take(limit)
        .map(|(experience, _)| experience)
        .collect()
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|term| {
            term.trim_matches(|ch: char| !ch.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|term| !term.is_empty())
        .collect()
}

fn relevance_score(experience: &ExperienceMemory, terms: &[String]) -> u32 {
    let title = experience.title.to_lowercase();
    let body = experience.body.to_lowercase();
    let mut score = 0;
    for term in terms {
        if title.contains(term) {
            score += 3;
        }
        if body.contains(term) {
            score += 1;
        }
    }
    score
}
