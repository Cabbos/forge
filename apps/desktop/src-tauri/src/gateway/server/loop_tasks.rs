//! Loop task lifecycle handlers: create, list, get, complete, cancel, recover.

use super::*;

pub(super) fn handle_create_loop_task(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<CreateLoopTaskRequest>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let goal = params.goal.trim().to_string();
    if goal.is_empty() {
        return invalid_params(request.id, "goal must not be empty");
    }
    let session_id = clean_optional_string(params.session_id);
    let profile_id = clean_optional_string(params.profile_id);
    let workspace_path = clean_optional_string(params.workspace_path);
    let idempotency_key = clean_optional_string(params.idempotency_key)
        .unwrap_or_else(|| format!("rpc:{}", request.id));
    match state
        .loop_event_journal
        .find_by_idempotency_key(&idempotency_key)
    {
        Ok(Some(existing)) => {
            let LoopRuntimeEvent::TaskCreated { task } = existing.event else {
                return invalid_params(
                    request.id,
                    format!("idempotency conflict for key: {idempotency_key}"),
                );
            };
            if !create_request_matches_task(
                &task,
                &goal,
                &session_id,
                &profile_id,
                &workspace_path,
                &params.policy,
                &params.budget,
                &params.completion_contract,
            ) {
                return invalid_params(
                    request.id,
                    format!("idempotency conflict for key: {idempotency_key}"),
                );
            }
            if let Err(error) = state
                .loop_task_projection_store
                .rebuild_from_journal(&state.loop_event_journal)
            {
                return invalid_params(request.id, error);
            }
            return loop_task_response(request.id, task);
        }
        Ok(None) => {}
        Err(error) => return invalid_params(request.id, error),
    }

    let task = LoopTaskRecord::new(
        goal,
        session_id,
        profile_id,
        workspace_path,
        params.policy,
        params.budget,
        params.completion_contract,
    );
    let event = LoopEventEnvelope::task_created(
        task.clone(),
        Some(request.id.clone()),
        Some(idempotency_key),
    );
    let append = match state.loop_event_journal.append_idempotent(event) {
        Ok(append) => append,
        Err(error) => return invalid_params(request.id, error),
    };
    let projection = match state
        .loop_task_projection_store
        .rebuild_from_journal(&state.loop_event_journal)
    {
        Ok(projection) => projection,
        Err(error) => return invalid_params(request.id, error),
    };
    let task = projection
        .find(&append.event.task_id)
        .cloned()
        .unwrap_or(task);
    loop_task_response(request.id, task)
}

pub(super) fn handle_list_loop_tasks(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let params = match request.params {
        Some(params) => match serde_json::from_value::<ListLoopTasksParams>(params) {
            Ok(params) => params,
            Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
        },
        None => ListLoopTasksParams {
            statuses: Vec::new(),
            limit: None,
        },
    };
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let projection = match state
        .loop_task_projection_store
        .load_or_rebuild(&state.loop_event_journal)
    {
        Ok(projection) => projection,
        Err(error) => return invalid_params(request.id, error),
    };
    let mut tasks = projection.tasks;
    if !params.statuses.is_empty() {
        tasks.retain(|task| params.statuses.contains(&task.status));
    }
    let total = tasks.len();
    tasks.truncate(limit);
    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(ListLoopTasksResult {
            ok: true,
            tasks,
            total,
        })
        .unwrap(),
    })
}

pub(super) fn handle_get_loop_task(state: &GatewayState, request: GatewayRequest) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<GetLoopTaskParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let task_id = params.task_id.trim().to_string();
    if task_id.is_empty() {
        return invalid_params(request.id, "task_id must not be empty");
    }
    let projection = match state
        .loop_task_projection_store
        .load_or_rebuild(&state.loop_event_journal)
    {
        Ok(projection) => projection,
        Err(error) => return invalid_params(request.id, error),
    };
    let Some(task) = projection.find(&task_id).cloned() else {
        return invalid_params(request.id, format!("loop task not found: {task_id}"));
    };
    loop_task_response(request.id, task)
}

pub(super) fn handle_evaluate_loop_task_completion(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<EvaluateLoopTaskCompletionParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let task_id = params.task_id.trim().to_string();
    if task_id.is_empty() {
        return invalid_params(request.id, "task_id must not be empty");
    }
    let projection = match state
        .loop_task_projection_store
        .load_or_rebuild(&state.loop_event_journal)
    {
        Ok(projection) => projection,
        Err(error) => return invalid_params(request.id, error),
    };
    let Some(task) = projection.find(&task_id).cloned() else {
        return invalid_params(request.id, format!("loop task not found: {task_id}"));
    };
    let result = evaluate_completion(&task, &task.evidence);
    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(EvaluateLoopTaskCompletionResult {
            ok: true,
            task,
            result,
        })
        .unwrap(),
    })
}

pub(super) fn handle_cancel_loop_task(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<CancelLoopTaskParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let task_id = params.task_id.trim().to_string();
    if task_id.is_empty() {
        return invalid_params(request.id, "task_id must not be empty");
    }
    let reason = clean_optional_string(params.reason);
    let projection = match state
        .loop_task_projection_store
        .load_or_rebuild(&state.loop_event_journal)
    {
        Ok(projection) => projection,
        Err(error) => return invalid_params(request.id, error),
    };
    let Some(task) = projection.find(&task_id).cloned() else {
        return invalid_params(request.id, format!("loop task not found: {task_id}"));
    };
    if task.status.is_terminal() {
        return cancel_loop_task_response(request.id, false, task);
    }

    let idempotency_key = cancel_idempotency_key(&task_id, reason.as_deref());
    let event = LoopEventEnvelope::task_canceled(
        task_id.clone(),
        reason,
        Some(request.id.clone()),
        Some(idempotency_key),
    );
    if let Err(error) = state.loop_event_journal.append_idempotent(event) {
        return invalid_params(request.id, error);
    }
    let projection = match state
        .loop_task_projection_store
        .rebuild_from_journal(&state.loop_event_journal)
    {
        Ok(projection) => projection,
        Err(error) => return invalid_params(request.id, error),
    };
    let Some(task) = projection.find(&task_id).cloned() else {
        return invalid_params(request.id, format!("loop task not found: {task_id}"));
    };
    cancel_loop_task_response(request.id, true, task)
}

pub(super) fn handle_recover_loop_task(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<RecoverLoopTaskParams>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
    };
    let task_id = params.task_id.trim().to_string();
    if task_id.is_empty() {
        return invalid_params(request.id, "task_id must not be empty");
    }
    let action = params.action;
    let reason = recovery_reason_for_action(action, params.reason);
    let projection = match state
        .loop_task_projection_store
        .load_or_rebuild(&state.loop_event_journal)
    {
        Ok(projection) => projection,
        Err(error) => return invalid_params(request.id, error),
    };
    let Some(task) = projection.find(&task_id).cloned() else {
        return invalid_params(request.id, format!("loop task not found: {task_id}"));
    };
    if action == RecoveryActionKind::ExportEvidence {
        let evidence = match recovery_action_evidence(&state.loop_event_journal, &task) {
            Ok(evidence) => evidence,
            Err(error) => return invalid_params(request.id, error),
        };
        return recover_loop_task_response(
            request.id,
            action,
            false,
            task,
            "Recovery evidence was exported without changing journal state.",
            Some(evidence),
        );
    }
    if task.status.is_terminal() {
        let evidence = match recovery_action_evidence(&state.loop_event_journal, &task) {
            Ok(evidence) => Some(evidence),
            Err(error) => return invalid_params(request.id, error),
        };
        return recover_loop_task_response(
            request.id,
            action,
            false,
            task,
            "Loop task is terminal; recovery state was left unchanged.",
            evidence,
        );
    }
    if action == RecoveryActionKind::RetryWaitingTask {
        if task.status != LoopTaskStatus::WaitingForInput {
            let evidence = match recovery_action_evidence(&state.loop_event_journal, &task) {
                Ok(evidence) => Some(evidence),
                Err(error) => return invalid_params(request.id, error),
            };
            return recover_loop_task_response(
                request.id,
                action,
                false,
                task,
                "Loop task is not waiting for input; retry was not applied.",
                evidence,
            );
        }

        let idempotency_key = clean_optional_string(params.idempotency_key)
            .unwrap_or_else(|| recover_idempotency_key(action, &task_id, &reason));
        let now_ms = now_millis();
        let event = LoopEventEnvelope {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: new_loop_event_id(),
            task_id: task_id.clone(),
            sequence: 0,
            event: LoopRuntimeEvent::TaskRequeued {
                task_id: task_id.clone(),
                reason,
                requeued_at_ms: now_ms,
            },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id: Some(request.id.clone()),
            causation_id: task.latest_event_id.clone(),
            idempotency_key: Some(idempotency_key),
            created_at_ms: now_ms,
        };
        let append = match state.loop_event_journal.append_idempotent(event) {
            Ok(append) => append,
            Err(error) => return invalid_params(request.id, error),
        };
        let projection = match state
            .loop_task_projection_store
            .rebuild_from_journal(&state.loop_event_journal)
        {
            Ok(projection) => projection,
            Err(error) => return invalid_params(request.id, error),
        };
        let Some(task) = projection.find(&task_id).cloned() else {
            return invalid_params(request.id, format!("loop task not found: {task_id}"));
        };
        let evidence = match recovery_action_evidence(&state.loop_event_journal, &task) {
            Ok(evidence) => Some(evidence),
            Err(error) => return invalid_params(request.id, error),
        };
        return recover_loop_task_response(
            request.id,
            action,
            append.appended,
            task,
            "Loop task was requeued for safe retry.",
            evidence,
        );
    }
    if task.status == LoopTaskStatus::Interrupted && task.recovery_state.is_some() {
        let notice = task
            .recovery_state
            .as_ref()
            .map(|recovery| recovery.notice.clone())
            .unwrap_or_else(|| "Loop task is already interrupted and recoverable.".to_string());
        let evidence = match recovery_action_evidence(&state.loop_event_journal, &task) {
            Ok(evidence) => Some(evidence),
            Err(error) => return invalid_params(request.id, error),
        };
        return recover_loop_task_response(request.id, action, false, task, notice, evidence);
    }

    let idempotency_key = clean_optional_string(params.idempotency_key)
        .unwrap_or_else(|| recover_idempotency_key(action, &task_id, &reason));
    let now_ms = now_millis();
    let event = LoopEventEnvelope {
        schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
        event_id: new_loop_event_id(),
        task_id: task_id.clone(),
        sequence: 0,
        event: LoopRuntimeEvent::TaskInterrupted {
            task_id: task_id.clone(),
            reason,
        },
        actor: LoopActor::Gateway,
        lease_id: task.lease.as_ref().map(|lease| lease.lease_id.clone()),
        attempt: None,
        correlation_id: Some(request.id.clone()),
        causation_id: task.latest_event_id.clone(),
        idempotency_key: Some(idempotency_key),
        created_at_ms: now_ms,
    };
    let append = match state.loop_event_journal.append_idempotent(event) {
        Ok(append) => append,
        Err(error) => return invalid_params(request.id, error),
    };
    let projection = match state
        .loop_task_projection_store
        .rebuild_from_journal(&state.loop_event_journal)
    {
        Ok(projection) => projection,
        Err(error) => return invalid_params(request.id, error),
    };
    let Some(task) = projection.find(&task_id).cloned() else {
        return invalid_params(request.id, format!("loop task not found: {task_id}"));
    };
    let notice = task
        .recovery_state
        .as_ref()
        .map(|recovery| recovery.notice.clone())
        .unwrap_or_else(|| "Loop task was marked interrupted.".to_string());
    let evidence = match recovery_action_evidence(&state.loop_event_journal, &task) {
        Ok(evidence) => Some(evidence),
        Err(error) => return invalid_params(request.id, error),
    };
    recover_loop_task_response(request.id, action, append.appended, task, notice, evidence)
}

pub(super) fn loop_task_response(id: String, task: LoopTaskRecord) -> GatewayReply {
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(LoopTaskResponse { ok: true, task }).unwrap(),
    })
}

pub(super) fn cancel_loop_task_response(
    id: String,
    changed: bool,
    task: LoopTaskRecord,
) -> GatewayReply {
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(CancelLoopTaskResult {
            ok: true,
            changed,
            task,
        })
        .unwrap(),
    })
}

pub(super) fn recover_loop_task_response(
    id: String,
    action: RecoveryActionKind,
    changed: bool,
    task: LoopTaskRecord,
    notice: impl Into<String>,
    evidence: Option<RecoveryActionEvidence>,
) -> GatewayReply {
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(RecoverLoopTaskResult {
            ok: true,
            action,
            changed,
            task,
            notice: notice.into(),
            evidence,
        })
        .unwrap(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn create_request_matches_task(
    task: &LoopTaskRecord,
    goal: &str,
    session_id: &Option<String>,
    profile_id: &Option<String>,
    workspace_path: &Option<String>,
    policy: &Option<crate::loop_runtime::LoopPolicy>,
    budget: &Option<crate::loop_runtime::LoopBudget>,
    completion_contract: &Option<crate::loop_runtime::LoopCompletionContract>,
) -> bool {
    task.goal == goal
        && &task.session_id == session_id
        && &task.profile_id == profile_id
        && &task.workspace_path == workspace_path
        && task.policy
            == policy
                .clone()
                .unwrap_or_else(crate::loop_runtime::LoopPolicy::default_for_background_task)
        && task.budget
            == budget
                .clone()
                .unwrap_or_else(crate::loop_runtime::LoopBudget::default_for_background_task)
        && task.completion_contract
            == completion_contract.clone().unwrap_or_else(
                crate::loop_runtime::LoopCompletionContract::default_for_background_task,
            )
}

pub(super) fn recovery_reason_for_action(
    action: RecoveryActionKind,
    reason: Option<String>,
) -> String {
    match action {
        RecoveryActionKind::MarkInterrupted => clean_optional_string(reason)
            .unwrap_or_else(|| "manual recovery marked task interrupted".to_string()),
        RecoveryActionKind::AbandonOrphan => clean_optional_string(reason)
            .map(ensure_orphan_recovery_reason)
            .unwrap_or_else(|| "orphaned loop task abandoned by operator".to_string()),
        RecoveryActionKind::RetryWaitingTask => clean_optional_string(reason)
            .unwrap_or_else(|| "waiting loop task requeued by operator for safe retry".to_string()),
        RecoveryActionKind::ExportEvidence => clean_optional_string(reason)
            .unwrap_or_else(|| "recovery evidence export requested".to_string()),
    }
}

pub(super) fn ensure_orphan_recovery_reason(reason: String) -> String {
    let normalized = reason.to_ascii_lowercase();
    if normalized.contains("orphan") || normalized.contains("stale lease") {
        reason
    } else {
        format!("orphaned task abandoned: {reason}")
    }
}

pub(super) fn recovery_action_evidence(
    journal: &LoopEventJournal,
    task: &LoopTaskRecord,
) -> Result<RecoveryActionEvidence, String> {
    let event_count = journal
        .load_all()?
        .iter()
        .filter(|event| event.task_id == task.id)
        .count();
    let recovery = task.recovery_state.as_ref();
    Ok(RecoveryActionEvidence {
        task_id: task.id.clone(),
        status: task.status,
        recovery_kind: recovery.map(|state| state.kind),
        recovery_reason: recovery.map(|state| state.reason.clone()),
        latest_event_id: task.latest_event_id.clone(),
        event_count,
        evidence_count: task.evidence.len(),
        policy_decision_count: task.policy_decisions.len(),
        open_gate_count: task.open_gates.len(),
    })
}

pub(super) fn cancel_idempotency_key(task_id: &str, reason: Option<&str>) -> String {
    format!(
        "cancel:{task_id}:{}",
        stable_text_fingerprint(reason.unwrap_or(""))
    )
}

pub(super) fn recover_idempotency_key(
    action: RecoveryActionKind,
    task_id: &str,
    reason: &str,
) -> String {
    format!(
        "recover:{action:?}:{task_id}:{}",
        stable_text_fingerprint(reason)
    )
}
