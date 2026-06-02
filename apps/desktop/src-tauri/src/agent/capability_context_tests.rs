use super::{
    build_capability_activation_text, build_turn_input_intent, format_turn_capability_snapshot,
    selected_slash_command, ComposerCapabilitySelection, TurnCapabilitySnapshot,
};

#[test]
fn selected_slash_command_canonicalizes_aliases() {
    let command = selected_slash_command(&[ComposerCapabilitySelection::SlashCommand {
        command: "/CR".to_string(),
    }]);

    assert_eq!(command, Some("/code-review"));
}

#[test]
fn turn_input_intent_normalizes_structured_composer_signals() {
    let intent = build_turn_input_intent(
        "  按钮没有反应  ",
        &[
            ComposerCapabilitySelection::SlashCommand {
                command: "/CR".to_string(),
            },
            ComposerCapabilitySelection::FileReference {
                path: " src/App.tsx ".to_string(),
            },
            ComposerCapabilitySelection::FileReference {
                path: "src/App.tsx".to_string(),
            },
            ComposerCapabilitySelection::FileReference {
                path: " ".to_string(),
            },
        ],
        vec![
            "obsidian: Forge 研发记录".to_string(),
            "obsidian: Forge 研发记录".to_string(),
            " ".to_string(),
        ],
    );

    assert_eq!(intent.user_text, "按钮没有反应");
    assert_eq!(intent.slash_command.as_deref(), Some("/code-review"));
    assert_eq!(intent.file_references, vec!["src/App.tsx"]);
    assert_eq!(intent.selected_connectors, vec!["obsidian: Forge 研发记录"]);
    assert!(intent.activation_text.contains("按钮没有反应"));
    assert!(intent.activation_text.contains("Action intent:"));
    assert!(intent.activation_text.contains("检查风险"));
}

#[test]
fn capability_snapshot_context_summarizes_hidden_capability_layer() {
    let context = format_turn_capability_snapshot(&TurnCapabilitySnapshot {
        slash_command: Some("/fix".to_string()),
        file_references: vec!["src/App.tsx".to_string()],
        selected_connectors: vec!["obsidian: Forge 研发记录".to_string()],
        matched_skills: vec!["code-review".to_string()],
        active_hooks: vec!["Workspace Boundary Guard".to_string()],
        enabled_mcp_servers: vec!["obsidian".to_string()],
        available_mcp_tools: vec!["obsidian__search_notes".to_string()],
    })
    .expect("context");

    assert!(context.contains("当前动作：/fix"));
    assert!(context.contains("参考文件：src/App.tsx"));
    assert!(context.contains("连接资料：obsidian: Forge 研发记录"));
    assert!(context.contains("自动启用技能：code-review"));
    assert!(context.contains("安全规则：Workspace Boundary Guard"));
    assert!(context.contains("可用连接：obsidian"));
    assert!(!context.contains("可用连接工具"));
    assert!(!context.contains("obsidian__search_notes"));
}

#[test]
fn capability_activation_text_keeps_slash_out_of_user_message() {
    let activation_text = build_capability_activation_text("按钮没有反应", Some("/fix"));

    assert!(activation_text.contains("按钮没有反应"));
    assert!(activation_text.contains("/fix"));
    assert_ne!(activation_text, "按钮没有反应");
}

#[test]
fn capability_activation_text_stays_clean_for_command_only_turns() {
    let activation_text = build_capability_activation_text("   ", Some("/fix"));

    assert!(!activation_text.starts_with('\n'));
    assert!(activation_text.contains("Selected slash command: /fix"));
    assert!(activation_text.contains("Action intent:"));
}

#[test]
fn slash_command_activation_text_includes_stable_action_intent() {
    let activation_text = build_capability_activation_text("按钮没有反应", Some("/fix"));

    assert!(activation_text.contains("Action intent:"));
    assert!(activation_text.contains("排查并修复"));
}

#[test]
fn capability_snapshot_translates_slash_command_for_hidden_context() {
    let context = format_turn_capability_snapshot(&TurnCapabilitySnapshot {
        slash_command: Some("/code-review".to_string()),
        ..TurnCapabilitySnapshot::default()
    })
    .expect("context");

    assert!(context.contains("当前动作：/code-review"));
    assert!(context.contains("检查风险"));
}

#[test]
fn capability_snapshot_includes_stable_action_intent_instruction() {
    let context = format_turn_capability_snapshot(&TurnCapabilitySnapshot {
        slash_command: Some("/fix".to_string()),
        ..TurnCapabilitySnapshot::default()
    })
    .expect("context");

    assert!(context.contains("动作意图："));
    assert!(context.contains("先定位根因"));
    assert!(context.contains("运行相关验证"));
}
