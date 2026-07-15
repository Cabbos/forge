use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::agent::time::now_ms;
use crate::continuity::ExperienceStatus;
use crate::ipc::workspace_files::working_dir_for_request_or_explicit;
use crate::memory::facts::{MemoryFactListFilter, UpsertMemoryFactInput};
use crate::memory::{
    plan_unified_context_memory_recall, MemoryListFilter, MemoryPatch, MemoryStatus, RecallPlan,
    UnifiedMemoryRecord, UnifiedMemorySelection, UnifiedMemoryStatus,
};
use crate::state::AppState;

const UNIFIED_MEMORY_DEFAULT_LIMIT: usize = 20;
const UNIFIED_CONTEXT_DEFAULT_LIMIT: usize = 8;
const UNIFIED_CONTEXT_DEFAULT_TOKEN_BUDGET: u32 = 2_048;

pub(crate) struct UnifiedMemoryRecallResult {
    pub(crate) selected: Vec<UnifiedMemorySelection>,
    pub(crate) plan: RecallPlan,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedMemoryActionKind {
    Archive,
    Restore,
    Forget,
    Pin,
    Unpin,
    MarkWrongProject,
    MarkLowValue,
    Edit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UnifiedMemoryActionPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnifiedMemoryAction {
    pub memory_id: String,
    pub action: UnifiedMemoryActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<UnifiedMemoryActionPatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedMemoryActionResult {
    pub memory_id: String,
    pub source: String,
    pub source_id: String,
    pub action: UnifiedMemoryActionKind,
    pub changed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resulting_status: Option<UnifiedMemoryStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<UnifiedMemoryRecord>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedMemoryActionErrorKind {
    InvalidId,
    UnknownSource,
    NotFound,
    UnsupportedAction,
    InvalidPatch,
    StoreError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnifiedMemoryActionError {
    pub kind: UnifiedMemoryActionErrorKind,
    pub memory_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    pub action: UnifiedMemoryActionKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedMemoryListFilter {
    Current,
    Archived,
}

pub(crate) async fn list_unified_memories_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
    query: Option<&str>,
    filter: UnifiedMemoryListFilter,
) -> Result<Vec<UnifiedMemoryRecord>, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let project_path = working_dir.to_string_lossy().to_string();
    let active_profile_id = state.profiles.active_profile_id();
    let mut records =
        collect_unified_memory_records(state, &project_path, active_profile_id.as_deref(), filter)
            .await?;

    if let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) {
        let query_lower = query.to_lowercase();
        records.retain(|record| {
            record.title.to_lowercase().contains(&query_lower)
                || record.body.to_lowercase().contains(&query_lower)
                || record
                    .tags
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(&query_lower))
        });
    }

    records.truncate(UNIFIED_MEMORY_DEFAULT_LIMIT);
    Ok(records)
}

pub(crate) async fn select_unified_memories_for_send_input(
    state: &Arc<AppState>,
    text: &str,
    project_path: &str,
) -> Result<UnifiedMemoryRecallResult, String> {
    let active_profile_id = state.profiles.active_profile_id();
    let records = collect_unified_memory_records(
        state,
        project_path,
        active_profile_id.as_deref(),
        UnifiedMemoryListFilter::Current,
    )
    .await?;
    let plan = plan_unified_context_memory_recall(
        &records,
        text,
        Some(project_path),
        active_profile_id.as_deref(),
        UNIFIED_CONTEXT_DEFAULT_LIMIT,
        UNIFIED_CONTEXT_DEFAULT_TOKEN_BUDGET,
    );
    let selected = plan.clone().into_selected();
    Ok(UnifiedMemoryRecallResult { selected, plan })
}

async fn collect_unified_memory_records(
    state: &Arc<AppState>,
    project_path: &str,
    active_profile_id: Option<&str>,
    filter: UnifiedMemoryListFilter,
) -> Result<Vec<UnifiedMemoryRecord>, String> {
    let mut records = Vec::new();

    let wiki = state
        .wiki_memory
        .list_all_statuses(MemoryListFilter {
            scope: None,
            project_path: Some(project_path.to_string()),
        })
        .await;
    records.extend(
        wiki.into_iter()
            .map(UnifiedMemoryRecord::from_wiki_memory)
            .filter(|record| record_matches_list_filter(record, filter)),
    );

    if filter == UnifiedMemoryListFilter::Current {
        let facts = state
            .memory_facts
            .list_with_filter(MemoryFactListFilter {
                query: None,
                profile_id: None,
            })
            .into_iter()
            .filter(|fact| {
                fact.profile_id.is_none() || fact.profile_id.as_deref() == active_profile_id
            });
        records.extend(facts.map(UnifiedMemoryRecord::from_memory_fact));
    }

    let experiences = state
        .continuity
        .list_experiences_for_project(project_path)?;
    records.extend(
        experiences
            .into_iter()
            .map(UnifiedMemoryRecord::from_continuity_experience)
            .filter(|record| record_matches_list_filter(record, filter)),
    );

    Ok(records)
}

fn record_matches_list_filter(
    record: &UnifiedMemoryRecord,
    filter: UnifiedMemoryListFilter,
) -> bool {
    match filter {
        UnifiedMemoryListFilter::Current => {
            matches!(
                record.status,
                crate::memory::UnifiedMemoryStatus::Accepted
                    | crate::memory::UnifiedMemoryStatus::Pinned
            )
        }
        UnifiedMemoryListFilter::Archived => {
            record.status == crate::memory::UnifiedMemoryStatus::Archived
        }
    }
}

pub(crate) async fn apply_unified_memory_action_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
    action: UnifiedMemoryAction,
) -> Result<UnifiedMemoryActionResult, UnifiedMemoryActionError> {
    let (source, source_id) = parse_unified_memory_id(&action.memory_id, action.action)?;
    if !matches!(
        source,
        "wiki_memory" | "memory_fact" | "continuity_experience"
    ) {
        return Err(action_error(
            UnifiedMemoryActionErrorKind::UnknownSource,
            &action.memory_id,
            Some(source),
            Some(source_id),
            action.action,
            format!("Unsupported unified memory source: {source}"),
        ));
    }
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir)
        .await
        .map_err(|error| {
            action_error(
                UnifiedMemoryActionErrorKind::StoreError,
                &action.memory_id,
                Some(source),
                Some(source_id),
                action.action,
                error,
            )
        })?;
    let project_path = working_dir.to_string_lossy().to_string();
    let active_profile_id = state.profiles.active_profile_id();
    let records = collect_unified_memory_records_for_action(
        state,
        &project_path,
        active_profile_id.as_deref(),
        action.action,
        &action.memory_id,
        Some(source),
        Some(source_id),
    )
    .await?;

    if !records.iter().any(|record| record.id == action.memory_id) {
        return Err(action_error(
            UnifiedMemoryActionErrorKind::NotFound,
            &action.memory_id,
            Some(source),
            Some(source_id),
            action.action,
            format!("{} not found or out of scope", action.memory_id),
        ));
    }

    match source {
        "wiki_memory" => {
            apply_wiki_memory_action(
                state,
                &action.memory_id,
                source_id,
                action.action,
                action.patch.as_ref(),
            )
            .await
        }
        "memory_fact" => apply_memory_fact_action(
            state,
            &action.memory_id,
            source_id,
            action.action,
            action.patch.as_ref(),
        ),
        "continuity_experience" => apply_continuity_experience_action(
            state,
            &project_path,
            session_id,
            &action.memory_id,
            source_id,
            action.action,
            action.patch.as_ref(),
        ),
        _ => unreachable!("source was checked above"),
    }
}

async fn collect_unified_memory_records_for_action(
    state: &Arc<AppState>,
    project_path: &str,
    active_profile_id: Option<&str>,
    action: UnifiedMemoryActionKind,
    memory_id: &str,
    source: Option<&str>,
    source_id: Option<&str>,
) -> Result<Vec<UnifiedMemoryRecord>, UnifiedMemoryActionError> {
    let filters: &[UnifiedMemoryListFilter] = match action {
        UnifiedMemoryActionKind::Restore => &[UnifiedMemoryListFilter::Archived],
        UnifiedMemoryActionKind::Forget => &[
            UnifiedMemoryListFilter::Current,
            UnifiedMemoryListFilter::Archived,
        ],
        UnifiedMemoryActionKind::Archive
        | UnifiedMemoryActionKind::Pin
        | UnifiedMemoryActionKind::Unpin
        | UnifiedMemoryActionKind::MarkWrongProject
        | UnifiedMemoryActionKind::MarkLowValue
        | UnifiedMemoryActionKind::Edit => &[UnifiedMemoryListFilter::Current],
    };
    let mut records = Vec::new();
    for filter in filters {
        let mut batch =
            collect_unified_memory_records(state, project_path, active_profile_id, *filter)
                .await
                .map_err(|error| {
                    action_error(
                        UnifiedMemoryActionErrorKind::StoreError,
                        memory_id,
                        source,
                        source_id,
                        action,
                        error,
                    )
                })?;
        records.append(&mut batch);
    }
    Ok(records)
}

fn parse_unified_memory_id(
    memory_id: &str,
    action: UnifiedMemoryActionKind,
) -> Result<(&str, &str), UnifiedMemoryActionError> {
    let trimmed = memory_id.trim();
    let Some((source, source_id)) = trimmed.split_once(':') else {
        return Err(action_error(
            UnifiedMemoryActionErrorKind::InvalidId,
            memory_id,
            None,
            None,
            action,
            format!("Unified memory id must include a source prefix: {memory_id}"),
        ));
    };
    let source = source.trim();
    let source_id = source_id.trim();
    if source.is_empty() || source_id.is_empty() {
        return Err(action_error(
            UnifiedMemoryActionErrorKind::InvalidId,
            memory_id,
            Some(source),
            Some(source_id),
            action,
            format!("Unified memory id is incomplete: {memory_id}"),
        ));
    }
    Ok((source, source_id))
}

async fn apply_wiki_memory_action(
    state: &Arc<AppState>,
    memory_id: &str,
    source_id: &str,
    action: UnifiedMemoryActionKind,
    patch: Option<&UnifiedMemoryActionPatch>,
) -> Result<UnifiedMemoryActionResult, UnifiedMemoryActionError> {
    reject_patch_for_non_fact(memory_id, "wiki_memory", source_id, action, patch)?;
    let (status, evidence) = status_action_target(action, "wiki_memory", memory_id, source_id)?;
    let updated = state
        .wiki_memory
        .update(
            source_id,
            MemoryPatch {
                status: Some(status),
                ..MemoryPatch::default()
            },
        )
        .await
        .map_err(|error| {
            action_error(
                UnifiedMemoryActionErrorKind::StoreError,
                memory_id,
                Some("wiki_memory"),
                Some(source_id),
                action,
                format!("{memory_id} not found: {error}"),
            )
        })?;
    Ok(action_result(
        memory_id,
        "wiki_memory",
        source_id,
        action,
        true,
        Some(UnifiedMemoryRecord::from_wiki_memory(updated)),
        evidence,
    ))
}

fn apply_memory_fact_action(
    state: &Arc<AppState>,
    memory_id: &str,
    source_id: &str,
    action: UnifiedMemoryActionKind,
    patch: Option<&UnifiedMemoryActionPatch>,
) -> Result<UnifiedMemoryActionResult, UnifiedMemoryActionError> {
    match action {
        UnifiedMemoryActionKind::Edit => {
            let current = state.memory_facts.get(source_id).ok_or_else(|| {
                action_error(
                    UnifiedMemoryActionErrorKind::NotFound,
                    memory_id,
                    Some("memory_fact"),
                    Some(source_id),
                    action,
                    format!("{memory_id} not found"),
                )
            })?;
            let patch = patch.ok_or_else(|| {
                action_error(
                    UnifiedMemoryActionErrorKind::InvalidPatch,
                    memory_id,
                    Some("memory_fact"),
                    Some(source_id),
                    action,
                    format!("edit requires a patch for {memory_id}"),
                )
            })?;
            let body = patch
                .body
                .as_deref()
                .unwrap_or(current.text.as_str())
                .trim()
                .to_string();
            if body.is_empty() {
                return Err(action_error(
                    UnifiedMemoryActionErrorKind::InvalidPatch,
                    memory_id,
                    Some("memory_fact"),
                    Some(source_id),
                    action,
                    "memory fact body must not be empty".to_string(),
                ));
            }
            let updated = state
                .memory_facts
                .upsert(UpsertMemoryFactInput {
                    id: Some(source_id.to_string()),
                    text: body,
                    tags: patch.tags.clone().unwrap_or(current.tags),
                    profile_id: current.profile_id,
                    source: current.source,
                })
                .map_err(|error| {
                    action_error(
                        UnifiedMemoryActionErrorKind::StoreError,
                        memory_id,
                        Some("memory_fact"),
                        Some(source_id),
                        action,
                        error,
                    )
                })?
                .fact;
            Ok(action_result(
                memory_id,
                "memory_fact",
                source_id,
                action,
                true,
                Some(UnifiedMemoryRecord::from_memory_fact(updated)),
                vec![
                    "edit_policy=profile_fact_detail_editor".to_string(),
                    "memory_fact_updated".to_string(),
                ],
            ))
        }
        UnifiedMemoryActionKind::Forget => {
            if state.memory_facts.get(source_id).is_none() {
                return Err(action_error(
                    UnifiedMemoryActionErrorKind::NotFound,
                    memory_id,
                    Some("memory_fact"),
                    Some(source_id),
                    action,
                    format!("{memory_id} not found"),
                ));
            }
            let removed = state.memory_facts.delete(source_id).map_err(|error| {
                action_error(
                    UnifiedMemoryActionErrorKind::StoreError,
                    memory_id,
                    Some("memory_fact"),
                    Some(source_id),
                    action,
                    error,
                )
            })?;
            if removed {
                Ok(action_result(
                    memory_id,
                    "memory_fact",
                    source_id,
                    action,
                    true,
                    None,
                    vec![
                        "forget_policy=delete_supported".to_string(),
                        "memory_fact_deleted".to_string(),
                    ],
                ))
            } else {
                Err(action_error(
                    UnifiedMemoryActionErrorKind::NotFound,
                    memory_id,
                    Some("memory_fact"),
                    Some(source_id),
                    action,
                    format!("{memory_id} not found"),
                ))
            }
        }
        UnifiedMemoryActionKind::Archive
        | UnifiedMemoryActionKind::Restore
        | UnifiedMemoryActionKind::Pin
        | UnifiedMemoryActionKind::Unpin
        | UnifiedMemoryActionKind::MarkWrongProject
        | UnifiedMemoryActionKind::MarkLowValue => Err(action_error(
            UnifiedMemoryActionErrorKind::UnsupportedAction,
            memory_id,
            Some("memory_fact"),
            Some(source_id),
            action,
            format!("{action:?} is unsupported for memory facts; edit or forget is allowed"),
        )),
    }
}

fn apply_continuity_experience_action(
    state: &Arc<AppState>,
    project_path: &str,
    session_id: Option<&str>,
    memory_id: &str,
    source_id: &str,
    action: UnifiedMemoryActionKind,
    patch: Option<&UnifiedMemoryActionPatch>,
) -> Result<UnifiedMemoryActionResult, UnifiedMemoryActionError> {
    reject_patch_for_non_fact(memory_id, "continuity_experience", source_id, action, patch)?;
    let (status, evidence) =
        experience_status_action_target(action, "continuity_experience", memory_id, source_id)?;
    if !state
        .continuity
        .list_experiences_for_project(project_path)
        .map_err(|error| {
            action_error(
                UnifiedMemoryActionErrorKind::StoreError,
                memory_id,
                Some("continuity_experience"),
                Some(source_id),
                action,
                error,
            )
        })?
        .iter()
        .any(|experience| experience.id == source_id)
    {
        return Err(action_error(
            UnifiedMemoryActionErrorKind::NotFound,
            memory_id,
            Some("continuity_experience"),
            Some(source_id),
            action,
            format!("{memory_id} not found"),
        ));
    }
    let updated = state
        .continuity
        .update_experience_status(project_path, source_id, status, session_id, now_ms())
        .map_err(|error| {
            action_error(
                UnifiedMemoryActionErrorKind::StoreError,
                memory_id,
                Some("continuity_experience"),
                Some(source_id),
                action,
                format!("{memory_id} not found: {error}"),
            )
        })?;
    Ok(action_result(
        memory_id,
        "continuity_experience",
        source_id,
        action,
        true,
        Some(UnifiedMemoryRecord::from_continuity_experience(updated)),
        evidence,
    ))
}

fn status_action_target(
    action: UnifiedMemoryActionKind,
    source: &str,
    memory_id: &str,
    source_id: &str,
) -> Result<(MemoryStatus, Vec<String>), UnifiedMemoryActionError> {
    let (status, reason) = match action {
        UnifiedMemoryActionKind::Archive => (MemoryStatus::Archived, "archive_supported"),
        UnifiedMemoryActionKind::Restore => (MemoryStatus::Accepted, "restore_supported"),
        UnifiedMemoryActionKind::Forget => (MemoryStatus::Forgotten, "forget_supported"),
        UnifiedMemoryActionKind::Pin => (MemoryStatus::Pinned, "pin_supported"),
        UnifiedMemoryActionKind::Unpin => (MemoryStatus::Accepted, "unpin_supported"),
        UnifiedMemoryActionKind::MarkWrongProject => {
            (MemoryStatus::Archived, "mark_wrong_project_archives_record")
        }
        UnifiedMemoryActionKind::MarkLowValue => {
            (MemoryStatus::Archived, "mark_low_value_archives_record")
        }
        UnifiedMemoryActionKind::Edit => {
            return Err(action_error(
                UnifiedMemoryActionErrorKind::UnsupportedAction,
                memory_id,
                Some(source),
                Some(source_id),
                action,
                "edit is only supported for memory facts".to_string(),
            ));
        }
    };
    Ok((
        status,
        vec![
            format!("source={source}"),
            reason.to_string(),
            format!("action={action:?}"),
        ],
    ))
}

fn experience_status_action_target(
    action: UnifiedMemoryActionKind,
    source: &str,
    memory_id: &str,
    source_id: &str,
) -> Result<(ExperienceStatus, Vec<String>), UnifiedMemoryActionError> {
    let (status, reason) = match action {
        UnifiedMemoryActionKind::Archive => (ExperienceStatus::Archived, "archive_supported"),
        UnifiedMemoryActionKind::Restore => (ExperienceStatus::Accepted, "restore_supported"),
        UnifiedMemoryActionKind::Forget => (ExperienceStatus::Forgotten, "forget_supported"),
        UnifiedMemoryActionKind::Pin => (ExperienceStatus::Pinned, "pin_supported"),
        UnifiedMemoryActionKind::Unpin => (ExperienceStatus::Accepted, "unpin_supported"),
        UnifiedMemoryActionKind::MarkWrongProject => (
            ExperienceStatus::Archived,
            "mark_wrong_project_archives_record",
        ),
        UnifiedMemoryActionKind::MarkLowValue => {
            (ExperienceStatus::Archived, "mark_low_value_archives_record")
        }
        UnifiedMemoryActionKind::Edit => {
            return Err(action_error(
                UnifiedMemoryActionErrorKind::UnsupportedAction,
                memory_id,
                Some(source),
                Some(source_id),
                action,
                "edit is only supported for memory facts".to_string(),
            ));
        }
    };
    Ok((
        status,
        vec![
            format!("source={source}"),
            reason.to_string(),
            format!("action={action:?}"),
        ],
    ))
}

fn reject_patch_for_non_fact(
    memory_id: &str,
    source: &str,
    source_id: &str,
    action: UnifiedMemoryActionKind,
    patch: Option<&UnifiedMemoryActionPatch>,
) -> Result<(), UnifiedMemoryActionError> {
    if patch.is_some() {
        return Err(action_error(
            UnifiedMemoryActionErrorKind::UnsupportedAction,
            memory_id,
            Some(source),
            Some(source_id),
            action,
            "edit patches are only supported for memory facts".to_string(),
        ));
    }
    if action == UnifiedMemoryActionKind::Edit {
        return Err(action_error(
            UnifiedMemoryActionErrorKind::UnsupportedAction,
            memory_id,
            Some(source),
            Some(source_id),
            action,
            "edit is only supported for memory facts".to_string(),
        ));
    }
    Ok(())
}

fn action_result(
    memory_id: &str,
    source: &str,
    source_id: &str,
    action: UnifiedMemoryActionKind,
    changed: bool,
    record: Option<UnifiedMemoryRecord>,
    evidence: Vec<String>,
) -> UnifiedMemoryActionResult {
    UnifiedMemoryActionResult {
        memory_id: memory_id.to_string(),
        source: source.to_string(),
        source_id: source_id.to_string(),
        action,
        changed,
        resulting_status: record.as_ref().map(|record| record.status.clone()),
        record,
        evidence,
    }
}

fn action_error(
    kind: UnifiedMemoryActionErrorKind,
    memory_id: &str,
    source: Option<&str>,
    source_id: Option<&str>,
    action: UnifiedMemoryActionKind,
    message: String,
) -> UnifiedMemoryActionError {
    UnifiedMemoryActionError {
        kind,
        memory_id: memory_id.to_string(),
        source: source.map(str::to_string),
        source_id: source_id.map(str::to_string),
        action,
        message,
    }
}

#[tauri::command]
pub async fn list_unified_memories(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
    query: Option<String>,
    filter: Option<UnifiedMemoryListFilter>,
) -> Result<Vec<UnifiedMemoryRecord>, String> {
    list_unified_memories_for_request(
        &state,
        session_id.as_deref(),
        working_dir.as_deref(),
        query.as_deref(),
        filter.unwrap_or(UnifiedMemoryListFilter::Current),
    )
    .await
}

#[tauri::command]
pub async fn apply_unified_memory_action(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
    action: UnifiedMemoryAction,
) -> Result<UnifiedMemoryActionResult, UnifiedMemoryActionError> {
    apply_unified_memory_action_for_request(
        &state,
        session_id.as_deref(),
        working_dir.as_deref(),
        action,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::continuity::{
        ContinuityEvent, ExperienceStatus, ReflectionEvent, ReflectionOutcome,
    };
    use crate::harness::Harness;
    use crate::memory::facts::{MemoryFactStore, UpsertMemoryFactInput};
    use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus, WikiMemory};
    use crate::memory::storage::WikiMemoryStore;
    use crate::profile::{ProfileStore, UpsertProfileInput};

    #[tokio::test]
    async fn unified_memory_records_include_wiki_active_profile_and_global_facts() {
        let nonce = uuid::Uuid::now_v7();
        let workspace = std::env::temp_dir().join(format!("forge-unified-memory-{nonce}"));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let workspace = workspace.canonicalize().expect("canonical workspace");
        let wiki_path = std::env::temp_dir().join(format!("forge-unified-wiki-{nonce}.json"));
        let facts_path = std::env::temp_dir().join(format!("forge-unified-facts-{nonce}.json"));
        let profiles_path =
            std::env::temp_dir().join(format!("forge-unified-profiles-{nonce}.json"));

        let mut app_state = AppState::new(Arc::new(Harness::new(workspace.clone())));
        app_state.wiki_memory = Arc::new(WikiMemoryStore::new(wiki_path.clone()));
        app_state.memory_facts = Arc::new(MemoryFactStore::new(facts_path.clone()));
        app_state.profiles = Arc::new(ProfileStore::new(profiles_path.clone()));
        let state = Arc::new(app_state);

        let work = state
            .profiles
            .upsert(UpsertProfileInput {
                id: Some("work".to_string()),
                name: "Work".to_string(),
                default_provider: None,
                default_model: None,
                default_workspace: None,
                api_key_overrides: None,
            })
            .expect("profile");
        state.profiles.set_active(&work.id).expect("active profile");

        state
            .wiki_memory
            .upsert_candidate(WikiMemory {
                id: "wiki-progress".to_string(),
                category: MemoryCategory::TaskState,
                scope: MemoryScope::Project,
                status: MemoryStatus::Accepted,
                title: "权限进度".to_string(),
                body: "完全访问按钮已完成".to_string(),
                project_path: Some(workspace.to_string_lossy().to_string()),
                source_session_id: Some("session-1".to_string()),
                source_message_ids: Vec::new(),
                confidence: 0.8,
                created_at: "1772582400000".to_string(),
                updated_at: "1772582400000".to_string(),
                last_used_at: None,
                use_count: 0,
                tags: vec!["task_state".to_string()],
            })
            .await
            .expect("wiki");

        state
            .memory_facts
            .upsert(UpsertMemoryFactInput {
                id: Some("fact-work".to_string()),
                text: "完全访问模式用于测试权限绕过".to_string(),
                tags: vec!["permission".to_string()],
                profile_id: Some("work".to_string()),
                source: Some("settings".to_string()),
            })
            .expect("fact");
        state
            .memory_facts
            .upsert(UpsertMemoryFactInput {
                id: Some("fact-global".to_string()),
                text: "全局记忆事实参与召回".to_string(),
                tags: vec!["global".to_string()],
                profile_id: None,
                source: Some("settings".to_string()),
            })
            .expect("global fact");
        state
            .memory_facts
            .upsert(UpsertMemoryFactInput {
                id: Some("fact-personal".to_string()),
                text: "不应该出现在 work profile".to_string(),
                tags: vec!["private".to_string()],
                profile_id: Some("personal".to_string()),
                source: Some("settings".to_string()),
            })
            .expect("other profile fact");

        let records = list_unified_memories_for_request(
            &state,
            None,
            Some(workspace.to_str().expect("utf8")),
            None,
            UnifiedMemoryListFilter::Current,
        )
        .await
        .expect("records");

        let ids = records
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>();
        assert!(ids.contains(&"wiki_memory:wiki-progress"));
        assert!(ids.contains(&"memory_fact:fact-work"));
        assert!(ids.contains(&"memory_fact:fact-global"));
        assert!(!ids.contains(&"memory_fact:fact-personal"));

        let _ = std::fs::remove_dir_all(workspace);
        let _ = std::fs::remove_file(wiki_path);
        let _ = std::fs::remove_file(facts_path);
        let _ = std::fs::remove_file(profiles_path);
    }

    #[tokio::test]
    async fn unified_memory_action_archives_wiki_memory_by_unified_id() {
        let fixture = UnifiedMemoryActionFixture::new();
        fixture.seed_wiki_memory("wiki-action").await;

        apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "wiki_memory:wiki-action".to_string(),
                action: UnifiedMemoryActionKind::Archive,
                patch: None,
            },
        )
        .await
        .expect("archive wiki memory");

        let records = list_unified_memories_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            Some("Wiki action"),
            UnifiedMemoryListFilter::Archived,
        )
        .await
        .expect("records");
        let archived = records
            .iter()
            .find(|record| record.id == "wiki_memory:wiki-action")
            .expect("archived wiki memory stays visible to archived filter");
        assert_eq!(
            archived.status,
            crate::memory::UnifiedMemoryStatus::Archived
        );

        fixture.cleanup();
    }

    #[tokio::test]
    async fn unified_memory_action_rejects_wiki_memory_from_another_project() {
        let fixture = UnifiedMemoryActionFixture::new();
        let other_workspace = std::env::temp_dir().join(format!(
            "forge-unified-action-other-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&other_workspace).expect("other workspace");
        let other_project_path = other_workspace
            .canonicalize()
            .expect("canonical other")
            .to_string_lossy()
            .to_string();
        fixture
            .seed_wiki_memory_for_project("wiki-other-project", &other_project_path)
            .await;

        let error = apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "wiki_memory:wiki-other-project".to_string(),
                action: UnifiedMemoryActionKind::Archive,
                patch: None,
            },
        )
        .await
        .expect_err("reject out-of-project wiki memory");

        assert_eq!(error.kind, UnifiedMemoryActionErrorKind::NotFound);
        assert!(error.message.contains("not found"));
        let records = list_unified_memories_for_request(
            &fixture.state,
            None,
            Some(other_project_path.as_str()),
            Some("Wiki action"),
            UnifiedMemoryListFilter::Current,
        )
        .await
        .expect("other project records");
        assert_eq!(
            records
                .iter()
                .find(|record| record.id == "wiki_memory:wiki-other-project")
                .expect("wiki memory")
                .status,
            crate::memory::UnifiedMemoryStatus::Accepted
        );

        let _ = std::fs::remove_dir_all(other_workspace);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn unified_memory_action_forgets_memory_fact_by_unified_id() {
        let fixture = UnifiedMemoryActionFixture::new();
        fixture.seed_memory_fact("fact-action");
        assert!(fixture.state.memory_facts.get("fact-action").is_some());

        apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "memory_fact:fact-action".to_string(),
                action: UnifiedMemoryActionKind::Forget,
                patch: None,
            },
        )
        .await
        .expect("forget fact");

        assert!(fixture.state.memory_facts.get("fact-action").is_none());
        fixture.cleanup();
    }

    #[tokio::test]
    async fn unified_memory_action_rejects_memory_fact_from_inactive_profile() {
        let fixture = UnifiedMemoryActionFixture::new();
        fixture.seed_profile("work");
        fixture.seed_profile("personal");
        fixture
            .state
            .profiles
            .set_active("work")
            .expect("active work");
        fixture.seed_memory_fact_for_profile("fact-personal-action", Some("personal"));

        let error = apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "memory_fact:fact-personal-action".to_string(),
                action: UnifiedMemoryActionKind::Forget,
                patch: None,
            },
        )
        .await
        .expect_err("reject inactive-profile fact");

        assert_eq!(error.kind, UnifiedMemoryActionErrorKind::NotFound);
        assert!(error.message.contains("not found"));
        assert!(fixture
            .state
            .memory_facts
            .get("fact-personal-action")
            .is_some());
        fixture.cleanup();
    }

    #[tokio::test]
    async fn unified_memory_action_archives_continuity_experience_by_unified_id() {
        let fixture = UnifiedMemoryActionFixture::new();
        let experience_id = fixture.seed_continuity_experience("continuity-action");

        apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: format!("continuity_experience:{experience_id}"),
                action: UnifiedMemoryActionKind::Archive,
                patch: None,
            },
        )
        .await
        .expect("archive continuity");

        let experiences = fixture
            .state
            .continuity
            .list_experiences_for_project(&fixture.project_path)
            .expect("experiences");
        let archived = experiences
            .iter()
            .find(|experience| experience.id == experience_id)
            .expect("experience");
        assert_eq!(archived.status, ExperienceStatus::Archived);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn unified_memory_action_v2_restores_wiki_memory_with_typed_result() {
        let fixture = UnifiedMemoryActionFixture::new();
        fixture
            .seed_wiki_memory_with_status(
                "wiki-restore",
                MemoryStatus::Archived,
                "Archived memory can return to current recall.",
            )
            .await;

        let result = apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "wiki_memory:wiki-restore".to_string(),
                action: UnifiedMemoryActionKind::Restore,
                patch: None,
            },
        )
        .await
        .expect("restore wiki memory");

        assert_eq!(result.memory_id, "wiki_memory:wiki-restore");
        assert_eq!(result.action, UnifiedMemoryActionKind::Restore);
        assert_eq!(result.source, "wiki_memory");
        assert_eq!(result.source_id, "wiki-restore");
        assert!(result.changed);
        assert_eq!(
            result.resulting_status,
            Some(crate::memory::UnifiedMemoryStatus::Accepted)
        );
        assert_eq!(
            result.record.as_ref().map(|record| &record.status),
            Some(&crate::memory::UnifiedMemoryStatus::Accepted)
        );
        assert!(result
            .evidence
            .iter()
            .any(|entry| entry.contains("restore_supported")));

        let current = list_unified_memories_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            Some("Archived memory"),
            UnifiedMemoryListFilter::Current,
        )
        .await
        .expect("current records");
        assert!(current.iter().any(|record| record.id == result.memory_id));

        fixture.cleanup();
    }

    #[tokio::test]
    async fn unified_memory_action_v2_edits_memory_fact_and_rejects_wiki_edit() {
        let fixture = UnifiedMemoryActionFixture::new();
        fixture.seed_memory_fact("fact-edit");
        fixture.seed_wiki_memory("wiki-edit").await;

        let result = apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "memory_fact:fact-edit".to_string(),
                action: UnifiedMemoryActionKind::Edit,
                patch: Some(UnifiedMemoryActionPatch {
                    body: Some("Edited fact body".to_string()),
                    tags: Some(vec!["edited".to_string(), "action".to_string()]),
                }),
            },
        )
        .await
        .expect("edit memory fact");

        assert_eq!(result.action, UnifiedMemoryActionKind::Edit);
        assert!(result.changed);
        assert_eq!(
            fixture
                .state
                .memory_facts
                .get("fact-edit")
                .expect("fact")
                .text,
            "Edited fact body"
        );
        assert_eq!(
            result.record.as_ref().map(|record| record.body.as_str()),
            Some("Edited fact body")
        );

        let error = apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "wiki_memory:wiki-edit".to_string(),
                action: UnifiedMemoryActionKind::Edit,
                patch: Some(UnifiedMemoryActionPatch {
                    body: Some("Wiki edit should be rejected".to_string()),
                    tags: None,
                }),
            },
        )
        .await
        .expect_err("wiki edit is rejected");

        assert_eq!(error.kind, UnifiedMemoryActionErrorKind::UnsupportedAction);
        assert_eq!(error.source.as_deref(), Some("wiki_memory"));
        assert!(error.message.contains("memory facts"));

        fixture.cleanup();
    }

    #[tokio::test]
    async fn unified_memory_action_v2_supports_pin_unpin_and_quality_marks_for_status_sources() {
        let fixture = UnifiedMemoryActionFixture::new();
        fixture.seed_wiki_memory("wiki-v2-actions").await;

        let pinned = apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "wiki_memory:wiki-v2-actions".to_string(),
                action: UnifiedMemoryActionKind::Pin,
                patch: None,
            },
        )
        .await
        .expect("pin wiki memory");
        assert_eq!(
            pinned.resulting_status,
            Some(crate::memory::UnifiedMemoryStatus::Pinned)
        );

        let unpinned = apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "wiki_memory:wiki-v2-actions".to_string(),
                action: UnifiedMemoryActionKind::Unpin,
                patch: None,
            },
        )
        .await
        .expect("unpin wiki memory");
        assert_eq!(
            unpinned.resulting_status,
            Some(crate::memory::UnifiedMemoryStatus::Accepted)
        );

        let low_value = apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "wiki_memory:wiki-v2-actions".to_string(),
                action: UnifiedMemoryActionKind::MarkLowValue,
                patch: None,
            },
        )
        .await
        .expect("mark low value");
        assert_eq!(
            low_value.resulting_status,
            Some(crate::memory::UnifiedMemoryStatus::Archived)
        );
        assert!(low_value
            .evidence
            .iter()
            .any(|entry| entry.contains("mark_low_value")));

        fixture
            .seed_wiki_memory_with_status(
                "wiki-wrong-project",
                MemoryStatus::Accepted,
                "Wrong project evidence",
            )
            .await;
        let wrong_project = apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "wiki_memory:wiki-wrong-project".to_string(),
                action: UnifiedMemoryActionKind::MarkWrongProject,
                patch: None,
            },
        )
        .await
        .expect("mark wrong project");
        assert_eq!(
            wrong_project.resulting_status,
            Some(crate::memory::UnifiedMemoryStatus::Archived)
        );
        assert!(wrong_project
            .evidence
            .iter()
            .any(|entry| entry.contains("mark_wrong_project")));

        fixture.cleanup();
    }

    #[tokio::test]
    async fn unified_memory_list_filters_current_and_archived_without_hidden_wiki_bodies() {
        let fixture = UnifiedMemoryActionFixture::new();
        fixture.seed_wiki_memory("wiki-current").await;
        fixture
            .seed_wiki_memory_with_status(
                "wiki-archived",
                MemoryStatus::Archived,
                "Archived body should be queryable only in archived filter",
            )
            .await;
        fixture
            .seed_wiki_memory_with_status(
                "wiki-forgotten",
                MemoryStatus::Forgotten,
                "FORGOTTEN SECRET BODY",
            )
            .await;
        fixture
            .seed_wiki_memory_with_status(
                "wiki-candidate",
                MemoryStatus::Candidate,
                "CANDIDATE SECRET BODY",
            )
            .await;

        let current = list_unified_memories_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            None,
            UnifiedMemoryListFilter::Current,
        )
        .await
        .expect("current records");
        let current_ids = current
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>();
        assert!(current_ids.contains(&"wiki_memory:wiki-current"));
        assert!(!current_ids.contains(&"wiki_memory:wiki-archived"));
        assert!(!current_ids.contains(&"wiki_memory:wiki-forgotten"));
        assert!(!current_ids.contains(&"wiki_memory:wiki-candidate"));
        assert!(!serde_json::to_string(&current)
            .expect("json")
            .contains("SECRET BODY"));

        let archived = list_unified_memories_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            None,
            UnifiedMemoryListFilter::Archived,
        )
        .await
        .expect("archived records");
        assert_eq!(
            archived
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>(),
            vec!["wiki_memory:wiki-archived"]
        );
        assert!(archived[0]
            .body
            .contains("Archived body should be queryable"));

        fixture.cleanup();
    }

    #[tokio::test]
    async fn unified_memory_action_rejects_unknown_source() {
        let fixture = UnifiedMemoryActionFixture::new();
        let error = apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "unknown_source:alpha".to_string(),
                action: UnifiedMemoryActionKind::Archive,
                patch: None,
            },
        )
        .await
        .expect_err("unknown source");

        assert_eq!(error.kind, UnifiedMemoryActionErrorKind::UnknownSource);
        assert!(error.message.contains("Unsupported unified memory source"));
        fixture.cleanup();
    }

    #[tokio::test]
    async fn unified_memory_action_rejects_missing_source_id() {
        let fixture = UnifiedMemoryActionFixture::new();
        let error = apply_unified_memory_action_for_request(
            &fixture.state,
            None,
            Some(fixture.project_path.as_str()),
            UnifiedMemoryAction {
                memory_id: "memory_fact:not-found".to_string(),
                action: UnifiedMemoryActionKind::Forget,
                patch: None,
            },
        )
        .await
        .expect_err("missing source id");

        assert_eq!(error.kind, UnifiedMemoryActionErrorKind::NotFound);
        assert!(error.message.contains("memory_fact:not-found"));
        assert!(error.message.contains("not found"));
        fixture.cleanup();
    }

    struct UnifiedMemoryActionFixture {
        state: Arc<AppState>,
        workspace: std::path::PathBuf,
        wiki_path: std::path::PathBuf,
        facts_path: std::path::PathBuf,
        profiles_path: std::path::PathBuf,
        project_path: String,
    }

    impl UnifiedMemoryActionFixture {
        fn new() -> Self {
            let nonce = uuid::Uuid::now_v7();
            let workspace = std::env::temp_dir().join(format!("forge-unified-action-{nonce}"));
            std::fs::create_dir_all(&workspace).expect("workspace");
            let workspace = workspace.canonicalize().expect("canonical workspace");
            let wiki_path =
                std::env::temp_dir().join(format!("forge-unified-action-wiki-{nonce}.json"));
            let facts_path =
                std::env::temp_dir().join(format!("forge-unified-action-facts-{nonce}.json"));
            let profiles_path =
                std::env::temp_dir().join(format!("forge-unified-action-profiles-{nonce}.json"));
            let mut app_state = AppState::new(Arc::new(Harness::new(workspace.clone())));
            app_state.wiki_memory = Arc::new(WikiMemoryStore::new(wiki_path.clone()));
            app_state.memory_facts = Arc::new(MemoryFactStore::new(facts_path.clone()));
            app_state.profiles = Arc::new(ProfileStore::new(profiles_path.clone()));
            let project_path = workspace.to_string_lossy().to_string();
            Self {
                state: Arc::new(app_state),
                workspace,
                wiki_path,
                facts_path,
                profiles_path,
                project_path,
            }
        }

        async fn seed_wiki_memory(&self, id: &str) {
            self.seed_wiki_memory_with_status(id, MemoryStatus::Accepted, "Archive route target")
                .await;
        }

        async fn seed_wiki_memory_with_status(&self, id: &str, status: MemoryStatus, body: &str) {
            self.seed_wiki_memory_for_project_with_status(id, &self.project_path, status, body)
                .await;
        }

        async fn seed_wiki_memory_for_project(&self, id: &str, project_path: &str) {
            self.seed_wiki_memory_for_project_with_status(
                id,
                project_path,
                MemoryStatus::Accepted,
                "Archive route target",
            )
            .await;
        }

        async fn seed_wiki_memory_for_project_with_status(
            &self,
            id: &str,
            project_path: &str,
            status: MemoryStatus,
            body: &str,
        ) {
            self.state
                .wiki_memory
                .upsert_candidate(WikiMemory {
                    id: id.to_string(),
                    category: MemoryCategory::TaskState,
                    scope: MemoryScope::Project,
                    status,
                    title: "Wiki action memory".to_string(),
                    body: body.to_string(),
                    project_path: Some(project_path.to_string()),
                    source_session_id: Some("session-action".to_string()),
                    source_message_ids: Vec::new(),
                    confidence: 0.9,
                    created_at: "1772582400000".to_string(),
                    updated_at: "1772582400000".to_string(),
                    last_used_at: None,
                    use_count: 0,
                    tags: vec!["action".to_string()],
                })
                .await
                .expect("wiki");
        }

        fn seed_memory_fact(&self, id: &str) {
            self.seed_memory_fact_for_profile(id, None);
        }

        fn seed_memory_fact_for_profile(&self, id: &str, profile_id: Option<&str>) {
            self.state
                .memory_facts
                .upsert(UpsertMemoryFactInput {
                    id: Some(id.to_string()),
                    text: "Fact action memory".to_string(),
                    tags: vec!["action".to_string()],
                    profile_id: profile_id.map(str::to_string),
                    source: Some("test".to_string()),
                })
                .expect("fact");
        }

        fn seed_profile(&self, id: &str) {
            self.state
                .profiles
                .upsert(UpsertProfileInput {
                    id: Some(id.to_string()),
                    name: id.to_string(),
                    default_provider: None,
                    default_model: None,
                    default_workspace: None,
                    api_key_overrides: None,
                })
                .expect("profile");
        }

        fn seed_continuity_experience(&self, id: &str) -> String {
            let reflection = ReflectionEvent {
                session_id: "session-action".to_string(),
                user_goal: id.to_string(),
                execution_summary: "Continuity action memory".to_string(),
                outcome: ReflectionOutcome::Completed,
                verification_summary: None,
                lessons: vec![
                    "Continuity action memory archives through unified action.".to_string()
                ],
                episode: None,
                timestamp_ms: 1772582400000,
            };
            self.state
                .continuity
                .record_event(&self.project_path, &ContinuityEvent::Reflection(reflection))
                .expect("reflection");
            let formed = self
                .state
                .continuity
                .form_experiences_for_session(&self.project_path, "session-action", 1772582400001)
                .expect("form");
            let experience_id = formed
                .first()
                .map(|experience| experience.id.clone())
                .expect("experience");
            self.state
                .continuity
                .update_experience_status(
                    &self.project_path,
                    &experience_id,
                    ExperienceStatus::Accepted,
                    Some("review-session"),
                    1772582400002,
                )
                .expect("accept");
            experience_id
        }

        fn cleanup(self) {
            let _ = std::fs::remove_dir_all(self.workspace);
            let _ = std::fs::remove_file(self.wiki_path);
            let _ = std::fs::remove_file(self.facts_path);
            let _ = std::fs::remove_file(self.profiles_path);
        }
    }
}
