use crate::loop_runtime::types::now_millis;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanGateRecord {
    pub gate_id: String,
    pub gate_type: HumanGateType,
    pub prompt: String,
    pub status: HumanGateStatus,
    pub requested_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<HumanGateDecision>,
}

impl HumanGateRecord {
    pub fn new(gate_id: String, gate_type: HumanGateType, prompt: String) -> Self {
        Self {
            gate_id,
            gate_type,
            prompt,
            status: HumanGateStatus::Open,
            requested_at_ms: now_millis(),
            decision: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HumanGateType {
    PolicyOverride,
    BudgetOverride,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HumanGateStatus {
    Open,
    Approved,
    Denied,
    Canceled,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HumanGateDecisionKind {
    Approved,
    Denied,
    Canceled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanGateDecision {
    pub kind: HumanGateDecisionKind,
    pub decided_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl HumanGateDecision {
    pub fn approved(decided_by: Option<String>, reason: Option<String>) -> Self {
        Self {
            kind: HumanGateDecisionKind::Approved,
            decided_at_ms: now_millis(),
            decided_by,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::loop_runtime::{
        HumanGateDecision, HumanGateDecisionKind, HumanGateType, LoopActor, LoopEventEnvelope,
        LoopRuntimeEvent, LoopTaskProjection, LoopTaskStatus, LOOP_RUNTIME_SCHEMA_VERSION,
    };

    #[test]
    fn human_gate_survives_projection_rebuild() {
        let events = vec![
            LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime"),
            LoopEventEnvelope::human_gate_requested_for_test(
                "loop-1",
                "gate-1",
                HumanGateType::PolicyOverride,
                "Approve dependency install",
            ),
        ];

        let projection = LoopTaskProjection::from_events(&events).unwrap();

        assert_eq!(projection.tasks[0].status, LoopTaskStatus::WaitingForReview);
        assert_eq!(projection.tasks[0].open_gates.len(), 1);
        assert_eq!(projection.tasks[0].open_gates[0].gate_id, "gate-1");
    }

    #[test]
    fn human_gate_denial_does_not_resume_task() {
        let events = vec![
            LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime"),
            LoopEventEnvelope::human_gate_requested_for_test(
                "loop-1",
                "gate-1",
                HumanGateType::PolicyOverride,
                "Approve dependency install",
            ),
            human_gate_resolved_for_test("loop-1", "gate-1", HumanGateDecisionKind::Denied, 3),
        ];

        let projection = LoopTaskProjection::from_events(&events).unwrap();

        assert_eq!(projection.tasks[0].status, LoopTaskStatus::WaitingForReview);
        assert!(projection.tasks[0].open_gates.is_empty());
        assert_eq!(
            projection.tasks[0].outcome.as_ref().unwrap().message,
            "human gate gate-1 denied"
        );
    }

    #[test]
    fn human_gate_cancel_does_not_resume_task() {
        let events = vec![
            LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime"),
            LoopEventEnvelope::human_gate_requested_for_test(
                "loop-1",
                "gate-1",
                HumanGateType::PolicyOverride,
                "Approve dependency install",
            ),
            human_gate_resolved_for_test("loop-1", "gate-1", HumanGateDecisionKind::Canceled, 3),
        ];

        let projection = LoopTaskProjection::from_events(&events).unwrap();

        assert_eq!(projection.tasks[0].status, LoopTaskStatus::WaitingForReview);
        assert!(projection.tasks[0].open_gates.is_empty());
        assert_eq!(
            projection.tasks[0].outcome.as_ref().unwrap().message,
            "human gate gate-1 canceled"
        );
    }

    #[test]
    fn human_gate_resolve_before_request_errors() {
        let events = vec![
            LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime"),
            human_gate_resolved_for_test("loop-1", "gate-1", HumanGateDecisionKind::Approved, 2),
        ];

        let error = LoopTaskProjection::from_events(&events).unwrap_err();

        assert!(error.contains("human gate resolved before request"));
    }

    #[test]
    fn duplicate_human_gate_request_with_different_payload_errors() {
        let mut duplicate = LoopEventEnvelope::human_gate_requested_for_test(
            "loop-1",
            "gate-1",
            HumanGateType::PolicyOverride,
            "Approve different thing",
        );
        duplicate.sequence = 3;

        let events = vec![
            LoopEventEnvelope::task_created_for_test("loop-1", "ship runtime"),
            LoopEventEnvelope::human_gate_requested_for_test(
                "loop-1",
                "gate-1",
                HumanGateType::PolicyOverride,
                "Approve dependency install",
            ),
            duplicate,
        ];

        let error = LoopTaskProjection::from_events(&events).unwrap_err();

        assert!(error.contains("duplicate human gate requested"));
    }

    fn human_gate_resolved_for_test(
        task_id: &str,
        gate_id: &str,
        kind: HumanGateDecisionKind,
        sequence: u64,
    ) -> LoopEventEnvelope {
        LoopEventEnvelope {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: format!("event-{task_id}-{gate_id}-resolved-{sequence}"),
            task_id: task_id.to_string(),
            sequence,
            event: LoopRuntimeEvent::HumanGateResolved {
                gate_id: gate_id.to_string(),
                decision: HumanGateDecision {
                    kind,
                    decided_at_ms: sequence,
                    decided_by: Some("reviewer".to_string()),
                    reason: Some("reviewed".to_string()),
                },
                resolved_at_ms: sequence,
            },
            actor: LoopActor::User {
                source: "test".to_string(),
            },
            correlation_id: None,
            idempotency_key: None,
            created_at_ms: sequence,
        }
    }
}
