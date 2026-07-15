use serde::{Deserialize, Serialize};

use crate::agent::capability_context::TurnInputIntent;
use crate::agent::context_builder::{
    estimate_context_block_tokens, estimate_text_tokens_u32, ContextSourceKind, HiddenContextPart,
};
use crate::forge_wiki::model::SelectedForgeWikiPage;
use crate::harness::permissions::PermissionMode;
use crate::memory::{RecallPlan, SelectedContextMemory};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreparedTurnMemoryAudit {
    pub memory_id: String,
    pub source: String,
    pub source_id: String,
    pub kind: String,
    pub score: f32,
    pub reason: String,
    pub project_match: bool,
    pub profile_match: bool,
    pub injected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreparedTurnContextSource {
    pub kind: String,
    pub label: String,
    pub reason: String,
    pub estimated_tokens: u32,
    pub injected: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextUsageBucketKind {
    VisibleInput,
    HiddenSystem,
    Memory,
    Files,
    ProjectRecords,
    CompactedTranscript,
    ConnectorContext,
    ReservedOutput,
}

impl ContextUsageBucketKind {
    fn label(self) -> &'static str {
        match self {
            ContextUsageBucketKind::VisibleInput => "可见输入",
            ContextUsageBucketKind::HiddenSystem => "隐藏系统",
            ContextUsageBucketKind::Memory => "记忆",
            ContextUsageBucketKind::Files => "文件",
            ContextUsageBucketKind::ProjectRecords => "项目记录",
            ContextUsageBucketKind::CompactedTranscript => "压缩转录",
            ContextUsageBucketKind::ConnectorContext => "连接资料",
            ContextUsageBucketKind::ReservedOutput => "预留输出",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextUsageBucket {
    pub kind: ContextUsageBucketKind,
    pub label: String,
    pub estimated_tokens: u32,
    pub source_count: u32,
    pub injected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextUsageEstimate {
    pub used_tokens: u32,
    pub context_window_tokens: Option<u32>,
    pub percent_used: Option<u32>,
    pub reserved_output_tokens: u32,
    pub sources: Vec<PreparedTurnContextSource>,
    pub buckets: Vec<ContextUsageBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreparedTurn {
    pub session_id: String,
    pub project_path: String,
    pub user_text: String,
    pub activation_text: String,
    pub selected_memory_ids: Vec<String>,
    #[serde(default)]
    pub selected_memory_audit: Vec<PreparedTurnMemoryAudit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_recall_plan: Option<RecallPlan>,
    pub selected_project_record_ids: Vec<String>,
    pub workflow_route: String,
    pub workflow_phase: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slash_command: Option<String>,
    pub permission_mode: PermissionMode,
    pub context_estimate: ContextUsageEstimate,
}

pub(crate) struct PreparedTurnBuildRequest<'a> {
    pub(crate) session_id: &'a str,
    pub(crate) project_path: &'a str,
    pub(crate) user_text: &'a str,
    pub(crate) activation_text: &'a str,
    pub(crate) input_intent: &'a TurnInputIntent,
    pub(crate) workflow_route: String,
    pub(crate) workflow_phase: String,
    pub(crate) hidden_contexts: &'a [HiddenContextPart],
    pub(crate) selected_memories: &'a [SelectedContextMemory],
    pub(crate) selected_memory_audit: &'a [PreparedTurnMemoryAudit],
    pub(crate) memory_recall_plan: Option<&'a RecallPlan>,
    pub(crate) selected_project_records: &'a [SelectedForgeWikiPage],
    pub(crate) permission_mode: PermissionMode,
    pub(crate) context_window_tokens: Option<u32>,
}

pub(crate) fn build_prepared_turn(request: PreparedTurnBuildRequest<'_>) -> PreparedTurn {
    let context_estimate = estimate_prepared_turn_context(
        request.user_text,
        request.hidden_contexts,
        request.context_window_tokens,
    );

    PreparedTurn {
        session_id: request.session_id.to_string(),
        project_path: request.project_path.to_string(),
        user_text: request.user_text.to_string(),
        activation_text: request.activation_text.to_string(),
        selected_memory_ids: unique_non_empty(
            request
                .selected_memories
                .iter()
                .map(|memory| memory.memory_id.as_str()),
        ),
        selected_memory_audit: unique_memory_audit(request.selected_memory_audit),
        memory_recall_plan: request.memory_recall_plan.cloned(),
        selected_project_record_ids: unique_non_empty(
            request
                .selected_project_records
                .iter()
                .map(|record| record.page_id.as_str()),
        ),
        workflow_route: request.workflow_route,
        workflow_phase: request.workflow_phase,
        slash_command: request.input_intent.slash_command.clone(),
        permission_mode: request.permission_mode,
        context_estimate,
    }
}

fn estimate_prepared_turn_context(
    user_text: &str,
    hidden_contexts: &[HiddenContextPart],
    context_window_tokens: Option<u32>,
) -> ContextUsageEstimate {
    let mut sources = Vec::new();
    let user_text = user_text.trim();
    if !user_text.is_empty() {
        sources.push(PreparedTurnContextSource {
            kind: "user_input".to_string(),
            label: "用户输入".to_string(),
            reason: "用户提交的本轮可见输入".to_string(),
            estimated_tokens: estimate_text_tokens_u32(user_text),
            injected: true,
        });
    }

    for context in hidden_contexts {
        let content = context.content.trim();
        if content.is_empty() {
            continue;
        }
        sources.push(PreparedTurnContextSource {
            kind: context.kind.as_str().to_string(),
            label: context.label.clone(),
            reason: context.reason.clone(),
            estimated_tokens: estimate_context_block_tokens(&context.label, content),
            injected: true,
        });
    }

    let used_tokens = sources
        .iter()
        .map(|source| source.estimated_tokens)
        .sum::<u32>();
    let percent_used = context_window_tokens.and_then(|window| {
        (window > 0).then(|| {
            let percent = (used_tokens as f64 / window as f64 * 100.0).round() as u32;
            percent.min(100)
        })
    });

    let reserved_output_tokens = reserved_output_tokens(context_window_tokens);
    let buckets = context_usage_buckets(&sources, reserved_output_tokens);

    ContextUsageEstimate {
        used_tokens,
        context_window_tokens,
        percent_used,
        reserved_output_tokens,
        sources,
        buckets,
    }
}

fn reserved_output_tokens(context_window_tokens: Option<u32>) -> u32 {
    context_window_tokens
        .map(|tokens| (tokens / 4).min(20_000))
        .unwrap_or(0)
}

fn context_usage_buckets(
    sources: &[PreparedTurnContextSource],
    reserved_output_tokens: u32,
) -> Vec<ContextUsageBucket> {
    let mut buckets = Vec::new();
    for source in sources {
        push_context_usage_bucket(
            &mut buckets,
            context_usage_bucket_kind_for_source(source.kind.as_str()),
            source.estimated_tokens,
            source.injected,
            1,
        );
    }
    push_context_usage_bucket(
        &mut buckets,
        ContextUsageBucketKind::ReservedOutput,
        reserved_output_tokens,
        false,
        0,
    );
    buckets
}

fn push_context_usage_bucket(
    buckets: &mut Vec<ContextUsageBucket>,
    kind: ContextUsageBucketKind,
    estimated_tokens: u32,
    injected: bool,
    source_count: u32,
) {
    if let Some(bucket) = buckets.iter_mut().find(|bucket| bucket.kind == kind) {
        bucket.estimated_tokens = bucket.estimated_tokens.saturating_add(estimated_tokens);
        bucket.source_count = bucket.source_count.saturating_add(source_count);
        bucket.injected |= injected;
        return;
    }
    buckets.push(ContextUsageBucket {
        kind,
        label: kind.label().to_string(),
        estimated_tokens,
        source_count,
        injected,
    });
}

fn context_usage_bucket_kind_for_source(kind: &str) -> ContextUsageBucketKind {
    match kind {
        "user_input" => ContextUsageBucketKind::VisibleInput,
        source_kind
            if source_kind == ContextSourceKind::MemoryContext.as_str()
                || source_kind == ContextSourceKind::ContinuityExperience.as_str() =>
        {
            ContextUsageBucketKind::Memory
        }
        source_kind if source_kind == ContextSourceKind::SelectedFiles.as_str() => {
            ContextUsageBucketKind::Files
        }
        source_kind if source_kind == ContextSourceKind::ProjectRecords.as_str() => {
            ContextUsageBucketKind::ProjectRecords
        }
        source_kind
            if source_kind == ContextSourceKind::PreviousSummary.as_str()
                || source_kind == ContextSourceKind::History.as_str() =>
        {
            ContextUsageBucketKind::CompactedTranscript
        }
        source_kind if source_kind == ContextSourceKind::ConnectorContext.as_str() => {
            ContextUsageBucketKind::ConnectorContext
        }
        _ => ContextUsageBucketKind::HiddenSystem,
    }
}

fn unique_non_empty<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() || unique.iter().any(|existing| existing == trimmed) {
            continue;
        }
        unique.push(trimmed.to_string());
    }
    unique
}

fn unique_memory_audit(values: &[PreparedTurnMemoryAudit]) -> Vec<PreparedTurnMemoryAudit> {
    let mut unique = Vec::new();
    for value in values {
        let memory_id = value.memory_id.trim();
        if memory_id.is_empty()
            || unique
                .iter()
                .any(|existing: &PreparedTurnMemoryAudit| existing.memory_id == memory_id)
        {
            continue;
        }
        let mut next = value.clone();
        next.memory_id = memory_id.to_string();
        unique.push(next);
    }
    unique
}
