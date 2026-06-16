use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::agent::capability_context::ComposerCapabilitySelection;
use crate::agent::event_sink::TauriEventEmitter;
use crate::gateway::client::{
    build_complete_session_input_request, build_list_session_inputs_request, GatewayClient,
};
use crate::gateway::protocol::{CompleteSessionInputResult, GatewayReply, ListSessionInputsResult};
use crate::gateway::server::default_socket_path;
use crate::gateway::session_input::SessionInputRecord;
use crate::ipc::handlers::{compact_session_context_for_state, is_manual_compact_request};
use crate::ipc::send_input_context::{
    build_prepared_send_input_turn, record_send_input_user_turn, resolve_send_input_session,
    run_reserved_send_input_turn, select_send_input_contexts, BuildPreparedSendInputTurnRequest,
    RunReservedSendInputTurnRequest, SelectSendInputContextsRequest,
};
use crate::state::AppState;
use tauri::Manager;

const SESSION_INPUT_POLL_INTERVAL_SECS: u64 = 2;
const SESSION_INPUT_POLL_LIMIT: usize = 8;

pub(crate) fn spawn_gateway_session_input_poller(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        while let Some(state) = app_handle.try_state::<Arc<AppState>>() {
            let state = state.inner().clone();
            poll_gateway_session_inputs_once(&state, &app_handle).await;
            tokio::time::sleep(Duration::from_secs(SESSION_INPUT_POLL_INTERVAL_SECS)).await;
        }
    });
}

pub(crate) async fn poll_gateway_session_inputs_once(
    state: &Arc<AppState>,
    app_handle: &tauri::AppHandle,
) {
    let session_ids = live_session_ids(state).await;
    if session_ids.is_empty() {
        return;
    }

    let socket_path = default_socket_path();
    if !socket_path.exists() {
        return;
    }

    let inputs = match list_gateway_session_inputs(&socket_path, session_ids).await {
        Ok(inputs) => inputs,
        Err(error) => {
            crate::app_log!("WARN", "[gateway] failed to list session inputs: {error}");
            return;
        }
    };

    for input in inputs {
        if let Err(error) = accept_gateway_session_input(state, app_handle, input).await {
            crate::app_log!("WARN", "[gateway] failed to consume session input: {error}");
        }
    }
}

async fn live_session_ids(state: &Arc<AppState>) -> Vec<String> {
    let mut session_ids = state
        .sessions
        .read()
        .await
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    session_ids.sort();
    session_ids
}

async fn list_gateway_session_inputs(
    socket_path: &Path,
    session_ids: Vec<String>,
) -> Result<Vec<SessionInputRecord>, String> {
    let request = build_list_session_inputs_request(session_ids, SESSION_INPUT_POLL_LIMIT)?;
    let mut client = GatewayClient::connect(&socket_path.to_path_buf()).await?;
    let reply = client.send(request).await?;
    match reply {
        GatewayReply::Ok(response) => {
            let result: ListSessionInputsResult = serde_json::from_value(response.result)
                .map_err(|error| format!("deserialize list_session_inputs result: {error}"))?;
            Ok(result.inputs)
        }
        GatewayReply::Err(error) => Err(format!("gateway error: {}", error.error.message)),
    }
}

async fn complete_gateway_session_input(
    input_id: &str,
) -> Result<CompleteSessionInputResult, String> {
    let request = build_complete_session_input_request(input_id)?;
    let socket_path = default_socket_path();
    let mut client = GatewayClient::connect(&socket_path).await?;
    let reply = client.send(request).await?;
    match reply {
        GatewayReply::Ok(response) => serde_json::from_value(response.result)
            .map_err(|error| format!("deserialize complete_session_input result: {error}")),
        GatewayReply::Err(error) => Err(format!("gateway error: {}", error.error.message)),
    }
}

async fn accept_gateway_session_input(
    state: &Arc<AppState>,
    app_handle: &tauri::AppHandle,
    input: SessionInputRecord,
) -> Result<(), String> {
    let prepared = prepare_gateway_session_input(&input)?;
    let session_id = prepared.session_id;
    let text = prepared.text;

    let capabilities = Vec::<ComposerCapabilitySelection>::new();
    if is_manual_compact_request(&text, &capabilities) {
        let emitter = TauriEventEmitter::new(app_handle.clone());
        compact_session_context_for_state(state, &session_id, &emitter).await?;
        complete_gateway_session_input(&input.id).await?;
        return Ok(());
    }

    let (session, project_path) =
        resolve_send_input_session(app_handle, state, &session_id).await?;
    let turn_guard =
        record_send_input_user_turn(state, &session, &session_id, &text, &project_path)?;
    if let Err(error) = complete_gateway_session_input(&input.id).await {
        crate::app_log!(
            "WARN",
            "[gateway] failed to complete accepted session input '{}': {}",
            input.id,
            error
        );
    }

    let contexts = select_send_input_contexts(SelectSendInputContextsRequest {
        state,
        app_handle,
        session_id: &session_id,
        text: &text,
        project_path: &project_path,
        harness: &session.harness,
        capabilities,
        mcp_context_selections: Vec::new(),
    })
    .await;
    let prepared = build_prepared_send_input_turn(BuildPreparedSendInputTurnRequest {
        session_id: &session_id,
        session: &session,
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
        state,
        app_handle,
        session: &session,
        text: &text,
        project_path: &project_path,
        workflow: &contexts.workflow,
        prepared,
        turn_guard,
    })
    .await
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedGatewaySessionInput {
    session_id: String,
    text: String,
}

fn prepare_gateway_session_input(
    input: &SessionInputRecord,
) -> Result<PreparedGatewaySessionInput, String> {
    let session_id = input.session_id.trim().to_string();
    if session_id.is_empty() {
        return Err(format!("input {} has blank session_id", input.id));
    }
    let text = input.message.trim().to_string();
    if text.is_empty() {
        return Err(format!("input {} has blank message", input.id));
    }
    Ok(PreparedGatewaySessionInput { session_id, text })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_constants_keep_gateway_session_input_batches_small() {
        const {
            assert!(SESSION_INPUT_POLL_INTERVAL_SECS >= 1);
        }
        assert!((1..=20).contains(&SESSION_INPUT_POLL_LIMIT));
    }

    #[test]
    fn prepare_gateway_session_input_trims_session_and_message() {
        let input = SessionInputRecord {
            id: "input-1".into(),
            session_id: " session-1 ".into(),
            message: " continue ".into(),
            received_at_ms: 10,
        };

        let prepared = prepare_gateway_session_input(&input).expect("prepared input");

        assert_eq!(
            prepared,
            PreparedGatewaySessionInput {
                session_id: "session-1".into(),
                text: "continue".into(),
            }
        );
    }

    #[test]
    fn prepare_gateway_session_input_rejects_blank_payloads() {
        let blank_session = SessionInputRecord {
            id: "input-1".into(),
            session_id: " ".into(),
            message: "continue".into(),
            received_at_ms: 10,
        };
        let blank_message = SessionInputRecord {
            id: "input-2".into(),
            session_id: "session-1".into(),
            message: " ".into(),
            received_at_ms: 10,
        };

        assert!(prepare_gateway_session_input(&blank_session)
            .expect_err("blank session")
            .contains("session_id"));
        assert!(prepare_gateway_session_input(&blank_message)
            .expect_err("blank message")
            .contains("message"));
    }
}
