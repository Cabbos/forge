use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::harness::db::Database;

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
                "read_file".into(), "read".into(),
                "list_directory".into(), "ls".into(), "list".into(),
                "search_files".into(), "glob".into(),
                "search_content".into(), "grep".into(),
                "web_search".into(), "web_fetch".into(),
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
                if is_readonly_shell_command(command) {
                    return PermissionDecision::Allow;
                }
                PermissionDecision::Ask {
                    question: format_shell_question(command),
                    kind: if is_dangerous_shell_command(command) {
                        "dangerous_cmd".to_string()
                    } else {
                        "shell_cmd".to_string()
                    },
                    remember_key: None,
                }
            }
            "ask_user" => PermissionDecision::Allow,
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
    pub async fn is_allowed(&self, session_id: &str, tool: &str, _input: &serde_json::Value) -> bool {
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
        cache.entry(session_id.to_string())
            .or_default()
            .insert(canonical_tool(tool).to_string(), true);
    }

    /// Add a global allowed pattern (persisted to config).
    pub async fn allow_pattern(&self, pattern: &str) {
        self.allowed_patterns.write().await.push(pattern.to_string());
    }

    /// Permanently approve a tool: add to in-memory allowed patterns and persist to database.
    pub async fn approve_permanently(&self, tool: &str) {
        let tool = canonical_tool(tool);
        self.allowed_patterns.write().await.push(tool.to_string());
        let _ = self.db.upsert_permission(tool, true);
    }

    /// Check if a tool needs confirmation. Returns Some(question) if it does.
    pub fn needs_confirmation(tool: &str) -> Option<String> {
        match tool {
            "write_to_file" | "edit_file" => Some("Write to file?".into()),
            "run_shell" => Some("Execute shell command?".into()),
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
        "bash" | "execute_command" | "shell" => "run_shell",
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
    let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("(未提供路径)");
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

fn is_readonly_shell_command(command: &str) -> bool {
    let lower = command.trim().to_lowercase();
    if lower.is_empty()
        || contains_shell_control(&lower)
        || references_external_path(&lower)
        || is_dangerous_shell_command(&lower)
    {
        return false;
    }

    let allowed_prefixes = [
        "pwd",
        "ls",
        "git status",
        "git diff",
        "git log",
        "git show",
        "rg ",
        "grep ",
        "find ",
        "cat ",
        "sed -n",
        "wc ",
        "npm run build",
        "cargo test",
        "cargo check",
        "cargo fmt --check",
    ];
    allowed_prefixes.iter().any(|prefix| {
        let prefix = *prefix;
        lower == prefix.trim_end()
            || lower.starts_with(prefix)
            || lower
                .strip_prefix(prefix.trim_end())
                .map(|rest| rest.starts_with(' '))
                .unwrap_or(false)
    })
}

fn is_dangerous_shell_command(command: &str) -> bool {
    let lower = command.trim().to_lowercase();
    let dangerous = [
        "rm ",
        "rmdir ",
        "sudo ",
        "su ",
        "chmod ",
        "chown ",
        "git push",
        "git reset",
        "git checkout --",
        "npm publish",
        "cargo publish",
        "curl ",
        "wget ",
        "dd ",
        "mkfs",
        "mv ",
        "cp ",
        "python -c",
        "node -e",
        "perl -e",
        "ruby -e",
    ];
    dangerous.iter().any(|pattern| {
        lower.starts_with(pattern)
            || lower.contains(&format!("&& {}", pattern))
            || lower.contains(&format!("|| {}", pattern))
            || lower.contains(&format!("; {}", pattern))
            || lower.contains(&format!("| {}", pattern))
    })
}

fn contains_shell_control(command: &str) -> bool {
    ["&&", "||", ";", "|", "`", "$(", ">", "<"]
        .iter()
        .any(|token| command.contains(token))
}

fn references_external_path(command: &str) -> bool {
    command.contains("~/")
        || command.contains("$home")
        || command.contains("../")
        || command.contains("..\\")
        || command.contains(" /")
        || command.starts_with('/')
        || command.contains(" file://")
}
