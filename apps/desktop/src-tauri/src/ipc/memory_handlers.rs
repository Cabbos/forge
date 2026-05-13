use std::sync::Arc;

use tauri::Emitter;

use crate::memory::{
    MemoryListFilter, MemoryPatch, MemoryScope, SelectedContextMemory, WikiMemory,
};
use crate::protocol::events::StreamEvent;
use crate::state::AppState;

#[tauri::command]
pub async fn list_memories(
    state: tauri::State<'_, Arc<AppState>>,
    scope: Option<String>,
    project_path: Option<String>,
) -> Result<Vec<WikiMemory>, String> {
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
) -> Result<Vec<SelectedContextMemory>, String> {
    Ok(state
        .wiki_memory
        .select(&message, project_path.as_deref(), 8)
        .await)
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
        let _ = app_handle.emit(
            "session-output",
            StreamEvent::MemoryUpdated {
                session_id: session_id.to_string(),
                memory: memory.clone(),
            },
        );
    }
}
