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
        let mut interrupted = Vec::new();
        for task in &mut self.tasks {
            if task.status == AgentTaskStatus::Running {
                task.status = AgentTaskStatus::Interrupted;
                task.resume_note =
                    Some("child task was running when the session was restored".to_string());
                task.updated_at_ms = timestamp_ms;
                task.ended_at_ms = Some(timestamp_ms);
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
}
