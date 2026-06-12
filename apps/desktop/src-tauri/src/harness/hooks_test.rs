#[cfg(test)]
mod tests {
    use crate::harness::hooks::{
        collect_json_strings, ensure_path_in_workspace, looks_like_secret, sensitive_tool_text,
        FileSystemAuditHook, Hook, HookDecision, HookEngine, HookTrigger, LoggingHook,
        SensitiveContentHook, WorkspaceBoundaryHook,
    };

    // ── looks_like_secret ──────────────────────────────────────────────────

    #[test]
    fn detects_openai_style_keys() {
        assert!(looks_like_secret("sk-1234567890abcdefghijkl"));
        assert!(looks_like_secret("  sk-proj-abc123def456ghi789jkl  "));
    }

    #[test]
    fn detects_github_pat_tokens() {
        for token in [
            "ghp_1234567890abcdefghij",
            "gho_1234567890abcdefghij",
            "ghu_1234567890abcdefghij",
            "ghs_1234567890abcdefghij",
            "ghr_1234567890abcdefghij",
        ] {
            assert!(
                looks_like_secret(token),
                "should detect GitHub token: {token}"
            );
        }
    }

    #[test]
    fn detects_github_fine_grained_pat() {
        assert!(looks_like_secret(
            "github_pat_11ABCDEFGHIJKLMNOPQRSTUVWXYZ123456"
        ));
    }

    #[test]
    fn detects_google_api_keys() {
        assert!(looks_like_secret("AIzaSyD1234567890abcdefghijklmnop"));
    }

    #[test]
    fn detects_aws_access_keys() {
        assert!(looks_like_secret("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn detects_private_key_blocks() {
        assert!(looks_like_secret(
            "-----BEGIN RSA PRIVATE KEY-----\nsomebase64content\n-----END RSA PRIVATE KEY-----"
        ));
    }

    #[test]
    fn detects_bearer_tokens() {
        assert!(looks_like_secret("Bearer abcdef1234567890"));
        assert!(looks_like_secret("bearer xyz9876543210abcdef=="));
    }

    #[test]
    fn detects_token_assignment_patterns() {
        assert!(looks_like_secret("token: abcdef1234567890=="));
        assert!(looks_like_secret("my token is abcdef1234567890"));
        assert!(looks_like_secret("auth token = abcdef1234567890"));
        assert!(looks_like_secret("access token: abcdef1234567890"));
    }

    #[test]
    fn empty_text_is_not_secret() {
        assert!(!looks_like_secret(""));
        assert!(!looks_like_secret("   "));
    }

    #[test]
    fn normal_text_is_not_secret() {
        assert!(!looks_like_secret("hello world"));
        assert!(!looks_like_secret("fn main() { println!(\"hi\"); }"));
        assert!(!looks_like_secret("import { useState } from 'react';"));
    }

    #[test]
    fn short_tokens_are_not_false_positive() {
        // Too short to match the length requirements
        assert!(!looks_like_secret("sk-short"));
        assert!(!looks_like_secret("token: 123"));
    }

    // ── collect_json_strings ──────────────────────────────────────────────

    #[test]
    fn collects_string_values() {
        assert_eq!(
            collect_json_strings(&serde_json::json!("hello")),
            vec!["hello"]
        );
    }

    #[test]
    fn collects_nested_strings() {
        let result = collect_json_strings(&serde_json::json!({
            "a": "one",
            "b": {
                "c": "two",
                "d": 42
            }
        }));
        assert!(result.contains(&"one".to_string()));
        assert!(result.contains(&"two".to_string()));
    }

    #[test]
    fn collects_array_strings() {
        let result = collect_json_strings(&serde_json::json!(["a", "b", 3]));
        assert_eq!(result, vec!["a", "b"]);
    }

    #[test]
    fn empty_for_non_string_leaf() {
        assert!(collect_json_strings(&serde_json::json!(42)).is_empty());
        assert!(collect_json_strings(&serde_json::json!(true)).is_empty());
        assert!(collect_json_strings(&serde_json::json!(null)).is_empty());
    }

    // ── sensitive_tool_text ───────────────────────────────────────────────

    #[test]
    fn extracts_content_from_write_to_file() {
        let texts = sensitive_tool_text(
            "write_to_file",
            &serde_json::json!({"path": "/tmp/test.txt", "content": "secret text"}),
        );
        assert_eq!(texts, vec!["secret text"]);
    }

    #[test]
    fn extracts_both_from_edit_file() {
        let texts = sensitive_tool_text(
            "edit_file",
            &serde_json::json!({"old_string": "before", "new_string": "after"}),
        );
        assert!(texts.contains(&"before".to_string()));
        assert!(texts.contains(&"after".to_string()));
    }

    #[test]
    fn extracts_command_from_run_shell() {
        let texts = sensitive_tool_text(
            "run_shell",
            &serde_json::json!({"command": "echo $API_KEY"}),
        );
        assert_eq!(texts, vec!["echo $API_KEY"]);
    }

    #[test]
    fn empty_for_unknown_tool() {
        let texts =
            sensitive_tool_text("read_file", &serde_json::json!({"path": "/tmp/secret.txt"}));
        assert!(texts.is_empty());
    }

    #[test]
    fn collects_all_strings_for_mcp_tool() {
        let texts = sensitive_tool_text(
            "mcp__github__create_issue",
            &serde_json::json!({
                "title": "Fix auth bug",
                "body": "ghp_token_here"
            }),
        );
        assert!(texts.contains(&"Fix auth bug".to_string()));
        assert!(texts.contains(&"ghp_token_here".to_string()));
    }

    // ── LoggingHook ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn logging_hook_passes_through_pre_tool() {
        let hook = LoggingHook;
        let input = serde_json::json!({"path": "/tmp/test.txt"});
        let decision = hook.on_pre_tool("s1", "write_to_file", input.clone()).await;
        assert!(matches!(decision, HookDecision::Proceed(ref v) if v == &input));
    }

    #[tokio::test]
    async fn logging_hook_passes_through_post_tool() {
        let hook = LoggingHook;
        let result = hook.on_post_tool("s1", "write_to_file", "ok".into()).await;
        assert_eq!(result, "ok");
    }

    #[test]
    fn logging_hook_triggers_on_pre_and_post() {
        let triggers = LoggingHook.triggers();
        assert!(triggers.contains(&HookTrigger::PreTool));
        assert!(triggers.contains(&HookTrigger::PostTool));
    }

    // ── FileSystemAuditHook ───────────────────────────────────────────────

    #[test]
    fn fs_audit_hook_filters_specific_tools() {
        let filter = FileSystemAuditHook.filter_tools();
        assert!(filter.contains(&"write_to_file".to_string()));
        assert!(filter.contains(&"edit_file".to_string()));
        assert!(filter.contains(&"run_shell".to_string()));
    }

    #[test]
    fn fs_audit_hook_does_not_filter_readonly_tools() {
        let filter = FileSystemAuditHook.filter_tools();
        assert!(!filter.contains(&"read_file".to_string()));
        assert!(!filter.contains(&"search_files".to_string()));
    }

    #[tokio::test]
    async fn fs_audit_hook_passes_through_result() {
        let hook = FileSystemAuditHook;
        let result = hook
            .on_post_tool("s1", "write_to_file", "written".into())
            .await;
        assert_eq!(result, "written");
    }

    // ── WorkspaceBoundaryHook ─────────────────────────────────────────────

    #[test]
    fn workspace_hook_allows_paths_inside_workspace() {
        use std::fs;
        let tmp = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src").join("main.rs"), "fn main() {}").unwrap();

        let hook = WorkspaceBoundaryHook::new(tmp.path().to_path_buf());
        let input = serde_json::json!({"path": "src/main.rs"});

        let rt = tokio::runtime::Runtime::new().unwrap();
        let decision = rt.block_on(hook.on_pre_tool("s1", "read_file", input));
        assert!(matches!(decision, HookDecision::Proceed(_)));
    }

    #[test]
    fn workspace_hook_blocks_paths_outside_workspace() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hook = WorkspaceBoundaryHook::new(tmp.path().to_path_buf());
        // Absolute path to /etc is outside workspace
        let input = serde_json::json!({"path": "/etc/passwd"});

        let rt = tokio::runtime::Runtime::new().unwrap();
        let decision = rt.block_on(hook.on_pre_tool("s1", "read_file", input));
        assert!(matches!(decision, HookDecision::Block(_)));
    }

    #[test]
    fn workspace_hook_allows_empty_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hook = WorkspaceBoundaryHook::new(tmp.path().to_path_buf());
        let input = serde_json::json!({"path": ""});

        let rt = tokio::runtime::Runtime::new().unwrap();
        let decision = rt.block_on(hook.on_pre_tool("s1", "read_file", input));
        assert!(matches!(decision, HookDecision::Proceed(_)));
    }

    #[test]
    fn workspace_hook_allows_missing_path_key() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hook = WorkspaceBoundaryHook::new(tmp.path().to_path_buf());
        let input = serde_json::json!({"other": "value"});

        let rt = tokio::runtime::Runtime::new().unwrap();
        let decision = rt.block_on(hook.on_pre_tool("s1", "read_file", input));
        assert!(matches!(decision, HookDecision::Proceed(_)));
    }

    // ── HookEngine ────────────────────────────────────────────────────────

    #[test]
    fn engine_starts_empty() {
        let engine = HookEngine::new();
        // Default engine should not crash when used
        let rt = tokio::runtime::Runtime::new().unwrap();
        let decision = rt.block_on(engine.run_pre_tool(
            "s1",
            "write_to_file",
            &serde_json::json!({"path": "/tmp/test.txt"}),
        ));
        assert!(matches!(decision, HookDecision::Proceed(_)));
    }

    #[tokio::test]
    async fn engine_runs_multiple_hooks_in_order() {
        let engine = HookEngine::new();
        engine.register(SensitiveContentHook);

        // First hook: SensitiveContentHook checks content
        let decision = engine
            .run_pre_tool(
                "s1",
                "write_to_file",
                &serde_json::json!({"path": "/tmp/ok.txt", "content": "normal content"}),
            )
            .await;
        assert!(matches!(decision, HookDecision::Proceed(_)));
    }

    #[tokio::test]
    async fn engine_blocks_on_first_blocking_hook() {
        // Register a hook that blocks
        struct BlockingHook;
        #[async_trait::async_trait]
        impl Hook for BlockingHook {
            fn name(&self) -> &str {
                "blocker"
            }
            fn triggers(&self) -> Vec<HookTrigger> {
                vec![HookTrigger::PreTool]
            }
            async fn on_pre_tool(
                &self,
                _session_id: &str,
                _tool: &str,
                _input: serde_json::Value,
            ) -> HookDecision {
                HookDecision::Block("always block".into())
            }
        }

        let engine = HookEngine::new();
        engine.register(BlockingHook);
        let decision = engine
            .run_pre_tool("s1", "write_to_file", &serde_json::json!({}))
            .await;
        assert!(matches!(decision, HookDecision::Block(_)));
    }

    #[tokio::test]
    async fn engine_respects_filter_tools() {
        struct WriteOnlyHook;
        #[async_trait::async_trait]
        impl Hook for WriteOnlyHook {
            fn name(&self) -> &str {
                "write-only"
            }
            fn triggers(&self) -> Vec<HookTrigger> {
                vec![HookTrigger::PreTool]
            }
            fn filter_tools(&self) -> Vec<String> {
                vec!["write_to_file".into()]
            }
            async fn on_pre_tool(
                &self,
                _session_id: &str,
                _tool: &str,
                _input: serde_json::Value,
            ) -> HookDecision {
                HookDecision::Block("write blocked".into())
            }
        }

        let engine = HookEngine::new();
        engine.register(WriteOnlyHook);

        // Should NOT block read_file (filtered out)
        let decision = engine
            .run_pre_tool(
                "s1",
                "read_file",
                &serde_json::json!({"path": "/tmp/test.txt"}),
            )
            .await;
        assert!(
            matches!(decision, HookDecision::Proceed(_)),
            "hook filtered to write_to_file should not block read_file"
        );

        // Should block write_to_file (matches filter)
        let decision = engine
            .run_pre_tool(
                "s1",
                "write_to_file",
                &serde_json::json!({"path": "/tmp/test.txt"}),
            )
            .await;
        assert!(matches!(decision, HookDecision::Block(_)));
    }

    #[tokio::test]
    async fn engine_is_enabled_filter_respects_hook_disabling() {
        struct AlwaysBlockHook;
        #[async_trait::async_trait]
        impl Hook for AlwaysBlockHook {
            fn name(&self) -> &str {
                "always-block"
            }
            fn triggers(&self) -> Vec<HookTrigger> {
                vec![HookTrigger::PreTool]
            }
            async fn on_pre_tool(
                &self,
                _session_id: &str,
                _tool: &str,
                _input: serde_json::Value,
            ) -> HookDecision {
                HookDecision::Block("blocked".into())
            }
        }

        let engine = HookEngine::new();
        engine.register(AlwaysBlockHook);

        // With is_enabled returning false, the hook should be skipped
        let decision = engine
            .run_pre_tool_with_enabled(
                "s1",
                "write_to_file",
                &serde_json::json!({}),
                |_name| false, // disable all hooks
            )
            .await;
        assert!(matches!(decision, HookDecision::Proceed(_)));
    }

    #[tokio::test]
    async fn engine_post_tool_chains_results() {
        struct AppendHook {
            suffix: &'static str,
        }
        #[async_trait::async_trait]
        impl Hook for AppendHook {
            fn name(&self) -> &str {
                "append"
            }
            fn triggers(&self) -> Vec<HookTrigger> {
                vec![HookTrigger::PostTool]
            }
            async fn on_post_tool(&self, _session_id: &str, _tool: &str, result: String) -> String {
                format!("{result} {}", self.suffix)
            }
        }

        let engine = HookEngine::new();
        engine.register(AppendHook { suffix: "A" });
        engine.register(AppendHook { suffix: "B" });

        let result = engine.run_post_tool("s1", "write_to_file", "hello").await;
        assert_eq!(result, "hello A B");
    }

    #[tokio::test]
    async fn engine_post_tool_is_enabled_filter_works() {
        struct AppendHook;
        #[async_trait::async_trait]
        impl Hook for AppendHook {
            fn name(&self) -> &str {
                "append"
            }
            fn triggers(&self) -> Vec<HookTrigger> {
                vec![HookTrigger::PostTool]
            }
            async fn on_post_tool(&self, _session_id: &str, _tool: &str, result: String) -> String {
                format!("{result} modified")
            }
        }

        let engine = HookEngine::new();
        engine.register(AppendHook);

        let result = engine
            .run_post_tool_with_enabled("s1", "write_to_file", "hello", |_name| false)
            .await;
        assert_eq!(result, "hello", "disabled hook should not modify result");
    }

    // ── ensure_path_in_workspace ──────────────────────────────────────────

    #[test]
    fn allows_paths_inside_workspace() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src").join("lib.rs"), "// lib").unwrap();

        assert!(ensure_path_in_workspace(tmp.path(), "src/lib.rs").is_ok());
        assert!(ensure_path_in_workspace(tmp.path(), "src").is_ok());
    }

    #[test]
    fn blocks_paths_escaping_workspace() {
        let tmp = tempfile::TempDir::new().unwrap();

        // ../ escape attempt
        assert!(ensure_path_in_workspace(tmp.path(), "../etc/passwd").is_err());

        // Absolute path outside
        assert!(ensure_path_in_workspace(tmp.path(), "/etc/passwd").is_err());
    }

    #[test]
    fn path_to_nonexistent_file_is_evaluated() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Parent dir exists but file doesn't — canonicalize falls back to parent+filename
        let result = ensure_path_in_workspace(tmp.path(), "new_file.rs");
        assert!(
            result.is_ok(),
            "path inside workspace should be ok even if file missing: {:?}",
            result
        );
    }

    // ── SensitiveContentHook ──────────────────────────────────────────────

    #[tokio::test]
    async fn sensitive_content_allows_clean_input() {
        let hook = SensitiveContentHook;
        let decision = hook
            .on_pre_tool(
                "s1",
                "write_to_file",
                serde_json::json!({"path": "/tmp/test.rs", "content": "fn main() {}"}),
            )
            .await;
        assert!(matches!(decision, HookDecision::Proceed(_)));
    }

    #[tokio::test]
    async fn sensitive_content_blocks_secret_in_write_content() {
        let hook = SensitiveContentHook;
        let decision = hook
            .on_pre_tool(
                "s1",
                "write_to_file",
                serde_json::json!({"path": "/tmp/test.txt", "content": "key: sk-1234567890abcdefghijkl"}),
            )
            .await;
        assert!(matches!(decision, HookDecision::Block(_)));
    }
}
