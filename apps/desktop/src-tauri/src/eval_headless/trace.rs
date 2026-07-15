use crate::agent::turn_state::{AgentTurnState, AgentVerificationStatus};
use crate::loop_runtime::{
    EvidenceRecord, LoopCompletionStatus, LoopTaskRecord, LoopTaskRecoveryKind, LoopTaskStatus,
};
use std::collections::BTreeMap;

pub fn build_trace_payload(input: super::types::TracePayloadInput) -> serde_json::Value {
    let mut event_summary = super::summary::summarize_events(&input.raw_events);
    let session_id = input
        .raw_events
        .iter()
        .map(|event| event.session_id().trim())
        .find(|session_id| !session_id.is_empty())
        .map(str::to_string);
    let verification_result = input
        .latest_turn
        .as_ref()
        .and_then(verification_result_from_turn);
    let (error, failure_reason, failure_category) =
        failure_fields(input.latest_turn.as_ref(), verification_result.as_ref());
    let mut tool_calls = std::mem::take(&mut event_summary.tool_calls);
    if let Some(turn) = input.latest_turn.as_ref() {
        tool_calls = super::summary::enrich_tool_calls_with_turn_tools(tool_calls, &turn.tools);
    }
    let file_diffs = input
        .file_diffs
        .into_iter()
        .map(|diff| {
            serde_json::json!({
                "path": diff.path,
                "change_type": diff.change_type,
                "diff": diff.diff,
            })
        })
        .collect::<Vec<_>>();
    let changed_files = input.changed_files;
    let evidence_failure_category = input
        .loop_task
        .as_ref()
        .map(loop_task_failure_category)
        .unwrap_or_else(|| failure_category.clone());
    let forge_run_evidence = forge_run_evidence(ForgeRunEvidenceInput {
        task_id: &input.task_id,
        prompt: &input.prompt,
        latest_turn: input.latest_turn.as_ref(),
        events: &input.raw_events,
        event_summary: &event_summary,
        tool_calls: &tool_calls,
        file_diffs: &file_diffs,
        changed_files: &changed_files,
        verification_result: verification_result.as_ref(),
        loop_task: input.loop_task.as_ref(),
        failure_category: &evidence_failure_category,
        failure_reason: failure_reason.as_deref(),
        continuity_formed_count: input.continuity_formed_count,
        continuity_error: input.continuity_error.as_deref(),
    });

    serde_json::json!({
        "task_id": input.task_id,
        "session_id": session_id,
        "user_prompt": input.prompt,
        "provider": input.provider,
        "model": input.model,
        "loop_task": input.loop_task.as_ref().map(loop_task_trace_summary),
        "forge_run_evidence": forge_run_evidence,
        "raw_events": input.raw_events
            .iter()
            .filter_map(|event| serde_json::to_value(event).ok())
            .collect::<Vec<_>>(),
        "tool_calls": tool_calls,
        "shell_outputs": event_summary.shell_outputs,
        "file_diffs": file_diffs,
        "changed_files": changed_files,
        "verification_result": verification_result,
        "headless_continuity_formed_count": input.continuity_formed_count,
        "headless_continuity_error": input.continuity_error,
        "final_answer": input.final_answer,
        "model_rounds": event_summary.model_rounds,
        "confirm_requests": event_summary.confirm_requests,
        "compact_events": event_summary.compact_events,
        "compact_count": event_summary.compact_count,
        "compact_estimated_tokens_saved": event_summary.compact_estimated_tokens_saved,
        "input_tokens": event_summary.input_tokens,
        "output_tokens": event_summary.output_tokens,
        "repair_attempts_used": input.repair_attempts_used,
        "validation_attempts": input.validation_attempts,
        "error": error,
        "failure_reason": failure_reason,
        "failure_category": failure_category,
        "duration_ms": input.duration_ms,
    })
}

struct ForgeRunEvidenceInput<'a> {
    task_id: &'a str,
    prompt: &'a str,
    latest_turn: Option<&'a AgentTurnState>,
    events: &'a [crate::protocol::events::StreamEvent],
    event_summary: &'a super::types::EventSummary,
    tool_calls: &'a [serde_json::Value],
    file_diffs: &'a [serde_json::Value],
    changed_files: &'a [String],
    verification_result: Option<&'a serde_json::Value>,
    loop_task: Option<&'a LoopTaskRecord>,
    failure_category: &'a str,
    failure_reason: Option<&'a str>,
    continuity_formed_count: Option<usize>,
    continuity_error: Option<&'a str>,
}

fn forge_run_evidence(input: ForgeRunEvidenceInput<'_>) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 2,
        "source": "forge_headless_trace",
        "prompt": input.prompt,
        "normalized_goal": input.latest_turn
            .map(|turn| turn.user_goal.as_str())
            .filter(|goal| !goal.trim().is_empty())
            .unwrap_or(input.task_id),
        "prepared_context": prepared_context_evidence(input.events, input.latest_turn, input.event_summary),
        "memory_audit": memory_audit_evidence(input.events),
        "permission_decisions": permission_decision_evidence(input.events),
        "tool_calls": input.tool_calls,
        "shell_outputs": input.event_summary.shell_outputs,
        "changed_files": input.changed_files,
        "file_diffs": input.file_diffs,
        "verification": input.verification_result,
        "provider_usage": provider_usage_evidence(input.events, input.event_summary),
        "failure_category": input.failure_category,
        "failure_reason": input.failure_reason,
        "recovery": input.loop_task.and_then(|task| {
            task.recovery_state
                .as_ref()
                .and_then(|recovery| serde_json::to_value(recovery).ok())
        }),
        "completion_eligibility": completion_eligibility_evidence(input.loop_task),
        "a2a_child_capsules": a2a_child_capsules_evidence(input.events),
        "continuity_lessons": continuity_lesson_evidence(
            input.continuity_formed_count,
            input.continuity_error,
        ),
    })
}

fn completion_eligibility_evidence(task: Option<&LoopTaskRecord>) -> serde_json::Value {
    let Some(result) = task.and_then(|task| task.completion_result.as_ref()) else {
        return serde_json::json!({ "status": "unknown" });
    };
    serde_json::json!({
        "status": result.status,
        "reasons": &result.reasons,
        "review_status": result.review_status,
        "commit_eligible": result.commit_eligible,
        "commit_blockers": &result.commit_blockers,
        "facts": &result.eligibility_facts,
    })
}

fn loop_task_trace_summary(task: &LoopTaskRecord) -> serde_json::Value {
    serde_json::json!({
        "task_id": task.id,
        "owner": task.owner,
        "status": task.status,
        "completion_result": task.completion_result,
        "completion_evidence": task.evidence.iter().map(evidence_trace_summary).collect::<Vec<_>>(),
        "failure_category": loop_task_failure_category(task),
        "usage": task.latest_usage_ledger,
        "recovery_state": task.recovery_state,
        "outcome": task.outcome,
    })
}

fn evidence_trace_summary(evidence: &EvidenceRecord) -> serde_json::Value {
    match evidence {
        EvidenceRecord::Command {
            evidence_id,
            check_name,
            success,
            ..
        } => serde_json::json!({
            "evidence_id": evidence_id,
            "kind": "command",
            "label": check_name,
            "success": success,
        }),
        EvidenceRecord::GitNexus {
            evidence_id, risk, ..
        } => serde_json::json!({
            "evidence_id": evidence_id,
            "kind": "gitnexus",
            "label": risk,
        }),
        EvidenceRecord::Commit {
            evidence_id,
            commit_sha,
            ..
        } => serde_json::json!({
            "evidence_id": evidence_id,
            "kind": "commit",
            "label": commit_sha,
        }),
        EvidenceRecord::Docs { evidence_id, paths } => serde_json::json!({
            "evidence_id": evidence_id,
            "kind": "docs",
            "label": paths.join(","),
        }),
        EvidenceRecord::Review {
            evidence_id,
            gate_id,
            ..
        } => serde_json::json!({
            "evidence_id": evidence_id,
            "kind": "review",
            "label": gate_id,
        }),
        EvidenceRecord::Budget {
            evidence_id,
            budget_exceeded,
        } => serde_json::json!({
            "evidence_id": evidence_id,
            "kind": "budget",
            "label": if *budget_exceeded { "budget_exceeded" } else { "budget_ok" },
        }),
    }
}

fn loop_task_failure_category(task: &LoopTaskRecord) -> String {
    if let Some(recovery) = task.recovery_state.as_ref() {
        return match recovery.kind {
            LoopTaskRecoveryKind::Orphaned => "orphaned".to_string(),
            LoopTaskRecoveryKind::Interrupted => "interrupted".to_string(),
        };
    }

    if let Some(result) = task.completion_result.as_ref() {
        return match result.status {
            LoopCompletionStatus::Complete => "none".to_string(),
            LoopCompletionStatus::Blocked => "blocked".to_string(),
            LoopCompletionStatus::WaitingForReview => "waiting_for_review".to_string(),
            LoopCompletionStatus::FailedBudget => "budget_exhausted".to_string(),
            LoopCompletionStatus::FailedRisk => "risk_exceeded".to_string(),
        };
    }

    match task.status {
        LoopTaskStatus::Failed => "failed".to_string(),
        LoopTaskStatus::Canceled => "canceled".to_string(),
        LoopTaskStatus::Interrupted => "interrupted".to_string(),
        _ => "none".to_string(),
    }
}

fn prepared_context_evidence(
    events: &[crate::protocol::events::StreamEvent],
    latest_turn: Option<&AgentTurnState>,
    event_summary: &super::types::EventSummary,
) -> serde_json::Value {
    serde_json::json!({
        "turn_prepared": latest_turn_prepared(events),
        "turn_context": latest_turn.map(|turn| {
            serde_json::json!({
                "sources": turn.context.sources,
                "estimated_tokens": turn.context.estimated_tokens,
                "budget_tokens": turn.context.budget_tokens,
                "omitted_sources": turn.context.omitted_sources,
            })
        }),
        "compact_count": event_summary.compact_count,
        "compact_events": event_summary.compact_events,
    })
}

fn latest_turn_prepared(
    events: &[crate::protocol::events::StreamEvent],
) -> Option<serde_json::Value> {
    events.iter().rev().find_map(|event| match event {
        crate::protocol::events::StreamEvent::TurnPrepared { prepared, .. } => {
            serde_json::to_value(prepared).ok()
        }
        _ => None,
    })
}

fn memory_audit_evidence(events: &[crate::protocol::events::StreamEvent]) -> serde_json::Value {
    let mut selected_memories = Vec::new();
    let mut selected_project_records = Vec::new();
    let mut selected_memory_audit = Vec::new();
    let mut memory_events = Vec::new();

    for event in events {
        match event {
            crate::protocol::events::StreamEvent::TurnPrepared { prepared, .. } => {
                selected_memories = prepared.selected_memory_ids.clone();
                selected_project_records = prepared.selected_project_record_ids.clone();
                selected_memory_audit = prepared
                    .selected_memory_audit
                    .iter()
                    .filter_map(|audit| serde_json::to_value(audit).ok())
                    .collect();
            }
            crate::protocol::events::StreamEvent::MemorySelection { selected, .. } => {
                memory_events.push(serde_json::json!({
                    "event_type": "memory_selection",
                    "selected": selected,
                }));
            }
            crate::protocol::events::StreamEvent::ForgeWikiContextSelected { selected, .. } => {
                memory_events.push(serde_json::json!({
                    "event_type": "forge_wiki_context_selected",
                    "selected": selected,
                }));
            }
            _ => {}
        }
    }

    serde_json::json!({
        "selected_memory_ids": selected_memories,
        "selected_memory_audit": selected_memory_audit,
        "selected_project_record_ids": selected_project_records,
        "events": memory_events,
    })
}

fn permission_decision_evidence(
    events: &[crate::protocol::events::StreamEvent],
) -> Vec<serde_json::Value> {
    events
        .iter()
        .filter_map(|event| match event {
            crate::protocol::events::StreamEvent::PermissionDecision {
                block_id, evidence, ..
            } => Some(serde_json::json!({
                "event_type": "permission_decision",
                "block_id": block_id,
                "evidence": evidence,
            })),
            crate::protocol::events::StreamEvent::ConfirmAsk {
                block_id,
                permission_evidence: Some(evidence),
                ..
            } => Some(serde_json::json!({
                "event_type": "confirm_ask",
                "block_id": block_id,
                "evidence": evidence,
            })),
            crate::protocol::events::StreamEvent::ConfirmResponse {
                block_id,
                permission_evidence: Some(evidence),
                approved,
                reason,
                replayed,
                ..
            } => Some(serde_json::json!({
                "event_type": "confirm_response",
                "block_id": block_id,
                "evidence": evidence,
                "approved": approved,
                "reason": reason,
                "replayed": replayed,
            })),
            _ => None,
        })
        .collect()
}

fn a2a_child_capsules_evidence(
    events: &[crate::protocol::events::StreamEvent],
) -> Vec<serde_json::Value> {
    let Some(state) = events.iter().rev().find_map(|event| match event {
        crate::protocol::events::StreamEvent::AgentA2AUpdated { state, .. } => Some(state),
        _ => None,
    }) else {
        return Vec::new();
    };

    let child_tasks = state
        .tasks
        .iter()
        .map(|task| (task.task_id.clone(), task))
        .collect::<BTreeMap<_, _>>();
    let mut capsules = Vec::new();
    for task in &state.tasks {
        for capsule in &task.child_capsules {
            let Ok(mut enriched) = serde_json::to_value(capsule) else {
                continue;
            };
            let Some(object) = enriched.as_object_mut() else {
                capsules.push(enriched);
                continue;
            };
            if let Some(child_task) = child_tasks.get(&capsule.child_task_id) {
                insert_child_task_value(object, "execution_mode", &child_task.execution_mode);
                insert_child_task_value(object, "worktree_path", &child_task.worktree_path);
                insert_child_task_value(object, "tests_passed", &child_task.tests_passed);
                insert_child_task_value(object, "diff_truncated", &child_task.diff_truncated);
                insert_child_task_value(object, "cleaned_up", &child_task.cleaned_up);
                insert_child_task_value(
                    object,
                    "changed_file_count",
                    &child_task.changed_file_count,
                );
                insert_child_task_value(
                    object,
                    "test_report_excerpt",
                    &child_task.test_report_excerpt,
                );
                insert_child_task_value(object, "lease_owner", &child_task.lease_owner);
                insert_child_task_value(
                    object,
                    "lease_acquired_at_ms",
                    &child_task.lease_acquired_at_ms,
                );
                insert_child_task_value(
                    object,
                    "lease_expires_at_ms",
                    &child_task.lease_expires_at_ms,
                );
                insert_child_task_value(object, "attempt_count", &child_task.attempt_count);
                insert_child_task_value(object, "max_attempts", &child_task.max_attempts);
                insert_child_task_value(object, "runtime_events", &child_task.runtime_events);
                insert_child_task_value(object, "review_gate", &child_task.review_gate);
                insert_child_task_value(object, "recovery_actions", &child_task.recovery_actions);
                insert_child_task_value(object, "resume_note", &child_task.resume_note);
            }
            capsules.push(enriched);
        }
    }
    capsules
}

fn insert_child_task_value<T: serde::Serialize>(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: &T,
) {
    if object.get(key).is_some_and(|value| !value.is_null()) {
        return;
    }
    let Ok(value) = serde_json::to_value(value) else {
        return;
    };
    if value.is_null() {
        return;
    }
    if value.as_array().is_some_and(Vec::is_empty) {
        return;
    }
    object.insert(key.to_string(), value);
}

fn provider_usage_evidence(
    events: &[crate::protocol::events::StreamEvent],
    event_summary: &super::types::EventSummary,
) -> serde_json::Value {
    let mut usage_events = Vec::new();
    for event in events {
        match event {
            crate::protocol::events::StreamEvent::ProviderUsage {
                provider_id,
                model,
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_creation_tokens,
                reasoning_tokens,
                estimated_cost_micros,
                pricing_source,
                source,
                reason,
                ..
            } => usage_events.push(serde_json::json!({
                "provider_id": provider_id,
                "model": model,
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "cache_read_tokens": cache_read_tokens,
                "cache_creation_tokens": cache_creation_tokens,
                "reasoning_tokens": reasoning_tokens,
                "estimated_cost_micros": estimated_cost_micros,
                "pricing_source": pricing_source,
                "source": source,
                "reason": reason,
                "has_unknown_input_tokens": input_tokens.is_none(),
                "has_unknown_output_tokens": output_tokens.is_none(),
                "has_unknown_cost": estimated_cost_micros.is_none(),
            })),
            crate::protocol::events::StreamEvent::Usage {
                input_tokens,
                output_tokens,
                estimated_cost_usd,
                ..
            } => usage_events.push(serde_json::json!({
                "provider_id": null,
                "model": null,
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "estimated_cost_usd": estimated_cost_usd,
                "source": "legacy_usage",
                "has_unknown_input_tokens": false,
                "has_unknown_output_tokens": false,
                "has_unknown_cost": false,
            })),
            _ => {}
        }
    }

    serde_json::json!({
        "events": usage_events,
        "latest": usage_events.last(),
        "input_tokens": event_summary.input_tokens,
        "output_tokens": event_summary.output_tokens,
    })
}

fn continuity_lesson_evidence(
    formed_count: Option<usize>,
    error: Option<&str>,
) -> Vec<serde_json::Value> {
    if formed_count.is_none() && error.is_none() {
        return Vec::new();
    }

    vec![serde_json::json!({
        "formed_count": formed_count,
        "error": error,
    })]
}

pub(crate) fn build_setup_error_payload(
    input: super::types::SetupErrorPayloadInput,
) -> serde_json::Value {
    let mut payload = build_trace_payload(super::types::TracePayloadInput {
        task_id: input.task_id,
        prompt: input.prompt,
        provider: input.display_provider,
        model: input.display_model,
        raw_events: Vec::new(),
        loop_task: None,
        latest_turn: None,
        file_diffs: Vec::new(),
        changed_files: Vec::new(),
        final_answer: String::new(),
        duration_ms: input.duration_ms,
        continuity_formed_count: None,
        continuity_error: None,
        repair_attempts_used: 0,
        validation_attempts: 0,
    });
    insert_agent_identity(&mut payload, &input.agent_provider, &input.agent_model);
    insert_failure_fields(
        &mut payload,
        &input.error,
        "runner_error",
        &input.failure_reason,
    );
    payload
}

pub fn insert_agent_identity(payload: &mut serde_json::Value, provider: &str, model: &str) {
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "forge_agent_provider".to_string(),
            serde_json::Value::String(provider.to_string()),
        );
        object.insert(
            "forge_agent_model".to_string(),
            serde_json::Value::String(model.to_string()),
        );
    }
}

pub fn insert_failure_fields(
    payload: &mut serde_json::Value,
    error: &str,
    failure_category: &str,
    failure_reason: &str,
) {
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "error".to_string(),
            serde_json::Value::String(error.to_string()),
        );
        object.insert(
            "failure_category".to_string(),
            serde_json::Value::String(failure_category.to_string()),
        );
        object.insert(
            "failure_reason".to_string(),
            serde_json::Value::String(failure_reason.to_string()),
        );
    }
}

pub(crate) fn verification_result_from_turn(turn: &AgentTurnState) -> Option<serde_json::Value> {
    let command = turn.verification.command.clone()?;
    let passed = matches!(
        turn.verification.status,
        AgentVerificationStatus::Passed | AgentVerificationStatus::Skipped
    );
    Some(serde_json::json!({
        "command": command,
        "passed": passed,
        "stdout": turn.verification.stdout_preview.clone().unwrap_or_default(),
        "stderr": turn.verification.stderr_preview.clone().unwrap_or_default(),
        "exit_code": turn.verification.exit_code.unwrap_or(if passed { 0 } else { 1 }),
        "duration_ms": turn.verification.duration_ms.unwrap_or(0),
    }))
}

pub(crate) fn failure_fields(
    turn: Option<&AgentTurnState>,
    verification_result: Option<&serde_json::Value>,
) -> (Option<String>, Option<String>, String) {
    if let Some(failure) = turn.and_then(|turn| turn.failure.as_ref()) {
        return (
            Some(failure.kind.clone()),
            Some(failure.message.clone()),
            failure.kind.clone(),
        );
    }

    if verification_result
        .and_then(|value| value.get("passed"))
        .and_then(|value| value.as_bool())
        == Some(false)
    {
        return (
            Some("verification_failed".to_string()),
            Some("Forge verification failed.".to_string()),
            "verification_failed".to_string(),
        );
    }

    (None, None, "none".to_string())
}
