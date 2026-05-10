//! MCP (Model Context Protocol) runtime — load and manage MCP servers.
//! Reads ~/.claude/mcp.json and ~/.claude/settings.json for server configs,
//! spawns server processes, and discovers their tools.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// An MCP server configuration from mcp.json or settings.json
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
}

/// A tool discovered from an MCP server
#[derive(Debug, Clone, Serialize)]
pub struct McpTool {
    pub server_name: String,
    pub tool_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// A running MCP server process and its tools
pub struct McpConnection {
    pub server_name: String,
    pub tools: Vec<McpTool>,
    process: std::process::Child,
    stdin: Box<dyn Write + Send>,
}

impl McpConnection {
    /// Send a JSON-RPC request to the MCP server and get the response.
    fn send_request(&mut self, method: &str, params: &serde_json::Value) -> Result<serde_json::Value, String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": method, "params": params,
        });
        let mut req_str = serde_json::to_string(&request).unwrap();
        req_str.push('\n');
        self.stdin.write_all(req_str.as_bytes()).map_err(|e| format!("Write error: {}", e))?;
        self.stdin.flush().map_err(|e| format!("Flush error: {}", e))?;

        // Read response from child's stdout (not implemented yet — needs async)
        // For now return a stub
        Err("MCP sync mode not yet implemented".to_string())
    }

    /// Call a tool on this MCP server.
    pub fn call_tool(&mut self, tool_name: &str, input: &serde_json::Value) -> Result<String, String> {
        let response = self.send_request("tools/call", &serde_json::json!({
            "name": tool_name,
            "arguments": input,
        }))?;
        Ok(serde_json::to_string(&response).unwrap_or_default())
    }
}

/// Discover MCP server configs from all known locations.
pub fn discover_configs() -> Vec<(String, McpServerConfig)> {
    let mut configs = Vec::new();

    // 1. ~/.claude/mcp.json — standard MCP config file
    let mcp_json = home_dir().join(".claude").join("mcp.json");
    if let Ok(content) = std::fs::read_to_string(&mcp_json) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(servers) = parsed.get("mcpServers").and_then(|s| s.as_object()) {
                for (name, cfg) in servers {
                    if let Ok(config) = serde_json::from_value::<McpServerConfig>(cfg.clone()) {
                        if !config.disabled { configs.push((name.clone(), config)); }
                    }
                }
            }
        }
    }

    // 2. ~/.claude/settings.json — mcpServers in Claude Code settings
    let settings_json = home_dir().join(".claude").join("settings.json");
    if let Ok(content) = std::fs::read_to_string(&settings_json) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(servers) = parsed.get("mcpServers").and_then(|s| s.as_object()) {
                for (name, cfg) in servers {
                    if let Ok(config) = serde_json::from_value::<McpServerConfig>(cfg.clone()) {
                        if !config.disabled && !configs.iter().any(|(n, _)| n == name) {
                            configs.push((name.clone(), config));
                        }
                    }
                }
            }
        }
    }

    configs
}

/// Start an MCP server and initialize the connection.
pub fn start_server(name: &str, config: &McpServerConfig) -> Result<McpConnection, String> {
    let mut cmd = Command::new(&config.command);
    cmd.args(&config.args);
    for (k, v) in &config.env { cmd.env(k, v); }
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to start MCP server '{}': {}", name, e))?;
    let stdin = child.stdin.take().ok_or("No stdin")?;
    let stdout = child.stdout.take().ok_or("No stdout")?;

    // Initialize MCP protocol
    let init_request = serde_json::json!({
        "jsonrpc": "2.0", "id": 0, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "tui-to-gui", "version": "0.3.0" }
        }
    });

    let mut writer = Box::new(stdin) as Box<dyn Write + Send>;
    let mut req = serde_json::to_string(&init_request).unwrap();
    req.push('\n');
    writer.write_all(req.as_bytes()).map_err(|e| format!("Init write error: {}", e))?;
    writer.flush().unwrap();

    // Read initialization response
    let mut reader = BufReader::new(stdout);
    let mut response = String::new();
    reader.read_line(&mut response).map_err(|e| format!("Init read error: {}", e))?;
    if serde_json::from_str::<serde_json::Value>(&response).is_err() {
        crate::app_log!("WARN", "MCP server '{}' init response not valid JSON: {}", name, response);
    }

    // Send initialized notification
    let notif = serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let mut notif_str = serde_json::to_string(&notif).unwrap();
    notif_str.push('\n');
    writer.write_all(notif_str.as_bytes()).ok();
    writer.flush().ok();

    // Discover tools (list_tools request)
    let list_request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}
    });
    let mut list_str = serde_json::to_string(&list_request).unwrap();
    list_str.push('\n');
    writer.write_all(list_str.as_bytes()).ok();
    writer.flush().ok();

    let mut tools_response = String::new();
    if reader.read_line(&mut tools_response).is_ok() {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&tools_response) {
            if let Some(tools) = parsed.get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_array()) {
                let discovered: Vec<McpTool> = tools.iter().map(|t| McpTool {
                    server_name: name.to_string(),
                    tool_name: t.get("name").and_then(|n| n.as_str()).unwrap_or("unknown").to_string(),
                    description: t.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string(),
                    input_schema: t.get("inputSchema").cloned().unwrap_or(serde_json::json!({})),
                }).collect();
                crate::app_log!("INFO", "MCP server '{}' loaded {} tools", name, discovered.len());
            }
        }
    }

    // For now, return without tools (the connection isn't fully async yet)
    Ok(McpConnection {
        server_name: name.to_string(),
        tools: Vec::new(), // Tools discovered but not stored in connection yet
        process: child,
        stdin: writer,
    })
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}
