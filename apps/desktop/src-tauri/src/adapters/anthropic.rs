use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use serde::Serialize;
use tauri::Emitter;
use tokio::sync::Notify;

use super::base::{AdapterError, AiAdapter, ChatMessage, StreamResult, ToolCall};
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;

const DEFAULT_MODEL: &str = "claude-sonnet-4-6";
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

const SYSTEM_PROMPT: &str = "\
You are a powerful AI coding agent running in a desktop GUI application. You have direct access to the user's filesystem and shell.

## Your capabilities
- Read, write, and edit files on the user's machine
- Execute shell commands (build, test, git, package management, etc.)
- Search code with glob patterns and regex
- Fetch information from the web
- Ask the user questions when you need clarification

## How to work
1. Understand the user's request thoroughly before acting
2. Use search tools to find relevant files and code
3. Read files before editing them
4. Make targeted edits with edit_file rather than rewriting entire files
5. Run build/test commands to verify your changes
6. Explain what you did and why

## Important rules
- Never assume file contents — always read first
- Use edit_file for small targeted changes, write_to_file for full rewrites
- Run shell commands with care — they have real effects on the user's system
- If you're unsure about something, use ask_user to clarify
- Keep responses concise and actionable
- Prefer standard library and well-known patterns over obscure solutions";

pub struct AnthropicAdapter {
    api_key: String,
    model: String,
    base_url: String,
    thinking_budget_tokens: u32,
    max_tokens: u32,
    client: reqwest::Client,
    /// Extra tools from MCP servers or other external sources
    external_tools: Vec<ToolDef>,
    /// Disable anthropic thinking extension (DeepSeek may not support it)
    disable_thinking: bool,
}

impl AnthropicAdapter {
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
            thinking_budget_tokens: 4000,
            max_tokens: 8192,
            client,
            external_tools: Vec::new(),
            disable_thinking: false,
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

    pub fn with_external_tools(mut self, tools: Vec<ToolDef>) -> Self {
        self.external_tools = tools;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_thinking_budget_tokens(mut self, budget: u32) -> Self {
        self.thinking_budget_tokens = budget;
        self
    }

    pub fn with_thinking_disabled(mut self) -> Self {
        self.disable_thinking = true;
        self
    }
}

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    stream: bool,
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDef>>,
}

#[derive(Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    type_: String,
    budget_tokens: u32,
}

#[derive(Serialize, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolDef {
    pub fn new(name: &str, description: &str, input_schema: serde_json::Value) -> Self {
        Self { name: name.to_string(), description: description.to_string(), input_schema }
    }
}

impl AnthropicAdapter {
    /// Filtered tools for sub-agents — excludes dangerous/recursive tools.
    /// The AI never sees these tools in its function list, so it can't call them.
    fn sub_agent_tools(&self) -> Vec<ToolDef> {
        const BLOCKED: &[&str] = &[
            "delegate_task",
            "write_to_file",
            "edit_file",
            "run_shell",
            "bash",
            "ask_user",
        ];
        self.tool_definitions()
            .into_iter()
            .filter(|t| !BLOCKED.contains(&t.name.as_str()))
            .collect()
    }

    fn tool_definitions(&self) -> Vec<ToolDef> {
    let mut tools = vec![
        ToolDef {
            name: "read_file".to_string(),
            description: "Read the contents of a file. Use this to inspect code, configs, logs, or any text file.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string","description":"Absolute or relative path to the file"}},"required":["path"]}),
        },
        ToolDef {
            name: "write_to_file".to_string(),
            description: "Create a new file or overwrite an existing file with new content. Use for creating or completely replacing files.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string","description":"Path to the file to write"},"content":{"type":"string","description":"Full file content"}},"required":["path","content"]}),
        },
        ToolDef {
            name: "edit_file".to_string(),
            description: "Replace a string in an existing file. Use for targeted edits without rewriting the entire file. The search string must exist in the file; the replacement replaces it.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string","description":"Path to the file to edit"},"old_string":{"type":"string","description":"Exact string to find and replace"},"new_string":{"type":"string","description":"Replacement string"}},"required":["path","old_string","new_string"]}),
        },
        ToolDef {
            name: "list_directory".to_string(),
            description: "List files and directories at the given path. Use to explore project structure.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string","description":"Directory path to list (defaults to working directory)"}},"required":[]}),
        },
        ToolDef {
            name: "search_files".to_string(),
            description: "Search for files matching a glob pattern. Returns relative file paths.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"pattern":{"type":"string","description":"Glob pattern like **/*.rs or src/**/*.ts"},"path":{"type":"string","description":"Directory to search in (defaults to working directory)"}},"required":["pattern"]}),
        },
        ToolDef {
            name: "search_content".to_string(),
            description: "Search file contents for a regex pattern. Like grep. Returns matching lines with file paths and line numbers.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"pattern":{"type":"string","description":"Regex pattern to search for in file contents"},"path":{"type":"string","description":"Directory or file to search in (defaults to working directory)"}},"required":["pattern"]}),
        },
        ToolDef {
            name: "run_shell".to_string(),
            description: "Execute a shell command. Use for build commands, git operations, tests, package management, and any CLI tool. Returns stdout, stderr, and exit code.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"command":{"type":"string","description":"Shell command to execute"},"timeout":{"type":"number","description":"Timeout in seconds (default 120)"}},"required":["command"]}),
        },
        ToolDef {
            name: "web_search".to_string(),
            description: "Search the web for current information. Returns titles, snippets, and URLs.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"query":{"type":"string","description":"Search query"}},"required":["query"]}),
        },
        ToolDef {
            name: "web_fetch".to_string(),
            description: "Fetch content from a URL and extract text. Use to read documentation, API references, or any web page.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"url":{"type":"string","description":"URL to fetch (http/https)"}},"required":["url"]}),
        },
        ToolDef {
            name: "ask_user".to_string(),
            description: "Ask the user a question when you need clarification, decisions, or more context. Use sparingly — only when truly needed.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"question":{"type":"string","description":"Question to ask the user"},"options":{"type":"array","items":{"type":"string"},"description":"Optional list of choices for the user"}},"required":["question"]}),
        },
        ToolDef {
            name: "git_diff".to_string(),
            description: "Show git diff of uncommitted changes. Without arguments shows unstaged changes. Use staged: true for staged changes, or path: 'file.rs' for a specific file.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"staged":{"type":"boolean","description":"Show staged changes (git diff --cached)"},"path":{"type":"string","description":"Show diff for a specific file only"}},"required":[]}),
        },
        ToolDef {
            name: "delegate_task".to_string(),
            description: "Dispatch an independent research subtask that runs in parallel with other subtasks. The sub-agent has read-only access (read_file, search_content, search_files, list_directory, web_search, web_fetch) and returns a text answer. Use this when you need to investigate multiple areas simultaneously — for example, searching for different patterns across the codebase, or reading multiple related files at once. Each delegate_task runs concurrently. Input: a clear, focused task description.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"task":{"type":"string","description":"Focused task description for the sub-agent. Be specific about what to find or investigate."}},"required":["task"]}),
        },
    ];
    tools.extend(self.external_tools.clone());
    tools
}
}

#[async_trait]
impl AiAdapter for AnthropicAdapter {
    fn model_id(&self) -> &str {
        &self.model
    }

    fn model_name(&self) -> &str {
        match self.model.as_str() {
            "deepseek-v4-pro[1m]" => "DeepSeek V4 Pro",
            "deepseek-v4-pro" => "DeepSeek V4 Pro",
            "deepseek-v4-flash[1m]" => "DeepSeek V4 Flash",
            "deepseek-v4-flash" => "DeepSeek V4 Flash",
            "claude-opus-4-7" => "Claude Opus 4.7",
            "claude-sonnet-4-6" => "Claude Sonnet 4.6",
            "claude-haiku-4-5-20251001" => "Claude Haiku 4.5",
            "glm-5-flash" => "GLM 5 Flash",
            _ => &self.model,
        }
    }

    /// Non-streaming API call — no frontend events. Used by sub-agents.
    async fn call(
        &self,
        messages: &[ChatMessage],
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        let mut system_parts: Vec<String> = vec![SYSTEM_PROMPT.to_string()];
        let filtered: Vec<ChatMessage> = messages
            .iter()
            .filter(|m| {
                if m.role == "system" {
                    if let serde_json::Value::String(ref s) = m.content {
                        if !s.is_empty() { system_parts.push(s.clone()); }
                    }
                    false
                } else { true }
            })
            .cloned()
            .collect();

        let body = AnthropicRequest {
            model: &self.model,
            max_tokens: self.max_tokens,
            stream: false,
            messages: &filtered,
            thinking: Some(ThinkingConfig { type_: "enabled".to_string(), budget_tokens: 2000 }),
            system: Some(system_parts.join("\n\n")),
            tools: Some(self.sub_agent_tools()),
        };

        // Race HTTP call against cancel token
        let url = format!("{}/v1/messages", self.base_url);
        let request = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body);

        let response = tokio::select! {
            r = request.send() => r,
            _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
        };
        let response = response.map_err(|e| AdapterError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = match tokio::select! {
                t = response.text() => t,
                _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
            } {
                Ok(t) => t,
                Err(_) => String::new(),
            };
            return Err(AdapterError::Http(format!("HTTP {status}: {text}")));
        }

        let parsed: serde_json::Value = match tokio::select! {
            j = response.json() => j,
            _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
        } {
            Ok(v) => v,
            Err(e) => return Err(AdapterError::Stream(e.to_string())),
        };

        let content = parsed["content"].as_array()
            .cloned()
            .unwrap_or_default();

        let tool_calls: Vec<ToolCall> = content.iter()
            .filter(|b| b["type"].as_str() == Some("tool_use"))
            .map(|b| ToolCall {
                id: b["id"].as_str().unwrap_or("").to_string(),
                name: b["name"].as_str().unwrap_or("").to_string(),
                input: b["input"].clone(),
            })
            .collect();

        let stop_reason = parsed["stop_reason"].as_str().map(|s| s.to_string());

        Ok(StreamResult {
            assistant_content: content,
            tool_calls,
            stop_reason,
        })
    }

    async fn stream_message(
        &self,
        session_id: &str,
        messages: &[ChatMessage],
        app_handle: &tauri::AppHandle,
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        // Extract system messages and build the combined system prompt
        let mut system_parts: Vec<String> = vec![SYSTEM_PROMPT.to_string()];
        let filtered: Vec<ChatMessage> = messages
            .iter()
            .filter(|m| {
                if m.role == "system" {
                    if let serde_json::Value::String(ref s) = m.content {
                        if !s.is_empty() {
                            system_parts.push(s.clone());
                        }
                    }
                    false // remove from messages list
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        let system = system_parts.join("\n\n");

        let body = AnthropicRequest {
            model: &self.model,
            max_tokens: self.max_tokens,
            stream: true,
            messages: &filtered,
            thinking: if self.disable_thinking {
                None
            } else {
                Some(ThinkingConfig {
                    type_: "enabled".to_string(),
                    budget_tokens: self.thinking_budget_tokens,
                })
            },
            system: Some(system),
            tools: Some(self.tool_definitions()),
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
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

        // SSE state
        let mut block_type: Option<String> = None;
        let mut active_block_id: Option<String> = None;

        // Tool call accumulation
        let mut current_tool_id: Option<String> = None;
        let mut current_tool_name: Option<String> = None;
        let mut current_tool_input_json = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut assistant_content: Vec<serde_json::Value> = Vec::new();
        let mut stop_reason: Option<String> = None;

        // Per-block text/thinking content accumulation (for assistant_content)
        let mut current_text = String::new();
        let mut current_thinking = String::new();
        let mut total_input_tokens: u32 = 0;

        loop {
            let chunk = tokio::select! {
                c = stream.next() => c,
                _ = cancel.notified() => {
                    crate::app_log!("INFO", "[anthropic] Stream cancelled for session {}", session_id);
                    return Err(AdapterError::Stream("Cancelled".to_string()));
                }
            };
            let chunk = match chunk {
                Some(c) => c,
                None => break,
            };
            let chunk = chunk.map_err(|e| AdapterError::Stream(e.to_string()))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text.replace("\r\n", "\n"));

            while let Some(event_end) = buffer.find("\n\n") {
                let event_data = buffer[..event_end].to_string();
                buffer = buffer[event_end + 2..].to_string();

                let mut data: Option<String> = None;
                for line in event_data.lines() {
                    if let Some(d) = line.strip_prefix("data: ") {
                        data = Some(d.trim().to_string());
                    }
                }

                let data = match data {
                    Some(d) if !d.is_empty() => d,
                    _ => continue,
                };

                match serde_json::from_str::<serde_json::Value>(&data) {
                    Ok(parsed) => {
                        let sse_type = parsed["type"].as_str().unwrap_or("");
                        let session = session_id.to_string();

                        match sse_type {
                            "message_start" => {
                                // Capture input tokens for cost tracking
                                if let Some(usage) = parsed["message"].get("usage") {
                                    total_input_tokens = usage.get("input_tokens")
                                        .and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                }
                                let _ = app_handle.emit(
                                    "session-output",
                                    StreamEvent::SessionStatus {
                                        session_id: session.clone(),
                                        status: "working".to_string(),
                                    },
                                );
                            }

                            "content_block_start" => {
                                let cb = &parsed["content_block"];
                                let cb_type = cb["type"].as_str().unwrap_or("");
                                let cb_id = BlockId::new();
                                let bid = cb_id.to_string();

                                current_text.clear();
                                current_thinking.clear();

                                match cb_type {
                                    "thinking" => {
                                        block_type = Some("thinking".to_string());
                                        active_block_id = Some(bid.clone());
                                        let _ = app_handle.emit(
                                            "session-output",
                                            StreamEvent::ThinkingStart {
                                                session_id: session.clone(),
                                                block_id: bid,
                                            },
                                        );
                                    }
                                    "text" => {
                                        block_type = Some("text".to_string());
                                        active_block_id = Some(bid.clone());
                                        let _ = app_handle.emit(
                                            "session-output",
                                            StreamEvent::TextStart {
                                                session_id: session.clone(),
                                                block_id: bid,
                                            },
                                        );
                                    }
                                    "tool_use" => {
                                        block_type = Some("tool_use".to_string());
                                        current_tool_id =
                                            Some(cb["id"].as_str().unwrap_or("").to_string());
                                        current_tool_name = Some(
                                            cb["name"].as_str().unwrap_or("unknown").to_string(),
                                        );
                                        let tool_block_id = current_tool_id
                                            .clone()
                                            .filter(|id| !id.is_empty())
                                            .unwrap_or(bid);
                                        active_block_id = Some(tool_block_id.clone());
                                        current_tool_input_json = String::new();

                                        let tool_input = cb["input"].clone();
                                        // If input is already fully populated (non-streaming or edge case),
                                        // start with that
                                        if tool_input.is_object()
                                            && !tool_input.as_object().unwrap().is_empty()
                                        {
                                            current_tool_input_json =
                                                serde_json::to_string(&tool_input).unwrap_or_default();
                                        }

                                        let _ = app_handle.emit(
                                            "session-output",
                                            StreamEvent::ToolCallStart {
                                                session_id: session.clone(),
                                                block_id: tool_block_id,
                                                tool_name: current_tool_name.clone().unwrap(),
                                                tool_input: cb["input"].clone(),
                                            },
                                        );
                                    }
                                    _ => {}
                                }
                            }

                            "content_block_delta" => {
                                let delta = &parsed["delta"];
                                let delta_type = delta["type"].as_str().unwrap_or("");
                                let bid = active_block_id.clone().unwrap_or_default();

                                match delta_type {
                                    "thinking_delta" => {
                                        let content =
                                            delta["thinking"].as_str().unwrap_or("");
                                        current_thinking.push_str(content);
                                        let _ = app_handle.emit(
                                            "session-output",
                                            StreamEvent::ThinkingChunk {
                                                session_id: session.clone(),
                                                block_id: bid,
                                                content: content.to_string(),
                                            },
                                        );
                                    }
                                    "text_delta" => {
                                        let content = delta["text"].as_str().unwrap_or("");
                                        current_text.push_str(content);
                                        let _ = app_handle.emit(
                                            "session-output",
                                            StreamEvent::TextChunk {
                                                session_id: session.clone(),
                                                block_id: bid,
                                                content: content.to_string(),
                                            },
                                        );
                                    }
                                    "input_json_delta" => {
                                        let partial =
                                            delta["partial_json"].as_str().unwrap_or("");
                                        current_tool_input_json.push_str(partial);
                                    }
                                    _ => {}
                                }
                            }

                            "content_block_stop" => {
                                let bid = active_block_id.clone().unwrap_or_default();

                                match block_type.as_deref() {
                                    Some("thinking") => {
                                        // Save thinking to conversation history
                                        if !current_thinking.is_empty() {
                                            assistant_content.push(serde_json::json!({"type":"thinking","thinking":current_thinking}));
                                        }
                                        let _ = app_handle.emit("session-output", StreamEvent::ThinkingEnd { session_id: session.clone(), block_id: bid });
                                    }
                                    Some("text") => {
                                        // Save text to conversation history
                                        if !current_text.is_empty() {
                                            assistant_content.push(serde_json::json!({"type":"text","text":current_text}));
                                        }
                                        let _ = app_handle.emit("session-output", StreamEvent::TextEnd { session_id: session.clone(), block_id: bid });
                                    }
                                    Some("tool_use") => {
                                        // Parse accumulated tool input JSON
                                        let input: serde_json::Value =
                                            serde_json::from_str(&current_tool_input_json)
                                                .unwrap_or(serde_json::Value::Object(
                                                    serde_json::Map::new(),
                                                ));

                                        let tool_id =
                                            current_tool_id.clone().unwrap_or_default();
                                        let tool_name =
                                            current_tool_name.clone().unwrap_or_default();

                                        tool_calls.push(ToolCall {
                                            id: tool_id.clone(),
                                            name: tool_name.clone(),
                                            input: input.clone(),
                                        });

                                        // Add tool_use block to assistant content
                                        assistant_content.push(serde_json::json!({
                                            "type": "tool_use",
                                            "id": tool_id,
                                            "name": tool_name,
                                            "input": input,
                                        }));

                                        let _ = app_handle.emit(
                                            "session-output",
                                            StreamEvent::ToolCallEnd {
                                                session_id: session.clone(),
                                                block_id: bid,
                                            },
                                        );

                                        current_tool_id = None;
                                        current_tool_name = None;
                                        current_tool_input_json.clear();
                                    }
                                    _ => {}
                                }
                                block_type = None;
                                active_block_id = None;
                            }

                            "message_delta" => {
                                stop_reason = parsed["delta"]["stop_reason"]
                                    .as_str()
                                    .map(|s| s.to_string());
                                // Capture usage data and emit cost event
                                if let Some(usage) = parsed["usage"].as_object() {
                                    let output = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                    let cost = estimate_cost(&self.model, total_input_tokens, output);
                                    let _ = app_handle.emit("session-output",
                                        StreamEvent::Usage {
                                            session_id: session.to_string(),
                                            input_tokens: total_input_tokens,
                                            output_tokens: output,
                                            estimated_cost_usd: cost,
                                        });
                                }
                            }

                            "message_stop" => {
                                let _ = app_handle.emit(
                                    "session-output",
                                    StreamEvent::SessionStatus {
                                        session_id: session.clone(),
                                        status: "idle".to_string(),
                                    },
                                );
                            }

                            "error" => {
                                let msg = parsed["error"]["message"]
                                    .as_str()
                                    .unwrap_or("Unknown error");
                                let _ = app_handle.emit(
                                    "session-output",
                                    StreamEvent::Error {
                                        session_id: session.clone(),
                                        block_id: BlockId::new().to_string(),
                                        message: msg.to_string(),
                                        code: "api_error".to_string(),
                                    },
                                );
                            }

                            _ => {}
                        }
                    }
                    Err(_) => {
                        log::warn!("Unparseable SSE data: {:.200}", data);
                    }
                }
            }
        }

        Ok(StreamResult {
            assistant_content,
            tool_calls,
            stop_reason,
        })
    }
}

/// Estimate cost based on model pricing (per 1M tokens).
pub fn estimate_cost(model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
    let (input_price, output_price): (f64, f64) = match model {
        m if m.contains("opus") => (15.0, 75.0),
        m if m.contains("sonnet") => (3.0, 15.0),
        m if m.contains("haiku") => (0.8, 4.0),
        m if m.contains("deepseek") => (0.14, 0.28),
        _ => (3.0, 15.0), // default sonnet pricing
    };
    (input_tokens as f64 / 1_000_000.0) * input_price
        + (output_tokens as f64 / 1_000_000.0) * output_price
}

/// Flush accumulated text or thinking content into the assistant content list.
fn flush_content(
    block_type: Option<&str>,
    text: &str,
    content: &mut Vec<serde_json::Value>,
) {
    match block_type {
        Some("text") if !text.is_empty() => {
            content.push(serde_json::json!({"type": "text", "text": text}));
        }
        Some("thinking") if !text.is_empty() => {
            content.push(serde_json::json!({"type": "thinking", "thinking": text}));
        }
        _ => {}
    }
}
