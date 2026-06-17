use std::sync::Arc;

use tokio::sync::Notify;

use crate::agent::event_sink::EventEmitter;
use crate::agent::session::AgentSession;
use crate::agent::session_guards::lock_unpoisoned;
use crate::agent::time::now_ms;
use crate::agent::tool_results::{build_tool_result_message_for_model, is_read_only_tool};
use crate::agent::turn_state::{completed_tool_trace, is_errorish_tool_result, running_tool_trace};

impl AgentSession {
    /// Execute a batch of tool calls: sub-agents, read tools in parallel, write tools sequentially.
    pub(crate) async fn execute_tools(
        &self,
        tool_calls: &[crate::adapters::base::ToolCall],
        emitter: &dyn EventEmitter,
        app_handle: Option<&tauri::AppHandle>,
        tool_emitter_override: Option<Arc<dyn EventEmitter>>,
        cancel: Arc<Notify>,
    ) {
        let (delegated, regular): (Vec<_>, Vec<_>) =
            tool_calls.iter().partition(|tc| tc.name == "delegate_task");

        let mut sub_results: Vec<(usize, String)> = Vec::new();
        if !delegated.is_empty() {
            let mut handles = Vec::new();
            for tc in &delegated {
                let started_at_ms = now_ms();
                self.record_latest_tool_emitter(
                    running_tool_trace(tc.id.clone(), tc.name.clone(), &tc.input, started_at_ms),
                    emitter,
                );
                let task = tc
                    .input
                    .get("task")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Investigate and report findings")
                    .to_string();
                let mode = tc
                    .input
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("research")
                    .to_string();
                let execution_mode = match mode.as_str() {
                    "patch_proposal" => crate::agent::a2a::types::AgentExecutionMode::PatchProposal,
                    "worktree_worker" => {
                        crate::agent::a2a::types::AgentExecutionMode::WorktreeWorker
                    }
                    _ => crate::agent::a2a::types::AgentExecutionMode::ReadOnly,
                };
                let idx = tool_calls.iter().position(|t| t.id == tc.id).unwrap_or(0);
                let a2a_task_id_result = {
                    let mut bus = lock_unpoisoned(&self.a2a_bus);
                    match delegate_parent_task_id_from_input(&tc.input, &bus) {
                        Ok(parent_task_id) => match (&execution_mode, parent_task_id.as_ref()) {
                            (
                                crate::agent::a2a::types::AgentExecutionMode::PatchProposal,
                                Some(parent_task_id),
                            ) => crate::agent::a2a::supervisor::assign_patch_proposal_child_task(
                                &mut bus,
                                parent_task_id,
                                "Delegated patch proposal task",
                                &task,
                                started_at_ms,
                            ),
                            (
                                crate::agent::a2a::types::AgentExecutionMode::WorktreeWorker,
                                Some(parent_task_id),
                            ) => crate::agent::a2a::supervisor::assign_worktree_worker_child_task(
                                &mut bus,
                                parent_task_id,
                                "Delegated worktree worker task",
                                &task,
                                started_at_ms,
                            ),
                            (_, Some(parent_task_id)) => {
                                crate::agent::a2a::supervisor::assign_delegate_child_task(
                                    &mut bus,
                                    parent_task_id,
                                    "Delegated research task",
                                    &task,
                                    started_at_ms,
                                )
                            }
                            (crate::agent::a2a::types::AgentExecutionMode::PatchProposal, None) => {
                                Ok(crate::agent::a2a::supervisor::assign_patch_proposal_task(
                                    &mut bus,
                                    "Delegated patch proposal task",
                                    &task,
                                    started_at_ms,
                                ))
                            }
                            (
                                crate::agent::a2a::types::AgentExecutionMode::WorktreeWorker,
                                None,
                            ) => Ok(crate::agent::a2a::supervisor::assign_worktree_worker_task(
                                &mut bus,
                                "Delegated worktree worker task",
                                &task,
                                started_at_ms,
                            )),
                            _ => Ok(crate::agent::a2a::supervisor::assign_delegate_task(
                                &mut bus,
                                "Delegated research task",
                                &task,
                                started_at_ms,
                            )),
                        },
                        Err(message) => Err(message),
                    }
                };
                let a2a_task_id = match a2a_task_id_result {
                    Ok(task_id) => task_id,
                    Err(message) => {
                        emitter.emit(self.tool_call_result_event(&tc.id, &message, true, 0));
                        self.record_latest_tool_emitter(
                            completed_tool_trace(
                                tc.id.clone(),
                                tc.name.clone(),
                                &tc.input,
                                &message,
                                started_at_ms,
                                now_ms(),
                            ),
                            emitter,
                        );
                        sub_results.push((idx, message));
                        continue;
                    }
                };
                self.emit_a2a_projection(emitter);
                let adapter = self.adapter.clone();
                let harness = self.harness.clone();
                let cancel = lock_unpoisoned(&self.cancel)
                    .clone()
                    .unwrap_or_else(|| Arc::new(Notify::new()));
                let wd = self.harness.working_dir.clone();
                let sub_emitter: Arc<dyn EventEmitter> = if let Some(app) = app_handle {
                    Arc::new(crate::agent::event_sink::TauriEventEmitter::new(
                        app.clone(),
                    ))
                } else if let Some(shared) = tool_emitter_override.clone() {
                    shared
                } else {
                    Arc::new(crate::agent::event_sink::NoopEventEmitter)
                };
                {
                    let mut bus = lock_unpoisoned(&self.a2a_bus);
                    bus.start_task(&a2a_task_id, now_ms());
                }
                self.sync_goal_task_for_a2a(crate::agent::goal_state::GoalTaskStatus::InProgress);
                self.emit_a2a_projection(emitter);
                let execution_mode_clone = execution_mode.clone();
                let worktree_id = a2a_task_id.as_str().to_string();
                handles.push((
                    idx,
                    tc.id.clone(),
                    tc.name.clone(),
                    tc.input.clone(),
                    started_at_ms,
                    a2a_task_id,
                    execution_mode,
                    tokio::spawn(async move {
                        let r = match execution_mode_clone {
                            crate::agent::a2a::types::AgentExecutionMode::PatchProposal => {
                                crate::agent::a2a::child::ChildAgentRuntime::run_patch_proposal(
                                    &task,
                                    adapter,
                                    harness,
                                    sub_emitter,
                                    cancel,
                                    &wd,
                                )
                                .await
                            }
                            crate::agent::a2a::types::AgentExecutionMode::WorktreeWorker => {
                                crate::agent::a2a::child::ChildAgentRuntime::run_worktree_worker(
                                    &worktree_id,
                                    &task,
                                    adapter,
                                    harness,
                                    sub_emitter,
                                    cancel,
                                    &wd,
                                )
                                .await
                            }
                            _ => {
                                crate::agent::a2a::child::ChildAgentRuntime::run_read_only(
                                    &task,
                                    adapter,
                                    harness,
                                    sub_emitter,
                                    cancel,
                                    &wd,
                                )
                                .await
                            }
                        };
                        (idx, r)
                    }),
                ));
            }
            for (
                fallback_idx,
                id,
                name,
                input,
                started_at_ms,
                a2a_task_id,
                execution_mode,
                handle,
            ) in handles
            {
                match handle.await {
                    Ok((idx, r)) => {
                        let api_text: String = match execution_mode {
                            crate::agent::a2a::types::AgentExecutionMode::WorktreeWorker => {
                                crate::agent::a2a::supervisor::worktree_result_for_model(&r)
                            }
                            _ => crate::agent::a2a::supervisor::delegate_result_for_model(&r),
                        };
                        match execution_mode {
                            crate::agent::a2a::types::AgentExecutionMode::PatchProposal => {
                                if let Some(proposal) =
                                    crate::agent::a2a::supervisor::extract_patch_proposal(&r)
                                {
                                    let artifact = crate::agent::a2a::types::AgentArtifact {
                                        artifact_id: format!("proposal-{}", a2a_task_id.as_str()),
                                        task_id: a2a_task_id.clone(),
                                        kind:
                                            crate::agent::a2a::types::AgentArtifactKind::PatchProposal,
                                        title: format!("Patch: {}", proposal.file_path),
                                        content: serde_json::to_string(&proposal).unwrap_or_default(),
                                        created_at_ms: now_ms(),
                                    };
                                    {
                                        let mut bus = lock_unpoisoned(&self.a2a_bus);
                                        bus.add_artifact(&a2a_task_id, artifact, now_ms());
                                        bus.complete_task(&a2a_task_id, &api_text, now_ms());
                                    }
                                } else {
                                    let mut bus = lock_unpoisoned(&self.a2a_bus);
                                    bus.complete_task(&a2a_task_id, &api_text, now_ms());
                                }
                            }
                            crate::agent::a2a::types::AgentExecutionMode::WorktreeWorker => {
                                let artifacts =
                                    crate::agent::a2a::supervisor::extract_worktree_artifacts(
                                        &r,
                                        &a2a_task_id,
                                    );
                                {
                                    let mut bus = lock_unpoisoned(&self.a2a_bus);
                                    for artifact in artifacts {
                                        bus.add_artifact(&a2a_task_id, artifact, now_ms());
                                    }
                                    bus.complete_task(&a2a_task_id, &api_text, now_ms());
                                }
                            }
                            _ => {
                                let mut bus = lock_unpoisoned(&self.a2a_bus);
                                bus.complete_task(&a2a_task_id, &api_text, now_ms());
                            }
                        }
                        self.sync_goal_task_for_a2a(
                            crate::agent::goal_state::GoalTaskStatus::Completed,
                        );
                        self.emit_a2a_projection(emitter);
                        emitter.emit(self.tool_call_result_event(&id, &r, false, 0));
                        self.record_latest_tool_emitter(
                            completed_tool_trace(
                                id.clone(),
                                name.clone(),
                                &input,
                                &r,
                                started_at_ms,
                                now_ms(),
                            ),
                            emitter,
                        );
                        sub_results.push((idx, api_text));
                    }
                    Err(err) => {
                        let message =
                            crate::agent::session_guards::sub_agent_join_error_message(&err);
                        {
                            let mut bus = lock_unpoisoned(&self.a2a_bus);
                            crate::agent::a2a::supervisor::record_child_failure(
                                &mut bus,
                                &a2a_task_id,
                                "join_error",
                                &message,
                                now_ms(),
                            );
                        }
                        self.emit_a2a_projection(emitter);
                        emitter.emit(self.tool_call_result_event(&id, &message, true, 0));
                        self.record_latest_tool_emitter(
                            completed_tool_trace(
                                id,
                                name,
                                &input,
                                &message,
                                started_at_ms,
                                now_ms(),
                            ),
                            emitter,
                        );
                        sub_results.push((fallback_idx, message));
                    }
                }
            }
        }

        let (reads, writes): (
            Vec<&crate::adapters::base::ToolCall>,
            Vec<&crate::adapters::base::ToolCall>,
        ) = regular.iter().partition(|tc| is_read_only_tool(&tc.name));

        let mut read_results: Vec<(String, String)> = Vec::new();
        {
            let mut handles = Vec::new();
            for tc in &reads {
                let h = self.harness.clone();
                let sid = self.id.clone();
                let name = tc.name.clone();
                let input = tc.input.clone();
                let tool_emitter: Arc<dyn EventEmitter> = if let Some(app) = app_handle {
                    Arc::new(crate::agent::event_sink::TauriEventEmitter::new(
                        app.clone(),
                    ))
                } else if let Some(shared) = tool_emitter_override.clone() {
                    shared
                } else {
                    Arc::new(crate::agent::event_sink::NoopEventEmitter)
                };
                let id = tc.id.clone();
                let started_at_ms = now_ms();
                let cancel_for_tool = cancel.clone();
                self.record_latest_tool_emitter(
                    running_tool_trace(id.clone(), name.clone(), &input, started_at_ms),
                    emitter,
                );
                handles.push(tokio::spawn(async move {
                    let result = h
                        .execute_tool_with_emitter(
                            &sid,
                            &name,
                            &input,
                            tool_emitter,
                            Some(&id),
                            Some(cancel_for_tool),
                        )
                        .await;
                    (id, name, input, started_at_ms, now_ms(), result)
                }));
            }
            for handle in handles {
                if let Ok((id, name, input, started_at_ms, ended_at_ms, result)) = handle.await {
                    self.record_latest_tool_emitter(
                        completed_tool_trace(
                            id.clone(),
                            name,
                            &input,
                            &result,
                            started_at_ms,
                            ended_at_ms,
                        ),
                        emitter,
                    );
                    read_results.push((id, result));
                }
            }
        }

        let mut write_results: Vec<(String, String)> = Vec::new();
        for tc in &writes {
            let started_at_ms = now_ms();
            self.record_latest_tool_emitter(
                running_tool_trace(tc.id.clone(), tc.name.clone(), &tc.input, started_at_ms),
                emitter,
            );
            let tool_emitter: Arc<dyn EventEmitter> = if let Some(app) = app_handle {
                Arc::new(crate::agent::event_sink::TauriEventEmitter::new(
                    app.clone(),
                ))
            } else if let Some(shared) = tool_emitter_override.clone() {
                shared
            } else {
                Arc::new(crate::agent::event_sink::NoopEventEmitter)
            };
            let result = self
                .harness
                .execute_tool_with_emitter(
                    &self.id,
                    &tc.name,
                    &tc.input,
                    tool_emitter,
                    Some(&tc.id),
                    Some(cancel.clone()),
                )
                .await;
            self.record_latest_tool_emitter(
                completed_tool_trace(
                    tc.id.clone(),
                    tc.name.clone(),
                    &tc.input,
                    &result,
                    started_at_ms,
                    now_ms(),
                ),
                emitter,
            );
            write_results.push((tc.id.clone(), result));
        }

        let mut result_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for (id, r) in read_results {
            result_map.insert(id, r);
        }
        for (id, r) in write_results {
            result_map.insert(id, r);
        }
        for (idx, r) in sub_results {
            if let Some(tc) = tool_calls.get(idx) {
                result_map.insert(tc.id.clone(), r);
            }
        }

        self.record_tool_counts_emitter(tool_calls, &result_map, emitter);

        let model_tool_results = build_tool_result_message_for_model(&result_map, tool_calls);
        for resolved in &model_tool_results.results {
            if resolved.missing {
                let Some(tc) = tool_calls.iter().find(|tc| tc.id == resolved.tool_call_id) else {
                    continue;
                };
                self.record_latest_tool_emitter(
                    completed_tool_trace(
                        tc.id.clone(),
                        tc.name.clone(),
                        &tc.input,
                        &resolved.content,
                        now_ms(),
                        now_ms(),
                    ),
                    emitter,
                );
            }
            crate::app_log!(
                "INFO",
                "Agent tool '{}' result ({} chars)",
                resolved.tool_name,
                resolved.content.len()
            );
        }
        lock_unpoisoned(&self.messages).push(model_tool_results.message);
    }

    fn record_tool_counts_emitter(
        &self,
        tool_calls: &[crate::adapters::base::ToolCall],
        result_map: &std::collections::HashMap<String, String>,
        emitter: &dyn EventEmitter,
    ) {
        let batch_total = tool_calls.len();
        let batch_failed = tool_calls
            .iter()
            .filter(|tc| {
                result_map
                    .get(&tc.id)
                    .map(|r| is_errorish_tool_result(r))
                    .unwrap_or(true)
            })
            .count();
        let (cumulative_total, cumulative_failed) = {
            let mut turn = lock_unpoisoned(&self.latest_turn);
            if let Some(turn) = turn.as_mut() {
                turn.tool_call_count += batch_total;
                turn.failed_tool_count += batch_failed;
                (turn.tool_call_count, turn.failed_tool_count)
            } else {
                (batch_total, batch_failed)
            }
        };
        lock_unpoisoned(&self.turn_metrics).record_tool_calls(cumulative_total, cumulative_failed);
        {
            let mut loop_guard = lock_unpoisoned(&self.loop_guard);
            loop_guard.record_tool_calls(batch_total);
            if batch_total > 0 {
                loop_guard.record_tool_batch(
                    tool_batch_signature(tool_calls),
                    tool_category_signature(tool_calls),
                    batch_failed < batch_total,
                );
            }
        }
        self.emit_with_emitter(emitter);
    }
}

pub(crate) fn delegate_parent_task_id_from_input(
    input: &serde_json::Value,
    bus: &crate::agent::a2a::bus::AgentA2ABus,
) -> Result<Option<crate::agent::a2a::types::AgentTaskId>, String> {
    let Some(parent_task_id) = input
        .get("parent_task_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let parent_task_id = crate::agent::a2a::types::AgentTaskId::new(parent_task_id);
    if bus.task(&parent_task_id).is_some() {
        Ok(Some(parent_task_id))
    } else {
        Err(format!(
            "parent_task_id '{}' does not match an existing A2A task",
            parent_task_id.as_str()
        ))
    }
}

pub(crate) fn tool_batch_signature(tool_calls: &[crate::adapters::base::ToolCall]) -> String {
    let mut parts = tool_calls
        .iter()
        .map(|tool_call| format!("{}:{}", tool_call.name, canonical_json(&tool_call.input)))
        .collect::<Vec<_>>();
    parts.sort();
    parts.join("\n")
}

/// Category-level signature that ignores file paths and command arguments,
/// so reading different files or running different shell commands still
/// counts as the same category batch.  Used to catch "keep exploring"
/// loops where the exact tool input changes but the overall pattern
/// (e.g. only read_file, only run_shell) repeats.
pub(crate) fn tool_category_signature(tool_calls: &[crate::adapters::base::ToolCall]) -> String {
    let mut names: Vec<String> = tool_calls.iter().map(|tc| tc.name.clone()).collect();
    names.sort();
    names.dedup();
    names.join(",")
}

pub(crate) fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(items) => {
            let body = items
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",");
            format!("[{body}]")
        }
        serde_json::Value::Object(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            let body = keys
                .into_iter()
                .map(|key| {
                    let encoded_key =
                        serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string());
                    let encoded_value = canonical_json(&map[key]);
                    format!("{encoded_key}:{encoded_value}")
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{body}}}")
        }
        _ => serde_json::to_string(value).unwrap_or_else(|_| "null".to_string()),
    }
}
