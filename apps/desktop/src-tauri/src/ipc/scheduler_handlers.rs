//! IPC handlers for scheduler (Phase 5-C MVP).
//!
//! Commands:
//! - `list_scheduled_tasks` — returns tasks + recent history + load_error.
//! - `upsert_scheduled_task` — create or update a scheduled task.
//! - `delete_scheduled_task` — delete a scheduled task by id.
//! - `set_scheduled_task_enabled` — enable/disable a scheduled task.
//! - `run_scheduled_task_now` — run a task immediately (MVP: records history).

use crate::scheduler::{ScheduledTask, SchedulerListPayload, UpsertScheduledTaskInput};
use crate::state::AppState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn list_scheduled_tasks(
    state: State<'_, Arc<AppState>>,
) -> Result<SchedulerListPayload, String> {
    let store = &state.scheduler;
    Ok(store.list_payload())
}

#[tauri::command]
pub async fn upsert_scheduled_task(
    state: State<'_, Arc<AppState>>,
    input: UpsertScheduledTaskInput,
) -> Result<ScheduledTask, String> {
    let store = &state.scheduler;
    store.upsert(input)
}

#[tauri::command]
pub async fn delete_scheduled_task(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<bool, String> {
    let store = &state.scheduler;
    store.delete(&id)
}

#[tauri::command]
pub async fn set_scheduled_task_enabled(
    state: State<'_, Arc<AppState>>,
    id: String,
    enabled: bool,
) -> Result<bool, String> {
    let store = &state.scheduler;
    store.set_enabled(&id, enabled)
}

#[tauri::command]
pub async fn run_scheduled_task_now(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<ScheduledTask, String> {
    let store = &state.scheduler;
    store.run_task_now(&id)
}
