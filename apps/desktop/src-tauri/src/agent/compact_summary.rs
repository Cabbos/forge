use crate::adapters::base::{ChatMessage, StreamResult};
use crate::agent::auto_compact::{render_messages_for_model_summary, CompactPlan};

const COMPACT_SUMMARY_SYSTEM_PROMPT: &str = "\
You are Forge's context compaction summarizer. Create a durable hidden summary \
that lets the next coding-agent turn continue without rereading the compacted \
messages. Preserve concrete technical facts, file paths, commands, errors, \
validation evidence, decisions, constraints, and explicit user preferences. \
Do not invent facts. Do not use tools. Return only the final compact summary.";

pub(crate) fn compact_summary_prompt_messages(
    plan: &CompactPlan,
    context_window_tokens: Option<u32>,
) -> Vec<ChatMessage> {
    let transcript_limit = compact_summary_transcript_char_limit(context_window_tokens);
    let transcript = render_messages_for_model_summary(&plan.compacted_messages, transcript_limit);
    let existing_summary = plan
        .existing_summary
        .as_deref()
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
        .unwrap_or("(none)");

    vec![
        ChatMessage::system(COMPACT_SUMMARY_SYSTEM_PROMPT),
        ChatMessage::user(&format!(
            "Forge needs to compact the older part of an agentic coding session.\n\n\
             Existing summary to preserve and update:\n{existing_summary}\n\n\
             Messages being compacted: {}\n\
             Messages retained verbatim after this compact pass: {}\n\n\
             Summarize only the compacted messages below. The retained messages will remain visible to the next model call, so do not duplicate routine recent chatter unless it is needed to bridge context.\n\n\
             Required sections:\n\
             - Primary Request And Intent\n\
             - Important Decisions And Constraints\n\
             - Files, Code, Commands, Data, And APIs Mentioned\n\
             - Errors, Fixes, And Validation Evidence\n\
             - User Preferences Explicitly Stated\n\
             - Pending Tasks And Next Step\n\
             - Bridge To Retained Context\n\n\
             Return plain Markdown inside <summary>...</summary>. Do not call tools.\n\n\
             <transcript>\n{transcript}\n</transcript>",
            plan.compacted_message_count(),
            plan.retained_message_count(),
        )),
    ]
}

pub(crate) fn compact_summary_transcript_char_limit(context_window_tokens: Option<u32>) -> usize {
    let context_limit = context_window_tokens.unwrap_or(128_000) as usize;
    let prompt_token_budget = context_limit.max(16_000).saturating_sub(24_000).max(8_000);
    prompt_token_budget.saturating_mul(3)
}

pub(crate) fn extract_compact_summary_text(result: &StreamResult) -> Result<String, String> {
    if !result.tool_calls.is_empty() {
        return Err("compact summary response unexpectedly contained tool calls".to_string());
    }

    let raw = assistant_text(&result.assistant_content);
    let extracted = extract_tag_contents(&raw, "summary")
        .unwrap_or_else(|| remove_tagged_blocks(&raw, "analysis"));
    let cleaned = remove_tagged_blocks(&extracted, "analysis")
        .trim()
        .to_string();

    if cleaned.is_empty() {
        return Err("compact summary response was empty".to_string());
    }

    Ok(cleaned)
}

fn assistant_text(assistant_content: &[serde_json::Value]) -> String {
    assistant_content
        .iter()
        .filter_map(|block| {
            if let Some(text) = block.as_str() {
                return Some(text);
            }
            (block.get("type").and_then(|value| value.as_str()) == Some("text"))
                .then(|| block.get("text").and_then(|value| value.as_str()))
                .flatten()
        })
        .collect::<Vec<_>>()
        .join("")
}

fn extract_tag_contents(text: &str, tag: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = lower.find(&open)? + open.len();
    let end = lower[start..].find(&close)? + start;
    Some(text[start..end].to_string())
}

fn remove_tagged_blocks(text: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut remaining = text.to_string();

    loop {
        let lower = remaining.to_ascii_lowercase();
        let Some(start) = lower.find(&open) else {
            break;
        };
        let content_start = start + open.len();
        let Some(close_offset) = lower[content_start..].find(&close) else {
            break;
        };
        let end = content_start + close_offset + close.len();
        remaining.replace_range(start..end, "");
    }

    remaining
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compact_plan_for_prompt() -> CompactPlan {
        CompactPlan {
            original_messages: vec![],
            compacted_messages: vec![
                ChatMessage::user("older user context"),
                ChatMessage::assistant(serde_json::Value::String(
                    "older assistant context".to_string(),
                )),
            ],
            retained_messages: vec![ChatMessage::user("recent retained context")],
            existing_summary: Some("prior durable summary".to_string()),
            estimated_tokens_before: 123,
        }
    }

    #[test]
    fn compact_summary_prompt_preserves_existing_summary_counts_and_transcript() {
        let prompt = compact_summary_prompt_messages(&compact_plan_for_prompt(), Some(32_000));

        assert_eq!(prompt.len(), 2);
        assert_eq!(prompt[0].role, "system");
        assert_eq!(prompt[1].role, "user");

        let user_prompt = prompt[1].content.as_str().expect("user prompt text");
        assert!(user_prompt.contains("prior durable summary"));
        assert!(user_prompt.contains("Messages being compacted: 2"));
        assert!(user_prompt.contains("Messages retained verbatim after this compact pass: 1"));
        assert!(user_prompt.contains("older user context"));
        assert!(user_prompt.contains("older assistant context"));
    }

    #[test]
    fn provider_conformance_compact_summary_prompt_is_tool_free_for_all_transport_families() {
        for provider in [
            "deepseek",
            "anthropic",
            "kimi",
            "glm",
            "minimax",
            "openai",
            "openrouter",
            "alibaba",
            "gemini",
            "xai",
            "groq",
            "mistral",
            "ollama",
            "custom_openai",
            "custom_anthropic",
        ] {
            let prompt = compact_summary_prompt_messages(&compact_plan_for_prompt(), Some(128_000));
            let system_prompt = prompt[0].content.as_str().expect("system prompt");
            let user_prompt = prompt[1].content.as_str().expect("user prompt");

            assert!(
                system_prompt.contains("Do not use tools"),
                "{provider} compact summary system prompt must forbid tools"
            );
            assert!(
                user_prompt.contains("Do not call tools"),
                "{provider} compact summary user prompt must forbid tools"
            );
        }
    }

    #[test]
    fn extract_compact_summary_text_prefers_summary_tag_and_strips_analysis() {
        let result = StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text",
                "text": "<analysis>scratch details</analysis>\n<summary>\nFinal summary\n</summary>",
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        };

        let summary = extract_compact_summary_text(&result).expect("summary");

        assert_eq!(summary, "Final summary");
        assert!(!summary.contains("scratch details"));
        assert!(!summary.contains("<analysis>"));
    }

    #[test]
    fn extract_compact_summary_text_removes_analysis_when_summary_tag_absent() {
        let result = StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text",
                "text": "<analysis>scratch details</analysis>\nFallback summary",
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        };

        let summary = extract_compact_summary_text(&result).expect("summary");

        assert_eq!(summary, "Fallback summary");
    }

    #[test]
    fn extract_compact_summary_text_removes_analysis_inside_summary_tag() {
        let result = StreamResult {
            assistant_content: vec![serde_json::json!({
                "type": "text",
                "text": "<summary>Keep this\n<analysis>scratch details</analysis>\nAnd this</summary>",
            })],
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
        };

        let summary = extract_compact_summary_text(&result).expect("summary");

        assert_eq!(summary, "Keep this\n\nAnd this");
    }

    #[test]
    fn provider_conformance_compact_summary_rejects_tool_calls_for_all_transport_families() {
        for provider in [
            "deepseek",
            "anthropic",
            "kimi",
            "glm",
            "minimax",
            "openai",
            "openrouter",
            "alibaba",
            "gemini",
            "xai",
            "groq",
            "mistral",
            "ollama",
            "custom_openai",
            "custom_anthropic",
        ] {
            let result = StreamResult {
                assistant_content: vec![serde_json::json!({
                    "type": "text",
                    "text": "<summary>Should not matter</summary>",
                })],
                tool_calls: vec![crate::adapters::base::ToolCall {
                    id: format!("{provider}-tool"),
                    name: "read_file".to_string(),
                    input: serde_json::json!({ "path": "src/lib.rs" }),
                }],
                stop_reason: Some("tool_use".to_string()),
            };

            let error = extract_compact_summary_text(&result).expect_err("tool call rejection");

            assert!(
                error.contains("tool calls"),
                "{provider} compact summary must reject tool calls"
            );
        }
    }
}
