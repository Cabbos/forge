#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ComposerCapabilitySelection {
    SlashCommand { command: String },
    FileReference { path: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TurnInputIntent {
    pub(crate) user_text: String,
    pub(crate) slash_command: Option<String>,
    pub(crate) file_references: Vec<String>,
    pub(crate) selected_connectors: Vec<String>,
    pub(crate) activation_text: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TurnCapabilitySnapshot {
    pub(crate) slash_command: Option<String>,
    pub(crate) file_references: Vec<String>,
    pub(crate) selected_connectors: Vec<String>,
    pub(crate) matched_skills: Vec<String>,
    pub(crate) active_hooks: Vec<String>,
    pub(crate) enabled_mcp_servers: Vec<String>,
    pub(crate) available_mcp_tools: Vec<String>,
}

pub(crate) fn build_turn_input_intent(
    text: &str,
    capabilities: &[ComposerCapabilitySelection],
    selected_connectors: Vec<String>,
) -> TurnInputIntent {
    let user_text = text.trim().to_string();
    let slash_command = selected_slash_command(capabilities).map(str::to_string);
    let file_references = selected_file_reference_paths(capabilities);
    let selected_connectors = unique_non_empty_items(&selected_connectors);
    let activation_text = build_capability_activation_text(&user_text, slash_command.as_deref());

    TurnInputIntent {
        user_text,
        slash_command,
        file_references,
        selected_connectors,
        activation_text,
    }
}

#[derive(Debug, Clone, Copy)]
struct SlashCommandIntent {
    label: &'static str,
    instruction: &'static str,
}

pub(crate) fn selected_slash_command(
    capabilities: &[ComposerCapabilitySelection],
) -> Option<&'static str> {
    capabilities.iter().find_map(|capability| match capability {
        ComposerCapabilitySelection::SlashCommand { command } => canonical_slash_command(command),
        ComposerCapabilitySelection::FileReference { .. } => None,
    })
}

pub(crate) fn selected_file_reference_paths(
    capabilities: &[ComposerCapabilitySelection],
) -> Vec<String> {
    let paths = capabilities
        .iter()
        .filter_map(|capability| match capability {
            ComposerCapabilitySelection::FileReference { path } => Some(path.clone()),
            ComposerCapabilitySelection::SlashCommand { .. } => None,
        })
        .collect::<Vec<_>>();
    unique_non_empty_items(&paths)
}

pub(crate) fn build_capability_activation_text(text: &str, slash_command: Option<&str>) -> String {
    let mut parts = Vec::new();
    let text = text.trim();
    if !text.is_empty() {
        parts.push(text.to_string());
    }
    if let Some(command) = slash_command
        .and_then(canonical_slash_command)
        .filter(|command| !command.is_empty())
    {
        parts.push(format!("Selected slash command: {command}"));
        if let Some(intent) = slash_command_intent(command) {
            parts.push(format!("Action intent: {}", intent.instruction));
        }
    }
    parts.join("\n\n")
}

pub(crate) fn format_turn_capability_snapshot(snapshot: &TurnCapabilitySnapshot) -> Option<String> {
    let mut lines = Vec::new();
    if let Some(command) = snapshot
        .slash_command
        .as_deref()
        .and_then(canonical_slash_command)
    {
        let intent = slash_command_intent(command)
            .map(|intent| format!("（{}）", intent.label))
            .unwrap_or_default();
        lines.push(format!("当前动作：{command}{intent}"));
        if let Some(intent) = slash_command_intent(command) {
            lines.push(format!("动作意图：{}", intent.instruction));
        }
    }
    push_snapshot_line(&mut lines, "参考文件", &snapshot.file_references);
    push_snapshot_line(&mut lines, "连接资料", &snapshot.selected_connectors);
    push_snapshot_line(&mut lines, "自动启用技能", &snapshot.matched_skills);
    push_snapshot_line(&mut lines, "安全规则", &snapshot.active_hooks);
    push_snapshot_line(&mut lines, "可用连接", &snapshot.enabled_mcp_servers);

    if lines.is_empty() {
        return None;
    }

    Some(format!(
        "本轮 Forge 已整理出以下隐形能力上下文。不要向用户复述这份清单，直接据此工作。\n\n{}",
        lines.join("\n")
    ))
}

fn canonical_slash_command(command: &str) -> Option<&'static str> {
    match command.trim().to_lowercase().as_str() {
        "/cr" | "/code-review" => Some("/code-review"),
        "/fix" => Some("/fix"),
        "/explain" => Some("/explain"),
        "/refactor" => Some("/refactor"),
        "/test" => Some("/test"),
        "/docs" => Some("/docs"),
        _ => None,
    }
}

fn slash_command_intent(command: &str) -> Option<SlashCommandIntent> {
    match command.trim() {
        "/code-review" => Some(SlashCommandIntent {
            label: "检查风险",
            instruction: "检查风险、回归点和缺失验证；优先给出高信号发现，除非用户明确要求，否则不要直接改代码。",
        }),
        "/fix" => Some(SlashCommandIntent {
            label: "排查并修复",
            instruction: "排查并修复用户描述的问题；先定位根因，再做小范围改动，并在可行时运行相关验证。",
        }),
        "/explain" => Some(SlashCommandIntent {
            label: "解释清楚",
            instruction: "用用户容易理解的语言解释代码、错误或方案；除非用户明确要求，否则不要直接改代码。",
        }),
        "/refactor" => Some(SlashCommandIntent {
            label: "整理结构",
            instruction: "在保持行为不变的前提下整理代码结构；改动要集中，并补充或运行能证明行为未变的检查。",
        }),
        "/test" => Some(SlashCommandIntent {
            label: "运行检查",
            instruction: "选择并运行与当前任务最相关的检查；清楚报告失败原因，必要时提出或执行最小修复。",
        }),
        "/docs" => Some(SlashCommandIntent {
            label: "整理文档",
            instruction: "补充或整理和当前任务直接相关的说明文档；保持文档准确、简洁，并避免扩大范围。",
        }),
        _ => None,
    }
}

fn push_snapshot_line(lines: &mut Vec<String>, label: &str, items: &[String]) {
    let items = unique_non_empty_items(items);
    if items.is_empty() {
        return;
    }
    let visible = items.iter().take(8).cloned().collect::<Vec<_>>();
    let suffix = if items.len() > visible.len() {
        format!("，另有 {} 项", items.len() - visible.len())
    } else {
        String::new()
    };
    lines.push(format!("{label}：{}{}", visible.join("、"), suffix));
}

fn unique_non_empty_items(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        let item = item.trim();
        if !item.is_empty() && !result.iter().any(|existing| existing == item) {
            result.push(item.to_string());
        }
    }
    result
}

#[cfg(test)]
#[path = "capability_context_tests.rs"]
mod tests;
