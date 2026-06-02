use std::sync::Arc;

use crate::continuity::ExperienceMemory;
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
