use crate::agent::a2a::projection::{
    AgentA2AChildCapsule, AgentA2AChildEventKind, AgentA2AChildRuntimeEvent,
    AgentA2AMessageProjection, AgentA2AProjection, AgentA2ARecoveryActionKind,
    AgentA2ARecoveryActionSuggestion, AgentA2AReviewGateKind, AgentA2AReviewGateProjection,
    AgentA2ATaskProjection, AgentFileIoEventProjection,
};
use crate::agent::a2a::types::{
    AgentArtifact, AgentExecutionMode, AgentId, AgentMessage, AgentMessageKind,
    AgentParentSessionContext, AgentRole, AgentTaskFailure, AgentTaskId, AgentTaskRecord,
    AgentTaskStatus,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentReviewDecision {
    Approve,
    Reject,
}

impl AgentReviewDecision {
    fn metadata_value(self) -> &'static str {
        match self {
            Self::Approve => "approved",
            Self::Reject => "rejected",
        }
    }

    fn suggested_action(self) -> &'static str {
        match self {
            Self::Approve => "Review approved by controller.",
            Self::Reject => "Review rejected by controller. Do not merge this worktree.",
        }
    }

    fn message_prefix(self) -> &'static str {
        match self {
            Self::Approve => "Review approved",
            Self::Reject => "Review rejected",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AgentA2ABus {
    pub tasks: Vec<AgentTaskRecord>,
    pub messages: Vec<AgentMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_session_context: Option<AgentParentSessionContext>,
    next_task_index: u64,
    next_agent_index: u64,
    next_message_index: u64,
}

impl AgentA2ABus {
    pub(crate) fn assign_task(
        &mut self,
        role: AgentRole,
        execution_mode: AgentExecutionMode,
        title: impl Into<String>,
        prompt: impl Into<String>,
        timestamp_ms: u64,
    ) -> AgentTaskId {
        self.next_task_index += 1;
        self.next_agent_index += 1;
        let task_id = AgentTaskId::new(format!("a2a-task-{}", self.next_task_index));
        let agent_id = AgentId::new(format!("a2a-agent-{}", self.next_agent_index));
        let title = title.into();
        let prompt = prompt.into();
        let record = AgentTaskRecord::new(
            task_id.clone(),
            agent_id.clone(),
            role,
            execution_mode,
            title.clone(),
            prompt,
            timestamp_ms,
        );
        self.tasks.push(record);
        self.push_message(
            task_id.clone(),
            agent_id,
            AgentMessageKind::TaskAssigned,
            title,
            timestamp_ms,
        );
        task_id
    }

    pub(crate) fn assign_child_task(
        &mut self,
        parent_task_id: &AgentTaskId,
        role: AgentRole,
        execution_mode: AgentExecutionMode,
        title: impl Into<String>,
        prompt: impl Into<String>,
        timestamp_ms: u64,
    ) -> Result<AgentTaskId, String> {
        let Some(parent_index) = self
            .tasks
            .iter()
            .position(|task| task.task_id == *parent_task_id)
        else {
            return Err(format!(
                "parent task '{}' does not exist",
                parent_task_id.as_str()
            ));
        };

        self.next_task_index += 1;
        self.next_agent_index += 1;
        let task_id = AgentTaskId::new(format!("a2a-task-{}", self.next_task_index));
        let agent_id = AgentId::new(format!("a2a-agent-{}", self.next_agent_index));
        let title = title.into();
        let prompt = prompt.into();
        let mut record = AgentTaskRecord::new(
            task_id.clone(),
            agent_id.clone(),
            role,
            execution_mode,
            title.clone(),
            prompt,
            timestamp_ms,
        );
        record.parent_task_id = Some(parent_task_id.clone());
        if self.tasks[parent_index].record_child_task_id(task_id.clone()) {
            self.tasks[parent_index].updated_at_ms = timestamp_ms;
        }
        self.tasks.push(record);
        self.push_message(
            task_id.clone(),
            agent_id,
            AgentMessageKind::TaskAssigned,
            title,
            timestamp_ms,
        );
        Ok(task_id)
    }

    pub(crate) fn task(&self, task_id: &AgentTaskId) -> Option<&AgentTaskRecord> {
        self.tasks.iter().find(|task| task.task_id == *task_id)
    }

    pub(crate) fn set_parent_session_context(&mut self, context: AgentParentSessionContext) {
        self.parent_session_context = Some(context);
    }

    pub(crate) fn parent_session_context(&self) -> Option<&AgentParentSessionContext> {
        self.parent_session_context.as_ref()
    }

    pub(crate) fn claim_task_lease(
        &mut self,
        task_id: &AgentTaskId,
        owner: impl Into<String>,
        timestamp_ms: u64,
        lease_duration_ms: u64,
    ) -> bool {
        let owner = owner.into();
        let Some(result) = self.update_task(task_id, timestamp_ms, |task| {
            if !is_lease_claimable_status(&task.status) {
                return None;
            }
            if let Some(current_owner) = task.lease_owner.as_deref() {
                if current_owner != owner && !lease_is_expired(task, timestamp_ms) {
                    return None;
                }
            }

            task.updated_at_ms = timestamp_ms;
            task.status = AgentTaskStatus::Running;
            task.started_at_ms.get_or_insert(timestamp_ms);
            task.ended_at_ms = None;
            task.resume_note = None;
            task.lease_owner = Some(owner.clone());
            task.lease_acquired_at_ms = Some(timestamp_ms);
            task.lease_expires_at_ms = Some(lease_expires_at(timestamp_ms, lease_duration_ms));
            task.last_heartbeat_at_ms = Some(timestamp_ms);
            task.attempt_count = task.attempt_count.saturating_add(1);
            Some((task.agent_id.clone(), task.task_id.clone()))
        }) else {
            return false;
        };
        let Some((agent_id, task_id_for_message)) = result else {
            return false;
        };
        self.push_message(
            task_id_for_message,
            agent_id,
            AgentMessageKind::LeaseClaimed,
            format!("Lease claimed by {owner}"),
            timestamp_ms,
        );
        true
    }

    pub(crate) fn heartbeat_task_lease(
        &mut self,
        task_id: &AgentTaskId,
        owner: &str,
        timestamp_ms: u64,
        lease_duration_ms: u64,
    ) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|task| task.task_id == *task_id) else {
            return false;
        };
        if task.lease_owner.as_deref() != Some(owner) || lease_is_expired(task, timestamp_ms) {
            return false;
        }

        task.updated_at_ms = timestamp_ms;
        task.last_heartbeat_at_ms = Some(timestamp_ms);
        task.lease_expires_at_ms = Some(lease_expires_at(timestamp_ms, lease_duration_ms));
        true
    }

    pub(crate) fn cancel_task(
        &mut self,
        task_id: &AgentTaskId,
        message: impl Into<String>,
        timestamp_ms: u64,
    ) -> bool {
        let message = message.into();
        let Some(agent_id) = self.update_task(task_id, timestamp_ms, |task| {
            if is_terminal_status(&task.status) {
                return None;
            }
            task.status = AgentTaskStatus::Cancelled;
            task.ended_at_ms = Some(timestamp_ms);
            task.failure = None;
            task.resume_note = None;
            clear_active_lease(task);
            Some(task.agent_id.clone())
        }) else {
            return false;
        };
        let Some(agent_id) = agent_id else {
            return false;
        };
        self.push_message(
            task_id.clone(),
            agent_id,
            AgentMessageKind::Cancelled,
            message,
            timestamp_ms,
        );
        true
    }

    pub(crate) fn retry_task(&mut self, task_id: &AgentTaskId, timestamp_ms: u64) -> bool {
        let Some(agent_id) = self.update_task(task_id, timestamp_ms, |task| {
            if !matches!(
                task.status,
                AgentTaskStatus::Failed | AgentTaskStatus::Interrupted
            ) {
                return None;
            }
            if !task
                .failure
                .as_ref()
                .map(|failure| failure.retryable)
                .unwrap_or(false)
            {
                return None;
            }
            if task.attempt_count >= task.max_attempts {
                return None;
            }

            task.status = AgentTaskStatus::Pending;
            task.failure = None;
            task.ended_at_ms = None;
            task.resume_note = None;
            clear_active_lease(task);
            Some(task.agent_id.clone())
        }) else {
            return false;
        };
        let Some(agent_id) = agent_id else {
            return false;
        };
        self.push_message(
            task_id.clone(),
            agent_id,
            AgentMessageKind::Progress,
            "Retry scheduled".to_string(),
            timestamp_ms,
        );
        true
    }

    pub(crate) fn start_task(&mut self, task_id: &AgentTaskId, timestamp_ms: u64) {
        let Some(agent_id) = self.update_task(task_id, timestamp_ms, |task| {
            task.status = AgentTaskStatus::Running;
            task.started_at_ms = Some(timestamp_ms);
            task.resume_note = None;
            task.agent_id.clone()
        }) else {
            return;
        };
        self.push_message(
            task_id.clone(),
            agent_id,
            AgentMessageKind::Started,
            "Started".to_string(),
            timestamp_ms,
        );
    }

    pub(crate) fn record_progress(
        &mut self,
        task_id: &AgentTaskId,
        message: impl Into<String>,
        timestamp_ms: u64,
    ) {
        let Some(agent_id) = self.task(task_id).map(|task| task.agent_id.clone()) else {
            return;
        };
        self.push_message(
            task_id.clone(),
            agent_id,
            AgentMessageKind::Progress,
            message.into(),
            timestamp_ms,
        );
    }

    pub(crate) fn complete_task(
        &mut self,
        task_id: &AgentTaskId,
        result: impl Into<String>,
        timestamp_ms: u64,
    ) {
        self.complete_task_with_artifacts(task_id, result, Vec::new(), timestamp_ms);
    }

    pub(crate) fn complete_task_with_artifacts(
        &mut self,
        task_id: &AgentTaskId,
        result: impl Into<String>,
        artifacts: Vec<AgentArtifact>,
        timestamp_ms: u64,
    ) {
        let result = result.into();
        let Some(agent_id) = self.update_task(task_id, timestamp_ms, |task| {
            task.status = AgentTaskStatus::Completed;
            task.ended_at_ms = Some(timestamp_ms);
            clear_active_lease(task);
            task.artifacts.extend(artifacts);
            task.agent_id.clone()
        }) else {
            return;
        };
        self.push_message(
            task_id.clone(),
            agent_id,
            AgentMessageKind::FinalResult,
            result,
            timestamp_ms,
        );
    }

    pub(crate) fn add_artifact(
        &mut self,
        task_id: &AgentTaskId,
        artifact: AgentArtifact,
        timestamp_ms: u64,
    ) {
        let Some(agent_id) = self.update_task(task_id, timestamp_ms, |task| {
            task.artifacts.push(artifact.clone());
            task.agent_id.clone()
        }) else {
            return;
        };
        self.push_message(
            task_id.clone(),
            agent_id,
            AgentMessageKind::ArtifactCreated,
            artifact.title.clone(),
            timestamp_ms,
        );
    }

    pub(crate) fn record_review_decision(
        &mut self,
        task_id: &AgentTaskId,
        decision: AgentReviewDecision,
        message: impl Into<String>,
        timestamp_ms: u64,
    ) -> Result<(), String> {
        let message = message.into();
        let Some(result) = self.update_task(task_id, timestamp_ms, |task| {
            if task.execution_mode != AgentExecutionMode::WorktreeWorker {
                return Err("Only worktree worker tasks can be reviewed".to_string());
            }
            let mut metadata = latest_worktree_metadata(task)
                .ok_or_else(|| "Task does not have worktree metadata".to_string())?;
            if metadata.get("needs_human_review").and_then(|v| v.as_bool()) != Some(true) {
                return Err("Task is not awaiting human review".to_string());
            }
            let Some(object) = metadata.as_object_mut() else {
                return Err("Worktree metadata is not an object".to_string());
            };
            object.insert("needs_human_review".to_string(), serde_json::json!(false));
            object.insert(
                "review_decision".to_string(),
                serde_json::json!(decision.metadata_value()),
            );
            object.insert(
                "reviewed_at_ms".to_string(),
                serde_json::json!(timestamp_ms),
            );
            if !message.trim().is_empty() {
                object.insert(
                    "review_message".to_string(),
                    serde_json::json!(message.clone()),
                );
            }
            object.insert(
                "suggested_action".to_string(),
                serde_json::json!(decision.suggested_action()),
            );

            task.artifacts.push(AgentArtifact {
                artifact_id: format!(
                    "review-{}-{}-{}",
                    decision.metadata_value(),
                    task.task_id.as_str(),
                    timestamp_ms
                ),
                task_id: task.task_id.clone(),
                kind: crate::agent::a2a::types::AgentArtifactKind::Evidence,
                title: "Worktree metadata".to_string(),
                content: metadata.to_string(),
                created_at_ms: timestamp_ms,
            });

            match decision {
                AgentReviewDecision::Approve => {
                    clear_active_lease(task);
                }
                AgentReviewDecision::Reject => {
                    task.status = AgentTaskStatus::Failed;
                    task.ended_at_ms = Some(timestamp_ms);
                    clear_active_lease(task);
                    task.failure = Some(AgentTaskFailure {
                        kind: "review_rejection".to_string(),
                        message: if message.trim().is_empty() {
                            "Review rejected".to_string()
                        } else {
                            message.clone()
                        },
                        retryable: false,
                        created_at_ms: timestamp_ms,
                    });
                }
            }

            Ok(task.agent_id.clone())
        }) else {
            return Err("Task not found".to_string());
        };
        let agent_id = result?;
        self.push_message(
            task_id.clone(),
            agent_id,
            match decision {
                AgentReviewDecision::Approve => AgentMessageKind::Progress,
                AgentReviewDecision::Reject => AgentMessageKind::Failed,
            },
            review_message(decision, &message),
            timestamp_ms,
        );
        Ok(())
    }

    pub(crate) fn fail_task(
        &mut self,
        task_id: &AgentTaskId,
        kind: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
        timestamp_ms: u64,
    ) {
        let kind = kind.into();
        let message = message.into();
        let Some(agent_id) = self.update_task(task_id, timestamp_ms, |task| {
            task.status = AgentTaskStatus::Failed;
            task.ended_at_ms = Some(timestamp_ms);
            clear_active_lease(task);
            task.failure = Some(AgentTaskFailure {
                kind: kind.clone(),
                message: message.clone(),
                retryable,
                created_at_ms: timestamp_ms,
            });
            task.agent_id.clone()
        }) else {
            return;
        };
        self.push_message(
            task_id.clone(),
            agent_id,
            AgentMessageKind::Failed,
            message,
            timestamp_ms,
        );
    }

    pub(crate) fn normalize_for_resume(&mut self, timestamp_ms: u64) {
        use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};

        let mut interrupted = Vec::new();
        for task in &mut self.tasks {
            if task.status == AgentTaskStatus::Running {
                task.status = AgentTaskStatus::Interrupted;
                task.updated_at_ms = timestamp_ms;
                task.ended_at_ms = Some(timestamp_ms);
                clear_active_lease(task);

                // For worktree workers, try to preserve the worktree path and
                // attach a recovery artifact so the UI can guide the user.
                let mut resume_note =
                    "child task was running when the session was restored".to_string();
                if task.execution_mode == AgentExecutionMode::WorktreeWorker {
                    let worktree_path = task.artifacts.iter().rev().find_map(|a| {
                        if a.kind == AgentArtifactKind::Evidence && a.title == "Worktree metadata" {
                            serde_json::from_str::<serde_json::Value>(&a.content)
                                .ok()
                                .and_then(|v| {
                                    v.get("worktree_path")
                                        .and_then(|p| p.as_str().map(|s| s.to_string()))
                                })
                        } else {
                            None
                        }
                    });
                    if let Some(ref path) = worktree_path {
                        resume_note.push_str(&format!(
                            " — worktree may still exist at {}. \
                                 Please inspect or re-run the task.",
                            path
                        ));
                        task.artifacts.push(AgentArtifact {
                            artifact_id: format!("interrupted-{}", task.task_id.as_str()),
                            task_id: task.task_id.clone(),
                            kind: AgentArtifactKind::Evidence,
                            title: "Interrupted worktree worker".to_string(),
                            content: serde_json::json!({
                                "status": "interrupted",
                                "worktree_path": path,
                                "advice": "This worktree worker was interrupted. \
                                           Please inspect the worktree or re-run the task.",
                            })
                            .to_string(),
                            created_at_ms: timestamp_ms,
                        });
                    }
                }
                task.resume_note = Some(resume_note);
                interrupted.push((task.task_id.clone(), task.agent_id.clone()));
            }
        }
        for (task_id, agent_id) in interrupted {
            self.push_message(
                task_id,
                agent_id,
                AgentMessageKind::Interrupted,
                "Child task was interrupted by session restore".to_string(),
                timestamp_ms,
            );
        }
    }

    pub(crate) fn projection(&self) -> AgentA2AProjection {
        let mut projection = AgentA2AProjection::default();
        let mut child_task_ids_by_parent: HashMap<AgentTaskId, Vec<String>> = HashMap::new();
        for task in &self.tasks {
            if let Some(parent_task_id) = task.parent_task_id.as_ref() {
                child_task_ids_by_parent
                    .entry(parent_task_id.clone())
                    .or_default()
                    .push(task.task_id.as_str().to_string());
            }
        }

        projection.tasks = self
            .tasks
            .iter()
            .map(|task| {
                match task.status {
                    AgentTaskStatus::Running => projection.running_count += 1,
                    AgentTaskStatus::Completed => projection.completed_count += 1,
                    AgentTaskStatus::Failed => projection.failed_count += 1,
                    AgentTaskStatus::Interrupted => projection.interrupted_count += 1,
                    AgentTaskStatus::Pending | AgentTaskStatus::Cancelled => {}
                }
                let latest_artifact = task.artifacts.last();
                let worktree_meta = latest_worktree_metadata(task);
                let (changed_files, changed_file_count) = changed_files_for_task(task);
                // Phase 4-B: test report excerpt from TestReport artifact.
                let test_report_excerpt = task.artifacts.iter().rev().find_map(|a| {
                    if a.kind == crate::agent::a2a::types::AgentArtifactKind::TestReport
                        || a.title == "Test report"
                    {
                        extract_test_report_excerpt(&a.content)
                    } else {
                        None
                    }
                });
                // Phase 4-B: diff_available from worktree metadata.
                let diff_available = worktree_meta
                    .as_ref()
                    .and_then(|v| v.get("diff_available").and_then(|x| x.as_bool()));
                let duration_ms = compute_duration_ms(task.started_at_ms, task.ended_at_ms);
                let file_io_events = worktree_meta
                    .as_ref()
                    .and_then(|value| value.get("file_io_events"))
                    .map(extract_file_io_events)
                    .unwrap_or_default();
                let runtime_events = if task.parent_task_id.is_some() {
                    self.child_runtime_events_for(task, worktree_meta.as_ref(), &file_io_events)
                } else {
                    Vec::new()
                };
                let usage_ledger = worktree_meta
                    .as_ref()
                    .and_then(|value| value.get("usage_ledger"))
                    .and_then(|value| {
                        serde_json::from_value::<crate::loop_runtime::LoopUsageLedger>(
                            value.clone(),
                        )
                        .ok()
                    });
                let mut child_task_ids = Vec::new();
                for child_task_id in &task.child_task_ids {
                    let child_task_id = child_task_id.as_str().to_string();
                    if !child_task_ids
                        .iter()
                        .any(|existing| existing == &child_task_id)
                    {
                        child_task_ids.push(child_task_id);
                    }
                }
                if let Some(derived_child_task_ids) = child_task_ids_by_parent.get(&task.task_id) {
                    for child_task_id in derived_child_task_ids {
                        if !child_task_ids
                            .iter()
                            .any(|existing| existing == child_task_id)
                        {
                            child_task_ids.push(child_task_id.clone());
                        }
                    }
                }
                let child_capsules = child_capsules_for_task(task, &self.tasks, &child_task_ids);
                let review_gate = review_gate_for_task(
                    task,
                    worktree_meta.as_ref(),
                    diff_available,
                    latest_artifact.map(|artifact| artifact.created_at_ms),
                );
                let recovery_actions = recovery_actions_for_task(task, worktree_meta.as_ref());
                AgentA2ATaskProjection {
                    task_id: task.task_id.as_str().to_string(),
                    agent_id: task.agent_id.as_str().to_string(),
                    role: task.role.as_str().to_string(),
                    execution_mode: task.execution_mode.as_str().to_string(),
                    status: task.status.as_str().to_string(),
                    title: task.title.clone(),
                    messages: self.messages_for(&task.task_id),
                    latest_message: self.latest_message_for(&task.task_id),
                    failure_message: task.failure.as_ref().map(|failure| failure.message.clone()),
                    updated_at_ms: task.updated_at_ms,
                    artifact_count: task.artifacts.len(),
                    latest_artifact_kind: latest_artifact.map(|a| a.kind.as_str().to_string()),
                    latest_artifact_title: latest_artifact.map(|a| a.title.clone()),
                    needs_human_review: worktree_meta
                        .as_ref()
                        .and_then(|v| v.get("needs_human_review").and_then(|x| x.as_bool())),
                    reason_codes: worktree_meta
                        .as_ref()
                        .and_then(|v| v.get("reason_codes"))
                        .and_then(|arr| {
                            arr.as_array().map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                        })
                        .unwrap_or_default(),
                    tests_passed: worktree_meta
                        .as_ref()
                        .and_then(|v| v.get("tests_passed").and_then(|x| x.as_bool())),
                    diff_truncated: worktree_meta
                        .as_ref()
                        .and_then(|v| v.get("diff_truncated").and_then(|x| x.as_bool())),
                    worktree_path: worktree_meta.as_ref().and_then(|v| {
                        v.get("worktree_path")
                            .and_then(|x| x.as_str().map(|s| s.to_string()))
                    }),
                    cleaned_up: worktree_meta
                        .as_ref()
                        .and_then(|v| v.get("cleaned_up").and_then(|x| x.as_bool())),
                    suggested_action: worktree_meta.as_ref().and_then(|v| {
                        v.get("suggested_action")
                            .and_then(|x| x.as_str().map(|s| s.to_string()))
                    }),
                    review_decision: worktree_meta.as_ref().and_then(|v| {
                        v.get("review_decision")
                            .and_then(|x| x.as_str().map(|s| s.to_string()))
                    }),
                    reviewed_at_ms: worktree_meta
                        .as_ref()
                        .and_then(|v| v.get("reviewed_at_ms").and_then(|x| x.as_u64())),
                    // Phase 4-A enriched fields.
                    parent_task_id: task
                        .parent_task_id
                        .as_ref()
                        .map(|id| id.as_str().to_string()),
                    child_task_ids,
                    created_at_ms: task.created_at_ms,
                    started_at_ms: task.started_at_ms,
                    ended_at_ms: task.ended_at_ms,
                    duration_ms,
                    retryable: task.failure.as_ref().map(|f| f.retryable),
                    failure_kind: task.failure.as_ref().map(|f| f.kind.clone()),
                    resume_note: task.resume_note.clone(),
                    latest_progress: self.latest_progress_for(&task.task_id),
                    // Phase 4-C — durable worker lease / retry state.
                    lease_owner: task.lease_owner.clone(),
                    lease_acquired_at_ms: task.lease_acquired_at_ms,
                    lease_expires_at_ms: task.lease_expires_at_ms,
                    last_heartbeat_at_ms: task.last_heartbeat_at_ms,
                    attempt_count: task.attempt_count,
                    max_attempts: task.max_attempts,
                    runtime_events,
                    child_capsules,
                    review_gate,
                    recovery_actions,
                    // Phase 4-B — diff-derived file visibility.
                    diff_available,
                    changed_file_count,
                    changed_files,
                    test_report_excerpt,
                    file_io_events,
                    usage_ledger,
                }
            })
            .collect();
        projection
    }

    fn latest_message_for(&self, task_id: &AgentTaskId) -> Option<String> {
        self.messages
            .iter()
            .rev()
            .find(|message| message.task_id == *task_id)
            .map(|message| message.content.clone())
    }

    fn latest_progress_for(&self, task_id: &AgentTaskId) -> Option<String> {
        self.messages
            .iter()
            .rev()
            .find(|message| {
                message.task_id == *task_id && matches!(message.kind, AgentMessageKind::Progress)
            })
            .map(|message| message.content.clone())
    }

    fn messages_for(&self, task_id: &AgentTaskId) -> Vec<AgentA2AMessageProjection> {
        self.messages
            .iter()
            .filter(|message| message.task_id == *task_id)
            .map(|message| AgentA2AMessageProjection {
                message_id: message.message_id.clone(),
                kind: message_kind_for_projection(&message.kind).to_string(),
                content: message.content.clone(),
                created_at_ms: message.created_at_ms,
            })
            .collect()
    }

    fn child_runtime_events_for(
        &self,
        task: &AgentTaskRecord,
        worktree_meta: Option<&serde_json::Value>,
        file_io_events: &[AgentFileIoEventProjection],
    ) -> Vec<AgentA2AChildRuntimeEvent> {
        let mut events = Vec::new();
        for message in self
            .messages
            .iter()
            .filter(|message| message.task_id == task.task_id)
        {
            if let Some(kind) = child_event_kind_for_message(&message.kind) {
                push_child_runtime_event(
                    &mut events,
                    kind,
                    child_event_label(kind),
                    message.content.clone(),
                    message.created_at_ms,
                );
            }
        }
        for artifact in &task.artifacts {
            if artifact.kind == crate::agent::a2a::types::AgentArtifactKind::PatchProposal {
                push_child_runtime_event(
                    &mut events,
                    AgentA2AChildEventKind::PatchProposed,
                    "Patch proposed",
                    artifact.title.clone(),
                    artifact.created_at_ms,
                );
            }
        }
        if let Some((metadata, created_at_ms)) = latest_worktree_metadata_with_time(task) {
            if metadata
                .get("needs_human_review")
                .and_then(|value| value.as_bool())
                == Some(true)
            {
                push_child_runtime_event(
                    &mut events,
                    AgentA2AChildEventKind::WaitingReview,
                    "Waiting for review",
                    metadata
                        .get("suggested_action")
                        .and_then(|value| value.as_str())
                        .unwrap_or("Human review required")
                        .to_string(),
                    created_at_ms,
                );
            }
        } else if worktree_meta
            .and_then(|metadata| metadata.get("needs_human_review"))
            .and_then(|value| value.as_bool())
            == Some(true)
        {
            push_child_runtime_event(
                &mut events,
                AgentA2AChildEventKind::WaitingReview,
                "Waiting for review",
                "Human review required".to_string(),
                task.updated_at_ms,
            );
        }
        for file_io in file_io_events {
            push_child_runtime_event(
                &mut events,
                AgentA2AChildEventKind::FileFact,
                "File fact",
                format!("{} {}", file_io.operation, file_io.path),
                task.updated_at_ms,
            );
        }
        events.sort_by_key(|event| event.created_at_ms);
        events
    }

    fn update_task<T>(
        &mut self,
        task_id: &AgentTaskId,
        timestamp_ms: u64,
        update: impl FnOnce(&mut AgentTaskRecord) -> T,
    ) -> Option<T> {
        let task = self
            .tasks
            .iter_mut()
            .find(|task| task.task_id == *task_id)?;
        task.updated_at_ms = timestamp_ms;
        Some(update(task))
    }

    fn push_message(
        &mut self,
        task_id: AgentTaskId,
        agent_id: AgentId,
        kind: AgentMessageKind,
        content: String,
        timestamp_ms: u64,
    ) {
        self.next_message_index += 1;
        self.messages.push(AgentMessage {
            message_id: format!("a2a-message-{}", self.next_message_index),
            task_id,
            agent_id,
            kind,
            content,
            created_at_ms: timestamp_ms,
        });
    }
}

/// Extract unique file paths from a git diff text blob.
/// Parses structured diff headers: `diff --git a/path b/path`, `+++ b/path`, `--- a/path`.
/// Deduplicates and normalizes by trimming `a/` / `b/` prefixes.
/// Handles `/dev/null` (added/deleted file) by falling back to the other side.
fn extract_files_from_diff_text(text: &str) -> Vec<String> {
    let mut header_paths: Vec<String> = Vec::new();
    let mut fallback_paths: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            // "a/path b/path" — prefer b/ side, fall back to a/ if b is /dev/null.
            let mut parts = rest.splitn(2, ' ');
            let a = parts.next().unwrap_or("");
            let b = parts.next().unwrap_or("");
            let b_norm = b.trim_start_matches("b/");
            if b_norm != "/dev/null" {
                push_unique_path(&mut header_paths, b_norm);
            } else {
                let a_norm = a.trim_start_matches("a/");
                if a_norm != "/dev/null" {
                    push_unique_path(&mut header_paths, a_norm);
                }
            }
        } else if let Some(rest) = line.strip_prefix("+++ ") {
            let p = rest.trim_start_matches("b/");
            if p != "/dev/null" {
                push_unique_path(&mut fallback_paths, p);
            }
        } else if let Some(rest) = line.strip_prefix("--- ") {
            let p = rest.trim_start_matches("a/");
            if p != "/dev/null" {
                push_unique_path(&mut fallback_paths, p);
            }
        }
    }
    if header_paths.is_empty() {
        fallback_paths
    } else {
        header_paths
    }
}

fn push_unique_path(paths: &mut Vec<String>, path: &str) {
    if !path.is_empty() && !paths.iter().any(|existing| existing == path) {
        paths.push(path.to_string());
    }
}

/// Extract a short single-line excerpt from a TestReport artifact's JSON content.
/// Looks for a "summary" or "result" field; falls back to the first non-empty line.
fn extract_test_report_excerpt(content: &str) -> Option<String> {
    // Try JSON first — look for a summary or result field.
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(s) = value.get("summary").and_then(|v| v.as_str()) {
            return Some(s.lines().next().unwrap_or(s).trim().to_string());
        }
        if let Some(s) = value.get("result").and_then(|v| v.as_str()) {
            return Some(s.lines().next().unwrap_or(s).trim().to_string());
        }
    }
    // Fallback: first non-empty line of content, trimmed.
    content
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
}

fn extract_file_io_events(value: &serde_json::Value) -> Vec<AgentFileIoEventProjection> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|event| {
            let path = event.get("path").and_then(|value| value.as_str())?;
            let operation = event.get("operation").and_then(|value| value.as_str())?;
            Some(AgentFileIoEventProjection {
                path: path.to_string(),
                operation: operation.to_string(),
            })
        })
        .collect()
}

fn child_event_kind_for_message(kind: &AgentMessageKind) -> Option<AgentA2AChildEventKind> {
    match kind {
        AgentMessageKind::TaskAssigned => Some(AgentA2AChildEventKind::Assigned),
        AgentMessageKind::LeaseClaimed => Some(AgentA2AChildEventKind::LeaseClaimed),
        AgentMessageKind::Started => Some(AgentA2AChildEventKind::Started),
        AgentMessageKind::Progress => Some(AgentA2AChildEventKind::Progress),
        AgentMessageKind::FinalResult => Some(AgentA2AChildEventKind::Completed),
        AgentMessageKind::Failed => Some(AgentA2AChildEventKind::Failed),
        AgentMessageKind::Cancelled => Some(AgentA2AChildEventKind::Abandoned),
        AgentMessageKind::Interrupted => Some(AgentA2AChildEventKind::Recovered),
        AgentMessageKind::Evidence | AgentMessageKind::ArtifactCreated => None,
    }
}

fn child_event_label(kind: AgentA2AChildEventKind) -> &'static str {
    match kind {
        AgentA2AChildEventKind::Assigned => "Assigned",
        AgentA2AChildEventKind::LeaseClaimed => "Lease claimed",
        AgentA2AChildEventKind::Started => "Started",
        AgentA2AChildEventKind::Progress => "Progress",
        AgentA2AChildEventKind::FileFact => "File fact",
        AgentA2AChildEventKind::PatchProposed => "Patch proposed",
        AgentA2AChildEventKind::WaitingReview => "Waiting for review",
        AgentA2AChildEventKind::Completed => "Completed",
        AgentA2AChildEventKind::Failed => "Failed",
        AgentA2AChildEventKind::Abandoned => "Abandoned",
        AgentA2AChildEventKind::Recovered => "Recovered",
    }
}

fn push_child_runtime_event(
    events: &mut Vec<AgentA2AChildRuntimeEvent>,
    kind: AgentA2AChildEventKind,
    label: impl Into<String>,
    detail: impl Into<String>,
    created_at_ms: u64,
) {
    events.push(AgentA2AChildRuntimeEvent {
        kind,
        label: label.into(),
        detail: detail.into(),
        created_at_ms,
    });
}

fn changed_files_for_task(task: &AgentTaskRecord) -> (Vec<String>, Option<usize>) {
    let all_diff_files = task
        .artifacts
        .iter()
        .rev()
        .find_map(|artifact| {
            if artifact.kind == crate::agent::a2a::types::AgentArtifactKind::DiffSummary
                || artifact.title == "Worktree diff"
            {
                Some(artifact.content.as_str())
            } else {
                None
            }
        })
        .map(extract_files_from_diff_text)
        .unwrap_or_default();
    let changed_files = all_diff_files.iter().take(8).cloned().collect();
    let changed_file_count = if all_diff_files.is_empty() {
        None
    } else {
        Some(all_diff_files.len())
    };
    (changed_files, changed_file_count)
}

fn child_capsules_for_task(
    parent: &AgentTaskRecord,
    all_tasks: &[AgentTaskRecord],
    child_task_ids: &[String],
) -> Vec<AgentA2AChildCapsule> {
    child_task_ids
        .iter()
        .filter_map(|child_task_id| {
            let child = all_tasks
                .iter()
                .find(|task| task.task_id.as_str() == child_task_id)?;
            Some(child_capsule_for(parent, child))
        })
        .collect()
}

fn child_capsule_for(parent: &AgentTaskRecord, child: &AgentTaskRecord) -> AgentA2AChildCapsule {
    let (changed_files, _) = changed_files_for_task(child);
    let worktree_meta = latest_worktree_metadata(child);
    let review_decision = worktree_meta
        .as_ref()
        .and_then(|metadata| metadata.get("review_decision"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let next_action = child_capsule_next_action(child, worktree_meta.as_ref());
    let failure_reason = child
        .failure
        .as_ref()
        .map(|failure| failure.message.clone());
    let artifact_titles = artifact_titles_for_task(child);
    let child_goal = child.prompt.trim().to_string();
    let status = child.status.as_str().to_string();
    let estimated_tokens = estimate_child_capsule_tokens(
        &child_goal,
        &status,
        &artifact_titles,
        &changed_files,
        review_decision.as_deref(),
        failure_reason.as_deref(),
        &next_action,
    );

    AgentA2AChildCapsule {
        capsule_id: format!(
            "child-capsule:{}:{}",
            parent.task_id.as_str(),
            child.task_id.as_str()
        ),
        parent_task_id: parent.task_id.as_str().to_string(),
        child_task_id: child.task_id.as_str().to_string(),
        child_goal,
        status,
        artifact_titles,
        changed_files,
        review_decision,
        failure_reason,
        next_action,
        estimated_tokens,
    }
}

fn child_capsule_next_action(
    child: &AgentTaskRecord,
    worktree_meta: Option<&serde_json::Value>,
) -> String {
    if let Some(action) = worktree_meta
        .and_then(|metadata| metadata.get("suggested_action"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return action.to_string();
    }
    match child.status {
        AgentTaskStatus::Pending => "Wait for the child task to start.".to_string(),
        AgentTaskStatus::Running => "Wait for the child task to finish.".to_string(),
        AgentTaskStatus::Completed => {
            "Review child evidence before using it for parent completion.".to_string()
        }
        AgentTaskStatus::Failed => {
            if child
                .failure
                .as_ref()
                .map(|failure| failure.retryable)
                .unwrap_or(false)
            {
                "Review failure evidence and decide whether to retry.".to_string()
            } else {
                "Review failure evidence before continuing the parent task.".to_string()
            }
        }
        AgentTaskStatus::Cancelled => "Child task was abandoned.".to_string(),
        AgentTaskStatus::Interrupted => {
            "Recover or abandon the interrupted child task before relying on it.".to_string()
        }
    }
}

fn review_gate_for_task(
    task: &AgentTaskRecord,
    worktree_meta: Option<&serde_json::Value>,
    diff_available: Option<bool>,
    latest_artifact_at_ms: Option<u64>,
) -> Option<AgentA2AReviewGateProjection> {
    if task.execution_mode != AgentExecutionMode::WorktreeWorker {
        return None;
    }

    let review_decision = metadata_string(worktree_meta, "review_decision");
    let reviewed_at_ms = worktree_meta
        .and_then(|metadata| metadata.get("reviewed_at_ms"))
        .and_then(|value| value.as_u64());
    let needs_human_review = worktree_meta
        .and_then(|metadata| metadata.get("needs_human_review"))
        .and_then(|value| value.as_bool());
    let suggested_action =
        metadata_string(worktree_meta, "suggested_action").filter(|value| !value.trim().is_empty());
    let review_message =
        metadata_string(worktree_meta, "review_message").filter(|value| !value.trim().is_empty());

    if review_decision.as_deref() == Some("approved") {
        if reviewed_at_ms
            .zip(latest_artifact_at_ms)
            .map(|(reviewed, latest)| latest > reviewed)
            .unwrap_or(false)
        {
            return Some(review_gate_projection(
                AgentA2AReviewGateKind::StaleReview,
                "Stale review",
                "Child artifacts changed after the last review decision.",
                "stale_review_blocks_parent_completion",
                task,
                reviewed_at_ms,
            ));
        }
        return Some(review_gate_projection(
            AgentA2AReviewGateKind::Approved,
            "Review approved",
            review_message
                .as_deref()
                .or(suggested_action.as_deref())
                .unwrap_or("Child review was approved."),
            "child_review_approved_only",
            task,
            reviewed_at_ms,
        ));
    }

    if review_decision.as_deref() == Some("rejected")
        || task
            .failure
            .as_ref()
            .map(|failure| failure.kind == "review_rejection")
            .unwrap_or(false)
    {
        return Some(review_gate_projection(
            AgentA2AReviewGateKind::Rejected,
            "Review rejected",
            task.failure
                .as_ref()
                .map(|failure| failure.message.as_str())
                .or(review_message.as_deref())
                .or(suggested_action.as_deref())
                .unwrap_or("Child review was rejected."),
            "child_review_rejected",
            task,
            reviewed_at_ms,
        ));
    }

    if needs_human_review == Some(true) {
        return Some(review_gate_projection(
            AgentA2AReviewGateKind::WaitingReview,
            "Waiting for review",
            suggested_action.as_deref().unwrap_or(
                "Human review is required before parent completion can rely on this child.",
            ),
            "blocks_parent_completion",
            task,
            reviewed_at_ms,
        ));
    }

    if task.status == AgentTaskStatus::Completed
        && task.parent_task_id.is_some()
        && diff_available == Some(true)
    {
        return Some(review_gate_projection(
            AgentA2AReviewGateKind::MissingEvidence,
            "Review evidence missing",
            "Child produced a diff but no review decision evidence is recorded.",
            "missing_review_evidence",
            task,
            reviewed_at_ms,
        ));
    }

    None
}

fn review_gate_projection(
    kind: AgentA2AReviewGateKind,
    label: impl Into<String>,
    reason: impl Into<String>,
    completion_impact: impl Into<String>,
    task: &AgentTaskRecord,
    reviewed_at_ms: Option<u64>,
) -> AgentA2AReviewGateProjection {
    AgentA2AReviewGateProjection {
        kind,
        label: label.into(),
        reason: reason.into(),
        completion_impact: completion_impact.into(),
        parent_task_id: task
            .parent_task_id
            .as_ref()
            .map(|task_id| task_id.as_str().to_string()),
        child_task_id: task.task_id.as_str().to_string(),
        reviewed_at_ms,
    }
}

fn recovery_actions_for_task(
    task: &AgentTaskRecord,
    worktree_meta: Option<&serde_json::Value>,
) -> Vec<AgentA2ARecoveryActionSuggestion> {
    let mut actions = Vec::new();
    let worktree_path = metadata_string(worktree_meta, "worktree_path")
        .or_else(|| worktree_path_from_resume_note(task.resume_note.as_deref()));

    match task.status {
        AgentTaskStatus::Failed => {
            if task
                .failure
                .as_ref()
                .map(|failure| failure.retryable)
                .unwrap_or(false)
                && task.attempt_count < task.max_attempts
            {
                push_recovery_action(
                    &mut actions,
                    AgentA2ARecoveryActionKind::Retry,
                    "Retry child task",
                    task.failure
                        .as_ref()
                        .map(|failure| failure.message.as_str())
                        .unwrap_or("Child task failed and can be retried."),
                    true,
                    true,
                    Some(task.attempt_count.saturating_add(1)),
                );
            }
            if let Some(path) = worktree_path.as_deref() {
                push_recovery_action(
                    &mut actions,
                    AgentA2ARecoveryActionKind::InspectWorktree,
                    "Inspect retained worktree",
                    format!("Inspect retained worktree at {path}."),
                    true,
                    false,
                    None,
                );
            }
            push_recovery_action(
                &mut actions,
                AgentA2ARecoveryActionKind::Abandon,
                "Abandon child task",
                "Mark this child as abandoned after reviewing failure evidence.",
                true,
                false,
                None,
            );
        }
        AgentTaskStatus::Interrupted => {
            if let Some(path) = worktree_path.as_deref() {
                push_recovery_action(
                    &mut actions,
                    AgentA2ARecoveryActionKind::InspectWorktree,
                    "Inspect retained worktree",
                    format!("Inspect retained worktree at {path}."),
                    true,
                    false,
                    None,
                );
            }
            push_recovery_action(
                &mut actions,
                AgentA2ARecoveryActionKind::Abandon,
                "Abandon interrupted child",
                "Decide whether this interrupted child should be abandoned before parent completion relies on it.",
                true,
                false,
                None,
            );
        }
        AgentTaskStatus::Pending
        | AgentTaskStatus::Running
        | AgentTaskStatus::Completed
        | AgentTaskStatus::Cancelled => {}
    }

    actions
}

fn push_recovery_action(
    actions: &mut Vec<AgentA2ARecoveryActionSuggestion>,
    action: AgentA2ARecoveryActionKind,
    label: impl Into<String>,
    reason: impl Into<String>,
    requires_human_approval: bool,
    retryable: bool,
    next_attempt: Option<u32>,
) {
    actions.push(AgentA2ARecoveryActionSuggestion {
        action,
        label: label.into(),
        reason: reason.into(),
        requires_human_approval,
        retryable,
        next_attempt,
    });
}

fn metadata_string(worktree_meta: Option<&serde_json::Value>, key: &str) -> Option<String> {
    worktree_meta
        .and_then(|metadata| metadata.get(key))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn worktree_path_from_resume_note(resume_note: Option<&str>) -> Option<String> {
    let note = resume_note?;
    let marker = "Inspect ";
    let start = note.find(marker)? + marker.len();
    let rest = &note[start..];
    let end = rest.find('.').unwrap_or(rest.len());
    Some(rest[..end].trim().to_string()).filter(|value| !value.is_empty())
}

fn artifact_titles_for_task(task: &AgentTaskRecord) -> Vec<String> {
    let mut titles = Vec::new();
    for artifact in &task.artifacts {
        if !artifact.title.trim().is_empty()
            && !titles
                .iter()
                .any(|existing: &String| existing == &artifact.title)
        {
            titles.push(artifact.title.clone());
        }
    }
    titles
}

fn estimate_child_capsule_tokens(
    child_goal: &str,
    status: &str,
    artifact_titles: &[String],
    changed_files: &[String],
    review_decision: Option<&str>,
    failure_reason: Option<&str>,
    next_action: &str,
) -> u32 {
    let mut text = format!("{child_goal}\n{status}\n{next_action}");
    for title in artifact_titles {
        text.push('\n');
        text.push_str(title);
    }
    for file in changed_files {
        text.push('\n');
        text.push_str(file);
    }
    if let Some(review_decision) = review_decision {
        text.push('\n');
        text.push_str(review_decision);
    }
    if let Some(failure_reason) = failure_reason {
        text.push('\n');
        text.push_str(failure_reason);
    }
    ((text.chars().count() as u32).saturating_add(3) / 4).max(1)
}

/// Compute duration in milliseconds for a finished task.
/// Running tasks intentionally return None; the UI derives live elapsed time.
fn compute_duration_ms(started_at_ms: Option<u64>, ended_at_ms: Option<u64>) -> Option<u64> {
    let start = started_at_ms?;
    let end = ended_at_ms?;
    // Guard against clock skew / zero duration.
    if end > start {
        Some(end - start)
    } else {
        Some(0)
    }
}

fn lease_expires_at(timestamp_ms: u64, lease_duration_ms: u64) -> u64 {
    timestamp_ms.saturating_add(lease_duration_ms)
}

fn lease_is_expired(task: &AgentTaskRecord, timestamp_ms: u64) -> bool {
    task.lease_expires_at_ms
        .map(|expires_at| timestamp_ms > expires_at)
        .unwrap_or(true)
}

fn clear_active_lease(task: &mut AgentTaskRecord) {
    task.lease_owner = None;
    task.lease_acquired_at_ms = None;
    task.lease_expires_at_ms = None;
    task.last_heartbeat_at_ms = None;
}

fn latest_worktree_metadata(task: &AgentTaskRecord) -> Option<serde_json::Value> {
    latest_worktree_metadata_with_time(task).map(|(metadata, _)| metadata)
}

fn latest_worktree_metadata_with_time(task: &AgentTaskRecord) -> Option<(serde_json::Value, u64)> {
    task.artifacts.iter().rev().find_map(|artifact| {
        if artifact.kind == crate::agent::a2a::types::AgentArtifactKind::Evidence
            && artifact.title == "Worktree metadata"
        {
            serde_json::from_str::<serde_json::Value>(&artifact.content)
                .ok()
                .map(|metadata| (metadata, artifact.created_at_ms))
        } else {
            None
        }
    })
}

fn review_message(decision: AgentReviewDecision, message: &str) -> String {
    let prefix = decision.message_prefix();
    let message = message.trim();
    if message.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}: {message}")
    }
}

fn is_terminal_status(status: &AgentTaskStatus) -> bool {
    matches!(
        status,
        AgentTaskStatus::Completed | AgentTaskStatus::Cancelled
    )
}

fn is_lease_claimable_status(status: &AgentTaskStatus) -> bool {
    matches!(status, AgentTaskStatus::Pending | AgentTaskStatus::Running)
}

fn message_kind_for_projection(kind: &AgentMessageKind) -> &'static str {
    match kind {
        AgentMessageKind::TaskAssigned => "task_assigned",
        AgentMessageKind::LeaseClaimed => "lease_claimed",
        AgentMessageKind::Started => "started",
        AgentMessageKind::Progress => "progress",
        AgentMessageKind::Evidence => "evidence",
        AgentMessageKind::ArtifactCreated => "artifact_created",
        AgentMessageKind::FinalResult => "final_result",
        AgentMessageKind::Failed => "failed",
        AgentMessageKind::Cancelled => "cancelled",
        AgentMessageKind::Interrupted => "interrupted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::a2a::projection::AgentA2AChildEventKind;
    use crate::agent::a2a::types::{AgentExecutionMode, AgentRole};

    #[test]
    fn bus_records_lifecycle_and_projection() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Inspect session loop",
            "Find where delegate_task is executed",
            10,
        );

        bus.start_task(&task_id, 20);
        bus.record_progress(&task_id, "Reading session.rs", 30);
        bus.complete_task(&task_id, "delegate_task is split before regular tools", 40);

        let projection = bus.projection();

        assert_eq!(projection.tasks.len(), 1);
        assert_eq!(projection.running_count, 0);
        assert_eq!(projection.completed_count, 1);
        assert_eq!(projection.tasks[0].status, "completed");
        assert_eq!(
            projection.tasks[0].latest_message.as_deref(),
            Some("delegate_task is split before regular tools")
        );
        let message_kinds = projection.tasks[0]
            .messages
            .iter()
            .map(|message| message.kind.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            message_kinds,
            vec!["task_assigned", "started", "progress", "final_result"]
        );
        assert_eq!(
            projection.tasks[0].messages[2].content,
            "Reading session.rs"
        );
    }

    #[test]
    fn resume_normalization_interrupts_running_tasks() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Reviewer,
            AgentExecutionMode::ReadOnly,
            "Review compact behavior",
            "Check compact edge cases",
            10,
        );
        bus.start_task(&task_id, 20);

        bus.normalize_for_resume(30);

        let task = bus.task(&task_id).expect("task");
        assert_eq!(
            task.status,
            crate::agent::a2a::types::AgentTaskStatus::Interrupted
        );
        assert_eq!(
            task.resume_note.as_deref(),
            Some("child task was running when the session was restored")
        );
    }

    #[test]
    fn resume_normalization_interrupts_worktree_worker_tasks() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement auth",
            "Add login flow",
            10,
        );
        bus.start_task(&task_id, 20);

        bus.normalize_for_resume(30);

        let task = bus.task(&task_id).expect("task");
        assert_eq!(
            task.status,
            crate::agent::a2a::types::AgentTaskStatus::Interrupted
        );
        let projection = bus.projection();
        assert_eq!(projection.interrupted_count, 1);
        assert_eq!(projection.tasks[0].execution_mode, "worktree_worker");
    }

    #[test]
    fn resume_preserves_worktree_path_for_interrupted_worker() {
        use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement auth",
            "Add login flow",
            10,
        );
        bus.start_task(&task_id, 20);

        // Pre-populate a worktree metadata artifact as if the worker had started.
        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "meta-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::Evidence,
                title: "Worktree metadata".to_string(),
                content: serde_json::json!({
                    "worktree_path": "/tmp/repo/.claude/worktrees/a2a-worktree-auth",
                    "cleaned_up": false,
                })
                .to_string(),
                created_at_ms: 25,
            },
            25,
        );

        bus.normalize_for_resume(30);

        let task = bus.task(&task_id).expect("task");
        assert_eq!(
            task.status,
            crate::agent::a2a::types::AgentTaskStatus::Interrupted
        );
        // Resume note should reference the worktree path.
        let note = task.resume_note.as_deref().expect("resume_note");
        assert!(
            note.contains("worktree may still exist"),
            "resume_note should mention worktree: {}",
            note
        );
        assert!(
            note.contains("/tmp/repo/.claude/worktrees/a2a-worktree-auth"),
            "resume_note should contain path: {}",
            note
        );
        // An interrupted artifact should have been added.
        let interrupted_artifact = task
            .artifacts
            .iter()
            .find(|a| a.title == "Interrupted worktree worker");
        assert!(
            interrupted_artifact.is_some(),
            "should add interrupted artifact"
        );
        let content = interrupted_artifact.unwrap().content.clone();
        assert!(content.contains("interrupted"));
        assert!(content.contains("a2a-worktree-auth"));
    }

    #[test]
    fn bus_adds_artifact_and_projection_reflects_it() {
        use crate::agent::a2a::types::AgentArtifactKind;

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::PatchProposal,
            "Propose fix",
            "Fix error handling",
            10,
        );
        bus.start_task(&task_id, 20);

        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "art-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::PatchProposal,
                title: "Add null check".to_string(),
                content: "{\"file_path\":\"src/main.rs\"}".to_string(),
                created_at_ms: 30,
            },
            30,
        );

        bus.complete_task(&task_id, "Done", 40);

        let projection = bus.projection();
        assert_eq!(projection.tasks[0].artifact_count, 1);
        assert_eq!(
            projection.tasks[0].latest_artifact_kind.as_deref(),
            Some("patch_proposal")
        );
        assert_eq!(
            projection.tasks[0].latest_artifact_title.as_deref(),
            Some("Add null check")
        );
    }

    #[test]
    fn complete_task_with_artifacts_stores_them() {
        use crate::agent::a2a::types::AgentArtifactKind;

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::PatchProposal,
            "Propose fix",
            "Fix error handling",
            10,
        );

        bus.complete_task_with_artifacts(
            &task_id,
            "Done",
            vec![AgentArtifact {
                artifact_id: "art-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::PatchProposal,
                title: "Proposal".to_string(),
                content: "{}".to_string(),
                created_at_ms: 20,
            }],
            30,
        );

        let task = bus.task(&task_id).expect("task");
        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(task.status, AgentTaskStatus::Completed);
    }

    #[test]
    fn projection_extracts_worktree_metadata_from_evidence_artifact() {
        use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement auth",
            "Add login flow",
            10,
        );
        bus.start_task(&task_id, 20);

        let meta = serde_json::json!({
            "worktree_path": "/tmp/repo/.claude/worktrees/a2a-worktree-auth",
            "cleaned_up": false,
            "diff_available": true,
            "diff_truncated": true,
            "tests_passed": false,
            "needs_human_review": true,
            "suggested_action": "Review before merge.",
            "reason_codes": ["diff was truncated", "tests failed"],
        });
        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "meta-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::Evidence,
                title: "Worktree metadata".to_string(),
                content: meta.to_string(),
                created_at_ms: 30,
            },
            30,
        );
        bus.complete_task(&task_id, "Done", 40);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.needs_human_review, Some(true));
        assert_eq!(task_proj.tests_passed, Some(false));
        assert_eq!(task_proj.diff_truncated, Some(true));
        assert_eq!(
            task_proj.worktree_path.as_deref(),
            Some("/tmp/repo/.claude/worktrees/a2a-worktree-auth")
        );
        assert_eq!(task_proj.cleaned_up, Some(false));
        assert_eq!(
            task_proj.suggested_action.as_deref(),
            Some("Review before merge.")
        );
        assert_eq!(
            task_proj.reason_codes,
            vec!["diff was truncated", "tests failed"]
        );
    }

    #[test]
    fn projection_contract_retains_review_lease_file_and_test_facts() {
        use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement runtime projection",
            "Add additive protocol events",
            10,
        );
        assert!(bus.claim_task_lease(&task_id, "worker-1", 20, 100));
        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "meta-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::Evidence,
                title: "Worktree metadata".to_string(),
                content: serde_json::json!({
                    "worktree_path": "/tmp/forge-runtime-worker",
                    "diff_available": true,
                    "diff_truncated": false,
                    "tests_passed": true,
                    "needs_human_review": true,
                    "reason_codes": ["tests_passed", "diff_available"],
                    "suggested_action": "Review before merge.",
                })
                .to_string(),
                created_at_ms: 25,
            },
            25,
        );
        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "diff-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::DiffSummary,
                title: "Worktree diff".to_string(),
                content: "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+new".to_string(),
                created_at_ms: 26,
            },
            26,
        );
        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "test-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::TestReport,
                title: "Test report".to_string(),
                content: r#"{"summary": "12 tests passed"}"#.to_string(),
                created_at_ms: 27,
            },
            27,
        );
        bus.complete_task(&task_id, "Patch ready", 30);

        let projection = bus.projection();
        assert_eq!(projection.completed_count, 1);
        assert_eq!(projection.running_count, 0);
        assert_eq!(projection.failed_count, 0);
        assert_eq!(projection.interrupted_count, 0);
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.status, "completed");
        assert_eq!(task_proj.lease_owner, None);
        assert_eq!(task_proj.attempt_count, 1);
        assert_eq!(task_proj.needs_human_review, Some(true));
        assert_eq!(task_proj.tests_passed, Some(true));
        assert_eq!(task_proj.diff_available, Some(true));
        assert_eq!(task_proj.changed_file_count, Some(1));
        assert_eq!(task_proj.changed_files, vec!["src/main.rs"]);
        assert_eq!(
            task_proj.test_report_excerpt.as_deref(),
            Some("12 tests passed")
        );
    }

    #[test]
    fn projection_exposes_child_runtime_events_and_parent_capsule() {
        use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};

        let mut bus = AgentA2ABus::default();
        let parent_id = bus.assign_task(
            AgentRole::Reviewer,
            AgentExecutionMode::ReadOnly,
            "Parent review",
            "Review child evidence",
            10,
        );
        let child_id = bus
            .assign_child_task(
                &parent_id,
                AgentRole::Implementer,
                AgentExecutionMode::WorktreeWorker,
                "Child worker",
                "Implement capsule summary",
                20,
            )
            .expect("child task assigned");
        assert!(bus.claim_task_lease(&child_id, "worker-1", 25, 100));
        bus.start_task(&child_id, 30);
        bus.record_progress(&child_id, "Editing src/lib.rs", 35);
        bus.add_artifact(
            &child_id,
            AgentArtifact {
                artifact_id: "patch-1".to_string(),
                task_id: child_id.clone(),
                kind: AgentArtifactKind::PatchProposal,
                title: "Patch proposal".to_string(),
                content: "{}".to_string(),
                created_at_ms: 36,
            },
            36,
        );
        bus.add_artifact(
            &child_id,
            AgentArtifact {
                artifact_id: "meta-1".to_string(),
                task_id: child_id.clone(),
                kind: AgentArtifactKind::Evidence,
                title: "Worktree metadata".to_string(),
                content: serde_json::json!({
                    "worktree_path": "/tmp/forge-child-worker",
                    "diff_available": true,
                    "diff_truncated": false,
                    "tests_passed": true,
                    "needs_human_review": true,
                    "suggested_action": "Review before merge.",
                    "file_io_events": [
                        { "path": "src/lib.rs", "operation": "diff_observed" }
                    ]
                })
                .to_string(),
                created_at_ms: 37,
            },
            37,
        );
        bus.add_artifact(
            &child_id,
            AgentArtifact {
                artifact_id: "diff-1".to_string(),
                task_id: child_id.clone(),
                kind: AgentArtifactKind::DiffSummary,
                title: "Worktree diff".to_string(),
                content: "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new".to_string(),
                created_at_ms: 38,
            },
            38,
        );
        bus.complete_task(&child_id, "Patch ready for review", 40);

        let projection = bus.projection();
        let parent = projection
            .tasks
            .iter()
            .find(|task| task.task_id == parent_id.as_str())
            .expect("parent projection");
        let child = projection
            .tasks
            .iter()
            .find(|task| task.task_id == child_id.as_str())
            .expect("child projection");

        for expected in [
            AgentA2AChildEventKind::Assigned,
            AgentA2AChildEventKind::LeaseClaimed,
            AgentA2AChildEventKind::Started,
            AgentA2AChildEventKind::Progress,
            AgentA2AChildEventKind::PatchProposed,
            AgentA2AChildEventKind::FileFact,
            AgentA2AChildEventKind::WaitingReview,
            AgentA2AChildEventKind::Completed,
        ] {
            assert!(
                child
                    .runtime_events
                    .iter()
                    .any(|event| event.kind == expected),
                "missing child runtime event: {expected:?}"
            );
        }

        assert_eq!(parent.child_capsules.len(), 1);
        let capsule = &parent.child_capsules[0];
        assert_eq!(capsule.parent_task_id, parent_id.as_str());
        assert_eq!(capsule.child_task_id, child_id.as_str());
        assert_eq!(capsule.child_goal, "Implement capsule summary");
        assert_eq!(capsule.status, "completed");
        assert_eq!(capsule.changed_files, vec!["src/lib.rs"]);
        assert_eq!(capsule.review_decision, None);
        assert_eq!(capsule.failure_reason, None);
        assert_eq!(capsule.next_action, "Review before merge.");
        assert!(capsule.estimated_tokens > 0);
        assert_eq!(parent.review_decision, None);
    }

    #[test]
    fn projection_exposes_review_gate_and_recovery_suggestions() {
        use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};

        fn projected<'a>(
            projection: &'a AgentA2AProjection,
            task_id: &AgentTaskId,
        ) -> &'a AgentA2ATaskProjection {
            projection
                .tasks
                .iter()
                .find(|task| task.task_id == task_id.as_str())
                .expect("task projection")
        }

        let mut bus = AgentA2ABus::default();
        let parent_id = bus.assign_task(
            AgentRole::Reviewer,
            AgentExecutionMode::ReadOnly,
            "Parent",
            "Coordinate children",
            10,
        );
        let review_child = bus
            .assign_child_task(
                &parent_id,
                AgentRole::Implementer,
                AgentExecutionMode::WorktreeWorker,
                "Review child",
                "Create a patch",
                20,
            )
            .expect("child task");
        assert!(bus.claim_task_lease(&review_child, "worker-1", 25, 100));
        bus.add_artifact(
            &review_child,
            AgentArtifact {
                artifact_id: "review-meta-1".to_string(),
                task_id: review_child.clone(),
                kind: AgentArtifactKind::Evidence,
                title: "Worktree metadata".to_string(),
                content: serde_json::json!({
                    "worktree_path": "/tmp/forge-review-child",
                    "needs_human_review": true,
                    "suggested_action": "Review before merge.",
                    "tests_passed": true,
                    "diff_available": true
                })
                .to_string(),
                created_at_ms: 30,
            },
            30,
        );
        bus.complete_task(&review_child, "Patch ready", 35);

        let projection = bus.projection();
        let child = projected(&projection, &review_child);
        let gate = child.review_gate.as_ref().expect("review gate");
        assert_eq!(gate.kind, AgentA2AReviewGateKind::WaitingReview);
        assert_eq!(gate.child_task_id, review_child.as_str());
        assert_eq!(gate.parent_task_id.as_deref(), Some(parent_id.as_str()));
        assert_eq!(gate.completion_impact, "blocks_parent_completion");
        assert!(
            gate.reason.contains("Review before merge"),
            "unexpected gate reason: {}",
            gate.reason
        );
        let parent = projected(&projection, &parent_id);
        assert_eq!(parent.review_gate, None);
        assert_eq!(parent.review_decision, None);

        bus.record_review_decision(&review_child, AgentReviewDecision::Approve, "ship it", 45)
            .expect("review approval");
        let projection = bus.projection();
        let child = projected(&projection, &review_child);
        let gate = child.review_gate.as_ref().expect("approved review gate");
        assert_eq!(gate.kind, AgentA2AReviewGateKind::Approved);
        assert_eq!(gate.reviewed_at_ms, Some(45));
        assert_eq!(gate.completion_impact, "child_review_approved_only");
        let parent = projected(&projection, &parent_id);
        assert_eq!(parent.review_gate, None);
        assert_eq!(parent.review_decision, None);

        let failed_child = bus
            .assign_child_task(
                &parent_id,
                AgentRole::Implementer,
                AgentExecutionMode::WorktreeWorker,
                "Failed child",
                "Try risky patch",
                50,
            )
            .expect("failed child");
        assert!(bus.claim_task_lease(&failed_child, "worker-2", 55, 100));
        bus.fail_task(&failed_child, "tool_error", "worker failed", true, 60);

        let projection = bus.projection();
        let failed = projected(&projection, &failed_child);
        assert!(failed
            .recovery_actions
            .iter()
            .any(|action| action.action == AgentA2ARecoveryActionKind::Retry
                && action.requires_human_approval
                && action.next_attempt == Some(2)));
        assert!(failed.recovery_actions.iter().any(|action| {
            action.action == AgentA2ARecoveryActionKind::Abandon
                && action.requires_human_approval
                && !action.retryable
        }));

        let interrupted_child = bus
            .assign_child_task(
                &parent_id,
                AgentRole::Implementer,
                AgentExecutionMode::WorktreeWorker,
                "Interrupted child",
                "Preserve worktree",
                70,
            )
            .expect("interrupted child");
        assert!(bus.claim_task_lease(&interrupted_child, "worker-3", 75, 100));
        bus.add_artifact(
            &interrupted_child,
            AgentArtifact {
                artifact_id: "interrupted-meta-1".to_string(),
                task_id: interrupted_child.clone(),
                kind: AgentArtifactKind::Evidence,
                title: "Worktree metadata".to_string(),
                content: serde_json::json!({
                    "worktree_path": "/tmp/forge-interrupted-child",
                    "needs_human_review": false,
                    "cleaned_up": false
                })
                .to_string(),
                created_at_ms: 80,
            },
            80,
        );
        bus.normalize_for_resume(90);

        let projection = bus.projection();
        let interrupted = projected(&projection, &interrupted_child);
        assert!(interrupted.recovery_actions.iter().any(|action| {
            action.action == AgentA2ARecoveryActionKind::InspectWorktree
                && action.reason.contains("/tmp/forge-interrupted-child")
        }));
        assert!(interrupted
            .recovery_actions
            .iter()
            .any(|action| action.action == AgentA2ARecoveryActionKind::Abandon));
    }

    #[test]
    fn projection_exposes_worktree_boundary_file_io_and_usage_ledger() {
        use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};
        use crate::loop_runtime::{LoopUsageLedger, UsageEvent};

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement telemetry",
            "Record boundary telemetry",
            10,
        );
        bus.start_task(&task_id, 20);
        let usage = LoopUsageLedger::from_events(vec![UsageEvent {
            provider_id: Some("anthropic".to_string()),
            model: Some("claude".to_string()),
            source: Some("anthropic".to_string()),
            reason: crate::protocol::events::ProviderUsageReason::PricingUnknown,
            input_tokens: Some(100),
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            reasoning_tokens: None,
            estimated_cost_micros: None,
            pricing_source: None,
        }])
        .with_runtime_counts(2, 3, 4000);
        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "meta-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::Evidence,
                title: "Worktree metadata".to_string(),
                content: serde_json::json!({
                    "worktree_path": "/tmp/forge-worker",
                    "cleaned_up": false,
                    "file_io_events": [
                        { "path": "/tmp/forge-worker", "operation": "worktree_created" },
                        { "path": "apps/desktop/src-tauri/src/agent/sub.rs", "operation": "diff_observed" },
                        { "path": "/tmp/forge-worker", "operation": "worktree_preserved" }
                    ],
                    "usage_ledger": usage,
                })
                .to_string(),
                created_at_ms: 25,
            },
            25,
        );

        let projection = bus.projection();
        let task = &projection.tasks[0];
        assert_eq!(task.file_io_events.len(), 3);
        assert_eq!(task.file_io_events[0].operation, "worktree_created");
        assert_eq!(
            task.file_io_events[1].path,
            "apps/desktop/src-tauri/src/agent/sub.rs"
        );
        let usage = task.usage_ledger.as_ref().expect("usage ledger");
        assert_eq!(usage.model.as_deref(), Some("claude"));
        assert_eq!(usage.input_tokens, Some(100));
        assert_eq!(usage.output_tokens, None);
        assert!(usage.has_unknown_output_tokens);
        assert!(usage.has_unknown_cost);
        assert_eq!(usage.turn_count, 2);
        assert_eq!(usage.tool_call_count, 3);
        assert_eq!(usage.elapsed_ms, 4000);
    }

    #[test]
    fn approve_review_clears_pending_review_metadata() {
        let mut bus = AgentA2ABus::default();
        let task_id = reviewed_worktree_task(&mut bus);

        bus.record_review_decision(&task_id, AgentReviewDecision::Approve, "Looks good", 50)
            .expect("approve review");

        let task = bus.task(&task_id).expect("task");
        assert_eq!(task.status, AgentTaskStatus::Completed);
        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.needs_human_review, Some(false));
        assert_eq!(task_proj.review_decision.as_deref(), Some("approved"));
        assert_eq!(task_proj.reviewed_at_ms, Some(50));
        assert_eq!(
            task_proj.latest_message.as_deref(),
            Some("Review approved: Looks good")
        );
    }

    #[test]
    fn reject_review_marks_task_failed_without_pending_review() {
        let mut bus = AgentA2ABus::default();
        let task_id = reviewed_worktree_task(&mut bus);

        bus.record_review_decision(
            &task_id,
            AgentReviewDecision::Reject,
            "Permission surface changed outside scope",
            50,
        )
        .expect("reject review");

        let task = bus.task(&task_id).expect("task");
        assert_eq!(task.status, AgentTaskStatus::Failed);
        assert_eq!(
            task.failure.as_ref().map(|failure| failure.kind.as_str()),
            Some("review_rejection")
        );
        assert_eq!(
            task.failure.as_ref().map(|failure| failure.retryable),
            Some(false)
        );
        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.needs_human_review, Some(false));
        assert_eq!(task_proj.review_decision.as_deref(), Some("rejected"));
        assert_eq!(task_proj.reviewed_at_ms, Some(50));
        assert_eq!(task_proj.failure_kind.as_deref(), Some("review_rejection"));
    }

    fn reviewed_worktree_task(bus: &mut AgentA2ABus) -> AgentTaskId {
        use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};

        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement settings recovery polish",
            "Polish settings recovery",
            10,
        );
        bus.start_task(&task_id, 20);
        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "meta-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::Evidence,
                title: "Worktree metadata".to_string(),
                content: serde_json::json!({
                    "worktree_path": "/tmp/forge-review-task-1",
                    "cleaned_up": false,
                    "diff_available": true,
                    "diff_truncated": false,
                    "tests_passed": true,
                    "needs_human_review": true,
                    "suggested_action": "Review and merge after checking settings recovery.",
                    "reason_codes": ["tests_passed", "diff_available"],
                })
                .to_string(),
                created_at_ms: 30,
            },
            30,
        );
        bus.complete_task(&task_id, "Patch ready for controller review", 40);
        task_id
    }

    // ── Phase 4-A tests: enriched projection fields ──

    #[test]
    fn compute_duration_returns_correct_value() {
        assert_eq!(compute_duration_ms(None, None), None);
        assert_eq!(compute_duration_ms(Some(100), Some(200)), Some(100));
        assert_eq!(compute_duration_ms(Some(100), None), None);
        assert_eq!(compute_duration_ms(Some(200), Some(100)), Some(0)); // clock skew guard
    }

    #[test]
    fn projection_includes_parent_task_id() {
        let mut bus = AgentA2ABus::default();
        let parent_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Parent",
            "parent prompt",
            10,
        );
        bus.start_task(&parent_id, 20);

        // Simulate a child task by pushing a record with a parent_task_id.
        let child_id = AgentTaskId::new("child-1");
        let mut child = AgentTaskRecord::new(
            child_id.clone(),
            AgentId::new("child-agent"),
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Child implementer",
            "child prompt",
            25,
        );
        child.parent_task_id = Some(parent_id.clone());
        child.started_at_ms = Some(30);
        child.ended_at_ms = Some(40);
        child.status = AgentTaskStatus::Completed;
        bus.tasks.push(child);

        let projection = bus.projection();
        // Should have 2 tasks.
        assert_eq!(projection.tasks.len(), 2);
        let child_proj = &projection.tasks[1];
        assert_eq!(child_proj.task_id, "child-1");
        assert_eq!(
            child_proj.parent_task_id.as_deref(),
            Some(parent_id.as_str())
        );
    }

    #[test]
    fn projection_derives_parent_child_task_ids_in_creation_order() {
        let mut bus = AgentA2ABus::default();
        let parent_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Parent",
            "parent prompt",
            10,
        );
        let child_a_id = bus
            .assign_child_task(
                &parent_id,
                AgentRole::Implementer,
                AgentExecutionMode::WorktreeWorker,
                "Child A",
                "child A prompt",
                20,
            )
            .expect("child A assigned");
        let child_b_id = bus
            .assign_child_task(
                &parent_id,
                AgentRole::Reviewer,
                AgentExecutionMode::PatchProposal,
                "Child B",
                "child B prompt",
                30,
            )
            .expect("child B assigned");
        let root_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Root",
            "root prompt",
            40,
        );

        let projection = bus.projection();
        let parent_projection = projection
            .tasks
            .iter()
            .find(|task| task.task_id == parent_id.as_str())
            .expect("parent projection");
        let child_projection = projection
            .tasks
            .iter()
            .find(|task| task.task_id == child_a_id.as_str())
            .expect("child projection");
        let root_projection = projection
            .tasks
            .iter()
            .find(|task| task.task_id == root_id.as_str())
            .expect("root projection");

        assert_eq!(
            parent_projection.child_task_ids,
            vec![
                child_a_id.as_str().to_string(),
                child_b_id.as_str().to_string()
            ]
        );
        let parent_json =
            serde_json::to_value(parent_projection).expect("serialize parent projection");
        assert_eq!(
            parent_json.get("child_task_ids"),
            Some(&serde_json::json!([
                child_a_id.as_str(),
                child_b_id.as_str()
            ]))
        );
        assert_eq!(
            child_projection.parent_task_id.as_deref(),
            Some(parent_id.as_str())
        );
        assert!(child_projection.child_task_ids.is_empty());
        assert!(root_projection.child_task_ids.is_empty());

        let root_json = serde_json::to_value(root_projection).expect("serialize root projection");
        assert!(root_json.get("child_task_ids").is_none());
    }

    #[test]
    fn assign_child_task_persists_parent_child_task_ids() {
        let mut bus = AgentA2ABus::default();
        let parent_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Parent",
            "parent prompt",
            10,
        );
        let child_id = bus
            .assign_child_task(
                &parent_id,
                AgentRole::Implementer,
                AgentExecutionMode::WorktreeWorker,
                "Child implementer",
                "child prompt",
                20,
            )
            .expect("child task assigned");

        let parent = bus.task(&parent_id).expect("parent task");
        let child = bus.task(&child_id).expect("child task");

        assert_eq!(child.parent_task_id.as_ref(), Some(&parent_id));
        assert_eq!(parent.child_task_ids, vec![child_id]);
    }

    #[test]
    fn projection_prefers_persisted_child_task_ids_and_appends_legacy_ids() {
        let mut bus = AgentA2ABus::default();
        let parent_id = AgentTaskId::new("parent");
        let child_a_id = AgentTaskId::new("child-a");
        let child_b_id = AgentTaskId::new("child-b");
        let mut parent = AgentTaskRecord::new(
            parent_id.clone(),
            AgentId::new("parent-agent"),
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Parent",
            "parent prompt",
            10,
        );
        assert!(parent.record_child_task_id(child_b_id.clone()));
        assert!(!parent.record_child_task_id(child_b_id.clone()));
        let mut child_a = AgentTaskRecord::new(
            child_a_id.clone(),
            AgentId::new("child-a-agent"),
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Child A",
            "child A prompt",
            20,
        );
        child_a.parent_task_id = Some(parent_id.clone());
        let mut child_b = AgentTaskRecord::new(
            child_b_id.clone(),
            AgentId::new("child-b-agent"),
            AgentRole::Reviewer,
            AgentExecutionMode::PatchProposal,
            "Child B",
            "child B prompt",
            30,
        );
        child_b.parent_task_id = Some(parent_id.clone());
        bus.tasks.push(parent);
        bus.tasks.push(child_a);
        bus.tasks.push(child_b);

        let projection = bus.projection();
        let parent_projection = projection
            .tasks
            .iter()
            .find(|task| task.task_id == parent_id.as_str())
            .expect("parent projection");

        assert_eq!(
            parent_projection.child_task_ids,
            vec![
                child_b_id.as_str().to_string(),
                child_a_id.as_str().to_string()
            ]
        );
    }

    #[test]
    fn assign_child_task_populates_parent_and_assign_task_stays_root() {
        let mut bus = AgentA2ABus::default();
        let parent_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Parent",
            "parent prompt",
            10,
        );
        let child_id = bus
            .assign_child_task(
                &parent_id,
                AgentRole::Implementer,
                AgentExecutionMode::WorktreeWorker,
                "Child implementer",
                "child prompt",
                20,
            )
            .expect("child task assigned");
        let root_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Root",
            "root prompt",
            30,
        );

        let parent = bus.task(&parent_id).expect("parent task");
        let child = bus.task(&child_id).expect("child task");
        let root = bus.task(&root_id).expect("root task");

        assert_eq!(parent.parent_task_id, None);
        assert_eq!(child.parent_task_id.as_ref(), Some(&parent_id));
        assert_eq!(root.parent_task_id, None);

        let projection = bus.projection();
        let child_projection = projection
            .tasks
            .iter()
            .find(|task| task.task_id == child_id.as_str())
            .expect("child projection");
        let root_projection = projection
            .tasks
            .iter()
            .find(|task| task.task_id == root_id.as_str())
            .expect("root projection");
        assert_eq!(
            child_projection.parent_task_id.as_deref(),
            Some(parent_id.as_str())
        );
        assert_eq!(root_projection.parent_task_id, None);
    }

    #[test]
    fn assign_child_task_rejects_missing_parent_without_mutating_bus() {
        let mut bus = AgentA2ABus::default();
        let before = bus.clone();
        let missing_parent_id = AgentTaskId::new("missing-parent");
        let result = bus.assign_child_task(
            &missing_parent_id,
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Child implementer",
            "child prompt",
            20,
        );

        assert!(
            result.is_err(),
            "missing parents must reject child assignment"
        );
        assert!(
            bus.projection().tasks.is_empty(),
            "rejecting a missing parent must leave the bus unchanged"
        );
        assert_eq!(bus, before);
    }

    #[test]
    fn parent_task_id_survives_bus_serialization_roundtrip() {
        let mut bus = AgentA2ABus::default();
        let parent_id = crate::agent::a2a::supervisor::assign_delegate_task(
            &mut bus,
            "Parent review",
            "Review plan",
            10,
        );
        let child_id = crate::agent::a2a::supervisor::assign_worktree_worker_child_task(
            &mut bus,
            &parent_id,
            "Child worker",
            "Implement plan",
            20,
        )
        .expect("child task assigned");

        let json = serde_json::to_string(&bus).expect("serialize bus");
        let restored: AgentA2ABus = serde_json::from_str(&json).expect("deserialize bus");
        let projection = restored.projection();
        let child_projection = projection
            .tasks
            .iter()
            .find(|task| task.task_id == child_id.as_str())
            .expect("child projection");

        assert_eq!(
            child_projection.parent_task_id.as_deref(),
            Some(parent_id.as_str())
        );
    }

    #[test]
    fn parent_child_task_ids_survive_bus_serialization_roundtrip() {
        let mut bus = AgentA2ABus::default();
        let parent_id = crate::agent::a2a::supervisor::assign_delegate_task(
            &mut bus,
            "Parent review",
            "Review plan",
            10,
        );
        let child_id = crate::agent::a2a::supervisor::assign_worktree_worker_child_task(
            &mut bus,
            &parent_id,
            "Child worker",
            "Implement plan",
            20,
        )
        .expect("child task assigned");

        let json = serde_json::to_string(&bus).expect("serialize bus");
        let restored: AgentA2ABus = serde_json::from_str(&json).expect("deserialize bus");
        let parent = restored.task(&parent_id).expect("parent task");
        let projection = restored.projection();
        let parent_projection = projection
            .tasks
            .iter()
            .find(|task| task.task_id == parent_id.as_str())
            .expect("parent projection");

        assert_eq!(parent.child_task_ids, vec![child_id.clone()]);
        assert_eq!(
            parent_projection.child_task_ids,
            vec![child_id.as_str().to_string()]
        );
    }

    #[test]
    fn parent_session_context_roundtrips_and_defaults() {
        let mut bus = AgentA2ABus::default();
        let root_task_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Root planning task",
            "Plan child work",
            10,
        );
        bus.set_parent_session_context(crate::agent::a2a::types::AgentParentSessionContext {
            parent_session_id: "session-1".to_string(),
            active_parent_task_id: root_task_id.clone(),
            root_task_id: root_task_id.clone(),
            selection_reason: "active planner selected child delegate".to_string(),
            updated_at_ms: 20,
        });

        let json = serde_json::to_string(&bus).expect("serialize bus");
        let restored: AgentA2ABus = serde_json::from_str(&json).expect("deserialize bus");
        let context = restored
            .parent_session_context()
            .expect("parent session context");

        assert_eq!(context.parent_session_id, "session-1");
        assert_eq!(context.active_parent_task_id, root_task_id);
        assert_eq!(context.root_task_id, root_task_id);
        assert_eq!(
            context.selection_reason,
            "active planner selected child delegate"
        );
        assert_eq!(context.updated_at_ms, 20);

        let legacy: AgentA2ABus = serde_json::from_value(serde_json::json!({
            "tasks": [],
            "messages": [],
            "next_task_index": 0,
            "next_agent_index": 0,
            "next_message_index": 0
        }))
        .expect("legacy bus without parent context");
        assert!(legacy.parent_session_context().is_none());
    }

    #[test]
    fn projection_includes_timing_fields() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::PatchProposal,
            "Propose fix",
            "Fix error",
            10,
        );
        bus.start_task(&task_id, 20);
        bus.complete_task(&task_id, "Done", 40);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.created_at_ms, 10);
        assert_eq!(task_proj.started_at_ms, Some(20));
        assert_eq!(task_proj.ended_at_ms, Some(40));
        assert_eq!(task_proj.duration_ms, Some(20));
    }

    #[test]
    fn task_lease_claim_records_owner_attempt_and_projection_fields() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement lifecycle",
            "Add durable worker state",
            10,
        );

        assert!(bus.claim_task_lease(&task_id, "worker-1", 20, 100));

        let task = bus.task(&task_id).expect("task");
        assert_eq!(task.lease_owner.as_deref(), Some("worker-1"));
        assert_eq!(task.lease_acquired_at_ms, Some(20));
        assert_eq!(task.lease_expires_at_ms, Some(120));
        assert_eq!(task.last_heartbeat_at_ms, Some(20));
        assert_eq!(task.attempt_count, 1);
        assert_eq!(task.max_attempts, 3);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.lease_owner.as_deref(), Some("worker-1"));
        assert_eq!(task_proj.lease_acquired_at_ms, Some(20));
        assert_eq!(task_proj.lease_expires_at_ms, Some(120));
        assert_eq!(task_proj.last_heartbeat_at_ms, Some(20));
        assert_eq!(task_proj.attempt_count, 1);
        assert_eq!(task_proj.max_attempts, 3);
    }

    #[test]
    fn task_lease_heartbeat_extends_only_current_unexpired_owner() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement lifecycle",
            "Add durable worker state",
            10,
        );

        assert!(bus.claim_task_lease(&task_id, "worker-1", 20, 100));
        assert!(!bus.heartbeat_task_lease(&task_id, "worker-2", 40, 100));
        assert!(bus.heartbeat_task_lease(&task_id, "worker-1", 50, 100));

        let task = bus.task(&task_id).expect("task");
        assert_eq!(task.last_heartbeat_at_ms, Some(50));
        assert_eq!(task.lease_expires_at_ms, Some(150));

        assert!(!bus.heartbeat_task_lease(&task_id, "worker-1", 151, 100));
        let task = bus.task(&task_id).expect("task");
        assert_eq!(task.last_heartbeat_at_ms, Some(50));
        assert_eq!(task.lease_expires_at_ms, Some(150));
    }

    #[test]
    fn cancel_task_clears_active_lease_and_marks_cancelled() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement lifecycle",
            "Add durable worker state",
            10,
        );
        assert!(bus.claim_task_lease(&task_id, "worker-1", 20, 100));

        assert!(bus.cancel_task(&task_id, "user_cancelled", 30));

        let task = bus.task(&task_id).expect("task");
        assert_eq!(task.status, AgentTaskStatus::Cancelled);
        assert_eq!(task.ended_at_ms, Some(30));
        assert!(task.lease_owner.is_none());
        assert!(task.lease_expires_at_ms.is_none());
        assert_eq!(
            bus.projection().tasks[0].latest_message.as_deref(),
            Some("user_cancelled")
        );
    }

    #[test]
    fn retry_task_requeues_retryable_failure_and_preserves_attempt_count() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement lifecycle",
            "Add durable worker state",
            10,
        );
        assert!(bus.claim_task_lease(&task_id, "worker-1", 20, 100));
        bus.fail_task(&task_id, "tool_error", "worker failed", true, 30);

        assert!(bus.retry_task(&task_id, 40));

        let task = bus.task(&task_id).expect("task");
        assert_eq!(task.status, AgentTaskStatus::Pending);
        assert_eq!(task.attempt_count, 1);
        assert!(task.failure.is_none());
        assert!(task.ended_at_ms.is_none());
        assert!(task.lease_owner.is_none());
        assert_eq!(
            bus.projection().tasks[0].latest_progress.as_deref(),
            Some("Retry scheduled")
        );
    }

    #[test]
    fn retry_task_rejects_non_retryable_or_exhausted_tasks() {
        let mut bus = AgentA2ABus::default();
        let non_retryable = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Non retryable",
            "No retry",
            10,
        );
        bus.fail_task(&non_retryable, "review_rejected", "no retry", false, 20);

        assert!(!bus.retry_task(&non_retryable, 30));

        let exhausted = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Exhausted",
            "No attempts left",
            40,
        );
        assert!(bus.claim_task_lease(&exhausted, "worker-1", 50, 100));
        assert!(bus.claim_task_lease(&exhausted, "worker-1", 60, 100));
        assert!(bus.claim_task_lease(&exhausted, "worker-1", 70, 100));
        bus.fail_task(&exhausted, "tool_error", "failed", true, 80);

        assert!(!bus.retry_task(&exhausted, 90));
        assert_eq!(bus.task(&exhausted).expect("task").attempt_count, 3);
    }

    #[test]
    fn projection_includes_failure_kind_and_retryable() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Reviewer,
            AgentExecutionMode::ReadOnly,
            "Review",
            "review prompt",
            10,
        );
        bus.start_task(&task_id, 20);
        bus.fail_task(&task_id, "tool_error", "bash failed", true, 30);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.status, "failed");
        assert_eq!(task_proj.failure_message.as_deref(), Some("bash failed"));
        assert_eq!(task_proj.failure_kind.as_deref(), Some("tool_error"));
        assert_eq!(task_proj.retryable, Some(true));
    }

    #[test]
    fn projection_includes_failure_not_retryable() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement",
            "prompt",
            10,
        );
        bus.fail_task(&task_id, "smoke_failure", "smoke test failed", false, 20);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.failure_kind.as_deref(), Some("smoke_failure"));
        assert_eq!(task_proj.retryable, Some(false));
    }

    #[test]
    fn projection_includes_resume_note_for_interrupted_task() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement auth",
            "Add login flow",
            10,
        );
        bus.start_task(&task_id, 20);
        bus.normalize_for_resume(30);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.status, "interrupted");
        assert!(task_proj
            .resume_note
            .as_ref()
            .is_some_and(|n| n.contains("session was restored")));
    }

    #[test]
    fn projection_includes_latest_progress_from_progress_message() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Inspect",
            "inspect prompt",
            10,
        );
        bus.start_task(&task_id, 20);
        bus.record_progress(&task_id, "Reading session.rs line 100", 25);
        bus.record_progress(&task_id, "Found delegate_task call", 30);
        bus.complete_task(&task_id, "Done", 40);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(
            task_proj.latest_progress.as_deref(),
            Some("Found delegate_task call")
        );
    }

    #[test]
    fn projection_latest_progress_none_when_no_progress() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Researcher,
            AgentExecutionMode::ReadOnly,
            "Inspect",
            "inspect prompt",
            10,
        );
        bus.start_task(&task_id, 20);
        bus.complete_task(&task_id, "Done", 30);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.latest_progress, None);
    }

    #[test]
    fn projection_leaves_duration_empty_for_running_task() {
        // Running elapsed time is a frontend concern because it changes while the task is live.
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement auth",
            "prompt",
            10,
        );
        bus.start_task(&task_id, 20);
        // Record progress bumps updated_at_ms.
        bus.record_progress(&task_id, "Working...", 30);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.started_at_ms, Some(20));
        assert_eq!(task_proj.ended_at_ms, None);
        assert_eq!(task_proj.duration_ms, None);
    }

    // ── Phase 4-B tests: diff file extraction and projection enrichment ──

    #[test]
    fn extract_files_from_modified_diff() {
        let diff = "diff --git a/src/main.rs b/src/main.rs\nindex abc..def 100644\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@\n+// new line";
        let files = extract_files_from_diff_text(diff);
        assert_eq!(files, vec!["src/main.rs"]);
    }

    #[test]
    fn extract_files_from_added_diff() {
        let diff = "diff --git a/src/new.rs b/src/new.rs\nnew file mode 100644\n--- /dev/null\n+++ b/src/new.rs\n@@ -0,0 +1 @@\n+fn main() {}";
        let files = extract_files_from_diff_text(diff);
        assert_eq!(files, vec!["src/new.rs"]);
    }

    #[test]
    fn extract_files_from_deleted_diff() {
        let diff = "diff --git a/src/old.rs b/src/old.rs\ndeleted file mode 100644\n--- a/src/old.rs\n+++ /dev/null\n@@ -1,1 +0,0 @@\n-fn old() {}";
        let files = extract_files_from_diff_text(diff);
        assert_eq!(files, vec!["src/old.rs"]);
    }

    #[test]
    fn extract_files_from_rename_diff_prefers_new_path() {
        let diff = "diff --git a/src/old_name.rs b/src/new_name.rs\nsimilarity index 95%\nrename from src/old_name.rs\nrename to src/new_name.rs\n--- a/src/old_name.rs\n+++ b/src/new_name.rs\n@@ -1 +1 @@\n-old\n+new";
        let files = extract_files_from_diff_text(diff);
        assert_eq!(files, vec!["src/new_name.rs"]);
    }

    #[test]
    fn extract_files_falls_back_to_patch_headers_without_diff_git_line() {
        let diff = "--- a/src/fallback.rs\n+++ b/src/fallback.rs\n@@ -1 +1 @@\n-old\n+new";
        let files = extract_files_from_diff_text(diff);
        assert_eq!(files, vec!["src/fallback.rs"]);
    }

    #[test]
    fn extract_files_deduplicates_paths() {
        let diff = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1,2 @@\n a\n+b\ndiff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -5 +5,2 @@\n c\n+d";
        let files = extract_files_from_diff_text(diff);
        assert_eq!(files, vec!["src/main.rs"]);
    }

    #[test]
    fn extract_files_truncation_handled_by_caller() {
        // extract_files_from_diff_text returns all unique paths; caller limits to 8.
        let diff = (0..12)
            .map(|i| format!("diff --git a/file{0}.rs b/file{0}.rs\n--- a/file{0}.rs\n+++ b/file{0}.rs\n@@ -1 +1 @@\n-x\n+y\n", i))
            .collect::<Vec<_>>()
            .join("");
        let all = extract_files_from_diff_text(&diff);
        assert_eq!(all.len(), 12);
        let limited: Vec<String> = all.iter().take(8).cloned().collect();
        assert_eq!(limited.len(), 8);
    }

    #[test]
    fn projection_no_diff_when_no_diff_artifact() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "No diff",
            "no diff prompt",
            10,
        );
        bus.start_task(&task_id, 20);
        bus.complete_task(&task_id, "Done", 30);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.changed_file_count, None);
        assert!(task_proj.changed_files.is_empty());
        assert_eq!(task_proj.test_report_excerpt, None);
    }

    #[test]
    fn projection_extracts_changed_files_from_diff_summary_artifact() {
        use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement feature",
            "prompt",
            10,
        );
        bus.start_task(&task_id, 20);

        let diff_content = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1,2 @@\n a\n+b\ndiff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1,2 @@\n c\n+d\ndiff --git a/src/new.rs b/src/new.rs\nnew file mode 100644\n--- /dev/null\n+++ b/src/new.rs\n@@ -0,0 +1 @@\n+fn main() {}";
        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "diff-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::DiffSummary,
                title: "Worktree diff".to_string(),
                content: diff_content.to_string(),
                created_at_ms: 25,
            },
            25,
        );
        bus.complete_task(&task_id, "Done", 30);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.changed_file_count, Some(3));
        assert_eq!(task_proj.changed_files.len(), 3);
        assert!(task_proj.changed_files.contains(&"src/main.rs".to_string()));
        assert!(task_proj.changed_files.contains(&"src/lib.rs".to_string()));
        assert!(task_proj.changed_files.contains(&"src/new.rs".to_string()));
    }

    #[test]
    fn projection_changed_files_limited_to_eight() {
        use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Many files",
            "prompt",
            10,
        );
        bus.start_task(&task_id, 20);

        // Build a diff with 15 unique files.
        let diff_content = (0..15)
            .map(|i| format!("diff --git a/file{0}.rs b/file{0}.rs\n--- a/file{0}.rs\n+++ b/file{0}.rs\n@@ -1 +1 @@\n-x\n+y\n", i))
            .collect::<Vec<_>>()
            .join("");
        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "diff-big".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::DiffSummary,
                title: "Worktree diff".to_string(),
                content: diff_content,
                created_at_ms: 25,
            },
            25,
        );
        bus.complete_task(&task_id, "Done", 30);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        // Total count reflects all 15 unique files.
        assert_eq!(task_proj.changed_file_count, Some(15));
        // But the projection list is capped at 8.
        assert_eq!(task_proj.changed_files.len(), 8);
    }

    #[test]
    fn projection_extracts_diff_available_from_worktree_metadata() {
        use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "Implement auth",
            "prompt",
            10,
        );
        bus.start_task(&task_id, 20);

        let meta = serde_json::json!({
            "worktree_path": "/tmp/wt1",
            "diff_available": true,
        });
        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "meta-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::Evidence,
                title: "Worktree metadata".to_string(),
                content: meta.to_string(),
                created_at_ms: 25,
            },
            25,
        );
        bus.complete_task(&task_id, "Done", 30);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.diff_available, Some(true));
    }

    #[test]
    fn projection_diff_available_none_when_no_worktree_meta() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Implementer,
            AgentExecutionMode::WorktreeWorker,
            "No meta",
            "prompt",
            10,
        );
        bus.start_task(&task_id, 20);
        bus.complete_task(&task_id, "Done", 30);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(task_proj.diff_available, None);
    }

    #[test]
    fn extract_test_report_excerpt_from_summary_field() {
        let content = r#"{"summary": "5 tests passed, 1 failed", "passed": 5, "failed": 1}"#;
        let excerpt = extract_test_report_excerpt(content);
        assert_eq!(excerpt.as_deref(), Some("5 tests passed, 1 failed"));
    }

    #[test]
    fn extract_test_report_excerpt_from_result_field() {
        let content = r#"{"result": "All tests pass", "exit_code": 0}"#;
        let excerpt = extract_test_report_excerpt(content);
        assert_eq!(excerpt.as_deref(), Some("All tests pass"));
    }

    #[test]
    fn extract_test_report_excerpt_fallback_to_first_line() {
        let content = "test result: ok. 3 passed; 0 failed;\n\nrunning 3 tests\n...";
        let excerpt = extract_test_report_excerpt(content);
        assert_eq!(
            excerpt.as_deref(),
            Some("test result: ok. 3 passed; 0 failed;")
        );
    }

    #[test]
    fn projection_includes_test_report_excerpt_from_artifact() {
        use crate::agent::a2a::types::{AgentArtifact, AgentArtifactKind};

        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::TestPlanner,
            AgentExecutionMode::WorktreeWorker,
            "Run tests",
            "prompt",
            10,
        );
        bus.start_task(&task_id, 20);

        bus.add_artifact(
            &task_id,
            AgentArtifact {
                artifact_id: "test-1".to_string(),
                task_id: task_id.clone(),
                kind: AgentArtifactKind::TestReport,
                title: "Test report".to_string(),
                content: r#"{"summary": "8 tests passed, 2 failed"}"#.to_string(),
                created_at_ms: 25,
            },
            25,
        );
        bus.complete_task(&task_id, "Done", 30);

        let projection = bus.projection();
        let task_proj = &projection.tasks[0];
        assert_eq!(
            task_proj.test_report_excerpt.as_deref(),
            Some("8 tests passed, 2 failed")
        );
    }
}
