use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

const FORGE_SOURCE_WARNING: &str = "这是 Forge 自己的开发目录。继续操作可能修改 Forge 本体。";

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
    let is_forge_source = is_forge_source_workspace(working_dir);

    WriteBoundary {
        title: "准备修改项目".to_string(),
        workspace_name: workspace_name(working_dir),
        workspace_path: working_dir.to_string_lossy().into_owned(),
        operation: operation_label(tool_name).to_string(),
        impact: impact_text(&affected_files),
        affected_files,
        command,
        risk: risk_for(kind, is_forge_source),
        recovery: "交付区会显示预览和检查点状态。".to_string(),
        warning: is_forge_source.then(|| FORGE_SOURCE_WARNING.to_string()),
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
        "这个命令可能影响当前工作空间".to_string()
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

fn workspace_name(working_dir: &Path) -> String {
    working_dir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("当前项目")
        .to_string()
}

fn is_forge_source_workspace(working_dir: &Path) -> bool {
    if !working_dir.join("src-tauri").is_dir() {
        return false;
    }

    let Ok(package_json) = std::fs::read_to_string(working_dir.join("package.json")) else {
        return false;
    };

    serde_json::from_str::<Value>(&package_json)
        .ok()
        .and_then(|package| {
            package
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .as_deref()
        == Some("forge")
}
