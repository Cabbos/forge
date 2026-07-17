//! Session input inbox handlers (enqueue, list, complete, clear-stale).

use super::*;

pub(super) fn handle_enqueue_session_input(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<EnqueueSessionInputParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let session_id = params.session_id.trim().to_string();
    if session_id.is_empty() {
        return invalid_params(request.id, "session_id must not be empty");
    }
    let message = params.message.trim().to_string();
    if message.is_empty() {
        return invalid_params(request.id, "message must not be empty");
    }

    let input_id = params
        .input_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(new_trigger_id);

    state.session_input_store.push(new_session_input_record(
        input_id.clone(),
        session_id.clone(),
        message,
    ));

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(EnqueueSessionInputResult {
            ok: true,
            input_id,
            session_id,
            pending_inputs: state.session_input_store.list().len(),
        })
        .unwrap(),
    })
}

pub(super) fn handle_list_session_inputs(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<ListSessionInputsParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let session_ids = clean_session_ids(params.session_ids);
    if session_ids.is_empty() {
        return invalid_params(request.id, "session_ids must not be empty");
    }
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let inputs = state
        .session_input_store
        .list_for_sessions(&session_ids, limit);

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(ListSessionInputsResult {
            ok: true,
            inputs,
            pending_inputs: state.session_input_store.list().len(),
        })
        .unwrap(),
    })
}

pub(super) fn handle_complete_session_input(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<CompleteSessionInputParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let input_id = params.input_id.trim().to_string();
    if input_id.is_empty() {
        return invalid_params(request.id, "input_id must not be empty");
    }
    let removed = state.session_input_store.complete(&input_id);

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(CompleteSessionInputResult {
            ok: true,
            input_id,
            removed,
            pending_inputs: state.session_input_store.list().len(),
        })
        .unwrap(),
    })
}

pub(super) fn handle_clear_stale_session_input(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<ClearStaleSessionInputParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let input_id = params.input_id.trim().to_string();
    if input_id.is_empty() {
        return invalid_params(request.id, "input_id must not be empty");
    }
    let reason = clean_optional_string(params.reason)
        .unwrap_or_else(|| "stale gateway session input cleared by operator".to_string());
    let evidence = state
        .session_input_store
        .clear_stale_with_record(&input_id, &reason);

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(ClearStaleSessionInputResult {
            ok: true,
            input_id,
            cleared: evidence.is_some(),
            pending_inputs: state.session_input_store.list().len(),
            evidence,
        })
        .unwrap(),
    })
}
