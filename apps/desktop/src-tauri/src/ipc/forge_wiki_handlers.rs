use std::sync::Arc;

use crate::forge_wiki::model::{ForgeWikiPage, ForgeWikiState, SelectedForgeWikiPage};
use crate::state::AppState;

#[tauri::command]
pub async fn get_forge_wiki_state(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
) -> Result<ForgeWikiState, String> {
    state.forge_wiki.get_state(&project_path).await
}

#[tauri::command]
pub async fn init_forge_wiki(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
) -> Result<ForgeWikiState, String> {
    state.forge_wiki.init(&project_path).await
}

#[tauri::command]
pub async fn list_forge_wiki_pages(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
) -> Result<Vec<ForgeWikiPage>, String> {
    state.forge_wiki.list_pages(&project_path).await
}

#[tauri::command]
pub async fn read_forge_wiki_page(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
    page_path: String,
) -> Result<String, String> {
    state.forge_wiki.read_page(&project_path, &page_path).await
}

#[tauri::command]
pub async fn select_forge_wiki_context(
    state: tauri::State<'_, Arc<AppState>>,
    project_path: String,
    message: String,
) -> Result<Vec<SelectedForgeWikiPage>, String> {
    state
        .forge_wiki
        .select_context(&project_path, &message, 4)
        .await
}
