use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

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
    } else if kind == "file_write" || kind == "shell_cmd" {
        WriteBoundaryRisk::Caution
    } else {
        WriteBoundaryRisk::Normal
    }
}
