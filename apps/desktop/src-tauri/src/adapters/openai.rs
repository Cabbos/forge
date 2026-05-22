use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use serde::Serialize;
use tauri::Emitter;

use super::base::{AdapterError, AiAdapter, ChatMessage, StreamResult, ToolCall};
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;

const DEFAULT_MODEL: &str = "gpt-4o";
const DEFAULT_BASE_URL: &str = "https://api.openai.com";

pub struct OpenAiAdapter {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenAiAdapter {
    pub fn new(api_key: String) -> Result<Self, AdapterError> {
        if api_key.trim().is_empty() {
            return Err(AdapterError::MissingApiKey);
        }
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|e| AdapterError::Http(e.to_string()))?;
        Ok(Self {
            api_key,
            model: DEFAULT_MODEL.to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
            client,
        })
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.to_string();
        self
    }
}

#[derive(Serialize)]
struct OpenAiRequest<'a> {
    model: &'a str,
    stream: bool,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    type_: String,
    function: OpenAiFunction,
}

#[derive(Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

fn openai_tools() -> Vec<OpenAiTool> {
    vec![
        OpenAiTool {
            type_: "function".to_string(),
            function: OpenAiFunction {
                name: "read_file".to_string(),
                description: "Read the contents of a file".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }),
            },
        },
        OpenAiTool {
            type_: "function".to_string(),
            function: OpenAiFunction {
                name: "write_to_file".to_string(),
                description: "Write or overwrite a file".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "content": {"type": "string"}
                    },
                    "required": ["path", "content"]
                }),
            },
        },
        OpenAiTool {
            type_: "function".to_string(),
            function: OpenAiFunction {
                name: "run_shell".to_string(),
                description: "Execute a shell command".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {"type": "string"}
                    },
                    "required": ["command"]
                }),
            },
        },
    ]
}

fn convert_messages(msgs: &[ChatMessage]) -> Vec<OpenAiMessage> {
    msgs.iter()
        .map(|m| {
            let role = match m.role.as_str() {
                "assistant" => "assistant".to_string(),
                _ => "user".to_string(),
            };
            OpenAiMessage {
                role,
                content: m.content.clone(),
                tool_calls: None,
                tool_call_id: None,
            }
        })
        .collect()
}

#[async_trait]
impl AiAdapter for OpenAiAdapter {
    fn model_id(&self) -> &str {
        &self.model
    }

    fn model_name(&self) -> &str {
        match self.model.as_str() {
            "gpt-4o" => "GPT-4o",
            "gpt-4-turbo" => "GPT-4 Turbo",
            "gpt-4.1" => "GPT-4.1",
            _ => "GPT",
        }
    }

    async fn stream_message(
        &self,
        session_id: &str,
        messages: &[ChatMessage],
        app_handle: &tauri::AppHandle,
    ) -> Result<StreamResult, AdapterError> {
        let body = OpenAiRequest {
            model: &self.model,
            stream: true,
            messages: convert_messages(messages),
            tools: Some(openai_tools()),
        };

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AdapterError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(AdapterError::Http(format!("HTTP {status}: {text}")));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut current_text = String::new();
        let mut active_block_id: Option<String> = None;
        let mut block_started = false;

        // Tool call accumulation (OpenAI streams tool calls differently)
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut assistant_content: Vec<serde_json::Value> = Vec::new();
        let mut stop_reason: Option<String> = None;

        let session = session_id.to_string();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AdapterError::Stream(e.to_string()))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text.replace("\r\n", "\n"));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                let data = match line.strip_prefix("data: ") {
                    Some(d) if d != "[DONE]" => d.to_string(),
                    _ => continue,
                };

                let parsed: serde_json::Value = match serde_json::from_str(&data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let choices = match parsed.get("choices") {
                    Some(c) => c,
                    None => continue,
                };

                let choice = match choices.get(0) {
                    Some(c) => c,
                    None => continue,
                };

                let delta = match choice.get("delta") {
                    Some(d) => d,
                    None => continue,
                };

                // Handle text content
                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                    if !content.is_empty() {
                        if !block_started {
                            let bid = BlockId::new().to_string();
                            active_block_id = Some(bid.clone());
                            block_started = true;
                            crate::transcript::emit_stream_event(&app_handle, StreamEvent::TextStart {
                                    session_id: session.clone(),
                                    block_id: bid,
                                });
                        }
                        current_text.push_str(content);
                        crate::transcript::emit_stream_event(&app_handle, StreamEvent::TextChunk {
                                session_id: session.clone(),
                                block_id: active_block_id.clone().unwrap_or_default(),
                                content: content.to_string(),
                            });
                    }
                }

                // Handle tool calls
                if let Some(tool_deltas) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                    for tc in tool_deltas {
                        let idx = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                        let id = tc
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or("")
                            .to_string();
                        let func = tc.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
                        let args = tc
                            .get("function")
                            .and_then(|f| f.get("arguments"))
                            .and_then(|a| a.as_str())
                            .unwrap_or("");

                        // Ensure we have enough entries
                        while tool_calls.len() <= idx {
                            tool_calls.push(ToolCall {
                                id: String::new(),
                                name: String::new(),
                                input: serde_json::Value::Null,
                            });
                        }
                        if !id.is_empty() {
                            tool_calls[idx].id = id;
                        }
                        if !func.is_empty() {
                            tool_calls[idx].name = func.to_string();
                        }
                        if !args.is_empty() {
                            let accumulated = format!(
                                "{}{}",
                                tool_calls[idx]
                                    .input
                                    .as_str()
                                    .unwrap_or(""),
                                args
                            );
                            tool_calls[idx].input =
                                serde_json::Value::String(accumulated);
                        }
                    }
                }

                // Handle finish
                if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
                    if reason != "null" && !reason.is_empty() {
                        stop_reason = Some(reason.to_string());

                        // End text block if was writing
                        if block_started {
                            let bid = active_block_id.clone().unwrap_or_default();
                            crate::transcript::emit_stream_event(&app_handle, StreamEvent::TextEnd {
                                    session_id: session.clone(),
                                    block_id: bid,
                                });
                            if !current_text.is_empty() {
                                assistant_content.push(serde_json::json!({
                                    "type": "text",
                                    "text": current_text.clone(),
                                }));
                            }
                            block_started = false;
                        }

                        // Finalize tool calls — parse accumulated JSON args
                        for tc in &mut tool_calls {
                            if let serde_json::Value::String(ref s) = tc.input {
                                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                                    tc.input = parsed;
                                }
                            }
                            assistant_content.push(serde_json::json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": tc.input,
                            }));
                        }

                        crate::transcript::emit_stream_event(&app_handle, StreamEvent::SessionStatus {
                                session_id: session.clone(),
                                status: "idle".to_string(),
                            });
                    }
                }
            }
        }

        // Flush any remaining text
        if block_started && !current_text.is_empty() {
            assistant_content.push(serde_json::json!({
                "type": "text",
                "text": current_text,
            }));
        }

        Ok(StreamResult {
            assistant_content,
            tool_calls,
            stop_reason,
        })
    }
}
