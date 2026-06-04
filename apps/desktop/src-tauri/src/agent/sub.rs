use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Notify;

use crate::adapters::base::{AiAdapter, ChatMessage};
use crate::harness::Harness;

const MAX_ROUNDS: usize = 20;
const MAX_RESULT_CHARS: usize = 8000;

/// Structured trace of one sub-agent round for the frontend viewer.
#[derive(Debug, Clone, Serialize)]
struct RoundTrace {
    round: usize,
    thinking: String,
    text: String,
    tool_calls: Vec<ToolCallTrace>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolCallTrace {
    name: String,
    input: String,
    result: String,
}

/// Final JSON payload returned to the frontend.
#[derive(Debug, Clone, Serialize)]
struct SubAgentResult {
    result: String,
    steps: Vec<RoundTrace>,
}

/// A lightweight ephemeral agent for read-only subtasks.
/// Runs in parallel with other sub-agents via tokio::spawn.
/// Returns JSON with full conversation trace for frontend rendering.
pub struct SubAgent;

impl SubAgent {
    pub async fn run(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        app_handle: &tauri::AppHandle,
        cancel: Arc<Notify>,
        working_dir: &std::path::Path,
    ) -> String {
        // Build context: project CLAUDE.md if available, plus working directory
        let project_ctx = crate::harness::read_project_context(working_dir).unwrap_or_default();
        let context_section = if project_ctx.is_empty() {
            format!("Working directory: {}\n", working_dir.display())
        } else {
            format!(
                "Working directory: {}\n\nProject context:\n{}\n",
                working_dir.display(),
                project_ctx
            )
        };

        let system = ChatMessage {
            role: "system".to_string(),
            content: serde_json::Value::String(format!(
                "You are a focused research sub-agent. Your task is to investigate a specific \
                question and return a concise answer.\n\
                You have read-only tools: read_file, search_content, search_files, list_directory, \
                web_search, web_fetch, git_diff.\n\
                Do NOT use write_to_file, edit_file, run_shell, or delegate_task.\n\
                Be thorough but concise. Return your findings as plain text.\n\n\
                {context_section}"
            )),
        };

        let task_msg = ChatMessage::user(task);
        let mut messages: Vec<ChatMessage> = vec![system, task_msg];
        let mut traces: Vec<RoundTrace> = Vec::new();

        for round in 0..MAX_ROUNDS {
            let stream_result = match adapter.call(&messages, cancel.clone()).await {
                Ok(r) => r,
                Err(e) => {
                    crate::app_log!("INFO", "[subagent] API error: {}", e);
                    return build_error_json(&format!("API error: {e}"), &traces);
                }
            };

            let thinking = extract_by_type(&stream_result.assistant_content, "thinking");
            let text = extract_by_type(&stream_result.assistant_content, "text");

            if !stream_result.assistant_content.is_empty() {
                messages.push(ChatMessage::assistant(serde_json::Value::Array(
                    stream_result.assistant_content.clone(),
                )));
            }

            if stream_result.tool_calls.is_empty() {
                crate::app_log!("INFO", "[subagent] round {}: done, no tool calls", round);
                traces.push(RoundTrace {
                    round,
                    thinking: truncate_each(thinking, 1000),
                    text: truncate_each(text.clone(), 2000),
                    tool_calls: vec![],
                });
                let result_text = text.clone();
                return build_result_json(&result_text, &traces);
            }

            crate::app_log!(
                "INFO",
                "[subagent] round {}: {} tool calls — {:?}",
                round,
                stream_result.tool_calls.len(),
                stream_result
                    .tool_calls
                    .iter()
                    .map(|tc| tc.name.clone())
                    .collect::<Vec<_>>()
            );

            // Execute tools in parallel (block dangerous tools)
            // Track by index so each result maps to the correct tool_use_id
            let tool_count = stream_result.tool_calls.len();
            let mut handles = Vec::new();
            for (i, tc) in stream_result.tool_calls.iter().enumerate() {
                let h = harness.clone();
                let name = tc.name.clone();
                let input = tc.input.clone();
                let input_str = serde_json::to_string(&tc.input).unwrap_or_default();
                let tool_id = tc.id.clone();
                let is_blocked = matches!(
                    name.as_str(),
                    "run_shell" | "write_to_file" | "edit_file" | "bash" | "delegate_task"
                );
                let app = app_handle.clone();
                let cancel_for_tool = cancel.clone();
                handles.push(tokio::spawn(async move {
                    let result = if is_blocked {
                        format!(
                            "Tool '{}' is blocked for sub-agents (read-only access only)",
                            name
                        )
                    } else {
                        h.execute_tool_with_block_id_and_cancel(
                            "sub",
                            &name,
                            &input,
                            &app,
                            Some(&tool_id),
                            Some(cancel_for_tool),
                        )
                        .await
                    };
                    (i, name, input_str, result, tool_id)
                }));
            }

            let mut tool_traces: Vec<ToolCallTrace> = Vec::new();
            let mut tool_results: Vec<serde_json::Value> =
                vec![serde_json::Value::Null; tool_count];

            for handle in handles {
                match handle.await {
                    Ok((i, name, input_str, result, tool_id)) => {
                        crate::app_log!(
                            "INFO",
                            "[subagent] tool '{}' result ({} chars)",
                            name,
                            result.len()
                        );
                        let truncated_input = truncate_each(input_str, 200);
                        let truncated_result = truncate_each(result.clone(), 1500);
                        tool_traces.push(ToolCallTrace {
                            name,
                            input: truncated_input,
                            result: truncated_result,
                        });
                        tool_results[i] = serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_id,
                            "content": result,
                        });
                    }
                    Err(e) => {
                        crate::app_log!("INFO", "[subagent] tool panicked: {}", e);
                    }
                }
            }
            // Remove any Null entries from tool_results (panicked tools)
            tool_results.retain(|v| !v.is_null());

            traces.push(RoundTrace {
                round,
                thinking: truncate_each(thinking.clone(), 1000),
                text: truncate_each(text.clone(), 2000),
                tool_calls: tool_traces,
            });

            messages.push(ChatMessage {
                role: "user".to_string(),
                content: serde_json::Value::Array(tool_results),
            });
        }

        // Max rounds — request final summary
        messages.push(ChatMessage::user(
            "Summarize your findings concisely. Do not use tools.",
        ));
        match adapter.call(&messages, cancel.clone()).await {
            Ok(r) => {
                let text = extract_by_type(&r.assistant_content, "text");
                crate::app_log!("INFO", "[subagent] complete: {} chars", text.len());
                build_result_json(&text, &traces)
            }
            Err(e) => build_error_json(&format!("Final error: {e}"), &traces),
        }
    }

    /// Run a sub-agent using an abstract event emitter.
    /// Currently delegates to `run` — the emitter path is only used when the
    /// parent session itself runs with an emitter (tests), and delegate_task
    /// calls are not produced by test adapters.
    pub async fn run_with_emitter(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        _emitter: &dyn crate::agent::event_sink::EventEmitter,
        cancel: Arc<Notify>,
        working_dir: &std::path::Path,
    ) -> String {
        // Sub-agents only use adapter.call() (no AppHandle needed) and
        // harness.execute_tool_with_emitter for tools.
        // For now, use adapter.call() directly and build a minimal loop.
        let project_ctx = crate::harness::read_project_context(working_dir).unwrap_or_default();
        let context_section = if project_ctx.is_empty() {
            format!("Working directory: {}\n", working_dir.display())
        } else {
            format!(
                "Working directory: {}\n\nProject context:\n{}\n",
                working_dir.display(),
                project_ctx
            )
        };

        let system = ChatMessage::system(&format!(
            "You are a focused research sub-agent. Your task is to investigate a specific \
            question and return a concise answer.\n\
            You have read-only tools: read_file, search_content, search_files, list_directory, \
            web_search, web_fetch, git_diff.\n\
            Do NOT use write_to_file, edit_file, run_shell, or delegate_task.\n\
            Be thorough but concise. Return your findings as plain text.\n\n\
            {context_section}"
        ));

        let task_msg = ChatMessage::user(task);
        let mut messages: Vec<ChatMessage> = vec![system, task_msg];

        for _round in 0..MAX_ROUNDS {
            let stream_result = match adapter.call(&messages, cancel.clone()).await {
                Ok(r) => r,
                Err(e) => {
                    return build_error_json(&format!("API error: {e}"), &[]);
                }
            };

            if !stream_result.assistant_content.is_empty() {
                messages.push(ChatMessage::assistant(serde_json::Value::Array(
                    stream_result.assistant_content.clone(),
                )));
            }

            if stream_result.tool_calls.is_empty() {
                let text = extract_by_type(&stream_result.assistant_content, "text");
                return build_result_json(&text, &[]);
            }

            let emitter_arc: Arc<dyn crate::agent::event_sink::EventEmitter> =
                Arc::new(crate::agent::event_sink::NoopEventEmitter);
            let mut result_map = std::collections::HashMap::new();
            for tc in &stream_result.tool_calls {
                let is_blocked = matches!(
                    tc.name.as_str(),
                    "run_shell" | "write_to_file" | "edit_file" | "bash" | "delegate_task"
                );
                let result = if is_blocked {
                    format!("Tool '{}' is blocked for sub-agents", tc.name)
                } else {
                    harness
                        .execute_tool_with_emitter(
                            "sub",
                            &tc.name,
                            &tc.input,
                            emitter_arc.clone(),
                            Some(&tc.id),
                            Some(cancel.clone()),
                        )
                        .await
                };
                result_map.insert(tc.id.clone(), result);
            }

            let model_tool_results =
                crate::agent::tool_results::build_tool_result_message_for_model(
                    &result_map,
                    &stream_result.tool_calls,
                );
            messages.push(model_tool_results.message);
        }

        build_error_json("Max rounds reached", &[])
    }
}

fn extract_by_type(content: &[serde_json::Value], target_type: &str) -> String {
    let result = content
        .iter()
        .filter_map(|block| {
            let t = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if t == target_type {
                block
                    .get(target_type)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    // Fallback: if no text blocks found, try thinking blocks (AI may return thinking-only)
    if !result.is_empty() || target_type != "text" {
        return result;
    }
    extract_by_type(content, "thinking")
}

fn build_result_json(text: &str, traces: &[RoundTrace]) -> String {
    let payload = SubAgentResult {
        result: truncate_any(text.to_string(), MAX_RESULT_CHARS),
        steps: traces.to_vec(),
    };
    serde_json::to_string(&payload).unwrap_or_else(|_| text.to_string())
}

fn build_error_json(error: &str, traces: &[RoundTrace]) -> String {
    let payload = SubAgentResult {
        result: error.to_string(),
        steps: traces.to_vec(),
    };
    serde_json::to_string(&payload).unwrap_or_else(|_| error.to_string())
}

fn truncate_each(s: String, max: usize) -> String {
    if s.len() <= max {
        s
    } else {
        let mut t = s.chars().take(max).collect::<String>();
        t.push_str("...");
        t
    }
}

fn truncate_any(s: String, max: usize) -> String {
    if s.len() <= max {
        s
    } else {
        let mut t = s.chars().take(max).collect::<String>();
        t.push_str("\n\n... (truncated)");
        t
    }
}
