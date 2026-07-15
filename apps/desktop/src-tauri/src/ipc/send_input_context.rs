use std::sync::Arc;

use crate::agent::capability_context::{
    build_turn_input_intent, format_turn_capability_snapshot, ComposerCapabilitySelection,
    TurnCapabilitySnapshot, TurnInputIntent,
};
use crate::agent::context_builder::{ContextSourceKind, HiddenContextPart};
use crate::agent::prepared_turn::{
    build_prepared_turn, PreparedTurn, PreparedTurnBuildRequest, PreparedTurnMemoryAudit,
};
use crate::agent::session::{AgentSession, TurnInflightGuard};
use crate::agent::time::now_ms;
use crate::agent::turn_state::{AgentTurnInputIntent, AgentTurnMetadata};
use crate::continuity::form_continuity_experience_context;
use crate::forge_wiki::model::SelectedForgeWikiPage;
use crate::harness::capability::CapabilityKind;
use crate::harness::permissions::PermissionMode;
use crate::harness::registry::CapabilityEntry;
use crate::harness::Harness;
use crate::ipc::delivery_summary::build_store_emit_delivery_summary;
use crate::ipc::file_references::{
    build_file_reference_context_with_paths, resolved_file_reference_paths_for_turn,
};
use crate::ipc::mcp_context::{build_mcp_context, McpContextBuildResult, McpContextSelection};
use crate::ipc::project_records::{
    propose_send_input_project_record_update, select_send_input_project_records_context,
    SendInputProjectRecordsSelection,
};
use crate::ipc::send_input_continuity::{
    record_failed_send_input_continuity, record_send_input_user_message_continuity,
    record_successful_send_input_continuity,
};
use crate::ipc::session_lifecycle::{
    save_session_snapshot_with_workflow, upgrade_missing_key_session_if_possible,
};
use crate::memory::{
    format_unified_memory_context, MemoryCategory, MemoryScope, RecallPlan, SelectedContextMemory,
    UnifiedMemoryKind, UnifiedMemoryScope, UnifiedMemorySelection, UnifiedMemorySource,
};
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;
use crate::state::AppState;
use crate::workflow::{classify_workflow_with_command, WorkflowState};

pub(crate) struct PreparedSendInputTurnContext {
    pub(crate) hidden_contexts: Vec<HiddenContextPart>,
    pub(crate) turn_metadata: AgentTurnMetadata,
    pub(crate) activation_text: String,
    pub(crate) prepared_turn: PreparedTurn,
}

pub(crate) struct SendInputMemorySelection {
    pub(crate) selected: Vec<SelectedContextMemory>,
    pub(crate) audit: Vec<PreparedTurnMemoryAudit>,
    pub(crate) context: Option<String>,
    pub(crate) recall_plan: Option<RecallPlan>,
}

pub(crate) async fn resolve_send_input_session(
    app_handle: &tauri::AppHandle,
    state: &Arc<AppState>,
    session_id: &str,
) -> Result<(Arc<AgentSession>, String), String> {
    let session = state
        .sessions
        .read()
        .await
        .get(session_id)
        .cloned()
        .ok_or_else(|| format!("Session not found: {session_id}"))?;
    let session = upgrade_missing_key_session_if_possible(app_handle, state, session).await?;
    let project_path = session.harness.working_dir.to_string_lossy().to_string();
    Ok((session, project_path))
}

pub(crate) fn record_send_input_user_turn(
    state: &Arc<AppState>,
    session: &AgentSession,
    session_id: &str,
    text: &str,
    project_path: &str,
) -> Result<TurnInflightGuard, String> {
    let turn_guard = reserve_turn_then_record_user_message(session, session_id, text, |event| {
        if let Err(error) = crate::transcript::append_stream_event(&event) {
            crate::app_log!("WARN", "[transcript] {}", error);
        }
    })?;
    record_send_input_user_message_continuity(state, project_path, session_id, text);
    Ok(turn_guard)
}

pub(crate) async fn select_send_input_memory_context(
    state: &Arc<AppState>,
    text: &str,
    project_path: &str,
) -> SendInputMemorySelection {
    let unified_recall = match crate::ipc::unified_memory::select_unified_memories_for_send_input(
        state,
        text,
        project_path,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            crate::app_log!("WARN", "[memory] unified recall failed: {}", error);
            return SendInputMemorySelection {
                selected: Vec::new(),
                audit: Vec::new(),
                context: None,
                recall_plan: None,
            };
        }
    };
    let unified = unified_recall.selected;
    if !unified.is_empty() {
        let ids = unified
            .iter()
            .map(|selection| selection.record.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        crate::app_log!("INFO", "[memory] unified recalled records: {}", ids);
    }
    let context = format_unified_memory_context(&unified);
    let audit = unified
        .iter()
        .map(|selection| unified_selection_to_audit(selection, project_path))
        .collect();
    let selected = unified
        .into_iter()
        .map(unified_selection_to_selected_context)
        .collect();
    SendInputMemorySelection {
        selected,
        audit,
        context,
        recall_plan: Some(unified_recall.plan),
    }
}

pub(crate) async fn select_send_input_continuity_context(
    state: &Arc<AppState>,
    text: &str,
    project_path: &str,
) -> Option<String> {
    match state
        .continuity
        .recall_experiences_for_project(project_path, text, 5)
    {
        Ok(experiences) => {
            if !experiences.is_empty() {
                let ids = experiences
                    .iter()
                    .map(|experience| experience.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                crate::app_log!("INFO", "[continuity] recalled experiences: {}", ids);
            }
            form_continuity_experience_context(&experiences)
        }
        Err(error) => {
            crate::app_log!("WARN", "[continuity] recall failed: {}", error);
            None
        }
    }
}

/// Classify workflow from user input, store it, emit the update event,
/// and return the input intent and workflow for downstream use.
pub(crate) async fn setup_send_input_workflow(
    state: &Arc<AppState>,
    app_handle: &tauri::AppHandle,
    session_id: &str,
    text: &str,
    capabilities: &[ComposerCapabilitySelection],
) -> (TurnInputIntent, WorkflowState) {
    let input_intent = build_turn_input_intent(text, capabilities, Vec::new());
    let workflow = classify_workflow_with_command(
        session_id,
        text,
        input_intent.slash_command.as_deref(),
        now_ms(),
    );
    state
        .workflow_states
        .write()
        .await
        .insert(session_id.to_string(), workflow.clone());
    crate::transcript::emit_stream_event(
        app_handle,
        StreamEvent::WorkflowUpdated {
            session_id: session_id.to_string(),
            state: workflow.clone(),
        },
    );
    (input_intent, workflow)
}

pub(crate) fn reserve_turn_then_record_user_message<F>(
    session: &AgentSession,
    session_id: &str,
    text: &str,
    record_user_message: F,
) -> Result<TurnInflightGuard, String>
where
    F: FnOnce(StreamEvent),
{
    let turn_guard = session.reserve_turn()?;
    record_user_message(StreamEvent::UserMessage {
        session_id: session_id.to_string(),
        block_id: BlockId::new().to_string(),
        content: text.to_string(),
    });
    Ok(turn_guard)
}

pub(crate) struct PrepareSendInputTurnRequest<'a> {
    pub(crate) session_id: &'a str,
    pub(crate) session: &'a AgentSession,
    pub(crate) text: &'a str,
    pub(crate) input_intent: TurnInputIntent,
    pub(crate) workflow: &'a WorkflowState,
    pub(crate) ready_connector_labels: Vec<String>,
    pub(crate) memory_context: Option<String>,
    pub(crate) wiki_context: Option<String>,
    pub(crate) continuity_context: Option<String>,
    pub(crate) connector_context: Option<String>,
    pub(crate) selected_memories: Vec<SelectedContextMemory>,
    pub(crate) selected_memory_audit: Vec<PreparedTurnMemoryAudit>,
    pub(crate) memory_recall_plan: Option<RecallPlan>,
    pub(crate) selected_project_records: Vec<SelectedForgeWikiPage>,
    pub(crate) permission_mode: PermissionMode,
}

async fn collect_turn_capability_snapshot(
    harness: &Harness,
    input_intent: &TurnInputIntent,
) -> TurnCapabilitySnapshot {
    harness.skill_loader.scan_all().await;
    let matched_skills = harness
        .skill_loader
        .matched_skills_for_request(&input_intent.activation_text)
        .await
        .into_iter()
        .map(|matched| matched.label())
        .collect::<Vec<_>>();

    TurnCapabilitySnapshot {
        slash_command: input_intent.slash_command.clone(),
        file_references: input_intent.file_references.clone(),
        selected_connectors: input_intent.selected_connectors.clone(),
        matched_skills,
        active_hooks: capability_names_by_kind(harness, CapabilityKind::Hook),
        enabled_mcp_servers: capability_names_by_kind(harness, CapabilityKind::McpServer),
        available_mcp_tools: harness
            .external_mcp_tool_definitions()
            .await
            .into_iter()
            .map(|tool| tool.name)
            .collect(),
    }
}

pub(crate) fn capability_names_by_kind(harness: &Harness, kind: CapabilityKind) -> Vec<String> {
    harness
        .capability_registry
        .all_entries()
        .into_iter()
        .filter(|entry| {
            entry.enabled && entry.metadata.kind == kind && is_turn_relevant_capability(entry)
        })
        .map(|entry| entry.metadata.name)
        .collect()
}

fn is_turn_relevant_capability(entry: &CapabilityEntry) -> bool {
    !matches!(
        entry.metadata.id.as_str(),
        "skill-loader" | "hook:logging" | "hook:fs-audit"
    )
}

pub(crate) async fn prepare_send_input_turn_context(
    request: PrepareSendInputTurnRequest<'_>,
) -> PreparedSendInputTurnContext {
    let PrepareSendInputTurnRequest {
        session_id,
        session,
        text,
        mut input_intent,
        workflow,
        ready_connector_labels,
        memory_context,
        wiki_context,
        continuity_context,
        connector_context,
        selected_memories,
        selected_memory_audit,
        memory_recall_plan,
        selected_project_records,
        permission_mode,
    } = request;
    let project_path = session.harness.working_dir.to_string_lossy().to_string();
    let resolved_file_references = resolved_file_reference_paths_for_turn(
        &session.harness.working_dir,
        text,
        &input_intent.file_references,
    );
    input_intent.file_references = resolved_file_references.clone();
    input_intent.selected_connectors = ready_connector_labels.clone();
    let capability_snapshot =
        collect_turn_capability_snapshot(&session.harness, &input_intent).await;
    let activation_text = input_intent.activation_text.clone();
    let mut hidden_contexts = Vec::new();
    if let Some(context) = format_turn_capability_snapshot(&capability_snapshot) {
        hidden_contexts.push(HiddenContextPart::new(
            ContextSourceKind::CapabilitySnapshot,
            "本轮能力",
            "本轮自动整理出的动作、资料、技能和安全规则",
            context,
        ));
    }
    if let Some(context) = build_file_reference_context_with_paths(
        &session.harness.working_dir,
        "",
        &resolved_file_references,
    ) {
        hidden_contexts.push(HiddenContextPart::new(
            ContextSourceKind::SelectedFiles,
            "选中文件",
            "用户选中的本轮参考文件",
            context,
        ));
    }
    if let Some(context) = memory_context {
        hidden_contexts.push(HiddenContextPart::new(
            ContextSourceKind::MemoryContext,
            "已保存背景",
            "自动匹配到的用户和项目背景",
            context,
        ));
    }
    if let Some(context) = wiki_context {
        hidden_contexts.push(HiddenContextPart::new(
            ContextSourceKind::ProjectRecords,
            "项目记录",
            "自动匹配到的项目记录",
            context,
        ));
    }
    if let Some(context) = continuity_context {
        hidden_contexts.push(HiddenContextPart::new(
            ContextSourceKind::ContinuityExperience,
            "经验回忆",
            "自动召回的已确认项目经验",
            context,
        ));
    }
    if let Some(context) = connector_context {
        hidden_contexts.push(HiddenContextPart::new(
            ContextSourceKind::ConnectorContext,
            "连接资料",
            "用户选中的连接资料",
            context,
        ));
    }
    let workflow_route = workflow_route_label(workflow);
    let workflow_phase = workflow_phase_label(workflow);
    let prepared_turn = build_prepared_turn(PreparedTurnBuildRequest {
        session_id,
        project_path: &project_path,
        user_text: text,
        activation_text: &activation_text,
        input_intent: &input_intent,
        workflow_route: workflow_route.clone(),
        workflow_phase: workflow_phase.clone(),
        hidden_contexts: &hidden_contexts,
        selected_memories: &selected_memories,
        selected_memory_audit: &selected_memory_audit,
        memory_recall_plan: memory_recall_plan.as_ref(),
        selected_project_records: &selected_project_records,
        permission_mode,
        context_window_tokens: session.context_window_tokens,
    });
    let turn_metadata = AgentTurnMetadata {
        session_id: session_id.to_string(),
        workspace_path: project_path,
        provider: session.agent_type.clone(),
        model: session.model_id.clone(),
        route: workflow_route,
        phase: workflow_phase,
        user_goal: text.to_string(),
        input_intent: AgentTurnInputIntent {
            slash_command: input_intent.slash_command.clone(),
            file_references: resolved_file_references,
            selected_connectors: ready_connector_labels,
            matched_skills: capability_snapshot.matched_skills.clone(),
            active_hooks: capability_snapshot.active_hooks.clone(),
            enabled_mcp_servers: capability_snapshot.enabled_mcp_servers.clone(),
            available_mcp_tools: capability_snapshot.available_mcp_tools.clone(),
        },
    };

    PreparedSendInputTurnContext {
        hidden_contexts,
        turn_metadata,
        activation_text,
        prepared_turn,
    }
}

fn workflow_route_label(workflow: &WorkflowState) -> String {
    serde_json::to_value(&workflow.route)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| workflow.developer_label.clone())
}

fn workflow_phase_label(workflow: &WorkflowState) -> String {
    serde_json::to_value(&workflow.phase)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| workflow.developer_label.clone())
}

pub(crate) async fn finalize_send_input_turn(
    state: &Arc<AppState>,
    app_handle: &tauri::AppHandle,
    session: &AgentSession,
    text: &str,
    project_path: &str,
    workflow: &WorkflowState,
    result: &Result<(), String>,
) {
    if let Err(error) = save_session_snapshot_with_workflow(state, session).await {
        crate::app_log!("WARN", "[session_snapshot] {}", error);
    }
    let latest_turn_for_delivery = session.snapshot().latest_turn;
    if result.is_ok() {
        record_successful_send_input_continuity(
            state,
            app_handle,
            &session.id,
            text,
            project_path,
            latest_turn_for_delivery.as_ref(),
        )
        .await;
        let writeback = propose_send_input_project_record_update(
            state,
            &session.id,
            text,
            project_path,
            workflow,
            latest_turn_for_delivery.as_ref(),
        )
        .await;
        if let Some(proposal) = writeback.proposal {
            crate::transcript::emit_stream_event(
                app_handle,
                StreamEvent::ForgeWikiUpdateProposed {
                    session_id: session.id.clone(),
                    proposal,
                },
            );
        }
        build_store_emit_delivery_summary(
            state,
            app_handle,
            &session.id,
            latest_turn_for_delivery.as_ref(),
            writeback.record_evidence,
        )
        .await;
        if let Err(error) = save_session_snapshot_with_workflow(state, session).await {
            crate::app_log!("WARN", "[session_snapshot] {}", error);
        }
    } else {
        record_failed_send_input_continuity(
            state,
            &session.id,
            text,
            project_path,
            latest_turn_for_delivery.as_ref(),
        );
    }
}

pub(crate) struct SendInputContextBundle {
    pub(crate) input_intent: TurnInputIntent,
    pub(crate) workflow: WorkflowState,
    pub(crate) project_records: SendInputProjectRecordsSelection,
    pub(crate) memory_selection: SendInputMemorySelection,
    pub(crate) continuity_context: Option<String>,
    pub(crate) mcp_result: McpContextBuildResult,
}

pub(crate) struct SelectSendInputContextsRequest<'a> {
    pub(crate) state: &'a Arc<AppState>,
    pub(crate) app_handle: &'a tauri::AppHandle,
    pub(crate) session_id: &'a str,
    pub(crate) text: &'a str,
    pub(crate) project_path: &'a str,
    pub(crate) harness: &'a Harness,
    pub(crate) capabilities: Vec<ComposerCapabilitySelection>,
    pub(crate) mcp_context_selections: Vec<McpContextSelection>,
}

pub(crate) async fn select_send_input_contexts(
    request: SelectSendInputContextsRequest<'_>,
) -> SendInputContextBundle {
    let SelectSendInputContextsRequest {
        state,
        app_handle,
        session_id,
        text,
        project_path,
        harness,
        capabilities,
        mcp_context_selections,
    } = request;

    let (input_intent, workflow) =
        setup_send_input_workflow(state, app_handle, session_id, text, &capabilities).await;
    let project_records =
        select_send_input_project_records_context(state, text, project_path).await;
    if !project_records.selected.is_empty() {
        crate::transcript::emit_stream_event(
            app_handle,
            StreamEvent::ForgeWikiContextSelected {
                session_id: session_id.to_string(),
                selected: project_records.selected.clone(),
            },
        );
    }
    let memory_selection = select_send_input_memory_context(state, text, project_path).await;
    crate::transcript::emit_stream_event(
        app_handle,
        StreamEvent::MemorySelection {
            session_id: session_id.to_string(),
            selected: memory_selection.selected.clone(),
        },
    );
    let mcp_result =
        build_mcp_context(harness, &mcp_context_selections, app_handle, session_id).await;
    SendInputContextBundle {
        input_intent,
        workflow,
        project_records,
        memory_selection,
        continuity_context: None,
        mcp_result,
    }
}

pub(crate) struct BuildPreparedSendInputTurnRequest<'a> {
    pub(crate) session_id: &'a str,
    pub(crate) session: &'a AgentSession,
    pub(crate) text: &'a str,
    pub(crate) input_intent: TurnInputIntent,
    pub(crate) workflow: &'a WorkflowState,
    pub(crate) ready_connector_labels: Vec<String>,
    pub(crate) memory_context: Option<String>,
    pub(crate) wiki_context: Option<String>,
    pub(crate) continuity_context: Option<String>,
    pub(crate) connector_context: Option<String>,
    pub(crate) selected_memories: Vec<SelectedContextMemory>,
    pub(crate) selected_memory_audit: Vec<PreparedTurnMemoryAudit>,
    pub(crate) memory_recall_plan: Option<RecallPlan>,
    pub(crate) selected_project_records: Vec<SelectedForgeWikiPage>,
    pub(crate) permission_mode: PermissionMode,
}

pub(crate) async fn build_prepared_send_input_turn(
    request: BuildPreparedSendInputTurnRequest<'_>,
) -> PreparedSendInputTurnContext {
    let BuildPreparedSendInputTurnRequest {
        session_id,
        session,
        text,
        input_intent,
        workflow,
        ready_connector_labels,
        memory_context,
        wiki_context,
        continuity_context,
        connector_context,
        selected_memories,
        selected_memory_audit,
        memory_recall_plan,
        selected_project_records,
        permission_mode,
    } = request;

    prepare_send_input_turn_context(PrepareSendInputTurnRequest {
        session_id,
        session,
        text,
        input_intent,
        workflow,
        ready_connector_labels,
        memory_context,
        wiki_context,
        continuity_context,
        connector_context,
        selected_memories,
        selected_memory_audit,
        memory_recall_plan,
        selected_project_records,
        permission_mode,
    })
    .await
}

pub(crate) fn emit_prepared_send_input_turn(
    app_handle: &tauri::AppHandle,
    session_id: &str,
    prepared: &PreparedSendInputTurnContext,
) {
    crate::transcript::emit_stream_event(
        app_handle,
        StreamEvent::TurnPrepared {
            session_id: session_id.to_string(),
            prepared: prepared.prepared_turn.clone(),
        },
    );
}

pub(crate) struct RunReservedSendInputTurnRequest<'a> {
    pub(crate) state: &'a Arc<AppState>,
    pub(crate) app_handle: &'a tauri::AppHandle,
    pub(crate) session: &'a AgentSession,
    pub(crate) text: &'a str,
    pub(crate) project_path: &'a str,
    pub(crate) workflow: &'a WorkflowState,
    pub(crate) prepared: PreparedSendInputTurnContext,
    pub(crate) turn_guard: TurnInflightGuard,
}

pub(crate) async fn run_reserved_send_input_turn(
    request: RunReservedSendInputTurnRequest<'_>,
) -> Result<(), String> {
    let RunReservedSendInputTurnRequest {
        state,
        app_handle,
        session,
        text,
        project_path,
        workflow,
        prepared,
        turn_guard,
    } = request;

    let result = session
        .send_message_with_reserved_turn(
            text,
            app_handle,
            prepared.hidden_contexts,
            Some(prepared.turn_metadata),
            Some(&prepared.activation_text),
            turn_guard,
        )
        .await;
    finalize_send_input_turn(
        state,
        app_handle,
        session,
        text,
        project_path,
        workflow,
        &result,
    )
    .await;
    result
}

fn unified_selection_to_selected_context(
    selection: UnifiedMemorySelection,
) -> SelectedContextMemory {
    let category = unified_kind_to_memory_category(&selection.record.kind);
    let scope = unified_scope_to_memory_scope(&selection.record.scope);
    SelectedContextMemory {
        memory_id: selection.record.id,
        title: selection.record.title,
        body: selection.record.body,
        category,
        scope,
        score: selection.score,
        reason: selection.reason,
        injected: selection.injected,
    }
}

fn unified_selection_to_audit(
    selection: &UnifiedMemorySelection,
    project_path: &str,
) -> PreparedTurnMemoryAudit {
    PreparedTurnMemoryAudit {
        memory_id: selection.record.id.clone(),
        source: unified_source_label(&selection.record.source).to_string(),
        source_id: selection.record.source_id.clone(),
        kind: unified_kind_label(&selection.record.kind).to_string(),
        score: selection.score,
        reason: selection.reason.clone(),
        project_match: selection
            .record
            .project_path
            .as_deref()
            .is_some_and(|record_project| {
                normalize_path(record_project) == normalize_path(project_path)
            }),
        profile_match: selection.record.profile_id.is_some(),
        injected: selection.injected,
    }
}

fn unified_source_label(source: &UnifiedMemorySource) -> &'static str {
    match source {
        UnifiedMemorySource::WikiMemory => "wiki_memory",
        UnifiedMemorySource::MemoryFact => "memory_fact",
        UnifiedMemorySource::ContinuityExperience => "continuity_experience",
    }
}

fn unified_kind_label(kind: &UnifiedMemoryKind) -> &'static str {
    match kind {
        UnifiedMemoryKind::Preference => "preference",
        UnifiedMemoryKind::ProjectFact => "project_fact",
        UnifiedMemoryKind::Decision => "decision",
        UnifiedMemoryKind::TaskState => "task_state",
        UnifiedMemoryKind::Lesson => "lesson",
        UnifiedMemoryKind::BugPattern => "bug_pattern",
        UnifiedMemoryKind::Workflow => "workflow",
    }
}

fn normalize_path(path: &str) -> String {
    path.trim_end_matches('/').to_string()
}

fn unified_kind_to_memory_category(kind: &UnifiedMemoryKind) -> MemoryCategory {
    match kind {
        UnifiedMemoryKind::Preference => MemoryCategory::Preference,
        UnifiedMemoryKind::Decision => MemoryCategory::Decision,
        UnifiedMemoryKind::TaskState => MemoryCategory::TaskState,
        UnifiedMemoryKind::ProjectFact
        | UnifiedMemoryKind::Lesson
        | UnifiedMemoryKind::BugPattern
        | UnifiedMemoryKind::Workflow => MemoryCategory::ProjectFact,
    }
}

fn unified_scope_to_memory_scope(scope: &UnifiedMemoryScope) -> MemoryScope {
    match scope {
        UnifiedMemoryScope::Session => MemoryScope::Session,
        UnifiedMemoryScope::UserProfile => MemoryScope::UserProfile,
        UnifiedMemoryScope::Project => MemoryScope::Project,
        UnifiedMemoryScope::Document => MemoryScope::Document,
    }
}

#[cfg(test)]
#[path = "send_input_context_tests.rs"]
mod tests;
