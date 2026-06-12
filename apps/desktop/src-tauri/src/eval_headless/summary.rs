use std::collections::HashMap;

use crate::agent::turn_state::{AgentToolTrace, AgentTurnStatus};
use crate::protocol::events::StreamEvent;

use super::types::{EventSummary, PendingShell, PendingTool};

pub(crate) fn summarize_events(events: &[StreamEvent]) -> EventSummary {
    let mut summary = EventSummary::default();
    let mut pending_tools: HashMap<String, PendingTool> = HashMap::new();
    let mut pending_shells: HashMap<String, PendingShell> = HashMap::new();
    let mut last_turn_was_calling_model = false;
    let mut calling_model_transitions = 0;

    for event in events {
        match event {
            StreamEvent::ToolCallStart {
                block_id,
                tool_name,
                tool_input,
                ..
            } => {
                summary.model_rounds += 1;
                pending_tools.insert(
                    block_id.clone(),
                    PendingTool {
                        name: tool_name.clone(),
                        input: tool_input.clone(),
                    },
                );
            }
            StreamEvent::ToolCallResult {
                block_id,
                result,
                is_error,
                duration_ms,
                ..
            } => {
                let pending = pending_tools.remove(block_id).unwrap_or_default();
                summary.tool_calls.push(serde_json::json!({
                    "command": format_tool_command(&pending.name, &pending.input),
                    "stdout": result,
                    "stderr": if *is_error { result.as_str() } else { "" },
                    "exit_code": if *is_error { 1 } else { 0 },
                    "duration_ms": duration_ms,
                }));
            }
            StreamEvent::ShellStart {
                block_id, command, ..
            } => {
                pending_shells.insert(
                    block_id.clone(),
                    PendingShell {
                        command: command.clone(),
                        stdout: String::new(),
                    },
                );
            }
            StreamEvent::ShellOutput {
                block_id, content, ..
            } => {
                pending_shells
                    .entry(block_id.clone())
                    .or_default()
                    .stdout
                    .push_str(content);
            }
            StreamEvent::ShellEnd {
                block_id,
                exit_code,
                ..
            } => {
                let pending = pending_shells.remove(block_id).unwrap_or_default();
                summary.shell_outputs.push(serde_json::json!({
                    "command": pending.command,
                    "stdout": pending.stdout,
                    "stderr": "",
                    "exit_code": exit_code,
                    "duration_ms": 0,
                }));
            }
            StreamEvent::ConfirmAsk { .. } => {
                summary.confirm_requests += 1;
            }
            StreamEvent::ContextCompacted {
                summary: compact_summary,
                retained_messages,
                compacted_messages,
                estimated_tokens_before,
                estimated_tokens_after,
                ..
            } => {
                let saved = estimated_tokens_before.saturating_sub(*estimated_tokens_after) as u64;
                let reduction_percent = if *estimated_tokens_before > 0 {
                    ((*estimated_tokens_before - *estimated_tokens_after) as f64
                        / *estimated_tokens_before as f64
                        * 100.0)
                        .round() as u64
                } else {
                    0
                };
                summary.compact_count += 1;
                summary.compact_estimated_tokens_saved += saved;
                summary.compact_events.push(serde_json::json!({
                    "summary": compact_summary,
                    "retained_messages": retained_messages,
                    "compacted_messages": compacted_messages,
                    "estimated_tokens_before": estimated_tokens_before,
                    "estimated_tokens_after": estimated_tokens_after,
                    "estimated_tokens_saved": saved,
                    "estimated_reduction_percent": reduction_percent,
                }));
            }
            StreamEvent::Usage {
                input_tokens,
                output_tokens,
                ..
            } => {
                summary.input_tokens = Some(*input_tokens);
                summary.output_tokens = Some(*output_tokens);
            }
            StreamEvent::AgentTurnUpdated { state, .. } => {
                let is_calling_model = state.status == AgentTurnStatus::CallingModel;
                if is_calling_model && !last_turn_was_calling_model {
                    calling_model_transitions += 1;
                }
                last_turn_was_calling_model = is_calling_model;
            }
            _ => {}
        }
    }

    if summary.model_rounds == 0 {
        summary.model_rounds = calling_model_transitions;
    }

    summary
}

pub(crate) fn enrich_tool_calls_with_turn_tools(
    mut tool_calls: Vec<serde_json::Value>,
    turn_tools: &[AgentToolTrace],
) -> Vec<serde_json::Value> {
    if tool_calls.is_empty() {
        return turn_tools.iter().map(tool_call_from_turn_tool).collect();
    }

    for (index, tool_call) in tool_calls.iter_mut().enumerate() {
        let command_is_empty = tool_call
            .get("command")
            .and_then(|value| value.as_str())
            .is_none_or(|command| command.trim().is_empty());
        if !command_is_empty {
            continue;
        }

        if let (Some(object), Some(turn_tool)) = (tool_call.as_object_mut(), turn_tools.get(index))
        {
            object.insert(
                "command".to_string(),
                serde_json::Value::String(format_turn_tool_command(turn_tool)),
            );
        }
    }

    tool_calls
}

pub(crate) fn tool_call_from_turn_tool(tool: &AgentToolTrace) -> serde_json::Value {
    let duration_ms = tool
        .ended_at_ms
        .map(|ended| ended.saturating_sub(tool.started_at_ms))
        .unwrap_or(0);
    serde_json::json!({
        "command": format_turn_tool_command(tool),
        "stdout": tool.result_summary.clone().unwrap_or_default(),
        "stderr": if tool.is_error {
            tool.result_summary.clone().unwrap_or_default()
        } else {
            String::new()
        },
        "exit_code": if tool.is_error { 1 } else { 0 },
        "duration_ms": duration_ms,
    })
}

pub(crate) fn format_turn_tool_command(tool: &AgentToolTrace) -> String {
    if let Some(command) = tool
        .command
        .as_ref()
        .filter(|command| !command.trim().is_empty())
    {
        return command.to_string();
    }
    if let Some(path) = tool
        .affected_files
        .first()
        .filter(|path| !path.trim().is_empty())
    {
        return format!("{} {path}", tool.name);
    }
    tool.name.clone()
}

pub(crate) fn format_tool_command(tool_name: &str, input: &serde_json::Value) -> String {
    let path = input
        .get("path")
        .and_then(|value| value.as_str())
        .or_else(|| input.get("command").and_then(|value| value.as_str()));
    match path {
        Some(path) if !path.trim().is_empty() => format!("{tool_name} {path}"),
        _ => tool_name.to_string(),
    }
}
