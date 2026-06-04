use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::adapters::base::AiAdapter;
use crate::adapters::missing_key::MissingKeyAdapter;
use crate::agent::capability_context::build_turn_input_intent;
use crate::agent::context_builder::ContextSourceKind;
use crate::agent::session::AgentSession;
use crate::continuity::ContinuityStore;
use crate::harness::capability::CapabilityKind;
use crate::harness::Harness;
use crate::ipc::project_records::{
    propose_send_input_project_record_update, select_send_input_project_records_context,
};
use crate::ipc::send_input_context::{
    capability_names_by_kind, prepare_send_input_turn_context, record_send_input_user_turn,
    reserve_turn_then_record_user_message, select_send_input_memory_context,
    PrepareSendInputTurnRequest,
};
use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus, WikiMemory};
use crate::memory::storage::now_string as memory_now_string;
use crate::state::AppState;
use crate::workflow::classify_workflow_with_command;

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

#[test]
fn turn_capability_names_omit_internal_infrastructure() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-turn-capabilities-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let harness = Harness::new(workspace.clone());

    let skills = capability_names_by_kind(&harness, CapabilityKind::Skill);
    let hooks = capability_names_by_kind(&harness, CapabilityKind::Hook);

    assert!(!skills.iter().any(|name| name == "Skill Loader"));
    assert!(!hooks.iter().any(|name| name == "Logging Hook"));
    assert!(!hooks.iter().any(|name| name == "File System Audit Hook"));
    assert!(hooks.iter().any(|name| name == "Sensitive Content Guard"));
    assert!(hooks.iter().any(|name| name == "Workspace Boundary Guard"));

    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn busy_session_does_not_record_user_message_before_turn_reservation() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-busy-turn-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        None,
    );
    let _active_turn = session.reserve_turn().expect("first turn should reserve");
    let mut recorded = Vec::new();

    let error = reserve_turn_then_record_user_message(&session, "session-1", "继续", |event| {
        recorded.push(event)
    })
    .expect_err("busy session should reject before recording");

    assert!(error.contains("上一条请求"));
    assert!(recorded.is_empty());

    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn busy_session_does_not_record_continuity_user_message_before_turn_reservation() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-busy-continuity-turn-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        None,
    );
    let state = Arc::new(AppState::new(Arc::new(Harness::new(workspace.clone()))));
    let _active_turn = session.reserve_turn().expect("first turn should reserve");
    let project_path = workspace.to_string_lossy().to_string();

    let error = record_send_input_user_turn(&state, &session, "session-1", "继续", &project_path)
        .expect_err("busy session should reject before recording");

    assert!(error.contains("上一条请求"));
    let events = ContinuityStore::open(workspace.join(".forge").join("continuity.db"))
        .expect("open continuity store")
        .list_events_for_session(&project_path, "session-1")
        .expect("list continuity events");
    assert!(events.is_empty());

    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn stopped_session_does_not_record_user_message_before_turn_reservation() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-stopped-turn-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let adapter =
        Arc::new(MissingKeyAdapter::new("DeepSeek", "deepseek-chat")) as Arc<dyn AiAdapter>;
    let session = AgentSession::new(
        "session-1".to_string(),
        "deepseek".to_string(),
        adapter,
        Arc::new(Harness::new(workspace.clone())),
        "system".to_string(),
        None,
    );
    session.running.store(false, Ordering::SeqCst);
    let mut recorded = Vec::new();

    let error = reserve_turn_then_record_user_message(&session, "session-1", "继续", |event| {
        recorded.push(event)
    })
    .expect_err("stopped session should reject before recording");

    assert!(error.contains("Session is not running"));
    assert!(recorded.is_empty());

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn send_input_turn_context_uses_session_workspace_for_metadata_and_file_references() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace =
        std::env::temp_dir().join(format!("forge-send-session-workspace-{nonce}"));
    let default_workspace =
        std::env::temp_dir().join(format!("forge-send-default-workspace-{nonce}"));
    std::fs::create_dir_all(session_workspace.join("src")).expect("session workspace");
    std::fs::create_dir_all(default_workspace.join("src")).expect("default workspace");
    std::fs::write(
        session_workspace.join("src/app.ts"),
        "export const source = 'session workspace';",
    )
    .expect("session file");
    std::fs::write(
        default_workspace.join("src/app.ts"),
        "export const source = 'default workspace';",
    )
    .expect("default file");
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
        .register_session("session-1".to_string(), session.clone())
        .await;
    let input_intent = build_turn_input_intent("请检查 @src/app.ts", &[], Vec::new());
    let workflow = classify_workflow_with_command("session-1", "请检查 @src/app.ts", None, 1);

    let prepared = prepare_send_input_turn_context(PrepareSendInputTurnRequest {
        session_id: "session-1",
        session: &session,
        text: "请检查 @src/app.ts",
        input_intent,
        workflow: &workflow,
        ready_connector_labels: Vec::new(),
        memory_context: None,
        wiki_context: None,
        continuity_context: None,
        connector_context: None,
    })
    .await;

    assert_eq!(
        std::path::PathBuf::from(&prepared.turn_metadata.workspace_path)
            .canonicalize()
            .expect("prepared workspace"),
        session_workspace.canonicalize().expect("session workspace")
    );
    assert_eq!(
        prepared.turn_metadata.input_intent.file_references,
        vec!["src/app.ts"]
    );
    let selected_files = prepared
        .hidden_contexts
        .iter()
        .find(|context| context.kind == ContextSourceKind::SelectedFiles)
        .expect("selected file context");
    assert!(selected_files.content.contains("session workspace"));
    assert!(!selected_files.content.contains("default workspace"));

    let _ = std::fs::remove_dir_all(session_workspace);
    let _ = std::fs::remove_dir_all(default_workspace);
}

#[tokio::test]
async fn send_input_turn_context_includes_continuity_experience_context() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-send-continuity-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let session = test_agent_session("session-1", &workspace);
    let input_intent = build_turn_input_intent("继续 package script 测试", &[], Vec::new());
    let workflow = classify_workflow_with_command("session-1", "继续 package script 测试", None, 1);

    let prepared = prepare_send_input_turn_context(PrepareSendInputTurnRequest {
        session_id: "session-1",
        session: &session,
        text: "继续 package script 测试",
        input_intent,
        workflow: &workflow,
        ready_connector_labels: Vec::new(),
        memory_context: None,
        wiki_context: None,
        continuity_context: Some(
            "Continuity Experience:\n- [pinned] Package script changes require npm test."
                .to_string(),
        ),
        connector_context: None,
    })
    .await;

    let continuity = prepared
        .hidden_contexts
        .iter()
        .find(|context| context.kind == ContextSourceKind::ContinuityExperience)
        .expect("continuity context");
    assert!(continuity.content.contains("[pinned] Package script"));

    let _ = std::fs::remove_dir_all(&workspace);
}

#[tokio::test]
async fn send_input_memory_selection_uses_session_workspace_over_default_harness() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace = std::env::temp_dir().join(format!("forge-send-memory-session-{nonce}"));
    let default_workspace = std::env::temp_dir().join(format!("forge-send-memory-default-{nonce}"));
    std::fs::create_dir_all(&session_workspace).expect("session workspace");
    std::fs::create_dir_all(&default_workspace).expect("default workspace");
    let memory_path = std::env::temp_dir().join(format!("forge-send-memory-{nonce}.json"));
    let mut app_state = AppState::new(Arc::new(Harness::new(default_workspace.clone())));
    app_state.wiki_memory = Arc::new(crate::memory::WikiMemoryStore::new(memory_path.clone()));
    let state = Arc::new(app_state);
    state
        .wiki_memory
        .upsert_candidate(test_project_memory(
            "session-memory",
            "session workspace progress",
            "只属于当前 session 项目的进度",
            session_workspace.to_str().expect("utf8"),
        ))
        .await
        .expect("insert session memory");
    state
        .wiki_memory
        .upsert_candidate(test_project_memory(
            "default-memory",
            "default workspace progress",
            "不应该被当前会话带入",
            default_workspace.to_str().expect("utf8"),
        ))
        .await
        .expect("insert default memory");

    let selected = select_send_input_memory_context(
        &state,
        "继续处理当前项目进度",
        session_workspace.to_str().expect("utf8"),
    )
    .await;

    assert!(selected
        .context
        .as_deref()
        .is_some_and(|context| context.contains("session workspace progress")));
    assert!(!selected
        .context
        .as_deref()
        .unwrap_or("")
        .contains("default workspace"));

    let _ = std::fs::remove_dir_all(session_workspace);
    let _ = std::fs::remove_dir_all(default_workspace);
    let _ = std::fs::remove_file(memory_path);
}

#[tokio::test]
async fn send_input_project_records_selection_uses_session_workspace_over_default_harness() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace = std::env::temp_dir().join(format!("forge-send-wiki-session-{nonce}"));
    let default_workspace = std::env::temp_dir().join(format!("forge-send-wiki-default-{nonce}"));
    std::fs::create_dir_all(&session_workspace).expect("session workspace");
    std::fs::create_dir_all(&default_workspace).expect("default workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        default_workspace.clone(),
    ))));
    state
        .forge_wiki
        .init(session_workspace.to_str().expect("utf8"))
        .await
        .expect("init session project records");
    state
        .forge_wiki
        .init(default_workspace.to_str().expect("utf8"))
        .await
        .expect("init default project records");
    std::fs::write(
        session_workspace.join(".forge/wiki/tasks.md"),
        "# 当前任务\n\nsession workspace project records",
    )
    .expect("write session records");
    std::fs::write(
        default_workspace.join(".forge/wiki/tasks.md"),
        "# 当前任务\n\ndefault workspace project records",
    )
    .expect("write default records");

    let selected = select_send_input_project_records_context(
        &state,
        "继续当前项目",
        session_workspace.to_str().expect("utf8"),
    )
    .await;

    assert!(selected
        .context
        .as_deref()
        .is_some_and(|context| context.contains("session workspace project records")));
    assert!(!selected
        .context
        .as_deref()
        .unwrap_or("")
        .contains("default workspace"));

    let _ = std::fs::remove_dir_all(session_workspace);
    let _ = std::fs::remove_dir_all(default_workspace);
}

#[tokio::test]
async fn send_input_project_record_writeback_uses_session_workspace_over_default_harness() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace =
        std::env::temp_dir().join(format!("forge-send-writeback-session-{nonce}"));
    let default_workspace =
        std::env::temp_dir().join(format!("forge-send-writeback-default-{nonce}"));
    std::fs::create_dir_all(&session_workspace).expect("session workspace");
    std::fs::create_dir_all(&default_workspace).expect("default workspace");
    let state = Arc::new(AppState::new(Arc::new(Harness::new(
        default_workspace.clone(),
    ))));
    state
        .forge_wiki
        .init(session_workspace.to_str().expect("utf8"))
        .await
        .expect("init session project records");
    state
        .forge_wiki
        .init(default_workspace.to_str().expect("utf8"))
        .await
        .expect("init default project records");
    let user_text = "新增下一步计划：session workspace writeback marker";
    let workflow = classify_workflow_with_command("session-1", user_text, None, 1);

    let writeback = propose_send_input_project_record_update(
        &state,
        "session-1",
        user_text,
        session_workspace.to_str().expect("utf8"),
        &workflow,
        None,
    )
    .await;

    assert!(writeback.record_evidence.is_some());
    assert!(writeback.proposal.is_some());
    let session_proposals =
        std::fs::read_to_string(session_workspace.join(".forge/wiki/.proposals.json"))
            .expect("session proposals");
    let default_proposals =
        std::fs::read_to_string(default_workspace.join(".forge/wiki/.proposals.json"))
            .unwrap_or_default();
    assert!(session_proposals.contains("session workspace writeback marker"));
    assert!(!default_proposals.contains("session workspace writeback marker"));

    let _ = std::fs::remove_dir_all(session_workspace);
    let _ = std::fs::remove_dir_all(default_workspace);
}
