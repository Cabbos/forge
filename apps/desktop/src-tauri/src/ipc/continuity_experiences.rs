use std::sync::Arc;

use crate::agent::time::now_ms;
use crate::continuity::{ExperienceMemory, ExperienceStatus};
use crate::ipc::workspace_files::working_dir_for_request_or_explicit;
use crate::state::AppState;

const CONTINUITY_RECALL_DEFAULT_LIMIT: usize = 8;
const CONTINUITY_RECALL_MAX_LIMIT: usize = 20;

pub(crate) async fn list_continuity_experiences_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<Vec<ExperienceMemory>, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let project_path = working_dir.to_string_lossy().to_string();
    state.continuity.list_experiences_for_project(&project_path)
}

#[tauri::command]
pub async fn list_continuity_experiences(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<Vec<ExperienceMemory>, String> {
    list_continuity_experiences_for_request(&state, session_id.as_deref(), working_dir.as_deref())
        .await
}

pub(crate) async fn search_continuity_experiences_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
    query: &str,
    limit: Option<usize>,
) -> Result<Vec<ExperienceMemory>, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let project_path = working_dir.to_string_lossy().to_string();
    let limit = limit
        .unwrap_or(CONTINUITY_RECALL_DEFAULT_LIMIT)
        .min(CONTINUITY_RECALL_MAX_LIMIT);
    state
        .continuity
        .search_experiences_for_project(&project_path, query, limit)
}

#[tauri::command]
pub async fn search_continuity_experiences(
    state: tauri::State<'_, Arc<AppState>>,
    query: String,
    session_id: Option<String>,
    working_dir: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ExperienceMemory>, String> {
    search_continuity_experiences_for_request(
        &state,
        session_id.as_deref(),
        working_dir.as_deref(),
        &query,
        limit,
    )
    .await
}

pub(crate) async fn update_continuity_experience_status_for_request(
    state: &Arc<AppState>,
    experience_id: &str,
    status: ExperienceStatus,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<ExperienceMemory, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let project_path = working_dir.to_string_lossy().to_string();
    state.continuity.update_experience_status(
        &project_path,
        experience_id,
        status,
        session_id,
        now_ms(),
    )
}

#[tauri::command]
pub async fn update_continuity_experience_status(
    state: tauri::State<'_, Arc<AppState>>,
    experience_id: String,
    status: ExperienceStatus,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<ExperienceMemory, String> {
    update_continuity_experience_status_for_request(
        &state,
        &experience_id,
        status,
        session_id.as_deref(),
        working_dir.as_deref(),
    )
    .await
}
