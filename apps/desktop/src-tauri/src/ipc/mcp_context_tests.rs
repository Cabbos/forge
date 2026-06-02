use crate::harness::mcp::McpResourceContent;
use crate::ipc::mcp_context::{
    format_mcp_resource_context, mcp_context_selection_label, McpContextBuilder,
    McpContextSelection, MCP_CONTEXT_ITEM_CHAR_LIMIT,
};

#[test]
fn mcp_resource_context_formats_source_and_text() {
    let selection = McpContextSelection::Resource {
        server_id: "obsidian".to_string(),
        uri: "file:///notes/forge.md".to_string(),
        name: Some("Forge 研发记录".to_string()),
        description: Some("项目研发记录".to_string()),
        mime_type: Some("text/markdown".to_string()),
    };
    let contents = vec![McpResourceContent {
        uri: "file:///notes/forge.md".to_string(),
        mime_type: Some("text/markdown".to_string()),
        text: Some("下一步先打通 MCP 资料加入本轮上下文。".to_string()),
        blob: None,
    }];

    let context = format_mcp_resource_context(&selection, &contents).expect("context");

    assert!(context.contains("User-selected connector resource"));
    assert!(context.contains("Forge 研发记录"));
    assert!(context.contains("obsidian"));
    assert!(context.contains("下一步先打通 MCP 资料加入本轮上下文。"));
}

#[test]
fn mcp_resource_context_truncates_large_text() {
    let selection = McpContextSelection::Resource {
        server_id: "obsidian".to_string(),
        uri: "file:///notes/large.md".to_string(),
        name: Some("大资料".to_string()),
        description: None,
        mime_type: Some("text/markdown".to_string()),
    };
    let contents = vec![McpResourceContent {
        uri: "file:///notes/large.md".to_string(),
        mime_type: Some("text/markdown".to_string()),
        text: Some("a".repeat(MCP_CONTEXT_ITEM_CHAR_LIMIT + 200)),
        blob: None,
    }];

    let context = format_mcp_resource_context(&selection, &contents).expect("context");

    assert!(context.len() < MCP_CONTEXT_ITEM_CHAR_LIMIT + 800);
    assert!(context.contains("truncated"));
}

#[test]
fn mcp_context_result_tracks_only_ready_connector_labels() {
    let ready = McpContextSelection::Resource {
        server_id: "obsidian".to_string(),
        uri: "file:///notes/forge.md".to_string(),
        name: Some("Forge 研发记录".to_string()),
        description: None,
        mime_type: Some("text/markdown".to_string()),
    };
    let failed = McpContextSelection::Prompt {
        server_id: "obsidian".to_string(),
        name: "broken-prompt".to_string(),
        description: None,
        arguments: None,
    };

    let mut builder = McpContextBuilder::default();
    builder.push_ready(&ready, "ready context".to_string());
    builder.push_error("failed context".to_string());
    let result = builder.finish();

    assert_eq!(result.ready_labels, vec!["obsidian: Forge 研发记录"]);
    let context = result.context.expect("context");
    assert!(context.contains("ready context"));
    assert!(context.contains("failed context"));
    assert!(!result
        .ready_labels
        .contains(&mcp_context_selection_label(&failed)));
}
