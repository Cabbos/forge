use std::{path::PathBuf, sync::Arc};

use crate::adapters::base::AiAdapter;
use crate::adapters::build_adapter;
use crate::adapters::missing_key::MissingKeyAdapter;
use crate::agent::capability_context::{build_turn_input_intent, ComposerCapabilitySelection};
use crate::agent::provider_capabilities::{
    context_window_tokens, default_model, missing_api_key_message, normalize_provider,
    provider_label,
};
use crate::agent::session::{AgentSession, TurnInflightGuard};
use crate::agent::snapshot::{
    delete_session_snapshot, list_session_snapshots, load_session_snapshot, save_session_snapshot,
    AgentSessionSnapshot,
};
use crate::agent::time::now_ms;
use crate::continuity::ExperienceMemory;
use crate::harness::Harness;
use crate::ipc::delivery_summary::{build_store_emit_delivery_summary, emit_delivery_summary};
use crate::ipc::file_search::find_files;
use crate::ipc::mcp_context::{build_mcp_context, McpContextSelection};
use crate::ipc::open_file::{open_file_macos, resolve_workspace_file_path};
use crate::ipc::project_records::{
    propose_send_input_project_record_update, select_send_input_project_records_context,
};
use crate::ipc::send_input_context::{
    prepare_send_input_turn_context, select_send_input_memory_context, PrepareSendInputTurnRequest,
};
use crate::ipc::send_input_continuity::{
    record_failed_send_input_continuity, record_send_input_user_message_continuity,
    record_successful_send_input_continuity,
};
use crate::ipc::workspace::resolve_bound_working_dir;
use crate::protocol::commands::{SessionCreated, SessionInfo};
use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;
use crate::settings;
use crate::state::AppState;
use crate::workflow::classify_workflow_with_command;
use crate::workspace_safety::{
    resolve_session_workspace_path as resolve_session_working_dir,
    resolve_workspace_path as resolve_safe_workspace_path,
};

#[derive(serde::Serialize)]
pub struct FilePreviewLine {
    number: usize,
    content: String,
    is_target: bool,
}

#[derive(serde::Serialize)]
pub struct FilePreview {
    path: String,
    display_path: String,
    requested_line: Option<u32>,
    start_line: usize,
    total_lines: usize,
    lines: Vec<FilePreviewLine>,
}

#[derive(serde::Serialize)]
pub struct McpContextSources {
    resources: Vec<McpContextResource>,
    prompts: Vec<McpContextPrompt>,
}

#[derive(serde::Serialize)]
pub struct McpContextResource {
    server_id: String,
    uri: String,
    name: String,
    description: String,
    mime_type: Option<String>,
}

#[derive(serde::Serialize)]
pub struct McpContextPrompt {
    server_id: String,
    name: String,
    description: String,
    arguments: Vec<McpContextPromptArgument>,
}

#[derive(serde::Serialize)]
pub struct McpContextPromptArgument {
    name: String,
    description: String,
    required: bool,
}

const CONTINUITY_RECALL_DEFAULT_LIMIT: usize = 8;
const CONTINUITY_RECALL_MAX_LIMIT: usize = 20;

fn emit_missing_api_key_notice(app_handle: &tauri::AppHandle, session_id: &str, provider: &str) {
    crate::transcript::emit_stream_event(
        app_handle,
        StreamEvent::Error {
            session_id: session_id.to_string(),
            block_id: BlockId::new().to_string(),
            message: missing_api_key_message(provider),
            code: "missing_api_key".to_string(),
        },
    );
}

fn reserve_turn_then_record_user_message<F>(
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

async fn upgrade_missing_key_session_if_possible(
    app_handle: &tauri::AppHandle,
    state: &Arc<AppState>,
    session: Arc<AgentSession>,
) -> Result<Arc<AgentSession>, String> {
    if !session.is_waiting_for_api_key() {
        return Ok(session);
    }

    let snapshot = session.snapshot();
    let provider = normalize_provider(Some(&snapshot.provider));
    let credentials = settings::detect_credentials(&provider);
    if credentials.api_key.trim().is_empty() {
        return Ok(session);
    }

    let working_dir = resolve_safe_workspace_path(&snapshot.working_dir)?;
    let harness = Arc::new(Harness::new_with_pending(
        working_dir.clone(),
        state.pending_confirms.clone(),
    ));
    let model_str = snapshot.model.clone();
    let external_tools = harness.external_mcp_tool_definitions().await;
    let adapter = build_adapter(
        &provider,
        &credentials.api_key,
        &model_str,
        credentials.api_base.as_deref(),
        external_tools,
    )?;
    let system_prompt = harness.build_system_prompt(&provider, &working_dir).await;
    let upgraded = AgentSession::new(
        snapshot.session_id.clone(),
        provider.clone(),
        adapter,
        harness,
        system_prompt,
        snapshot.context_window_tokens,
    );
    upgraded.restore_state(snapshot.messages, snapshot.summary, snapshot.latest_turn);
    let upgraded = Arc::new(upgraded);
    state
        .register_session(snapshot.session_id.clone(), upgraded.clone())
        .await;
    let _ = upgraded
        .harness
        .dispatch_session_start_event(&snapshot.session_id)
        .await;
    crate::transcript::emit_stream_event(
        app_handle,
        StreamEvent::SessionStarted {
            session_id: snapshot.session_id,
            agent_type: provider,
            model: model_str,
            context_window_tokens: upgraded.context_window_tokens,
        },
    );
    Ok(upgraded)
}

async fn save_session_snapshot_with_workflow(
    state: &Arc<AppState>,
    session: &AgentSession,
) -> Result<(), String> {
    let snapshot = session_snapshot_with_workflow_state(state, session).await;
    save_session_snapshot(&snapshot)
}

async fn session_snapshot_with_workflow_state(
    state: &Arc<AppState>,
    session: &AgentSession,
) -> AgentSessionSnapshot {
    let latest_workflow = state.workflow_states.read().await.get(&session.id).cloned();
    let latest_delivery = state.delivery_states.read().await.get(&session.id).cloned();
    let mut snapshot = session.snapshot();
    if let Some(workflow) = latest_workflow {
        snapshot = snapshot.with_latest_workflow(workflow);
    }
    if let Some(delivery) = latest_delivery {
        snapshot = snapshot.with_latest_delivery(delivery);
    }
    snapshot
}

#[tauri::command]
pub async fn create_session(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    working_dir: String,
    provider: Option<String>,
    api_key: String,
    model: Option<String>,
) -> Result<SessionCreated, String> {
    let session_id = uuid::Uuid::now_v7().to_string();
    let provider = normalize_provider(provider.as_deref());
    let credentials = settings::detect_credentials(&provider);

    let key = if api_key.is_empty() {
        credentials.api_key
    } else {
        api_key
    };

    let model_str = model
        .or(credentials.model)
        .unwrap_or_else(|| default_model(&provider).to_string());
    let working_dir = resolve_session_working_dir(&working_dir)?;
    let harness = Arc::new(Harness::new_with_pending(
        working_dir.clone(),
        state.pending_confirms.clone(),
    ));
    let context_window_tokens = context_window_tokens(&provider, &model_str);
    let missing_api_key = key.trim().is_empty();
    let external_tools = if missing_api_key {
        Vec::new()
    } else {
        harness.external_mcp_tool_definitions().await
    };
    let adapter = if missing_api_key {
        Arc::new(MissingKeyAdapter::new(
            provider_label(&provider),
            &model_str,
        )) as Arc<dyn AiAdapter>
    } else {
        build_adapter(
            &provider,
            &key,
            &model_str,
            credentials.api_base.as_deref(),
            external_tools,
        )?
    };

    // Build system prompt from harness (active skills + project CLAUDE.md)
    let system_prompt = harness.build_system_prompt(&provider, &working_dir).await;

    let session = AgentSession::new(
        session_id.clone(),
        provider.clone(),
        adapter,
        harness.clone(),
        system_prompt,
        context_window_tokens,
    );

    crate::transcript::emit_stream_event(
        &app_handle,
        StreamEvent::SessionStarted {
            session_id: session_id.clone(),
            agent_type: provider.clone(),
            model: model_str.clone(),
            context_window_tokens,
        },
    );
    if missing_api_key {
        emit_missing_api_key_notice(&app_handle, &session_id, &provider);
    }

    let session = Arc::new(session);
    if let Err(error) = save_session_snapshot(&session.snapshot()) {
        crate::app_log!("WARN", "[session_snapshot] {}", error);
    }
    state.register_session(session_id.clone(), session).await;
    let _ = harness.dispatch_session_start_event(&session_id).await;
    Ok(SessionCreated {
        session_id,
        provider,
        model: model_str,
        missing_api_key,
    })
}

#[tauri::command]
pub async fn list_mcp_context_sources(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
) -> Result<McpContextSources, String> {
    let Some(harness) = mcp_context_harness_for_session(&state, session_id.as_deref()).await?
    else {
        return Ok(McpContextSources {
            resources: Vec::new(),
            prompts: Vec::new(),
        });
    };

    let resources = harness
        .external_mcp_resource_definitions()
        .await
        .into_iter()
        .map(|resource| McpContextResource {
            server_id: resource.server_id,
            uri: resource.uri,
            name: resource.name,
            description: resource.description,
            mime_type: resource.mime_type,
        })
        .collect();
    let prompts = harness
        .external_mcp_prompt_definitions()
        .await
        .into_iter()
        .map(|prompt| McpContextPrompt {
            server_id: prompt.server_id,
            name: prompt.name,
            description: prompt.description,
            arguments: prompt
                .arguments
                .into_iter()
                .map(|argument| McpContextPromptArgument {
                    name: argument.name,
                    description: argument.description,
                    required: argument.required,
                })
                .collect(),
        })
        .collect();

    Ok(McpContextSources { resources, prompts })
}

async fn mcp_context_harness_for_session(
    state: &Arc<AppState>,
    session_id: Option<&str>,
) -> Result<Option<Arc<Harness>>, String> {
    let Some(session_id) = session_id else {
        return Ok(None);
    };
    state
        .sessions
        .read()
        .await
        .get(session_id)
        .map(|session| Some(session.harness.clone()))
        .ok_or_else(|| "当前会话不可用，请重新打开对话或重新选择项目。".to_string())
}

#[tauri::command]
pub async fn resume_session(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<SessionCreated, String> {
    let existing_session = state.sessions.read().await.get(&session_id).cloned();
    if let Some(session) = existing_session {
        let session = upgrade_missing_key_session_if_possible(&app_handle, &state, session).await?;
        session.resume(&app_handle);
        let _ = session
            .harness
            .dispatch_session_start_event(&session_id)
            .await;
        if let Err(error) = save_session_snapshot_with_workflow(&state, &session).await {
            crate::app_log!("WARN", "[session_snapshot] {}", error);
        }
        if let Some(workflow) = state.workflow_states.read().await.get(&session_id).cloned() {
            crate::transcript::emit_stream_event(
                &app_handle,
                StreamEvent::WorkflowUpdated {
                    session_id: session_id.clone(),
                    state: workflow,
                },
            );
        }
        session.emit_latest_turn_projection(&app_handle);
        if let Some(delivery) = state.delivery_states.read().await.get(&session_id).cloned() {
            emit_delivery_summary(&app_handle, &session_id, delivery);
        }
        return Ok(SessionCreated {
            session_id,
            provider: normalize_provider(Some(&session.agent_type)),
            model: session.model_id.clone(),
            missing_api_key: session.is_waiting_for_api_key(),
        });
    }

    let snapshot = load_session_snapshot(&session_id)?;
    let provider = normalize_provider(Some(&snapshot.provider));
    let credentials = settings::detect_credentials(&provider);
    let latest_workflow = snapshot.latest_workflow.clone();
    let latest_delivery = snapshot.latest_delivery.clone();

    let model_str = snapshot.model.clone();
    let context_window_tokens = snapshot
        .context_window_tokens
        .or_else(|| context_window_tokens(&provider, &model_str));
    let missing_api_key = credentials.api_key.trim().is_empty();
    let working_dir = resolve_safe_workspace_path(&snapshot.working_dir)?;
    let harness = Arc::new(Harness::new_with_pending(
        working_dir.clone(),
        state.pending_confirms.clone(),
    ));
    let external_tools = if missing_api_key {
        Vec::new()
    } else {
        harness.external_mcp_tool_definitions().await
    };
    let adapter = if missing_api_key {
        Arc::new(MissingKeyAdapter::new(
            provider_label(&provider),
            &model_str,
        )) as Arc<dyn AiAdapter>
    } else {
        build_adapter(
            &provider,
            &credentials.api_key,
            &model_str,
            credentials.api_base.as_deref(),
            external_tools,
        )?
    };
    let system_prompt = harness.build_system_prompt(&provider, &working_dir).await;

    let session = AgentSession::new(
        snapshot.session_id.clone(),
        provider.clone(),
        adapter,
        harness,
        system_prompt,
        context_window_tokens,
    );
    session.restore_state(snapshot.messages, snapshot.summary, snapshot.latest_turn);
    let session = Arc::new(session);
    state
        .register_session(snapshot.session_id.clone(), session.clone())
        .await;
    let _ = session
        .harness
        .dispatch_session_start_event(&snapshot.session_id)
        .await;
    if let Err(error) = save_session_snapshot_with_workflow(&state, &session).await {
        crate::app_log!("WARN", "[session_snapshot] {}", error);
    }

    crate::transcript::emit_stream_event(
        &app_handle,
        StreamEvent::SessionStarted {
            session_id: snapshot.session_id.clone(),
            agent_type: provider.clone(),
            model: model_str.clone(),
            context_window_tokens,
        },
    );
    if let Some(workflow) = latest_workflow {
        state
            .workflow_states
            .write()
            .await
            .insert(snapshot.session_id.clone(), workflow.clone());
        crate::transcript::emit_stream_event(
            &app_handle,
            StreamEvent::WorkflowUpdated {
                session_id: snapshot.session_id.clone(),
                state: workflow,
            },
        );
    }
    session.emit_latest_turn_projection(&app_handle);
    if let Some(delivery) = latest_delivery {
        state
            .delivery_states
            .write()
            .await
            .insert(snapshot.session_id.clone(), delivery.clone());
        emit_delivery_summary(&app_handle, &snapshot.session_id, delivery);
    }
    if missing_api_key {
        emit_missing_api_key_notice(&app_handle, &snapshot.session_id, &provider);
    }

    Ok(SessionCreated {
        session_id: snapshot.session_id,
        provider,
        model: model_str,
        missing_api_key,
    })
}

#[tauri::command]
pub async fn send_input(
    state: tauri::State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
    session_id: String,
    text: String,
    mcp_context: Option<Vec<McpContextSelection>>,
    capabilities: Option<Vec<ComposerCapabilitySelection>>,
) -> Result<(), String> {
    let session = state.sessions.read().await.get(&session_id).cloned();
    match session {
        Some(s) => {
            let s = upgrade_missing_key_session_if_possible(&app_handle, &state, s).await?;
            let project_path = s.harness.working_dir.to_string_lossy().to_string();
            record_send_input_user_message_continuity(&state, &project_path, &session_id, &text);
            let turn_guard =
                reserve_turn_then_record_user_message(&s, &session_id, &text, |event| {
                    if let Err(error) = crate::transcript::append_stream_event(&event) {
                        crate::app_log!("WARN", "[transcript] {}", error);
                    }
                })?;
            let capabilities = capabilities.unwrap_or_default();
            let mcp_context_selections = mcp_context.unwrap_or_default();
            let input_intent = build_turn_input_intent(&text, &capabilities, Vec::new());
            let workflow = classify_workflow_with_command(
                &session_id,
                &text,
                input_intent.slash_command.as_deref(),
                now_ms(),
            );
            state
                .workflow_states
                .write()
                .await
                .insert(session_id.clone(), workflow.clone());
            crate::transcript::emit_stream_event(
                &app_handle,
                StreamEvent::WorkflowUpdated {
                    session_id: session_id.clone(),
                    state: workflow.clone(),
                },
            );
            let project_records =
                select_send_input_project_records_context(&state, &text, &project_path).await;
            if !project_records.selected.is_empty() {
                crate::transcript::emit_stream_event(
                    &app_handle,
                    StreamEvent::ForgeWikiContextSelected {
                        session_id: session_id.clone(),
                        selected: project_records.selected.clone(),
                    },
                );
            }
            let memory_selection =
                select_send_input_memory_context(&state, &text, &project_path).await;
            crate::transcript::emit_stream_event(
                &app_handle,
                StreamEvent::MemorySelection {
                    session_id: session_id.clone(),
                    selected: memory_selection.selected.clone(),
                },
            );
            let mcp_context_result = build_mcp_context(
                &s.harness,
                &mcp_context_selections,
                &app_handle,
                &session_id,
            )
            .await;
            let ready_connector_labels = mcp_context_result.ready_labels.clone();
            let prepared = prepare_send_input_turn_context(PrepareSendInputTurnRequest {
                session_id: &session_id,
                session: &s,
                text: &text,
                input_intent,
                workflow: &workflow,
                ready_connector_labels,
                memory_context: memory_selection.context,
                wiki_context: project_records.context,
                connector_context: mcp_context_result.context,
            })
            .await;
            let result = s
                .send_message_with_reserved_turn(
                    &text,
                    &app_handle,
                    prepared.hidden_contexts,
                    Some(prepared.turn_metadata),
                    Some(&prepared.activation_text),
                    turn_guard,
                )
                .await;
            if let Err(error) = save_session_snapshot_with_workflow(&state, &s).await {
                crate::app_log!("WARN", "[session_snapshot] {}", error);
            }
            let latest_turn_for_delivery = s.snapshot().latest_turn;
            if result.is_ok() {
                record_successful_send_input_continuity(
                    &state,
                    &app_handle,
                    &session_id,
                    &text,
                    &project_path,
                    latest_turn_for_delivery.as_ref(),
                )
                .await;
                let writeback = propose_send_input_project_record_update(
                    &state,
                    &session_id,
                    &text,
                    &project_path,
                    &workflow,
                    latest_turn_for_delivery.as_ref(),
                )
                .await;
                if let Some(proposal) = writeback.proposal {
                    crate::transcript::emit_stream_event(
                        &app_handle,
                        StreamEvent::ForgeWikiUpdateProposed {
                            session_id: session_id.clone(),
                            proposal,
                        },
                    );
                }
                build_store_emit_delivery_summary(
                    &state,
                    &app_handle,
                    &session_id,
                    latest_turn_for_delivery.as_ref(),
                    writeback.record_evidence,
                )
                .await;
                if let Err(error) = save_session_snapshot_with_workflow(&state, &s).await {
                    crate::app_log!("WARN", "[session_snapshot] {}", error);
                }
            } else {
                record_failed_send_input_continuity(
                    &state,
                    &session_id,
                    &text,
                    &project_path,
                    latest_turn_for_delivery.as_ref(),
                );
            }
            result
        }
        None => Err(format!("Session not found: {session_id}")),
    }
}

#[tauri::command]
pub async fn kill_session(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), String> {
    if let Some(s) = state.sessions.read().await.get(&session_id).cloned() {
        s.kill(&app_handle);
        let _ = s.harness.dispatch_session_stop_event(&session_id).await;
        if let Err(error) = save_session_snapshot_with_workflow(&state, &s).await {
            crate::app_log!("WARN", "[session_snapshot] {}", error);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_session(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), String> {
    if let Some(s) = state.sessions.read().await.get(&session_id).cloned() {
        s.kill(&app_handle);
        let _ = s.harness.dispatch_session_stop_event(&session_id).await;
    }
    state.unregister_session(&session_id).await;
    state.workflow_states.write().await.remove(&session_id);
    state.delivery_states.write().await.remove(&session_id);
    if let Err(error) = delete_session_snapshot(&session_id) {
        crate::app_log!("WARN", "[session_snapshot] {}", error);
    }
    if let Err(error) = crate::transcript::delete_transcript(&session_id) {
        crate::app_log!("WARN", "[transcript] {}", error);
    }
    Ok(())
}

#[tauri::command]
pub async fn list_sessions(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<SessionInfo>, String> {
    let snapshots = list_session_snapshots()?;
    Ok(list_session_infos_for_state(&state, snapshots).await)
}

async fn list_session_infos_for_state(
    state: &Arc<AppState>,
    snapshots: Vec<AgentSessionSnapshot>,
) -> Vec<SessionInfo> {
    let mut by_id = std::collections::HashMap::new();
    for snapshot in snapshots {
        by_id.insert(
            snapshot.session_id.clone(),
            SessionInfo {
                id: snapshot.session_id,
                provider: snapshot.provider,
                model: snapshot.model,
                status: "stopped".to_string(),
                created_at: String::new(),
                working_dir: Some(snapshot.working_dir),
                created_at_ms: Some(snapshot.created_at_ms),
                updated_at_ms: Some(snapshot.updated_at_ms),
                context_window_tokens: snapshot.context_window_tokens,
                latest_workflow: snapshot.latest_workflow,
                latest_delivery: snapshot.latest_delivery,
            },
        );
    }

    let sessions = state.sessions.read().await;
    let workflow_states = state.workflow_states.read().await;
    let delivery_states = state.delivery_states.read().await;
    for (id, session) in sessions.iter() {
        let status = session.status.lock();
        let snapshot = session.snapshot();
        by_id.insert(
            id.clone(),
            SessionInfo {
                id: id.clone(),
                provider: session.agent_type.clone(),
                model: session.model_id.clone(),
                status: status.as_str().to_string(),
                created_at: String::new(),
                working_dir: Some(snapshot.working_dir),
                created_at_ms: Some(snapshot.created_at_ms),
                updated_at_ms: Some(snapshot.updated_at_ms),
                context_window_tokens: snapshot.context_window_tokens,
                latest_workflow: workflow_states
                    .get(id)
                    .cloned()
                    .or(snapshot.latest_workflow),
                latest_delivery: delivery_states
                    .get(id)
                    .cloned()
                    .or(snapshot.latest_delivery),
            },
        );
    }

    let mut result: Vec<_> = by_id.into_values().collect();
    result.sort_by(|a, b| {
        b.updated_at_ms
            .unwrap_or(0)
            .cmp(&a.updated_at_ms.unwrap_or(0))
    });
    result
}

#[tauri::command]
pub async fn confirm_response(
    state: tauri::State<'_, Arc<AppState>>,
    block_id: String,
    approved: bool,
) -> Result<(), String> {
    let sender = { state.pending_confirms.write().await.remove(&block_id) };
    match sender {
        Some(tx) => tx
            .send(approved)
            .map_err(|_| format!("Confirm receiver already closed for: {block_id}")),
        None => Err(format!("No pending confirm for: {block_id}")),
    }
}

#[tauri::command]
pub async fn list_continuity_experiences(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<Vec<ExperienceMemory>, String> {
    list_continuity_experiences_for_request(&state, session_id.as_deref(), working_dir.as_deref())
        .await
}

async fn list_continuity_experiences_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<Vec<ExperienceMemory>, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let project_path = working_dir.to_string_lossy().to_string();
    state.continuity.list_experiences_for_project(&project_path)
}

#[tauri::command]
pub async fn search_continuity_experiences(
    state: tauri::State<'_, Arc<AppState>>,
    query: String,
    session_id: Option<String>,
    working_dir: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ExperienceMemory>, String> {
    search_continuity_experiences_for_request(
        &state,
        session_id.as_deref(),
        working_dir.as_deref(),
        &query,
        limit,
    )
    .await
}

async fn search_continuity_experiences_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
    query: &str,
    limit: Option<usize>,
) -> Result<Vec<ExperienceMemory>, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let project_path = working_dir.to_string_lossy().to_string();
    let limit = limit
        .unwrap_or(CONTINUITY_RECALL_DEFAULT_LIMIT)
        .min(CONTINUITY_RECALL_MAX_LIMIT);
    state
        .continuity
        .search_experiences_for_project(&project_path, query, limit)
}

#[tauri::command]
pub async fn search_workspace_files(
    state: tauri::State<'_, Arc<AppState>>,
    query: String,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<Vec<String>, String> {
    search_workspace_files_for_request(
        &state,
        &query,
        session_id.as_deref(),
        working_dir.as_deref(),
    )
    .await
}

async fn search_workspace_files_for_request(
    state: &Arc<AppState>,
    query: &str,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<Vec<String>, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let results = find_files(&working_dir, query, 20);
    Ok(results)
}

#[tauri::command]
pub async fn get_default_working_dir(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<String, String> {
    Ok(state.harness.working_dir.to_string_lossy().to_string())
}

/// Preview a small slice of a file around a target line inside the app.
#[tauri::command]
pub async fn preview_file(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
    line: Option<u32>,
    context: Option<u32>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<FilePreview, String> {
    preview_file_for_request(
        &state,
        &path,
        line,
        context,
        session_id.as_deref(),
        working_dir.as_deref(),
    )
    .await
}

async fn preview_file_for_request(
    state: &Arc<AppState>,
    path: &str,
    line: Option<u32>,
    context: Option<u32>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<FilePreview, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let full_path = resolve_workspace_file_path(&working_dir, path)?;

    crate::app_log!(
        "INFO",
        "[preview_file] request path={} line={:?} resolved={}",
        path,
        line,
        full_path.display()
    );

    if !full_path.exists() {
        return Err(format!("File not found: {}", full_path.display()));
    }
    if !full_path.is_file() {
        return Err(format!("Not a file: {}", full_path.display()));
    }

    let metadata = std::fs::metadata(&full_path)
        .map_err(|e| format!("Unable to read file metadata: {}", e))?;
    if metadata.len() > 2_000_000 {
        return Err("File is too large to preview safely.".into());
    }

    let content = std::fs::read_to_string(&full_path)
        .map_err(|_| "This file is not plain text, so it cannot be previewed here.".to_string())?;

    let all_lines: Vec<&str> = content.lines().collect();
    let total_lines = all_lines.len().max(1);
    let target_line = line.map(|l| l.max(1) as usize);
    let context = context.unwrap_or(40).clamp(5, 160) as usize;

    let (start_line, end_line) = if let Some(target) = target_line {
        let target = target.min(total_lines);
        (
            target.saturating_sub(context).max(1),
            (target + context).min(total_lines),
        )
    } else {
        (1, (context * 2).min(total_lines))
    };

    let lines = (start_line..=end_line)
        .map(|number| FilePreviewLine {
            number,
            content: all_lines.get(number - 1).copied().unwrap_or("").to_string(),
            is_target: target_line.map(|target| target == number).unwrap_or(false),
        })
        .collect::<Vec<_>>();

    let display_path = full_path
        .strip_prefix(&working_dir)
        .unwrap_or(&full_path)
        .to_string_lossy()
        .to_string();

    Ok(FilePreview {
        path: full_path.to_string_lossy().to_string(),
        display_path,
        requested_line: line,
        start_line,
        total_lines,
        lines,
    })
}

/// Open a file in the system's default editor at a specific line.
#[tauri::command]
pub async fn open_file(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
    line: Option<u32>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<(), String> {
    let full_path =
        open_file_target_for_request(&state, &path, session_id.as_deref(), working_dir.as_deref())
            .await?;

    crate::app_log!(
        "INFO",
        "[open_file] request path={} line={:?} resolved={}",
        path,
        line,
        full_path.display()
    );

    let path_str = full_path.to_string_lossy().to_string();

    #[cfg(target_os = "macos")]
    {
        open_file_macos(&path_str, line)?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (path_str, line);
        return Err("open_file is only supported on macOS currently".into());
    }

    Ok(())
}

async fn open_file_target_for_request(
    state: &Arc<AppState>,
    path: &str,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<PathBuf, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let full_path = resolve_workspace_file_path(&working_dir, path)?;
    if !full_path.exists() {
        let message = format!("File not found: {}", full_path.display());
        crate::app_log!("WARN", "[open_file] {}", message);
        return Err(message);
    }
    Ok(full_path)
}

async fn working_dir_for_request_or_explicit(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<std::path::PathBuf, String> {
    resolve_bound_working_dir(state, session_id, working_dir).await
}
#[tauri::command]
pub async fn get_api_key_status() -> Result<Vec<settings::KeyStatus>, String> {
    Ok(settings::Settings::load().key_status())
}

#[tauri::command]
pub async fn set_api_key(provider: String, key: String) -> Result<(), String> {
    settings::Settings::load().set_api_key(&provider, &key)
}

#[cfg(test)]
#[path = "handlers_tests.rs"]
mod tests;
