use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::adapters::base::AiAdapter;
use crate::adapters::missing_key::MissingKeyAdapter;
use crate::agent::capability_context::build_turn_input_intent;
use crate::agent::context_builder::ContextSourceKind;
use crate::agent::prepared_turn::{ContextUsageBucketKind, PreparedTurnMemoryAudit};
use crate::agent::session::AgentSession;
use crate::continuity::{
    ContinuityEvent, ContinuityStore, ExperienceStatus, ReflectionEvent, ReflectionOutcome,
};
use crate::forge_wiki::model::{ForgeWikiPageKind, SelectedForgeWikiPage};
use crate::harness::capability::CapabilityKind;
use crate::harness::permissions::PermissionMode;
use crate::harness::Harness;
use crate::ipc::project_records::{
    propose_send_input_project_record_update, select_send_input_project_records_context,
};
use crate::ipc::send_input_context::{
    capability_names_by_kind, prepare_send_input_turn_context, record_send_input_user_turn,
    reserve_turn_then_record_user_message, select_send_input_memory_context,
    PrepareSendInputTurnRequest,
};
use crate::memory::facts::{MemoryFactStore, UpsertMemoryFactInput};
use crate::memory::model::{MemoryStatus, WikiMemory};
use crate::memory::storage::now_string as memory_now_string;
use crate::memory::{MemoryCategory, MemoryScope, SelectedContextMemory};
use crate::profile::{ProfileStore, UpsertProfileInput};
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
        selected_memories: Vec::new(),
        selected_memory_audit: Vec::new(),
        memory_recall_plan: None,
        selected_project_records: Vec::new(),
        permission_mode: PermissionMode::ManualConfirm,
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
        selected_memories: Vec::new(),
        selected_memory_audit: Vec::new(),
        memory_recall_plan: None,
        selected_project_records: Vec::new(),
        permission_mode: PermissionMode::ManualConfirm,
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
async fn send_input_prepared_turn_contract_summarizes_sources_without_hidden_bodies() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-prepared-turn-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let session = test_agent_session("session-1", &workspace);
    let input_intent = build_turn_input_intent("/plan continue with records", &[], Vec::new());
    let workflow = classify_workflow_with_command(
        "session-1",
        "/plan continue with records",
        Some("/plan"),
        1,
    );
    let selected_memory = SelectedContextMemory {
        memory_id: "memory_fact:alpha".to_string(),
        title: "Alpha memory".to_string(),
        body: "SECRET MEMORY BODY".to_string(),
        category: MemoryCategory::ProjectFact,
        scope: MemoryScope::Project,
        score: 0.91,
        reason: "Matches current goal".to_string(),
        injected: true,
    };
    let selected_project_record = SelectedForgeWikiPage {
        page_id: "tasks.md".to_string(),
        title: "Tasks".to_string(),
        path: ".forge/wiki/tasks.md".to_string(),
        kind: ForgeWikiPageKind::Tasks,
        summary: "Task summary".to_string(),
        score: 0.88,
        reason: "Current task record".to_string(),
        injected: true,
    };
    let selected_memory_audit = PreparedTurnMemoryAudit {
        memory_id: "memory_fact:alpha".to_string(),
        source: "memory_fact".to_string(),
        source_id: "alpha".to_string(),
        kind: "project_fact".to_string(),
        score: 0.91,
        reason: "Matches current goal".to_string(),
        project_match: true,
        profile_match: false,
        injected: true,
    };

    let prepared = prepare_send_input_turn_context(PrepareSendInputTurnRequest {
        session_id: "session-1",
        session: &session,
        text: "/plan continue with records",
        input_intent,
        workflow: &workflow,
        ready_connector_labels: Vec::new(),
        memory_context: Some("SECRET MEMORY BODY".to_string()),
        wiki_context: Some("SECRET PROJECT RECORD BODY".to_string()),
        continuity_context: None,
        connector_context: None,
        selected_memories: vec![selected_memory.clone(), selected_memory],
        selected_memory_audit: vec![selected_memory_audit.clone(), selected_memory_audit],
        memory_recall_plan: None,
        selected_project_records: vec![selected_project_record.clone(), selected_project_record],
        permission_mode: PermissionMode::TrustCurrentProject,
    })
    .await;

    assert_eq!(
        prepared.prepared_turn.selected_memory_ids,
        vec!["memory_fact:alpha"]
    );
    assert_eq!(
        prepared.prepared_turn.selected_memory_audit.len(),
        1,
        "audit keeps one entry per selected unified memory"
    );
    assert_eq!(
        prepared.prepared_turn.selected_memory_audit[0].source,
        "memory_fact"
    );
    assert_eq!(
        prepared.prepared_turn.selected_memory_audit[0].source_id,
        "alpha"
    );
    assert_eq!(
        prepared.prepared_turn.selected_memory_audit[0].kind,
        "project_fact"
    );
    assert_eq!(
        prepared.prepared_turn.selected_memory_audit[0].reason,
        "Matches current goal"
    );
    assert!(prepared.prepared_turn.selected_memory_audit[0].project_match);
    assert!(prepared.prepared_turn.selected_memory_audit[0].injected);
    assert_eq!(
        prepared.prepared_turn.selected_project_record_ids,
        vec!["tasks.md"]
    );
    assert_eq!(
        prepared.prepared_turn.permission_mode,
        PermissionMode::TrustCurrentProject
    );
    assert_eq!(
        prepared
            .prepared_turn
            .context_estimate
            .context_window_tokens,
        Some(128_000)
    );
    assert!(prepared.prepared_turn.context_estimate.used_tokens > 0);
    assert_eq!(
        prepared
            .prepared_turn
            .context_estimate
            .sources
            .iter()
            .filter(|source| source.kind == "project_records")
            .count(),
        1
    );
    assert!(prepared
        .prepared_turn
        .context_estimate
        .sources
        .iter()
        .any(|source| source.kind == "user_input" && source.estimated_tokens > 0));
    let context_buckets = &prepared.prepared_turn.context_estimate.buckets;
    assert_eq!(
        context_buckets
            .iter()
            .filter(|bucket| bucket.kind == ContextUsageBucketKind::VisibleInput)
            .count(),
        1
    );
    assert!(
        context_buckets
            .iter()
            .any(|bucket| bucket.kind == ContextUsageBucketKind::Memory
                && bucket.estimated_tokens > 0)
    );
    assert!(context_buckets.iter().any(|bucket| bucket.kind
        == ContextUsageBucketKind::ProjectRecords
        && bucket.estimated_tokens > 0));
    assert!(context_buckets
        .iter()
        .any(|bucket| bucket.kind == ContextUsageBucketKind::HiddenSystem
            && bucket.estimated_tokens > 0));
    assert!(context_buckets.iter().any(|bucket| bucket.kind
        == ContextUsageBucketKind::ReservedOutput
        && bucket.estimated_tokens == 20_000));
    let non_reserved_bucket_tokens = context_buckets
        .iter()
        .filter(|bucket| bucket.kind != ContextUsageBucketKind::ReservedOutput)
        .map(|bucket| bucket.estimated_tokens)
        .sum::<u32>();
    assert_eq!(
        non_reserved_bucket_tokens,
        prepared.prepared_turn.context_estimate.used_tokens
    );

    let event = crate::protocol::events::StreamEvent::TurnPrepared {
        session_id: "session-1".to_string(),
        prepared: prepared.prepared_turn.clone(),
    };
    let json = serde_json::to_string(&event).expect("serialize turn_prepared");
    assert!(json.contains("\"event_type\":\"turn_prepared\""));
    assert!(json.contains("\"permission_mode\":\"trust_current_project\""));
    assert!(json.contains("\"selected_memory_ids\":[\"memory_fact:alpha\"]"));
    assert!(json.contains("\"selected_memory_audit\":["));
    assert!(json.contains("\"source\":\"memory_fact\""));
    assert!(json.contains("\"source_id\":\"alpha\""));
    assert!(json.contains("\"kind\":\"project_fact\""));
    assert!(json.contains("\"project_match\":true"));
    assert!(!json.contains("SECRET MEMORY BODY"));
    assert!(!json.contains("SECRET PROJECT RECORD BODY"));

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
async fn send_input_memory_recall_plan_reports_budget_without_exposing_body() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-send-memory-plan-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let workspace = workspace.canonicalize().expect("canonical workspace");
    let memory_path = std::env::temp_dir().join(format!("forge-send-memory-plan-{nonce}.json"));
    let mut app_state = AppState::new(Arc::new(Harness::new(workspace.clone())));
    app_state.wiki_memory = Arc::new(crate::memory::WikiMemoryStore::new(memory_path.clone()));
    let state = Arc::new(app_state);
    let project_path = workspace.to_string_lossy().to_string();
    state
        .wiki_memory
        .upsert_candidate(test_project_memory(
            "planned-memory",
            "permission confirmation plan",
            "SECRET MEMORY BODY SHOULD ONLY BE HIDDEN CONTEXT",
            &project_path,
        ))
        .await
        .expect("insert memory");

    let selected =
        select_send_input_memory_context(&state, "permission confirmation plan", &project_path)
            .await;
    let recall_plan = selected.recall_plan.clone().expect("recall plan");
    assert_eq!(recall_plan.budget.candidate_count, 1);
    assert_eq!(recall_plan.budget.injected_count, 1);
    assert_eq!(
        recall_plan.selected_memory_ids,
        vec!["wiki_memory:planned-memory"]
    );
    assert!(recall_plan.budget.estimated_injected_tokens > 0);
    assert_eq!(
        recall_plan.candidates[0].decision,
        crate::memory::RecallDecision::Injected
    );
    let recall_json = serde_json::to_string(&recall_plan).expect("serialize recall plan");
    assert!(recall_json.contains("\"estimated_injected_tokens\""));
    assert!(!recall_json.contains("SECRET MEMORY BODY"));

    let session = test_agent_session("session-memory-plan", &workspace);
    let workflow = classify_workflow_with_command(
        "session-memory-plan",
        "permission confirmation plan",
        None,
        1772582400000,
    );
    let input_intent = build_turn_input_intent("permission confirmation plan", &[], Vec::new());
    let prepared = prepare_send_input_turn_context(PrepareSendInputTurnRequest {
        session_id: "session-memory-plan",
        session: &session,
        text: "permission confirmation plan",
        input_intent,
        workflow: &workflow,
        ready_connector_labels: Vec::new(),
        memory_context: selected.context,
        wiki_context: None,
        continuity_context: None,
        connector_context: None,
        selected_memories: selected.selected,
        selected_memory_audit: selected.audit,
        memory_recall_plan: selected.recall_plan,
        selected_project_records: Vec::new(),
        permission_mode: PermissionMode::ManualConfirm,
    })
    .await;

    assert!(prepared.prepared_turn.memory_recall_plan.is_some());
    let event = crate::protocol::events::StreamEvent::TurnPrepared {
        session_id: "session-memory-plan".to_string(),
        prepared: prepared.prepared_turn,
    };
    let json = serde_json::to_string(&event).expect("serialize turn_prepared");
    assert!(json.contains("\"memory_recall_plan\""));
    assert!(json.contains("\"budget\""));
    assert!(!json.contains("SECRET MEMORY BODY"));

    let _ = std::fs::remove_dir_all(workspace);
    let _ = std::fs::remove_file(memory_path);
}

#[tokio::test]
async fn send_input_memory_selection_includes_active_profile_and_global_facts() {
    let nonce = uuid::Uuid::now_v7();
    let session_workspace = std::env::temp_dir().join(format!("forge-send-memory-facts-{nonce}"));
    std::fs::create_dir_all(&session_workspace).expect("session workspace");
    let memory_path = std::env::temp_dir().join(format!("forge-send-memory-facts-{nonce}.json"));
    let facts_path =
        std::env::temp_dir().join(format!("forge-send-memory-facts-store-{nonce}.json"));
    let profiles_path =
        std::env::temp_dir().join(format!("forge-send-memory-facts-profiles-{nonce}.json"));
    let mut app_state = AppState::new(Arc::new(Harness::new(session_workspace.clone())));
    app_state.wiki_memory = Arc::new(crate::memory::WikiMemoryStore::new(memory_path.clone()));
    app_state.memory_facts = Arc::new(MemoryFactStore::new(facts_path.clone()));
    app_state.profiles = Arc::new(ProfileStore::new(profiles_path.clone()));
    let state = Arc::new(app_state);
    let work_profile = state
        .profiles
        .upsert(UpsertProfileInput {
            id: Some("work".to_string()),
            name: "Work".to_string(),
            default_provider: None,
            default_model: None,
            default_workspace: None,
        })
        .expect("work profile");
    state
        .profiles
        .set_active(&work_profile.id)
        .expect("set active profile");
    state
        .wiki_memory
        .upsert_candidate(test_project_memory(
            "wiki-memory",
            "gateway queue wiki progress",
            "wiki memory for gateway queue work",
            session_workspace.to_str().expect("utf8"),
        ))
        .await
        .expect("insert wiki memory");
    state
        .memory_facts
        .upsert(UpsertMemoryFactInput {
            id: Some("active-fact".to_string()),
            text: "gateway queue replay metadata lives in diagnostics".to_string(),
            tags: vec!["decision".to_string()],
            profile_id: Some("work".to_string()),
            source: Some("settings".to_string()),
        })
        .expect("active fact");
    state
        .memory_facts
        .upsert(UpsertMemoryFactInput {
            id: Some("global-fact".to_string()),
            text: "gateway queue trigger smoke uses TCP JSON lines".to_string(),
            tags: vec!["project".to_string()],
            profile_id: None,
            source: Some("settings".to_string()),
        })
        .expect("global fact");
    state
        .memory_facts
        .upsert(UpsertMemoryFactInput {
            id: Some("other-profile-fact".to_string()),
            text: "gateway queue private note from another profile".to_string(),
            tags: vec!["project".to_string()],
            profile_id: Some("personal".to_string()),
            source: Some("settings".to_string()),
        })
        .expect("other profile fact");

    let selected = select_send_input_memory_context(
        &state,
        "gateway queue",
        session_workspace.to_str().expect("utf8"),
    )
    .await;

    let selected_ids = selected
        .selected
        .iter()
        .map(|memory| memory.memory_id.as_str())
        .collect::<Vec<_>>();
    assert!(selected_ids.contains(&"wiki_memory:wiki-memory"));
    assert!(selected_ids.contains(&"memory_fact:active-fact"));
    assert!(selected_ids.contains(&"memory_fact:global-fact"));
    assert!(!selected_ids.contains(&"memory_fact:other-profile-fact"));
    let context = selected.context.expect("memory context");
    assert!(context.contains("gateway queue replay metadata"));
    assert!(context.contains("gateway queue trigger smoke"));
    assert!(!context.contains("private note from another profile"));

    let _ = std::fs::remove_dir_all(session_workspace);
    let _ = std::fs::remove_file(memory_path);
    let _ = std::fs::remove_file(facts_path);
    let _ = std::fs::remove_file(profiles_path);
}

#[tokio::test]
async fn send_input_memory_selection_includes_accepted_continuity_experience() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-send-memory-continuity-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let workspace = workspace.canonicalize().expect("canonical workspace");
    let app_state = AppState::new(Arc::new(Harness::new(workspace.clone())));
    let state = Arc::new(app_state);
    let project_path = workspace.to_string_lossy().to_string();
    let reflection = ReflectionEvent {
        session_id: "session-1".to_string(),
        user_goal: "Fix permission confirmation card".to_string(),
        execution_summary: "Accepted continuity lesson for permission confirmation cards."
            .to_string(),
        outcome: ReflectionOutcome::Completed,
        verification_summary: Some("unit test passed".to_string()),
        lessons: vec![
            "Permission confirmation card fix requires unified memory recall.".to_string(),
        ],
        episode: None,
        timestamp_ms: 1772582400000,
    };

    state
        .continuity
        .record_event(&project_path, &ContinuityEvent::Reflection(reflection))
        .expect("record reflection");
    let formed = state
        .continuity
        .form_experiences_for_session(&project_path, "session-1", 1772582400001)
        .expect("form experiences");
    state
        .continuity
        .update_experience_status(
            &project_path,
            &formed[0].id,
            ExperienceStatus::Accepted,
            Some("review-session"),
            1772582400002,
        )
        .expect("accept experience");

    let selected =
        select_send_input_memory_context(&state, "permission confirmation card fix", &project_path)
            .await;

    let selected_ids = selected
        .selected
        .iter()
        .map(|memory| memory.memory_id.as_str())
        .collect::<Vec<_>>();
    assert!(selected_ids
        .iter()
        .any(|id| id.starts_with("continuity_experience:")));
    let context = selected.context.expect("memory context");
    assert!(context.contains("## Work Memory"));
    assert!(context.contains("continuity_experience/lesson"));
    assert!(context.contains("Permission confirmation card fix"));

    let _ = std::fs::remove_dir_all(workspace);
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
