#[cfg(test)]
mod tests {
    use super::super::permissions::{PermissionDecision, PermissionGate, PermissionMode};
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
    async fn trust_current_project_allows_workspace_writes_for_matching_workspace() {
        let (db, dir) = temp_db();
        std::fs::create_dir_all(dir.join("src")).expect("create src");
        std::fs::write(dir.join("src/main.rs"), "fn main() {}").expect("write main.rs");
        let gate = PermissionGate::new(db);

        gate.trust_current_project("session-1", &dir).await;

        let state = gate.permission_mode_state("session-1", Some(&dir)).await;
        let canonical_dir = dir.canonicalize().expect("canonical temp dir");
        assert_eq!(state.mode, PermissionMode::TrustCurrentProject);
        assert_eq!(
            state.workspace_path.as_deref(),
            Some(canonical_dir.to_string_lossy().as_ref())
        );
        assert!(!state.session_scoped);

        let input = serde_json::json!({"path": "src/main.rs"});
        let decision = gate.check("session-1", "write_to_file", &input, &dir).await;
        assert!(
            matches!(decision, PermissionDecision::Allow),
            "trusted current project should allow matching workspace writes: {:?}",
            decision
        );

        let other_session = gate.check("session-2", "write_to_file", &input, &dir).await;
        assert!(
            matches!(other_session, PermissionDecision::Allow),
            "trust mode should follow the trusted workspace into new sessions: {:?}",
            other_session
        );
        let other_state = gate.permission_mode_state("session-2", Some(&dir)).await;
        assert_eq!(other_state.mode, PermissionMode::TrustCurrentProject);

        gate.restore_manual_confirm("session-2", Some(&dir)).await;
        let restored = gate.check("session-1", "write_to_file", &input, &dir).await;
        assert!(
            matches!(restored, PermissionDecision::Ask { .. }),
            "manual confirmation should be restored after disabling trust mode: {:?}",
            restored
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn trust_current_project_does_not_allow_sensitive_or_external_paths() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        gate.trust_current_project("session-1", &dir).await;

        let env_decision = gate
            .check(
                "session-1",
                "write_to_file",
                &serde_json::json!({"path": ".env"}),
                &dir,
            )
            .await;
        assert!(
            matches!(env_decision, PermissionDecision::Ask { .. }),
            "sensitive files should still require confirmation: {:?}",
            env_decision
        );

        let outside_decision = gate
            .check(
                "session-1",
                "write_to_file",
                &serde_json::json!({"path": "/etc/passwd"}),
                &dir,
            )
            .await;
        assert!(
            matches!(outside_decision, PermissionDecision::Deny { .. }),
            "outside workspace writes should still be denied: {:?}",
            outside_decision
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn full_access_current_project_allows_confirmable_workspace_operations() {
        let (db, dir) = temp_db();
        std::fs::create_dir_all(dir.join("src")).expect("create src");
        std::fs::write(dir.join("src/main.rs"), "fn main() {}").expect("write main.rs");
        let gate = PermissionGate::new(db);

        gate.full_access_current_project("session-1", &dir).await;

        let state = gate.permission_mode_state("session-1", Some(&dir)).await;
        let canonical_dir = dir.canonicalize().expect("canonical temp dir");
        assert_eq!(state.mode, PermissionMode::FullAccess);
        assert_eq!(
            state.workspace_path.as_deref(),
            Some(canonical_dir.to_string_lossy().as_ref())
        );
        assert!(!state.session_scoped);

        let write_decision = gate
            .check(
                "session-1",
                "write_to_file",
                &serde_json::json!({"path": ".env"}),
                &dir,
            )
            .await;
        assert!(
            matches!(write_decision, PermissionDecision::Allow),
            "full access should allow sensitive workspace writes without prompting: {:?}",
            write_decision
        );

        let shell_decision = gate
            .check(
                "session-1",
                "run_shell",
                &serde_json::json!({"command": "npm install left-pad"}),
                &dir,
            )
            .await;
        assert!(
            matches!(shell_decision, PermissionDecision::Allow),
            "full access should allow confirmable shell commands: {:?}",
            shell_decision
        );

        let mcp_decision = gate
            .check(
                "session-1",
                "mcp_read_resource",
                &serde_json::json!({"server_id": "obsidian", "uri": "file:///notes/forge.md"}),
                &dir,
            )
            .await;
        assert!(
            matches!(mcp_decision, PermissionDecision::Allow),
            "full access should allow connector context reads: {:?}",
            mcp_decision
        );

        let unknown_decision = gate
            .check(
                "session-1",
                "unknown_magic_tool",
                &serde_json::json!({}),
                &dir,
            )
            .await;
        assert!(
            matches!(unknown_decision, PermissionDecision::Allow),
            "full access should allow otherwise confirmable tools: {:?}",
            unknown_decision
        );

        let other_session = gate
            .check(
                "session-2",
                "run_shell",
                &serde_json::json!({"command": "npm install left-pad"}),
                &dir,
            )
            .await;
        assert!(
            matches!(other_session, PermissionDecision::Allow),
            "full access should follow the workspace into new sessions: {:?}",
            other_session
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn full_access_current_project_keeps_hard_blocks_and_deny_rules() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        gate.full_access_current_project("session-1", &dir).await;

        let outside_decision = gate
            .check(
                "session-1",
                "edit_file",
                &serde_json::json!({"path": "/etc/passwd"}),
                &dir,
            )
            .await;
        assert!(
            matches!(outside_decision, PermissionDecision::Deny { .. }),
            "full access must not allow writes outside the workspace: {:?}",
            outside_decision
        );

        let blocked_shell = gate
            .check(
                "session-1",
                "run_shell",
                &serde_json::json!({"command": "rm -rf /"}),
                &dir,
            )
            .await;
        assert!(
            matches!(blocked_shell, PermissionDecision::Deny { .. }),
            "full access must not bypass catastrophic shell blocks: {:?}",
            blocked_shell
        );

        gate.deny_permanently("run_shell").await;
        let denied_shell = gate
            .check(
                "session-1",
                "run_shell",
                &serde_json::json!({"command": "npm install left-pad"}),
                &dir,
            )
            .await;
        assert!(
            matches!(denied_shell, PermissionDecision::Deny { .. }),
            "explicit deny rules should override full access: {:?}",
            denied_shell
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn switching_modes_for_session_clears_previous_workspace_mode() {
        let (db, dir) = temp_db();
        let other_dir = std::env::temp_dir().join(format!(
            "forge-perm-gate-test-other-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&other_dir).expect("other temp dir");
        let gate = PermissionGate::new(db);

        gate.full_access_current_project("session-1", &dir).await;
        assert_eq!(
            gate.permission_mode_state("session-1", Some(&dir))
                .await
                .mode,
            PermissionMode::FullAccess
        );

        gate.trust_current_project("session-1", &other_dir).await;
        assert_eq!(
            gate.permission_mode_state("session-1", Some(&other_dir))
                .await
                .mode,
            PermissionMode::TrustCurrentProject
        );
        assert_eq!(
            gate.permission_mode_state("session-2", Some(&dir))
                .await
                .mode,
            PermissionMode::ManualConfirm
        );

        let former_full_access = gate
            .check(
                "session-2",
                "run_shell",
                &serde_json::json!({"command": "npm install left-pad"}),
                &dir,
            )
            .await;
        assert!(
            matches!(former_full_access, PermissionDecision::Ask { .. }),
            "switching modes should clear previous full-access workspace state: {:?}",
            former_full_access
        );

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&other_dir);
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

    #[tokio::test]
    async fn persistent_deny_overrides_default_allowed_tools() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"path": "src/main.rs"});

        gate.deny_permanently("read").await;

        let decision = gate.check("session-1", "read", &input, &dir).await;
        assert!(
            matches!(decision, PermissionDecision::Deny { .. }),
            "persistent deny should override built-in read allowlist: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn reset_permission_restores_default_policy() {
        let (db, dir) = temp_db();
        let gate = PermissionGate::new(db);
        let input = serde_json::json!({"path": "src/main.rs"});

        gate.deny_permanently("read_file").await;
        assert!(
            matches!(
                gate.check("session-1", "read_file", &input, &dir).await,
                PermissionDecision::Deny { .. }
            ),
            "read_file should be denied before reset"
        );

        gate.reset_permission("read_file").await;

        let decision = gate.check("session-1", "read_file", &input, &dir).await;
        assert!(
            matches!(decision, PermissionDecision::Allow),
            "reset should restore the built-in read_file allow policy: {:?}",
            decision
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn database_lists_latest_permission_rules() {
        let (db, dir) = temp_db();

        db.upsert_permission("write_to_file", true)
            .expect("allow write");
        db.upsert_permission("write_to_file", false)
            .expect("deny write");
        db.upsert_permission("run_shell", true)
            .expect("allow shell");

        let rules = db.list_permission_rules().expect("list rules");

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].tool_name, "run_shell");
        assert!(rules[0].approved);
        assert_eq!(rules[1].tool_name, "write_to_file");
        assert!(!rules[1].approved);
        assert!(!db.is_permission_approved("write_to_file").unwrap());
        assert!(db.is_permission_denied("write_to_file").unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
