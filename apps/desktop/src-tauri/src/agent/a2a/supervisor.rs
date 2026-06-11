use crate::agent::a2a::bus::AgentA2ABus;
use crate::agent::a2a::types::{AgentExecutionMode, AgentRole, AgentTaskId};

pub(crate) fn delegate_result_for_model(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|value| {
            value
                .get("result")
                .and_then(|result| result.as_str())
                .map(|result| result.to_string())
        })
        .unwrap_or_else(|| raw.to_string())
}

pub(crate) fn assign_delegate_task(
    bus: &mut AgentA2ABus,
    title: &str,
    prompt: &str,
    timestamp_ms: u64,
) -> AgentTaskId {
    bus.assign_task(
        AgentRole::Researcher,
        AgentExecutionMode::ReadOnly,
        title,
        prompt,
        timestamp_ms,
    )
}

pub(crate) fn record_child_failure(
    bus: &mut AgentA2ABus,
    task_id: &AgentTaskId,
    kind: &str,
    message: &str,
    timestamp_ms: u64,
) {
    bus.fail_task(task_id, kind, message, true, timestamp_ms);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::a2a::bus::AgentA2ABus;

    #[test]
    fn delegate_result_for_model_extracts_json_result() {
        let raw = serde_json::json!({
            "result": "Found compact trigger in auto_compact.rs",
            "steps": []
        })
        .to_string();

        assert_eq!(
            delegate_result_for_model(&raw),
            "Found compact trigger in auto_compact.rs"
        );
    }

    #[test]
    fn join_error_records_failed_task() {
        let mut bus = AgentA2ABus::default();
        let task_id = bus.assign_task(
            crate::agent::a2a::types::AgentRole::Researcher,
            crate::agent::a2a::types::AgentExecutionMode::ReadOnly,
            "Read files",
            "Read files",
            10,
        );

        record_child_failure(&mut bus, &task_id, "join_error", "subagent panicked", 20);

        let projection = bus.projection();
        assert_eq!(projection.failed_count, 1);
        assert_eq!(
            projection.tasks[0].failure_message.as_deref(),
            Some("subagent panicked")
        );
    }
}
