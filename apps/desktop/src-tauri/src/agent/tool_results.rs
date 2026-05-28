use crate::adapters::base::{repair_tool_result_adjacency, ChatMessage, ToolCall};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToolResultResolution {
    pub(crate) content: String,
    pub(crate) missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrderedToolResultForModel {
    pub(crate) tool_call_id: String,
    pub(crate) tool_name: String,
    pub(crate) content: String,
    pub(crate) missing: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolResultMessageForModel {
    pub(crate) message: ChatMessage,
    pub(crate) results: Vec<OrderedToolResultForModel>,
}

pub(crate) fn resolve_tool_result_for_model(
    result_map: &std::collections::HashMap<String, String>,
    tool_call: &ToolCall,
) -> ToolResultResolution {
    if let Some(content) = result_map.get(&tool_call.id) {
        return ToolResultResolution {
            content: content.clone(),
            missing: false,
        };
    }

    ToolResultResolution {
        content: format!("Tool result missing: {}", tool_call.name),
        missing: true,
    }
}

pub(crate) fn build_tool_result_message_for_model(
    result_map: &std::collections::HashMap<String, String>,
    tool_calls: &[ToolCall],
) -> ToolResultMessageForModel {
    let results = tool_calls
        .iter()
        .map(|tool_call| {
            let resolution = resolve_tool_result_for_model(result_map, tool_call);
            OrderedToolResultForModel {
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                content: resolution.content,
                missing: resolution.missing,
            }
        })
        .collect::<Vec<_>>();
    let blocks = results
        .iter()
        .map(|result| {
            let mut block = serde_json::json!({
                "type": "tool_result",
                "tool_use_id": result.tool_call_id,
                "content": result.content,
            });
            if result.missing {
                block["is_error"] = serde_json::Value::Bool(true);
            }
            block
        })
        .collect::<Vec<_>>();

    ToolResultMessageForModel {
        message: ChatMessage {
            role: "user".to_string(),
            content: serde_json::Value::Array(blocks),
        },
        results,
    }
}

pub(crate) fn repair_tool_use_adjacency(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    repair_tool_result_adjacency(&messages)
}

pub(crate) fn is_read_only_tool(name: &str) -> bool {
    const READ_ONLY_TOOLS: &[&str] = &[
        "read_file",
        "read",
        "list_directory",
        "ls",
        "list",
        "search_files",
        "glob",
        "search_content",
        "grep",
        "web_search",
        "web_fetch",
        "git_diff",
    ];
    READ_ONLY_TOOLS.contains(&name)
}

pub(crate) fn push_assistant_result_with_synthetic_tool_results(
    messages: &mut Vec<ChatMessage>,
    assistant_content: Vec<serde_json::Value>,
    tool_calls: &[ToolCall],
    reason: &str,
) {
    if assistant_content.is_empty() {
        return;
    }

    messages.push(ChatMessage::assistant(serde_json::Value::Array(
        assistant_content,
    )));
    if tool_calls.is_empty() {
        let pending = messages.last().map(assistant_tool_uses).unwrap_or_default();
        if !pending.is_empty() {
            messages.push(synthetic_tool_result_message_with_reason(&pending, reason));
        }
        return;
    }
    let pending = tool_calls
        .iter()
        .map(|tool_call| PendingToolUse {
            id: tool_call.id.clone(),
            name: tool_call.name.clone(),
        })
        .collect::<Vec<_>>();
    messages.push(synthetic_tool_result_message_with_reason(&pending, reason));
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingToolUse {
    id: String,
    name: String,
}

fn assistant_tool_uses(message: &ChatMessage) -> Vec<PendingToolUse> {
    if message.role != "assistant" {
        return Vec::new();
    }
    message
        .content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(|value| value.as_str()) == Some("tool_use"))
        .filter_map(|block| {
            let id = block
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .trim();
            if id.is_empty() {
                return None;
            }
            Some(PendingToolUse {
                id: id.to_string(),
                name: block
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown_tool")
                    .to_string(),
            })
        })
        .collect()
}

fn synthetic_tool_result_message_with_reason(
    pending_tool_uses: &[PendingToolUse],
    reason: &str,
) -> ChatMessage {
    ChatMessage {
        role: "user".to_string(),
        content: serde_json::Value::Array(synthetic_tool_result_blocks(pending_tool_uses, reason)),
    }
}

fn synthetic_tool_result_blocks(
    pending_tool_uses: &[PendingToolUse],
    reason: &str,
) -> Vec<serde_json::Value> {
    pending_tool_uses
        .iter()
        .map(|tool_use| {
            serde_json::json!({
                "type": "tool_result",
                "tool_use_id": tool_use.id,
                "content": format!(
                    "Tool result unavailable: {reason}. The previous tool call was interrupted before Forge could capture its result. Tool: '{}'. Re-check the current workspace state before continuing.",
                    tool_use.name
                ),
                "is_error": true
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        build_tool_result_message_for_model, push_assistant_result_with_synthetic_tool_results,
        repair_tool_use_adjacency, resolve_tool_result_for_model,
    };
    use crate::adapters::base::{ChatMessage, ToolCall};

    #[test]
    fn tool_result_message_preserves_tool_order_and_marks_missing_results() {
        let tool_calls = vec![
            ToolCall {
                id: "call-a".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path": "src/App.tsx"}),
            },
            ToolCall {
                id: "call-b".to_string(),
                name: "run_shell".to_string(),
                input: serde_json::json!({"command": "npm test"}),
            },
            ToolCall {
                id: "call-c".to_string(),
                name: "git_diff".to_string(),
                input: serde_json::json!({}),
            },
        ];
        let result_map = std::collections::HashMap::from([
            ("call-c".to_string(), "diff output".to_string()),
            ("call-a".to_string(), "file output".to_string()),
        ]);

        let model_results = build_tool_result_message_for_model(&result_map, &tool_calls);

        assert_eq!(model_results.message.role, "user");
        let blocks = model_results
            .message
            .content
            .as_array()
            .expect("tool result blocks");
        let ids = blocks
            .iter()
            .map(|block| {
                block
                    .get("tool_use_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["call-a", "call-b", "call-c"]);
        assert_eq!(
            blocks[0].get("content").and_then(|value| value.as_str()),
            Some("file output")
        );
        assert!(blocks[1]
            .get("content")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .contains("Tool result missing: run_shell"));
        assert_eq!(
            blocks[1].get("is_error").and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            model_results
                .results
                .iter()
                .filter(|result| result.missing)
                .map(|result| result.tool_call_id.as_str())
                .collect::<Vec<_>>(),
            vec!["call-b"]
        );
    }

    #[test]
    fn missing_tool_result_resolution_names_tool_and_marks_missing() {
        let result_map = std::collections::HashMap::new();
        let tool_call = ToolCall {
            id: "tool-1".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({ "path": "src/main.rs" }),
        };

        let resolution = resolve_tool_result_for_model(&result_map, &tool_call);

        assert!(resolution.missing);
        assert_eq!(resolution.content, "Tool result missing: read_file");
    }

    #[test]
    fn repair_tool_use_adjacency_inserts_missing_result_before_follow_up() {
        let messages = vec![
            ChatMessage::user("先检查项目"),
            ChatMessage::assistant(serde_json::json!([{
                "type": "tool_use",
                "id": "call_1",
                "name": "read_file",
                "input": {"path": "src/App.tsx"}
            }])),
            ChatMessage::user("继续"),
        ];

        let repaired = repair_tool_use_adjacency(messages);

        assert_eq!(repaired.len(), 4);
        assert_eq!(repaired[2].role, "user");
        let ids = tool_result_ids(&repaired[2]);
        assert_eq!(ids, vec!["call_1"]);
        assert!(repaired[2]
            .content
            .to_string()
            .contains("previous tool call was interrupted"));
        assert_eq!(
            repaired[3].content,
            serde_json::Value::String("继续".to_string())
        );
    }

    #[test]
    fn repair_tool_use_adjacency_fills_partial_result_message() {
        let messages = vec![
            ChatMessage::assistant(serde_json::json!([
                {
                    "type": "tool_use",
                    "id": "call_1",
                    "name": "read_file",
                    "input": {"path": "src/App.tsx"}
                },
                {
                    "type": "tool_use",
                    "id": "call_2",
                    "name": "read_file",
                    "input": {"path": "src/main.tsx"}
                }
            ])),
            ChatMessage {
                role: "user".to_string(),
                content: serde_json::json!([{
                    "type": "tool_result",
                    "tool_use_id": "call_1",
                    "content": "ok"
                }]),
            },
        ];

        let repaired = repair_tool_use_adjacency(messages);

        assert_eq!(repaired.len(), 2);
        let ids = tool_result_ids(&repaired[1]);
        assert_eq!(ids, vec!["call_1", "call_2"]);
    }

    #[test]
    fn final_summary_tool_calls_are_closed_with_synthetic_results() {
        let mut messages = Vec::new();
        let assistant_content = vec![serde_json::json!({
            "type": "tool_use",
            "id": "call_1",
            "name": "read_file",
            "input": {"path": "src/App.tsx"}
        })];
        let tool_calls = vec![ToolCall {
            id: "call_1".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "src/App.tsx"}),
        }];

        push_assistant_result_with_synthetic_tool_results(
            &mut messages,
            assistant_content,
            &tool_calls,
            "final_summary_tool_call_not_executed",
        );

        assert_eq!(messages.len(), 2);
        assert_eq!(tool_result_ids(&messages[1]), vec!["call_1"]);
        assert!(messages[1]
            .content
            .to_string()
            .contains("final_summary_tool_call_not_executed"));
    }

    #[test]
    fn final_summary_closes_tool_use_even_when_adapter_tool_calls_are_empty() {
        let mut messages = Vec::new();
        let assistant_content = vec![serde_json::json!({
            "type": "tool_use",
            "id": "call_1",
            "name": "read_file",
            "input": {"path": "src/App.tsx"}
        })];

        push_assistant_result_with_synthetic_tool_results(
            &mut messages,
            assistant_content,
            &[],
            "final_summary_tool_call_not_executed",
        );

        assert_eq!(messages.len(), 2);
        assert_eq!(tool_result_ids(&messages[1]), vec!["call_1"]);
    }

    fn tool_result_ids(message: &ChatMessage) -> Vec<&str> {
        message
            .content
            .as_array()
            .into_iter()
            .flatten()
            .filter(|block| {
                block.get("type").and_then(|value| value.as_str()) == Some("tool_result")
            })
            .filter_map(|block| block.get("tool_use_id").and_then(|value| value.as_str()))
            .collect()
    }
}
