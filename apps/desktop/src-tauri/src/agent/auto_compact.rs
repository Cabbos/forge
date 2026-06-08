use crate::adapters::base::ChatMessage;

const DEFAULT_CONTEXT_WINDOW_TOKENS: usize = 128_000;
const DEFAULT_RESERVED_OUTPUT_TOKENS: usize = 20_000;
const AUTO_COMPACT_BUFFER_TOKENS: usize = 13_000;
const MAX_HISTORY_MESSAGES_BEFORE_COMPACT: usize = 80;
const RETAIN_RECENT_MESSAGES: usize = 32;
const OVERFLOW_RETRY_RETAIN_RECENT_MESSAGES: usize = 16;
const MIN_COMPACT_MESSAGES: usize = 8;
const MAX_SUMMARY_CHARS: usize = 14_000;
const MAX_SUMMARY_ITEM_CHARS: usize = 360;
const MAX_MODEL_SUMMARY_ITEM_CHARS: usize = 12_000;
const MAX_CONSECUTIVE_AUTO_COMPACT_MISSES: u8 = 3;

#[derive(Debug, Clone)]
pub(crate) struct CompactResult {
    pub(crate) messages: Vec<ChatMessage>,
    pub(crate) summary: Option<String>,
    pub(crate) stats: Option<CompactStats>,
    pub(crate) attempted: bool,
    pub(crate) skipped_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompactStats {
    pub(crate) summary: String,
    pub(crate) retained_messages: usize,
    pub(crate) compacted_messages: usize,
    pub(crate) estimated_tokens_before: u32,
    pub(crate) estimated_tokens_after: u32,
}

impl CompactResult {
    pub(crate) fn unchanged(messages: Vec<ChatMessage>, summary: Option<String>) -> Self {
        Self {
            messages,
            summary,
            stats: None,
            attempted: false,
            skipped_reason: None,
        }
    }

    pub(crate) fn skipped(
        messages: Vec<ChatMessage>,
        summary: Option<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            messages,
            summary,
            stats: None,
            attempted: true,
            skipped_reason: Some(reason.into()),
        }
    }

    fn compacted(messages: Vec<ChatMessage>, summary: String, stats: CompactStats) -> Self {
        Self {
            messages,
            summary: Some(summary),
            stats: Some(stats),
            attempted: true,
            skipped_reason: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CompactPlan {
    pub(crate) original_messages: Vec<ChatMessage>,
    pub(crate) compacted_messages: Vec<ChatMessage>,
    pub(crate) retained_messages: Vec<ChatMessage>,
    pub(crate) existing_summary: Option<String>,
    pub(crate) estimated_tokens_before: usize,
}

impl CompactPlan {
    pub(crate) fn retained_message_count(&self) -> usize {
        self.retained_messages.len()
    }

    pub(crate) fn compacted_message_count(&self) -> usize {
        self.compacted_messages.len()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AutoCompactGuard {
    consecutive_misses: u8,
}

impl AutoCompactGuard {
    pub(crate) fn record_result(&mut self, result: &CompactResult) {
        if result.stats.is_some() {
            self.consecutive_misses = 0;
            return;
        }

        if result.attempted && result.skipped_reason.is_some() {
            self.consecutive_misses = self.consecutive_misses.saturating_add(1);
        }
    }

    pub(crate) fn should_skip_proactive_compaction(&self) -> bool {
        self.consecutive_misses >= MAX_CONSECUTIVE_AUTO_COMPACT_MISSES
    }

    pub(crate) fn record_proactive_skip(&mut self) {
        self.consecutive_misses = self.consecutive_misses.saturating_sub(1);
    }
}

pub(crate) fn compact_messages_if_needed(
    msgs: Vec<ChatMessage>,
    existing_summary: Option<String>,
    context_window_tokens: Option<u32>,
) -> CompactResult {
    match prepare_compaction_if_needed(msgs, existing_summary, context_window_tokens) {
        Ok(plan) => finalize_compaction_plan_with_heuristic_summary(plan),
        Err(result) => *result,
    }
}

pub(crate) fn compact_messages_for_overflow_retry(
    msgs: Vec<ChatMessage>,
    existing_summary: Option<String>,
) -> CompactResult {
    match prepare_compaction_for_overflow_retry(msgs, existing_summary) {
        Ok(plan) => finalize_compaction_plan_with_heuristic_summary(plan),
        Err(result) => *result,
    }
}

pub(crate) fn compact_messages_now(
    msgs: Vec<ChatMessage>,
    existing_summary: Option<String>,
) -> CompactResult {
    match prepare_compaction_now(msgs, existing_summary) {
        Ok(plan) => finalize_compaction_plan_with_heuristic_summary(plan),
        Err(result) => *result,
    }
}

pub(crate) fn prepare_compaction_if_needed(
    msgs: Vec<ChatMessage>,
    existing_summary: Option<String>,
    context_window_tokens: Option<u32>,
) -> Result<CompactPlan, Box<CompactResult>> {
    let existing_summary_tokens = existing_summary
        .as_deref()
        .map(estimate_text_tokens)
        .unwrap_or(0);
    let estimated_before = estimate_messages_tokens(&msgs) + existing_summary_tokens;
    let context_limit = effective_context_limit(context_window_tokens);
    let compact_threshold = auto_compact_threshold(context_limit);
    let over_budget = estimated_before > compact_threshold;
    let too_many_messages = msgs.len() > MAX_HISTORY_MESSAGES_BEFORE_COMPACT;

    if !over_budget && !too_many_messages {
        return Err(Box::new(CompactResult::unchanged(msgs, existing_summary)));
    }

    prepare_compaction_with_retention(
        msgs,
        existing_summary,
        estimated_before,
        RETAIN_RECENT_MESSAGES,
    )
}

pub(crate) fn prepare_compaction_for_overflow_retry(
    msgs: Vec<ChatMessage>,
    existing_summary: Option<String>,
) -> Result<CompactPlan, Box<CompactResult>> {
    let existing_summary_tokens = existing_summary
        .as_deref()
        .map(estimate_text_tokens)
        .unwrap_or(0);
    let estimated_before = estimate_messages_tokens(&msgs) + existing_summary_tokens;

    prepare_compaction_with_retention(
        msgs,
        existing_summary,
        estimated_before,
        OVERFLOW_RETRY_RETAIN_RECENT_MESSAGES,
    )
}

pub(crate) fn prepare_compaction_now(
    msgs: Vec<ChatMessage>,
    existing_summary: Option<String>,
) -> Result<CompactPlan, Box<CompactResult>> {
    let existing_summary_tokens = existing_summary
        .as_deref()
        .map(estimate_text_tokens)
        .unwrap_or(0);
    let estimated_before = estimate_messages_tokens(&msgs) + existing_summary_tokens;

    prepare_compaction_with_retention(
        msgs,
        existing_summary,
        estimated_before,
        RETAIN_RECENT_MESSAGES,
    )
}

fn prepare_compaction_with_retention(
    msgs: Vec<ChatMessage>,
    existing_summary: Option<String>,
    estimated_before: usize,
    retain_recent_messages: usize,
) -> Result<CompactPlan, Box<CompactResult>> {
    if msgs.len() <= retain_recent_messages || msgs.len() <= MIN_COMPACT_MESSAGES {
        return Err(Box::new(CompactResult::skipped(
            msgs,
            existing_summary,
            "history_too_short",
        )));
    }

    let split_at = msgs.len().saturating_sub(retain_recent_messages);
    let Some(start) = (split_at..msgs.len())
        .find(|&i| is_safe_retention_boundary(&msgs[i]))
        .or_else(|| {
            (0..split_at)
                .rev()
                .find(|&i| is_safe_retention_boundary(&msgs[i]))
        })
    else {
        return Err(Box::new(CompactResult::skipped(
            msgs,
            existing_summary,
            "no_safe_retention_boundary",
        )));
    };

    if start < MIN_COMPACT_MESSAGES {
        return Err(Box::new(CompactResult::skipped(
            msgs,
            existing_summary,
            "too_few_messages_to_compact",
        )));
    }

    let compacted_messages = msgs[..start].to_vec();
    let retained_messages = msgs[start..].to_vec();

    Ok(CompactPlan {
        original_messages: msgs,
        compacted_messages,
        retained_messages,
        existing_summary,
        estimated_tokens_before: estimated_before,
    })
}

pub(crate) fn finalize_compaction_plan_with_heuristic_summary(plan: CompactPlan) -> CompactResult {
    let Some(summary) = build_summary(&plan.compacted_messages) else {
        return CompactResult::skipped(
            plan.original_messages,
            plan.existing_summary,
            "empty_summary",
        );
    };
    finalize_compaction_plan(plan, summary)
}

pub(crate) fn finalize_compaction_plan(plan: CompactPlan, summary_update: String) -> CompactResult {
    if summary_update.trim().is_empty() {
        return CompactResult::skipped(
            plan.original_messages,
            plan.existing_summary,
            "empty_summary",
        );
    }

    let merged_summary = merge_summaries(plan.existing_summary, summary_update);
    let estimated_after =
        estimate_messages_tokens(&plan.retained_messages) + estimate_text_tokens(&merged_summary);
    let stats = CompactStats {
        summary: merged_summary.clone(),
        retained_messages: plan.retained_messages.len(),
        compacted_messages: plan.compacted_messages.len(),
        estimated_tokens_before: to_u32_tokens(plan.estimated_tokens_before),
        estimated_tokens_after: to_u32_tokens(estimated_after),
    };

    CompactResult::compacted(plan.retained_messages, merged_summary, stats)
}

fn build_summary(msgs: &[ChatMessage]) -> Option<String> {
    let mut lines = Vec::new();
    for msg in msgs {
        if let Some(line) = summarize_message(msg) {
            lines.push(format!(
                "- {}",
                truncate_chars(&line, MAX_SUMMARY_ITEM_CHARS)
            ));
        }
        if lines.len() >= 18 {
            break;
        }
    }

    if lines.is_empty() {
        return None;
    }

    let mut summary = String::from("[Earlier conversation summary]\n");
    for line in lines {
        summary.push_str(&line);
        summary.push('\n');
    }
    if msgs.len() > 18 {
        summary.push_str(&format!(
            "- ... {} older messages compacted\n",
            msgs.len() - 18
        ));
    }
    Some(summary.trim_end().to_string())
}

pub(crate) fn render_messages_for_model_summary(msgs: &[ChatMessage], char_limit: usize) -> String {
    if msgs.is_empty() {
        return String::new();
    }

    let mut rendered = String::new();
    for (index, msg) in msgs.iter().enumerate() {
        let content = compact_content(&msg.content, MAX_MODEL_SUMMARY_ITEM_CHARS);
        if content.is_empty() {
            continue;
        }
        rendered.push_str(&format!(
            "<message index=\"{}\" role=\"{}\">\n{}\n</message>\n\n",
            index + 1,
            msg.role,
            content
        ));
    }

    truncate_middle_chars(
        rendered.trim_end(),
        char_limit.max(MAX_SUMMARY_ITEM_CHARS * 2),
    )
}

fn summarize_message(msg: &ChatMessage) -> Option<String> {
    let text = compact_content(&msg.content, MAX_SUMMARY_ITEM_CHARS);
    if text.is_empty() {
        return None;
    }
    let label = if is_tool_result(msg) {
        "Tool result"
    } else {
        match msg.role.as_str() {
            "assistant" => "Assistant",
            "system" => "System",
            "tool" => "Tool result",
            "user" => "User",
            other => other,
        }
    };
    Some(format!("{label}: {text}"))
}

fn merge_summaries(existing: Option<String>, update: String) -> String {
    let update = update.trim();
    let Some(old) = existing.filter(|old| !old.trim().is_empty()) else {
        return truncate_chars(update, MAX_SUMMARY_CHARS);
    };

    let old = old.trim();
    let update_len = update.chars().count();
    if update_len + 1 >= MAX_SUMMARY_CHARS {
        return truncate_chars(update, MAX_SUMMARY_CHARS);
    }

    let old_budget = MAX_SUMMARY_CHARS - update_len - 1;
    format!("{}\n{}", truncate_chars(old, old_budget), update)
}

fn compact_content(value: &serde_json::Value, limit: usize) -> String {
    let raw = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if let Some(part) = compact_content_block(item) {
                    parts.push(part);
                }
            }
            parts.join(" | ")
        }
        serde_json::Value::Object(_) => {
            if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
                text.to_string()
            } else if let Some(content) = value.get("content") {
                compact_content(content, limit)
            } else {
                serde_json::to_string(value).unwrap_or_default()
            }
        }
        other => other.to_string(),
    };
    truncate_chars(&collapse_whitespace(&raw), limit)
}

fn compact_content_block(block: &serde_json::Value) -> Option<String> {
    let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match block_type {
        "text" => block
            .get("text")
            .and_then(|v| v.as_str())
            .map(|text| text.to_string()),
        "tool_use" => {
            let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
            let input = block
                .get("input")
                .map(|v| compact_content(v, 120))
                .unwrap_or_default();
            Some(if input.is_empty() {
                format!("Tool requested: {name}")
            } else {
                format!("Tool requested: {name} ({input})")
            })
        }
        "tool_result" => block.get("content").map(|content| {
            format!(
                "Tool result: {}",
                compact_content(content, MAX_SUMMARY_ITEM_CHARS)
            )
        }),
        "thinking" | "redacted_thinking" => None,
        _ => {
            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                Some(text.to_string())
            } else if let Some(content) = block.get("content") {
                Some(compact_content(content, MAX_SUMMARY_ITEM_CHARS))
            } else {
                Some(serde_json::to_string(block).unwrap_or_default())
            }
        }
    }
}

fn estimate_messages_tokens(msgs: &[ChatMessage]) -> usize {
    msgs.iter().map(estimate_message_tokens).sum()
}

fn estimate_message_tokens(msg: &ChatMessage) -> usize {
    estimate_text_tokens(&msg.role) + estimate_value_tokens(&msg.content) + 8
}

fn estimate_value_tokens(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::String(s) => estimate_text_tokens(s),
        serde_json::Value::Array(items) => {
            items.iter().map(estimate_value_tokens).sum::<usize>() + (items.len() * 4)
        }
        serde_json::Value::Object(map) => {
            map.iter()
                .map(|(key, value)| estimate_text_tokens(key) + estimate_value_tokens(value))
                .sum::<usize>()
                + (map.len() * 4)
        }
        serde_json::Value::Null => 1,
        other => estimate_text_tokens(&other.to_string()),
    }
}

fn estimate_text_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(3)
}

fn effective_context_limit(context_window_tokens: Option<u32>) -> usize {
    context_window_tokens
        .map(|tokens| tokens as usize)
        .unwrap_or(DEFAULT_CONTEXT_WINDOW_TOKENS)
        .max(16_000)
}

fn auto_compact_threshold(context_limit: usize) -> usize {
    let reserved_output = DEFAULT_RESERVED_OUTPUT_TOKENS.min(context_limit / 4);
    let buffer = AUTO_COMPACT_BUFFER_TOKENS.min(context_limit / 10);
    context_limit
        .saturating_sub(reserved_output)
        .saturating_sub(buffer)
        .max(8_000)
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    if limit <= 3 {
        return ".".repeat(limit);
    }
    let mut shortened = text.chars().take(limit - 3).collect::<String>();
    shortened.push_str("...");
    shortened
}

fn truncate_middle_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    if limit <= 32 {
        return truncate_chars(text, limit);
    }

    let marker = "\n\n<omitted_middle_messages_due_to_summary_prompt_budget />\n\n";
    let marker_len = marker.chars().count();
    if marker_len >= limit {
        return truncate_chars(text, limit);
    }

    let remaining = limit - marker_len;
    let head_len = remaining * 3 / 5;
    let tail_len = remaining - head_len;
    let head: String = text.chars().take(head_len).collect();
    let tail: String = text
        .chars()
        .rev()
        .take(tail_len)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{head}{marker}{tail}")
}

fn to_u32_tokens(value: usize) -> u32 {
    value.min(u32::MAX as usize) as u32
}

fn is_tool_result(msg: &ChatMessage) -> bool {
    if let serde_json::Value::Array(ref blocks) = msg.content {
        blocks
            .iter()
            .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_result"))
    } else {
        false
    }
}

fn is_safe_retention_boundary(msg: &ChatMessage) -> bool {
    msg.role == "user" && !is_tool_result(msg)
}

#[cfg(test)]
mod tests {
    use crate::adapters::base::ChatMessage;

    use super::{
        compact_messages_for_overflow_retry, compact_messages_if_needed, compact_messages_now,
        AutoCompactGuard, CompactResult, CompactStats,
    };

    fn numbered_messages(count: usize) -> Vec<ChatMessage> {
        (0..count)
            .map(|index| {
                if index % 2 == 0 {
                    ChatMessage::user(&format!("user message {index}"))
                } else {
                    ChatMessage::assistant(serde_json::Value::String(format!(
                        "assistant message {index}"
                    )))
                }
            })
            .collect()
    }

    #[test]
    fn does_not_compact_small_history() {
        let messages = numbered_messages(12);

        let result = compact_messages_if_needed(messages.clone(), None, None);

        assert!(result.stats.is_none());
        assert_eq!(result.messages.len(), messages.len());
        assert!(result.summary.is_none());
    }

    #[test]
    fn compacts_when_message_count_threshold_is_exceeded_and_retains_recent_messages() {
        let messages = numbered_messages(90);

        let result = compact_messages_if_needed(messages, None, None);

        let stats = result.stats.expect("expected compaction stats");
        assert_eq!(stats.retained_messages, 32);
        assert_eq!(result.messages.len(), 32);
        assert_eq!(stats.compacted_messages, 58);
        assert_eq!(result.messages[0].role, "user");
        assert_eq!(
            result.messages[0].content,
            serde_json::Value::String("user message 58".to_string())
        );
    }

    #[test]
    fn merges_existing_summary_with_new_summary() {
        let messages = numbered_messages(90);

        let result =
            compact_messages_if_needed(messages, Some("previous summary".to_string()), None);

        let summary = result.summary.expect("expected merged summary");
        assert!(summary.starts_with("previous summary\n[Earlier conversation summary]"));
    }

    #[test]
    fn merge_preserves_newest_summary_when_existing_summary_is_long() {
        let messages = numbered_messages(90);
        let existing = "older summary ".repeat(2_000);

        let result = compact_messages_if_needed(messages, Some(existing), None);

        let summary = result.summary.expect("expected merged summary");
        assert!(summary.contains("user message 0"));
    }

    #[test]
    fn summarizes_tool_results_with_tool_result_label() {
        let mut messages = vec![ChatMessage::tool_result("tool-1", "read file output")];
        messages.extend(numbered_messages(90));

        let result = compact_messages_if_needed(messages, None, None);

        let summary = result.summary.expect("expected summary");
        assert!(summary.contains("Tool result: Tool result: read file output"));
    }

    #[test]
    fn omits_thinking_blocks_from_summary() {
        let mut messages = vec![ChatMessage::assistant(serde_json::json!([
            {
                "type": "thinking",
                "thinking": "private reasoning that should not appear"
            },
            {
                "type": "text",
                "text": "visible assistant text"
            }
        ]))];
        messages.extend(numbered_messages(90));

        let result = compact_messages_if_needed(messages, None, None);

        let summary = result.summary.expect("expected summary");
        assert!(summary.contains("visible assistant text"));
        assert!(!summary.contains("private reasoning"));
    }

    #[test]
    fn compacts_when_token_threshold_is_exceeded() {
        let messages = numbered_messages(40)
            .into_iter()
            .map(|mut message| {
                message.content = serde_json::Value::String("x".repeat(4000));
                message
            })
            .collect();

        let result = compact_messages_if_needed(messages, None, Some(16_000));

        assert!(result.stats.is_some());
    }

    #[test]
    fn overflow_retry_compacts_below_proactive_threshold_when_history_is_long_enough() {
        let messages = numbered_messages(40);

        let proactive = compact_messages_if_needed(messages.clone(), None, None);
        assert!(proactive.stats.is_none());

        let result = compact_messages_for_overflow_retry(messages, None);

        let stats = result.stats.expect("expected overflow retry compaction");
        assert_eq!(stats.retained_messages, 16);
        assert_eq!(result.messages.len(), 16);
        assert_eq!(stats.compacted_messages, 24);
        assert_eq!(
            result.messages[0].content,
            serde_json::Value::String("user message 24".to_string())
        );
    }

    #[test]
    fn manual_compact_compresses_history_below_proactive_threshold_when_safe() {
        let messages = numbered_messages(40);

        let proactive = compact_messages_if_needed(messages.clone(), None, None);
        assert!(proactive.stats.is_none());

        let result = compact_messages_now(messages, None);

        let stats = result.stats.expect("expected manual compaction");
        assert_eq!(stats.retained_messages, 32);
        assert_eq!(stats.compacted_messages, 8);
        assert_eq!(result.messages.len(), 32);
        assert_eq!(
            result.messages[0].content,
            serde_json::Value::String("user message 8".to_string())
        );
    }

    #[test]
    fn overflow_retry_does_not_compact_short_history() {
        let messages = numbered_messages(16);

        let result = compact_messages_for_overflow_retry(messages.clone(), None);

        assert!(result.stats.is_none());
        assert_eq!(result.messages.len(), messages.len());
        assert!(result.summary.is_none());
    }

    #[test]
    fn overflow_retry_merges_existing_summary() {
        let messages = numbered_messages(40);

        let result =
            compact_messages_for_overflow_retry(messages, Some("previous summary".to_string()));

        let summary = result.summary.expect("expected merged summary");
        assert!(summary.starts_with("previous summary\n[Earlier conversation summary]"));
    }

    #[test]
    fn overflow_retry_keeps_tool_result_with_its_user_turn_boundary() {
        let mut messages = numbered_messages(20);
        messages.push(ChatMessage::user("current user request"));
        for index in 0..24 {
            messages.push(ChatMessage::assistant(serde_json::json!([{
                "type": "tool_use",
                "id": format!("tool-{index}"),
                "name": "read_file",
                "input": {"path": format!("file-{index}.ts")}
            }])));
            messages.push(ChatMessage::tool_result(
                &format!("tool-{index}"),
                &format!("result {index}"),
            ));
        }

        let result = compact_messages_for_overflow_retry(messages, None);

        assert_eq!(result.messages[0].role, "user");
        assert_eq!(
            result.messages[0].content,
            serde_json::Value::String("current user request".to_string())
        );
    }

    #[test]
    fn reports_attempted_compaction_when_no_safe_boundary_exists() {
        let messages = (0..90)
            .map(|index| {
                ChatMessage::assistant(serde_json::Value::String(format!(
                    "assistant-only message {index}"
                )))
            })
            .collect::<Vec<_>>();

        let result = compact_messages_if_needed(messages.clone(), None, None);

        assert!(result.attempted);
        assert_eq!(
            result.skipped_reason.as_deref(),
            Some("no_safe_retention_boundary")
        );
        assert!(result.stats.is_none());
        assert_eq!(result.messages.len(), messages.len());
    }

    #[test]
    fn auto_compact_guard_pauses_after_consecutive_misses_and_resets_on_success() {
        let miss = CompactResult {
            messages: numbered_messages(90),
            summary: None,
            stats: None,
            attempted: true,
            skipped_reason: Some("no_safe_retention_boundary".to_string()),
        };
        let success = CompactResult {
            messages: numbered_messages(32),
            summary: Some("summary".to_string()),
            stats: Some(CompactStats {
                summary: "summary".to_string(),
                retained_messages: 32,
                compacted_messages: 58,
                estimated_tokens_before: 1000,
                estimated_tokens_after: 250,
            }),
            attempted: true,
            skipped_reason: None,
        };
        let mut guard = AutoCompactGuard::default();

        guard.record_result(&miss);
        guard.record_result(&miss);
        assert!(!guard.should_skip_proactive_compaction());

        guard.record_result(&miss);
        assert!(guard.should_skip_proactive_compaction());

        guard.record_result(&success);
        assert!(!guard.should_skip_proactive_compaction());
    }

    #[test]
    fn auto_compact_guard_cools_down_after_one_skipped_proactive_attempt() {
        let miss = CompactResult {
            messages: numbered_messages(90),
            summary: None,
            stats: None,
            attempted: true,
            skipped_reason: Some("no_safe_retention_boundary".to_string()),
        };
        let mut guard = AutoCompactGuard::default();

        guard.record_result(&miss);
        guard.record_result(&miss);
        guard.record_result(&miss);
        assert!(guard.should_skip_proactive_compaction());

        guard.record_proactive_skip();

        assert!(!guard.should_skip_proactive_compaction());
    }
}
