use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use futures::StreamExt;
use serde::Serialize;
use tokio::sync::Notify;

use super::base::{
    repair_tool_result_adjacency, AdapterError, AiAdapter, ChatMessage, StreamResult, ToolCall,
    ToolDef,
};
use crate::agent::event_sink::EventEmitter;
use crate::consts::{AGENT_API_TIMEOUT, HTTP_CONNECT_TIMEOUT};
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;

const DEFAULT_MODEL: &str = "deepseek-v4-flash";

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
    external_tools: RwLock<Vec<ToolDef>>,
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
            external_tools: RwLock::new(Vec::new()),
            client: reqwest::Client::builder()
                .connect_timeout(HTTP_CONNECT_TIMEOUT)
                .timeout(AGENT_API_TIMEOUT)
                .build()
                .map_err(|e| AdapterError::Http(e.to_string()))?,
        })
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }
    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_string();
        self
    }

    pub fn with_external_tools(self, tools: Vec<ToolDef>) -> Self {
        self.set_external_tools(tools);
        self
    }

    fn tool_definitions_for_request(&self) -> Vec<ToolDef> {
        let mut tools = tool_definitions();
        tools.extend(
            self.external_tools
                .read()
                .expect("external tools lock poisoned")
                .clone(),
        );
        tools
    }

    fn request_for_messages(&self, messages: &[ChatMessage], stream: bool) -> OpenAiRequest {
        let repaired_messages = repair_tool_result_adjacency(messages);
        let openai_msgs = convert_messages(&repaired_messages);
        let tools: Vec<OpenAiTool> = self
            .tool_definitions_for_request()
            .into_iter()
            .map(|td| OpenAiTool {
                type_: "function".to_string(),
                function: OpenAiFunctionDef {
                    name: td.name,
                    description: td.description,
                    parameters: td.input_schema,
                },
            })
            .collect();

        OpenAiRequest {
            model: self.model.clone(),
            messages: openai_msgs,
            stream,
            max_tokens: Some(self.max_tokens),
            tools: if tools.is_empty() { None } else { Some(tools) },
        }
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
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
    fn set_external_tools(&self, tools: Vec<ToolDef>) {
        *self
            .external_tools
            .write()
            .expect("external tools lock poisoned") = tools;
    }

    fn model_id(&self) -> &str {
        &self.model
    }
    fn model_name(&self) -> &str {
        match self.model.as_str() {
            "deepseek-chat" => "DeepSeek Chat",
            "deepseek-reasoner" => "DeepSeek Reasoner",
            "gpt-4o" => "GPT-4o",
            "gpt-4o-mini" => "GPT-4o Mini",
            _ => &self.model,
        }
    }

    async fn call(
        &self,
        messages: &[ChatMessage],
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        let request = self.request_for_messages(messages, false);

        let response = tokio::select! {
            response = self
                .client
                .post(format!("{}/chat/completions", self.base_url))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&request)
                .send() => response,
            _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
        }
        .map_err(|e| AdapterError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text: String = tokio::select! {
                text = response.text() => text,
                _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
            }
            .unwrap_or_default();
            return Err(AdapterError::Http(format!("HTTP {status}: {text}")));
        }

        let parsed: serde_json::Value = tokio::select! {
            json = response.json() => json,
            _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
        }
        .map_err(|e| AdapterError::Stream(e.to_string()))?;

        if parsed["error"].is_object() {
            let message = parsed["error"]["message"]
                .as_str()
                .unwrap_or("Unknown OpenAI-compatible API error");
            return Err(AdapterError::Api {
                code: "api_error".to_string(),
                message: message.to_string(),
            });
        }

        Ok(parse_openai_chat_completion(&parsed))
    }

    async fn compact_summary(
        &self,
        messages: &[ChatMessage],
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        let mut request = self.request_for_messages(messages, false);
        request.tools = None;

        let response = tokio::select! {
            response = self
                .client
                .post(format!("{}/chat/completions", self.base_url))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&request)
                .send() => response,
            _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
        }
        .map_err(|e| AdapterError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text: String = tokio::select! {
                text = response.text() => text,
                _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
            }
            .unwrap_or_default();
            return Err(AdapterError::Http(format!("HTTP {status}: {text}")));
        }

        let parsed: serde_json::Value = tokio::select! {
            json = response.json() => json,
            _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
        }
        .map_err(|e| AdapterError::Stream(e.to_string()))?;

        if parsed["error"].is_object() {
            let message = parsed["error"]["message"]
                .as_str()
                .unwrap_or("Unknown OpenAI-compatible API error");
            return Err(AdapterError::Api {
                code: "api_error".to_string(),
                message: message.to_string(),
            });
        }

        Ok(parse_openai_chat_completion(&parsed))
    }

    async fn stream_message_with_emitter(
        &self,
        session_id: &str,
        messages: &[ChatMessage],
        emitter: &dyn EventEmitter,
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        crate::app_log!(
            "INFO",
            "OpenAI adapter streaming — {} messages, model={}",
            messages.len(),
            self.model
        );
        let request = self.request_for_messages(messages, true);
        crate::app_log!(
            "INFO",
            "Converted to {} OpenAI messages",
            request.messages.len()
        );

        // Debug: log request body for troubleshooting tool message issues
        let body_json = serde_json::to_string_pretty(&request).unwrap_or_default();
        crate::app_log!(
            "INFO",
            "OpenAI request body ({} bytes): {}",
            body_json.len(),
            &body_json[..body_json.len().min(2000)]
        );

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .body(body_json)
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

        let mut active_text_block_id: Option<String> = None;
        let mut parser = OpenAiStreamParser::default();

        loop {
            let chunk = tokio::select! {
                c = stream.next() => c,
                _ = cancel.notified() => {
                    crate::app_log!("INFO", "[openai] Stream cancelled for session {}", session_id);
                    return Err(AdapterError::Stream("Cancelled".to_string()));
                }
            };
            let chunk = match chunk {
                Some(c) => c,
                None => break,
            };
            let chunk = chunk.map_err(|e| AdapterError::Stream(e.to_string()))?;
            let text = String::from_utf8_lossy(&chunk);
            for data in drain_openai_sse_data(&mut buffer, &text) {
                let parsed: serde_json::Value = match serde_json::from_str(&data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let session = session_id.to_string();
                let update = parser.apply_event(&parsed);

                if update.text_started {
                    active_text_block_id = Some(BlockId::new().to_string());
                    emitter.emit(StreamEvent::TextStart {
                        session_id: session.clone(),
                        block_id: active_text_block_id.clone().unwrap(),
                    });
                }
                for content in update.text_chunks {
                    emitter.emit(StreamEvent::TextChunk {
                        session_id: session.clone(),
                        block_id: active_text_block_id.clone().unwrap_or_default(),
                        content,
                    });
                }

                if let Some((input_toks, output_toks)) = update.usage {
                    let cost = crate::adapters::anthropic::estimate_cost(
                        &self.model,
                        input_toks,
                        output_toks,
                    );
                    emitter.emit(StreamEvent::Usage {
                        session_id: session.clone(),
                        input_tokens: input_toks,
                        output_tokens: output_toks,
                        estimated_cost_usd: cost,
                    });
                }

                if let Some(msg) = update.error {
                    emitter.emit(StreamEvent::Error {
                        session_id: session.clone(),
                        block_id: BlockId::new().to_string(),
                        message: msg,
                        code: "api_error".to_string(),
                    });
                }
            }
        }

        // Flush remaining text — reuse the active block_id for continuity
        let has_text = parser.has_text();
        let result = parser.finish();
        if has_text {
            let bid = active_text_block_id.unwrap_or_else(|| BlockId::new().to_string());
            emitter.emit(StreamEvent::TextEnd {
                session_id: session_id.to_string(),
                block_id: bid,
            });
        }

        for tool_call in &result.tool_calls {
            let bid = tool_call.id.clone();
            emitter.emit(StreamEvent::ToolCallStart {
                session_id: session_id.to_string(),
                block_id: bid.clone(),
                tool_name: tool_call.name.clone(),
                tool_input: tool_call.input.clone(),
            });
            emitter.emit(StreamEvent::ToolCallEnd {
                session_id: session_id.to_string(),
                block_id: bid,
            });
        }

        Ok(result)
    }
}

#[derive(Default)]
struct OpenAiStreamParser {
    current_text: String,
    current_reasoning: String,
    tool_call_buffers: Vec<OpenAiToolCallBuffer>,
    stop_reason: Option<String>,
}

#[derive(Default)]
struct OpenAiToolCallBuffer {
    id: String,
    name: String,
    arguments_json: String,
}

#[derive(Default)]
struct OpenAiStreamUpdate {
    text_started: bool,
    text_chunks: Vec<String>,
    usage: Option<(u32, u32)>,
    error: Option<String>,
}

impl OpenAiStreamParser {
    fn apply_event(&mut self, parsed: &serde_json::Value) -> OpenAiStreamUpdate {
        let mut update = OpenAiStreamUpdate::default();

        if let Some(choices) = parsed["choices"].as_array() {
            for choice in choices {
                let finish = choice["finish_reason"].as_str().unwrap_or("");
                if !finish.is_empty() {
                    self.stop_reason = Some(finish.to_string());
                }

                let delta = &choice["delta"];

                if let Some(rc) = delta["reasoning_content"].as_str() {
                    self.current_reasoning.push_str(rc);
                }

                if let Some(content) = delta["content"].as_str() {
                    if self.current_text.is_empty() {
                        update.text_started = true;
                    }
                    self.current_text.push_str(content);
                    update.text_chunks.push(content.to_string());
                }

                if let Some(tcs) = delta["tool_calls"].as_array() {
                    for tc in tcs {
                        let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                        while self.tool_call_buffers.len() <= idx {
                            self.tool_call_buffers.push(OpenAiToolCallBuffer::default());
                        }

                        if let Some(id) = tc["id"].as_str() {
                            self.tool_call_buffers[idx].id = id.to_string();
                        }
                        if let Some(func) = tc["function"].as_object() {
                            if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
                                if !name.is_empty() {
                                    self.tool_call_buffers[idx].name = name.to_string();
                                }
                            }
                            if let Some(args) = func.get("arguments").and_then(|v| v.as_str()) {
                                self.tool_call_buffers[idx].arguments_json.push_str(args);
                            }
                        }
                    }
                }
            }
        }

        if let Some(usage) = parsed["usage"].as_object() {
            let input_tokens = usage
                .get("prompt_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let output_tokens = usage
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            update.usage = Some((input_tokens, output_tokens));
        }

        if parsed["error"].is_object() {
            update.error = Some(
                parsed["error"]["message"]
                    .as_str()
                    .unwrap_or("Unknown error")
                    .to_string(),
            );
        }

        update
    }

    fn has_text(&self) -> bool {
        !self.current_text.is_empty()
    }

    fn finish(self) -> StreamResult {
        let mut assistant_content = Vec::new();
        let mut tool_calls = Vec::new();

        if !self.current_text.is_empty() {
            assistant_content.push(serde_json::json!({"type":"text","text":self.current_text}));
        }
        if !self.current_reasoning.is_empty() {
            assistant_content.push(
                serde_json::json!({"type":"reasoning","reasoning_content":self.current_reasoning}),
            );
        }

        for buffer in self.tool_call_buffers {
            if buffer.id.is_empty() || buffer.name.is_empty() {
                continue;
            }
            let id = buffer.id;
            let name = buffer.name;
            let input: serde_json::Value = serde_json::from_str(&buffer.arguments_json)
                .unwrap_or(serde_json::Value::Object(Default::default()));
            assistant_content.push(serde_json::json!({
                "type": "tool_use",
                "id": id.clone(),
                "name": name.clone(),
                "input": input.clone(),
            }));
            tool_calls.push(ToolCall { id, name, input });
        }

        StreamResult {
            assistant_content,
            tool_calls,
            stop_reason: self.stop_reason,
        }
    }
}

fn drain_openai_sse_data(buffer: &mut String, chunk: &str) -> Vec<String> {
    buffer.push_str(chunk);
    let mut events = Vec::new();

    while let Some(event_end) = buffer.find("\n\n") {
        let event_data = buffer[..event_end].to_string();
        buffer.replace_range(..event_end + 2, "");

        let data = event_data
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .collect::<Vec<_>>()
            .join("");

        if !data.is_empty() && data != "[DONE]" {
            events.push(data);
        }
    }

    events
}

fn parse_openai_chat_completion(parsed: &serde_json::Value) -> StreamResult {
    let mut assistant_content: Vec<serde_json::Value> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut stop_reason: Option<String> = None;

    let Some(choice) = parsed
        .get("choices")
        .and_then(|value| value.as_array())
        .and_then(|choices| choices.first())
    else {
        return StreamResult {
            assistant_content,
            tool_calls,
            stop_reason,
        };
    };

    stop_reason = choice
        .get("finish_reason")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let message = choice
        .get("message")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(content) = message.get("content").and_then(|value| value.as_str()) {
        if !content.is_empty() {
            assistant_content.push(serde_json::json!({"type": "text", "text": content}));
        }
    }

    if let Some(reasoning) = message
        .get("reasoning_content")
        .and_then(|value| value.as_str())
    {
        if !reasoning.is_empty() {
            assistant_content
                .push(serde_json::json!({"type": "reasoning", "reasoning_content": reasoning}));
        }
    }

    for tool_call in message
        .get("tool_calls")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
    {
        let id = tool_call
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let function = tool_call
            .get("function")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let name = function
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        if id.is_empty() || name.is_empty() {
            continue;
        }

        let input = match function.get("arguments") {
            Some(serde_json::Value::String(arguments)) => serde_json::from_str(arguments)
                .unwrap_or_else(|_| serde_json::Value::Object(Default::default())),
            Some(value) => value.clone(),
            None => serde_json::Value::Object(Default::default()),
        };
        assistant_content.push(serde_json::json!({
            "type": "tool_use",
            "id": id.clone(),
            "name": name.clone(),
            "input": input.clone(),
        }));
        tool_calls.push(ToolCall { id, name, input });
    }

    StreamResult {
        assistant_content,
        tool_calls,
        stop_reason,
    }
}

fn convert_messages(messages: &[ChatMessage]) -> Vec<OpenAiMessage> {
    let mut result = Vec::new();
    // Use the first ChatMessage as system prompt if it has role "system", otherwise use default
    let has_system = messages
        .first()
        .map(|m| m.role == "system")
        .unwrap_or(false);
    let roles: Vec<&str> = messages.iter().map(|m| m.role.as_str()).collect();
    crate::app_log!(
        "INFO",
        "[convert_messages] {} messages, roles: {:?}, has_system: {}",
        messages.len(),
        roles,
        has_system
    );
    if !has_system {
        crate::app_log!(
            "INFO",
            "[convert_messages] using default SYSTEM_PROMPT (no system message found)"
        );
        result.push(OpenAiMessage {
            role: "system".to_string(),
            content: serde_json::Value::String(SYSTEM_PROMPT.to_string()),
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        });
    } else {
        if let serde_json::Value::String(ref s) = messages[0].content {
            crate::app_log!(
                "INFO",
                "[convert_messages] using custom system prompt: {} chars, has 'Active Skills': {}",
                s.len(),
                s.contains("Active Skills")
            );
        }
    }

    for msg in messages {
        match &msg.content {
            serde_json::Value::String(s) => {
                result.push(OpenAiMessage {
                    role: msg.role.clone(),
                    content: serde_json::Value::String(s.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                });
            }
            serde_json::Value::Object(ref obj) if msg.role == "tool" => {
                // Direct pass-through for tool messages
                let tool_call_id = obj
                    .get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let content = obj
                    .get("content")
                    .cloned()
                    .unwrap_or(serde_json::Value::String("".into()));
                result.push(OpenAiMessage {
                    role: "tool".to_string(),
                    content,
                    tool_calls: None,
                    tool_call_id,
                    reasoning_content: None,
                });
            }
            serde_json::Value::Array(blocks) => {
                if msg.role == "assistant" {
                    let mut text_parts = Vec::new();
                    let mut tool_calls: Vec<OpenAiToolCallMsg> = Vec::new();
                    let mut reasoning = None;
                    for block in blocks {
                        match block["type"].as_str() {
                            Some("text") => {
                                if let Some(t) = block["text"].as_str() {
                                    text_parts.push(t.to_string());
                                }
                            }
                            Some("reasoning") => {
                                reasoning =
                                    block["reasoning_content"].as_str().map(|s| s.to_string());
                            }
                            Some("tool_use") => {
                                let id = block["id"].as_str().unwrap_or("").to_string();
                                let name = block["name"].as_str().unwrap_or("").to_string();
                                let args = block["input"].to_string();
                                tool_calls.push(OpenAiToolCallMsg {
                                    id,
                                    type_: "function".to_string(),
                                    function: OpenAiFunctionCall {
                                        name,
                                        arguments: args,
                                    },
                                });
                            }
                            _ => {}
                        }
                    }
                    let content = serde_json::Value::String(text_parts.join("\n"));
                    result.push(OpenAiMessage {
                        role: "assistant".to_string(),
                        content,
                        tool_calls: if tool_calls.is_empty() {
                            None
                        } else {
                            Some(tool_calls)
                        },
                        tool_call_id: None,
                        reasoning_content: reasoning,
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
                                reasoning_content: None,
                            });
                        }
                    }
                }
            }
            serde_json::Value::Object(ref obj) if msg.role == "tool" => {
                let tool_call_id = obj
                    .get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let content = obj
                    .get("content")
                    .cloned()
                    .unwrap_or(serde_json::Value::String("".into()));
                result.push(OpenAiMessage {
                    role: "tool".to_string(),
                    content,
                    tool_calls: None,
                    tool_call_id,
                    reasoning_content: None,
                });
            }
            _ => {}
        }
    }
    result
}

pub fn tool_definitions() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "read_file".into(),
            description: "Read the contents of a file".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        },
        ToolDef {
            name: "write_to_file".into(),
            description: "Create or overwrite a file".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}),
        },
        ToolDef {
            name: "edit_file".into(),
            description: "Replace a string in a file".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"},"old_string":{"type":"string"},"new_string":{"type":"string"}},"required":["path","old_string","new_string"]}),
        },
        ToolDef {
            name: "list_directory".into(),
            description: "List directory contents".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"}}}),
        },
        ToolDef {
            name: "search_files".into(),
            description: "Search files by glob pattern".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"}},"required":["pattern"]}),
        },
        ToolDef {
            name: "search_content".into(),
            description: "Search file contents (grep)".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"}},"required":["pattern"]}),
        },
        ToolDef {
            name: "run_shell".into(),
            description: "Execute a shell command".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"command":{"type":"string"},"timeout":{"type":"number"}},"required":["command"]}),
        },
        ToolDef {
            name: "web_search".into(),
            description: "Search the web".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}),
        },
        ToolDef {
            name: "web_fetch".into(),
            description: "Fetch a URL".into(),
            input_schema: serde_json::json!({"type":"object","properties":{"url":{"type":"string"}},"required":["url"]}),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn external_tools_are_included_in_openai_compatible_definitions() {
        let adapter = OpenAiCompatibleAdapter::new("test-key".to_string())
            .unwrap()
            .with_external_tools(vec![ToolDef::new(
                "mcp__fixture__echo",
                "Echo through MCP",
                serde_json::json!({"type": "object"}),
            )]);

        let names = adapter
            .tool_definitions_for_request()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"mcp__fixture__echo".to_string()));
    }

    #[test]
    fn external_tools_can_be_replaced_after_session_creation() {
        let adapter = OpenAiCompatibleAdapter::new("test-key".to_string())
            .unwrap()
            .with_external_tools(vec![ToolDef::new(
                "mcp__old__tool",
                "Old MCP tool",
                serde_json::json!({"type": "object"}),
            )]);

        adapter.set_external_tools(vec![ToolDef::new(
            "mcp__new__tool",
            "New MCP tool",
            serde_json::json!({"type": "object"}),
        )]);

        let names = adapter
            .tool_definitions_for_request()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"mcp__new__tool".to_string()));
        assert!(!names.contains(&"mcp__old__tool".to_string()));
    }

    #[test]
    fn parses_non_streaming_text_and_tool_calls() {
        let parsed = serde_json::json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "content": "先检查文件。",
                    "reasoning_content": "Need file context",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\":\"src/App.tsx\"}"
                        }
                    }]
                }
            }]
        });

        let result = parse_openai_chat_completion(&parsed);

        assert_eq!(result.stop_reason.as_deref(), Some("tool_calls"));
        assert_eq!(
            result.assistant_content,
            vec![
                serde_json::json!({"type": "text", "text": "先检查文件。"}),
                serde_json::json!({"type": "reasoning", "reasoning_content": "Need file context"}),
                serde_json::json!({
                    "type": "tool_use",
                    "id": "call_1",
                    "name": "read_file",
                    "input": { "path": "src/App.tsx" }
                })
            ]
        );
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].id, "call_1");
        assert_eq!(result.tool_calls[0].name, "read_file");
        assert_eq!(
            result.tool_calls[0].input,
            serde_json::json!({ "path": "src/App.tsx" })
        );
    }

    #[test]
    fn parses_non_streaming_invalid_tool_arguments_as_empty_object() {
        let parsed = serde_json::json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{not-json"
                        }
                    }]
                }
            }]
        });

        let result = parse_openai_chat_completion(&parsed);

        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].input, serde_json::json!({}));
    }

    #[test]
    fn parses_non_streaming_empty_string_arguments_as_empty_object() {
        let parsed = serde_json::json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": ""
                        }
                    }]
                }
            }]
        });

        let result = parse_openai_chat_completion(&parsed);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].input, serde_json::json!({}));
    }

    #[test]
    fn parses_non_streaming_null_arguments_as_empty_object() {
        let parsed = serde_json::json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": null
                        }
                    }]
                }
            }]
        });

        let result = parse_openai_chat_completion(&parsed);
        assert_eq!(result.tool_calls.len(), 1);
        // null arguments: parser keeps the null value as-is
        assert_eq!(result.tool_calls[0].input, serde_json::json!(null));
    }

    #[test]
    fn parses_non_streaming_array_arguments_fallback() {
        // Some providers return arguments as a JSON array instead of object
        let parsed = serde_json::json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "[1, 2, 3]"
                        }
                    }]
                }
            }]
        });

        let result = parse_openai_chat_completion(&parsed);
        assert_eq!(result.tool_calls.len(), 1);
        // Array is valid JSON but not an object — should still parse
        assert_eq!(result.tool_calls[0].input, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn parses_non_streaming_missing_tool_id() {
        let parsed = serde_json::json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "tool_calls": [{
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\":\"src/main.rs\"}"
                        }
                    }]
                }
            }]
        });

        let result = parse_openai_chat_completion(&parsed);
        // Missing id causes parser to skip the tool call entirely
        assert_eq!(result.tool_calls.len(), 0);
    }

    #[test]
    fn parses_non_streaming_missing_tool_name() {
        let parsed = serde_json::json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "arguments": "{\"path\":\"src/main.rs\"}"
                        }
                    }]
                }
            }]
        });

        let result = parse_openai_chat_completion(&parsed);
        // Missing name causes parser to skip the tool call entirely
        assert_eq!(result.tool_calls.len(), 0);
    }

    #[test]
    fn parses_non_streaming_multiple_tool_calls_order_preserved() {
        let parsed = serde_json::json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "tool_calls": [
                        {
                            "id": "call_a",
                            "type": "function",
                            "function": { "name": "read_file", "arguments": "{\"path\":\"a.rs\"}" }
                        },
                        {
                            "id": "call_b",
                            "type": "function",
                            "function": { "name": "run_shell", "arguments": "{\"command\":\"ls\"}" }
                        },
                        {
                            "id": "call_c",
                            "type": "function",
                            "function": { "name": "write_to_file", "arguments": "{\"path\":\"c.rs\",\"content\":\"x\"}" }
                        }
                    ]
                }
            }]
        });

        let result = parse_openai_chat_completion(&parsed);
        assert_eq!(result.tool_calls.len(), 3);
        assert_eq!(result.tool_calls[0].id, "call_a");
        assert_eq!(result.tool_calls[0].name, "read_file");
        assert_eq!(result.tool_calls[1].id, "call_b");
        assert_eq!(result.tool_calls[1].name, "run_shell");
        assert_eq!(result.tool_calls[2].id, "call_c");
        assert_eq!(result.tool_calls[2].name, "write_to_file");
    }

    #[test]
    fn parses_non_streaming_no_content_no_tool_calls() {
        let parsed = serde_json::json!({
            "choices": [{
                "finish_reason": "stop",
                "message": {}
            }]
        });

        let result = parse_openai_chat_completion(&parsed);
        assert!(result.tool_calls.is_empty());
        assert!(result.assistant_content.is_empty());
    }

    #[test]
    fn parses_streaming_tool_call_split_across_sse_and_network_chunks() {
        let first_delta = serde_json::json!({
            "choices": [{
                "delta": {
                    "content": "先看文件。",
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\""
                        }
                    }]
                }
            }]
        });
        let second_delta = serde_json::json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": {
                            "arguments": ":\"src/App.tsx\"}"
                        }
                    }]
                }
            }]
        });

        let first_event = format!("data: {first_delta}\n\n");
        let second_event = format!("data: {second_delta}\n\ndata: [DONE]\n\n");
        let result = parse_openai_stream_chunks(&[
            &first_event[..first_event.len() - 1],
            &first_event[first_event.len() - 1..],
            &second_event,
        ]);

        assert_eq!(
            result.assistant_content,
            vec![
                serde_json::json!({"type": "text", "text": "先看文件。"}),
                serde_json::json!({
                    "type": "tool_use",
                    "id": "call_1",
                    "name": "read_file",
                    "input": { "path": "src/App.tsx" }
                })
            ]
        );
        assert_eq!(result.stop_reason.as_deref(), Some("tool_calls"));
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].id, "call_1");
        assert_eq!(result.tool_calls[0].name, "read_file");
        assert_eq!(
            result.tool_calls[0].input,
            serde_json::json!({ "path": "src/App.tsx" })
        );
    }

    #[tokio::test]
    async fn call_repairs_tool_history_before_serializing_request() {
        let (base_url, received_body) = spawn_json_capture_server(serde_json::json!({
            "choices": [{
                "finish_reason": "stop",
                "message": { "content": "ok" }
            }]
        }));
        let adapter = OpenAiCompatibleAdapter::new("test-key".to_string())
            .unwrap()
            .with_base_url(&base_url);
        let messages = vec![
            ChatMessage::assistant(serde_json::json!([{
                "type": "tool_use",
                "id": "call_1",
                "name": "read_file",
                "input": { "path": "src/App.tsx" }
            }])),
            ChatMessage::user("继续处理"),
        ];

        let result = adapter
            .call(&messages, Arc::new(Notify::new()))
            .await
            .expect("adapter call");
        let request_body = received_body
            .recv_timeout(Duration::from_secs(2))
            .expect("captured request body");
        let request_messages = request_body["messages"]
            .as_array()
            .expect("request messages");

        assert_eq!(
            result.assistant_content,
            vec![serde_json::json!({"type": "text", "text": "ok"})]
        );
        assert_openai_tool_result_contract(request_messages);
        assert!(request_messages
            .iter()
            .any(|message| { message["role"] == "tool" && message["tool_call_id"] == "call_1" }));
        assert!(request_messages
            .iter()
            .any(|message| { message["role"] == "user" && message["content"] == "继续处理" }));
    }

    #[test]
    fn stream_request_repairs_tool_history_before_serializing_request() {
        let adapter = OpenAiCompatibleAdapter::new("test-key".to_string()).unwrap();
        let messages = vec![
            ChatMessage::assistant(serde_json::json!([{
                "type": "tool_use",
                "id": "call_1",
                "name": "read_file",
                "input": { "path": "src/App.tsx" }
            }])),
            ChatMessage::user("继续处理"),
        ];

        let request = adapter.request_for_messages(&messages, true);
        let request_body = serde_json::to_value(&request).expect("serialize request");
        let request_messages = request_body["messages"]
            .as_array()
            .expect("request messages");

        assert_eq!(request_body["stream"], true);
        assert_openai_tool_result_contract(request_messages);
    }

    fn assert_openai_tool_result_contract(messages: &[serde_json::Value]) {
        for (index, message) in messages.iter().enumerate() {
            let Some(tool_calls) = message.get("tool_calls").and_then(|value| value.as_array())
            else {
                continue;
            };
            let ids = tool_calls
                .iter()
                .filter_map(|tool_call| tool_call.get("id").and_then(|value| value.as_str()))
                .collect::<Vec<_>>();
            for (offset, id) in ids.iter().enumerate() {
                let next = messages.get(index + 1 + offset).unwrap_or_else(|| {
                    panic!("assistant tool_call {id} is missing an immediate tool message")
                });
                assert_eq!(next["role"], "tool");
                assert_eq!(next["tool_call_id"], *id);
            }
        }
    }

    fn parse_openai_stream_chunks(chunks: &[&str]) -> StreamResult {
        let mut buffer = String::new();
        let mut parser = OpenAiStreamParser::default();

        for chunk in chunks {
            for data in drain_openai_sse_data(&mut buffer, chunk) {
                let parsed: serde_json::Value =
                    serde_json::from_str(&data).expect("stream data json");
                parser.apply_event(&parsed);
            }
        }

        parser.finish()
    }

    fn spawn_json_capture_server(
        response_body: serde_json::Value,
    ) -> (String, mpsc::Receiver<serde_json::Value>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let request_body = read_http_json_body(&mut stream);
            tx.send(request_body).expect("send request body");

            let response = response_body.to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                response.len(),
                response
            )
            .expect("write response");
        });

        (base_url, rx)
    }

    fn read_http_json_body(stream: &mut std::net::TcpStream) -> serde_json::Value {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 4096];
        let header_end = loop {
            let read = stream.read(&mut chunk).expect("read request");
            assert!(read > 0, "connection closed before headers");
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(index) = find_subslice(&buffer, b"\r\n\r\n") {
                break index + 4;
            }
        };
        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.split_once(':').and_then(|(name, value)| {
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
            })
            .expect("content-length header");
        while buffer.len() < header_end + content_length {
            let read = stream.read(&mut chunk).expect("read request body");
            assert!(read > 0, "connection closed before body");
            buffer.extend_from_slice(&chunk[..read]);
        }
        serde_json::from_slice(&buffer[header_end..header_end + content_length])
            .expect("request body json")
    }

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }
}
