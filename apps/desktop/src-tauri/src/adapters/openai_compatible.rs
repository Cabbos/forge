use async_trait::async_trait;
use futures::StreamExt;
use serde::Serialize;
use tauri::Emitter;

use super::base::{AdapterError, AiAdapter, ChatMessage, StreamResult, ToolCall};
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;

const DEFAULT_MODEL: &str = "deepseek-chat";

const SYSTEM_PROMPT: &str = "\
You are a powerful AI coding agent running in a desktop GUI application. You have direct access to the user's filesystem and shell.

## Your capabilities
- Read, write, and edit files on the user's machine
- Execute shell commands (build, test, git, package management, etc.)
- Search code with glob patterns and regex
- Fetch information from the web

## Important rules
- Never assume file contents — always read first
- Make targeted edits rather than rewriting entire files
- Run build/test commands to verify your changes
- Keep responses concise and actionable";

pub struct OpenAiCompatibleAdapter {
    api_key: String,
    model: String,
    base_url: String,
    max_tokens: u32,
    client: reqwest::Client,
}

impl OpenAiCompatibleAdapter {
    pub fn new(api_key: String) -> Result<Self, AdapterError> {
        if api_key.trim().is_empty() {
            return Err(AdapterError::MissingApiKey);
        }
        Ok(Self {
            api_key,
            model: DEFAULT_MODEL.to_string(),
            base_url: "https://api.deepseek.com/v1".to_string(),
            max_tokens: 8192,
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(10))
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .map_err(|e| AdapterError::Http(e.to_string()))?,
        })
    }

    pub fn with_model(mut self, model: &str) -> Self { self.model = model.to_string(); self }
    pub fn with_base_url(mut self, url: &str) -> Self { self.base_url = url.to_string(); self }
}

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCallMsg>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize)]
struct OpenAiToolCallMsg {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    function: OpenAiFunctionCall,
}

#[derive(Serialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    type_: String,
    function: OpenAiFunctionDef,
}

#[derive(Serialize)]
struct OpenAiFunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[async_trait]
impl AiAdapter for OpenAiCompatibleAdapter {
    fn model_id(&self) -> &str { &self.model }
    fn model_name(&self) -> &str {
        match self.model.as_str() {
            "deepseek-chat" => "DeepSeek Chat",
            "deepseek-reasoner" => "DeepSeek Reasoner",
            "gpt-4o" => "GPT-4o",
            "gpt-4o-mini" => "GPT-4o Mini",
            _ => &self.model,
        }
    }

    async fn stream_message(
        &self,
        session_id: &str,
        messages: &[ChatMessage],
        app_handle: &tauri::AppHandle,
    ) -> Result<StreamResult, AdapterError> {
        let openai_msgs = convert_messages(messages);

        let tools: Vec<OpenAiTool> = tool_definitions().into_iter().map(|td| OpenAiTool {
            type_: "function".to_string(),
            function: OpenAiFunctionDef {
                name: td.name,
                description: td.description,
                parameters: td.input_schema,
            },
        }).collect();

        let request = OpenAiRequest {
            model: self.model.clone(),
            messages: openai_msgs,
            stream: true,
            max_tokens: Some(self.max_tokens),
            tools: if tools.is_empty() { None } else { Some(tools) },
        };

        let response = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
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

        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut assistant_content: Vec<serde_json::Value> = Vec::new();
        let mut stop_reason: Option<String> = None;
        let mut current_text = String::new();
        let mut active_text_block_id: Option<String> = None;
        let mut tool_call_buffers: Vec<(usize, String, String, String)> = Vec::new(); // (idx, id, name, args_json)

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AdapterError::Stream(e.to_string()))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            while let Some(event_end) = buffer.find("\n\n") {
                let event_data = buffer[..event_end].to_string();
                buffer = buffer[event_end + 2..].to_string();

                let data: String = event_data
                    .lines()
                    .filter_map(|l| l.strip_prefix("data: "))
                    .collect::<Vec<_>>()
                    .join("");

                if data.is_empty() || data == "[DONE]" { continue; }

                let parsed: serde_json::Value = match serde_json::from_str(&data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let session = session_id.to_string();

                if let Some(choices) = parsed["choices"].as_array() {
                    for choice in choices {
                        let finish = choice["finish_reason"].as_str().unwrap_or("");
                        if !finish.is_empty() {
                            stop_reason = Some(finish.to_string());
                        }

                        let delta = &choice["delta"];

                        // Text content — reuse same block_id for streaming continuity
                        if let Some(content) = delta["content"].as_str() {
                            if current_text.is_empty() {
                                active_text_block_id = Some(BlockId::new().to_string());
                                let _ = app_handle.emit("session-output",
                                    StreamEvent::TextStart { session_id: session.clone(), block_id: active_text_block_id.clone().unwrap() });
                            }
                            current_text.push_str(content);
                            let _ = app_handle.emit("session-output",
                                StreamEvent::TextChunk { session_id: session.clone(), block_id: active_text_block_id.clone().unwrap_or_default(), content: content.to_string() });
                        }

                        // Tool calls
                        if let Some(tcs) = delta["tool_calls"].as_array() {
                            for tc in tcs {
                                let idx = tc["index"].as_u64().unwrap_or(0) as usize;

                                // Find or create buffer slot
                                while tool_call_buffers.len() <= idx {
                                    tool_call_buffers.push((tool_call_buffers.len(), String::new(), String::new(), String::new()));
                                }

                                if let Some(id) = tc["id"].as_str() {
                                    tool_call_buffers[idx].1 = id.to_string();
                                }
                                if let Some(func) = tc["function"].as_object() {
                                    if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
                                        if !name.is_empty() { tool_call_buffers[idx].2 = name.to_string(); }
                                    }
                                    if let Some(args) = func.get("arguments").and_then(|v| v.as_str()) {
                                        tool_call_buffers[idx].3.push_str(args);
                                    }
                                }
                            }
                        }
                    }
                }

                // Usage
                if let Some(usage) = parsed["usage"].as_object() {
                    let input_toks = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let output_toks = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let cost = crate::adapters::anthropic::estimate_cost(&self.model, input_toks, output_toks);
                    let _ = app_handle.emit("session-output",
                        StreamEvent::Usage { session_id: session.clone(), input_tokens: input_toks, output_tokens: output_toks, estimated_cost_usd: cost });
                }

                // Error
                if parsed["error"].is_object() {
                    let msg = parsed["error"]["message"].as_str().unwrap_or("Unknown error");
                    let _ = app_handle.emit("session-output",
                        StreamEvent::Error { session_id: session.clone(), block_id: BlockId::new().to_string(), message: msg.to_string(), code: "api_error".to_string() });
                }
            }
        }

        // Flush remaining text — reuse the active block_id for continuity
        if !current_text.is_empty() {
            assistant_content.push(serde_json::json!({"type":"text","text":current_text}));
            let bid = active_text_block_id.unwrap_or_else(|| BlockId::new().to_string());
            let _ = app_handle.emit("session-output",
                StreamEvent::TextEnd { session_id: session_id.to_string(), block_id: bid });
        }

        // Convert completed tool call buffers
        for (idx, id, name, args_json) in &tool_call_buffers {
            if !id.is_empty() && !name.is_empty() {
                let input: serde_json::Value = serde_json::from_str(args_json)
                    .unwrap_or(serde_json::Value::Object(Default::default()));
                tool_calls.push(ToolCall { id: id.clone(), name: name.clone(), input: input.clone() });
                let bid = BlockId::new().to_string();
                let _ = app_handle.emit("session-output",
                    StreamEvent::ToolCallStart { session_id: session_id.to_string(), block_id: bid.clone(), tool_name: name.clone(), tool_input: input.clone() });
                let _ = app_handle.emit("session-output",
                    StreamEvent::ToolCallEnd { session_id: session_id.to_string(), block_id: bid });
            }
        }

        Ok(StreamResult { assistant_content, tool_calls, stop_reason })
    }
}

fn convert_messages(messages: &[ChatMessage]) -> Vec<OpenAiMessage> {
    let mut result = vec![OpenAiMessage {
        role: "system".to_string(),
        content: serde_json::Value::String(SYSTEM_PROMPT.to_string()),
        tool_calls: None,
        tool_call_id: None,
    }];

    for msg in messages {
        match &msg.content {
            serde_json::Value::String(s) => {
                result.push(OpenAiMessage {
                    role: msg.role.clone(),
                    content: serde_json::Value::String(s.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            serde_json::Value::Array(blocks) => {
                if msg.role == "assistant" {
                    let mut text_parts = Vec::new();
                    let mut tool_calls: Vec<OpenAiToolCallMsg> = Vec::new();
                    for block in blocks {
                        match block["type"].as_str() {
                            Some("text") => {
                                if let Some(t) = block["text"].as_str() {
                                    text_parts.push(t.to_string());
                                }
                            }
                            Some("tool_use") => {
                                let id = block["id"].as_str().unwrap_or("").to_string();
                                let name = block["name"].as_str().unwrap_or("").to_string();
                                let args = block["input"].to_string();
                                tool_calls.push(OpenAiToolCallMsg {
                                    id, type_: "function".to_string(),
                                    function: OpenAiFunctionCall { name, arguments: args },
                                });
                            }
                            _ => {}
                        }
                    }
                    let content = if !text_parts.is_empty() {
                        serde_json::Value::String(text_parts.join("\n"))
                    } else {
                        serde_json::Value::Null
                    };
                    result.push(OpenAiMessage {
                        role: "assistant".to_string(),
                        content,
                        tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                        tool_call_id: None,
                    });
                } else if msg.role == "user" {
                    for block in blocks {
                        if block["type"].as_str() == Some("tool_result") {
                            let id = block["tool_use_id"].as_str().unwrap_or("").to_string();
                            let content = match block["content"].as_str() {
                                Some(s) => serde_json::Value::String(s.to_string()),
                                None => block["content"].clone(),
                            };
                            result.push(OpenAiMessage {
                                role: "tool".to_string(),
                                content,
                                tool_calls: None,
                                tool_call_id: Some(id),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    result
}

pub fn tool_definitions() -> Vec<super::anthropic::ToolDef> {
    vec![
        super::anthropic::ToolDef { name: "read_file".into(), description: "Read the contents of a file".into(), input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}) },
        super::anthropic::ToolDef { name: "write_to_file".into(), description: "Create or overwrite a file".into(), input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}) },
        super::anthropic::ToolDef { name: "edit_file".into(), description: "Replace a string in a file".into(), input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"},"old_string":{"type":"string"},"new_string":{"type":"string"}},"required":["path","old_string","new_string"]}) },
        super::anthropic::ToolDef { name: "list_directory".into(), description: "List directory contents".into(), input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"}}}) },
        super::anthropic::ToolDef { name: "search_files".into(), description: "Search files by glob pattern".into(), input_schema: serde_json::json!({"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"}},"required":["pattern"]}) },
        super::anthropic::ToolDef { name: "search_content".into(), description: "Search file contents (grep)".into(), input_schema: serde_json::json!({"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"}},"required":["pattern"]}) },
        super::anthropic::ToolDef { name: "run_shell".into(), description: "Execute a shell command".into(), input_schema: serde_json::json!({"type":"object","properties":{"command":{"type":"string"},"timeout":{"type":"number"}},"required":["command"]}) },
        super::anthropic::ToolDef { name: "web_search".into(), description: "Search the web".into(), input_schema: serde_json::json!({"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}) },
        super::anthropic::ToolDef { name: "web_fetch".into(), description: "Fetch a URL".into(), input_schema: serde_json::json!({"type":"object","properties":{"url":{"type":"string"}},"required":["url"]}) },
    ]
}
