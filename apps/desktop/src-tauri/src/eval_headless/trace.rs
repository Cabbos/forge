use crate::agent::turn_state::{AgentTurnState, AgentVerificationStatus};

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

    serde_json::json!({
        "task_id": input.task_id,
        "session_id": session_id,
        "user_prompt": input.prompt,
        "provider": input.provider,
        "model": input.model,
        "raw_events": input.raw_events
            .iter()
            .filter_map(|event| serde_json::to_value(event).ok())
            .collect::<Vec<_>>(),
        "tool_calls": tool_calls,
        "shell_outputs": event_summary.shell_outputs,
        "file_diffs": input.file_diffs
            .into_iter()
            .map(|diff| {
                serde_json::json!({
                    "path": diff.path,
                    "change_type": diff.change_type,
                    "diff": diff.diff,
                })
            })
            .collect::<Vec<_>>(),
        "changed_files": input.changed_files,
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

pub(crate) fn build_setup_error_payload(
    input: super::types::SetupErrorPayloadInput,
) -> serde_json::Value {
    let mut payload = build_trace_payload(super::types::TracePayloadInput {
        task_id: input.task_id,
        prompt: input.prompt,
        provider: input.display_provider,
        model: input.display_model,
        raw_events: Vec::new(),
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
