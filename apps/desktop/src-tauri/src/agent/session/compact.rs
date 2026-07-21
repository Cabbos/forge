use std::sync::Arc;

use tokio::sync::Notify;

use crate::agent::auto_compact::{
    finalize_compaction_plan, finalize_compaction_plan_with_heuristic_summary,
    prepare_compaction_now, CompactPlan, CompactResult, CompactStats,
};
use crate::agent::compact_summary::{
    compact_summary_prompt_messages, extract_compact_summary_text,
};
use crate::agent::event_sink::EventEmitter;
use crate::agent::manual_compact::ManualCompactResult;
use crate::agent::session::AgentSession;
use crate::agent::session_guards::lock_unpoisoned;
use crate::agent::time::now_ms;
use crate::agent::turn_state::AgentCompactTrace;

pub(crate) fn compact_summary_was_cancelled(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("cancelled") || lower.contains("canceled")
}

impl AgentSession {
    pub(crate) async fn compact_plan_with_summary(
        &self,
        plan: &CompactPlan,
        cancel: Arc<Notify>,
        fallback_on_model_error: bool,
    ) -> Result<CompactResult, String> {
        if self.adapter.is_missing_api_key_adapter() {
            return Ok(finalize_compaction_plan_with_heuristic_summary(
                plan.clone(),
            ));
        }

        match self.generate_model_compact_summary(plan, cancel).await {
            Ok(summary) => Ok(finalize_compaction_plan(plan.clone(), summary)),
            Err(err) if compact_summary_was_cancelled(&err) => Err(err),
            Err(err) if fallback_on_model_error => {
                crate::app_log!(
                    "WARN",
                    "Falling back to heuristic compact summary for session {}: {}",
                    self.id,
                    err
                );
                Ok(finalize_compaction_plan_with_heuristic_summary(
                    plan.clone(),
                ))
            }
            Err(err) => Err(err),
        }
    }

    async fn generate_model_compact_summary(
        &self,
        plan: &CompactPlan,
        cancel: Arc<Notify>,
    ) -> Result<String, String> {
        let messages = compact_summary_prompt_messages(plan, self.context_window_tokens);
        let result = self
            .adapter
            .compact_summary(&messages, cancel)
            .await
            .map_err(|err| err.to_string())?;
        extract_compact_summary_text(&result)
    }

    pub(crate) async fn compact_now_with_emitter(
        &self,
        emitter: &dyn EventEmitter,
    ) -> Result<ManualCompactResult, String> {
        emitter.emit(self.context_compact_start_event());
        let all_messages = lock_unpoisoned(&self.messages).clone();
        let existing_summary = lock_unpoisoned(&self.summary).clone();
        let compacted = match prepare_compaction_now(all_messages, existing_summary) {
            Ok(plan) => {
                self.compact_plan_with_summary(&plan, Arc::new(Notify::new()), false)
                    .await?
            }
            Err(result) => *result,
        };

        if let Some(stats) = compacted.stats.as_ref() {
            lock_unpoisoned(&self.auto_compact_guard).record_result(&compacted);
            self.apply_compaction_emitter(&compacted, stats, "manual_compact", emitter);
            return Ok(ManualCompactResult {
                compacted: true,
                skipped_reason: None,
                retained_messages: stats.retained_messages,
                compacted_messages: stats.compacted_messages,
                estimated_tokens_before: stats.estimated_tokens_before,
                estimated_tokens_after: stats.estimated_tokens_after,
            });
        }

        let retained_messages = compacted.messages.len();
        let skipped_reason = compacted.skipped_reason.clone();
        if let Some(reason) = skipped_reason.as_deref() {
            emitter.emit(self.context_compact_skipped_event(reason, retained_messages));
        }

        Ok(ManualCompactResult {
            compacted: false,
            skipped_reason,
            retained_messages,
            compacted_messages: 0,
            estimated_tokens_before: 0,
            estimated_tokens_after: 0,
        })
    }

    pub(crate) fn apply_compaction_emitter(
        &self,
        compacted: &CompactResult,
        stats: &CompactStats,
        reason: &str,
        emitter: &dyn EventEmitter,
    ) {
        let checkpoint_id = format!("compact-{}-{}", reason, uuid::Uuid::now_v7());
        let _ = self.replace_conversation(
            checkpoint_id,
            compacted.messages.clone(),
            compacted.summary.clone(),
            crate::agent::session_mutation::SessionMutationSource::Compaction,
        );
        let saved_tokens = {
            let mut metrics = lock_unpoisoned(&self.turn_metrics);
            metrics.record_compaction(stats.estimated_tokens_before, stats.estimated_tokens_after);
            metrics.snapshot().compact_saved_tokens
        };
        if let Some(turn) = lock_unpoisoned(&self.latest_turn).as_mut() {
            turn.compact_saved_tokens = saved_tokens;
        }
        emitter.emit(self.context_compacted_event(stats));
        self.record_latest_compact_emitter(
            AgentCompactTrace {
                reason: reason.to_string(),
                retained_messages: stats.retained_messages,
                compacted_messages: stats.compacted_messages,
                estimated_tokens_before: Some(stats.estimated_tokens_before),
                estimated_tokens_after: Some(stats.estimated_tokens_after),
                created_at_ms: now_ms(),
            },
            emitter,
        );
    }
}
