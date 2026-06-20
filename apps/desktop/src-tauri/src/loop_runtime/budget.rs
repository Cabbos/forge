use crate::loop_runtime::gates::HumanGateType;
use crate::loop_runtime::types::LoopBudget;
use crate::protocol::events::ProviderUsageReason;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageEvent {
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub reason: ProviderUsageReason,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default)]
    pub cache_read_tokens: Option<u64>,
    #[serde(default)]
    pub cache_creation_tokens: Option<u64>,
    #[serde(default)]
    pub reasoning_tokens: Option<u64>,
    #[serde(default)]
    pub estimated_cost_micros: Option<u64>,
    #[serde(default)]
    pub pricing_source: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopUsageLedger {
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default)]
    pub cache_read_tokens: Option<u64>,
    #[serde(default)]
    pub cache_creation_tokens: Option<u64>,
    #[serde(default)]
    pub reasoning_tokens: Option<u64>,
    #[serde(default)]
    pub estimated_cost_micros: Option<u64>,
    #[serde(default)]
    pub pricing_source: Option<String>,
    #[serde(default)]
    pub has_unknown_input_tokens: bool,
    #[serde(default)]
    pub has_unknown_output_tokens: bool,
    #[serde(default)]
    pub has_unknown_cost: bool,
    #[serde(default)]
    pub turn_count: u32,
    #[serde(default)]
    pub tool_call_count: u32,
    #[serde(default)]
    pub elapsed_ms: u64,
}

impl LoopUsageLedger {
    pub fn from_events(events: Vec<UsageEvent>) -> Self {
        let has_events = !events.is_empty();
        let provider_id = first_stable_string(&events, |event| event.provider_id.as_deref());
        let model = first_stable_model(&events);
        let (input_tokens, has_unknown_input_tokens) =
            sum_optional_usage(events.iter().map(|event| event.input_tokens));
        let (output_tokens, has_unknown_output_tokens) =
            sum_optional_usage(events.iter().map(|event| event.output_tokens));
        let (cache_read_tokens, _) =
            sum_optional_usage(events.iter().map(|event| event.cache_read_tokens));
        let (cache_creation_tokens, _) =
            sum_optional_usage(events.iter().map(|event| event.cache_creation_tokens));
        let (reasoning_tokens, _) =
            sum_optional_usage(events.iter().map(|event| event.reasoning_tokens));
        let (estimated_cost_micros, has_unknown_cost) =
            sum_optional_usage(events.iter().map(|event| event.estimated_cost_micros));
        let pricing_source = first_stable_string(&events, |event| event.pricing_source.as_deref());

        Self {
            provider_id,
            model,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            reasoning_tokens,
            estimated_cost_micros,
            pricing_source,
            has_unknown_input_tokens: has_unknown_input_tokens || !has_events,
            has_unknown_output_tokens: has_unknown_output_tokens || !has_events,
            has_unknown_cost: has_unknown_cost || !has_events,
            turn_count: 0,
            tool_call_count: 0,
            elapsed_ms: 0,
        }
    }

    pub fn with_runtime_counts(
        mut self,
        turn_count: u32,
        tool_call_count: u32,
        elapsed_ms: u64,
    ) -> Self {
        self.turn_count = turn_count;
        self.tool_call_count = tool_call_count;
        self.elapsed_ms = elapsed_ms;
        self
    }

    pub fn unknown(
        model: Option<String>,
        turn_count: u32,
        tool_call_count: u32,
        elapsed_ms: u64,
    ) -> Self {
        Self::from_events(vec![UsageEvent {
            provider_id: None,
            model,
            source: None,
            reason: ProviderUsageReason::ProviderOmitted,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            reasoning_tokens: None,
            estimated_cost_micros: None,
            pricing_source: None,
        }])
        .with_runtime_counts(turn_count, tool_call_count, elapsed_ms)
    }
}

fn first_stable_model(events: &[UsageEvent]) -> Option<String> {
    first_stable_string(events, |event| event.model.as_deref())
}

fn first_stable_string(
    events: &[UsageEvent],
    value: impl Fn(&UsageEvent) -> Option<&str>,
) -> Option<String> {
    let mut models = events
        .iter()
        .filter_map(value)
        .filter(|model| !model.trim().is_empty());
    let first = models.next()?.to_string();
    if models.all(|model| model == first) {
        Some(first)
    } else {
        None
    }
}

fn sum_optional_usage(values: impl Iterator<Item = Option<u64>>) -> (Option<u64>, bool) {
    let mut total = 0u64;
    let mut has_known = false;
    let mut has_unknown = false;
    for value in values {
        match value {
            Some(known) => {
                has_known = true;
                total = total.saturating_add(known);
            }
            None => has_unknown = true,
        }
    }
    (has_known.then_some(total), has_unknown)
}

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
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost_micros: Option<u64>,
    #[serde(default = "default_true")]
    pub has_unknown_token_usage: bool,
    #[serde(default = "default_true")]
    pub has_unknown_cost: bool,
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
            input_tokens: None,
            output_tokens: None,
            estimated_cost_micros: None,
            has_unknown_token_usage: true,
            has_unknown_cost: true,
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

fn default_true() -> bool {
    true
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
    use crate::loop_runtime::{
        BudgetSnapshot, HumanGateType, LoopActor, LoopBudget, LoopEventEnvelope, LoopRuntimeEvent,
        LoopTaskProjection, LoopUsageLedger, UsageEvent, LOOP_RUNTIME_SCHEMA_VERSION,
    };
    use crate::protocol::events::ProviderUsageReason;

    #[test]
    fn usage_ledger_sums_known_tokens_and_preserves_unknown_cost() {
        let usage = LoopUsageLedger::from_events(vec![
            UsageEvent {
                provider_id: Some("anthropic".into()),
                model: Some("claude".into()),
                source: Some("anthropic".into()),
                reason: ProviderUsageReason::PricingUnknown,
                input_tokens: Some(100),
                output_tokens: Some(50),
                cache_read_tokens: None,
                cache_creation_tokens: None,
                reasoning_tokens: None,
                estimated_cost_micros: None,
                pricing_source: None,
            },
            UsageEvent {
                provider_id: Some("anthropic".into()),
                model: Some("claude".into()),
                source: Some("anthropic".into()),
                reason: ProviderUsageReason::ProviderReported,
                input_tokens: Some(25),
                output_tokens: None,
                cache_read_tokens: None,
                cache_creation_tokens: None,
                reasoning_tokens: None,
                estimated_cost_micros: Some(10),
                pricing_source: Some("forge_static_pricing_2026_06_20".into()),
            },
        ]);

        assert_eq!(usage.input_tokens, Some(125));
        assert_eq!(usage.output_tokens, Some(50));
        assert_eq!(usage.estimated_cost_micros, Some(10));
        assert!(usage.has_unknown_output_tokens);
        assert!(usage.has_unknown_cost);
    }

    #[test]
    fn usage_ledger_preserves_known_tokens_when_pricing_is_unknown() {
        let usage = LoopUsageLedger::from_events(vec![UsageEvent {
            provider_id: Some("custom_openai".into()),
            model: Some("mystery-model".into()),
            source: Some("openai_compatible".into()),
            reason: ProviderUsageReason::PricingUnknown,
            input_tokens: Some(77),
            output_tokens: Some(33),
            cache_read_tokens: None,
            cache_creation_tokens: None,
            reasoning_tokens: None,
            estimated_cost_micros: None,
            pricing_source: None,
        }]);

        assert_eq!(usage.provider_id.as_deref(), Some("custom_openai"));
        assert_eq!(usage.model.as_deref(), Some("mystery-model"));
        assert_eq!(usage.input_tokens, Some(77));
        assert_eq!(usage.output_tokens, Some(33));
        assert_eq!(usage.estimated_cost_micros, None);
        assert!(!usage.has_unknown_input_tokens);
        assert!(!usage.has_unknown_output_tokens);
        assert!(usage.has_unknown_cost);
    }

    #[test]
    fn usage_ledger_sums_cache_reasoning_tokens_and_preserves_pricing_source() {
        let usage = LoopUsageLedger::from_events(vec![
            UsageEvent {
                provider_id: Some("anthropic".into()),
                model: Some("claude-sonnet".into()),
                source: Some("anthropic".into()),
                reason: ProviderUsageReason::ProviderReported,
                input_tokens: Some(100),
                output_tokens: Some(50),
                cache_read_tokens: Some(20),
                cache_creation_tokens: None,
                reasoning_tokens: Some(7),
                estimated_cost_micros: Some(10),
                pricing_source: Some("forge_static_pricing_2026_06_20".into()),
            },
            UsageEvent {
                provider_id: Some("anthropic".into()),
                model: Some("claude-sonnet".into()),
                source: Some("anthropic".into()),
                reason: ProviderUsageReason::ProviderReported,
                input_tokens: Some(25),
                output_tokens: Some(5),
                cache_read_tokens: Some(2),
                cache_creation_tokens: Some(3),
                reasoning_tokens: Some(11),
                estimated_cost_micros: Some(2),
                pricing_source: Some("forge_static_pricing_2026_06_20".into()),
            },
        ]);

        assert_eq!(usage.provider_id.as_deref(), Some("anthropic"));
        assert_eq!(usage.cache_read_tokens, Some(22));
        assert_eq!(usage.cache_creation_tokens, Some(3));
        assert_eq!(usage.reasoning_tokens, Some(18));
        assert_eq!(usage.estimated_cost_micros, Some(12));
        assert_eq!(
            usage.pricing_source.as_deref(),
            Some("forge_static_pricing_2026_06_20")
        );
    }

    #[test]
    fn usage_ledger_marks_empty_usage_as_unknown() {
        let usage = LoopUsageLedger::from_events(Vec::new());

        assert_eq!(usage.input_tokens, None);
        assert_eq!(usage.output_tokens, None);
        assert_eq!(usage.estimated_cost_micros, None);
        assert!(usage.has_unknown_input_tokens);
        assert!(usage.has_unknown_output_tokens);
        assert!(usage.has_unknown_cost);
    }

    #[test]
    fn usage_ledger_serializes_unknown_usage_as_explicit_nulls() {
        let usage = LoopUsageLedger::unknown(Some("claude".to_string()), 2, 3, 4000);

        let json = serde_json::to_value(usage).unwrap();

        assert_eq!(json["input_tokens"], serde_json::Value::Null);
        assert_eq!(json["output_tokens"], serde_json::Value::Null);
        assert_eq!(json["estimated_cost_micros"], serde_json::Value::Null);
        assert_eq!(json["has_unknown_input_tokens"], true);
        assert_eq!(json["has_unknown_output_tokens"], true);
        assert_eq!(json["has_unknown_cost"], true);
        assert_eq!(json["turn_count"], 2);
        assert_eq!(json["tool_call_count"], 3);
        assert_eq!(json["elapsed_ms"], 4000);
    }

    #[test]
    fn usage_event_serializes_unknown_usage_as_explicit_nulls() {
        let event = UsageEvent {
            provider_id: Some("anthropic".to_string()),
            model: Some("claude".to_string()),
            source: Some("anthropic".to_string()),
            reason: ProviderUsageReason::ProviderOmitted,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            reasoning_tokens: None,
            estimated_cost_micros: None,
            pricing_source: None,
        };

        let json = serde_json::to_value(event).unwrap();

        assert_eq!(json["provider_id"], "anthropic");
        assert_eq!(json["input_tokens"], serde_json::Value::Null);
        assert_eq!(json["output_tokens"], serde_json::Value::Null);
        assert_eq!(json["cache_read_tokens"], serde_json::Value::Null);
        assert_eq!(json["cache_creation_tokens"], serde_json::Value::Null);
        assert_eq!(json["reasoning_tokens"], serde_json::Value::Null);
        assert_eq!(json["estimated_cost_micros"], serde_json::Value::Null);
        assert_eq!(json["pricing_source"], serde_json::Value::Null);
        assert_eq!(json["source"], "anthropic");
        assert_eq!(json["reason"], "provider_omitted");
    }

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
            input_tokens: None,
            output_tokens: None,
            estimated_cost_micros: None,
            has_unknown_token_usage: true,
            has_unknown_cost: true,
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

    #[test]
    fn old_budget_snapshot_json_defaults_unknown_token_and_cost_flags() {
        let json = serde_json::json!({
            "budget_exceeded": false,
            "model_call_in_flight": false,
            "tool_call_started": false,
            "long_running_tool_supports_cancel": false,
            "model_rounds_used": 1,
            "tool_calls_used": 2,
            "elapsed_ms": 3000,
            "estimated_cost_micros": null
        });

        let snapshot: BudgetSnapshot = serde_json::from_value(json).unwrap();

        assert_eq!(snapshot.input_tokens, None);
        assert_eq!(snapshot.output_tokens, None);
        assert_eq!(snapshot.estimated_cost_micros, None);
        assert!(snapshot.has_unknown_token_usage);
        assert!(snapshot.has_unknown_cost);
    }

    #[test]
    fn budget_snapshot_recorded_preserves_unknown_token_and_cost_facts() {
        let created = LoopEventEnvelope::task_created_for_test("loop-1", "track budget");
        let snapshot = BudgetSnapshot {
            budget_exceeded: false,
            model_call_in_flight: false,
            tool_call_started: false,
            long_running_tool_supports_cancel: false,
            model_rounds_used: 1,
            tool_calls_used: 2,
            elapsed_ms: 3000,
            input_tokens: None,
            output_tokens: None,
            estimated_cost_micros: None,
            has_unknown_token_usage: true,
            has_unknown_cost: true,
        };
        let recorded = LoopEventEnvelope {
            schema_version: LOOP_RUNTIME_SCHEMA_VERSION,
            event_id: "event-loop-1-budget".to_string(),
            task_id: "loop-1".to_string(),
            sequence: 2,
            event: LoopRuntimeEvent::BudgetSnapshotRecorded {
                task_id: "loop-1".to_string(),
                snapshot: snapshot.clone(),
            },
            actor: LoopActor::Gateway,
            lease_id: None,
            attempt: None,
            correlation_id: None,
            causation_id: None,
            idempotency_key: None,
            created_at_ms: 2,
        };

        let projection = LoopTaskProjection::from_events(&[created, recorded]).unwrap();

        assert_eq!(projection.tasks[0].latest_budget_snapshot, Some(snapshot));
    }
}
