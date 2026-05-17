use crate::adapters::base::ChatMessage;

const DEFAULT_CONTEXT_WINDOW_TOKENS: usize = 128_000;
const AUTO_COMPACT_THRESHOLD_NUMERATOR: usize = 7;
const AUTO_COMPACT_THRESHOLD_DENOMINATOR: usize = 10;
const MAX_HISTORY_MESSAGES_BEFORE_COMPACT: usize = 80;
const RETAIN_RECENT_MESSAGES: usize = 32;
const OVERFLOW_RETRY_RETAIN_RECENT_MESSAGES: usize = 16;
const MIN_COMPACT_MESSAGES: usize = 8;
const MAX_SUMMARY_CHARS: usize = 14_000;
const MAX_SUMMARY_ITEM_CHARS: usize = 360;

#[derive(Debug, Clone)]
pub(crate) struct CompactResult {
    pub(crate) messages: Vec<ChatMessage>,
    pub(crate) summary: Option<String>,
    pub(crate) stats: Option<CompactStats>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompactStats {
    pub(crate) summary: String,
    pub(crate) retained_messages: usize,
    pub(crate) compacted_messages: usize,
    pub(crate) estimated_tokens_before: u32,
    pub(crate) estimated_tokens_after: u32,
}

pub(crate) fn compact_messages_if_needed(
    msgs: Vec<ChatMessage>,
    existing_summary: Option<String>,
    context_window_tokens: Option<u32>,
) -> CompactResult {
    let existing_summary_tokens = existing_summary
        .as_deref()
        .map(estimate_text_tokens)
        .unwrap_or(0);
    let estimated_before = estimate_messages_tokens(&msgs) + existing_summary_tokens;
    let context_limit = context_window_tokens
        .map(|tokens| tokens as usize)
        .unwrap_or(DEFAULT_CONTEXT_WINDOW_TOKENS)
        .max(16_000);
    let compact_threshold = (context_limit * AUTO_COMPACT_THRESHOLD_NUMERATOR
        / AUTO_COMPACT_THRESHOLD_DENOMINATOR)
        .max(8_000);
    let over_budget = estimated_before > compact_threshold;
    let too_many_messages = msgs.len() > MAX_HISTORY_MESSAGES_BEFORE_COMPACT;

    if !over_budget && !too_many_messages {
        return CompactResult {
            messages: msgs,
            summary: existing_summary,
            stats: None,
        };
    }

    compact_messages_with_retention(
        msgs,
        existing_summary,
        estimated_before,
        RETAIN_RECENT_MESSAGES,
    )
}

pub(crate) fn compact_messages_for_overflow_retry(
    msgs: Vec<ChatMessage>,
    existing_summary: Option<String>,
) -> CompactResult {
    let existing_summary_tokens = existing_summary
        .as_deref()
        .map(estimate_text_tokens)
        .unwrap_or(0);
    let estimated_before = estimate_messages_tokens(&msgs) + existing_summary_tokens;

    compact_messages_with_retention(
        msgs,
        existing_summary,
        estimated_before,
        OVERFLOW_RETRY_RETAIN_RECENT_MESSAGES,
    )
}

fn compact_messages_with_retention(
    msgs: Vec<ChatMessage>,
    existing_summary: Option<String>,
    estimated_before: usize,
    retain_recent_messages: usize,
) -> CompactResult {
    if msgs.len() <= retain_recent_messages || msgs.len() <= MIN_COMPACT_MESSAGES {
        return CompactResult {
            messages: msgs,
            summary: existing_summary,
            stats: None,
        };
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
        return CompactResult {
            messages: msgs,
            summary: existing_summary,
            stats: None,
        };
    };

    if start < MIN_COMPACT_MESSAGES {
        return CompactResult {
            messages: msgs,
            summary: existing_summary,
            stats: None,
        };
    }

    let compacted_messages = msgs[..start].to_vec();
    let retained_messages = msgs[start..].to_vec();
    let new_summary = match build_summary(&compacted_messages) {
        Some(summary) => summary,
        None => {
            return CompactResult {
                messages: msgs,
                summary: existing_summary,
                stats: None,
            }
        }
    };
    let merged_summary = merge_summaries(existing_summary, new_summary);
    let estimated_after =
        estimate_messages_tokens(&retained_messages) + estimate_text_tokens(&merged_summary);
    let stats = CompactStats {
        summary: merged_summary.clone(),
        retained_messages: retained_messages.len(),
        compacted_messages: compacted_messages.len(),
        estimated_tokens_before: to_u32_tokens(estimated_before),
        estimated_tokens_after: to_u32_tokens(estimated_after),
    };

    CompactResult {
        messages: retained_messages,
        summary: Some(merged_summary),
        stats: Some(stats),
    }
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

    use super::{compact_messages_for_overflow_retry, compact_messages_if_needed};

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
}
