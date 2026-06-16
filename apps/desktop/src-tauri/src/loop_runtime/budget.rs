use crate::loop_runtime::gates::HumanGateType;
use crate::loop_runtime::types::LoopBudget;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetSnapshot {
    pub budget_exceeded: bool,
    pub model_call_in_flight: bool,
    pub tool_call_started: bool,
    pub long_running_tool_supports_cancel: bool,
    pub model_rounds_used: u32,
    pub tool_calls_used: u32,
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost_micros: Option<u64>,
}

impl BudgetSnapshot {
    pub fn empty() -> Self {
        Self {
            budget_exceeded: false,
            model_call_in_flight: false,
            tool_call_started: false,
            long_running_tool_supports_cancel: false,
            model_rounds_used: 0,
            tool_calls_used: 0,
            elapsed_ms: 0,
            estimated_cost_micros: None,
        }
    }

    #[cfg(test)]
    pub fn empty_for_test() -> Self {
        Self::empty()
    }

    pub fn decide(&self, budget: &LoopBudget) -> BudgetDecision {
        let exceeded = self.budget_exceeded
            || self.model_rounds_used >= budget.max_model_rounds
            || self.tool_calls_used >= budget.max_tool_calls
            || self.elapsed_ms >= budget.max_elapsed_ms
            || budget
                .max_estimated_cost_micros
                .zip(self.estimated_cost_micros)
                .is_some_and(|(max, used)| used >= max);

        if !exceeded {
            return BudgetDecision::allowed("within_loop_budget");
        }

        if self.model_call_in_flight {
            return BudgetDecision {
                allowed: true,
                reason: "budget_exceeded_wait_for_model_call".to_string(),
                request_human_gate: false,
                required_gate_type: None,
                wait_for_in_flight_model: true,
                allow_interrupt: false,
            };
        }

        if self.tool_call_started && self.long_running_tool_supports_cancel {
            return BudgetDecision {
                allowed: true,
                reason: "budget_exceeded_interrupt_allowed".to_string(),
                request_human_gate: true,
                required_gate_type: Some(HumanGateType::BudgetOverride),
                wait_for_in_flight_model: false,
                allow_interrupt: true,
            };
        }

        BudgetDecision {
            allowed: false,
            reason: "budget_exceeded_requires_human_approval".to_string(),
            request_human_gate: true,
            required_gate_type: Some(HumanGateType::BudgetOverride),
            wait_for_in_flight_model: false,
            allow_interrupt: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetDecision {
    pub allowed: bool,
    pub reason: String,
    pub request_human_gate: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_gate_type: Option<HumanGateType>,
    pub wait_for_in_flight_model: bool,
    pub allow_interrupt: bool,
}

impl BudgetDecision {
    fn allowed(reason: &str) -> Self {
        Self {
            allowed: true,
            reason: reason.to_string(),
            request_human_gate: false,
            required_gate_type: None,
            wait_for_in_flight_model: false,
            allow_interrupt: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::loop_runtime::{BudgetSnapshot, HumanGateType, LoopBudget};

    #[test]
    fn budget_snapshot_blocks_not_started_tool_after_budget_exceeded() {
        let budget = LoopBudget {
            max_model_rounds: 1,
            max_tool_calls: 1,
            max_elapsed_ms: 1,
            max_estimated_cost_micros: None,
        };
        let snapshot = BudgetSnapshot {
            budget_exceeded: false,
            model_call_in_flight: false,
            tool_call_started: false,
            long_running_tool_supports_cancel: false,
            model_rounds_used: 1,
            tool_calls_used: 1,
            elapsed_ms: 2,
            estimated_cost_micros: None,
        };

        let decision = snapshot.decide(&budget);

        assert!(!decision.allowed);
        assert!(decision.request_human_gate);
        assert_eq!(
            decision.required_gate_type,
            Some(HumanGateType::BudgetOverride)
        );
        assert_eq!(decision.reason, "budget_exceeded_requires_human_approval");
    }
}
