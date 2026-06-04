use parking_lot::Mutex;
use std::sync::Arc;

use crate::agent::event_sink::EventEmitter;
use crate::protocol::events::StreamEvent;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TurnUsageSnapshot {
    pub(crate) estimated_context_tokens_before_model_call: Option<u32>,
    pub(crate) provider_input_tokens: Option<u32>,
    pub(crate) provider_output_tokens: Option<u32>,
    pub(crate) compact_count: usize,
    pub(crate) compact_saved_tokens: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TurnMetrics {
    current: TurnUsageSnapshot,
}

impl TurnMetrics {
    pub(crate) fn begin_turn(&mut self) {
        self.current = TurnUsageSnapshot::default();
    }

    pub(crate) fn record_context_before_model_call(&mut self, estimated_tokens: Option<u32>) {
        self.current.estimated_context_tokens_before_model_call = estimated_tokens;
    }

    pub(crate) fn record_provider_usage(&mut self, input_tokens: u32, output_tokens: u32) {
        self.current.provider_input_tokens = Some(
            self.current
                .provider_input_tokens
                .unwrap_or_default()
                .saturating_add(input_tokens),
        );
        self.current.provider_output_tokens = Some(
            self.current
                .provider_output_tokens
                .unwrap_or_default()
                .saturating_add(output_tokens),
        );
    }

    pub(crate) fn record_compaction(
        &mut self,
        estimated_tokens_before: u32,
        estimated_tokens_after: u32,
    ) {
        self.current.compact_count += 1;
        self.current.compact_saved_tokens = self
            .current
            .compact_saved_tokens
            .saturating_add(estimated_tokens_before.saturating_sub(estimated_tokens_after));
    }

    pub(crate) fn snapshot(&self) -> TurnUsageSnapshot {
        self.current.clone()
    }
}

pub(crate) struct TurnMetricsEventEmitter<'a> {
    inner: &'a dyn EventEmitter,
    metrics: Arc<Mutex<TurnMetrics>>,
}

impl<'a> TurnMetricsEventEmitter<'a> {
    pub(crate) fn new(inner: &'a dyn EventEmitter, metrics: Arc<Mutex<TurnMetrics>>) -> Self {
        Self { inner, metrics }
    }
}

impl EventEmitter for TurnMetricsEventEmitter<'_> {
    fn emit(&self, event: StreamEvent) {
        if let StreamEvent::Usage {
            input_tokens,
            output_tokens,
            ..
        } = &event
        {
            self.metrics
                .lock()
                .record_provider_usage(*input_tokens, *output_tokens);
        }
        self.inner.emit(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_accumulates_context_usage_and_compaction_for_one_turn() {
        let mut metrics = TurnMetrics::default();

        metrics.begin_turn();
        metrics.record_context_before_model_call(Some(42_000));
        metrics.record_provider_usage(1_200, 340);
        metrics.record_compaction(80_000, 47_500);
        metrics.record_provider_usage(800, 160);

        let snapshot = metrics.snapshot();
        assert_eq!(
            snapshot.estimated_context_tokens_before_model_call,
            Some(42_000)
        );
        assert_eq!(snapshot.provider_input_tokens, Some(2_000));
        assert_eq!(snapshot.provider_output_tokens, Some(500));
        assert_eq!(snapshot.compact_count, 1);
        assert_eq!(snapshot.compact_saved_tokens, 32_500);
    }

    #[test]
    fn begin_turn_resets_previous_turn_metrics() {
        let mut metrics = TurnMetrics::default();

        metrics.begin_turn();
        metrics.record_context_before_model_call(Some(10));
        metrics.record_provider_usage(20, 30);
        metrics.record_compaction(100, 60);

        metrics.begin_turn();

        assert_eq!(metrics.snapshot(), TurnUsageSnapshot::default());
    }
}
