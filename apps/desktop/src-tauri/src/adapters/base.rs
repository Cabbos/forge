use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

/// A chat message in the format expected by AI APIs.
/// Content can be a plain string (for simple text) or a JSON value
/// (for structured content like tool_use and tool_result blocks).
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// A tool call extracted from the streaming response.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
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
    ) -> Result<StreamResult, AdapterError>;

    /// Call the AI API without emitting any frontend events.
    /// Used by sub-agents that shouldn't pollute the main UI.
    async fn call(
        &self,
        messages: &[ChatMessage],
        cancel: std::sync::Arc<Notify>,
    ) -> Result<StreamResult, AdapterError>;

    /// Model identifier (e.g. "claude-sonnet-4-6").
    fn model_id(&self) -> &str;

    /// Human-readable model name.
    fn model_name(&self) -> &str;

    /// True when this adapter is a placeholder waiting for user credentials.
    fn is_missing_api_key_adapter(&self) -> bool {
        false
    }
}
