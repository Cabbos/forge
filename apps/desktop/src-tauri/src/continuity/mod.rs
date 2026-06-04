use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::memory::risk::should_reject_persistent_memory;

mod compiler;
mod episode;
mod filters;
mod formatters;
mod service;
mod store;
mod turn_adapters;

pub use compiler::ExperienceCompiler;
pub use episode::{build_episode_from_turn, Episode, FileChangeRecord, ToolFailureRecord};
pub use service::ContinuityService;
pub use store::ContinuityStore;
pub use turn_adapters::*;

// Re-export types that appear in Episode's public API so consumers don't need
// to depend on the private agent module.
pub use crate::agent::turn_state::AgentVerificationStatus;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum ContinuityEvent {
    UserMessage {
        session_id: String,
        content: String,
        timestamp_ms: u64,
    },
    AssistantResponse {
        session_id: String,
        content_summary: String,
        timestamp_ms: u64,
    },
    ToolExecution {
        session_id: String,
        tool_name: String,
        input_summary: String,
        output_summary: String,
        is_error: bool,
        timestamp_ms: u64,
    },
    FileChange {
        session_id: String,
        path: String,
        operation: FileOperation,
        diff_summary: String,
        timestamp_ms: u64,
    },
    Reflection(ReflectionEvent),
    ExperienceStatusChanged {
        experience_id: String,
        old_status: ExperienceStatus,
        new_status: ExperienceStatus,
        session_id: String,
        project_path: Option<String>,
        timestamp_ms: u64,
    },
}

impl ContinuityEvent {
    fn session_id(&self) -> &str {
        match self {
            ContinuityEvent::UserMessage { session_id, .. }
            | ContinuityEvent::AssistantResponse { session_id, .. }
            | ContinuityEvent::ToolExecution { session_id, .. }
            | ContinuityEvent::FileChange { session_id, .. } => session_id,
            ContinuityEvent::Reflection(reflection) => &reflection.session_id,
            ContinuityEvent::ExperienceStatusChanged { session_id, .. } => session_id,
        }
    }

    fn timestamp_ms(&self) -> u64 {
        match self {
            ContinuityEvent::UserMessage { timestamp_ms, .. }
            | ContinuityEvent::AssistantResponse { timestamp_ms, .. }
            | ContinuityEvent::ToolExecution { timestamp_ms, .. }
            | ContinuityEvent::FileChange { timestamp_ms, .. } => *timestamp_ms,
            ContinuityEvent::Reflection(reflection) => reflection.timestamp_ms,
            ContinuityEvent::ExperienceStatusChanged { timestamp_ms, .. } => *timestamp_ms,
        }
    }

    fn event_type(&self) -> &'static str {
        match self {
            ContinuityEvent::UserMessage { .. } => "user_message",
            ContinuityEvent::AssistantResponse { .. } => "assistant_response",
            ContinuityEvent::ToolExecution { .. } => "tool_execution",
            ContinuityEvent::FileChange { .. } => "file_change",
            ContinuityEvent::Reflection(_) => "reflection",
            ContinuityEvent::ExperienceStatusChanged { .. } => "experience_status_changed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileOperation {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReflectionOutcome {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReflectionEvent {
    pub session_id: String,
    pub user_goal: String,
    pub execution_summary: String,
    pub outcome: ReflectionOutcome,
    pub verification_summary: Option<String>,
    pub lessons: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode: Option<Episode>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExperienceKind {
    Lesson,
    BugPattern,
    Workflow,
    Decision,
    Preference,
    ProjectFact,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExperienceStatus {
    Candidate,
    Accepted,
    Pinned,
    Forgotten,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExperienceMemory {
    pub id: String,
    pub kind: ExperienceKind,
    pub status: ExperienceStatus,
    pub title: String,
    pub body: String,
    pub project_path: Option<String>,
    pub source_session_id: Option<String>,
    pub confidence: f32,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub tags: Vec<String>,
}

pub fn form_experiences_from_reflection(
    reflection: &ReflectionEvent,
    project_path: Option<&str>,
    now_ms: u64,
) -> Vec<ExperienceMemory> {
    // V0.4: episode-level compiler takes precedence when rich turn metadata is available.
    if let Some(ref episode) = reflection.episode {
        return ExperienceCompiler::compile(episode, project_path, now_ms);
    }

    // Fallback: legacy lesson-string formation for older reflections without episode data.
    let mut seen = HashSet::new();
    let mut experiences = Vec::new();

    for (lesson_index, lesson) in reflection.lessons.iter().enumerate() {
        let body = normalize_text(lesson);
        if should_reject_experience_lesson(&body) {
            continue;
        }

        let dedupe_key = body.to_lowercase();
        if !seen.insert(dedupe_key) {
            continue;
        }

        experiences.push(ExperienceMemory {
            id: format!(
                "experience:{}:{}:{}:{}",
                project_id_component(project_path),
                reflection.session_id,
                reflection.timestamp_ms,
                lesson_index
            ),
            kind: ExperienceKind::Lesson,
            status: ExperienceStatus::Candidate,
            title: title_from_body(&body),
            body,
            project_path: project_path.map(str::to_string),
            source_session_id: Some(reflection.session_id.clone()),
            confidence: confidence_for_outcome(&reflection.outcome),
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            tags: Vec::new(),
        });
    }

    experiences
}

pub fn form_continuity_experience_context(experiences: &[ExperienceMemory]) -> Option<String> {
    let lines = experiences
        .iter()
        .filter(|experience| {
            matches!(
                experience.status,
                ExperienceStatus::Accepted | ExperienceStatus::Pinned
            )
        })
        .take(5)
        .map(|experience| {
            format!(
                "- [{}] {}",
                experience_status_label(&experience.status),
                truncate_chars(&normalize_text(&experience.body), 220)
            )
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        return None;
    }

    Some(format!("Continuity Experience:\n{}", lines.join("\n")))
}

pub(crate) fn should_reject_experience_lesson(body: &str) -> bool {
    let body = normalize_text(body);
    body.is_empty()
        || should_reject_persistent_memory(&body)
        || is_prompt_echo_question(&body)
        || is_low_value_continuation(&body)
        || looks_like_raw_user_prompt(&body)
        || is_too_short_for_lesson(&body)
}

fn is_too_short_for_lesson(body: &str) -> bool {
    body.chars().count() < 15
}

fn looks_like_raw_user_prompt(body: &str) -> bool {
    let lower = body.to_lowercase();
    // Reject lessons that look like they are the user's original prompt rather than a learning
    let prompt_markers = [
        "我们现在在 ",
        "请检查",
        "请先做",
        "请优先",
        "帮我实现",
        "帮我做",
        "帮我写",
        "目标不是",
        "做一次 ",
        "人工测试",
        "人工验证",
        "验证本地",
        "完整闭环",
        "观察候选",
        "i want to",
        "please implement",
        "please build",
    ];
    prompt_markers
        .iter()
        .any(|marker| lower.contains(&marker.to_lowercase()))
}

fn is_low_value_continuation(body: &str) -> bool {
    let lower = body.to_lowercase();
    let low_value_phrases = [
        "继续",
        "就行",
        "可以的",
        "好的",
        "ok",
        "往下走",
        "然后呢",
        "下一步",
        "接下来",
    ];
    // If the entire lesson is just one of these phrases (or very close), reject it
    let words: Vec<_> = body.split_whitespace().collect();
    if words.len() <= 2 {
        return low_value_phrases
            .iter()
            .any(|phrase| lower.contains(*phrase));
    }
    false
}

fn is_prompt_echo_question(body: &str) -> bool {
    let parts = split_label_parts(body);
    if parts.len() < 3 {
        return false;
    }

    let tail = parts.last().map(|part| part.trim()).unwrap_or_default();
    if tail.is_empty() || !is_question_like(tail) {
        return false;
    }

    let prior = parts[..parts.len() - 1].join(" ");
    prior.contains(tail)
}

fn split_label_parts(value: &str) -> Vec<&str> {
    value
        .split([':', '：'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn is_question_like(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.ends_with('?')
        || trimmed.ends_with('？')
        || trimmed.ends_with('呢')
        || trimmed.ends_with('吗')
        || trimmed.contains("什么")
        || trimmed.contains("如何")
        || trimmed.contains("怎么")
        || trimmed.contains("为什么")
        || trimmed.contains("是否")
        || trimmed.contains("能不能")
        || trimmed.contains("可以继续")
}

fn confidence_for_outcome(outcome: &ReflectionOutcome) -> f32 {
    match outcome {
        ReflectionOutcome::Completed => 0.74,
        ReflectionOutcome::Failed => 0.62,
        ReflectionOutcome::Cancelled => 0.55,
    }
}

fn experience_status_label(status: &ExperienceStatus) -> &'static str {
    match status {
        ExperienceStatus::Candidate => "candidate",
        ExperienceStatus::Accepted => "accepted",
        ExperienceStatus::Pinned => "pinned",
        ExperienceStatus::Forgotten => "forgotten",
        ExperienceStatus::Archived => "archived",
    }
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn title_from_body(body: &str) -> String {
    const MAX_TITLE_CHARS: usize = 80;
    let sentence = body
        .split(['.', '。', '!', '！', '?', '？'])
        .next()
        .unwrap_or(body)
        .trim();
    let title_source = if sentence.is_empty() { body } else { sentence };
    truncate_chars(title_source, MAX_TITLE_CHARS)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn project_id_component(project_path: Option<&str>) -> String {
    let Some(project_path) = project_path else {
        return "global".to_string();
    };
    format!("project-{:016x}", stable_hash(project_path.as_bytes()))
}

fn stable_hash(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
