//! Pending-trigger queue handlers and trigger-run replay/inspection.

use super::*;

pub(super) fn handle_list_triggers(state: &GatewayState, id: String) -> GatewayReply {
    let triggers = state.trigger_store.list();
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(triggers).unwrap(),
    })
}

pub(super) fn handle_drain_triggers(state: &GatewayState, id: String) -> GatewayReply {
    let triggers = state.trigger_store.drain();
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(triggers).unwrap(),
    })
}

pub(super) fn handle_enqueue_trigger(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<EnqueueTriggerParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let message = params.message.trim().to_string();
    if message.is_empty() {
        return invalid_params(request.id, "message must not be empty");
    }

    let trigger_id = params
        .trigger_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(new_trigger_id);

    state.trigger_store.push(PendingTrigger {
        id: trigger_id.clone(),
        message,
        profile_id: clean_optional_string(params.profile_id),
        provider: clean_optional_string(params.provider),
        model: clean_optional_string(params.model),
        workspace_path: clean_optional_string(params.workspace_path),
        attempt_count: 0,
        claimed_at_ms: None,
        received_at_ms: now_millis(),
    });

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(EnqueueTriggerResult {
            ok: true,
            trigger_id,
            pending_triggers: state
                .trigger_store
                .list()
                .iter()
                .filter(|trigger| trigger.claimed_at_ms.is_none())
                .count(),
        })
        .unwrap(),
    })
}

pub(super) fn handle_cancel_trigger(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<CancelTriggerParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let trigger_id = params.trigger_id.trim().to_string();
    if trigger_id.is_empty() {
        return invalid_params(request.id, "trigger_id must not be empty");
    }

    let removed = state.trigger_store.complete(&trigger_id);
    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(CancelTriggerResult {
            ok: true,
            trigger_id,
            removed,
            pending_triggers: count_available_triggers(state),
        })
        .unwrap(),
    })
}

pub(super) fn handle_replay_trigger_run(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<ReplayTriggerRunParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let run_id = params.run_id.trim().to_string();
    if run_id.is_empty() {
        return invalid_params(request.id, "run_id must not be empty");
    }

    let Some(run) = state.trigger_run_store.find(&run_id) else {
        return invalid_params(request.id, format!("run_id not found: {run_id}"));
    };
    let Some(message) = run
        .trigger_message
        .as_deref()
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_string)
    else {
        return invalid_params(
            request.id,
            format!("run {run_id} cannot be replayed because trigger metadata is missing"),
        );
    };

    let trigger_id = new_trigger_id();
    state.trigger_store.push(PendingTrigger {
        id: trigger_id.clone(),
        message,
        profile_id: clean_optional_string(run.profile_id),
        provider: clean_optional_string(run.provider),
        model: clean_optional_string(run.model),
        workspace_path: clean_optional_string(run.workspace_path),
        attempt_count: 0,
        claimed_at_ms: None,
        received_at_ms: now_millis(),
    });

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(ReplayTriggerRunResult {
            ok: true,
            run_id,
            trigger_id,
            pending_triggers: count_available_triggers(state),
        })
        .unwrap(),
    })
}

pub(super) fn handle_get_trigger_run(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<GetTriggerRunParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let run_id = params.run_id.trim().to_string();
    if run_id.is_empty() {
        return invalid_params(request.id, "run_id must not be empty");
    }

    let Some(run) = state.trigger_run_store.find(&run_id) else {
        return invalid_params(request.id, format!("run_id not found: {run_id}"));
    };

    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(GetTriggerRunResult { ok: true, run }).unwrap(),
    })
}

pub(super) fn handle_list_trigger_runs(state: &GatewayState, id: String) -> GatewayReply {
    let runs = state.trigger_run_store.list();
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(runs).unwrap(),
    })
}

pub(super) fn count_available_triggers(state: &GatewayState) -> usize {
    count_pending_triggers(&state.trigger_store.list())
}

pub(super) fn count_pending_triggers(triggers: &[PendingTrigger]) -> usize {
    triggers
        .iter()
        .filter(|trigger| trigger.claimed_at_ms.is_none())
        .count()
}

pub(super) fn new_trigger_id() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}
