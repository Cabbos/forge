use crate::adapters::base::ChatMessage;
use crate::agent::turn_state::{AgentTurnContextSnapshot, AgentTurnContextSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContextSourceKind {
    SystemPrompt,
    PreviousSummary,
    SelectedFiles,
    MemoryContext,
    ContinuityExperience,
    ProjectRecords,
    ConnectorContext,
    CapabilitySnapshot,
    RecoveryTrace,
    History,
}

impl ContextSourceKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ContextSourceKind::SystemPrompt => "system_prompt",
            ContextSourceKind::PreviousSummary => "previous_summary",
            ContextSourceKind::SelectedFiles => "selected_files",
            ContextSourceKind::MemoryContext => "memory_context",
            ContextSourceKind::ContinuityExperience => "continuity_experience",
            ContextSourceKind::ProjectRecords => "project_records",
            ContextSourceKind::ConnectorContext => "connector_context",
            ContextSourceKind::CapabilitySnapshot => "capability_snapshot",
            ContextSourceKind::RecoveryTrace => "recovery_trace",
            ContextSourceKind::History => "history",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextSource {
    pub(crate) kind: ContextSourceKind,
    pub(crate) label: String,
    pub(crate) reason: String,
    pub(crate) estimated_tokens: Option<u32>,
    pub(crate) injected: bool,
}

impl ContextSource {
    fn new(
        kind: ContextSourceKind,
        label: impl Into<String>,
        reason: impl Into<String>,
        estimated_tokens: Option<u32>,
        injected: bool,
    ) -> Self {
        Self {
            kind,
            label: label.into(),
            reason: reason.into(),
            estimated_tokens,
            injected,
        }
    }

    pub(crate) fn to_turn_context_source(&self) -> AgentTurnContextSource {
        AgentTurnContextSource {
            kind: self.kind.as_str().to_string(),
            label: self.label.clone(),
            reason: self.reason.clone(),
            estimated_tokens: self.estimated_tokens,
            injected: self.injected,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HiddenContextPart {
    pub(crate) kind: ContextSourceKind,
    pub(crate) label: String,
    pub(crate) reason: String,
    pub(crate) content: String,
}

impl HiddenContextPart {
    pub(crate) fn new(
        kind: ContextSourceKind,
        label: impl Into<String>,
        reason: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            label: label.into(),
            reason: reason.into(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ContextBundle {
    pub(crate) messages: Vec<ChatMessage>,
    pub(crate) sources: Vec<ContextSource>,
    pub(crate) estimated_tokens: Option<u32>,
    pub(crate) budget_tokens: Option<u32>,
    pub(crate) omitted_sources: Vec<ContextSource>,
}

impl ContextBundle {
    pub(crate) fn to_turn_context_snapshot(&self) -> AgentTurnContextSnapshot {
        AgentTurnContextSnapshot {
            sources: self
                .sources
                .iter()
                .map(ContextSource::to_turn_context_source)
                .collect(),
            estimated_tokens: self.estimated_tokens,
            budget_tokens: self.budget_tokens,
            omitted_sources: self
                .omitted_sources
                .iter()
                .map(ContextSource::to_turn_context_source)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ContextBuilder {
    messages: Vec<ChatMessage>,
    summary: Option<String>,
    memory_context: Option<String>,
    hidden_contexts: Vec<HiddenContextPart>,
    system_prompt: String,
    context_window_tokens: Option<u32>,
}

impl ContextBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn messages(mut self, messages: Vec<ChatMessage>) -> Self {
        self.messages = messages;
        self
    }

    pub(crate) fn summary(mut self, summary: Option<String>) -> Self {
        self.summary = summary.filter(|summary| !summary.trim().is_empty());
        self
    }

    pub(crate) fn memory_context(mut self, memory_context: Option<String>) -> Self {
        self.memory_context = memory_context.filter(|context| !context.trim().is_empty());
        self
    }

    pub(crate) fn hidden_context(mut self, hidden_context: HiddenContextPart) -> Self {
        if !hidden_context.content.trim().is_empty() {
            self.hidden_contexts.push(hidden_context);
        }
        self
    }

    pub(crate) fn hidden_contexts(
        mut self,
        hidden_contexts: impl IntoIterator<Item = HiddenContextPart>,
    ) -> Self {
        self.hidden_contexts.extend(
            hidden_contexts
                .into_iter()
                .filter(|context| !context.content.trim().is_empty()),
        );
        self
    }

    pub(crate) fn system_prompt(mut self, system_prompt: impl Into<String>) -> Self {
        self.system_prompt = system_prompt.into();
        self
    }

    pub(crate) fn context_window_tokens(mut self, context_window_tokens: Option<u32>) -> Self {
        self.context_window_tokens = context_window_tokens;
        self
    }

    pub(crate) fn build(self) -> ContextBundle {
        let mut messages = Vec::new();
        let mut sources = Vec::new();

        let system_prompt = self.system_prompt.trim();
        if !system_prompt.is_empty() {
            messages.push(ChatMessage::system(system_prompt));
            sources.push(ContextSource::new(
                ContextSourceKind::SystemPrompt,
                "基础规则",
                "本轮请求的基础工作规则",
                Some(estimate_text_tokens_u32(system_prompt)),
                true,
            ));
        }

        if let Some(summary) = self.summary {
            let content = format!("## 历史摘要\n{}", summary.trim());
            messages.push(ChatMessage::user(&content));
            sources.push(ContextSource::new(
                ContextSourceKind::PreviousSummary,
                "历史摘要",
                "自动整理过的早期对话信息",
                Some(estimate_text_tokens_u32(&content)),
                true,
            ));
        }

        if let Some(memory_context) = self.memory_context {
            let content = memory_context.trim();
            messages.push(ChatMessage::user(content));
            sources.push(ContextSource::new(
                ContextSourceKind::MemoryContext,
                "已保存背景",
                "本轮自动带入的用户和项目背景",
                Some(estimate_text_tokens_u32(content)),
                true,
            ));
        }

        let hidden_blocks = self
            .hidden_contexts
            .into_iter()
            .filter_map(|part| {
                let content = part.content.trim();
                if content.is_empty() {
                    return None;
                }
                let block = format_hidden_context_block(&part.label, content);
                sources.push(ContextSource::new(
                    part.kind,
                    part.label,
                    part.reason,
                    Some(estimate_text_tokens_u32(&block)),
                    true,
                ));
                Some(block)
            })
            .collect::<Vec<_>>();
        if !hidden_blocks.is_empty() {
            messages.push(ChatMessage::user(&hidden_blocks.join("\n\n---\n\n")));
        }

        let history_tokens = estimate_messages_tokens_u32(&self.messages);
        let has_history = !self.messages.is_empty();
        messages.extend(self.messages);
        if has_history {
            sources.push(ContextSource::new(
                ContextSourceKind::History,
                "对话记录",
                "本轮保留的可见对话记录",
                Some(history_tokens),
                true,
            ));
        }

        ContextBundle {
            estimated_tokens: Some(estimate_messages_tokens_u32(&messages)),
            messages,
            sources,
            budget_tokens: self.context_window_tokens,
            omitted_sources: Vec::new(),
        }
    }
}

fn estimate_messages_tokens_u32(messages: &[ChatMessage]) -> u32 {
    to_u32_tokens(messages.iter().map(estimate_message_tokens).sum())
}

fn estimate_message_tokens(message: &ChatMessage) -> usize {
    estimate_text_tokens(&message.role) + estimate_value_tokens(&message.content) + 8
}

fn estimate_value_tokens(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::String(text) => estimate_text_tokens(text),
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

pub(crate) fn estimate_context_block_tokens(label: &str, content: &str) -> u32 {
    estimate_text_tokens_u32(&format_hidden_context_block(label, content))
}

pub(crate) fn estimate_text_tokens_u32(text: &str) -> u32 {
    to_u32_tokens(estimate_text_tokens(text))
}

fn format_hidden_context_block(label: &str, content: &str) -> String {
    format!("## {}\n\n{}", label.trim(), content.trim())
}

fn estimate_text_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(3)
}

fn to_u32_tokens(value: usize) -> u32 {
    value.min(u32::MAX as usize) as u32
}

#[cfg(test)]
mod tests {
    use crate::adapters::base::ChatMessage;

    use super::{ContextBuilder, ContextSourceKind, HiddenContextPart};

    fn history() -> Vec<ChatMessage> {
        vec![ChatMessage::user("hello")]
    }

    fn text(message: &ChatMessage) -> &str {
        message
            .content
            .as_str()
            .expect("expected string message content")
    }

    #[test]
    fn empty_system_prompt_is_not_inserted() {
        let bundle = ContextBuilder::new()
            .messages(history())
            .system_prompt("  ")
            .build();

        assert_eq!(bundle.messages.len(), 1);
        assert_eq!(bundle.messages[0].role, "user");
        assert_eq!(text(&bundle.messages[0]), "hello");
        assert!(!bundle
            .sources
            .iter()
            .any(|source| source.kind == ContextSourceKind::SystemPrompt));
    }

    #[test]
    fn system_prompt_is_inserted_first() {
        let bundle = ContextBuilder::new()
            .messages(history())
            .system_prompt("system rules")
            .build();

        assert_eq!(bundle.messages[0].role, "system");
        assert_eq!(text(&bundle.messages[0]), "system rules");
        assert_eq!(bundle.messages[1].role, "user");
        assert_eq!(text(&bundle.messages[1]), "hello");
    }

    #[test]
    fn summary_precedes_memory_context() {
        let bundle = ContextBuilder::new()
            .messages(history())
            .summary(Some("earlier work".to_string()))
            .memory_context(Some("wiki notes".to_string()))
            .build();

        assert_eq!(bundle.messages.len(), 3);
        assert_eq!(text(&bundle.messages[0]), "## 历史摘要\nearlier work");
        assert_eq!(text(&bundle.messages[1]), "wiki notes");
        assert_eq!(text(&bundle.messages[2]), "hello");
    }

    #[test]
    fn hidden_context_parts_keep_distinct_source_kinds() {
        let bundle = ContextBuilder::new()
            .messages(history())
            .hidden_context(HiddenContextPart::new(
                ContextSourceKind::SelectedFiles,
                "Selected files",
                "Files selected by the user for this turn",
                "file body",
            ))
            .hidden_context(HiddenContextPart::new(
                ContextSourceKind::MemoryContext,
                "Saved background",
                "Relevant saved user/project background",
                "memory body",
            ))
            .hidden_context(HiddenContextPart::new(
                ContextSourceKind::ProjectRecords,
                "Project records",
                "Relevant project notes selected for this turn",
                "record body",
            ))
            .hidden_context(HiddenContextPart::new(
                ContextSourceKind::ConnectorContext,
                "Connector context",
                "Connector material selected by the user",
                "connector body",
            ))
            .build();

        let injected_context_messages = bundle
            .messages
            .iter()
            .filter(|message| {
                message.role == "user"
                    && text(message).contains("file body")
                    && text(message).contains("connector body")
            })
            .count();
        let source_kinds = bundle
            .sources
            .iter()
            .map(|source| source.kind.as_str())
            .collect::<Vec<_>>();

        assert_eq!(injected_context_messages, 1);
        assert!(source_kinds.contains(&"selected_files"));
        assert!(source_kinds.contains(&"memory_context"));
        assert!(source_kinds.contains(&"project_records"));
        assert!(source_kinds.contains(&"connector_context"));
    }

    #[test]
    fn without_summary_or_memory_only_history_remains() {
        let original = history();
        let bundle = ContextBuilder::new().messages(original.clone()).build();

        assert_eq!(bundle.messages.len(), original.len());
        assert_eq!(text(&bundle.messages[0]), text(&original[0]));
    }

    #[test]
    fn bundle_generates_agent_turn_context_snapshot() {
        let bundle = ContextBuilder::new()
            .messages(history())
            .system_prompt("system rules")
            .summary(Some("earlier work".to_string()))
            .memory_context(Some("wiki notes".to_string()))
            .context_window_tokens(Some(12_000))
            .build();

        let snapshot = bundle.to_turn_context_snapshot();

        assert_eq!(snapshot.sources.len(), bundle.sources.len());
        assert_eq!(snapshot.budget_tokens, Some(12_000));
        assert!(snapshot.omitted_sources.is_empty());
        assert!(snapshot
            .sources
            .iter()
            .any(|source| source.kind == "system_prompt" && source.injected));
    }

    #[test]
    fn context_source_labels_use_product_language() {
        let bundle = ContextBuilder::new()
            .messages(history())
            .system_prompt("system rules")
            .summary(Some("earlier work".to_string()))
            .memory_context(Some("saved facts".to_string()))
            .build();

        let labels = bundle
            .sources
            .iter()
            .map(|source| source.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            labels,
            vec!["基础规则", "历史摘要", "已保存背景", "对话记录"]
        );
        assert!(bundle.sources.iter().all(|source| {
            !source.label.contains("Memory")
                && !source.label.contains("wiki")
                && !source.label.contains("System")
                && !source.label.contains("Conversation")
        }));
    }

    #[test]
    fn token_estimation_is_non_empty() {
        let bundle = ContextBuilder::new()
            .messages(history())
            .system_prompt("system rules")
            .summary(Some("earlier work".to_string()))
            .memory_context(Some("wiki notes".to_string()))
            .build();

        assert!(bundle.estimated_tokens.is_some_and(|tokens| tokens > 0));
        assert!(bundle
            .sources
            .iter()
            .all(|source| source.estimated_tokens.is_some_and(|tokens| tokens > 0)));
    }
}
