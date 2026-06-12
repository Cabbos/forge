//! IPC handlers for user profile management.

use std::sync::Arc;

use crate::profile::{ForgeProfile, ProfileListPayload, UpsertProfileInput};
use crate::state::AppState;

#[tauri::command]
pub async fn list_profiles(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<ProfileListPayload, String> {
    Ok(state.profiles.list_payload())
}

#[tauri::command]
pub async fn upsert_profile(
    state: tauri::State<'_, Arc<AppState>>,
    input: UpsertProfileInput,
) -> Result<ForgeProfile, String> {
    state.profiles.upsert(input)
}

#[tauri::command]
pub async fn delete_profile(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<bool, String> {
    state.profiles.delete(&id)
}

#[tauri::command]
pub async fn set_active_profile(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<ProfileListPayload, String> {
    state.profiles.set_active(&id)?;
    Ok(state.profiles.list_payload())
}
