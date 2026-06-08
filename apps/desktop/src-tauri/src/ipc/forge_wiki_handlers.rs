use std::sync::Arc;

use crate::forge_wiki::model::{
    ForgeWikiPage, ForgeWikiState, ForgeWikiUpdateProposal, SelectedForgeWikiPage,
};
use crate::ipc::workspace::resolve_bound_working_dir;
use crate::state::AppState;
use crate::workspace_safety::resolve_workspace_path;

#[tauri::command]
pub async fn get_forge_wiki_state(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
    session_id: Option<String>,
) -> Result<ForgeWikiState, String> {
    let project_path = normalize_forge_wiki_project_path_for_request(
        &state,
        session_id.as_deref(),
        Some(&project_path),
    )
    .await?;
    state.forge_wiki.get_state(&project_path).await
}

#[tauri::command]
pub async fn init_forge_wiki(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
    session_id: Option<String>,
) -> Result<ForgeWikiState, String> {
    let project_path = normalize_forge_wiki_project_path_for_request(
        &state,
        session_id.as_deref(),
        Some(&project_path),
    )
    .await?;
    state.forge_wiki.init(&project_path).await
}

#[tauri::command]
pub async fn list_forge_wiki_pages(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
    session_id: Option<String>,
) -> Result<Vec<ForgeWikiPage>, String> {
    let project_path = normalize_forge_wiki_project_path_for_request(
        &state,
        session_id.as_deref(),
        Some(&project_path),
    )
    .await?;
    state.forge_wiki.list_pages(&project_path).await
}

#[tauri::command]
pub async fn read_forge_wiki_page(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
    page_path: String,
    session_id: Option<String>,
) -> Result<String, String> {
    let project_path = normalize_forge_wiki_project_path_for_request(
        &state,
        session_id.as_deref(),
        Some(&project_path),
    )
    .await?;
    state.forge_wiki.read_page(&project_path, &page_path).await
}

#[tauri::command]
pub async fn select_forge_wiki_context(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
    message: String,
    session_id: Option<String>,
) -> Result<Vec<SelectedForgeWikiPage>, String> {
    let project_path = normalize_forge_wiki_project_path_for_request(
        &state,
        session_id.as_deref(),
        Some(&project_path),
    )
    .await?;
    state
        .forge_wiki
        .select_context(&project_path, &message, 4)
        .await
}

#[tauri::command]
pub async fn create_forge_wiki_update_proposal(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
    session_id: Option<String>,
    target_pages: Vec<String>,
    title: String,
    summary: String,
) -> Result<ForgeWikiUpdateProposal, String> {
    let project_path = normalize_forge_wiki_project_path_for_request(
        &state,
        session_id.as_deref(),
        Some(&project_path),
    )
    .await?;
    state
        .forge_wiki
        .create_update_proposal(
            &project_path,
            session_id.as_deref(),
            target_pages,
            title,
            summary,
        )
        .await
}

#[tauri::command]
pub async fn accept_forge_wiki_update_proposal(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
    proposal_id: String,
    session_id: Option<String>,
) -> Result<ForgeWikiUpdateProposal, String> {
    let project_path = normalize_forge_wiki_project_path_for_request(
        &state,
        session_id.as_deref(),
        Some(&project_path),
    )
    .await?;
    state
        .forge_wiki
        .accept_update_proposal(&project_path, &proposal_id)
        .await
}

#[tauri::command]
pub async fn discard_forge_wiki_update_proposal(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
    proposal_id: String,
    session_id: Option<String>,
) -> Result<ForgeWikiUpdateProposal, String> {
    let project_path = normalize_forge_wiki_project_path_for_request(
        &state,
        session_id.as_deref(),
        Some(&project_path),
    )
    .await?;
    state
        .forge_wiki
        .discard_update_proposal(&project_path, &proposal_id)
        .await
}

async fn normalize_forge_wiki_project_path_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    project_path: Option<&str>,
) -> Result<String, String> {
    if session_id.is_some() {
        return resolve_bound_working_dir(state, session_id, project_path)
            .await
            .map(|path| path.to_string_lossy().to_string());
    }

    let Some(project_path) = project_path else {
        return Err("当前请求没有绑定工作空间，请先选择项目或从已有会话发起。".to_string());
    };
    normalize_forge_wiki_project_path(project_path)
}

fn normalize_forge_wiki_project_path(project_path: &str) -> Result<String, String> {
    resolve_workspace_path(project_path).map(|path| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::normalize_forge_wiki_project_path;
    use super::normalize_forge_wiki_project_path_for_request;
    use crate::harness::Harness;
    use crate::state::AppState;
    use crate::workspace_safety::BROAD_WORKSPACE_REASON;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("forge-wiki-handler-{name}-{nanos}"));
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn forge_wiki_project_path_rejects_broad_roots() {
        let err = normalize_forge_wiki_project_path("/").expect_err("broad root");

        assert_eq!(err, BROAD_WORKSPACE_REASON);
    }

    #[test]
    fn forge_wiki_project_path_is_canonicalized() {
        let workspace = temp_dir("canonical");
        let input = workspace.join(".").to_string_lossy().to_string();

        let resolved = normalize_forge_wiki_project_path(&input).expect("resolve project path");

        assert_eq!(
            PathBuf::from(resolved),
            workspace.canonicalize().expect("canonical workspace")
        );
    }

    #[tokio::test]
    async fn forge_wiki_project_path_with_stale_session_does_not_fallback_to_explicit_path() {
        let workspace = temp_dir("stale-session");
        let state = Arc::new(AppState::new(Arc::new(Harness::new(workspace.clone()))));

        let err = normalize_forge_wiki_project_path_for_request(
            &state,
            Some("missing-session"),
            Some(workspace.to_str().expect("utf8")),
        )
        .await
        .expect_err("stale session should not fall back to explicit path");

        assert!(err.contains("会话"));
    }
}
