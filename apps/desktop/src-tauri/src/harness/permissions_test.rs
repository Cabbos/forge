#[cfg(test)]
mod tests {
    use super::super::permissions::{PermissionDecision, PermissionGate};
    use crate::harness::db::Database;
    use std::sync::Arc;

    fn temp_db() -> (Arc<Database>, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("forge-perm-gate-test-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let db_path = dir.join("test.db");
        let db = Arc::new(Database::open(&db_path).expect("open db"));
        (db, dir)
    }

    #[tokio::test]
    async fn check_allows_read_file_without_prompt() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"path": "src/main.rs"});
        let decision = gate.check("session-1", "read_file", &input, &dir).await;
        assert!(
            matches!(decision, PermissionDecision::Allow),
            "read_file should be pre-approved: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn check_asks_for_write_to_file() {
        let (db, dir) = temp_db();
        // Create the file so path resolution succeeds
        std::fs::create_dir_all(dir.join("src")).expect("create src");
        std::fs::write(dir.join("src/main.rs"), "fn main() {}").expect("write main.rs");
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"path": "src/main.rs"});
        let decision = gate.check("session-1", "write_to_file", &input, &dir).await;
        assert!(
            matches!(decision, PermissionDecision::Ask { .. }),
            "write_to_file should ask for confirmation: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn check_denies_write_outside_workspace() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"path": "/etc/passwd"});
        let decision = gate.check("session-1", "write_to_file", &input, &dir).await;
        assert!(
            matches!(decision, PermissionDecision::Deny { .. }),
            "write outside workspace should be denied: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn check_allows_ask_user_without_prompt() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"question": "Continue?"});
        let decision = gate.check("session-1", "ask_user", &input, &dir).await;
        assert!(
            matches!(decision, PermissionDecision::Allow),
            "ask_user should be pre-approved: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn check_allows_readonly_shell_command() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"command": "git status"});
        let decision = gate.check("session-1", "run_shell", &input, &dir).await;
        assert!(
            matches!(decision, PermissionDecision::Allow),
            "readonly shell should be allowed: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn check_asks_for_dangerous_shell_command() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"command": "rm -rf build"});
        let decision = gate.check("session-1", "run_shell", &input, &dir).await;
        assert!(
            matches!(decision, PermissionDecision::Ask { ref kind, .. } if kind == "dangerous_cmd"),
            "dangerous shell should ask with dangerous_cmd kind: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn check_blocks_catastrophic_shell_command() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"command": "rm -rf /"});
        let decision = gate.check("session-1", "run_shell", &input, &dir).await;
        assert!(
            matches!(decision, PermissionDecision::Deny { .. }),
            "catastrophic shell should be denied: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn is_allowed_returns_true_for_pre_approved_tools() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"path": "src/main.rs"});
        assert!(
            gate.is_allowed("session-1", "read_file", &input).await,
            "read_file should be pre-approved"
        );
        assert!(
            gate.is_allowed("session-1", "list_directory", &input).await,
            "list_directory should be pre-approved"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn is_allowed_returns_false_for_unapproved_tools() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"path": "src/main.rs"});
        assert!(
            !gate.is_allowed("session-1", "write_to_file", &input).await,
            "write_to_file should not be pre-approved"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn approve_in_session_makes_tool_allowed() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"path": "src/main.rs"});
        assert!(
            !gate.is_allowed("session-1", "write_to_file", &input).await,
            "should not be allowed before approval"
        );
        gate.approve_in_session("session-1", "write_to_file").await;
        assert!(
            gate.is_allowed("session-1", "write_to_file", &input).await,
            "should be allowed after session approval"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn allow_pattern_adds_global_approval() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"path": "src/main.rs"});
        assert!(
            !gate.is_allowed("session-1", "custom_tool", &input).await,
            "should not be allowed before pattern"
        );
        gate.allow_pattern("custom_tool").await;
        assert!(
            gate.is_allowed("session-1", "custom_tool", &input).await,
            "should be allowed after adding pattern"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn approve_permanently_persists_to_database() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db.clone());
        let input = serde_json::json!({"path": "src/main.rs"});
        assert!(
            !gate.is_allowed("session-1", "permanent_tool", &input).await,
            "should not be allowed before permanent approval"
        );
        gate.approve_permanently("permanent_tool").await;
        assert!(
            gate.is_allowed("session-1", "permanent_tool", &input).await,
            "should be allowed after permanent approval"
        );
        // Verify database persistence
        assert!(
            db.is_permission_approved("permanent_tool").unwrap_or(false),
            "database should record the approval"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn clear_session_removes_session_approvals() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"path": "src/main.rs"});
        gate.approve_in_session("session-1", "write_to_file").await;
        assert!(
            gate.is_allowed("session-1", "write_to_file", &input).await,
            "should be allowed after approval"
        );
        gate.clear_session("session-1").await;
        assert!(
            !gate.is_allowed("session-1", "write_to_file", &input).await,
            "should not be allowed after session cleared"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn session_approvals_are_isolated_per_session() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"path": "src/main.rs"});
        gate.approve_in_session("session-1", "write_to_file").await;
        assert!(
            gate.is_allowed("session-1", "write_to_file", &input).await,
            "session-1 should be allowed"
        );
        assert!(
            !gate.is_allowed("session-2", "write_to_file", &input).await,
            "session-2 should not be allowed"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn check_asks_for_mcp_read_resource() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"server_id": "test", "uri": "file:///test"});
        let decision = gate
            .check("session-1", "mcp_read_resource", &input, &dir)
            .await;
        assert!(
            matches!(decision, PermissionDecision::Ask { ref kind, .. } if kind == "mcp_resource_read"),
            "mcp_read_resource should ask: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn check_asks_for_mcp_get_prompt() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"server_id": "test", "name": "greeting"});
        let decision = gate
            .check("session-1", "mcp_get_prompt", &input, &dir)
            .await;
        assert!(
            matches!(decision, PermissionDecision::Ask { ref kind, .. } if kind == "mcp_prompt_get"),
            "mcp_get_prompt should ask: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn check_asks_for_unknown_tools() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({});
        let decision = gate
            .check("session-1", "unknown_magic_tool", &input, &dir)
            .await;
        assert!(
            matches!(decision, PermissionDecision::Ask { ref kind, .. } if kind == "confirm"),
            "unknown tool should ask for confirm: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn canonical_tool_aliases_work_in_check() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"path": "src/main.rs"});
        // "read" is an alias for "read_file"
        let decision = gate.check("session-1", "read", &input, &dir).await;
        assert!(
            matches!(decision, PermissionDecision::Allow),
            "read alias should be allowed: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
