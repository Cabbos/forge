use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc, Mutex};

use tauri::Emitter;
use tokio::sync::Notify;

use crate::adapters::base::{AiAdapter, ChatMessage};
use crate::agent::snapshot::AgentSessionSnapshot;
use crate::harness::Harness;
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;

const DEFAULT_CONTEXT_WINDOW_TOKENS: usize = 128_000;
const AUTO_COMPACT_THRESHOLD_NUMERATOR: usize = 7;
const AUTO_COMPACT_THRESHOLD_DENOMINATOR: usize = 10;
const MAX_HISTORY_MESSAGES_BEFORE_COMPACT: usize = 80;
const RETAIN_RECENT_MESSAGES: usize = 32;
const MIN_COMPACT_MESSAGES: usize = 8;
const MAX_SUMMARY_CHARS: usize = 14_000;
const MAX_SUMMARY_ITEM_CHARS: usize = 360;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Starting,
    Running,
    Stopped,
    Error(String),
}

impl SessionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            SessionStatus::Starting => "starting",
            SessionStatus::Running => "running",
            SessionStatus::Stopped => "stopped",
            SessionStatus::Error(_) => "error",
        }
    }
}

pub struct AgentSession {
    pub id: String,
    pub agent_type: String,
    pub model: String,
    pub model_id: String,
    pub status: Arc<Mutex<SessionStatus>>,
    pub(crate) adapter: Arc<Box<dyn AiAdapter>>,
    pub(crate) messages: Arc<Mutex<Vec<ChatMessage>>>,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) harness: Arc<Harness>,
    pub(crate) system_prompt: Mutex<String>,
    pub(crate) summary: Mutex<Option<String>>,
    pub(crate) context_window_tokens: Option<u32>,
    pub(crate) cancel: Mutex<Option<Arc<Notify>>>,
}

impl AgentSession {
    pub fn new(
        id: String,
        agent_type: String,
        adapter: Arc<Box<dyn AiAdapter>>,
        harness: Arc<Harness>,
        system_prompt: String,
        context_window_tokens: Option<u32>,
    ) -> Self {
        let model_id = adapter.model_id().to_string();
        let model = adapter.model_name().to_string();

        let session = AgentSession {
            id: id.clone(),
            agent_type: agent_type.clone(),
            model: model.clone(),
            model_id,
            status: Arc::new(Mutex::new(SessionStatus::Starting)),
            adapter,
            messages: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(AtomicBool::new(true)),
            harness,
            system_prompt: Mutex::new(system_prompt),
            summary: Mutex::new(None),
            context_window_tokens,
            cancel: Mutex::new(None),
        };

        *session.status.lock().unwrap() = SessionStatus::Running;
        session
    }

    pub fn set_system_prompt(&self, prompt: String) {
        *self.system_prompt.lock().unwrap() = prompt;
    }

    pub fn is_waiting_for_api_key(&self) -> bool {
        self.adapter.is_missing_api_key_adapter()
    }

    pub fn restore_state(&self, messages: Vec<ChatMessage>, summary: Option<String>) {
        *self.messages.lock().unwrap() = messages;
        *self.summary.lock().unwrap() = summary;
    }

    pub fn snapshot(&self) -> AgentSessionSnapshot {
        AgentSessionSnapshot::new(
            self.id.clone(),
            self.agent_type.clone(),
            self.model_id.clone(),
            self.harness.working_dir.to_string_lossy().to_string(),
            self.messages.lock().unwrap().clone(),
            self.summary.lock().unwrap().clone(),
            self.context_window_tokens,
        )
    }

    /// Send a user message and run the agent loop through the harness.
    pub async fn send_message(
        &self,
        text: &str,
        app_handle: &tauri::AppHandle,
    ) -> Result<(), String> {
        self.send_message_with_context(text, app_handle, None).await
    }

    /// Send a user message with optional hidden memory context for this turn.
    pub async fn send_message_with_context(
        &self,
        text: &str,
        app_handle: &tauri::AppHandle,
        memory_context: Option<String>,
    ) -> Result<(), String> {
        if !self.running.load(Ordering::SeqCst) {
            return Err("Session is not running".to_string());
        }

        crate::app_log!(
            "INFO",
            "Agent received user message, history size: {}",
            self.messages.lock().unwrap().len()
        );

        // Add user message to history
        self.messages.lock().unwrap().push(ChatMessage::user(text));
        let memory_context = memory_context.filter(|context| !context.trim().is_empty());

        // Fresh cancel token for this request
        let cancel = Arc::new(Notify::new());
        *self.cancel.lock().unwrap() = Some(cancel.clone());

        // Agent loop: up to 10 tool-call round-trips with final text summary fallback
        for _round in 0..10 {
            if !self.running.load(Ordering::SeqCst) {
                break;
            }

            let all_messages = self.messages.lock().unwrap().clone();
            let existing_summary = self.summary.lock().unwrap().clone();
            let compacted = compact_messages_if_needed(
                all_messages,
                existing_summary,
                self.context_window_tokens,
            );

            if let Some(stats) = compacted.stats.as_ref() {
                *self.summary.lock().unwrap() = compacted.summary.clone();
                *self.messages.lock().unwrap() = compacted.messages.clone();
                let _ = app_handle.emit(
                    "session-output",
                    StreamEvent::ContextCompacted {
                        session_id: self.id.clone(),
                        block_id: BlockId::new().to_string(),
                        summary: stats.summary.clone(),
                        retained_messages: stats.retained_messages,
                        compacted_messages: stats.compacted_messages,
                        estimated_tokens_before: to_u32_tokens(stats.estimated_tokens_before),
                        estimated_tokens_after: to_u32_tokens(stats.estimated_tokens_after),
                    },
                );
            }

            let messages = compacted.messages;
            let summary_ctx = compacted.summary;
            let mut msgs_with_context =
                apply_turn_context(messages, summary_ctx.as_deref(), memory_context.as_deref());
            // Prepend system prompt with skill instructions
            let sp = self.system_prompt.lock().unwrap().clone();
            crate::app_log!(
                "INFO",
                "[send_message] system_prompt length: {} chars, has 'Active Skills': {}",
                sp.len(),
                sp.contains("Active Skills")
            );
            if !sp.is_empty() {
                msgs_with_context.insert(0, ChatMessage::system(&sp));
            }

            let mut retries = 0;
            let result = loop {
                match self
                    .adapter
                    .stream_message(&self.id, &msgs_with_context, app_handle, cancel.clone())
                    .await
                {
                    Ok(r) => break r,
                    Err(e) => {
                        let msg = e.to_string();
                        if retries < 2
                            && (msg.contains("500")
                                || msg.contains("503")
                                || msg.contains("429")
                                || msg.contains("timed out"))
                        {
                            retries += 1;
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            continue;
                        }
                        let err_msg = format!("API error: {}", msg);
                        // Don't stop the session — let user retry
                        let _ = app_handle.emit(
                            "session-output",
                            StreamEvent::Error {
                                session_id: self.id.clone(),
                                block_id: BlockId::new().to_string(),
                                message: err_msg.clone(),
                                code: "api_error".to_string(),
                            },
                        );
                        return Err(err_msg);
                    }
                }
            };

            // Save assistant response (DeepSeek Anthropic API requires thinking blocks in history)
            if !result.assistant_content.is_empty() {
                self.messages.lock().unwrap().push(ChatMessage::assistant(
                    serde_json::Value::Array(result.assistant_content.clone()),
                ));
            }

            // No tool calls = done
            if result.tool_calls.is_empty() {
                crate::app_log!("INFO", "Agent turn {}: no tool calls, done", _round);
                break;
            }

            crate::app_log!(
                "INFO",
                "Agent turn {}: {} tool calls to execute: {:?}",
                _round,
                result.tool_calls.len(),
                result
                    .tool_calls
                    .iter()
                    .map(|tc| tc.name.clone())
                    .collect::<Vec<_>>()
            );

            // Separate delegate_task calls from regular tool calls
            let (delegated, regular): (Vec<_>, Vec<_>) = result
                .tool_calls
                .iter()
                .partition(|tc| tc.name == "delegate_task");

            // Run delegated tasks as sub-agents in parallel
            let mut sub_results: Vec<(usize, String)> = Vec::new();
            if !delegated.is_empty() {
                let mut handles = Vec::new();
                for tc in &delegated {
                    let task = tc
                        .input
                        .get("task")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Investigate and report findings")
                        .to_string();
                    let adapter = self.adapter.clone();
                    let harness = self.harness.clone();
                    let app = app_handle.clone();
                    let cancel = self
                        .cancel
                        .lock()
                        .unwrap()
                        .clone()
                        .unwrap_or_else(|| Arc::new(Notify::new()));
                    let idx = result
                        .tool_calls
                        .iter()
                        .position(|t| t.id == tc.id)
                        .unwrap_or(0);
                    let wd = self.harness.working_dir.clone();
                    handles.push(tokio::spawn(async move {
                        let r = crate::agent::sub::SubAgent::run(
                            &task, adapter, harness, &app, cancel, &wd,
                        )
                        .await;
                        (idx, r)
                    }));
                }
                for handle in handles {
                    if let Ok((idx, r)) = handle.await {
                        // r is JSON: {"result": "...", "steps": [...]}
                        // Extract just the result text for the main agent's context
                        let api_text: String = serde_json::from_str::<serde_json::Value>(&r)
                            .ok()
                            .and_then(|v| {
                                v.get("result")
                                    .and_then(|r| r.as_str())
                                    .map(|s| s.to_string())
                            })
                            .unwrap_or_else(|| r.clone());
                        crate::app_log!(
                            "INFO",
                            "Agent sub-agent result ({} chars, api_text {} chars)",
                            r.len(),
                            api_text.len()
                        );
                        // Emit full JSON to frontend for SubAgentTrace rendering
                        if let Some(tc) = result.tool_calls.get(idx) {
                            let _ = app_handle.emit(
                                "session-output",
                                StreamEvent::ToolCallResult {
                                    session_id: self.id.clone(),
                                    block_id: tc.id.clone(),
                                    result: r.clone(),
                                    is_error: false,
                                    duration_ms: 0,
                                },
                            );
                        }
                        // Feed only the result text to the main agent
                        sub_results.push((idx, api_text));
                    }
                }
            }

            // Execute regular tools through the harness
            let (reads, writes): (
                Vec<&crate::adapters::base::ToolCall>,
                Vec<&crate::adapters::base::ToolCall>,
            ) = regular.iter().partition(|tc| is_read_only_tool(&tc.name));

            let mut read_results: Vec<(String, String)> = Vec::new();
            {
                let mut handles = Vec::new();
                for tc in &reads {
                    let h = self.harness.clone();
                    let sid = self.id.clone();
                    let name = tc.name.clone();
                    let input = tc.input.clone();
                    let app = app_handle.clone();
                    let id = tc.id.clone();
                    handles.push(tokio::spawn(async move {
                        let result = h
                            .execute_tool_with_block_id(&sid, &name, &input, &app, Some(&id))
                            .await;
                        (id, result)
                    }));
                }
                for handle in handles {
                    if let Ok((id, result)) = handle.await {
                        read_results.push((id, result));
                    }
                }
            }

            let mut write_results: Vec<(String, String)> = Vec::new();
            for tc in &writes {
                let result = self
                    .harness
                    .execute_tool_with_block_id(
                        &self.id,
                        &tc.name,
                        &tc.input,
                        app_handle,
                        Some(&tc.id),
                    )
                    .await;
                write_results.push((tc.id.clone(), result));
            }

            // Build results map: tool_call_id → result string
            let mut result_map: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for (id, r) in read_results {
                result_map.insert(id, r);
            }
            for (id, r) in write_results {
                result_map.insert(id, r);
            }
            for (idx, r) in sub_results {
                if let Some(tc) = result.tool_calls.get(idx) {
                    result_map.insert(tc.id.clone(), r);
                }
            }

            // Feed results back in original order, grouped into one user message
            let mut tool_results: Vec<serde_json::Value> = Vec::new();
            for tc in &result.tool_calls {
                let exec_result = result_map
                    .get(&tc.id)
                    .cloned()
                    .unwrap_or_else(|| "Tool result missing".to_string());
                crate::app_log!(
                    "INFO",
                    "Agent tool '{}' result ({} chars)",
                    tc.name,
                    exec_result.len()
                );
                tool_results.push(serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": &tc.id,
                    "content": exec_result,
                }));
            }
            self.messages.lock().unwrap().push(ChatMessage {
                role: "user".to_string(),
                content: serde_json::Value::Array(tool_results),
            });

            // Yield briefly so frontend receives & renders events before next API call
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        // Ensure final text response: append instruction, call API one more time if needed
        {
            let messages = self.messages.lock().unwrap().clone();
            let summary = self.summary.lock().unwrap().clone();
            let mut msgs =
                apply_turn_context(messages, summary.as_deref(), memory_context.as_deref());
            let sp = self.system_prompt.lock().unwrap().clone();
            if !sp.is_empty() {
                msgs.insert(0, ChatMessage::system(&sp));
            }
            let last_role = msgs.last().map(|m| m.role.clone()).unwrap_or_default();
            if last_role == "tool" || last_role == "user" {
                msgs.push(ChatMessage::user("Based on the above, provide your final answer as plain text. Do not use tools."));
                crate::app_log!("INFO", "Agent loop complete — requesting text-only summary");
                let _ = self
                    .adapter
                    .stream_message(&self.id, &msgs, app_handle, cancel.clone())
                    .await;
            }
        }

        crate::app_log!("INFO", "Agent loop complete");
        Ok(())
    }

    pub fn kill(&self, app_handle: &tauri::AppHandle) {
        self.running.store(false, Ordering::SeqCst);
        *self.status.lock().unwrap() = SessionStatus::Stopped;
        // Cancel in-flight HTTP stream
        if let Some(cancel) = self.cancel.lock().unwrap().take() {
            cancel.notify_one();
        }
        let _ = app_handle.emit(
            "session-output",
            StreamEvent::SessionStopped {
                session_id: self.id.clone(),
                reason: "killed".to_string(),
            },
        );
    }
}

fn apply_turn_context(
    messages: Vec<ChatMessage>,
    summary: Option<&str>,
    memory_context: Option<&str>,
) -> Vec<ChatMessage> {
    let mut with_context = Vec::new();
    if let Some(summary) = summary.filter(|summary| !summary.trim().is_empty()) {
        with_context.push(ChatMessage::user(&format!(
            "## Previous conversation summary\n{}",
            summary
        )));
    }
    if let Some(memory_context) = memory_context.filter(|context| !context.trim().is_empty()) {
        with_context.push(ChatMessage::user(memory_context));
    }
    with_context.extend(messages);
    with_context
}

#[derive(Debug, Clone)]
struct CompactResult {
    messages: Vec<ChatMessage>,
    summary: Option<String>,
    stats: Option<CompactStats>,
}

#[derive(Debug, Clone)]
struct CompactStats {
    summary: String,
    retained_messages: usize,
    compacted_messages: usize,
    estimated_tokens_before: usize,
    estimated_tokens_after: usize,
}

fn compact_messages_if_needed(
    msgs: Vec<ChatMessage>,
    existing_summary: Option<String>,
    context_window_tokens: Option<u32>,
) -> CompactResult {
    let existing_summary_tokens = existing_summary
        .as_deref()
        .map(estimate_text_tokens)
        .unwrap_or(0);
    let estimated_before = estimate_messages_tokens(&msgs) + existing_summary_tokens;
    let context_limit = context_window_tokens
        .map(|tokens| tokens as usize)
        .unwrap_or(DEFAULT_CONTEXT_WINDOW_TOKENS)
        .max(16_000);
    let compact_threshold = (context_limit * AUTO_COMPACT_THRESHOLD_NUMERATOR
        / AUTO_COMPACT_THRESHOLD_DENOMINATOR)
        .max(8_000);
    let over_budget = estimated_before > compact_threshold;
    let too_many_messages = msgs.len() > MAX_HISTORY_MESSAGES_BEFORE_COMPACT;

    if !over_budget && !too_many_messages {
        return CompactResult {
            messages: msgs,
            summary: existing_summary,
            stats: None,
        };
    }

    if msgs.len() <= RETAIN_RECENT_MESSAGES || msgs.len() <= MIN_COMPACT_MESSAGES {
        return CompactResult {
            messages: msgs,
            summary: existing_summary,
            stats: None,
        };
    }

    let split_at = msgs.len().saturating_sub(RETAIN_RECENT_MESSAGES);
    let start = (split_at..msgs.len())
        .find(|&i| msgs[i].role == "user" && !is_tool_result(&msgs[i]))
        .unwrap_or(split_at);

    if start < MIN_COMPACT_MESSAGES {
        return CompactResult {
            messages: msgs,
            summary: existing_summary,
            stats: None,
        };
    }

    let compacted_messages = msgs[..start].to_vec();
    let retained_messages = msgs[start..].to_vec();
    let new_summary = match build_summary(&compacted_messages) {
        Some(summary) => summary,
        None => {
            return CompactResult {
                messages: msgs,
                summary: existing_summary,
                stats: None,
            }
        }
    };
    let merged_summary = merge_summaries(existing_summary, new_summary);
    let estimated_after =
        estimate_messages_tokens(&retained_messages) + estimate_text_tokens(&merged_summary);
    let stats = CompactStats {
        summary: merged_summary.clone(),
        retained_messages: retained_messages.len(),
        compacted_messages: compacted_messages.len(),
        estimated_tokens_before: estimated_before,
        estimated_tokens_after: estimated_after,
    };

    CompactResult {
        messages: retained_messages,
        summary: Some(merged_summary),
        stats: Some(stats),
    }
}

fn build_summary(msgs: &[ChatMessage]) -> Option<String> {
    let mut lines = Vec::new();
    for msg in msgs {
        if let Some(line) = summarize_message(msg) {
            lines.push(format!(
                "- {}",
                truncate_chars(&line, MAX_SUMMARY_ITEM_CHARS)
            ));
        }
        if lines.len() >= 18 {
            break;
        }
    }

    if lines.is_empty() {
        return None;
    }

    let mut summary = String::from("[Earlier conversation summary]\n");
    for line in lines {
        summary.push_str(&line);
        summary.push('\n');
    }
    if msgs.len() > 18 {
        summary.push_str(&format!(
            "- ... {} older messages compacted\n",
            msgs.len() - 18
        ));
    }
    Some(summary.trim_end().to_string())
}

fn summarize_message(msg: &ChatMessage) -> Option<String> {
    let text = compact_content(&msg.content, MAX_SUMMARY_ITEM_CHARS);
    if text.is_empty() {
        return None;
    }
    let label = if is_tool_result(msg) {
        "Tool result"
    } else {
        match msg.role.as_str() {
            "assistant" => "Assistant",
            "system" => "System",
            "tool" => "Tool result",
            "user" => "User",
            other => other,
        }
    };
    Some(format!("{label}: {text}"))
}

fn merge_summaries(existing: Option<String>, update: String) -> String {
    let merged = match existing {
        Some(old) if !old.trim().is_empty() => format!("{}\n{}", old.trim(), update.trim()),
        _ => update,
    };
    truncate_chars(&merged, MAX_SUMMARY_CHARS)
}

fn compact_content(value: &serde_json::Value, limit: usize) -> String {
    let raw = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if let Some(part) = compact_content_block(item) {
                    parts.push(part);
                }
            }
            parts.join(" | ")
        }
        serde_json::Value::Object(_) => {
            if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
                text.to_string()
            } else if let Some(content) = value.get("content") {
                compact_content(content, limit)
            } else {
                serde_json::to_string(value).unwrap_or_default()
            }
        }
        other => other.to_string(),
    };
    truncate_chars(&collapse_whitespace(&raw), limit)
}

fn compact_content_block(block: &serde_json::Value) -> Option<String> {
    let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match block_type {
        "text" => block
            .get("text")
            .and_then(|v| v.as_str())
            .map(|text| text.to_string()),
        "tool_use" => {
            let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
            let input = block
                .get("input")
                .map(|v| compact_content(v, 120))
                .unwrap_or_default();
            Some(if input.is_empty() {
                format!("Tool requested: {name}")
            } else {
                format!("Tool requested: {name} ({input})")
            })
        }
        "tool_result" => block.get("content").map(|content| {
            format!(
                "Tool result: {}",
                compact_content(content, MAX_SUMMARY_ITEM_CHARS)
            )
        }),
        "thinking" | "redacted_thinking" => None,
        _ => {
            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                Some(text.to_string())
            } else if let Some(content) = block.get("content") {
                Some(compact_content(content, MAX_SUMMARY_ITEM_CHARS))
            } else {
                Some(serde_json::to_string(block).unwrap_or_default())
            }
        }
    }
}

fn estimate_messages_tokens(msgs: &[ChatMessage]) -> usize {
    msgs.iter().map(estimate_message_tokens).sum()
}

fn estimate_message_tokens(msg: &ChatMessage) -> usize {
    estimate_text_tokens(&msg.role) + estimate_value_tokens(&msg.content) + 8
}

fn estimate_value_tokens(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::String(s) => estimate_text_tokens(s),
        serde_json::Value::Array(items) => {
            items.iter().map(estimate_value_tokens).sum::<usize>() + (items.len() * 4)
        }
        serde_json::Value::Object(map) => {
            map.iter()
                .map(|(key, value)| estimate_text_tokens(key) + estimate_value_tokens(value))
                .sum::<usize>()
                + (map.len() * 4)
        }
        serde_json::Value::Null => 1,
        other => estimate_text_tokens(&other.to_string()),
    }
}

fn estimate_text_tokens(text: &str) -> usize {
    (text.chars().count() + 2) / 3
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    if limit <= 3 {
        return ".".repeat(limit);
    }
    let mut shortened = text.chars().take(limit - 3).collect::<String>();
    shortened.push_str("...");
    shortened
}

fn to_u32_tokens(value: usize) -> u32 {
    value.min(u32::MAX as usize) as u32
}

fn is_read_only_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "read"
            | "list_directory"
            | "ls"
            | "list"
            | "search_files"
            | "glob"
            | "search_content"
            | "grep"
            | "web_search"
            | "web_fetch"
            | "git_diff"
    )
}

fn is_tool_result(msg: &ChatMessage) -> bool {
    if let serde_json::Value::Array(ref blocks) = msg.content {
        blocks
            .iter()
            .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_result"))
    } else {
        false
    }
}
