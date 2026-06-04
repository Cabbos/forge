use std::sync::Arc;

use crate::agent::capability_context::ComposerCapabilitySelection;
use crate::agent::event_sink::{EventEmitter, TauriEventEmitter};
use crate::agent::provider_capabilities::{default_model, normalize_provider};
use crate::agent::session::ManualCompactResult;
use crate::agent::snapshot::save_session_snapshot;
use crate::ipc::mcp_context::McpContextSelection;
use crate::ipc::send_input_context::{
    build_prepared_send_input_turn, record_send_input_user_turn, resolve_send_input_session,
    run_reserved_send_input_turn, select_send_input_contexts, BuildPreparedSendInputTurnRequest,
    RunReservedSendInputTurnRequest, SelectSendInputContextsRequest,
};
use crate::ipc::session_builder::{build_agent_session, BuildAgentSessionRequest};
use crate::ipc::session_lifecycle::{
    emit_missing_api_key_notice, emit_restored_session_startup, emit_session_started,
    register_and_dispatch_session_start, restore_session_from_snapshot, resume_existing_session,
};
use crate::protocol::commands::SessionCreated;
use crate::settings;
use crate::state::AppState;
use crate::workspace_safety::resolve_session_workspace_path as resolve_session_working_dir;

const MANUAL_COMPACT_COMMAND: &str = "/compact";

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
    let capabilities = capabilities.unwrap_or_default();
    if is_manual_compact_request(&text, &capabilities) {
        let emitter = TauriEventEmitter::new(app_handle);
        compact_session_context_for_state(&state, &session_id, &emitter).await?;
        return Ok(());
    }

    let (s, project_path) = resolve_send_input_session(&app_handle, &state, &session_id).await?;
    let turn_guard = record_send_input_user_turn(&state, &s, &session_id, &text, &project_path)?;
    let contexts = select_send_input_contexts(SelectSendInputContextsRequest {
        state: &state,
        app_handle: &app_handle,
        session_id: &session_id,
        text: &text,
        project_path: &project_path,
        harness: &s.harness,
        capabilities,
        mcp_context_selections: mcp_context.unwrap_or_default(),
    })
    .await;
    let prepared = build_prepared_send_input_turn(BuildPreparedSendInputTurnRequest {
        session_id: &session_id,
        session: &s,
        text: &text,
        input_intent: contexts.input_intent,
        workflow: &contexts.workflow,
        ready_connector_labels: contexts.mcp_result.ready_labels,
        memory_context: contexts.memory_selection.context,
        wiki_context: contexts.project_records.context,
        continuity_context: contexts.continuity_context,
        connector_context: contexts.mcp_result.context,
    })
    .await;
    run_reserved_send_input_turn(RunReservedSendInputTurnRequest {
        state: &state,
        app_handle: &app_handle,
        session: &s,
        text: &text,
        project_path: &project_path,
        workflow: &contexts.workflow,
        prepared,
        turn_guard,
    })
    .await
}

#[tauri::command]
pub async fn compact_session_context(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<ManualCompactResult, String> {
    let emitter = TauriEventEmitter::new(app_handle);
    compact_session_context_for_state(&state, &session_id, &emitter).await
}

pub(crate) async fn compact_session_context_for_state(
    state: &Arc<AppState>,
    session_id: &str,
    emitter: &dyn EventEmitter,
) -> Result<ManualCompactResult, String> {
    let session = state
        .sessions
        .read()
        .await
        .get(session_id)
        .cloned()
        .ok_or_else(|| format!("Session not found: {session_id}"))?;
    session.compact_now_with_emitter(emitter).await
}

pub(crate) fn is_manual_compact_request(
    text: &str,
    capabilities: &[ComposerCapabilitySelection],
) -> bool {
    text.trim().eq_ignore_ascii_case(MANUAL_COMPACT_COMMAND)
        || capabilities.iter().any(|capability| match capability {
            ComposerCapabilitySelection::SlashCommand { command } => {
                command.trim().eq_ignore_ascii_case(MANUAL_COMPACT_COMMAND)
            }
            ComposerCapabilitySelection::FileReference { .. } => false,
        })
}

#[cfg(test)]
#[path = "handlers_tests.rs"]
mod tests;
