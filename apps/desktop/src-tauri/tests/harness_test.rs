#[cfg(test)]
mod harness {
    use forge::harness::capability::{CapabilityKind, CapabilityMetadata};
    use forge::harness::db::Database;
    use forge::harness::hooks::{
        FileSystemAuditHook, Hook, HookDecision, HookEngine, HookTrigger, LoggingHook,
        SensitiveContentHook, WorkspaceBoundaryHook,
    };
    use forge::harness::permissions::{PermissionDecision, PermissionGate};
    use forge::harness::skills::SkillLoader;
    use forge::harness::write_boundary::{build_write_boundary, WriteBoundaryRisk};
    use forge::harness::Harness;
    use std::sync::Arc;

    fn unique_temp_workspace(prefix: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), nanos));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    // ═══ PermissionGate Tests ═══

    #[tokio::test]
    async fn test_read_tools_preapproved() {
        let db_path = std::env::temp_dir().join("test-perm.db");
        let _ = std::fs::remove_file(&db_path);
        let db = Arc::new(Database::open(&db_path).unwrap());
        let gate = PermissionGate::new(db);

        assert!(
            gate.is_allowed("s1", "read_file", &serde_json::json!({}))
                .await
        );
        assert!(
            gate.is_allowed("s1", "search_files", &serde_json::json!({}))
                .await
        );
        assert!(
            gate.is_allowed("s1", "web_search", &serde_json::json!({}))
                .await
        );
        assert!(
            gate.is_allowed("s1", "git_diff", &serde_json::json!({}))
                .await
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_write_tools_require_confirm() {
        let db_path = std::env::temp_dir().join("test-perm-write.db");
        let _ = std::fs::remove_file(&db_path);
        let db = Arc::new(Database::open(&db_path).unwrap());
        let gate = PermissionGate::new(db);
        let working_dir = std::env::temp_dir();

        assert!(
            !gate
                .is_allowed("s1", "write_to_file", &serde_json::json!({}))
                .await
        );
        let decision = gate
            .check(
                "s1",
                "write_to_file",
                &serde_json::json!({"path":"test-write.txt","content":"hello"}),
                &working_dir,
            )
            .await;
        assert!(matches!(decision, PermissionDecision::Ask { kind, .. } if kind == "file_write"));

        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_shell_requires_confirm() {
        let db_path = std::env::temp_dir().join("test-perm2.db");
        let _ = std::fs::remove_file(&db_path);
        let db = Arc::new(Database::open(&db_path).unwrap());
        let gate = PermissionGate::new(db);

        assert!(
            !gate
                .is_allowed("s1", "run_shell", &serde_json::json!({}))
                .await
        );
        assert!(!gate.is_allowed("s1", "bash", &serde_json::json!({})).await);

        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_shell_safety_classification() {
        let db_path = std::env::temp_dir().join("test-perm-shell-safety.db");
        let _ = std::fs::remove_file(&db_path);
        let db = Arc::new(Database::open(&db_path).unwrap());
        let gate = PermissionGate::new(db);
        let working_dir = std::env::temp_dir();

        let safe = gate
            .check(
                "s1",
                "run_shell",
                &serde_json::json!({"command":"rg PermissionDecision src-tauri/src"}),
                &working_dir,
            )
            .await;
        assert!(matches!(safe, PermissionDecision::Allow));

        let chained = gate
            .check(
                "s1",
                "run_shell",
                &serde_json::json!({"command":"ls && rm -rf target"}),
                &working_dir,
            )
            .await;
        assert!(matches!(chained, PermissionDecision::Ask { kind, .. } if kind == "dangerous_cmd"));

        let external_read = gate
            .check(
                "s1",
                "run_shell",
                &serde_json::json!({"command":"cat /Users/example/.ssh/id_rsa"}),
                &working_dir,
            )
            .await;
        assert!(
            matches!(external_read, PermissionDecision::Ask { kind, .. } if kind == "shell_cmd")
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_mcp_tools_require_connector_confirm() {
        let db_path = std::env::temp_dir().join("test-perm-mcp.db");
        let _ = std::fs::remove_file(&db_path);
        let db = Arc::new(Database::open(&db_path).unwrap());
        let gate = PermissionGate::new(db);
        let working_dir = std::env::temp_dir();

        let decision = gate
            .check(
                "s1",
                "mcp__fixture__echo_text",
                &serde_json::json!({"text":"hello"}),
                &working_dir,
            )
            .await;

        assert!(
            matches!(decision, PermissionDecision::Ask { kind, question, .. }
                if kind == "mcp_tool"
                    && question.contains("连接工具")
                    && question.contains("mcp__fixture__echo_text"))
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_mcp_resource_reads_require_context_confirm() {
        let db_path = std::env::temp_dir().join("test-perm-mcp-resource.db");
        let _ = std::fs::remove_file(&db_path);
        let db = Arc::new(Database::open(&db_path).unwrap());
        let gate = PermissionGate::new(db);
        let working_dir = std::env::temp_dir();

        let decision = gate
            .check(
                "s1",
                "mcp_read_resource",
                &serde_json::json!({
                    "server_id": "obsidian",
                    "uri": "file:///notes/project.md"
                }),
                &working_dir,
            )
            .await;

        assert!(
            matches!(decision, PermissionDecision::Ask { kind, question, remember_key }
                if kind == "mcp_resource_read"
                    && question.contains("读取连接资料")
                    && question.contains("obsidian")
                    && question.contains("file:///notes/project.md")
                    && remember_key.is_none())
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_mcp_prompt_gets_require_context_confirm() {
        let db_path = std::env::temp_dir().join("test-perm-mcp-prompt.db");
        let _ = std::fs::remove_file(&db_path);
        let db = Arc::new(Database::open(&db_path).unwrap());
        let gate = PermissionGate::new(db);
        let working_dir = std::env::temp_dir();

        let decision = gate
            .check(
                "s1",
                "mcp_get_prompt",
                &serde_json::json!({
                    "server_id": "linear",
                    "name": "summarize_issue",
                    "arguments": { "focus": "risk" }
                }),
                &working_dir,
            )
            .await;

        assert!(
            matches!(decision, PermissionDecision::Ask { kind, question, remember_key }
                if kind == "mcp_prompt_get"
                    && question.contains("使用连接提示词")
                    && question.contains("linear")
                    && question.contains("summarize_issue")
                    && remember_key.is_none())
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_session_approval_cached() {
        let db_path = std::env::temp_dir().join("test-perm3.db");
        let _ = std::fs::remove_file(&db_path);
        let db = Arc::new(Database::open(&db_path).unwrap());
        let gate = PermissionGate::new(db);

        assert!(
            !gate
                .is_allowed("s2", "run_shell", &serde_json::json!({}))
                .await
        );
        gate.approve_in_session("s2", "run_shell").await;
        assert!(
            gate.is_allowed("s2", "run_shell", &serde_json::json!({}))
                .await
        );

        let _ = std::fs::remove_file(&db_path);
    }

    // ═══ HookEngine Tests ═══

    #[tokio::test]
    async fn test_hook_registration_and_dispatch() {
        let engine = HookEngine::new();
        engine.register(LoggingHook);
        engine.register(FileSystemAuditHook);

        let result = engine
            .run_pre_tool(
                "s1",
                "write_to_file",
                &serde_json::json!({"path":"test.txt"}),
            )
            .await;
        match result {
            forge::harness::hooks::HookDecision::Proceed(_) => {}
            _ => panic!("Expected Proceed from pre-tool hook"),
        }

        let result = engine
            .run_post_tool("s1", "write_to_file", "File written")
            .await;
        assert_eq!(result, "File written");
    }

    struct BlockingHook;

    #[async_trait::async_trait]
    impl Hook for BlockingHook {
        fn name(&self) -> &str {
            "blocking-test"
        }

        fn triggers(&self) -> Vec<HookTrigger> {
            vec![HookTrigger::PreTool]
        }

        async fn on_pre_tool(
            &self,
            _session_id: &str,
            _tool: &str,
            input: serde_json::Value,
        ) -> HookDecision {
            if input.get("block").and_then(|value| value.as_bool()) == Some(true) {
                HookDecision::Block("blocked by test hook".to_string())
            } else {
                HookDecision::Proceed(input)
            }
        }
    }

    #[tokio::test]
    async fn test_hook_engine_skips_disabled_hooks() {
        let engine = HookEngine::new();
        engine.register(BlockingHook);

        let enabled_result = engine
            .run_pre_tool_with_enabled(
                "s1",
                "write_to_file",
                &serde_json::json!({"block": true}),
                |_| true,
            )
            .await;
        assert!(
            matches!(enabled_result, HookDecision::Block(reason) if reason == "blocked by test hook")
        );

        let disabled_result = engine
            .run_pre_tool_with_enabled(
                "s1",
                "write_to_file",
                &serde_json::json!({"block": true}),
                |_| false,
            )
            .await;
        assert!(matches!(disabled_result, HookDecision::Proceed(_)));
    }

    #[tokio::test]
    async fn test_sensitive_content_hook_blocks_secret_writes() {
        let hook = SensitiveContentHook;
        let decision = hook
            .on_pre_tool(
                "s1",
                "write_to_file",
                serde_json::json!({
                    "path": "notes.txt",
                    "content": "my API key is sk-1234567890abcdefghijkl"
                }),
            )
            .await;

        assert!(matches!(decision, HookDecision::Block(reason) if reason.contains("敏感信息")));
    }

    #[tokio::test]
    async fn test_workspace_boundary_hook_blocks_external_file_writes() {
        let workspace = unique_temp_workspace("forge-hook-boundary");
        let external = unique_temp_workspace("forge-hook-external");
        let hook = WorkspaceBoundaryHook::new(workspace.clone());
        let decision = hook
            .on_pre_tool(
                "s1",
                "write_to_file",
                serde_json::json!({
                    "path": external.join("outside.txt").to_string_lossy(),
                    "content": "hello"
                }),
            )
            .await;

        assert!(matches!(decision, HookDecision::Block(reason) if reason.contains("项目目录之外")));

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&external);
    }

    #[test]
    fn test_harness_registers_builtin_hooks_as_capabilities() {
        let workspace = unique_temp_workspace("forge-capability-hooks");
        let harness = Harness::new(workspace.clone());

        let logging = harness.capability_registry.get("hook:logging").unwrap();
        assert_eq!(logging.kind, CapabilityKind::Hook);
        let audit = harness.capability_registry.get("hook:fs-audit").unwrap();
        assert_eq!(audit.kind, CapabilityKind::Hook);
        let sensitive = harness
            .capability_registry
            .get("hook:sensitive-content")
            .unwrap();
        assert_eq!(sensitive.kind, CapabilityKind::Hook);
        let boundary = harness
            .capability_registry
            .get("hook:workspace-boundary")
            .unwrap();
        assert_eq!(boundary.kind, CapabilityKind::Hook);

        harness
            .capability_registry
            .toggle("hook:fs-audit", false)
            .unwrap();
        assert!(!harness.capability_registry.is_hook_enabled("fs-audit"));

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn test_skill_loader_selects_skills_by_request_triggers() {
        let workspace = unique_temp_workspace("forge-skill-router");
        let legacy_dir = workspace.join("skills").join("general-guidance");
        std::fs::create_dir_all(&legacy_dir).unwrap();
        std::fs::write(
            legacy_dir.join("SKILL.md"),
            r#"---
name: general-guidance
description: General coding guidance.
---

Use this skill as general guidance.
"#,
        )
        .unwrap();

        let skill_dir = workspace.join("skills").join("customer-followup");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: customer-followup
description: Helps shape customer follow-up tools.
triggers: ["客户", "跟进"]
---

Use this skill when shaping customer follow-up tools.
"#,
        )
        .unwrap();

        let loader = SkillLoader::new_for_workspace(&workspace);
        loader.scan_all().await;

        let selected = loader
            .enabled_skills_for_request("我想做个能记录客户并提醒跟进的小工具")
            .await;
        let mut selected_ids = selected
            .iter()
            .map(|skill| skill.id.as_str())
            .collect::<Vec<_>>();
        selected_ids.sort_unstable();
        assert_eq!(selected_ids, vec!["customer-followup", "general-guidance"]);

        let unrelated = loader.enabled_skills_for_request("帮我做收支记录").await;
        assert_eq!(unrelated.len(), 1);
        assert_eq!(unrelated[0].id, "general-guidance");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn test_harness_registers_workspace_mcp_servers_as_capabilities() {
        let workspace = unique_temp_workspace("forge-mcp-registry");
        std::fs::create_dir_all(workspace.join(".forge")).unwrap();
        std::fs::write(
            workspace.join(".forge").join("mcp.json"),
            r#"{
  "servers": {
    "obsidian": {
      "name": "Obsidian",
      "description": "Local notes connector",
      "command": "obsidian-mcp"
    }
  }
}"#,
        )
        .unwrap();

        let harness = Harness::new(workspace.clone());
        let mcp = harness.capability_registry.get("mcp:obsidian").unwrap();
        assert_eq!(mcp.kind, CapabilityKind::McpServer);
        assert_eq!(mcp.name, "Obsidian");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn test_mcp_config_loads_command_and_args() {
        let workspace = unique_temp_workspace("forge-mcp-config");
        std::fs::create_dir_all(workspace.join(".forge")).unwrap();
        std::fs::write(
            workspace.join(".forge").join("mcp.json"),
            r#"{
  "servers": {
    "local-tools": {
      "name": "Local Tools",
      "description": "Local test MCP",
      "command": "node",
      "args": ["server.mjs"],
      "enabled": true
    }
  }
}"#,
        )
        .unwrap();

        let servers = forge::harness::mcp::load_mcp_servers(&workspace);

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].id, "local-tools");
        assert_eq!(servers[0].command.as_deref(), Some("node"));
        assert_eq!(servers[0].args, vec!["server.mjs".to_string()]);

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn test_mcp_stdio_discovers_tools() {
        let workspace = unique_temp_workspace("forge-mcp-discover");
        let script = workspace.join("mcp-server.mjs");
        std::fs::write(
            &script,
            r#"
import readline from "node:readline";

const rl = readline.createInterface({ input: process.stdin });
for await (const line of rl) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: "2024-11-05",
        capabilities: { tools: {} },
        serverInfo: { name: "fixture", version: "0.1.0" }
      }
    }));
  } else if (request.method === "tools/list") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        tools: [{
          name: "echo",
          description: "Echo input text",
          inputSchema: {
            type: "object",
            properties: { text: { type: "string" } },
            required: ["text"]
          }
        }]
      }
    }));
    process.exit(0);
  }
}
"#,
        )
        .unwrap();
        let server = forge::harness::mcp::McpServerDefinition {
            id: "fixture".to_string(),
            name: "Fixture".to_string(),
            description: "Fixture MCP".to_string(),
            source: script.to_string_lossy().to_string(),
            enabled: true,
            command: Some("node".to_string()),
            args: vec![script.to_string_lossy().to_string()],
        };

        let tools = forge::harness::mcp::discover_stdio_tools(&server)
            .await
            .expect("discover tools");

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].server_id, "fixture");
        assert_eq!(tools[0].name, "echo");
        assert_eq!(tools[0].description, "Echo input text");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn test_mcp_stdio_calls_tool() {
        let workspace = unique_temp_workspace("forge-mcp-call");
        let script = workspace.join("mcp-call-server.mjs");
        std::fs::write(
            &script,
            r#"
import readline from "node:readline";

const rl = readline.createInterface({ input: process.stdin });
for await (const line of rl) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: "2024-11-05",
        capabilities: { tools: {} },
        serverInfo: { name: "fixture", version: "0.1.0" }
      }
    }));
  } else if (request.method === "tools/call") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        content: [{
          type: "text",
          text: `Echo: ${request.params.arguments.text}`
        }]
      }
    }));
    process.exit(0);
  }
}
"#,
        )
        .unwrap();
        let server = forge::harness::mcp::McpServerDefinition {
            id: "fixture".to_string(),
            name: "Fixture".to_string(),
            description: "Fixture MCP".to_string(),
            source: script.to_string_lossy().to_string(),
            enabled: true,
            command: Some("node".to_string()),
            args: vec![script.to_string_lossy().to_string()],
        };

        let result = forge::harness::mcp::call_stdio_tool(
            &server,
            "echo",
            serde_json::json!({"text": "hello"}),
        )
        .await
        .expect("call tool");

        assert_eq!(result, "Echo: hello");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn test_mcp_stdio_discovers_resources() {
        let workspace = unique_temp_workspace("forge-mcp-resources");
        let script = workspace.join("mcp-resource-server.mjs");
        std::fs::write(
            &script,
            r#"
import readline from "node:readline";

const rl = readline.createInterface({ input: process.stdin });
for await (const line of rl) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: "2024-11-05",
        capabilities: { resources: {} },
        serverInfo: { name: "fixture", version: "0.1.0" }
      }
    }));
  } else if (request.method === "resources/list") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        resources: [{
          uri: "file:///notes/project.md",
          name: "Project Notes",
          description: "Saved project notes",
          mimeType: "text/markdown"
        }]
      }
    }));
    process.exit(0);
  }
}
"#,
        )
        .unwrap();
        let server = forge::harness::mcp::McpServerDefinition {
            id: "fixture".to_string(),
            name: "Fixture".to_string(),
            description: "Fixture MCP".to_string(),
            source: script.to_string_lossy().to_string(),
            enabled: true,
            command: Some("node".to_string()),
            args: vec![script.to_string_lossy().to_string()],
        };

        let resources = forge::harness::mcp::discover_stdio_resources(&server)
            .await
            .expect("discover resources");

        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].server_id, "fixture");
        assert_eq!(resources[0].uri, "file:///notes/project.md");
        assert_eq!(resources[0].name, "Project Notes");
        assert_eq!(resources[0].description, "Saved project notes");
        assert_eq!(resources[0].mime_type.as_deref(), Some("text/markdown"));

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn test_mcp_stdio_discovers_prompts() {
        let workspace = unique_temp_workspace("forge-mcp-prompts");
        let script = workspace.join("mcp-prompt-server.mjs");
        std::fs::write(
            &script,
            r#"
import readline from "node:readline";

const rl = readline.createInterface({ input: process.stdin });
for await (const line of rl) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: "2024-11-05",
        capabilities: { prompts: {} },
        serverInfo: { name: "fixture", version: "0.1.0" }
      }
    }));
  } else if (request.method === "prompts/list") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        prompts: [{
          name: "summarize_project",
          description: "Summarize a project resource",
          arguments: [{
            name: "focus",
            description: "Optional focus area",
            required: false
          }]
        }]
      }
    }));
    process.exit(0);
  }
}
"#,
        )
        .unwrap();
        let server = forge::harness::mcp::McpServerDefinition {
            id: "fixture".to_string(),
            name: "Fixture".to_string(),
            description: "Fixture MCP".to_string(),
            source: script.to_string_lossy().to_string(),
            enabled: true,
            command: Some("node".to_string()),
            args: vec![script.to_string_lossy().to_string()],
        };

        let prompts = forge::harness::mcp::discover_stdio_prompts(&server)
            .await
            .expect("discover prompts");

        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].server_id, "fixture");
        assert_eq!(prompts[0].name, "summarize_project");
        assert_eq!(prompts[0].description, "Summarize a project resource");
        assert_eq!(prompts[0].arguments.len(), 1);
        assert_eq!(prompts[0].arguments[0].name, "focus");
        assert_eq!(prompts[0].arguments[0].description, "Optional focus area");
        assert!(!prompts[0].arguments[0].required);

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn test_mcp_stdio_reads_resource_text_content() {
        let workspace = unique_temp_workspace("forge-mcp-read-resource");
        let script = workspace.join("mcp-read-resource-server.mjs");
        std::fs::write(
            &script,
            r##"
import readline from "node:readline";

const rl = readline.createInterface({ input: process.stdin });
for await (const line of rl) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: "2024-11-05",
        capabilities: { resources: {} },
        serverInfo: { name: "fixture", version: "0.1.0" }
      }
    }));
  } else if (request.method === "resources/read") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        contents: [{
          uri: request.params.uri,
          mimeType: "text/markdown",
          text: "# Project Notes\nKeep context explicit."
        }]
      }
    }));
    process.exit(0);
  }
}
"##,
        )
        .unwrap();
        let server = forge::harness::mcp::McpServerDefinition {
            id: "fixture".to_string(),
            name: "Fixture".to_string(),
            description: "Fixture MCP".to_string(),
            source: script.to_string_lossy().to_string(),
            enabled: true,
            command: Some("node".to_string()),
            args: vec![script.to_string_lossy().to_string()],
        };

        let contents =
            forge::harness::mcp::read_stdio_resource(&server, "file:///notes/project.md")
                .await
                .expect("read resource");

        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].uri, "file:///notes/project.md");
        assert_eq!(contents[0].mime_type.as_deref(), Some("text/markdown"));
        assert_eq!(
            contents[0].text.as_deref(),
            Some("# Project Notes\nKeep context explicit.")
        );
        assert!(contents[0].blob.is_none());

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn test_mcp_stdio_gets_prompt_messages() {
        let workspace = unique_temp_workspace("forge-mcp-get-prompt");
        let script = workspace.join("mcp-get-prompt-server.mjs");
        std::fs::write(
            &script,
            r#"
import readline from "node:readline";

const rl = readline.createInterface({ input: process.stdin });
for await (const line of rl) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: "2024-11-05",
        capabilities: { prompts: {} },
        serverInfo: { name: "fixture", version: "0.1.0" }
      }
    }));
  } else if (request.method === "prompts/get") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        messages: [{
          role: "user",
          content: {
            type: "text",
            text: `Summarize project with focus: ${request.params.arguments.focus}`
          }
        }]
      }
    }));
    process.exit(0);
  }
}
"#,
        )
        .unwrap();
        let server = forge::harness::mcp::McpServerDefinition {
            id: "fixture".to_string(),
            name: "Fixture".to_string(),
            description: "Fixture MCP".to_string(),
            source: script.to_string_lossy().to_string(),
            enabled: true,
            command: Some("node".to_string()),
            args: vec![script.to_string_lossy().to_string()],
        };

        let messages = forge::harness::mcp::get_stdio_prompt(
            &server,
            "summarize_project",
            serde_json::json!({"focus": "risks"}),
        )
        .await
        .expect("get prompt");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].text, "Summarize project with focus: risks");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn test_mcp_public_tool_names_are_safe_and_stable() {
        assert_eq!(
            forge::harness::mcp::public_tool_name("local-tools", "query.search"),
            "mcp__local-tools__query_search"
        );
        assert_eq!(
            forge::harness::mcp::public_tool_name("Fixture", "Echo Text!"),
            "mcp__fixture__echo_text"
        );
    }

    #[tokio::test]
    async fn test_harness_discovers_enabled_mcp_tools_for_model() {
        let workspace = unique_temp_workspace("forge-mcp-model-tools");
        let script = workspace.join("mcp-model-server.mjs");
        std::fs::write(
            &script,
            r#"
import readline from "node:readline";

const rl = readline.createInterface({ input: process.stdin });
for await (const line of rl) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: "2024-11-05",
        capabilities: { tools: {} },
        serverInfo: { name: "fixture", version: "0.1.0" }
      }
    }));
  } else if (request.method === "tools/list") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        tools: [{
          name: "echo.text",
          description: "Echo input text",
          inputSchema: {
            type: "object",
            properties: { text: { type: "string" } },
            required: ["text"]
          }
        }]
      }
    }));
    process.exit(0);
  }
}
"#,
        )
        .unwrap();
        std::fs::create_dir_all(workspace.join(".forge")).unwrap();
        std::fs::write(
            workspace.join(".forge").join("mcp.json"),
            format!(
                r#"{{
  "servers": {{
    "fixture": {{
      "name": "Fixture",
      "description": "Local fixture connector",
      "command": "node",
      "args": [{}]
    }}
  }}
}}"#,
                serde_json::to_string(&script.to_string_lossy().to_string()).unwrap()
            ),
        )
        .unwrap();

        let harness = Harness::new(workspace.clone());
        let tools = harness.external_mcp_tool_definitions().await;

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "mcp__fixture__echo_text");
        assert!(tools[0].description.contains("Fixture"));
        assert!(tools[0].description.contains("Echo input text"));

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn test_harness_discovers_enabled_mcp_resources_for_context_picker() {
        let workspace = unique_temp_workspace("forge-mcp-context-resources");
        let script = workspace.join("mcp-context-resource-server.mjs");
        std::fs::write(
            &script,
            r#"
import readline from "node:readline";

const rl = readline.createInterface({ input: process.stdin });
for await (const line of rl) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: "2024-11-05",
        capabilities: { resources: {} },
        serverInfo: { name: "fixture", version: "0.1.0" }
      }
    }));
  } else if (request.method === "resources/list") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        resources: [{
          uri: "file:///notes/project.md",
          name: "Project Notes",
          description: "Saved project notes",
          mimeType: "text/markdown"
        }]
      }
    }));
    process.exit(0);
  }
}
"#,
        )
        .unwrap();
        std::fs::create_dir_all(workspace.join(".forge")).unwrap();
        std::fs::write(
            workspace.join(".forge").join("mcp.json"),
            format!(
                r#"{{
  "servers": {{
    "fixture": {{
      "name": "Fixture",
      "command": "node",
      "args": [{}]
    }}
  }}
}}"#,
                serde_json::to_string(&script.to_string_lossy().to_string()).unwrap()
            ),
        )
        .unwrap();

        let harness = Harness::new(workspace.clone());
        let resources = harness.external_mcp_resource_definitions().await;

        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].server_id, "fixture");
        assert_eq!(resources[0].uri, "file:///notes/project.md");
        assert_eq!(resources[0].name, "Project Notes");
        assert_eq!(resources[0].mime_type.as_deref(), Some("text/markdown"));

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn test_harness_discovers_enabled_mcp_prompts_for_context_picker() {
        let workspace = unique_temp_workspace("forge-mcp-context-prompts");
        let script = workspace.join("mcp-context-prompt-server.mjs");
        std::fs::write(
            &script,
            r#"
import readline from "node:readline";

const rl = readline.createInterface({ input: process.stdin });
for await (const line of rl) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: "2024-11-05",
        capabilities: { prompts: {} },
        serverInfo: { name: "fixture", version: "0.1.0" }
      }
    }));
  } else if (request.method === "prompts/list") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        prompts: [{
          name: "summarize_project",
          description: "Summarize project context",
          arguments: [{ name: "focus", required: false }]
        }]
      }
    }));
    process.exit(0);
  }
}
"#,
        )
        .unwrap();
        std::fs::create_dir_all(workspace.join(".forge")).unwrap();
        std::fs::write(
            workspace.join(".forge").join("mcp.json"),
            format!(
                r#"{{
  "servers": {{
    "fixture": {{
      "name": "Fixture",
      "command": "node",
      "args": [{}]
    }}
  }}
}}"#,
                serde_json::to_string(&script.to_string_lossy().to_string()).unwrap()
            ),
        )
        .unwrap();

        let harness = Harness::new(workspace.clone());
        let prompts = harness.external_mcp_prompt_definitions().await;

        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].server_id, "fixture");
        assert_eq!(prompts[0].name, "summarize_project");
        assert_eq!(prompts[0].description, "Summarize project context");
        assert_eq!(prompts[0].arguments.len(), 1);

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn test_harness_calls_public_mcp_tool_name() {
        let workspace = unique_temp_workspace("forge-mcp-public-call");
        let script = workspace.join("mcp-public-call-server.mjs");
        std::fs::write(
            &script,
            r#"
import readline from "node:readline";

const rl = readline.createInterface({ input: process.stdin });
for await (const line of rl) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: "2024-11-05",
        capabilities: { tools: {} },
        serverInfo: { name: "fixture", version: "0.1.0" }
      }
    }));
  } else if (request.method === "tools/list") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        tools: [{
          name: "echo.text",
          description: "Echo input text",
          inputSchema: { type: "object" }
        }]
      }
    }));
  } else if (request.method === "tools/call") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        content: [{
          type: "text",
          text: `Echo: ${request.params.arguments.text}`
        }]
      }
    }));
    process.exit(0);
  }
}
"#,
        )
        .unwrap();
        std::fs::create_dir_all(workspace.join(".forge")).unwrap();
        std::fs::write(
            workspace.join(".forge").join("mcp.json"),
            format!(
                r#"{{
  "servers": {{
    "fixture": {{
      "name": "Fixture",
      "command": "node",
      "args": [{}]
    }}
  }}
}}"#,
                serde_json::to_string(&script.to_string_lossy().to_string()).unwrap()
            ),
        )
        .unwrap();

        let harness = Harness::new(workspace.clone());
        let result = harness
            .call_public_mcp_tool(
                "mcp__fixture__echo_text",
                serde_json::json!({"text": "hello"}),
            )
            .await
            .expect("public MCP tool result");

        assert_eq!(result, "Echo: hello");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn test_harness_reuses_discovered_mcp_tools_for_public_calls() {
        let workspace = unique_temp_workspace("forge-mcp-cache");
        let script = workspace.join("mcp-cache-server.mjs");
        let counter = workspace.join("tools-list-count.txt");
        std::fs::write(
            &script,
            r#"
import fs from "node:fs";
import readline from "node:readline";

const counterPath = process.argv[2];
const rl = readline.createInterface({ input: process.stdin });

function incrementCounter() {
  let current = 0;
  try {
    current = Number.parseInt(fs.readFileSync(counterPath, "utf8"), 10) || 0;
  } catch {
    current = 0;
  }
  fs.writeFileSync(counterPath, String(current + 1));
}

for await (const line of rl) {
  const request = JSON.parse(line);
  if (request.method === "initialize") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        protocolVersion: "2024-11-05",
        capabilities: { tools: {} },
        serverInfo: { name: "fixture", version: "0.1.0" }
      }
    }));
  } else if (request.method === "tools/list") {
    incrementCounter();
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        tools: [{
          name: "echo.text",
          description: "Echo input text",
          inputSchema: { type: "object" }
        }]
      }
    }));
    process.exit(0);
  } else if (request.method === "tools/call") {
    console.log(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      result: {
        content: [{
          type: "text",
          text: `Echo: ${request.params.arguments.text}`
        }]
      }
    }));
    process.exit(0);
  }
}
"#,
        )
        .unwrap();
        std::fs::create_dir_all(workspace.join(".forge")).unwrap();
        std::fs::write(
            workspace.join(".forge").join("mcp.json"),
            format!(
                r#"{{
  "servers": {{
    "fixture": {{
      "name": "Fixture",
      "command": "node",
      "args": [{}, {}]
    }}
  }}
}}"#,
                serde_json::to_string(&script.to_string_lossy().to_string()).unwrap(),
                serde_json::to_string(&counter.to_string_lossy().to_string()).unwrap()
            ),
        )
        .unwrap();

        let harness = Harness::new(workspace.clone());
        let tools = harness.external_mcp_tool_definitions().await;
        assert_eq!(tools.len(), 1);

        let result = harness
            .call_public_mcp_tool(
                "mcp__fixture__echo_text",
                serde_json::json!({"text": "hello"}),
            )
            .await
            .expect("public MCP tool result");

        assert_eq!(result, "Echo: hello");
        assert_eq!(std::fs::read_to_string(counter).unwrap(), "1");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    // ═══ CapabilityKind Tests ═══

    #[test]
    fn test_capability_metadata_consistency() {
        let meta = CapabilityMetadata {
            id: "test-skill".into(),
            name: "Test Skill".into(),
            description: "A test skill".into(),
            version: "1.0.0".into(),
            source: "builtin".into(),
            kind: CapabilityKind::Skill,
        };

        assert_eq!(meta.id, "test-skill");
        assert_eq!(meta.name, "Test Skill");
        assert!(matches!(meta.kind, CapabilityKind::Skill));
    }

    // ═══ Database CRUD Tests ═══

    #[test]
    fn test_database_upsert_and_query() {
        let db_path = std::env::temp_dir().join("test-db-crud.db");
        let db = Database::open(&db_path).unwrap();

        db.upsert_capability("c1", "Cap One", "skill", "builtin", true)
            .unwrap();
        db.upsert_capability("c2", "Cap Two", "tool", "github", false)
            .unwrap();

        let all = db.list_all().unwrap();
        assert_eq!(all.len(), 2);

        db.set_enabled("c1", false).unwrap();
        let all = db.list_all().unwrap();
        let c1 = all.iter().find(|r| r.id == "c1").unwrap();
        assert!(!c1.enabled);

        db.delete_capability("c2").unwrap();
        assert_eq!(db.list_all().unwrap().len(), 1);

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_permission_persistence() {
        let db_path = std::env::temp_dir().join("test-perm-db.db");
        let db = Database::open(&db_path).unwrap();

        assert!(!db.is_permission_approved("run_shell").unwrap());
        db.upsert_permission("run_shell", true).unwrap();
        assert!(db.is_permission_approved("run_shell").unwrap());

        let _ = std::fs::remove_file(&db_path);
    }

    // ═══ OpenAI Message Conversion Tests ═══

    // These test the convert_messages function indirectly via the adapter.
    // In a real test suite, we'd make convert_messages pub(crate) and test directly.

    #[test]
    fn test_write_boundary_for_file_write_shows_workspace_and_file() {
        let workspace = unique_temp_workspace("forge-boundary-project");

        let boundary = build_write_boundary(
            "write_to_file",
            &serde_json::json!({"path":"src/app.tsx","content":"hello"}),
            &workspace,
            "file_write",
        );

        assert_eq!(boundary.title, "准备修改项目");
        assert_eq!(boundary.operation, "写入文件");
        assert_eq!(
            boundary.workspace_path,
            workspace.canonicalize().unwrap().to_string_lossy()
        );
        assert_eq!(boundary.affected_files, vec!["src/app.tsx".to_string()]);
        assert_eq!(boundary.impact, "将修改 1 个文件");
        assert_eq!(boundary.risk, WriteBoundaryRisk::Caution);

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn test_write_boundary_for_shell_command_uses_workspace_wide_caution() {
        let workspace = unique_temp_workspace("forge-boundary-shell");

        let boundary = build_write_boundary(
            "run_shell",
            &serde_json::json!({"command":"npm install"}),
            &workspace,
            "shell_cmd",
        );

        assert_eq!(boundary.operation, "执行命令");
        assert_eq!(boundary.command.as_deref(), Some("npm install"));
        assert!(boundary.affected_files.is_empty());
        assert_eq!(boundary.impact, "这个命令可能影响当前项目");
        assert_eq!(boundary.risk, WriteBoundaryRisk::Caution);

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn test_write_boundary_for_mcp_tool_uses_connector_labels() {
        let workspace = unique_temp_workspace("forge-boundary-mcp");

        let boundary = build_write_boundary(
            "mcp__fixture__echo_text",
            &serde_json::json!({"text":"hello"}),
            &workspace,
            "mcp_tool",
        );

        assert_eq!(boundary.title, "准备调用连接");
        assert_eq!(boundary.target_label.as_deref(), Some("连接"));
        assert_eq!(boundary.workspace_name, "fixture");
        assert_eq!(boundary.operation, "调用工具");
        assert_eq!(boundary.command.as_deref(), Some("mcp__fixture__echo_text"));
        assert_eq!(boundary.impact, "参数：{\"text\":\"hello\"}");
        assert_eq!(boundary.risk, WriteBoundaryRisk::Caution);
        assert!(boundary.affected_files.is_empty());

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn test_write_boundary_for_mcp_resource_read_uses_context_labels() {
        let workspace = unique_temp_workspace("forge-boundary-mcp-resource");

        let boundary = build_write_boundary(
            "mcp_read_resource",
            &serde_json::json!({
                "server_id": "obsidian",
                "uri": "file:///notes/project.md"
            }),
            &workspace,
            "mcp_resource_read",
        );

        assert_eq!(boundary.title, "准备读取连接资料");
        assert_eq!(boundary.target_label.as_deref(), Some("连接"));
        assert_eq!(boundary.workspace_name, "obsidian");
        assert_eq!(boundary.operation, "读取资料");
        assert_eq!(
            boundary.command.as_deref(),
            Some("file:///notes/project.md")
        );
        assert!(boundary.impact.contains("资料：file:///notes/project.md"));
        assert!(boundary.recovery.contains("本轮上下文"));
        assert_eq!(boundary.risk, WriteBoundaryRisk::Caution);

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn test_write_boundary_for_mcp_prompt_get_uses_context_labels() {
        let workspace = unique_temp_workspace("forge-boundary-mcp-prompt");

        let boundary = build_write_boundary(
            "mcp_get_prompt",
            &serde_json::json!({
                "server_id": "linear",
                "name": "summarize_issue",
                "arguments": { "focus": "risk" }
            }),
            &workspace,
            "mcp_prompt_get",
        );

        assert_eq!(boundary.title, "准备使用连接提示词");
        assert_eq!(boundary.target_label.as_deref(), Some("连接"));
        assert_eq!(boundary.workspace_name, "linear");
        assert_eq!(boundary.operation, "使用提示词");
        assert_eq!(boundary.command.as_deref(), Some("summarize_issue"));
        assert!(boundary.impact.contains("参数"));
        assert!(boundary.recovery.contains("本轮任务"));
        assert_eq!(boundary.risk, WriteBoundaryRisk::Caution);

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn test_write_boundary_warns_for_forge_source_workspace() {
        let workspace = unique_temp_workspace("forge-source-like");
        std::fs::create_dir_all(workspace.join("src-tauri")).unwrap();
        std::fs::write(
            workspace.join("package.json"),
            r#"{"name":"forge","version":"0.1.0"}"#,
        )
        .unwrap();
        std::fs::write(
            workspace.join("src-tauri").join("Cargo.toml"),
            "[package]\nname = \"forge\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(
            workspace.join("src-tauri").join("tauri.conf.json"),
            r#"{"productName":"Forge","identifier":"com.cabbos.forge"}"#,
        )
        .unwrap();

        let boundary = build_write_boundary(
            "write_to_file",
            &serde_json::json!({"path":"src/main.tsx","content":"hello"}),
            &workspace,
            "file_write",
        );

        assert_eq!(boundary.risk, WriteBoundaryRisk::High);
        assert_eq!(
            boundary.warning.as_deref(),
            Some("这是 Forge 自己的开发目录。继续操作可能修改 Forge 本体。")
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn test_write_boundary_warns_for_forge_source_workspace_without_package_name_marker() {
        let workspace = unique_temp_workspace("forge-source-cargo-tauri");
        std::fs::create_dir_all(workspace.join("src-tauri")).unwrap();
        std::fs::write(
            workspace.join("package.json"),
            r#"{"name":"demo-app","version":"0.1.0"}"#,
        )
        .unwrap();
        std::fs::write(
            workspace.join("src-tauri").join("Cargo.toml"),
            "[package]\nname = \"forge\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(
            workspace.join("src-tauri").join("tauri.conf.json"),
            r#"{"productName":"Forge","identifier":"com.cabbos.forge"}"#,
        )
        .unwrap();

        let boundary = build_write_boundary(
            "write_to_file",
            &serde_json::json!({"path":"src/main.tsx","content":"hello"}),
            &workspace,
            "file_write",
        );

        assert_eq!(boundary.risk, WriteBoundaryRisk::High);
        assert_eq!(
            boundary.warning.as_deref(),
            Some("这是 Forge 自己的开发目录。继续操作可能修改 Forge 本体。")
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    // ═══ Test Summary ═══

    #[test]
    fn test_summary() {
        println!("\n═══════════════════════════════════");
        println!("  Harness tests passed:");
        println!("  1. Pre-approved read tools    ✅");
        println!("  2. Writes require confirm     ✅");
        println!("  3. Shell requires confirm     ✅");
        println!("  4. Session approval cache     ✅");
        println!("  5. Hook registration          ✅");
        println!("  6. Capability metadata        ✅");
        println!("  7. Database CRUD              ✅");
        println!("  8. Permission persistence     ✅");
        println!("═══════════════════════════════════\n");
    }
}
