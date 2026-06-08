/// Loop budget enforcement for the agent turn.
///
/// Tracks model rounds, tool calls, compact attempts, and overflow retries,
/// providing a structured stop reason when any budget is exhausted.
#[derive(Debug, Clone)]
pub(crate) struct LoopGuard {
    max_model_rounds: usize,
    max_tool_calls: usize,
    max_compact_attempts: usize,
    max_overflow_retries: usize,
    model_rounds: usize,
    tool_calls: usize,
    compact_attempts: usize,
    overflow_retries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoopStopReason {
    ModelRoundLimit,
    ToolCallLimit,
    CompactUnavailable,
    RepeatedOverflow,
}

impl LoopGuard {
    pub(crate) fn default_limits() -> Self {
        Self {
            max_model_rounds: 80,
            max_tool_calls: 200,
            max_compact_attempts: 10,
            max_overflow_retries: 3,
            model_rounds: 0,
            tool_calls: 0,
            compact_attempts: 0,
            overflow_retries: 0,
        }
    }

    pub(crate) fn with_max_model_rounds(mut self, limit: usize) -> Self {
        self.max_model_rounds = limit;
        self
    }

    pub(crate) fn with_max_tool_calls(mut self, limit: usize) -> Self {
        self.max_tool_calls = limit;
        self
    }

    pub(crate) fn record_model_round(&mut self) {
        self.model_rounds += 1;
    }

    pub(crate) fn record_tool_calls(&mut self, count: usize) {
        self.tool_calls += count;
    }

    pub(crate) fn record_compact_attempt(&mut self) {
        self.compact_attempts += 1;
    }

    pub(crate) fn record_overflow_retry(&mut self) {
        self.overflow_retries += 1;
    }

    /// Check whether the loop may continue.  If a budget is exhausted,
    /// return the stop reason that should be recorded on the turn.
    pub(crate) fn check(&self) -> Result<(), LoopStopReason> {
        if self.model_rounds >= self.max_model_rounds {
            return Err(LoopStopReason::ModelRoundLimit);
        }
        if self.tool_calls >= self.max_tool_calls {
            return Err(LoopStopReason::ToolCallLimit);
        }
        if self.compact_attempts >= self.max_compact_attempts {
            return Err(LoopStopReason::CompactUnavailable);
        }
        if self.overflow_retries >= self.max_overflow_retries {
            return Err(LoopStopReason::RepeatedOverflow);
        }
        Ok(())
    }

    pub(crate) fn model_rounds(&self) -> usize {
        self.model_rounds
    }

    pub(crate) fn tool_calls(&self) -> usize {
        self.tool_calls
    }

    pub(crate) fn overflow_retries(&self) -> usize {
        self.overflow_retries
    }

    pub(crate) fn reset(&mut self) {
        self.model_rounds = 0;
        self.tool_calls = 0;
        self.compact_attempts = 0;
        self.overflow_retries = 0;
    }
}

impl LoopStopReason {
    /// Human-readable stop reason string for TurnState.stop_reason.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            LoopStopReason::ModelRoundLimit => "model_round_limit",
            LoopStopReason::ToolCallLimit => "tool_call_limit",
            LoopStopReason::CompactUnavailable => "compact_unavailable",
            LoopStopReason::RepeatedOverflow => "repeated_overflow",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LoopGuard, LoopStopReason};

    #[test]
    fn default_guard_allows_many_iterations() {
        let guard = LoopGuard::default_limits();
        assert!(guard.check().is_ok());
    }

    #[test]
    fn model_round_limit_stops_loop() {
        let mut guard = LoopGuard::default_limits().with_max_model_rounds(3);
        guard.record_model_round();
        guard.record_model_round();
        assert!(guard.check().is_ok());

        guard.record_model_round();
        assert_eq!(guard.check(), Err(LoopStopReason::ModelRoundLimit));
    }

    #[test]
    fn tool_call_limit_stops_loop() {
        let mut guard = LoopGuard::default_limits().with_max_tool_calls(5);
        guard.record_tool_calls(3);
        assert!(guard.check().is_ok());

        guard.record_tool_calls(2);
        assert_eq!(guard.check(), Err(LoopStopReason::ToolCallLimit));
    }

    #[test]
    fn repeated_overflow_stops_loop() {
        let mut guard = LoopGuard::default_limits().with_max_model_rounds(100);
        guard.record_overflow_retry();
        guard.record_overflow_retry();
        assert!(guard.check().is_ok());

        guard.record_overflow_retry();
        assert_eq!(guard.check(), Err(LoopStopReason::RepeatedOverflow));
    }

    #[test]
    fn compact_unavailable_stops_loop() {
        let mut guard = LoopGuard::default_limits().with_max_model_rounds(100);
        for _ in 0..9 {
            guard.record_compact_attempt();
        }
        assert!(guard.check().is_ok());

        guard.record_compact_attempt();
        assert_eq!(guard.check(), Err(LoopStopReason::CompactUnavailable));
    }

    #[test]
    fn stop_reason_strings_are_stable() {
        assert_eq!(
            LoopStopReason::ModelRoundLimit.as_str(),
            "model_round_limit"
        );
        assert_eq!(LoopStopReason::ToolCallLimit.as_str(), "tool_call_limit");
        assert_eq!(
            LoopStopReason::CompactUnavailable.as_str(),
            "compact_unavailable"
        );
        assert_eq!(
            LoopStopReason::RepeatedOverflow.as_str(),
            "repeated_overflow"
        );
    }

    #[test]
    fn reset_clears_all_counters() {
        let mut guard = LoopGuard::default_limits().with_max_model_rounds(3);
        guard.record_model_round();
        guard.record_model_round();
        guard.record_tool_calls(5);
        guard.record_compact_attempt();
        guard.record_overflow_retry();

        assert_eq!(guard.model_rounds(), 2);
        assert_eq!(guard.tool_calls(), 5);

        guard.reset();

        assert_eq!(guard.model_rounds(), 0);
        assert_eq!(guard.tool_calls(), 0);
        assert!(guard.check().is_ok());

        // Should allow 3 more rounds after reset
        guard.record_model_round();
        guard.record_model_round();
        guard.record_model_round();
        assert_eq!(guard.check(), Err(LoopStopReason::ModelRoundLimit));
    }
}
