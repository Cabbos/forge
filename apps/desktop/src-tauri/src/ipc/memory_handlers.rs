use std::sync::Arc;

use crate::ipc::workspace::resolve_bound_working_dir;
use crate::memory::facts::{MemoryFact, UpsertMemoryFactInput, UpsertMemoryFactOutput};
use crate::memory::{
    MemoryListFilter, MemoryPatch, MemoryScope, SelectedContextMemory, WikiMemory,
};
use crate::protocol::events::StreamEvent;
use crate::state::AppState;
use crate::workspace_safety::resolve_optional_workspace_path;

#[tauri::command]
pub async fn list_memories(
    state: tauri::State<'_, Arc<AppState>>,
    scope: Option<String>,
    project_path: Option<String>,
    session_id: Option<String>,
) -> Result<Vec<WikiMemory>, String> {
    let project_path = normalize_memory_project_path_for_request(
        &state,
        session_id.as_deref(),
        project_path.as_deref(),
    )
    .await?;
    let filter = MemoryListFilter {
        scope: parse_scope(scope.as_deref())?,
        project_path,
    };
    Ok(state.wiki_memory.list(filter).await)
}

#[tauri::command]
pub async fn update_memory(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
    memory_id: String,
    patch: MemoryPatch,
) -> Result<WikiMemory, String> {
    let memory = state.wiki_memory.update(&memory_id, patch).await?;
    emit_memory_updated(&app_handle, session_id.as_deref(), &memory);
    Ok(memory)
}

#[tauri::command]
pub async fn forget_memory(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
    memory_id: String,
) -> Result<WikiMemory, String> {
    let memory = state.wiki_memory.forget(&memory_id).await?;
    emit_memory_updated(&app_handle, session_id.as_deref(), &memory);
    Ok(memory)
}

#[tauri::command]
pub async fn pin_memory(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
    memory_id: String,
) -> Result<WikiMemory, String> {
    let memory = state.wiki_memory.pin(&memory_id).await?;
    emit_memory_updated(&app_handle, session_id.as_deref(), &memory);
    Ok(memory)
}

#[tauri::command]
pub async fn select_context_memories(
    state: tauri::State<'_, Arc<AppState>>,
    message: String,
    project_path: Option<String>,
    session_id: Option<String>,
) -> Result<Vec<SelectedContextMemory>, String> {
    let project_path = normalize_memory_project_path_for_request(
        &state,
        session_id.as_deref(),
        project_path.as_deref(),
    )
    .await?;
    Ok(state
        .wiki_memory
        .select(&message, project_path.as_deref(), 8)
        .await)
}

async fn normalize_memory_project_path_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    project_path: Option<&str>,
) -> Result<Option<String>, String> {
    if session_id.is_some() {
        return resolve_bound_working_dir(state, session_id, project_path)
            .await
            .map(|path| Some(path.to_string_lossy().to_string()));
    }
    normalize_memory_project_path(project_path)
}

fn normalize_memory_project_path(project_path: Option<&str>) -> Result<Option<String>, String> {
    resolve_optional_workspace_path(project_path)
        .map(|path| path.map(|path| path.to_string_lossy().to_string()))
}

fn parse_scope(scope: Option<&str>) -> Result<Option<MemoryScope>, String> {
    let Some(scope) = scope else {
        return Ok(None);
    };

    let scope = scope.trim().to_lowercase();
    if scope.is_empty() {
        return Ok(None);
    }

    match scope.as_str() {
        "session" => Ok(Some(MemoryScope::Session)),
        "user_profile" => Ok(Some(MemoryScope::UserProfile)),
        "project" => Ok(Some(MemoryScope::Project)),
        "document" => Ok(Some(MemoryScope::Document)),
        other => Err(format!("Unsupported memory scope: {other}")),
    }
}

fn emit_memory_updated(
    app_handle: &tauri::AppHandle,
    session_id: Option<&str>,
    memory: &WikiMemory,
) {
    if let Some(session_id) = session_id {
        crate::transcript::emit_stream_event(
            app_handle,
            StreamEvent::MemoryUpdated {
                session_id: session_id.to_string(),
                memory: memory.clone(),
            },
        );
    }
}

// ── Memory Facts IPC (Phase 5-A) ─────────────────────────────────────────────

#[tauri::command]
pub async fn list_memory_facts(
    state: tauri::State<'_, Arc<AppState>>,
    query: Option<String>,
) -> Result<Vec<MemoryFact>, String> {
    Ok(state.memory_facts.list(query.as_deref()))
}

#[tauri::command]
pub async fn upsert_memory_fact(
    state: tauri::State<'_, Arc<AppState>>,
    input: UpsertMemoryFactInput,
) -> Result<UpsertMemoryFactOutput, String> {
    state.memory_facts.upsert(input)
}

#[tauri::command]
pub async fn delete_memory_fact(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<bool, String> {
    state.memory_facts.delete(&id)
}

#[cfg(test)]
mod tests {
    use super::normalize_memory_project_path;
    use super::normalize_memory_project_path_for_request;
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
        let path = std::env::temp_dir().join(format!("forge-memory-handler-{name}-{nanos}"));
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn memory_project_path_rejects_broad_roots() {
        let err = normalize_memory_project_path(Some("/")).expect_err("broad root");

        assert_eq!(err, BROAD_WORKSPACE_REASON);
    }

    #[test]
    fn memory_project_path_is_canonicalized() {
        let workspace = temp_dir("canonical");
        let input = workspace.join(".").to_string_lossy().to_string();

        let resolved = normalize_memory_project_path(Some(&input))
            .expect("resolve project path")
            .expect("project path");

        assert_eq!(
            PathBuf::from(resolved),
            workspace.canonicalize().expect("canonical workspace")
        );
    }

    #[test]
    fn blank_memory_project_path_is_ignored() {
        let resolved =
            normalize_memory_project_path(Some("  ")).expect("blank path should not fail");

        assert_eq!(resolved, None);
    }

    #[tokio::test]
    async fn memory_project_path_with_stale_session_does_not_fallback_to_explicit_path() {
        let workspace = temp_dir("stale-session");
        let state = Arc::new(AppState::new(Arc::new(Harness::new(workspace.clone()))));

        let err = normalize_memory_project_path_for_request(
            &state,
            Some("missing-session"),
            Some(workspace.to_str().expect("utf8")),
        )
        .await
        .expect_err("stale session should not fall back to explicit path");

        assert!(err.contains("会话"));
    }
}
