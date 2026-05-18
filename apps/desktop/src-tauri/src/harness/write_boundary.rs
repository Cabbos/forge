use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::harness::mcp;
use crate::workspace_safety::{classify_existing_workspace_path, WorkspaceRisk};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteBoundaryRisk {
    Normal,
    Caution,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteBoundary {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_label: Option<String>,
    pub workspace_name: String,
    pub workspace_path: String,
    pub operation: String,
    pub affected_files: Vec<String>,
    pub command: Option<String>,
    pub impact: String,
    pub risk: WriteBoundaryRisk,
    pub recovery: String,
    pub warning: Option<String>,
}

pub fn build_write_boundary(
    tool_name: &str,
    input: &Value,
    working_dir: &Path,
    kind: &str,
) -> WriteBoundary {
    let tool_name = canonical_tool_name(tool_name);
    if kind == "mcp_tool" || mcp::is_public_tool_name(tool_name) {
        return build_mcp_boundary(tool_name, input, working_dir);
    }
    if kind == "mcp_resource_read" || tool_name == "mcp_read_resource" {
        return build_mcp_resource_boundary(input, working_dir);
    }
    if kind == "mcp_prompt_get" || tool_name == "mcp_get_prompt" {
        return build_mcp_prompt_boundary(input, working_dir);
    }
    let affected_files = affected_files_for(tool_name, input);
    let command = if tool_name == "run_shell" {
        input
            .get("command")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    } else {
        None
    };
    let workspace_safety = classify_existing_workspace_path(working_dir);
    let is_forge_source = workspace_safety.risk == WorkspaceRisk::High;

    WriteBoundary {
        title: "准备修改项目".to_string(),
        target_label: None,
        workspace_name: workspace_safety.display_name,
        workspace_path: workspace_safety
            .canonical_path
            .to_string_lossy()
            .into_owned(),
        operation: operation_label(tool_name).to_string(),
        impact: impact_text(&affected_files),
        affected_files,
        command,
        risk: risk_for(kind, is_forge_source),
        recovery: "交付区会显示预览和检查点状态。".to_string(),
        warning: workspace_safety.warning,
    }
}

fn build_mcp_boundary(tool_name: &str, input: &Value, working_dir: &Path) -> WriteBoundary {
    let (server, tool) = mcp::public_tool_segments(tool_name).unwrap_or(("连接", tool_name));
    let workspace = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());

    WriteBoundary {
        title: "准备调用连接".to_string(),
        target_label: Some("连接".to_string()),
        workspace_name: server.to_string(),
        workspace_path: workspace.to_string_lossy().into_owned(),
        operation: "调用工具".to_string(),
        affected_files: Vec::new(),
        command: Some(tool_name.to_string()),
        impact: format!("参数：{}", summarize_json(input, 500)),
        risk: WriteBoundaryRisk::Caution,
        recovery: format!(
            "将调用 {} 提供的 {} 工具；Forge 不会绕过连接自身的权限。",
            server, tool
        ),
        warning: None,
    }
}

fn build_mcp_resource_boundary(input: &Value, working_dir: &Path) -> WriteBoundary {
    let server = input
        .get("server_id")
        .and_then(Value::as_str)
        .unwrap_or("连接");
    let uri = input
        .get("uri")
        .and_then(Value::as_str)
        .unwrap_or("(未提供资料地址)");
    let workspace = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());

    WriteBoundary {
        title: "准备读取连接资料".to_string(),
        target_label: Some("连接".to_string()),
        workspace_name: server.to_string(),
        workspace_path: workspace.to_string_lossy().into_owned(),
        operation: "读取资料".to_string(),
        affected_files: Vec::new(),
        command: Some(uri.to_string()),
        impact: format!("资料：{}", truncate_text(uri, 500)),
        risk: WriteBoundaryRisk::Caution,
        recovery: "读取结果只应进入本轮上下文；取消后不会读取连接资料。".to_string(),
        warning: None,
    }
}

fn build_mcp_prompt_boundary(input: &Value, working_dir: &Path) -> WriteBoundary {
    let server = input
        .get("server_id")
        .and_then(Value::as_str)
        .unwrap_or("连接");
    let name = input
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("(未提供提示词名称)");
    let arguments = input
        .get("arguments")
        .map(|value| summarize_json(value, 500))
        .unwrap_or_else(|| "{}".to_string());
    let workspace = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());

    WriteBoundary {
        title: "准备使用连接提示词".to_string(),
        target_label: Some("连接".to_string()),
        workspace_name: server.to_string(),
        workspace_path: workspace.to_string_lossy().into_owned(),
        operation: "使用提示词".to_string(),
        affected_files: Vec::new(),
        command: Some(name.to_string()),
        impact: format!("参数：{}", arguments),
        risk: WriteBoundaryRisk::Caution,
        recovery: "提示词结果只应辅助本轮任务；取消后不会使用连接提示词。".to_string(),
        warning: None,
    }
}

fn canonical_tool_name(tool_name: &str) -> &str {
    match tool_name {
        "write" | "write_file" => "write_to_file",
        "edit" => "edit_file",
        "bash" | "execute_command" | "shell" => "run_shell",
        other => other,
    }
}

fn affected_files_for(tool_name: &str, input: &Value) -> Vec<String> {
    match tool_name {
        "write_to_file" | "edit_file" => input
            .get("path")
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty())
            .map(|path| vec![path.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn operation_label(tool_name: &str) -> &'static str {
    match tool_name {
        "edit_file" => "修改文件",
        "write_to_file" => "写入文件",
        "run_shell" => "执行命令",
        _ => "执行操作",
    }
}

fn impact_text(affected_files: &[String]) -> String {
    if affected_files.is_empty() {
        "这个命令可能影响当前项目".to_string()
    } else {
        format!("将修改 {} 个文件", affected_files.len())
    }
}

fn risk_for(kind: &str, is_forge_source: bool) -> WriteBoundaryRisk {
    if is_forge_source || kind == "dangerous_cmd" {
        WriteBoundaryRisk::High
    } else if kind == "file_write" || kind == "shell_cmd" || kind == "mcp_tool" {
        WriteBoundaryRisk::Caution
    } else {
        WriteBoundaryRisk::Normal
    }
}

fn summarize_json(value: &Value, max_chars: usize) -> String {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
    truncate_text(&text, max_chars)
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let text = value;
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut truncated = text.chars().take(max_chars).collect::<String>();
    truncated.push('…');
    truncated
}
