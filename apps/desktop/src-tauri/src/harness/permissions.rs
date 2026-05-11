use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::harness::db::Database;

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
                "write_to_file".into(), "write_file".into(), "write".into(),
                "edit_file".into(), "edit".into(),
            ]),
            session_cache: RwLock::new(HashMap::new()),
            db,
        }
    }

    /// Check if a tool is allowed without prompting.
    pub async fn is_allowed(&self, session_id: &str, tool: &str, _input: &serde_json::Value) -> bool {
        // 0. Check persistent database first
        if self.db.is_permission_approved(tool).unwrap_or(false) {
            return true;
        }

        // 1. Check global patterns
        {
            let patterns = self.allowed_patterns.read().await;
            if patterns.iter().any(|p| tool.contains(p) || p == tool) {
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
            .insert(tool.to_string(), true);
    }

    /// Add a global allowed pattern (persisted to config).
    pub async fn allow_pattern(&self, pattern: &str) {
        self.allowed_patterns.write().await.push(pattern.to_string());
    }

    /// Permanently approve a tool: add to in-memory allowed patterns and persist to database.
    pub async fn approve_permanently(&self, tool: &str) {
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
