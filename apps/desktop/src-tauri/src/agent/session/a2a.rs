use crate::agent::a2a::projection::AgentA2AProjection;
use crate::agent::event_sink::EventEmitter;
use crate::agent::session::AgentSession;
use crate::agent::session_guards::lock_unpoisoned;

impl AgentSession {
    pub(crate) fn a2a_projection(&self) -> AgentA2AProjection {
        lock_unpoisoned(&self.a2a_bus).projection()
    }

    pub(crate) fn emit_a2a_projection(&self, emitter: &dyn EventEmitter) {
        let state = self.a2a_projection();
        emitter.emit(crate::protocol::events::StreamEvent::AgentA2AUpdated {
            session_id: self.id.clone(),
            state,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::adapters::missing_key::MissingKeyAdapter;
    use crate::agent::a2a::bus::AgentA2ABus;
    use crate::agent::a2a::types::{AgentExecutionMode, AgentRole};
    use crate::agent::session::AgentSession;
    use crate::harness::Harness;

    #[test]
    fn a2a_projection_returns_current_bus_state() {
        let workspace = std::env::temp_dir().join(format!(
            "forge-session-a2a-projection-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let session = AgentSession::new(
            "session-1".to_string(),
            "openai".to_string(),
            Arc::new(MissingKeyAdapter::new("OpenAI", "gpt-5")),
            Arc::new(Harness::new(workspace.clone())),
            "system".to_string(),
            Some(128_000),
        );
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            AgentRole::Reviewer,
            AgentExecutionMode::ReadOnly,
            "Review runtime",
            "Inspect A2A state",
            10,
        );
        bus.complete_task(&task_id, "reviewed", 20);
        session.restore_state(Vec::new(), None, None, None, Some(bus));

        let projection = session.a2a_projection();

        assert_eq!(projection.completed_count, 1);
        assert_eq!(
            projection.tasks[0].latest_message.as_deref(),
            Some("reviewed")
        );

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn snapshot_restore_preserves_a2a_parent_task_id() {
        let workspace = std::env::temp_dir().join(format!(
            "forge-session-a2a-lineage-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let session = AgentSession::new(
            "session-lineage".to_string(),
            "openai".to_string(),
            Arc::new(MissingKeyAdapter::new("OpenAI", "gpt-5")),
            Arc::new(Harness::new(workspace.clone())),
            "system".to_string(),
            Some(128_000),
        );
        let mut bus = AgentA2ABus::default();
        let parent_id = crate::agent::a2a::supervisor::assign_delegate_task(
            &mut bus,
            "Parent review",
            "Review implementation plan",
            10,
        );
        let child_id = crate::agent::a2a::supervisor::assign_worktree_worker_child_task(
            &mut bus,
            &parent_id,
            "Child worktree worker",
            "Implement the reviewed plan",
            20,
        )
        .expect("child task assigned");
        session.restore_state(Vec::new(), None, None, None, Some(bus));

        let snapshot = session.snapshot();
        let restored_session = AgentSession::new(
            "session-lineage".to_string(),
            "openai".to_string(),
            Arc::new(MissingKeyAdapter::new("OpenAI", "gpt-5")),
            Arc::new(Harness::new(workspace.clone())),
            "system".to_string(),
            Some(128_000),
        );
        restored_session.restore_state(Vec::new(), None, None, None, snapshot.a2a_state);
        let projection = restored_session.a2a_projection();
        let child_projection = projection
            .tasks
            .iter()
            .find(|task| task.task_id == child_id.as_str())
            .expect("child projection");

        assert_eq!(
            child_projection.parent_task_id.as_deref(),
            Some(parent_id.as_str())
        );

        let _ = std::fs::remove_dir_all(workspace);
    }
}
