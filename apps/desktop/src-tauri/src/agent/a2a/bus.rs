use crate::agent::a2a::projection::{
    AgentA2AMessageProjection, AgentA2AProjection, AgentA2ATaskProjection,
};
use crate::agent::a2a::types::{
    AgentArtifact, AgentExecutionMode, AgentId, AgentMessage, AgentMessageKind, AgentRole,
    AgentTaskFailure, AgentTaskId, AgentTaskRecord, AgentTaskStatus,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AgentA2ABus {
    pub tasks: Vec<AgentTaskRecord>,
    pub messages: Vec<AgentMessage>,
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

    pub(crate) fn task(&self, task_id: &AgentTaskId) -> Option<&AgentTaskRecord> {
        self.tasks.iter().find(|task| task.task_id == *task_id)
    }

    pub(crate) fn claim_task_lease(
        &mut self,
        task_id: &AgentTaskId,
        owner: impl Into<String>,
        timestamp_ms: u64,
        lease_duration_ms: u64,
    ) -> bool {
        let owner = owner.into();
        let Some(task) = self.tasks.iter_mut().find(|task| task.task_id == *task_id) else {
            return false;
        };
        if !is_lease_claimable_status(&task.status) {
            return false;
        }
        if let Some(current_owner) = task.lease_owner.as_deref() {
            if current_owner != owner && !lease_is_expired(task, timestamp_ms) {
                return false;
            }
        }

        task.updated_at_ms = timestamp_ms;
        task.status = AgentTaskStatus::Running;
        task.started_at_ms.get_or_insert(timestamp_ms);
        task.ended_at_ms = None;
        task.resume_note = None;
        task.lease_owner = Some(owner);
        task.lease_acquired_at_ms = Some(timestamp_ms);
        task.lease_expires_at_ms = Some(lease_expires_at(timestamp_ms, lease_duration_ms));
        task.last_heartbeat_at_ms = Some(timestamp_ms);
        task.attempt_count = task.attempt_count.saturating_add(1);
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
                let mut worktree_meta: Option<serde_json::Value> = None;
                // Search for the most recent worktree metadata artifact (Evidence with title "Worktree metadata").
                for artifact in task.artifacts.iter().rev() {
                    if artifact.kind == crate::agent::a2a::types::AgentArtifactKind::Evidence
                        && artifact.title == "Worktree metadata"
                    {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&artifact.content)
                        {
                            worktree_meta = Some(v);
                        }
                        break;
                    }
                }
                // Phase 4-B: extract changed files from DiffSummary artifacts.
                let diff_text = task.artifacts.iter().rev().find_map(|a| {
                    if a.kind == crate::agent::a2a::types::AgentArtifactKind::DiffSummary
                        || a.title == "Worktree diff"
                    {
                        Some(a.content.as_str())
                    } else {
                        None
                    }
                });
                let all_diff_files = diff_text
                    .map(extract_files_from_diff_text)
                    .unwrap_or_default();
                // Limit to a small safe number (first 8 unique paths).
                let changed_files: Vec<String> = all_diff_files.iter().take(8).cloned().collect();
                let changed_file_count = if all_diff_files.is_empty() {
                    None
                } else {
                    Some(all_diff_files.len())
                };
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
                    // Phase 4-A enriched fields.
                    parent_task_id: task
                        .parent_task_id
                        .as_ref()
                        .map(|id| id.as_str().to_string()),
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
                    // Phase 4-B — diff-derived file visibility.
                    diff_available,
                    changed_file_count,
                    changed_files,
                    test_report_excerpt,
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
