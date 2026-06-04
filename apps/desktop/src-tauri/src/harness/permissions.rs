use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::harness::db::Database;
use crate::harness::mcp;
use crate::harness::shell_policy::{classify_shell_command, ShellPolicyDecision, ShellSafetyLevel};

#[derive(Debug, Clone)]
pub enum PermissionDecision {
    Allow,
    Ask {
        question: String,
        kind: String,
        remember_key: Option<String>,
    },
    Deny {
        reason: String,
    },
}

/// Permission gate with pattern-based approval and per-session memory.
/// Inspired by Claude Code's `settings.json` permissions model.
pub struct PermissionGate {
    /// Glob patterns that are permanently allowed.
    allowed_patterns: RwLock<Vec<String>>,
    /// Per-session cached approvals (pattern → allowed).
    session_cache: RwLock<HashMap<String, HashMap<String, bool>>>,
    /// Persistent database-backed permission store.
    db: Arc<Database>,
}

impl PermissionGate {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            allowed_patterns: RwLock::new(vec![
                "read_file".into(),
                "read".into(),
                "list_directory".into(),
                "ls".into(),
                "list".into(),
                "search_files".into(),
                "glob".into(),
                "search_content".into(),
                "grep".into(),
                "web_search".into(),
                "web_fetch".into(),
                "git_diff".into(),
            ]),
            session_cache: RwLock::new(HashMap::new()),
            db,
        }
    }

    pub async fn check(
        &self,
        session_id: &str,
        tool: &str,
        input: &serde_json::Value,
        working_dir: &std::path::Path,
    ) -> PermissionDecision {
        let canonical = canonical_tool(tool);
        match canonical {
            "write_to_file" | "edit_file" => {
                if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                    if let Err(reason) = ensure_path_in_workspace(working_dir, path) {
                        return PermissionDecision::Deny { reason };
                    }
                }
                if self.is_allowed(session_id, tool, input).await {
                    return PermissionDecision::Allow;
                }
                PermissionDecision::Ask {
                    question: format_file_question(tool, input),
                    kind: "file_write".to_string(),
                    remember_key: None,
                }
            }
            "run_shell" => {
                let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
                match classify_shell_command(command) {
                    ShellPolicyDecision::AllowReadonly => PermissionDecision::Allow,
                    ShellPolicyDecision::Blocked { reason } => PermissionDecision::Deny { reason },
                    ShellPolicyDecision::NeedsConfirmation { safety } => PermissionDecision::Ask {
                        question: format_shell_question(command),
                        kind: if safety == ShellSafetyLevel::Dangerous {
                            "dangerous_cmd".to_string()
                        } else {
                            "shell_cmd".to_string()
                        },
                        remember_key: None,
                    },
                }
            }
            "ask_user" => PermissionDecision::Allow,
            "mcp_read_resource" => PermissionDecision::Ask {
                question: format_mcp_resource_question(input),
                kind: "mcp_resource_read".to_string(),
                remember_key: None,
            },
            "mcp_get_prompt" => PermissionDecision::Ask {
                question: format_mcp_prompt_question(input),
                kind: "mcp_prompt_get".to_string(),
                remember_key: None,
            },
            tool if mcp::is_public_tool_name(tool) => {
                if self.is_allowed(session_id, tool, input).await {
                    return PermissionDecision::Allow;
                }
                PermissionDecision::Ask {
                    question: format_mcp_question(tool, input),
                    kind: "mcp_tool".to_string(),
                    remember_key: Some(canonical.to_string()),
                }
            }
            _ => {
                if self.is_allowed(session_id, tool, input).await {
                    return PermissionDecision::Allow;
                }
                PermissionDecision::Ask {
                    question: format!("这个操作需要你确认后才能继续：{}", tool),
                    kind: "confirm".to_string(),
                    remember_key: Some(canonical.to_string()),
                }
            }
        }
    }

    /// Check if a tool is allowed without prompting.
    pub async fn is_allowed(
        &self,
        session_id: &str,
        tool: &str,
        _input: &serde_json::Value,
    ) -> bool {
        let tool = canonical_tool(tool);

        // 0. Check persistent database first
        if self.db.is_permission_approved(tool).unwrap_or(false) {
            return true;
        }

        // 1. Check global patterns
        {
            let patterns = self.allowed_patterns.read().await;
            if patterns.iter().any(|p| p == tool) {
                return true;
            }
        }

        // 2. Check session cache (user already approved this pattern)
        {
            let cache = self.session_cache.read().await;
            if let Some(session_patterns) = cache.get(session_id) {
                if session_patterns.get(tool).copied().unwrap_or(false) {
                    return true;
                }
            }
        }

        // Needs user confirmation
        false
    }

    /// Cache a user's approval for the current session.
    pub async fn approve_in_session(&self, session_id: &str, tool: &str) {
        let mut cache = self.session_cache.write().await;
        cache
            .entry(session_id.to_string())
            .or_default()
            .insert(canonical_tool(tool).to_string(), true);
    }

    /// Add a global allowed pattern (persisted to config).
    pub async fn allow_pattern(&self, pattern: &str) {
        self.allowed_patterns
            .write()
            .await
            .push(pattern.to_string());
    }

    /// Permanently approve a tool: add to in-memory allowed patterns and persist to database.
    pub async fn approve_permanently(&self, tool: &str) {
        let tool = canonical_tool(tool);
        self.allowed_patterns.write().await.push(tool.to_string());
        let _ = self.db.upsert_permission(tool, true);
    }

    /// Check if a tool needs confirmation. Returns Some(question) if it does.
    pub fn needs_confirmation(tool: &str) -> Option<String> {
        if mcp::is_public_tool_name(tool) {
            return Some("Call connector tool?".into());
        }
        match tool {
            "write_to_file" | "edit_file" => Some("Write to file?".into()),
            "run_shell" => Some("Execute shell command?".into()),
            "mcp_read_resource" => Some("Read connector resource?".into()),
            "mcp_get_prompt" => Some("Use connector prompt?".into()),
            _ => None,
        }
    }

    /// Clear session cache on session stop.
    pub async fn clear_session(&self, session_id: &str) {
        self.session_cache.write().await.remove(session_id);
    }
}

fn canonical_tool(tool: &str) -> &str {
    match tool {
        "read" => "read_file",
        "write" | "write_file" => "write_to_file",
        "edit" => "edit_file",
        "ls" | "list" => "list_directory",
        "glob" => "search_files",
        "grep" => "search_content",
        "bash" | "execute_command" | "shell" | "shell_command" | "run_command"
        | "run_shell_command" => "run_shell",
        other => other,
    }
}

fn ensure_path_in_workspace(working_dir: &std::path::Path, path: &str) -> Result<(), String> {
    let requested = std::path::Path::new(path);
    let resolved = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        working_dir.join(requested)
    };

    let canonical = resolved.canonicalize().or_else(|_| {
        resolved
            .parent()
            .and_then(|parent| {
                let parent = parent.canonicalize().ok()?;
                let file_name = resolved.file_name()?;
                Some(parent.join(file_name))
            })
            .ok_or_else(|| format!("无法确认路径是否安全：{}", resolved.display()))
    })?;
    let workspace = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());

    if canonical.starts_with(&workspace) {
        Ok(())
    } else {
        Err(format!(
            "已阻止：这个操作会修改项目目录之外的文件。\n目标：{}\n项目：{}",
            canonical.display(),
            workspace.display()
        ))
    }
}

fn format_file_question(tool: &str, input: &serde_json::Value) -> String {
    let path = input
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("(未提供路径)");
    let action = match canonical_tool(tool) {
        "edit_file" => "修改文件",
        _ => "写入文件",
    };
    format!(
        "AI 想要{}：{}\n\n建议你先确认这个文件是不是本次需求相关；同意后它才会真正改动。",
        action, path
    )
}

fn format_shell_question(command: &str) -> String {
    format!(
        "AI 想要执行下面这条命令：\n\n{}\n\n如果你不确定这条命令的作用，可以先拒绝，再让它解释命令风险。",
        command
    )
}

fn format_mcp_question(tool: &str, input: &serde_json::Value) -> String {
    let args = serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string());
    format!(
        "Forge 想要调用连接工具：{}\n\n参数：{}\n\n连接工具可能会读取或操作该连接提供的数据；不确定时可以取消，再让 Forge 说明用途。",
        tool,
        truncate_inline(&args, 500)
    )
}

fn format_mcp_resource_question(input: &serde_json::Value) -> String {
    let server = input
        .get("server_id")
        .and_then(|value| value.as_str())
        .unwrap_or("连接");
    let uri = input
        .get("uri")
        .and_then(|value| value.as_str())
        .unwrap_or("(未提供资料地址)");

    format!(
        "Forge 想要从 {} 读取连接资料：{}\n\n读取后只应作为本轮上下文使用；不确定时可以取消，再让 Forge 说明为什么需要它。",
        server, uri
    )
}

fn format_mcp_prompt_question(input: &serde_json::Value) -> String {
    let server = input
        .get("server_id")
        .and_then(|value| value.as_str())
        .unwrap_or("连接");
    let name = input
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("(未提供提示词名称)");
    let arguments = input
        .get("arguments")
        .map(|value| serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()))
        .unwrap_or_else(|| "{}".to_string());

    format!(
        "Forge 想要使用连接提示词：{}\n\n连接：{}\n参数：{}\n\n提示词只应辅助本轮任务；不确定时可以取消，再让 Forge 说明用途。",
        name,
        server,
        truncate_inline(&arguments, 500)
    )
}

fn truncate_inline(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_string();
    }
    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── canonical_tool ────────────────────────────────────────────

    #[test]
    fn canonical_tool_normalizes_aliases() {
        assert_eq!(canonical_tool("read"), "read_file");
        assert_eq!(canonical_tool("write"), "write_to_file");
        assert_eq!(canonical_tool("write_file"), "write_to_file");
        assert_eq!(canonical_tool("edit"), "edit_file");
        assert_eq!(canonical_tool("ls"), "list_directory");
        assert_eq!(canonical_tool("list"), "list_directory");
        assert_eq!(canonical_tool("glob"), "search_files");
        assert_eq!(canonical_tool("grep"), "search_content");
        assert_eq!(canonical_tool("bash"), "run_shell");
        assert_eq!(canonical_tool("execute_command"), "run_shell");
        assert_eq!(canonical_tool("shell"), "run_shell");
        assert_eq!(canonical_tool("shell_command"), "run_shell");
        assert_eq!(canonical_tool("run_command"), "run_shell");
        assert_eq!(canonical_tool("run_shell_command"), "run_shell");
    }

    #[test]
    fn canonical_tool_passes_through_unknown() {
        assert_eq!(canonical_tool("read_file"), "read_file");
        assert_eq!(canonical_tool("run_shell"), "run_shell");
        assert_eq!(canonical_tool("custom_tool"), "custom_tool");
    }

    // ── needs_confirmation ────────────────────────────────────────

    #[test]
    fn needs_confirmation_returns_for_write_tools() {
        assert!(PermissionGate::needs_confirmation("write_to_file").is_some());
        assert!(PermissionGate::needs_confirmation("edit_file").is_some());
        assert!(PermissionGate::needs_confirmation("run_shell").is_some());
        assert!(PermissionGate::needs_confirmation("mcp_read_resource").is_some());
        assert!(PermissionGate::needs_confirmation("mcp_get_prompt").is_some());
    }

    #[test]
    fn needs_confirmation_none_for_read_tools() {
        assert!(PermissionGate::needs_confirmation("read_file").is_none());
        assert!(PermissionGate::needs_confirmation("search_files").is_none());
        assert!(PermissionGate::needs_confirmation("list_directory").is_none());
        assert!(PermissionGate::needs_confirmation("unknown_tool").is_none());
    }

    // ── ensure_path_in_workspace ──────────────────────────────────

    #[test]
    fn path_in_workspace_is_allowed() {
        let workspace =
            std::env::temp_dir().join(format!("forge-perm-test-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(workspace.join("src")).unwrap();
        std::fs::write(workspace.join("src/main.rs"), "fn main() {}").unwrap();

        assert!(ensure_path_in_workspace(&workspace, "src/main.rs").is_ok());
        assert!(ensure_path_in_workspace(&workspace, "src/../src/main.rs").is_ok());

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn path_outside_workspace_is_rejected() {
        let workspace =
            std::env::temp_dir().join(format!("forge-perm-test-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&workspace).unwrap();

        let result = ensure_path_in_workspace(&workspace, "/etc/passwd");
        assert!(
            result.is_err(),
            "absolute path outside workspace should be rejected"
        );

        let result = ensure_path_in_workspace(&workspace, "../../etc/passwd");
        assert!(result.is_err(), "traversal path should be rejected");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn path_nonexistent_file_in_workspace_is_allowed() {
        let workspace =
            std::env::temp_dir().join(format!("forge-perm-test-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&workspace).unwrap();

        // File doesn't exist yet, but path resolves within workspace
        assert!(ensure_path_in_workspace(&workspace, "new_file.txt").is_ok());

        let _ = std::fs::remove_dir_all(&workspace);
    }

    // ── format_file_question ──────────────────────────────────────

    #[test]
    fn format_file_question_includes_path_and_chinese() {
        let input = serde_json::json!({"path": "src/main.rs"});
        let q = format_file_question("write_to_file", &input);
        assert!(q.contains("src/main.rs"), "should include path");
        assert!(q.contains("写入文件"), "should describe write action");
        assert!(q.contains("确认"), "should ask for confirmation");
    }

    #[test]
    fn format_file_question_edit_file_action() {
        let input = serde_json::json!({"path": "README.md"});
        let q = format_file_question("edit_file", &input);
        assert!(q.contains("修改文件"), "edit_file should use modify action");
        assert!(q.contains("README.md"));
    }

    #[test]
    fn format_file_question_missing_path() {
        let input = serde_json::json!({});
        let q = format_file_question("write_to_file", &input);
        assert!(
            q.contains("未提供路径"),
            "missing path should show placeholder"
        );
    }

    // ── format_shell_question ─────────────────────────────────────

    #[test]
    fn format_shell_question_includes_command() {
        let q = format_shell_question("npm run build");
        assert!(q.contains("npm run build"));
        assert!(q.contains("命令"), "should mention command");
    }

    // ── truncate_inline ───────────────────────────────────────────

    #[test]
    fn truncate_inline_short_string_unchanged() {
        assert_eq!(truncate_inline("hello", 10), "hello");
    }

    #[test]
    fn truncate_inline_exact_boundary() {
        assert_eq!(truncate_inline("hello", 5), "hello");
    }

    #[test]
    fn truncate_inline_long_string_truncated() {
        let result = truncate_inline("hello world", 5);
        assert_eq!(result, "hello…");
    }

    #[test]
    fn truncate_inline_unicode_chars() {
        let result = truncate_inline("你好世界测试", 3);
        assert_eq!(result, "你好世…");
    }
}
