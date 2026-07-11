use super::*;
use crate::adapters::base::{AiAdapter, ChatMessage};
use crate::adapters::missing_key::MissingKeyAdapter;
use crate::agent::session::AgentSession;
use crate::agent::turn_state::{
    AgentToolCategory, AgentToolStatus, AgentToolTrace, AgentTurnState, AgentTurnStatus,
};
use crate::continuity::{
    build_send_input_reflection_event, continuity_events_from_turn, continuity_lessons_from_turn,
    ContinuityEvent, FileOperation, ReflectionEvent, ReflectionOutcome,
};
use crate::harness::Harness;
use crate::ipc::continuity_experiences::{
    list_continuity_experiences_for_request, search_continuity_experiences_for_request,
};
use crate::ipc::delivery_summary::build_delivery_summary_for_session;
use crate::ipc::file_search::find_files;
use crate::ipc::mcp_context::mcp_context_harness_for_session;
use crate::ipc::open_file::resolve_workspace_file_path;
use crate::ipc::send_input_context::select_send_input_memory_context;
use crate::ipc::workspace_files::{
    open_file_target_for_request, preview_file_for_request, search_workspace_files_for_request,
    working_dir_for_request_or_explicit,
};
use crate::memory::facts::MemoryFactStore;
use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus, WikiMemory};
use crate::memory::storage::now_string as memory_now_string;
use crate::workspace_safety::resolve_optional_workspace_path as resolve_requested_working_dir;

fn test_agent_session(id: &str, workspace: &std::path::Path) -> Arc<AgentSession> {
    Arc::new(AgentSession::new(
        id.to_string(),
        "deepseek".to_string(),
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")),
        Arc::new(Harness::new(workspace.to_path_buf())),
        "system".to_string(),
        Some(128_000),
    ))
}

fn test_project_memory(id: &str, title: &str, body: &str, project_path: &str) -> WikiMemory {
    let now = memory_now_string();
    WikiMemory {
        id: id.to_string(),
        category: MemoryCategory::TaskState,
        scope: MemoryScope::Project,
        status: MemoryStatus::Pinned,
        title: title.to_string(),
        body: body.to_string(),
        project_path: Some(project_path.to_string()),
        source_session_id: Some("session-1".to_string()),
        source_message_ids: vec!["message-1".to_string()],
        confidence: 1.0,
        created_at: now.clone(),
        updated_at: now,
        last_used_at: None,
        use_count: 0,
        tags: vec!["进度".to_string()],
    }
}

fn seed_compactable_history(session: &AgentSession, count: usize) {
    let mut messages = crate::agent::session_guards::lock_unpoisoned(&session.messages);
    for index in 0..count {
        if index % 2 == 0 {
            messages.push(ChatMessage::user(&format!("user message {index}")));
        } else {
            messages.push(ChatMessage::assistant(serde_json::Value::String(format!(
                "assistant message {index}"
            ))));
        }
    }
}

#[tokio::test]
async fn compact_session_context_for_state_compacts_registered_session() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-ipc-manual-compact-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(workspace.clone()))));
    let session = test_agent_session("session-compact", &workspace);
    seed_compactable_history(&session, 40);
    state
        .register_session("session-compact".to_string(), session.clone())
        .await;
    let emitter = crate::agent::event_sink::CollectingEventEmitter::new();

    let result = compact_session_context_for_state(&state, "session-compact", &emitter)
        .await
        .expect("manual compact should succeed");

    assert!(result.compacted);
    assert_eq!(result.compacted_messages, 8);
    assert_eq!(
        crate::agent::session_guards::lock_unpoisoned(&session.messages).len(),
        32
    );
    assert!(emitter.drain().iter().any(|event| matches!(
        event,
        crate::protocol::events::StreamEvent::ContextCompacted {
            session_id,
            compacted_messages: 8,
            retained_messages: 32,
            ..
        } if session_id == "session-compact"
    )));

    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn manual_compact_request_matches_text_and_structured_capability() {
    assert!(is_manual_compact_request("/compact", &[]));
    assert!(is_manual_compact_request(
        "请按所选动作继续。",
        &[ComposerCapabilitySelection::SlashCommand {
            command: "/compact".to_string(),
        }]
    ));
    assert!(!is_manual_compact_request("/fix", &[]));
}

fn test_profile(
    overrides: impl FnOnce(&mut crate::profile::ForgeProfile),
) -> crate::profile::ForgeProfile {
    let mut profile = crate::profile::ForgeProfile {
        id: "ops".to_string(),
        name: "Ops".to_string(),
        default_provider: Some("anthropic".to_string()),
        default_model: Some("claude-sonnet-4-5".to_string()),
        default_workspace: Some("/Users/cabbos/project/ops".to_string()),
        credential_overrides: Default::default(),
        created_at_ms: 1,
        updated_at_ms: 1,
    };
    overrides(&mut profile);
    profile
}

#[test]
fn create_session_defaults_use_profile_provider_model_and_workspace() {
    let profile = test_profile(|_| {});

    let defaults = resolve_create_session_defaults(
        "/Users/cabbos/project/forge".to_string(),
        Some("deepseek".to_string()),
        Some("deepseek-chat".to_string()),
        Some(&profile),
    );

    assert_eq!(defaults.working_dir, "/Users/cabbos/project/ops");
    assert_eq!(defaults.provider.as_deref(), Some("anthropic"));
    assert_eq!(defaults.model.as_deref(), Some("claude-sonnet-4-5"));
}

#[test]
fn create_session_defaults_fall_back_to_request_without_profile() {
    let defaults = resolve_create_session_defaults(
        "/Users/cabbos/project/forge".to_string(),
        Some("deepseek".to_string()),
        Some("deepseek-chat".to_string()),
        None,
    );

    assert_eq!(defaults.working_dir, "/Users/cabbos/project/forge");
    assert_eq!(defaults.provider.as_deref(), Some("deepseek"));
    assert_eq!(defaults.model.as_deref(), Some("deepseek-chat"));
}

#[test]
fn create_session_defaults_ignore_blank_profile_fields() {
    let profile = test_profile(|profile| {
        profile.default_provider = Some("  ".to_string());
        profile.default_model = Some(String::new());
        profile.default_workspace = Some("   ".to_string());
    });

    let defaults = resolve_create_session_defaults(
        "/workspace".to_string(),
        Some("openai".to_string()),
        Some("gpt-5-codex".to_string()),
        Some(&profile),
    );

    assert_eq!(defaults.working_dir, "/workspace");
    assert_eq!(defaults.provider.as_deref(), Some("openai"));
    assert_eq!(defaults.model.as_deref(), Some("gpt-5-codex"));
}

fn record_test_continuity_lesson(
    state: &Arc<AppState>,
    project_path: &std::path::Path,
    session_id: &str,
    lesson: &str,
    timestamp_ms: u64,
) {
    let project_path = project_path.to_string_lossy().to_string();
    let reflection = ContinuityEvent::Reflection(ReflectionEvent {
        session_id: session_id.to_string(),
        user_goal: "continue continuity".to_string(),
        execution_summary: "test reflection".to_string(),
        outcome: ReflectionOutcome::Completed,
        verification_summary: Some("test passed".to_string()),
        lessons: vec![lesson.to_string()],
        episode: None,
        timestamp_ms,
    });
    state
        .continuity
        .record_event(&project_path, &reflection)
        .expect("record continuity event");
    state
        .continuity
        .form_experiences_for_session(&project_path, session_id, timestamp_ms + 1)
        .expect("form continuity experiences");
}

#[tokio::test]
async fn list_continuity_experiences_uses_session_workspace() {
    let nonce = uuid::Uuid::now_v7();
    let default_workspace = std::env::temp_dir().join(format!("forge-continuity-default-{nonce}"));
    let session_workspace = std::env::temp_dir().join(format!("forge-continuity-session-{nonce}"));
    std::fs::create_dir_all(&default_workspace).expect("default workspace");
    std::fs::create_dir_all(&session_workspace).expect("session workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        default_workspace.clone(),
    ))));
    state
        .register_session(
            "session-1".to_string(),
            test_agent_session("session-1", &session_workspace),
        )
        .await;
    record_test_continuity_lesson(
        &state,
        &default_workspace,
        "default-session",
        "Default workspace reflection should stay isolated.",
        10,
    );
    record_test_continuity_lesson(
        &state,
        &session_workspace,
        "session-1",
        "Session workspace reflection should be listed.",
        20,
    );

    let experiences = list_continuity_experiences_for_request(&state, Some("session-1"), None)
        .await
        .expect("list continuity experiences");

    assert_eq!(experiences.len(), 1);
    assert_eq!(
        experiences[0].project_path.as_deref(),
        Some(session_workspace.to_string_lossy().as_ref())
    );
    assert_eq!(
        experiences[0].body,
        "Session workspace reflection should be listed."
    );

    let _ = std::fs::remove_dir_all(default_workspace);
    let _ = std::fs::remove_dir_all(session_workspace);
}

#[tokio::test]
async fn search_continuity_experiences_uses_session_workspace() {
    let nonce = uuid::Uuid::now_v7();
    let default_workspace =
        std::env::temp_dir().join(format!("forge-continuity-search-default-{nonce}"));
    let session_workspace =
        std::env::temp_dir().join(format!("forge-continuity-search-session-{nonce}"));
    std::fs::create_dir_all(&default_workspace).expect("default workspace");
    std::fs::create_dir_all(&session_workspace).expect("session workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        default_workspace.clone(),
    ))));
    state
        .register_session(
            "session-1".to_string(),
            test_agent_session("session-1", &session_workspace),
        )
        .await;
    record_test_continuity_lesson(
        &state,
        &default_workspace,
        "default-session",
        "Reflection in the default workspace must not leak.",
        10,
    );
    record_test_continuity_lesson(
        &state,
        &session_workspace,
        "session-1",
        "Reflection in the session workspace should be searchable.",
        20,
    );

    let experiences = search_continuity_experiences_for_request(
        &state,
        Some("session-1"),
        None,
        "reflection searchable",
        Some(5),
    )
    .await
    .expect("search continuity experiences");

    assert_eq!(experiences.len(), 1);
    assert_eq!(
        experiences[0].project_path.as_deref(),
        Some(session_workspace.to_string_lossy().as_ref())
    );
    assert_eq!(
        experiences[0].body,
        "Reflection in the session workspace should be searchable."
    );

    let _ = std::fs::remove_dir_all(default_workspace);
    let _ = std::fs::remove_dir_all(session_workspace);
}

#[test]
fn continuity_reflection_does_not_include_memory_candidates_as_lessons() {
    // Memory candidates (user preferences, project facts) must NOT become
    // continuity reflection lessons to avoid prompt-echo pollution.
    let event = build_send_input_reflection_event(
        "session-1",
        "继续经验系统",
        ReflectionOutcome::Completed,
        vec![],
        None,
        42,
    );

    assert_eq!(
        event,
        ContinuityEvent::Reflection(ReflectionEvent {
            session_id: "session-1".to_string(),
            user_goal: "继续经验系统".to_string(),
            execution_summary: "send_input completed successfully".to_string(),
            outcome: ReflectionOutcome::Completed,
            verification_summary: None,
            lessons: vec![],
            episode: None,
            timestamp_ms: 42,
        })
    );
}

#[test]
fn continuity_events_from_turn_include_tools_file_changes_and_assistant_summary() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/repo/forge".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "direct".to_string(),
        "idle".to_string(),
        "Add continuity events".to_string(),
    );
    turn.record_tool(AgentToolTrace {
        tool_call_id: "tool-1".to_string(),
        name: "edit_file".to_string(),
        category: AgentToolCategory::Write,
        status: AgentToolStatus::Completed,
        started_at_ms: 10,
        ended_at_ms: Some(20),
        result_summary: Some("Edited continuity store".to_string()),
        is_error: false,
        affected_files: vec!["src-tauri/src/continuity/store.rs".to_string()],
        command: None,
    });
    turn.record_tool(AgentToolTrace {
        tool_call_id: "tool-2".to_string(),
        name: "bash".to_string(),
        category: AgentToolCategory::Shell,
        status: AgentToolStatus::Failed,
        started_at_ms: 30,
        ended_at_ms: Some(35),
        result_summary: Some("cargo test failed".to_string()),
        is_error: true,
        affected_files: Vec::new(),
        command: Some("cargo test continuity".to_string()),
    });
    turn.mark_status(AgentTurnStatus::Completed);
    turn.updated_at_ms = 50;

    let events = continuity_events_from_turn(&turn);

    assert_eq!(events.len(), 4);
    assert_eq!(
        events[0],
        ContinuityEvent::ToolExecution {
            session_id: "session-1".to_string(),
            tool_name: "edit_file".to_string(),
            input_summary: "files=src-tauri/src/continuity/store.rs".to_string(),
            output_summary: "Edited continuity store".to_string(),
            is_error: false,
            timestamp_ms: 20,
        }
    );
    assert_eq!(
        events[1],
        ContinuityEvent::FileChange {
            session_id: "session-1".to_string(),
            path: "src-tauri/src/continuity/store.rs".to_string(),
            operation: FileOperation::Modified,
            diff_summary: "tool=edit_file; Edited continuity store".to_string(),
            timestamp_ms: 20,
        }
    );
    assert_eq!(
        events[2],
        ContinuityEvent::ToolExecution {
            session_id: "session-1".to_string(),
            tool_name: "bash".to_string(),
            input_summary: "command=cargo test continuity".to_string(),
            output_summary: "cargo test failed".to_string(),
            is_error: true,
            timestamp_ms: 35,
        }
    );
    assert_eq!(
        events[3],
        ContinuityEvent::AssistantResponse {
            session_id: "session-1".to_string(),
            content_summary: "turn_status=completed; tools=2; failed_tools=1".to_string(),
            timestamp_ms: 50,
        }
    );
}

#[test]
fn continuity_lessons_from_turn_capture_failures_conservatively() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/repo/forge".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "direct".to_string(),
        "idle".to_string(),
        "Add continuity FTS recall".to_string(),
    );
    turn.record_tool(AgentToolTrace {
        tool_call_id: "tool-1".to_string(),
        name: "bash".to_string(),
        category: AgentToolCategory::Shell,
        status: AgentToolStatus::Failed,
        started_at_ms: 10,
        ended_at_ms: Some(20),
        result_summary: Some("sqlite error: no such module fts5".to_string()),
        is_error: true,
        affected_files: Vec::new(),
        command: Some("cargo test continuity".to_string()),
    });
    turn.set_verification(crate::agent::turn_state::AgentVerificationTrace {
        status: crate::agent::turn_state::AgentVerificationStatus::Failed,
        command: Some("cargo test continuity".to_string()),
        exit_code: Some(101),
        stdout_preview: None,
        stderr_preview: Some("no such module fts5".to_string()),
        duration_ms: Some(1200),
        completed_at_ms: Some(30),
    });

    let lessons = continuity_lessons_from_turn(&turn);

    assert_eq!(
        lessons,
        vec![
            "Tool `bash` failed: command=cargo test continuity -> sqlite error: no such module fts5",
            "Verification `cargo test continuity` failed: exit_code=101; stderr=no such module fts5",
        ]
    );
}

#[test]
fn continuity_lessons_from_turn_ignore_shell_success_looking_false_errors() {
    let mut turn = AgentTurnState::new(
        "turn-1".to_string(),
        "session-1".to_string(),
        "/repo/forge".to_string(),
        "openai".to_string(),
        "gpt-5".to_string(),
        "direct".to_string(),
        "idle".to_string(),
        "Run final verification".to_string(),
    );
    turn.record_tool(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "run_shell".to_string(),
            category: AgentToolCategory::Shell,
            status: AgentToolStatus::Failed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some(
                "Exit code: -1 Stdout: > continuity-manual-test-app@0.1.0 build > tsc && vite build vite v7.3.5 building client environment for production... transforming... ✓ 30 modules transformed. rendering chunks... computing gzip size... ✓ built in 339ms Stderr:"
                    .to_string(),
            ),
            is_error: true,
            affected_files: Vec::new(),
            command: Some("npm run build".to_string()),
        });
    turn.mark_status(AgentTurnStatus::Completed);

    let lessons = continuity_lessons_from_turn(&turn);

    assert!(lessons.is_empty());
}

#[test]
fn explicit_working_dir_resolves_to_canonical_workspace() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-explicit-workspace-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");

    let resolved = resolve_requested_working_dir(Some(workspace.to_str().expect("utf8")))
        .expect("resolve")
        .expect("explicit workspace");

    assert_eq!(resolved, workspace.canonicalize().expect("canonical"));

    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn explicit_working_dir_rejects_broad_workspace() {
    let result = resolve_requested_working_dir(Some("/"));

    assert!(result.is_err());
}

#[tokio::test]
async fn workspace_bound_request_requires_session_or_explicit_working_dir() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-request-workspace-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(workspace.clone()))));

    let error = working_dir_for_request_or_explicit(&state, None, None)
        .await
        .expect_err("missing workspace should fail");

    assert!(error.contains("工作空间"));

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn workspace_bound_request_uses_session_workspace_over_explicit_working_dir() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace =
        std::env::temp_dir().join(format!("forge-request-session-workspace-{nonce}"));
    let explicit_workspace =
        std::env::temp_dir().join(format!("forge-request-explicit-workspace-{nonce}"));
    std::fs::create_dir_all(&session_workspace).expect("session workspace");
    std::fs::create_dir_all(&explicit_workspace).expect("explicit workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        explicit_workspace.clone(),
    ))));
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = Arc::new(AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(session_workspace.clone())),
        "system".to_string(),
        Some(128_000),
    ));
    state
        .register_session("session-1".to_string(), session)
        .await;

    let resolved = working_dir_for_request_or_explicit(
        &state,
        Some("session-1"),
        Some(explicit_workspace.to_str().expect("utf8")),
    )
    .await
    .expect("session workspace should resolve");

    assert_eq!(
        resolved.canonicalize().expect("resolved workspace"),
        session_workspace.canonicalize().expect("session workspace")
    );
    assert_ne!(
        resolved.canonicalize().expect("resolved workspace"),
        explicit_workspace
            .canonicalize()
            .expect("explicit workspace")
    );

    let _ = std::fs::remove_dir_all(session_workspace);
    let _ = std::fs::remove_dir_all(explicit_workspace);
}

#[tokio::test]
async fn search_workspace_files_uses_session_workspace_over_explicit_working_dir() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace =
        std::env::temp_dir().join(format!("forge-search-session-workspace-{nonce}"));
    let explicit_workspace =
        std::env::temp_dir().join(format!("forge-search-explicit-workspace-{nonce}"));
    std::fs::create_dir_all(session_workspace.join("src")).expect("session workspace");
    std::fs::create_dir_all(explicit_workspace.join("src")).expect("explicit workspace");
    std::fs::write(session_workspace.join("src/session-owned.ts"), "session")
        .expect("session file");
    std::fs::write(explicit_workspace.join("src/explicit-owned.ts"), "explicit")
        .expect("explicit file");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        explicit_workspace.clone(),
    ))));
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = Arc::new(AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(session_workspace.clone())),
        "system".to_string(),
        Some(128_000),
    ));
    state
        .register_session("session-1".to_string(), session)
        .await;

    let results = search_workspace_files_for_request(
        &state,
        "owned",
        Some("session-1"),
        Some(explicit_workspace.to_str().expect("utf8")),
    )
    .await
    .expect("search should use session workspace");

    assert!(results.iter().any(|path| path == "src/session-owned.ts"));
    assert!(!results.iter().any(|path| path == "src/explicit-owned.ts"));

    let _ = std::fs::remove_dir_all(session_workspace);
    let _ = std::fs::remove_dir_all(explicit_workspace);
}

#[tokio::test]
async fn preview_file_uses_session_workspace_over_explicit_working_dir() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace =
        std::env::temp_dir().join(format!("forge-preview-session-workspace-{nonce}"));
    let explicit_workspace =
        std::env::temp_dir().join(format!("forge-preview-explicit-workspace-{nonce}"));
    std::fs::create_dir_all(session_workspace.join("src")).expect("session workspace");
    std::fs::create_dir_all(explicit_workspace.join("src")).expect("explicit workspace");
    std::fs::write(
        session_workspace.join("src/app.ts"),
        "export const source = 'session workspace';",
    )
    .expect("session file");
    std::fs::write(
        explicit_workspace.join("src/app.ts"),
        "export const source = 'explicit workspace';",
    )
    .expect("explicit file");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        explicit_workspace.clone(),
    ))));
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = Arc::new(AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(session_workspace.clone())),
        "system".to_string(),
        Some(128_000),
    ));
    state
        .register_session("session-1".to_string(), session)
        .await;

    let preview = preview_file_for_request(
        &state,
        "src/app.ts",
        None,
        Some(10),
        Some("session-1"),
        Some(explicit_workspace.to_str().expect("utf8")),
    )
    .await
    .expect("preview should use session workspace");

    assert!(preview
        .lines
        .iter()
        .any(|line| line.content.contains("session workspace")));
    assert!(!preview
        .lines
        .iter()
        .any(|line| line.content.contains("explicit workspace")));

    let _ = std::fs::remove_dir_all(session_workspace);
    let _ = std::fs::remove_dir_all(explicit_workspace);
}

#[tokio::test]
async fn open_file_target_uses_session_workspace_over_explicit_working_dir() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace =
        std::env::temp_dir().join(format!("forge-open-session-workspace-{nonce}"));
    let explicit_workspace =
        std::env::temp_dir().join(format!("forge-open-explicit-workspace-{nonce}"));
    std::fs::create_dir_all(session_workspace.join("src")).expect("session workspace");
    std::fs::create_dir_all(explicit_workspace.join("src")).expect("explicit workspace");
    std::fs::write(session_workspace.join("src/app.ts"), "session").expect("session file");
    std::fs::write(explicit_workspace.join("src/app.ts"), "explicit").expect("explicit file");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        explicit_workspace.clone(),
    ))));
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = Arc::new(AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(session_workspace.clone())),
        "system".to_string(),
        Some(128_000),
    ));
    state
        .register_session("session-1".to_string(), session)
        .await;

    let target = open_file_target_for_request(
        &state,
        "src/app.ts",
        Some("session-1"),
        Some(explicit_workspace.to_str().expect("utf8")),
    )
    .await
    .expect("open target should use session workspace");

    assert!(target.starts_with(session_workspace.canonicalize().expect("session workspace")));
    assert!(!target.starts_with(
        explicit_workspace
            .canonicalize()
            .expect("explicit workspace")
    ));

    let _ = std::fs::remove_dir_all(session_workspace);
    let _ = std::fs::remove_dir_all(explicit_workspace);
}

#[tokio::test]
async fn delivery_summary_uses_session_workspace_over_default_harness() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace = std::env::temp_dir().join(format!("forge-delivery-session-{nonce}"));
    let default_workspace = std::env::temp_dir().join(format!("forge-delivery-default-{nonce}"));
    std::fs::create_dir_all(&session_workspace).expect("session workspace");
    std::fs::create_dir_all(&default_workspace).expect("default workspace");
    std::fs::write(
        session_workspace.join("package.json"),
        r#"{"scripts":{"dev":"vite --host 127.0.0.1 --port 59731"}}"#,
    )
    .expect("session package");
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&session_workspace)
        .output()
        .expect("git init session workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        default_workspace.clone(),
    ))));
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = Arc::new(AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(session_workspace.clone())),
        "system".to_string(),
        Some(128_000),
    ));
    state
        .register_session("session-1".to_string(), session)
        .await;

    let built = build_delivery_summary_for_session(&state, "session-1", None, None).await;

    assert_eq!(
        std::path::PathBuf::from(
            built
                .summary
                .project_path
                .as_deref()
                .expect("summary project path")
        )
        .canonicalize()
        .expect("summary workspace"),
        session_workspace.canonicalize().expect("session workspace")
    );
    assert_eq!(built.summary.checkpoint_label, "还没有检查点");
    assert_ne!(
        std::path::PathBuf::from(built.summary.project_path.unwrap())
            .canonicalize()
            .expect("summary workspace"),
        default_workspace.canonicalize().expect("default workspace")
    );

    let _ = std::fs::remove_dir_all(session_workspace);
    let _ = std::fs::remove_dir_all(default_workspace);
}

#[tokio::test]
async fn mcp_context_sources_reject_unknown_session_instead_of_default_harness() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-mcp-default-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(workspace.clone()))));

    let error = mcp_context_harness_for_session(&state, Some("missing-session"))
        .await
        .err()
        .expect("missing session should not use default harness");

    assert!(error.contains("会话"));

    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn mcp_context_sources_use_session_workspace_over_default_harness() {
    let nonce = uuid::Uuid::now_v7();
    let default_workspace =
        std::env::temp_dir().join(format!("forge-mcp-default-workspace-{nonce}"));
    let session_workspace =
        std::env::temp_dir().join(format!("forge-mcp-session-workspace-{nonce}"));
    std::fs::create_dir_all(&default_workspace).expect("default workspace");
    std::fs::create_dir_all(&session_workspace).expect("session workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        default_workspace.clone(),
    ))));
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = Arc::new(AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(session_workspace.clone())),
        "system".to_string(),
        Some(128_000),
    ));
    state
        .register_session("session-1".to_string(), session)
        .await;

    let harness = mcp_context_harness_for_session(&state, Some("session-1"))
        .await
        .expect("session harness lookup")
        .expect("session harness");

    assert_eq!(
        harness.working_dir.canonicalize().expect("session harness"),
        session_workspace.canonicalize().expect("session workspace")
    );
    assert_ne!(
        harness.working_dir.canonicalize().expect("session harness"),
        default_workspace.canonicalize().expect("default workspace")
    );

    let _ = std::fs::remove_dir_all(default_workspace);
    let _ = std::fs::remove_dir_all(session_workspace);
}

#[test]
fn workspace_file_path_rejects_absolute_path_outside_workspace() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-preview-workspace-{nonce}"));
    let outside = std::env::temp_dir().join(format!("forge-preview-outside-{nonce}.txt"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::write(&outside, "outside secret").expect("outside file");

    let error = resolve_workspace_file_path(&workspace, outside.to_str().expect("utf8"))
        .expect_err("absolute path outside workspace should be rejected");

    assert!(error.contains("当前项目"));
    assert!(
        !error.contains(outside.to_str().expect("utf8")),
        "outside absolute path should not be echoed to the UI"
    );

    let _ = std::fs::remove_dir_all(&workspace);
    let _ = std::fs::remove_file(&outside);
}

#[test]
fn workspace_file_search_finds_nested_file_matches() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-file-search-{nonce}"));
    std::fs::create_dir_all(workspace.join("src/components")).expect("workspace");
    std::fs::write(
        workspace.join("src/components/WaterTracker.tsx"),
        "export function WaterTracker() {}",
    )
    .expect("file");

    let results = find_files(&workspace, "water", 20);

    assert_eq!(results, vec!["src/components/WaterTracker.tsx"]);

    let _ = std::fs::remove_dir_all(&workspace);
}

#[cfg(unix)]
#[test]
fn workspace_file_search_skips_symlinked_external_directories() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-file-search-link-{nonce}"));
    let outside = std::env::temp_dir().join(format!("forge-file-search-outside-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&outside).expect("outside");
    std::fs::write(outside.join("ForgeSecret.ts"), "export const secret = 1;")
        .expect("outside file");
    std::os::unix::fs::symlink(&outside, workspace.join("linked-outside")).expect("symlink");

    let results = find_files(&workspace, "ForgeSecret", 20);

    assert!(results.is_empty());

    let _ = std::fs::remove_dir_all(&workspace);
    let _ = std::fs::remove_dir_all(&outside);
}

#[tokio::test]
async fn pending_confirms_multiple_resolved_independently() {
    let pending: Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    > = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
    let (tx_a, rx_a) = tokio::sync::oneshot::channel();
    let (tx_b, rx_b) = tokio::sync::oneshot::channel();
    let (tx_c, rx_c) = tokio::sync::oneshot::channel();
    pending.write().await.insert("block-a".to_string(), tx_a);
    pending.write().await.insert("block-b".to_string(), tx_b);
    pending.write().await.insert("block-c".to_string(), tx_c);
    {
        pending
            .write()
            .await
            .remove("block-a")
            .unwrap()
            .send(true)
            .unwrap();
    }
    assert!(rx_a.await.unwrap());
    {
        pending
            .write()
            .await
            .remove("block-b")
            .unwrap()
            .send(false)
            .unwrap();
    }
    assert!(!rx_b.await.unwrap());
    assert!(pending.read().await.contains_key("block-c"));
    {
        pending
            .write()
            .await
            .remove("block-c")
            .unwrap()
            .send(true)
            .unwrap();
    }
    assert!(rx_c.await.unwrap());
    assert!(pending.read().await.is_empty());
}

#[tokio::test]
async fn pending_confirms_wrong_block_id_returns_none() {
    let pending: Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    > = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
    let (tx, _rx) = tokio::sync::oneshot::channel();
    pending.write().await.insert("block-real".to_string(), tx);
    let result = pending.write().await.remove("block-fake");
    assert!(result.is_none(), "wrong block_id should return None");
    assert!(pending.read().await.contains_key("block-real"));
}

#[tokio::test]
async fn pending_confirms_double_response_fails() {
    let pending: Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    > = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
    let (tx, rx) = tokio::sync::oneshot::channel();
    pending.write().await.insert("block-1".to_string(), tx);
    let sender = pending.write().await.remove("block-1").unwrap();
    assert!(sender.send(true).is_ok());
    assert!(rx.await.unwrap());
    let result = pending.write().await.remove("block-1");
    assert!(result.is_none(), "already resolved confirm should be gone");
}

#[tokio::test]
async fn pending_confirms_cancel_drops_sender_without_response() {
    let pending: Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    > = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
    let (tx, rx) = tokio::sync::oneshot::channel();
    pending.write().await.insert("block-kill".to_string(), tx);
    pending.write().await.remove("block-kill");
    let result = rx.await;
    assert!(result.is_err(), "dropped sender should close the channel");
}

// ── Cross-project memory pollution regression ────────────────────

#[tokio::test]
async fn tomato_clock_global_preference_not_injected_in_different_project_context() {
    // Simulates the original incident: a UserProfile preference with task-like
    // content ("番茄钟") exists in memory. User is now in a different project
    // (forge-backend) and says "继续". The memory must NOT be injected.
    let nonce = uuid::Uuid::now_v7();
    let forge_workspace = std::env::temp_dir().join(format!("forge-regression-{nonce}"));
    std::fs::create_dir_all(&forge_workspace).expect("workspace");
    let memory_path = std::env::temp_dir().join(format!("forge-regression-{nonce}.json"));
    let facts_path = std::env::temp_dir().join(format!("forge-regression-facts-{nonce}.json"));
    let mut app_state = AppState::new(Arc::new(Harness::new(forge_workspace.clone())));
    app_state.wiki_memory = Arc::new(crate::memory::WikiMemoryStore::new(memory_path.clone()));
    app_state.memory_facts = Arc::new(MemoryFactStore::new(facts_path.clone()));

    // Insert the pollution: task-like content stored as UserProfile
    let now = memory_now_string();
    let pollution = WikiMemory {
        id: "tomato-clock-pollution".to_string(),
        category: MemoryCategory::Preference,
        scope: MemoryScope::UserProfile,
        status: MemoryStatus::Accepted,
        title: "用户偏好：我想做一个番茄钟小工具".to_string(),
        body: "我想做一个番茄钟小工具，可以开始、暂停、重置。请优先推进到一个可预览的第一版。"
            .to_string(),
        project_path: None,
        source_session_id: Some("old-session".to_string()),
        source_message_ids: vec![],
        confidence: 0.8,
        created_at: now.clone(),
        updated_at: now,
        last_used_at: Some("old-time".to_string()),
        use_count: 12,
        tags: vec!["preference".to_string()],
    };
    app_state
        .wiki_memory
        .upsert_candidate(pollution)
        .await
        .expect("insert pollution");

    let state = Arc::new(app_state);

    // User says "继续" in the forge-backend project context
    let selected =
        select_send_input_memory_context(&state, "继续", forge_workspace.to_str().expect("utf8"))
            .await;

    let context_text = selected.context.unwrap_or_default();
    assert!(
        !context_text.contains("番茄钟"),
        "番茄钟 must not appear in context for different project, got: {context_text}"
    );

    let _ = std::fs::remove_dir_all(forge_workspace);
    let _ = std::fs::remove_file(memory_path);
    let _ = std::fs::remove_file(facts_path);
}

#[tokio::test]
async fn forgotten_memory_not_injected_via_select_context() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-forget-select-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let memory_path = std::env::temp_dir().join(format!("forge-forget-select-{nonce}.json"));
    let facts_path = std::env::temp_dir().join(format!("forge-forget-facts-{nonce}.json"));
    let mut app_state = AppState::new(Arc::new(Harness::new(workspace.clone())));
    app_state.wiki_memory = Arc::new(crate::memory::WikiMemoryStore::new(memory_path.clone()));
    app_state.memory_facts = Arc::new(MemoryFactStore::new(facts_path.clone()));

    let now = memory_now_string();
    let memory = WikiMemory {
        id: "will-forget".to_string(),
        category: MemoryCategory::Preference,
        scope: MemoryScope::UserProfile,
        status: MemoryStatus::Accepted,
        title: "用户偏好".to_string(),
        body: "以后都用中文回复。".to_string(),
        project_path: None,
        source_session_id: Some("s1".to_string()),
        source_message_ids: vec![],
        confidence: 0.8,
        created_at: now.clone(),
        updated_at: now,
        last_used_at: None,
        use_count: 0,
        tags: vec!["preference".to_string()],
    };
    let state = Arc::new(app_state);
    state
        .wiki_memory
        .upsert_candidate(memory)
        .await
        .expect("insert");

    // Verify it IS injected before forgetting
    let selected_before = select_send_input_memory_context(
        &state,
        "以后回复用中文",
        workspace.to_str().expect("utf8"),
    )
    .await;
    assert!(
        selected_before
            .selected
            .iter()
            .any(|m| m.memory_id == "wiki_memory:will-forget"),
        "memory should be injected before forgetting"
    );

    // Forget it
    state
        .wiki_memory
        .forget("will-forget")
        .await
        .expect("forget");

    // Verify it is NOT injected after forgetting
    let selected_after = select_send_input_memory_context(
        &state,
        "以后回复用中文",
        workspace.to_str().expect("utf8"),
    )
    .await;
    assert!(
        !selected_after
            .selected
            .iter()
            .any(|m| m.memory_id == "wiki_memory:will-forget"),
        "forgotten memory must not be injected"
    );

    let _ = std::fs::remove_dir_all(workspace);
    let _ = std::fs::remove_file(memory_path);
    let _ = std::fs::remove_file(facts_path);
}
