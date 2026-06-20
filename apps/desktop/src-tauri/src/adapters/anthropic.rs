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
use crate::protocol::events::{ProviderUsageReason, StreamEvent};
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
    external_tools: RwLock<Vec<ToolDef>>,
    /// Disable anthropic thinking extension (DeepSeek may not support it)
    disable_thinking: bool,
}

impl AnthropicAdapter {
    pub fn new(api_key: String) -> Result<Self, AdapterError> {
        if api_key.trim().is_empty() {
            return Err(AdapterError::MissingApiKey);
        }
        let client = reqwest::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(AGENT_API_TIMEOUT)
            .build()
            .map_err(|e| AdapterError::Http(e.to_string()))?;
        Ok(Self {
            api_key,
            model: DEFAULT_MODEL.to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
            thinking_budget_tokens: 4000,
            max_tokens: 8192,
            client,
            external_tools: RwLock::new(Vec::new()),
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

    pub fn with_external_tools(self, tools: Vec<ToolDef>) -> Self {
        self.set_external_tools(tools);
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

    fn request_for_messages(
        &self,
        messages: &[ChatMessage],
        stream: bool,
        use_sub_agent_tools: bool,
    ) -> AnthropicRequest {
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
                    false
                } else {
                    true
                }
            })
            .cloned()
            .collect();
        let filtered = repair_tool_result_adjacency(&filtered);
        let thinking = if use_sub_agent_tools {
            Some(ThinkingConfig {
                type_: "enabled".to_string(),
                budget_tokens: 2000,
            })
        } else if self.disable_thinking {
            None
        } else {
            Some(ThinkingConfig {
                type_: "enabled".to_string(),
                budget_tokens: self.thinking_budget_tokens,
            })
        };
        let tools = if use_sub_agent_tools {
            self.sub_agent_tools()
        } else {
            self.tool_definitions()
        };

        AnthropicRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            stream,
            messages: filtered,
            thinking,
            system: Some(system_parts.join("\n\n")),
            tools: Some(tools),
        }
    }
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    stream: bool,
    messages: Vec<ChatMessage>,
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
            description: "Dispatch an independent subtask that runs in parallel with other subtasks. Use 'research' mode for read-only investigation, 'patch_proposal' mode for a structured improvement proposal without writing files, and 'worktree_worker' mode for isolated implementation in a temporary git worktree. Each delegate_task runs concurrently.".to_string(),
            input_schema: serde_json::json!({"type":"object","properties":{"task":{"type":"string","description":"Focused task description for the sub-agent. Be specific about what to analyze, investigate, or implement."},"mode":{"type":"string","enum":["research","patch_proposal","worktree_worker"],"description":"Execution mode. 'research' (default) — read-only investigation returning text findings. 'patch_proposal' — code analysis that produces a structured patch proposal artifact without modifying files. 'worktree_worker' — isolated implementation in a temporary git worktree, returning diff/test artifacts without merging."},"root_planning_task":{"type":"boolean","description":"Set true only when this delegate should start a new root planning task instead of attaching to the active parent task context."}},"required":["task"]}),
        },
    ];
        tools.extend(
            self.external_tools
                .read()
                .expect("external tools lock poisoned")
                .clone(),
        );
        tools
    }

    async fn call_non_streaming(
        &self,
        messages: &[ChatMessage],
        cancel: Arc<Notify>,
        usage_emitter: Option<(&str, &dyn EventEmitter)>,
    ) -> Result<StreamResult, AdapterError> {
        let body = self.request_for_messages(messages, false, true);

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
            let text: String = tokio::select! {
                t = response.text() => t,
                _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
            }
            .unwrap_or_default();
            return Err(AdapterError::Http(format!("HTTP {status}: {text}")));
        }

        let parsed: serde_json::Value = match tokio::select! {
            j = response.json() => j,
            _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
        } {
            Ok(v) => v,
            Err(e) => return Err(AdapterError::Stream(e.to_string())),
        };

        if let Some((session_id, emitter)) = usage_emitter {
            emit_usage_events(
                emitter,
                session_id,
                "anthropic",
                &self.model,
                anthropic_usage_from_response(&parsed),
            );
        }

        let content = parsed["content"].as_array().cloned().unwrap_or_default();

        let tool_calls: Vec<ToolCall> = content
            .iter()
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
}

#[async_trait]
impl AiAdapter for AnthropicAdapter {
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
            "deepseek-v4-pro[1m]" => "DeepSeek V4 Pro 1M",
            "deepseek-v4-pro" => "DeepSeek V4 Pro",
            "deepseek-v4-flash[1m]" => "DeepSeek V4 Flash 1M",
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
        self.call_non_streaming(messages, cancel, None).await
    }

    async fn call_with_emitter(
        &self,
        session_id: &str,
        messages: &[ChatMessage],
        emitter: &dyn EventEmitter,
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        self.call_non_streaming(messages, cancel, Some((session_id, emitter)))
            .await
    }

    async fn compact_summary(
        &self,
        messages: &[ChatMessage],
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        let mut body = self.request_for_messages(messages, false, false);
        body.tools = None;
        body.thinking = None;

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
            let text: String = tokio::select! {
                t = response.text() => t,
                _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
            }
            .unwrap_or_default();
            return Err(AdapterError::Http(format!("HTTP {status}: {text}")));
        }

        let parsed: serde_json::Value = match tokio::select! {
            j = response.json() => j,
            _ = cancel.notified() => return Err(AdapterError::Stream("Cancelled".to_string())),
        } {
            Ok(v) => v,
            Err(e) => return Err(AdapterError::Stream(e.to_string())),
        };

        let content = parsed["content"].as_array().cloned().unwrap_or_default();
        let stop_reason = parsed["stop_reason"].as_str().map(|s| s.to_string());

        Ok(StreamResult {
            assistant_content: content,
            tool_calls: Vec::new(),
            stop_reason,
        })
    }

    async fn stream_message_with_emitter(
        &self,
        session_id: &str,
        messages: &[ChatMessage],
        emitter: &dyn EventEmitter,
        cancel: Arc<Notify>,
    ) -> Result<StreamResult, AdapterError> {
        let body = self.request_for_messages(messages, true, false);

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
        let mut active_block_id: Option<String> = None;
        let mut parser = AnthropicStreamParser::default();
        let mut saw_usage = false;

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

            for data in drain_anthropic_sse_data(&mut buffer, &text) {
                let session = session_id.to_string();
                match serde_json::from_str::<serde_json::Value>(&data) {
                    Ok(parsed) => {
                        let update = parser.apply_event(&parsed);

                        if let Some(status) = update.session_status {
                            emitter.emit(StreamEvent::SessionStatus {
                                session_id: session.clone(),
                                status,
                            });
                        }

                        if let Some(block_start) = update.block_start {
                            let bid = BlockId::new().to_string();
                            match block_start {
                                AnthropicBlockStart::Thinking => {
                                    active_block_id = Some(bid.clone());
                                    emitter.emit(StreamEvent::ThinkingStart {
                                        session_id: session.clone(),
                                        block_id: bid,
                                    });
                                }
                                AnthropicBlockStart::Text => {
                                    active_block_id = Some(bid.clone());
                                    emitter.emit(StreamEvent::TextStart {
                                        session_id: session.clone(),
                                        block_id: bid,
                                    });
                                }
                                AnthropicBlockStart::ToolUse { id, name, input } => {
                                    let tool_block_id =
                                        if id.is_empty() { bid } else { id.clone() };
                                    active_block_id = Some(tool_block_id.clone());
                                    emitter.emit(StreamEvent::ToolCallStart {
                                        session_id: session.clone(),
                                        block_id: tool_block_id,
                                        tool_name: name,
                                        tool_input: input,
                                    });
                                }
                            }
                        }

                        if let Some(content) = update.thinking_chunk {
                            emitter.emit(StreamEvent::ThinkingChunk {
                                session_id: session.clone(),
                                block_id: active_block_id.clone().unwrap_or_default(),
                                content,
                            });
                        }

                        if let Some(content) = update.text_chunk {
                            emitter.emit(StreamEvent::TextChunk {
                                session_id: session.clone(),
                                block_id: active_block_id.clone().unwrap_or_default(),
                                content,
                            });
                        }

                        if let Some(block_end) = update.block_end {
                            let bid = active_block_id.clone().unwrap_or_default();
                            match block_end {
                                AnthropicBlockEnd::Thinking => {
                                    emitter.emit(StreamEvent::ThinkingEnd {
                                        session_id: session.clone(),
                                        block_id: bid,
                                    });
                                }
                                AnthropicBlockEnd::Text => {
                                    emitter.emit(StreamEvent::TextEnd {
                                        session_id: session.clone(),
                                        block_id: bid,
                                    });
                                }
                                AnthropicBlockEnd::ToolUse => {
                                    emitter.emit(StreamEvent::ToolCallEnd {
                                        session_id: session.clone(),
                                        block_id: bid,
                                    });
                                }
                            }
                            active_block_id = None;
                        }

                        if let Some((input_tokens, output_tokens)) = update.usage {
                            saw_usage = true;
                            emit_usage_events(
                                emitter,
                                &session,
                                "anthropic",
                                &self.model,
                                Some((input_tokens, output_tokens)),
                            );
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
                    Err(_) => {
                        log::warn!("Unparseable SSE data: {:.200}", data);
                    }
                }
            }
        }

        if !saw_usage {
            emit_usage_events(emitter, session_id, "anthropic", &self.model, None);
        }

        Ok(parser.finish())
    }
}

#[derive(Default)]
struct AnthropicStreamParser {
    block_type: Option<String>,
    current_tool_id: Option<String>,
    current_tool_name: Option<String>,
    current_tool_input_json: String,
    tool_calls: Vec<ToolCall>,
    assistant_content: Vec<serde_json::Value>,
    stop_reason: Option<String>,
    current_text: String,
    current_thinking: String,
    total_input_tokens: u32,
}

#[derive(Default)]
struct AnthropicStreamUpdate {
    session_status: Option<String>,
    block_start: Option<AnthropicBlockStart>,
    thinking_chunk: Option<String>,
    text_chunk: Option<String>,
    block_end: Option<AnthropicBlockEnd>,
    usage: Option<(u32, u32)>,
    error: Option<String>,
}

enum AnthropicBlockStart {
    Thinking,
    Text,
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

enum AnthropicBlockEnd {
    Thinking,
    Text,
    ToolUse,
}

impl AnthropicStreamParser {
    fn apply_event(&mut self, parsed: &serde_json::Value) -> AnthropicStreamUpdate {
        let mut update = AnthropicStreamUpdate::default();
        let sse_type = parsed["type"].as_str().unwrap_or("");

        match sse_type {
            "message_start" => {
                if let Some(usage) = parsed["message"].get("usage") {
                    self.total_input_tokens = usage
                        .get("input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                }
                update.session_status = Some("working".to_string());
            }
            "content_block_start" => {
                let cb = &parsed["content_block"];
                let cb_type = cb["type"].as_str().unwrap_or("");
                self.current_text.clear();
                self.current_thinking.clear();

                match cb_type {
                    "thinking" => {
                        self.block_type = Some("thinking".to_string());
                        update.block_start = Some(AnthropicBlockStart::Thinking);
                    }
                    "text" => {
                        self.block_type = Some("text".to_string());
                        update.block_start = Some(AnthropicBlockStart::Text);
                    }
                    "tool_use" => {
                        self.block_type = Some("tool_use".to_string());
                        self.current_tool_id = Some(cb["id"].as_str().unwrap_or("").to_string());
                        self.current_tool_name =
                            Some(cb["name"].as_str().unwrap_or("unknown").to_string());
                        self.current_tool_input_json.clear();

                        let tool_input = cb["input"].clone();
                        if tool_input.is_object() && !tool_input.as_object().unwrap().is_empty() {
                            self.current_tool_input_json =
                                serde_json::to_string(&tool_input).unwrap_or_default();
                        }

                        update.block_start = Some(AnthropicBlockStart::ToolUse {
                            id: self.current_tool_id.clone().unwrap_or_default(),
                            name: self.current_tool_name.clone().unwrap_or_default(),
                            input: cb["input"].clone(),
                        });
                    }
                    _ => {}
                }
            }
            "content_block_delta" => {
                let delta = &parsed["delta"];
                let delta_type = delta["type"].as_str().unwrap_or("");

                match delta_type {
                    "thinking_delta" => {
                        let content = delta["thinking"].as_str().unwrap_or("");
                        self.current_thinking.push_str(content);
                        update.thinking_chunk = Some(content.to_string());
                    }
                    "text_delta" => {
                        let content = delta["text"].as_str().unwrap_or("");
                        self.current_text.push_str(content);
                        update.text_chunk = Some(content.to_string());
                    }
                    "input_json_delta" => {
                        let partial = delta["partial_json"].as_str().unwrap_or("");
                        self.current_tool_input_json.push_str(partial);
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                match self.block_type.as_deref() {
                    Some("thinking") => {
                        flush_content(
                            Some("thinking"),
                            &self.current_thinking,
                            &mut self.assistant_content,
                        );
                        update.block_end = Some(AnthropicBlockEnd::Thinking);
                    }
                    Some("text") => {
                        flush_content(
                            Some("text"),
                            &self.current_text,
                            &mut self.assistant_content,
                        );
                        update.block_end = Some(AnthropicBlockEnd::Text);
                    }
                    Some("tool_use") => {
                        let input: serde_json::Value =
                            serde_json::from_str(&self.current_tool_input_json)
                                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                        let tool_id = self.current_tool_id.clone().unwrap_or_default();
                        let tool_name = self.current_tool_name.clone().unwrap_or_default();

                        self.tool_calls.push(ToolCall {
                            id: tool_id.clone(),
                            name: tool_name.clone(),
                            input: input.clone(),
                        });
                        self.assistant_content.push(serde_json::json!({
                            "type": "tool_use",
                            "id": tool_id,
                            "name": tool_name,
                            "input": input,
                        }));

                        self.current_tool_id = None;
                        self.current_tool_name = None;
                        self.current_tool_input_json.clear();
                        update.block_end = Some(AnthropicBlockEnd::ToolUse);
                    }
                    _ => {}
                }
                self.block_type = None;
            }
            "message_delta" => {
                self.stop_reason = parsed["delta"]["stop_reason"].as_str().map(str::to_string);
                if let Some(usage) = parsed["usage"].as_object() {
                    let output_tokens = usage
                        .get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    update.usage = Some((self.total_input_tokens, output_tokens));
                }
            }
            "message_stop" => {
                update.session_status = Some("idle".to_string());
            }
            "error" => {
                update.error = Some(
                    parsed["error"]["message"]
                        .as_str()
                        .unwrap_or("Unknown error")
                        .to_string(),
                );
            }
            _ => {}
        }

        update
    }

    fn finish(self) -> StreamResult {
        StreamResult {
            assistant_content: self.assistant_content,
            tool_calls: self.tool_calls,
            stop_reason: self.stop_reason,
        }
    }
}

fn drain_anthropic_sse_data(buffer: &mut String, chunk: &str) -> Vec<String> {
    buffer.push_str(&chunk.replace("\r\n", "\n"));
    let mut events = Vec::new();

    while let Some(event_end) = buffer.find("\n\n") {
        let event_data = buffer[..event_end].to_string();
        buffer.replace_range(..event_end + 2, "");

        let data = event_data
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .next_back()
            .map(str::trim)
            .filter(|data| !data.is_empty())
            .map(str::to_string);

        if let Some(data) = data {
            events.push(data);
        }
    }

    events
}

pub(crate) fn emit_usage_events(
    emitter: &dyn EventEmitter,
    session_id: &str,
    source: &str,
    model: &str,
    usage: Option<(u32, u32)>,
) {
    let (input_tokens, output_tokens, estimated_cost_micros, reason) = match usage {
        Some((input_tokens, output_tokens)) => {
            match estimate_cost_micros(model, input_tokens, output_tokens) {
                Some(estimated_cost_micros) => (
                    Some(input_tokens.into()),
                    Some(output_tokens.into()),
                    Some(estimated_cost_micros),
                    ProviderUsageReason::ProviderReported,
                ),
                None => (
                    Some(input_tokens.into()),
                    Some(output_tokens.into()),
                    None,
                    ProviderUsageReason::PricingUnknown,
                ),
            }
        }
        None => (None, None, None, ProviderUsageReason::ProviderOmitted),
    };

    emitter.emit(StreamEvent::ProviderUsage {
        session_id: session_id.to_string(),
        block_id: uuid::Uuid::now_v7().to_string(),
        model: Some(model.to_string()),
        input_tokens,
        output_tokens,
        estimated_cost_micros,
        source: Some(source.to_string()),
        reason,
    });

    if let (Some(input_tokens), Some(output_tokens), Some(estimated_cost_micros)) =
        (input_tokens, output_tokens, estimated_cost_micros)
    {
        let Ok(input_tokens) = input_tokens.try_into() else {
            return;
        };
        let Ok(output_tokens) = output_tokens.try_into() else {
            return;
        };
        emitter.emit(StreamEvent::Usage {
            session_id: session_id.to_string(),
            input_tokens,
            output_tokens,
            estimated_cost_usd: estimated_cost_micros as f64 / 1_000_000.0,
        });
    }
}

/// Estimate cost based on known model pricing (per 1M tokens).
pub fn estimate_cost(model: &str, input_tokens: u32, output_tokens: u32) -> Option<f64> {
    estimate_cost_micros(model, input_tokens, output_tokens)
        .map(|micros| micros as f64 / 1_000_000.0)
}

pub fn estimate_cost_micros(model: &str, input_tokens: u32, output_tokens: u32) -> Option<u64> {
    let (input_price, output_price): (f64, f64) = pricing_for_model(model)?;
    let cost = (input_tokens as f64 * input_price) + (output_tokens as f64 * output_price);
    Some(cost.round() as u64)
}

fn pricing_for_model(model: &str) -> Option<(f64, f64)> {
    let model = model.to_ascii_lowercase();
    match model.as_str() {
        m if m.contains("opus") => Some((15.0, 75.0)),
        m if m.contains("sonnet") => Some((3.0, 15.0)),
        m if m.contains("haiku") => Some((0.8, 4.0)),
        m if m.contains("deepseek") => Some((0.14, 0.28)),
        _ => None,
    }
}

fn anthropic_usage_from_response(parsed: &serde_json::Value) -> Option<(u32, u32)> {
    let usage = parsed["usage"].as_object()?;
    let input_tokens = usage.get("input_tokens")?.as_u64()?.try_into().ok()?;
    let output_tokens = usage.get("output_tokens")?.as_u64()?.try_into().ok()?;
    Some((input_tokens, output_tokens))
}

/// Flush accumulated text or thinking content into the assistant content list.
fn flush_content(block_type: Option<&str>, text: &str, content: &mut Vec<serde_json::Value>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::mpsc;
    use std::time::Duration;

    #[derive(Default)]
    struct CaptureEmitter {
        events: parking_lot::Mutex<Vec<StreamEvent>>,
    }

    impl CaptureEmitter {
        fn events(&self) -> Vec<StreamEvent> {
            self.events.lock().clone()
        }
    }

    impl EventEmitter for CaptureEmitter {
        fn emit(&self, event: StreamEvent) {
            self.events.lock().push(event);
        }
    }

    #[test]
    fn external_tools_can_be_replaced_after_session_creation() {
        let adapter = AnthropicAdapter::new("test-key".to_string())
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
            .tool_definitions()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"mcp__new__tool".to_string()));
        assert!(!names.contains(&"mcp__old__tool".to_string()));
    }

    #[test]
    fn delegate_task_schema_exposes_worktree_worker_mode() {
        let adapter = AnthropicAdapter::new("test-key".to_string()).unwrap();
        let delegate = adapter
            .tool_definitions()
            .into_iter()
            .find(|tool| tool.name == "delegate_task")
            .expect("delegate_task tool");
        let modes = delegate.input_schema["properties"]["mode"]["enum"]
            .as_array()
            .expect("mode enum")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();

        assert!(modes.contains(&"research"));
        assert!(modes.contains(&"patch_proposal"));
        assert!(modes.contains(&"worktree_worker"));
    }

    #[test]
    fn delegate_task_schema_exposes_optional_root_planning_task_flag() {
        let adapter = AnthropicAdapter::new("test-key".to_string()).unwrap();
        let delegate = adapter
            .tool_definitions()
            .into_iter()
            .find(|tool| tool.name == "delegate_task")
            .expect("delegate_task tool");
        let root_planning_task = &delegate.input_schema["properties"]["root_planning_task"];
        let required = delegate.input_schema["required"]
            .as_array()
            .expect("required array")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();

        assert_eq!(root_planning_task["type"], "boolean");
        assert!(root_planning_task["description"]
            .as_str()
            .expect("root_planning_task description")
            .contains("root"));
        assert!(!required.contains(&"root_planning_task"));
    }

    #[tokio::test]
    async fn call_repairs_tool_history_before_serializing_request() {
        let (base_url, received_body) = spawn_json_capture_server(serde_json::json!({
            "content": [{ "type": "text", "text": "ok" }],
            "stop_reason": "end_turn"
        }));
        let adapter = AnthropicAdapter::new("test-key".to_string())
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
        assert_anthropic_tool_result_contract(request_messages);
        assert!(request_messages
            .iter()
            .any(|message| { message["role"] == "user" && message["content"] == "继续处理" }));
    }

    #[tokio::test]
    async fn call_with_emitter_uses_subagent_tools_and_emits_usage() {
        let (base_url, received_body) = spawn_json_capture_server(serde_json::json!({
            "content": [{ "type": "text", "text": "ok" }],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 123,
                "output_tokens": 45
            }
        }));
        let adapter = AnthropicAdapter::new("test-key".to_string())
            .unwrap()
            .with_base_url(&base_url);
        let emitter = CaptureEmitter::default();

        let result = adapter
            .call_with_emitter(
                "subagent",
                &[ChatMessage::user("inspect")],
                &emitter,
                Arc::new(Notify::new()),
            )
            .await
            .expect("adapter call");
        let request_body = received_body
            .recv_timeout(Duration::from_secs(2))
            .expect("captured request body");
        let tool_names = request_body["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();

        assert_eq!(request_body["stream"], false);
        assert_eq!(
            result.assistant_content,
            vec![serde_json::json!({"type": "text", "text": "ok"})]
        );
        assert!(tool_names.contains(&"read_file"));
        assert!(!tool_names.contains(&"write_to_file"));
        assert!(!tool_names.contains(&"edit_file"));
        assert!(!tool_names.contains(&"run_shell"));
        assert!(!tool_names.contains(&"delegate_task"));
        assert!(emitter.events().iter().any(|event| matches!(
            event,
            StreamEvent::Usage {
                session_id,
                input_tokens: 123,
                output_tokens: 45,
                ..
            } if session_id == "subagent"
        )));
        assert!(emitter.events().iter().any(|event| matches!(
            event,
            StreamEvent::ProviderUsage {
                session_id,
                model: Some(model),
                input_tokens: Some(123),
                output_tokens: Some(45),
                estimated_cost_micros: Some(1044),
                source: Some(source),
                reason: crate::protocol::events::ProviderUsageReason::ProviderReported,
                ..
            } if session_id == "subagent"
                && model == "claude-sonnet-4-6"
                && source == "anthropic"
        )));
    }

    #[tokio::test]
    async fn call_with_emitter_records_unknown_usage_when_provider_omits_usage() {
        let (base_url, _received_body) = spawn_json_capture_server(serde_json::json!({
            "content": [{ "type": "text", "text": "ok" }],
            "stop_reason": "end_turn"
        }));
        let adapter = AnthropicAdapter::new("test-key".to_string())
            .unwrap()
            .with_base_url(&base_url);
        let emitter = CaptureEmitter::default();

        adapter
            .call_with_emitter(
                "subagent",
                &[ChatMessage::user("inspect")],
                &emitter,
                Arc::new(Notify::new()),
            )
            .await
            .expect("adapter call");

        assert!(emitter.events().iter().any(|event| matches!(
            event,
            StreamEvent::ProviderUsage {
                session_id,
                model: Some(model),
                input_tokens: None,
                output_tokens: None,
                estimated_cost_micros: None,
                source: Some(source),
                reason: crate::protocol::events::ProviderUsageReason::ProviderOmitted,
                ..
            } if session_id == "subagent"
                && model == "claude-sonnet-4-6"
                && source == "anthropic"
        )));
        assert!(!emitter
            .events()
            .iter()
            .any(|event| matches!(event, StreamEvent::Usage { .. })));
    }

    #[tokio::test]
    async fn call_with_emitter_records_known_tokens_and_unknown_cost_for_unknown_pricing() {
        let (base_url, _received_body) = spawn_json_capture_server(serde_json::json!({
            "content": [{ "type": "text", "text": "ok" }],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 123,
                "output_tokens": 45
            }
        }));
        let adapter = AnthropicAdapter::new("test-key".to_string())
            .unwrap()
            .with_model("mystery-model-v1")
            .with_base_url(&base_url);
        let emitter = CaptureEmitter::default();

        adapter
            .call_with_emitter(
                "subagent",
                &[ChatMessage::user("inspect")],
                &emitter,
                Arc::new(Notify::new()),
            )
            .await
            .expect("adapter call");

        assert!(emitter.events().iter().any(|event| matches!(
            event,
            StreamEvent::ProviderUsage {
                session_id,
                model: Some(model),
                input_tokens: Some(123),
                output_tokens: Some(45),
                estimated_cost_micros: None,
                source: Some(source),
                reason: crate::protocol::events::ProviderUsageReason::PricingUnknown,
                ..
            } if session_id == "subagent"
                && model == "mystery-model-v1"
                && source == "anthropic"
        )));
        assert!(!emitter
            .events()
            .iter()
            .any(|event| matches!(event, StreamEvent::Usage { .. })));
    }

    #[test]
    fn stream_request_repairs_tool_history_before_serializing_request() {
        let adapter = AnthropicAdapter::new("test-key".to_string()).unwrap();
        let messages = vec![
            ChatMessage::assistant(serde_json::json!([{
                "type": "tool_use",
                "id": "call_1",
                "name": "read_file",
                "input": { "path": "src/App.tsx" }
            }])),
            ChatMessage::user("继续处理"),
        ];

        let request = adapter.request_for_messages(&messages, true, false);
        let request_body = serde_json::to_value(&request).expect("serialize request");
        let request_messages = request_body["messages"]
            .as_array()
            .expect("request messages");

        assert_eq!(request_body["stream"], true);
        assert_anthropic_tool_result_contract(request_messages);
    }

    #[test]
    fn parses_streaming_tool_call_split_across_sse_and_network_chunks() {
        let text_start = serde_json::json!({
            "type": "content_block_start",
            "content_block": { "type": "text", "text": "" }
        });
        let text_delta = serde_json::json!({
            "type": "content_block_delta",
            "delta": { "type": "text_delta", "text": "先看文件。" }
        });
        let text_stop = serde_json::json!({ "type": "content_block_stop" });
        let tool_start = serde_json::json!({
            "type": "content_block_start",
            "content_block": {
                "type": "tool_use",
                "id": "toolu_1",
                "name": "read_file",
                "input": {}
            }
        });
        let tool_delta_1 = serde_json::json!({
            "type": "content_block_delta",
            "delta": { "type": "input_json_delta", "partial_json": "{\"path\"" }
        });
        let tool_delta_2 = serde_json::json!({
            "type": "content_block_delta",
            "delta": { "type": "input_json_delta", "partial_json": ":\"src/App.tsx\"}" }
        });
        let tool_stop = serde_json::json!({ "type": "content_block_stop" });
        let message_delta = serde_json::json!({
            "type": "message_delta",
            "delta": { "stop_reason": "tool_use" }
        });

        let first_event = format!("data: {text_start}\n\ndata: {text_delta}\n\n");
        let rest = format!(
            "data: {text_stop}\n\ndata: {{not-json}}\n\ndata: {tool_start}\n\ndata: {tool_delta_1}\n\ndata: {tool_delta_2}\n\ndata: {tool_stop}\n\ndata: {message_delta}\n\n"
        );
        let result = parse_anthropic_stream_chunks(&[
            &first_event[..first_event.len() - 2],
            &first_event[first_event.len() - 2..],
            &rest,
        ]);

        assert_eq!(
            result.assistant_content,
            vec![
                serde_json::json!({"type": "text", "text": "先看文件。"}),
                serde_json::json!({
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "read_file",
                    "input": { "path": "src/App.tsx" }
                })
            ]
        );
        assert_eq!(result.stop_reason.as_deref(), Some("tool_use"));
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].id, "toolu_1");
        assert_eq!(result.tool_calls[0].name, "read_file");
        assert_eq!(
            result.tool_calls[0].input,
            serde_json::json!({ "path": "src/App.tsx" })
        );
    }

    #[test]
    fn provider_conformance_anthropic_streaming_fixture_captures_text_thinking_tool_and_usage() {
        let events = [
            serde_json::json!({
                "type": "message_start",
                "message": {
                    "usage": { "input_tokens": 123 }
                }
            }),
            serde_json::json!({
                "type": "content_block_start",
                "content_block": { "type": "thinking", "thinking": "" }
            }),
            serde_json::json!({
                "type": "content_block_delta",
                "delta": { "type": "thinking_delta", "thinking": "Need the file before editing." }
            }),
            serde_json::json!({ "type": "content_block_stop" }),
            serde_json::json!({
                "type": "content_block_start",
                "content_block": { "type": "text", "text": "" }
            }),
            serde_json::json!({
                "type": "content_block_delta",
                "delta": { "type": "text_delta", "text": "先看文件。" }
            }),
            serde_json::json!({ "type": "content_block_stop" }),
            serde_json::json!({
                "type": "content_block_start",
                "content_block": {
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "read_file",
                    "input": {}
                }
            }),
            serde_json::json!({
                "type": "content_block_delta",
                "delta": { "type": "input_json_delta", "partial_json": "{\"path\"" }
            }),
            serde_json::json!({
                "type": "content_block_delta",
                "delta": { "type": "input_json_delta", "partial_json": ":\"src/App.tsx\"}" }
            }),
            serde_json::json!({ "type": "content_block_stop" }),
            serde_json::json!({
                "type": "message_delta",
                "delta": { "stop_reason": "tool_use" },
                "usage": { "output_tokens": 45 }
            }),
        ];

        let mut parser = AnthropicStreamParser::default();
        let mut thinking_chunk = None;
        let mut text_chunk = None;
        let mut usage = None;
        for event in events {
            let update = parser.apply_event(&event);
            thinking_chunk = thinking_chunk.or(update.thinking_chunk);
            text_chunk = text_chunk.or(update.text_chunk);
            usage = usage.or(update.usage);
        }
        let result = parser.finish();

        assert_eq!(
            thinking_chunk.as_deref(),
            Some("Need the file before editing.")
        );
        assert_eq!(text_chunk.as_deref(), Some("先看文件。"));
        assert_eq!(usage, Some((123, 45)));
        assert_eq!(
            result.assistant_content,
            vec![
                serde_json::json!({
                    "type": "thinking",
                    "thinking": "Need the file before editing."
                }),
                serde_json::json!({"type": "text", "text": "先看文件。"}),
                serde_json::json!({
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "read_file",
                    "input": { "path": "src/App.tsx" }
                })
            ]
        );
        assert_eq!(result.stop_reason.as_deref(), Some("tool_use"));
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].id, "toolu_1");
        assert_eq!(result.tool_calls[0].name, "read_file");
        assert_eq!(
            result.tool_calls[0].input,
            serde_json::json!({ "path": "src/App.tsx" })
        );
    }

    fn assert_anthropic_tool_result_contract(messages: &[serde_json::Value]) {
        for (index, message) in messages.iter().enumerate() {
            let Some(content) = message.get("content").and_then(|value| value.as_array()) else {
                continue;
            };
            let ids = content
                .iter()
                .filter(|block| {
                    block.get("type").and_then(|value| value.as_str()) == Some("tool_use")
                })
                .filter_map(|block| block.get("id").and_then(|value| value.as_str()))
                .collect::<Vec<_>>();
            if ids.is_empty() {
                continue;
            }
            let next = messages.get(index + 1).unwrap_or_else(|| {
                panic!("assistant tool_use at {index} is missing an immediate tool_result message")
            });
            assert_eq!(next["role"], "user");
            let result_ids = next["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|block| {
                    block.get("type").and_then(|value| value.as_str()) == Some("tool_result")
                })
                .filter_map(|block| block.get("tool_use_id").and_then(|value| value.as_str()))
                .collect::<Vec<_>>();
            for id in ids {
                assert!(result_ids.contains(&id), "missing tool_result for {id}");
            }
        }
    }

    fn parse_anthropic_stream_chunks(chunks: &[&str]) -> StreamResult {
        let mut buffer = String::new();
        let mut parser = AnthropicStreamParser::default();

        for chunk in chunks {
            for data in drain_anthropic_sse_data(&mut buffer, chunk) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
                    parser.apply_event(&parsed);
                }
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
