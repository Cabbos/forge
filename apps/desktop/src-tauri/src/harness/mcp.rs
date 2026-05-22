use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Notify;

use crate::process_runner::{configure_command_process_group, kill_child_process_group};

type McpStdoutLines = tokio::io::Lines<BufReader<ChildStdout>>;

struct StdioMcpSession {
    child: Child,
    stdin: ChildStdin,
    reader: McpStdoutLines,
}

#[derive(Debug, Clone)]
pub struct McpServerDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub enabled: bool,
    pub command: Option<String>,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpToolDefinition {
    pub server_id: String,
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpResourceDefinition {
    pub server_id: String,
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpPromptDefinition {
    pub server_id: String,
    pub name: String,
    pub description: String,
    pub arguments: Vec<McpPromptArgumentDefinition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpPromptArgumentDefinition {
    pub name: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpResourceContent {
    pub uri: String,
    pub mime_type: Option<String>,
    pub text: Option<String>,
    pub blob: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpPromptMessage {
    pub role: String,
    pub text: String,
}

pub fn public_tool_name(server_id: &str, tool_name: &str) -> String {
    format!(
        "mcp__{}__{}",
        sanitize_public_segment(server_id),
        sanitize_public_segment(tool_name)
    )
}

pub fn is_public_tool_name(tool_name: &str) -> bool {
    tool_name.starts_with("mcp__")
}

pub fn public_tool_segments(tool_name: &str) -> Option<(&str, &str)> {
    let rest = tool_name.strip_prefix("mcp__")?;
    rest.split_once("__")
}

#[derive(Debug, Deserialize)]
struct McpConfig {
    #[serde(default)]
    servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, Deserialize)]
struct McpServerConfig {
    name: Option<String>,
    description: Option<String>,
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    enabled: Option<bool>,
}

pub fn load_mcp_servers(working_dir: &Path) -> Vec<McpServerDefinition> {
    let config_path = working_dir.join(".forge").join("mcp.json");
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return Vec::new();
    };
    let Ok(config) = serde_json::from_str::<McpConfig>(&content) else {
        return Vec::new();
    };

    config
        .servers
        .into_iter()
        .filter_map(|(id, server)| {
            let id = normalize_id(&id)?;
            let command = server.command;
            Some(McpServerDefinition {
                name: server.name.unwrap_or_else(|| id.clone()),
                description: server
                    .description
                    .or_else(|| command.clone())
                    .unwrap_or_default(),
                source: config_path.to_string_lossy().to_string(),
                enabled: server.enabled.unwrap_or(true),
                command,
                args: server.args,
                id,
            })
        })
        .collect()
}

pub async fn discover_stdio_tools(
    server: &McpServerDefinition,
) -> Result<Vec<McpToolDefinition>, String> {
    let mut session = start_stdio_session(server).await?;
    initialize_stdio_session(&mut session).await?;
    write_json_line(
        &mut session.stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await?;
    let response = read_response(&mut session.reader, 2).await?;
    let tools = parse_tools_response(&server.id, &response);

    close_stdio_session(session).await;

    Ok(tools)
}

pub async fn discover_stdio_resources(
    server: &McpServerDefinition,
) -> Result<Vec<McpResourceDefinition>, String> {
    let mut session = start_stdio_session(server).await?;
    initialize_stdio_session(&mut session).await?;
    write_json_line(
        &mut session.stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "resources/list",
            "params": {}
        }),
    )
    .await?;
    let response = read_response(&mut session.reader, 2).await?;
    let resources = parse_resources_response(&server.id, &response);

    close_stdio_session(session).await;

    Ok(resources)
}

pub async fn discover_stdio_prompts(
    server: &McpServerDefinition,
) -> Result<Vec<McpPromptDefinition>, String> {
    let mut session = start_stdio_session(server).await?;
    initialize_stdio_session(&mut session).await?;
    write_json_line(
        &mut session.stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "prompts/list",
            "params": {}
        }),
    )
    .await?;
    let response = read_response(&mut session.reader, 2).await?;
    let prompts = parse_prompts_response(&server.id, &response);

    close_stdio_session(session).await;

    Ok(prompts)
}

pub async fn read_stdio_resource(
    server: &McpServerDefinition,
    uri: &str,
) -> Result<Vec<McpResourceContent>, String> {
    let mut session = start_stdio_session(server).await?;
    initialize_stdio_session(&mut session).await?;
    write_json_line(
        &mut session.stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "resources/read",
            "params": {
                "uri": uri
            }
        }),
    )
    .await?;
    let response = read_response(&mut session.reader, 2).await?;
    let contents = parse_resource_read_response(&response);

    close_stdio_session(session).await;

    Ok(contents)
}

pub async fn get_stdio_prompt(
    server: &McpServerDefinition,
    prompt_name: &str,
    arguments: serde_json::Value,
) -> Result<Vec<McpPromptMessage>, String> {
    let mut session = start_stdio_session(server).await?;
    initialize_stdio_session(&mut session).await?;
    write_json_line(
        &mut session.stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "prompts/get",
            "params": {
                "name": prompt_name,
                "arguments": arguments
            }
        }),
    )
    .await?;
    let response = read_response(&mut session.reader, 2).await?;
    let messages = parse_prompt_get_response(&response);

    close_stdio_session(session).await;

    Ok(messages)
}

pub async fn call_stdio_tool(
    server: &McpServerDefinition,
    tool_name: &str,
    arguments: serde_json::Value,
) -> Result<String, String> {
    call_stdio_tool_with_cancel(server, tool_name, arguments, None).await
}

pub async fn call_stdio_tool_with_cancel(
    server: &McpServerDefinition,
    tool_name: &str,
    arguments: serde_json::Value,
    cancel: Option<Arc<Notify>>,
) -> Result<String, String> {
    let mut session = start_stdio_session(server).await?;
    initialize_stdio_session(&mut session).await?;
    write_json_line(
        &mut session.stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        }),
    )
    .await?;
    let response = read_response_with_cancel(&mut session.reader, 2, cancel).await?;
    let result = parse_tool_call_response(&response);

    close_stdio_session(session).await;

    Ok(result)
}

async fn start_stdio_session(server: &McpServerDefinition) -> Result<StdioMcpSession, String> {
    let command = server
        .command
        .as_deref()
        .ok_or_else(|| format!("MCP server '{}' has no command configured", server.id))?;

    let mut command = Command::new(command);
    command
        .args(&server.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_command_process_group(&mut command);

    let mut child = command
        .spawn()
        .map_err(|err| format!("Failed to start MCP server '{}': {err}", server.id))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| format!("MCP server '{}' stdin is unavailable", server.id))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("MCP server '{}' stdout is unavailable", server.id))?;

    Ok(StdioMcpSession {
        child,
        stdin,
        reader: BufReader::new(stdout).lines(),
    })
}

async fn initialize_stdio_session(session: &mut StdioMcpSession) -> Result<(), String> {
    write_json_line(
        &mut session.stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "Forge",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        }),
    )
    .await?;
    let _ = read_response(&mut session.reader, 1).await?;

    write_json_line(
        &mut session.stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }),
    )
    .await
}

async fn close_stdio_session(mut session: StdioMcpSession) {
    kill_child_process_group(&mut session.child).await;
    let _ = session.child.wait().await;
}

async fn write_json_line(stdin: &mut ChildStdin, value: serde_json::Value) -> Result<(), String> {
    let mut line = serde_json::to_string(&value)
        .map_err(|err| format!("Failed to encode MCP request: {err}"))?;
    line.push('\n');
    stdin
        .write_all(line.as_bytes())
        .await
        .map_err(|err| format!("Failed to write MCP request: {err}"))?;
    stdin
        .flush()
        .await
        .map_err(|err| format!("Failed to flush MCP request: {err}"))
}

async fn read_response(
    reader: &mut McpStdoutLines,
    expected_id: u64,
) -> Result<serde_json::Value, String> {
    let deadline = std::time::Duration::from_secs(5);
    loop {
        let line = tokio::time::timeout(deadline, reader.next_line())
            .await
            .map_err(|_| format!("Timed out waiting for MCP response id {expected_id}"))?
            .map_err(|err| format!("Failed to read MCP response: {err}"))?
            .ok_or_else(|| format!("MCP server closed stdout before response id {expected_id}"))?;
        let value = serde_json::from_str::<serde_json::Value>(&line)
            .map_err(|err| format!("Invalid MCP response JSON: {err}"))?;
        if value.get("id").and_then(|id| id.as_u64()) != Some(expected_id) {
            continue;
        }
        if let Some(error) = value.get("error") {
            return Err(format!("MCP response error: {}", error));
        }
        return Ok(value);
    }
}

async fn read_response_with_cancel(
    reader: &mut McpStdoutLines,
    expected_id: u64,
    cancel: Option<Arc<Notify>>,
) -> Result<serde_json::Value, String> {
    if let Some(cancel) = cancel {
        tokio::select! {
            result = read_response(reader, expected_id) => result,
            _ = cancel.notified() => Err("MCP tool call cancelled".to_string()),
        }
    } else {
        read_response(reader, expected_id).await
    }
}

fn parse_tools_response(server_id: &str, response: &serde_json::Value) -> Vec<McpToolDefinition> {
    response
        .get("result")
        .and_then(|result| result.get("tools"))
        .and_then(|tools| tools.as_array())
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    let name = tool.get("name").and_then(|value| value.as_str())?;
                    Some(McpToolDefinition {
                        server_id: server_id.to_string(),
                        name: name.to_string(),
                        description: tool
                            .get("description")
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        input_schema: tool
                            .get("inputSchema")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!({"type": "object"})),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_resources_response(
    server_id: &str,
    response: &serde_json::Value,
) -> Vec<McpResourceDefinition> {
    response
        .get("result")
        .and_then(|result| result.get("resources"))
        .and_then(|resources| resources.as_array())
        .map(|resources| {
            resources
                .iter()
                .filter_map(|resource| {
                    let uri = resource.get("uri").and_then(|value| value.as_str())?;
                    Some(McpResourceDefinition {
                        server_id: server_id.to_string(),
                        uri: uri.to_string(),
                        name: resource
                            .get("name")
                            .and_then(|value| value.as_str())
                            .unwrap_or(uri)
                            .to_string(),
                        description: resource
                            .get("description")
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        mime_type: resource
                            .get("mimeType")
                            .and_then(|value| value.as_str())
                            .map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_prompts_response(
    server_id: &str,
    response: &serde_json::Value,
) -> Vec<McpPromptDefinition> {
    response
        .get("result")
        .and_then(|result| result.get("prompts"))
        .and_then(|prompts| prompts.as_array())
        .map(|prompts| {
            prompts
                .iter()
                .filter_map(|prompt| {
                    let name = prompt.get("name").and_then(|value| value.as_str())?;
                    Some(McpPromptDefinition {
                        server_id: server_id.to_string(),
                        name: name.to_string(),
                        description: prompt
                            .get("description")
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        arguments: parse_prompt_arguments(prompt.get("arguments")),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_prompt_arguments(
    arguments: Option<&serde_json::Value>,
) -> Vec<McpPromptArgumentDefinition> {
    arguments
        .and_then(|arguments| arguments.as_array())
        .map(|arguments| {
            arguments
                .iter()
                .filter_map(|argument| {
                    let name = argument.get("name").and_then(|value| value.as_str())?;
                    Some(McpPromptArgumentDefinition {
                        name: name.to_string(),
                        description: argument
                            .get("description")
                            .and_then(|value| value.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        required: argument
                            .get("required")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_resource_read_response(response: &serde_json::Value) -> Vec<McpResourceContent> {
    response
        .get("result")
        .and_then(|result| result.get("contents"))
        .and_then(|contents| contents.as_array())
        .map(|contents| {
            contents
                .iter()
                .filter_map(|content| {
                    let uri = content.get("uri").and_then(|value| value.as_str())?;
                    Some(McpResourceContent {
                        uri: uri.to_string(),
                        mime_type: content
                            .get("mimeType")
                            .and_then(|value| value.as_str())
                            .map(str::to_string),
                        text: content
                            .get("text")
                            .and_then(|value| value.as_str())
                            .map(str::to_string),
                        blob: content
                            .get("blob")
                            .and_then(|value| value.as_str())
                            .map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_prompt_get_response(response: &serde_json::Value) -> Vec<McpPromptMessage> {
    response
        .get("result")
        .and_then(|result| result.get("messages"))
        .and_then(|messages| messages.as_array())
        .map(|messages| {
            messages
                .iter()
                .filter_map(|message| {
                    let role = message.get("role").and_then(|value| value.as_str())?;
                    let text = extract_text_content(message.get("content")?)?;
                    Some(McpPromptMessage {
                        role: role.to_string(),
                        text,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn extract_text_content(content: &serde_json::Value) -> Option<String> {
    if let Some(text) = content.get("text").and_then(|value| value.as_str()) {
        return Some(text.to_string());
    }

    content.as_array().and_then(|items| {
        let text = items
            .iter()
            .filter_map(|item| item.get("text").and_then(|value| value.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        (!text.trim().is_empty()).then_some(text)
    })
}

fn parse_tool_call_response(response: &serde_json::Value) -> String {
    let Some(result) = response.get("result") else {
        return String::new();
    };
    let is_error = result
        .get("isError")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let text = result
        .get("content")
        .and_then(|content| content.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if item.get("type").and_then(|value| value.as_str()) == Some("text") {
                        item.get("text").and_then(|value| value.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| result.to_string());

    if is_error {
        format!("Error: {}", text)
    } else {
        text
    }
}

fn sanitize_public_segment(value: &str) -> String {
    let mut output = String::new();
    let mut last_was_separator = false;

    for ch in value.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            output.push(ch);
            last_was_separator = false;
        } else if !last_was_separator {
            output.push('_');
            last_was_separator = true;
        }
    }

    let output = output.trim_matches('_').to_string();
    if output.is_empty() {
        "tool".to_string()
    } else {
        output
    }
}

fn normalize_id(id: &str) -> Option<String> {
    let normalized = id.trim().to_lowercase();
    if normalized.is_empty()
        || normalized.contains("..")
        || normalized.contains('/')
        || normalized.contains('\\')
    {
        return None;
    }
    Some(normalized)
}
