use std::sync::Arc;

use crate::adapters::build_adapter;
use crate::agent::provider_capabilities::{missing_api_key_message, normalize_provider};
use crate::agent::session::AgentSession;
use crate::agent::snapshot::{list_session_snapshots, save_session_snapshot, AgentSessionSnapshot};
use crate::harness::Harness;
use crate::protocol::commands::SessionInfo;
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;
use crate::settings;
use crate::state::AppState;
use crate::workspace_safety::resolve_workspace_path as resolve_safe_workspace_path;

pub(crate) fn emit_missing_api_key_notice(
    app_handle: &tauri::AppHandle,
    session_id: &str,
    provider: &str,
) {
    crate::transcript::emit_stream_event(
        app_handle,
        StreamEvent::Error {
            session_id: session_id.to_string(),
            block_id: BlockId::new().to_string(),
            message: missing_api_key_message(provider),
            code: "missing_api_key".to_string(),
        },
    );
}

pub(crate) async fn upgrade_missing_key_session_if_possible(
    app_handle: &tauri::AppHandle,
    state: &Arc<AppState>,
    session: Arc<AgentSession>,
) -> Result<Arc<AgentSession>, String> {
    if !session.is_waiting_for_api_key() {
        return Ok(session);
    }

    let snapshot = session.snapshot();
    let provider = normalize_provider(Some(&snapshot.provider));
    let credentials = settings::detect_credentials(&provider);
    if credentials.api_key.trim().is_empty() {
        return Ok(session);
    }

    let working_dir = resolve_safe_workspace_path(&snapshot.working_dir)?;
    let harness = Arc::new(Harness::new_with_pending(
        working_dir.clone(),
        state.pending_confirms.clone(),
    ));
    let model_str = snapshot.model.clone();
    let external_tools = harness.external_mcp_tool_definitions().await;
    let adapter = build_adapter(
        &provider,
        &credentials.api_key,
        &model_str,
        credentials.api_base.as_deref(),
        external_tools,
    )?;
    let system_prompt = harness.build_system_prompt(&provider, &working_dir).await;
    let upgraded = AgentSession::new(
        snapshot.session_id.clone(),
        provider.clone(),
        adapter,
        harness,
        system_prompt,
        snapshot.context_window_tokens,
    );
    upgraded.restore_state(snapshot.messages, snapshot.summary, snapshot.latest_turn);
    let upgraded = Arc::new(upgraded);
    state
        .register_session(snapshot.session_id.clone(), upgraded.clone())
        .await;
    let _ = upgraded
        .harness
        .dispatch_session_start_event(&snapshot.session_id)
        .await;
    crate::transcript::emit_stream_event(
        app_handle,
        StreamEvent::SessionStarted {
            session_id: snapshot.session_id,
            agent_type: provider,
            model: model_str,
            context_window_tokens: upgraded.context_window_tokens,
        },
    );
    Ok(upgraded)
}

pub(crate) async fn save_session_snapshot_with_workflow(
    state: &Arc<AppState>,
    session: &AgentSession,
) -> Result<(), String> {
    let snapshot = session_snapshot_with_workflow_state(state, session).await;
    save_session_snapshot(&snapshot)
}

pub(crate) async fn session_snapshot_with_workflow_state(
    state: &Arc<AppState>,
    session: &AgentSession,
) -> AgentSessionSnapshot {
    let latest_workflow = state.workflow_states.read().await.get(&session.id).cloned();
    let latest_delivery = state.delivery_states.read().await.get(&session.id).cloned();
    let mut snapshot = session.snapshot();
    if let Some(workflow) = latest_workflow {
        snapshot = snapshot.with_latest_workflow(workflow);
    }
    if let Some(delivery) = latest_delivery {
        snapshot = snapshot.with_latest_delivery(delivery);
    }
    snapshot
}

pub(crate) async fn list_session_infos_for_state(
    state: &Arc<AppState>,
    snapshots: Vec<AgentSessionSnapshot>,
) -> Vec<SessionInfo> {
    let mut by_id = std::collections::HashMap::new();
    for snapshot in snapshots {
        by_id.insert(
            snapshot.session_id.clone(),
            SessionInfo {
                id: snapshot.session_id,
                provider: snapshot.provider,
                model: snapshot.model,
                status: "stopped".to_string(),
                created_at: String::new(),
                working_dir: Some(snapshot.working_dir),
                created_at_ms: Some(snapshot.created_at_ms),
                updated_at_ms: Some(snapshot.updated_at_ms),
                context_window_tokens: snapshot.context_window_tokens,
                latest_workflow: snapshot.latest_workflow,
                latest_delivery: snapshot.latest_delivery,
            },
        );
    }

    let sessions = state.sessions.read().await;
    let workflow_states = state.workflow_states.read().await;
    let delivery_states = state.delivery_states.read().await;
    for (id, session) in sessions.iter() {
        let status = session.status.lock();
        let snapshot = session.snapshot();
        by_id.insert(
            id.clone(),
            SessionInfo {
                id: id.clone(),
                provider: session.agent_type.clone(),
                model: session.model_id.clone(),
                status: status.as_str().to_string(),
                created_at: String::new(),
                working_dir: Some(snapshot.working_dir),
                created_at_ms: Some(snapshot.created_at_ms),
                updated_at_ms: Some(snapshot.updated_at_ms),
                context_window_tokens: snapshot.context_window_tokens,
                latest_workflow: workflow_states
                    .get(id)
                    .cloned()
                    .or(snapshot.latest_workflow),
                latest_delivery: delivery_states
                    .get(id)
                    .cloned()
                    .or(snapshot.latest_delivery),
            },
        );
    }

    let mut result: Vec<_> = by_id.into_values().collect();
    result.sort_by(|a, b| {
        b.updated_at_ms
            .unwrap_or(0)
            .cmp(&a.updated_at_ms.unwrap_or(0))
    });
    result
}

#[tauri::command]
pub async fn list_sessions(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<SessionInfo>, String> {
    let snapshots = list_session_snapshots()?;
    Ok(list_session_infos_for_state(&state, snapshots).await)
}
