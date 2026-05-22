use std::sync::Arc;

use crate::protocol::events::StreamEvent;
use crate::state::AppState;
use crate::workflow::{workflow_state_from_override, WorkflowOverrideAction, WorkflowState};

#[tauri::command]
pub async fn get_workflow_state(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<Option<WorkflowState>, String> {
    Ok(state.workflow_states.read().await.get(&session_id).cloned())
}

#[tauri::command]
pub async fn override_workflow_route(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    action: WorkflowOverrideAction,
) -> Result<WorkflowState, String> {
    let workflow = workflow_state_from_override(&session_id, action, now_ms());
    state
        .workflow_states
        .write()
        .await
        .insert(session_id.clone(), workflow.clone());
    crate::transcript::emit_stream_event(
        &app_handle,
        StreamEvent::WorkflowUpdated {
            session_id,
            state: workflow.clone(),
        },
    );
    Ok(workflow)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
