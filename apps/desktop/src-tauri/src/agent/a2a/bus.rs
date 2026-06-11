use crate::agent::a2a::projection::{AgentA2AProjection, AgentA2ATaskProjection};
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
                AgentA2ATaskProjection {
                    task_id: task.task_id.as_str().to_string(),
                    agent_id: task.agent_id.as_str().to_string(),
                    role: task.role.as_str().to_string(),
                    execution_mode: task.execution_mode.as_str().to_string(),
                    status: task.status.as_str().to_string(),
                    title: task.title.clone(),
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
}
