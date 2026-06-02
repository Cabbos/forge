use std::sync::Arc;

use crate::agent::capability_context::ComposerCapabilitySelection;
use crate::agent::provider_capabilities::{default_model, normalize_provider};
use crate::agent::snapshot::save_session_snapshot;
use crate::ipc::mcp_context::{build_mcp_context, McpContextSelection};
use crate::ipc::project_records::select_send_input_project_records_context;
use crate::ipc::send_input_context::{
    finalize_send_input_turn, prepare_send_input_turn_context,
    reserve_turn_then_record_user_message, resolve_send_input_session,
    select_send_input_memory_context, setup_send_input_workflow, PrepareSendInputTurnRequest,
};
use crate::ipc::send_input_continuity::record_send_input_user_message_continuity;
use crate::ipc::session_builder::{build_agent_session, BuildAgentSessionRequest};
use crate::ipc::session_lifecycle::{
    emit_missing_api_key_notice, emit_restored_session_startup, emit_session_started,
    register_and_dispatch_session_start, restore_session_from_snapshot, resume_existing_session,
};
use crate::protocol::commands::SessionCreated;
use crate::protocol::events::StreamEvent;
use crate::settings;
use crate::state::AppState;
use crate::workspace_safety::resolve_session_workspace_path as resolve_session_working_dir;

#[tauri::command]
pub async fn create_session(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    working_dir: String,
    provider: Option<String>,
    api_key: String,
    model: Option<String>,
) -> Result<SessionCreated, String> {
    let session_id = uuid::Uuid::now_v7().to_string();
    let provider = normalize_provider(provider.as_deref());
    let credentials = settings::detect_credentials(&provider);

    let key = if api_key.is_empty() {
        credentials.api_key
    } else {
        api_key
    };

    let model_str = model
        .or(credentials.model)
        .unwrap_or_else(|| default_model(&provider).to_string());
    let working_dir = resolve_session_working_dir(&working_dir)?;
    let (session, missing_api_key) = build_agent_session(BuildAgentSessionRequest {
        session_id: session_id.clone(),
        provider: provider.clone(),
        model: model_str.clone(),
        api_key: &key,
        api_base: credentials.api_base.as_deref(),
        working_dir: &working_dir,
        pending_confirms: state.pending_confirms.clone(),
        existing_context_window_tokens: None,
    })
    .await?;

    emit_session_started(
        &app_handle,
        &session_id,
        &provider,
        &model_str,
        session.context_window_tokens,
    );
    if missing_api_key {
        emit_missing_api_key_notice(&app_handle, &session_id, &provider);
    }

    let session = Arc::new(session);
    if let Err(error) = save_session_snapshot(&session.snapshot()) {
        crate::app_log!("WARN", "[session_snapshot] {}", error);
    }
    register_and_dispatch_session_start(&state, session, &session_id).await;
    Ok(SessionCreated {
        session_id,
        provider,
        model: model_str,
        missing_api_key,
    })
}

#[tauri::command]
pub async fn resume_session(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<SessionCreated, String> {
    let existing_session = state.sessions.read().await.get(&session_id).cloned();
    if let Some(session) = existing_session {
        return resume_existing_session(&app_handle, &state, &session_id, session).await;
    }

    let restored = restore_session_from_snapshot(&state, &session_id).await?;
    emit_restored_session_startup(&state, &app_handle, &session_id, &restored).await;
    Ok(SessionCreated {
        session_id,
        provider: restored.provider,
        model: restored.model,
        missing_api_key: restored.missing_api_key,
    })
}

#[tauri::command]
pub async fn send_input(
    state: tauri::State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    session_id: String,
    text: String,
    mcp_context: Option<Vec<McpContextSelection>>,
    capabilities: Option<Vec<ComposerCapabilitySelection>>,
) -> Result<(), String> {
    let (s, project_path) = resolve_send_input_session(&app_handle, &state, &session_id).await?;
    record_send_input_user_message_continuity(&state, &project_path, &session_id, &text);
    let turn_guard = reserve_turn_then_record_user_message(&s, &session_id, &text, |event| {
        if let Err(error) = crate::transcript::append_stream_event(&event) {
            crate::app_log!("WARN", "[transcript] {}", error);
        }
    })?;
    let capabilities = capabilities.unwrap_or_default();
    let mcp_context_selections = mcp_context.unwrap_or_default();
    let (input_intent, workflow) =
        setup_send_input_workflow(&state, &app_handle, &session_id, &text, &capabilities).await;
    let project_records =
        select_send_input_project_records_context(&state, &text, &project_path).await;
    if !project_records.selected.is_empty() {
        crate::transcript::emit_stream_event(
            &app_handle,
            StreamEvent::ForgeWikiContextSelected {
                session_id: session_id.clone(),
                selected: project_records.selected.clone(),
            },
        );
    }
    let memory_selection = select_send_input_memory_context(&state, &text, &project_path).await;
    crate::transcript::emit_stream_event(
        &app_handle,
        StreamEvent::MemorySelection {
            session_id: session_id.clone(),
            selected: memory_selection.selected.clone(),
        },
    );
    let mcp_context_result = build_mcp_context(
        &s.harness,
        &mcp_context_selections,
        &app_handle,
        &session_id,
    )
    .await;
    let ready_connector_labels = mcp_context_result.ready_labels.clone();
    let prepared = prepare_send_input_turn_context(PrepareSendInputTurnRequest {
        session_id: &session_id,
        session: &s,
        text: &text,
        input_intent,
        workflow: &workflow,
        ready_connector_labels,
        memory_context: memory_selection.context,
        wiki_context: project_records.context,
        connector_context: mcp_context_result.context,
    })
    .await;
    let result = s
        .send_message_with_reserved_turn(
            &text,
            &app_handle,
            prepared.hidden_contexts,
            Some(prepared.turn_metadata),
            Some(&prepared.activation_text),
            turn_guard,
        )
        .await;
    finalize_send_input_turn(
        &state,
        &app_handle,
        &s,
        &text,
        &project_path,
        &workflow,
        &result,
    )
    .await;
    result
}

#[cfg(test)]
#[path = "handlers_tests.rs"]
mod tests;
