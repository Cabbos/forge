use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

const ORPHAN_TOOL_RESULT_PREVIEW_LIMIT: usize = 4000;

/// A chat message in the format expected by AI APIs.
/// Content can be a plain string (for simple text) or a JSON value
/// (for structured content like tool_use and tool_result blocks).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,               // "user" | "assistant"
    pub content: serde_json::Value, // String or Vec<ContentBlock>
}

impl ChatMessage {
    pub fn user(text: &str) -> Self {
        ChatMessage {
            role: "user".to_string(),
            content: serde_json::Value::String(text.to_string()),
        }
    }

    pub fn assistant(content: serde_json::Value) -> Self {
        ChatMessage {
            role: "assistant".to_string(),
            content,
        }
    }

    pub fn system(text: &str) -> Self {
        ChatMessage {
            role: "system".to_string(),
            content: serde_json::Value::String(text.to_string()),
        }
    }

    pub fn tool_result(tool_use_id: &str, result: &str) -> Self {
        ChatMessage {
            role: "user".to_string(),
            content: serde_json::json!([{
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                "content": result,
            }]),
        }
    }

    /// Create a tool result message with OpenAI-compatible role and tool_call_id tracking.
    pub fn tool(tool_use_id: &str, result: &str) -> Self {
        ChatMessage {
            role: "tool".to_string(),
            content: serde_json::json!({
                "tool_call_id": tool_use_id,
                "content": result,
            }),
        }
    }
}

/// Repair Anthropic-style tool_use/tool_result adjacency before provider requests.
///
/// Some providers reject any assistant `tool_use` that is not immediately followed by
/// matching `tool_result` blocks. This guard turns interrupted turns into explicit
/// synthetic error results so the next request can continue instead of failing with
/// a provider-side 400.
pub fn repair_tool_result_adjacency(messages: &[ChatMessage]) -> Vec<ChatMessage> {
    let mut repaired = Vec::with_capacity(messages.len());
    let mut iter = messages.iter().cloned().peekable();

    while let Some(message) = iter.next() {
        if let Some(message) = openai_tool_message_as_tool_result(&message) {
            repaired.push(orphan_tool_result_message_as_text(&message));
            continue;
        }
        if message.role == "user" && message_has_tool_results(&message) {
            repaired.push(orphan_tool_result_message_as_text(&message));
            continue;
        }

        let pending_tool_uses = assistant_tool_uses(&message);
        if pending_tool_uses.is_empty() {
            repaired.push(message);
            continue;
        }

        repaired.push(message);

        let Some(next_message) = iter.peek_mut() else {
            repaired.push(synthetic_tool_result_message(&pending_tool_uses));
            continue;
        };

        if openai_tool_message_as_tool_result(next_message).is_some() {
            let mut tool_result_message = ChatMessage {
                role: "user".to_string(),
                content: serde_json::Value::Array(Vec::new()),
            };
            while let Some(converted_tool_message) =
                iter.peek().and_then(openai_tool_message_as_tool_result)
            {
                merge_tool_result_blocks(&mut tool_result_message, converted_tool_message);
                iter.next();
            }
            append_missing_tool_results(&mut tool_result_message, &pending_tool_uses);
            repaired.push(tool_result_message);
        } else if next_message.role == "user" && message_has_tool_results(next_message) {
            let mut tool_result_message = iter.next().expect("peeked message must exist");
            append_missing_tool_results(&mut tool_result_message, &pending_tool_uses);
            repaired.push(tool_result_message);
        } else {
            repaired.push(synthetic_tool_result_message(&pending_tool_uses));
        }
    }

    repaired
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingToolUse {
    id: String,
    name: String,
}

fn assistant_tool_uses(message: &ChatMessage) -> Vec<PendingToolUse> {
    if message.role != "assistant" {
        return Vec::new();
    }
    message
        .content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(|value| value.as_str()) == Some("tool_use"))
        .filter_map(|block| {
            let id = block
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .trim();
            if id.is_empty() {
                return None;
            }
            Some(PendingToolUse {
                id: id.to_string(),
                name: block
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown_tool")
                    .to_string(),
            })
        })
        .collect()
}

fn message_has_tool_results(message: &ChatMessage) -> bool {
    message.content.as_array().is_some_and(|blocks| {
        blocks
            .iter()
            .any(|block| block.get("type").and_then(|value| value.as_str()) == Some("tool_result"))
    })
}

fn orphan_tool_result_message_as_text(message: &ChatMessage) -> ChatMessage {
    let ids = tool_result_ids(message);
    let suffix = if ids.is_empty() {
        String::new()
    } else {
        format!(" Tool result ids: {}.", ids.join(", "))
    };
    let payload = compact_json_preview(&message.content, ORPHAN_TOOL_RESULT_PREVIEW_LIMIT);
    ChatMessage::user(&format!(
        "Discarded orphan tool result while repairing chat history.{suffix} Original tool result payload: {payload}. Re-check the current workspace state before relying on previous tool output."
    ))
}

fn compact_json_preview(value: &serde_json::Value, limit: usize) -> String {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "<unserializable>".to_string());
    if text.chars().count() <= limit {
        return text;
    }
    let preview: String = text.chars().take(limit).collect();
    format!("{preview}...<truncated>")
}

fn tool_result_ids(message: &ChatMessage) -> Vec<String> {
    message
        .content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(|value| value.as_str()) == Some("tool_result"))
        .filter_map(|block| block.get("tool_use_id").and_then(|value| value.as_str()))
        .map(ToString::to_string)
        .collect()
}

fn openai_tool_message_as_tool_result(message: &ChatMessage) -> Option<ChatMessage> {
    if message.role != "tool" {
        return None;
    }
    let content = message.content.as_object()?;
    let tool_use_id = content
        .get("tool_call_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();
    if tool_use_id.is_empty() {
        return None;
    }

    Some(ChatMessage {
        role: "user".to_string(),
        content: serde_json::json!([{
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content
                .get("content")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::String(String::new())),
        }]),
    })
}

fn merge_tool_result_blocks(target: &mut ChatMessage, source: ChatMessage) {
    let Some(target_blocks) = target.content.as_array_mut() else {
        return;
    };
    let Some(source_blocks) = source.content.as_array() else {
        return;
    };
    target_blocks.extend(source_blocks.iter().cloned());
}

fn append_missing_tool_results(message: &mut ChatMessage, pending_tool_uses: &[PendingToolUse]) {
    let Some(blocks) = message.content.as_array_mut() else {
        return;
    };
    let pending_ids = pending_tool_uses
        .iter()
        .map(|tool_use| tool_use.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    blocks.retain(|block| {
        if block.get("type").and_then(|value| value.as_str()) != Some("tool_result") {
            return true;
        }
        block
            .get("tool_use_id")
            .and_then(|value| value.as_str())
            .is_some_and(|id| pending_ids.contains(id))
    });
    let existing = blocks
        .iter()
        .filter(|block| block.get("type").and_then(|value| value.as_str()) == Some("tool_result"))
        .filter_map(|block| block.get("tool_use_id").and_then(|value| value.as_str()))
        .collect::<std::collections::HashSet<_>>();

    let missing = pending_tool_uses
        .iter()
        .filter(|tool_use| !existing.contains(tool_use.id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    blocks.extend(synthetic_tool_result_blocks(
        &missing,
        "previous_tool_call_interrupted",
    ));
}

fn synthetic_tool_result_message(pending_tool_uses: &[PendingToolUse]) -> ChatMessage {
    ChatMessage {
        role: "user".to_string(),
        content: serde_json::Value::Array(synthetic_tool_result_blocks(
            pending_tool_uses,
            "previous_tool_call_interrupted",
        )),
    }
}

fn synthetic_tool_result_blocks(
    pending_tool_uses: &[PendingToolUse],
    reason: &str,
) -> Vec<serde_json::Value> {
    pending_tool_uses
        .iter()
        .map(|tool_use| {
            serde_json::json!({
                "type": "tool_result",
                "tool_use_id": tool_use.id,
                "content": format!(
                    "Tool result unavailable: {reason}. The previous tool call was interrupted before Forge could capture its result. Tool: '{}'. Re-check the current workspace state before continuing.",
                    tool_use.name
                ),
                "is_error": true
            })
        })
        .collect()
}

/// A tool call extracted from the streaming response.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolDef {
    pub fn new(name: &str, description: &str, input_schema: serde_json::Value) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
        }
    }
}

/// Result of a streaming call — assistant content + any tool calls to execute.
#[derive(Debug, Clone)]
pub struct StreamResult {
    /// The full assistant message content blocks (for history).
    pub assistant_content: Vec<serde_json::Value>,
    /// Tool calls that need local execution.
    pub tool_calls: Vec<ToolCall>,
    /// Stop reason from the API.
    pub stop_reason: Option<String>,
}

/// Errors from AI adapters.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Stream error: {0}")]
    Stream(String),
    #[error("API error: {code} — {message}")]
    Api { code: String, message: String },
    #[error("Missing API key")]
    MissingApiKey,
}

/// Each AI provider adapter implements this trait.
#[async_trait]
pub trait AiAdapter: Send + Sync {
    /// Stream a message to the AI API and emit events to the frontend.
    async fn stream_message(
        &self,
        session_id: &str,
        messages: &[ChatMessage],
        app_handle: &tauri::AppHandle,
        cancel: std::sync::Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        let emitter = crate::agent::event_sink::TauriEventEmitter::new(app_handle.clone());
        self.stream_message_with_emitter(session_id, messages, &emitter, cancel)
            .await
    }

    /// Stream a message and send provider events through the shared event sink.
    ///
    /// Implement streaming providers here so production and tests observe the
    /// same event path. The `stream_message` AppHandle wrapper above exists for
    /// legacy call sites only.
    async fn stream_message_with_emitter(
        &self,
        session_id: &str,
        messages: &[ChatMessage],
        emitter: &dyn crate::agent::event_sink::EventEmitter,
        cancel: std::sync::Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        self.call_with_emitter(session_id, messages, emitter, cancel)
            .await
    }

    /// Call the AI API without emitting any frontend events.
    /// Used by sub-agents that shouldn't pollute the main UI.
    async fn call(
        &self,
        messages: &[ChatMessage],
        cancel: std::sync::Arc<Notify>,
    ) -> Result<StreamResult, AdapterError>;

    /// Call the model for context compaction summary generation.
    ///
    /// Providers should disable normal agent tools here: a compaction pass must
    /// summarize already-observed context, not mutate the workspace or ask for
    /// new tool results.
    async fn compact_summary(
        &self,
        messages: &[ChatMessage],
        cancel: std::sync::Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        self.call(messages, cancel).await
    }

    /// Call the AI API without frontend streaming, optionally emitting telemetry.
    ///
    /// Sub-agent call sites use this path so they preserve the same tool surface
    /// as `call()` while adapters that already know response usage can emit it.
    /// The safe default delegates to `call()` and ignores the emitter.
    async fn call_with_emitter(
        &self,
        _session_id: &str,
        messages: &[ChatMessage],
        _emitter: &dyn crate::agent::event_sink::EventEmitter,
        cancel: std::sync::Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        self.call(messages, cancel).await
    }

    /// Model identifier (e.g. "claude-sonnet-4-6").
    fn model_id(&self) -> &str;

    /// Human-readable model name.
    fn model_name(&self) -> &str;

    /// True when this adapter is a placeholder waiting for user credentials.
    fn is_missing_api_key_adapter(&self) -> bool {
        false
    }

    /// Replace external tools, such as MCP tools discovered after the session was created.
    fn set_external_tools(&self, _tools: Vec<ToolDef>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_result_ids(message: &ChatMessage) -> Vec<String> {
        message
            .content
            .as_array()
            .into_iter()
            .flatten()
            .filter(|block| {
                block.get("type").and_then(|value| value.as_str()) == Some("tool_result")
            })
            .filter_map(|block| block.get("tool_use_id").and_then(|value| value.as_str()))
            .map(ToString::to_string)
            .collect()
    }

    fn tool_use_ids(message: &ChatMessage) -> Vec<String> {
        message
            .content
            .as_array()
            .into_iter()
            .flatten()
            .filter(|block| block.get("type").and_then(|value| value.as_str()) == Some("tool_use"))
            .filter_map(|block| block.get("id").and_then(|value| value.as_str()))
            .map(ToString::to_string)
            .collect()
    }

    fn assert_provider_tool_result_contract(messages: &[ChatMessage]) {
        for (index, message) in messages.iter().enumerate() {
            let tool_use_ids = tool_use_ids(message);
            if tool_use_ids.is_empty() {
                continue;
            }

            let next = messages.get(index + 1).unwrap_or_else(|| {
                panic!("assistant tool_use at {index} has no following message")
            });
            assert_eq!(
                next.role, "user",
                "assistant tool_use at {index} must be followed by a user tool_result message"
            );
            let result_ids = tool_result_ids(next);
            for tool_use_id in &tool_use_ids {
                assert!(
                    result_ids.contains(tool_use_id),
                    "assistant tool_use {tool_use_id} at {index} is missing an immediate tool_result; got {result_ids:?}"
                );
            }
        }
    }

    #[test]
    fn repair_tool_result_adjacency_inserts_missing_result_before_follow_up() {
        let messages = vec![
            ChatMessage::assistant(serde_json::json!([{
                "type": "tool_use",
                "id": "call_1",
                "name": "read_file",
                "input": {"path": "src/App.tsx"}
            }])),
            ChatMessage::user("继续"),
        ];

        let repaired = repair_tool_result_adjacency(&messages);

        assert_eq!(repaired.len(), 3);
        assert_eq!(tool_result_ids(&repaired[1]), vec!["call_1"]);
        assert_provider_tool_result_contract(&repaired);
        assert_eq!(
            repaired[2].content,
            serde_json::Value::String("继续".to_string())
        );
    }

    #[test]
    fn repair_tool_result_adjacency_fills_partial_result_message() {
        let messages = vec![
            ChatMessage::assistant(serde_json::json!([
                {
                    "type": "tool_use",
                    "id": "call_1",
                    "name": "read_file",
                    "input": {"path": "src/App.tsx"}
                },
                {
                    "type": "tool_use",
                    "id": "call_2",
                    "name": "read_file",
                    "input": {"path": "src/main.tsx"}
                }
            ])),
            ChatMessage {
                role: "user".to_string(),
                content: serde_json::json!([{
                    "type": "tool_result",
                    "tool_use_id": "call_1",
                    "content": "ok"
                }]),
            },
        ];

        let repaired = repair_tool_result_adjacency(&messages);

        assert_eq!(repaired.len(), 2);
        assert_eq!(tool_result_ids(&repaired[1]), vec!["call_1", "call_2"]);
        assert_provider_tool_result_contract(&repaired);
    }

    #[test]
    fn repair_tool_result_adjacency_removes_unmatched_result_blocks() {
        let messages = vec![
            ChatMessage::assistant(serde_json::json!([{
                "type": "tool_use",
                "id": "call_1",
                "name": "read_file",
                "input": {"path": "src/App.tsx"}
            }])),
            ChatMessage {
                role: "user".to_string(),
                content: serde_json::json!([
                    {
                        "type": "tool_result",
                        "tool_use_id": "old_call",
                        "content": "stale result"
                    },
                    {
                        "type": "tool_result",
                        "tool_use_id": "call_1",
                        "content": "ok"
                    }
                ]),
            },
        ];

        let repaired = repair_tool_result_adjacency(&messages);

        assert_eq!(repaired.len(), 2);
        assert_eq!(tool_result_ids(&repaired[1]), vec!["call_1"]);
        assert!(!repaired[1].content.to_string().contains("old_call"));
        assert_provider_tool_result_contract(&repaired);
    }

    #[test]
    fn repair_tool_result_adjacency_preserves_valid_history() {
        let messages = vec![
            ChatMessage::assistant(serde_json::json!([{
                "type": "tool_use",
                "id": "call_1",
                "name": "read_file",
                "input": {"path": "src/App.tsx"}
            }])),
            ChatMessage::tool_result("call_1", "ok"),
            ChatMessage::user("继续"),
        ];

        let repaired = repair_tool_result_adjacency(&messages);

        assert_eq!(repaired.len(), 3);
        assert_eq!(repaired[1].content, messages[1].content);
        assert_provider_tool_result_contract(&repaired);
    }

    #[test]
    fn repair_tool_result_adjacency_converts_openai_tool_message() {
        let messages = vec![
            ChatMessage::assistant(serde_json::json!([{
                "type": "tool_use",
                "id": "call_1",
                "name": "read_file",
                "input": {"path": "src/App.tsx"}
            }])),
            ChatMessage::tool("call_1", "ok"),
            ChatMessage::user("继续"),
        ];

        let repaired = repair_tool_result_adjacency(&messages);

        assert_eq!(repaired.len(), 3);
        assert_eq!(repaired[1].role, "user");
        assert_eq!(tool_result_ids(&repaired[1]), vec!["call_1"]);
        assert_provider_tool_result_contract(&repaired);
        assert_eq!(
            repaired[2].content,
            serde_json::Value::String("继续".to_string())
        );
    }

    #[test]
    fn repair_tool_result_adjacency_combines_consecutive_openai_tool_messages() {
        let messages = vec![
            ChatMessage::assistant(serde_json::json!([
                {
                    "type": "tool_use",
                    "id": "call_1",
                    "name": "read_file",
                    "input": {"path": "src/App.tsx"}
                },
                {
                    "type": "tool_use",
                    "id": "call_2",
                    "name": "read_file",
                    "input": {"path": "src/main.tsx"}
                }
            ])),
            ChatMessage::tool("call_1", "app"),
            ChatMessage::tool("call_2", "main"),
            ChatMessage::user("继续"),
        ];

        let repaired = repair_tool_result_adjacency(&messages);

        assert_eq!(repaired.len(), 3);
        assert_eq!(repaired[1].role, "user");
        assert_eq!(tool_result_ids(&repaired[1]), vec!["call_1", "call_2"]);
        assert_provider_tool_result_contract(&repaired);
        assert_eq!(
            repaired[2].content,
            serde_json::Value::String("继续".to_string())
        );
    }

    #[test]
    fn repair_tool_result_adjacency_recovers_multiple_broken_turns_before_provider_call() {
        let messages = vec![
            ChatMessage::system("system rules"),
            ChatMessage::user("先检查项目"),
            ChatMessage::assistant(serde_json::json!([
                {
                    "type": "tool_use",
                    "id": "call_a",
                    "name": "read_file",
                    "input": {"path": "src/App.tsx"}
                },
                {
                    "type": "tool_use",
                    "id": "call_b",
                    "name": "bash",
                    "input": {"command": "npm install"}
                }
            ])),
            ChatMessage::tool("call_a", "app content"),
            ChatMessage::user("中断了，继续"),
            ChatMessage::assistant(serde_json::json!([{
                "type": "tool_use",
                "id": "call_c",
                "name": "bash",
                "input": {"command": "npm run build"}
            }])),
        ];

        let repaired = repair_tool_result_adjacency(&messages);

        assert_provider_tool_result_contract(&repaired);
        assert_eq!(tool_result_ids(&repaired[3]), vec!["call_a", "call_b"]);
        assert_eq!(
            tool_result_ids(repaired.last().expect("synthetic result")),
            vec!["call_c"]
        );
        assert!(
            repaired[3].content.to_string().contains("interrupted")
                || repaired[3].content.to_string().contains("missing")
                || repaired[3].content.to_string().contains("unavailable")
        );
    }

    #[test]
    fn repair_tool_result_adjacency_downgrades_orphan_tool_result() {
        let messages = vec![
            ChatMessage::tool_result("call_1", "ok"),
            ChatMessage::user("继续"),
        ];

        let repaired = repair_tool_result_adjacency(&messages);

        assert_eq!(repaired.len(), 2);
        assert_eq!(repaired[0].role, "user");
        assert!(tool_result_ids(&repaired[0]).is_empty());
        assert!(repaired[0]
            .content
            .as_str()
            .unwrap_or_default()
            .contains("Discarded orphan tool result"));
        assert!(repaired[0]
            .content
            .as_str()
            .unwrap_or_default()
            .contains("ok"));
        assert_eq!(
            repaired[1].content,
            serde_json::Value::String("继续".to_string())
        );
    }
}
