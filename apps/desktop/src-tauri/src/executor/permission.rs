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
                "rm ", "rmdir ", "del ", "deltree ",
                "sudo ", "su ",
                "chmod 777", "chown ",
                "> /dev/", "mkfs.", "dd if=",
                "curl ", "wget ",
                "git push", "git reset --hard",
                "npm publish", "cargo publish",
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
        let resolved = if p.is_absolute() { p.to_path_buf() } else { self.working_dir.join(p) };
        if let Ok(canonical) = resolved.canonicalize() {
            if !canonical.starts_with(&self.working_dir) {
                return PermissionDecision::Deny {
                    reason: format!("Write to {} blocked: outside working directory", canonical.display()),
                };
            }
        }
        PermissionDecision::Allow
    }

    /// Check a shell command for dangerous patterns.
    pub fn check_shell_command(&self, command: &str) -> PermissionDecision {
        let lower = command.to_lowercase().trim().to_string();
        for pattern in &self.dangerous_patterns {
            let p = pattern.trim();
            if lower.starts_with(p) || format!("{} {}", lower.split_whitespace().next().unwrap_or(""), p.trim()) == lower {
                if self.approved.iter().any(|a| a == p) {
                    return PermissionDecision::Allow;
                }
                return PermissionDecision::Ask {
                    question: format!("Allow this potentially dangerous command?\n\n```\n{}\n```", command),
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
