use std::collections::HashSet;
use std::path::PathBuf;

/// Permission decision for a requested operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    /// Operation is safe — proceed.
    Allow,
    /// Operation needs user approval — emit confirm_ask.
    Ask { question: String, kind: String },
    /// Operation is blocked (e.g., outside working directory).
    Deny { reason: String },
}

/// Gate that intercepts potentially dangerous operations.
pub struct PermissionGate {
    working_dir: PathBuf,
    /// Operation patterns that need confirmation.
    dangerous_patterns: Vec<&'static str>,
    /// Previously approved operations within this session.
    approved: HashSet<String>,
}

impl PermissionGate {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            dangerous_patterns: vec![
                "rm ",
                "rmdir ",
                "del ",
                "deltree ",
                "sudo ",
                "su ",
                "chmod 777",
                "chown ",
                "> /dev/",
                "mkfs.",
                "dd if=",
                "curl ",
                "wget ",
                "git push",
                "git reset --hard",
                "npm publish",
                "cargo publish",
            ],
            approved: HashSet::new(),
        }
    }

    /// Mark an operation as pre-approved.
    pub fn approve(&mut self, key: &str) {
        self.approved.insert(key.to_string());
    }

    /// Check a file write operation.
    pub fn check_file_write(&self, path: &str) -> PermissionDecision {
        let p = std::path::Path::new(path);
        let resolved = if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.working_dir.join(p)
        };
        let work_canon = self
            .working_dir
            .canonicalize()
            .unwrap_or_else(|_| self.working_dir.clone());
        if let Ok(canonical) = resolved.canonicalize() {
            if !canonical.starts_with(&work_canon) {
                return PermissionDecision::Deny {
                    reason: format!(
                        "Write to {} blocked: outside working directory",
                        canonical.display()
                    ),
                };
            }
        } else if let Some(parent) = resolved.parent() {
            if let Ok(parent_canon) = parent.canonicalize() {
                if !parent_canon.starts_with(&work_canon) {
                    return PermissionDecision::Deny {
                        reason: format!(
                            "Write to {} blocked: outside working directory",
                            resolved.display()
                        ),
                    };
                }
            }
        }
        PermissionDecision::Allow
    }

    /// Check a shell command for dangerous patterns.
    pub fn check_shell_command(&self, command: &str) -> PermissionDecision {
        let lower = command.to_lowercase().trim().to_string();
        if contains_shell_control(&lower) || references_external_path(&lower) {
            return PermissionDecision::Ask {
                question: format!("Allow this shell command?\n\n```\n{}\n```", command),
                kind: "shell_cmd".to_string(),
            };
        }
        for pattern in &self.dangerous_patterns {
            let p = pattern.trim();
            if lower.starts_with(p)
                || lower.contains(&format!("&& {}", p))
                || lower.contains(&format!("|| {}", p))
                || lower.contains(&format!("; {}", p))
                || lower.contains(&format!("| {}", p))
            {
                if self.approved.iter().any(|a| a == p) {
                    return PermissionDecision::Allow;
                }
                return PermissionDecision::Ask {
                    question: format!(
                        "Allow this potentially dangerous command?\n\n```\n{}\n```",
                        command
                    ),
                    kind: "dangerous_cmd".to_string(),
                };
            }
        }
        PermissionDecision::Allow
    }

    /// Check a file delete operation.
    pub fn check_file_delete(&self, _path: &str) -> PermissionDecision {
        PermissionDecision::Ask {
            question: format!("Delete file: {}?", _path),
            kind: "file_delete".to_string(),
        }
    }
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
