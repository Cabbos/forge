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
    max_repeated_tool_batches: usize,
    max_repeated_category_batches: usize,
    max_no_progress_batches: usize,
    model_rounds: usize,
    tool_calls: usize,
    compact_attempts: usize,
    overflow_retries: usize,
    repeated_tool_batches: usize,
    repeated_category_batches: usize,
    no_progress_batches: usize,
    last_tool_batch_signature: Option<String>,
    last_tool_category_signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoopStopReason {
    ModelRoundLimit,
    ToolCallLimit,
    CompactUnavailable,
    RepeatedOverflow,
    ToolLoopDetected,
    RepeatedNoProgress,
    RepeatedCategoryBatch,
}

impl LoopGuard {
    pub(crate) fn default_limits() -> Self {
        Self {
            max_model_rounds: 80,
            max_tool_calls: 200,
            max_compact_attempts: 10,
            max_overflow_retries: 3,
            max_repeated_tool_batches: 4,
            max_repeated_category_batches: 5,
            max_no_progress_batches: 4,
            model_rounds: 0,
            tool_calls: 0,
            compact_attempts: 0,
            overflow_retries: 0,
            repeated_tool_batches: 0,
            repeated_category_batches: 0,
            no_progress_batches: 0,
            last_tool_batch_signature: None,
            last_tool_category_signature: None,
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

    pub(crate) fn with_max_repeated_tool_batches(mut self, limit: usize) -> Self {
        self.max_repeated_tool_batches = limit;
        self
    }

    pub(crate) fn with_max_no_progress_batches(mut self, limit: usize) -> Self {
        self.max_no_progress_batches = limit;
        self
    }

    pub(crate) fn with_max_repeated_category_batches(mut self, limit: usize) -> Self {
        self.max_repeated_category_batches = limit;
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

    pub(crate) fn record_tool_batch(
        &mut self,
        tool_batch_signature: impl Into<String>,
        tool_category_signature: impl Into<String>,
        made_progress: bool,
    ) {
        let tool_batch_signature = tool_batch_signature.into();
        if self
            .last_tool_batch_signature
            .as_ref()
            .is_some_and(|previous| previous == &tool_batch_signature)
        {
            self.repeated_tool_batches += 1;
        } else {
            self.last_tool_batch_signature = Some(tool_batch_signature);
            self.repeated_tool_batches = 1;
        }

        let tool_category_signature = tool_category_signature.into();
        if self
            .last_tool_category_signature
            .as_ref()
            .is_some_and(|previous| previous == &tool_category_signature)
        {
            self.repeated_category_batches += 1;
        } else {
            self.last_tool_category_signature = Some(tool_category_signature);
            self.repeated_category_batches = 1;
        }

        if made_progress {
            self.no_progress_batches = 0;
        } else {
            self.no_progress_batches += 1;
        }
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
        if self.no_progress_batches >= self.max_no_progress_batches {
            return Err(LoopStopReason::RepeatedNoProgress);
        }
        if self.repeated_tool_batches >= self.max_repeated_tool_batches {
            return Err(LoopStopReason::ToolLoopDetected);
        }
        if self.repeated_category_batches >= self.max_repeated_category_batches {
            return Err(LoopStopReason::RepeatedCategoryBatch);
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
        self.repeated_tool_batches = 0;
        self.repeated_category_batches = 0;
        self.no_progress_batches = 0;
        self.last_tool_batch_signature = None;
        self.last_tool_category_signature = None;
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
            LoopStopReason::ToolLoopDetected => "tool_loop_detected",
            LoopStopReason::RepeatedNoProgress => "repeated_no_progress",
            LoopStopReason::RepeatedCategoryBatch => "repeated_category_batch",
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
    fn repeated_tool_batch_stops_loop() {
        let mut guard = LoopGuard::default_limits().with_max_repeated_tool_batches(3);
        guard.record_tool_batch("read_file:{\"path\":\"data.txt\"}", "read_file", true);
        guard.record_tool_batch("read_file:{\"path\":\"data.txt\"}", "read_file", true);
        assert!(guard.check().is_ok());

        guard.record_tool_batch("read_file:{\"path\":\"data.txt\"}", "read_file", true);
        assert_eq!(guard.check(), Err(LoopStopReason::ToolLoopDetected));
    }

    #[test]
    fn repeated_no_progress_batches_stop_loop() {
        let mut guard = LoopGuard::default_limits().with_max_no_progress_batches(3);
        guard.record_tool_batch("read_file:{\"path\":\"missing-1.txt\"}", "read_file", false);
        guard.record_tool_batch("read_file:{\"path\":\"missing-2.txt\"}", "read_file", false);
        assert!(guard.check().is_ok());

        guard.record_tool_batch("read_file:{\"path\":\"missing-3.txt\"}", "read_file", false);
        assert_eq!(guard.check(), Err(LoopStopReason::RepeatedNoProgress));
    }

    #[test]
    fn progress_resets_no_progress_counter() {
        let mut guard = LoopGuard::default_limits().with_max_no_progress_batches(3);
        guard.record_tool_batch("read_file:{\"path\":\"missing-1.txt\"}", "read_file", false);
        guard.record_tool_batch("read_file:{\"path\":\"ok.txt\"}", "read_file", true);
        guard.record_tool_batch("read_file:{\"path\":\"missing-2.txt\"}", "read_file", false);
        guard.record_tool_batch("read_file:{\"path\":\"missing-3.txt\"}", "read_file", false);

        assert!(guard.check().is_ok());
    }

    #[test]
    fn repeated_category_batch_stops_loop_when_similar_tools_repeat() {
        let mut guard = LoopGuard::default_limits().with_max_repeated_category_batches(4);
        // Different exact signatures but same category (all read_file)
        guard.record_tool_batch("read_file:{\"path\":\"a.txt\"}", "read_file", true);
        guard.record_tool_batch("read_file:{\"path\":\"b.txt\"}", "read_file", true);
        guard.record_tool_batch("read_file:{\"path\":\"c.txt\"}", "read_file", true);
        assert!(guard.check().is_ok());

        guard.record_tool_batch("read_file:{\"path\":\"d.txt\"}", "read_file", true);
        assert_eq!(guard.check(), Err(LoopStopReason::RepeatedCategoryBatch));
    }

    #[test]
    fn repeated_category_batch_different_category_resets_counter() {
        let mut guard = LoopGuard::default_limits().with_max_repeated_category_batches(3);
        guard.record_tool_batch("read_file:{\"path\":\"a.txt\"}", "read_file", true);
        guard.record_tool_batch("read_file:{\"path\":\"b.txt\"}", "read_file", true);
        guard.record_tool_batch("run_shell:{\"cmd\":\"test\"}", "run_shell", true);
        guard.record_tool_batch("read_file:{\"path\":\"c.txt\"}", "read_file", true);

        // Counter reset when category changed to run_shell, so read_file count is only 1 now
        assert!(guard.check().is_ok());
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
        assert_eq!(
            LoopStopReason::ToolLoopDetected.as_str(),
            "tool_loop_detected"
        );
        assert_eq!(
            LoopStopReason::RepeatedNoProgress.as_str(),
            "repeated_no_progress"
        );
        assert_eq!(
            LoopStopReason::RepeatedCategoryBatch.as_str(),
            "repeated_category_batch"
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
        guard.record_tool_batch("read_file:{\"path\":\"data.txt\"}", "read_file", false);

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
