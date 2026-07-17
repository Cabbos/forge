//! Headless resume and gateway ownership gates (read-only diagnostics, eligibility).

use super::*;

pub(super) fn handle_request_headless_resume(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<HeadlessResumeControlParams>(params) {
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
    if task.status.is_terminal() {
        return headless_resume_control_response(
            request.id,
            task,
            params.mode,
            false,
            false,
            "Loop task is terminal, and no headless AgentSession was created.",
            None,
        );
    }

    match params.mode {
        HeadlessResumeMode::Disabled => headless_resume_control_response(
            request.id,
            task,
            HeadlessResumeMode::Disabled,
            false,
            false,
            "Headless autonomous resume is disabled by default and requires durable human approval; no headless AgentSession was created.",
            None,
        ),
        HeadlessResumeMode::RequireHumanApproval => headless_resume_control_response(
            request.id,
            task,
            HeadlessResumeMode::RequireHumanApproval,
            false,
            false,
            "Headless resume requires durable human approval before any future autonomous owner may run; no headless AgentSession was created.",
            None,
        ),
        HeadlessResumeMode::ApprovedForTask => {
            let Some(approved_by) = clean_optional_string(params.approved_by) else {
                return invalid_params(request.id, "approved_by is required for approved_for_task");
            };
            let Some(approved_at_ms) = params.approved_at_ms else {
                return invalid_params(
                    request.id,
                    "approved_at_ms is required for approved_for_task",
                );
            };
            let Some(expires_at_ms) = params.expires_at_ms else {
                return invalid_params(
                    request.id,
                    "expires_at_ms is required for approved_for_task",
                );
            };
            if expires_at_ms <= approved_at_ms {
                return invalid_params(
                    request.id,
                    "expires_at_ms must be greater than approved_at_ms",
                );
            }
            let scope = clean_optional_string(params.scope).unwrap_or_else(|| "task".to_string());
            let approval = HeadlessResumeApproval {
                task_id: task_id.clone(),
                approved_by,
                approved_at_ms,
                scope,
                expires_at_ms,
            };
            if let Some(existing) = task.headless_resume_approval.as_ref() {
                if existing == &approval {
                    return headless_resume_control_response(
                        request.id,
                        task,
                        HeadlessResumeMode::ApprovedForTask,
                        false,
                        true,
                        "Headless resume approval was already recorded for this task, and no headless AgentSession was created; autonomous resume remains disabled in Task 4A.",
                        Some(approval),
                    );
                }
                return invalid_params(
                    request.id,
                    format!("duplicate headless resume approval recorded: {task_id}"),
                );
            }
            let idempotency_key =
                clean_optional_string(params.idempotency_key).unwrap_or_else(|| {
                    format!(
                        "headless_resume_approval:{task_id}:{}",
                        stable_text_fingerprint(&format!(
                            "{}:{}:{}:{}",
                            approval.approved_by,
                            approval.approved_at_ms,
                            approval.scope,
                            approval.expires_at_ms
                        ))
                    )
                });
            let event = LoopEventEnvelope::headless_resume_approval_recorded(
                task_id.clone(),
                approval.clone(),
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
            let Some(task) = projection.find(&task_id).cloned() else {
                return invalid_params(request.id, format!("loop task not found: {task_id}"));
            };
            headless_resume_control_response(
                request.id,
                task,
                HeadlessResumeMode::ApprovedForTask,
                append.appended,
                true,
                "Headless resume approval was recorded for this task, and no headless AgentSession was created; autonomous resume remains disabled in Task 4A.",
                Some(approval),
            )
        }
    }
}

pub(super) fn handle_run_gateway_read_only_owner_diagnostics(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let Some(params) = request.params else {
        return invalid_params(request.id, "missing params");
    };
    let params = match serde_json::from_value::<GatewayReadOnlyOwnerDiagnosticsParams>(params) {
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
    if task.status.is_terminal() {
        return gateway_read_only_owner_diagnostics_response(
            request.id,
            task,
            None,
            false,
            false,
            false,
            "Read-only diagnostics owner was not started because the loop task is terminal.",
        );
    }

    let approved_by = clean_optional_string(params.approved_by);
    if approved_by.is_none() && !params.dev_only_allow {
        return gateway_read_only_owner_diagnostics_response(
            request.id,
            task,
            None,
            false,
            false,
            false,
            "Gateway read-only owner diagnostics requires explicit human approval or a dev-only explicit flag; no owner run was started.",
        );
    }

    let requested_at_ms = params.requested_at_ms.unwrap_or_else(now_millis);
    let expires_at_ms = params
        .expires_at_ms
        .unwrap_or_else(|| requested_at_ms.saturating_add(60_000));
    if expires_at_ms <= requested_at_ms {
        return invalid_params(
            request.id,
            "expires_at_ms must be greater than requested_at_ms",
        );
    }
    let session_id = clean_optional_string(params.session_id);
    let idempotency_key = clean_optional_string(params.idempotency_key).unwrap_or_else(|| {
        format!(
            "gateway_read_only_owner:{task_id}:{}",
            session_id.as_deref().unwrap_or("projection")
        )
    });

    if let Some(existing) = task
        .headless_owner_runs
        .iter()
        .find(|owner_run| owner_run.matches_idempotency_key(&task_id, &idempotency_key))
        .cloned()
    {
        return gateway_read_only_owner_diagnostics_response(
            request.id,
            task,
            Some(existing),
            true,
            true,
            true,
            "Gateway read-only diagnostics owner evidence was already recorded; no duplicate side effects were produced.",
        );
    }

    let attempt = task.headless_owner_runs.len() as u32 + 1;
    let owner_run_id = format!("gateway-readonly-owner:{task_id}:{attempt}");
    let lease_id = format!("gateway-readonly-lease:{task_id}:{attempt}");
    let correlation_id = format!("gateway-readonly-correlation:{task_id}:{attempt}");
    let owner_run = HeadlessOwnerRun {
        owner_run_id: owner_run_id.clone(),
        task_id: task_id.clone(),
        session_id: session_id.clone(),
        lease_id: lease_id.clone(),
        attempt,
        state: HeadlessOwnerRunState::Requested,
        snapshot_source: if session_id.is_some() {
            HeadlessOwnerSnapshotSource::CurrentDesktopSession
        } else {
            HeadlessOwnerSnapshotSource::WorkspaceSnapshot
        },
        snapshot_ref: session_id
            .clone()
            .or_else(|| Some(format!("loop_projection:{task_id}"))),
        human_gate_id: approved_by
            .as_ref()
            .map(|approver| format!("human-approval:{task_id}:{approver}"))
            .unwrap_or_else(|| format!("dev-only-read-only-owner:{task_id}:{attempt}")),
        policy_decision_id: format!("gateway-readonly-policy:{task_id}:{attempt}"),
        budget_snapshot_id: format!("gateway-readonly-budget:{task_id}:{attempt}"),
        idempotency_key: idempotency_key.clone(),
        correlation_id: correlation_id.clone(),
        causation_id: Some(request.id.clone()),
        requested_by: approved_by
            .map(|approver| format!("human:{approver}"))
            .unwrap_or_else(|| "dev_only:gateway_read_only_owner".to_string()),
        requested_at_ms,
        heartbeat_at_ms: None,
        expires_at_ms,
        cancellation_reason: None,
        waiting_reason: None,
        executor_kind: HeadlessOwnerExecutorKind::DryRun,
        evidence_refs: gateway_read_only_owner_evidence_refs(&task_id, attempt),
    };
    if let Err(error) = owner_run.validate_authorization_bundle() {
        return invalid_params(request.id, error);
    }

    let request_event = LoopEventEnvelope::headless_owner_run_requested(owner_run.clone());
    if let Err(error) = state.loop_event_journal.append_idempotent(request_event) {
        return invalid_params(request.id, error);
    }
    let lease_event = gateway_read_only_owner_state_event(
        &task_id,
        &owner_run_id,
        &lease_id,
        attempt,
        &correlation_id,
        Some(owner_run.requested_at_ms),
        HeadlessOwnerRunState::LeaseAcquired,
        requested_at_ms,
        "Gateway read-only diagnostics lease acquired.",
        &idempotency_key,
    );
    if let Err(error) = state.loop_event_journal.append_idempotent(lease_event) {
        return invalid_params(request.id, error);
    }
    let completed_event = gateway_read_only_owner_state_event(
        &task_id,
        &owner_run_id,
        &lease_id,
        attempt,
        &correlation_id,
        Some(requested_at_ms),
        HeadlessOwnerRunState::Completed,
        requested_at_ms.saturating_add(1),
        "Gateway read-only diagnostics completed without provider/tool/file side effects.",
        &idempotency_key,
    );
    if let Err(error) = state.loop_event_journal.append_idempotent(completed_event) {
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
    let owner_run = task
        .headless_owner_runs
        .iter()
        .find(|owner_run| owner_run.owner_run_id == owner_run_id)
        .cloned();
    gateway_read_only_owner_diagnostics_response(
        request.id,
        task,
        owner_run,
        true,
        true,
        true,
        "Gateway read-only diagnostics owner completed without provider, tool, shell, file, confirmation, or commit side effects.",
    )
}

pub(super) fn handle_evaluate_gateway_ownership_eligibility(
    state: &GatewayState,
    request: GatewayRequest,
) -> GatewayReply {
    let params = match request.params {
        Some(params) => match serde_json::from_value::<GatewayOwnershipEligibilityParams>(params) {
            Ok(params) => params,
            Err(error) => return invalid_params(request.id, format!("invalid params: {error}")),
        },
        None => GatewayOwnershipEligibilityParams {
            session_id: None,
            task_id: None,
            requested_mode: GatewayOwnershipMode::GatewayReadOnlyOwner,
        },
    };
    GatewayReply::Ok(GatewayResponse {
        id: request.id,
        result: serde_json::to_value(evaluate_gateway_ownership_eligibility(state, params))
            .unwrap(),
    })
}

pub(super) fn evaluate_gateway_ownership_eligibility(
    state: &GatewayState,
    params: GatewayOwnershipEligibilityParams,
) -> GatewayOwnershipEligibilityResult {
    let session_id = clean_optional_string(params.session_id);
    let task_id = clean_optional_string(params.task_id);
    let capability = default_gateway_ownership_capability();
    let mut reasons = Vec::new();
    let mut missing_evidence = Vec::new();
    let mut proposal_only = false;
    let mut would_generate_patch_proposal = false;
    let would_apply_patch = false;

    if !capability.gateway_can_own_sessions {
        push_unique(&mut reasons, "gateway_ownership_disabled");
    }
    if capability.ownership_mode == GatewayOwnershipMode::LocalDefault {
        push_unique(&mut reasons, "local_default_owner");
    }
    match params.requested_mode {
        GatewayOwnershipMode::LocalDefault => {
            push_unique(&mut reasons, "gateway_owner_not_requested");
        }
        GatewayOwnershipMode::GatewayOptIn => {
            push_unique(&mut reasons, "gateway_opt_in_not_enabled");
        }
        GatewayOwnershipMode::GatewayOptInDryRun => {
            push_unique(&mut reasons, "gateway_opt_in_dry_run_not_enabled");
        }
        GatewayOwnershipMode::GatewayReadOnlyOwner => {
            push_unique(&mut reasons, "read_only_owner_requires_explicit_approval");
        }
        GatewayOwnershipMode::GatewayPatchProposalOwner => {
            proposal_only = true;
            would_generate_patch_proposal = true;
            push_unique(&mut reasons, "patch_proposal_owner_requires_gate");
            push_unique(&mut missing_evidence, "patch_proposal_review_gate");
            push_unique(&mut missing_evidence, "diff_evidence_contract");
        }
        GatewayOwnershipMode::GatewayToolOwnerBlockedByDefault => {
            push_unique(&mut reasons, "tool_owner_blocked_by_default");
        }
    }

    if let Some(session_id) = session_id.as_deref() {
        let attach = state.attach_session(session_id);
        if !attach.control.gateway_can_read_snapshot {
            push_unique(&mut missing_evidence, "session_snapshot");
        }
        if attach.status != GatewaySessionAttachStatus::Live {
            push_unique(
                &mut reasons,
                format!(
                    "session_attach_{}",
                    gateway_session_attach_status_label(attach.status)
                ),
            );
        }
    } else {
        push_unique(&mut missing_evidence, "session_context");
    }

    if let Some(task_id) = task_id.as_deref() {
        match state
            .loop_task_projection_store
            .load_or_rebuild(&state.loop_event_journal)
        {
            Ok(projection) => {
                if let Some(task) = projection.find(task_id) {
                    if task.recovery_state.is_none() {
                        push_unique(&mut missing_evidence, "recovery_evidence");
                    }
                    if task.status.is_terminal() {
                        push_unique(&mut reasons, "task_is_terminal");
                    }
                } else {
                    push_unique(&mut missing_evidence, "runtime_projection");
                    push_unique(&mut reasons, "loop_task_not_found");
                }
            }
            Err(_) => {
                push_unique(&mut missing_evidence, "runtime_projection");
                push_unique(&mut reasons, "runtime_projection_unavailable");
            }
        }
    } else {
        push_unique(&mut missing_evidence, "runtime_projection");
    }

    push_unique(&mut missing_evidence, "memory_recall_audit");
    push_unique(&mut missing_evidence, "context_capsule");
    push_unique(&mut missing_evidence, "permission_decision_ledger");

    let decision = if capability.gateway_can_own_sessions && reasons.is_empty() {
        GatewayOwnershipEligibilityDecision::RequiresHumanApproval
    } else {
        GatewayOwnershipEligibilityDecision::Deny
    };
    let required_action = match decision {
        GatewayOwnershipEligibilityDecision::Allow => {
            "Gateway ownership may proceed without additional action.".to_string()
        }
        GatewayOwnershipEligibilityDecision::RequiresHumanApproval => {
            "Require explicit human approval before starting a gateway owner run.".to_string()
        }
        GatewayOwnershipEligibilityDecision::Deny => {
            if params.requested_mode == GatewayOwnershipMode::GatewayPatchProposalOwner {
                "Keep the desktop runtime as owner; patch proposal ownership remains proposal-only, requires review evidence, and direct apply/write stays blocked.".to_string()
            } else {
                "Keep the desktop runtime as owner; resolve the listed gateway eligibility gaps first."
                    .to_string()
            }
        }
    };

    GatewayOwnershipEligibilityResult {
        ok: true,
        decision,
        requested_mode: params.requested_mode,
        session_id,
        task_id,
        reasons,
        missing_evidence,
        required_action,
        proposal_only,
        would_generate_patch_proposal,
        would_apply_patch,
        would_execute_provider: false,
        would_execute_tools: false,
        would_write_files: false,
        changes_task_state: false,
    }
}

pub(super) fn gateway_read_only_owner_diagnostics_response(
    id: String,
    task: LoopTaskRecord,
    owner_run: Option<HeadlessOwnerRun>,
    started: bool,
    completed: bool,
    gateway_can_resume: bool,
    message: impl Into<String>,
) -> GatewayReply {
    let summary = gateway_read_only_owner_summary(&task, owner_run.as_ref());
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(GatewayReadOnlyOwnerDiagnosticsResult {
            ok: started && completed,
            started,
            completed,
            gateway_can_resume,
            task,
            owner_run,
            summary,
            message: message.into(),
            side_effects: gateway_read_only_owner_side_effects(),
        })
        .unwrap(),
    })
}

pub(super) fn gateway_read_only_owner_side_effects() -> GatewayReadOnlyOwnerSideEffects {
    GatewayReadOnlyOwnerSideEffects {
        provider: false,
        tools: false,
        shell: false,
        write_files: false,
        confirmations: false,
        commits: false,
    }
}

pub(super) fn gateway_read_only_owner_summary(
    task: &LoopTaskRecord,
    owner_run: Option<&HeadlessOwnerRun>,
) -> String {
    let owner_state = owner_run
        .map(|run| format!("{:?}", run.state))
        .unwrap_or_else(|| "not_started".to_string());
    format!(
        "Read-only diagnostics for task {}: status={:?}, owner_state={}, owner_runs={}, no provider, tool, shell, file, confirmation, or commit side effects.",
        task.id,
        task.status,
        owner_state,
        task.headless_owner_runs.len()
    )
}

pub(super) fn gateway_read_only_owner_evidence_refs(task_id: &str, attempt: u32) -> Vec<String> {
    vec![
        "gateway_read_only_diagnostics".to_string(),
        format!("loop_projection:{task_id}"),
        format!("gateway_read_only_owner_attempt:{attempt}"),
    ]
}

#[allow(clippy::too_many_arguments)]
pub(super) fn gateway_read_only_owner_state_event(
    task_id: &str,
    owner_run_id: &str,
    lease_id: &str,
    attempt: u32,
    correlation_id: &str,
    causation_at_ms: Option<u64>,
    state: HeadlessOwnerRunState,
    heartbeat_at_ms: u64,
    waiting_reason: &str,
    owner_idempotency_key: &str,
) -> LoopEventEnvelope {
    let state_key = match state {
        HeadlessOwnerRunState::LeaseAcquired => "lease_acquired",
        HeadlessOwnerRunState::Completed => "completed",
        _ => "state",
    };
    LoopEventEnvelope {
        schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
        event_id: new_loop_event_id(),
        task_id: task_id.to_string(),
        sequence: 0,
        event: LoopRuntimeEvent::HeadlessOwnerRunStateRecorded {
            task_id: task_id.to_string(),
            owner_run_id: owner_run_id.to_string(),
            state,
            heartbeat_at_ms: Some(heartbeat_at_ms),
            cancellation_reason: None,
            waiting_reason: Some(waiting_reason.to_string()),
            evidence_refs: gateway_read_only_owner_evidence_refs(task_id, attempt),
        },
        actor: LoopActor::Gateway,
        lease_id: Some(lease_id.to_string()),
        attempt: Some(attempt),
        correlation_id: Some(correlation_id.to_string()),
        causation_id: causation_at_ms
            .map(|value| format!("gateway-readonly-owner:{task_id}:{value}")),
        idempotency_key: Some(format!("{owner_idempotency_key}:{state_key}")),
        created_at_ms: heartbeat_at_ms,
    }
}

pub(super) fn headless_resume_control_response(
    id: String,
    task: LoopTaskRecord,
    mode: HeadlessResumeMode,
    approval_recorded: bool,
    ok: bool,
    message: impl Into<String>,
    approval: Option<HeadlessResumeApproval>,
) -> GatewayReply {
    GatewayReply::Ok(GatewayResponse {
        id,
        result: serde_json::to_value(HeadlessResumeControlResult {
            ok,
            task,
            mode,
            approval_recorded,
            gateway_can_resume: false,
            message: message.into(),
            approval,
        })
        .unwrap(),
    })
}
