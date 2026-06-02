use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::memory::risk::should_reject_persistent_memory;

mod service;
mod store;

pub use service::ContinuityService;
pub use store::ContinuityStore;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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
}

impl ContinuityEvent {
    fn session_id(&self) -> &str {
        match self {
            ContinuityEvent::UserMessage { session_id, .. }
            | ContinuityEvent::AssistantResponse { session_id, .. }
            | ContinuityEvent::ToolExecution { session_id, .. }
            | ContinuityEvent::FileChange { session_id, .. } => session_id,
            ContinuityEvent::Reflection(reflection) => &reflection.session_id,
        }
    }

    fn timestamp_ms(&self) -> u64 {
        match self {
            ContinuityEvent::UserMessage { timestamp_ms, .. }
            | ContinuityEvent::AssistantResponse { timestamp_ms, .. }
            | ContinuityEvent::ToolExecution { timestamp_ms, .. }
            | ContinuityEvent::FileChange { timestamp_ms, .. } => *timestamp_ms,
            ContinuityEvent::Reflection(reflection) => reflection.timestamp_ms,
        }
    }

    fn event_type(&self) -> &'static str {
        match self {
            ContinuityEvent::UserMessage { .. } => "user_message",
            ContinuityEvent::AssistantResponse { .. } => "assistant_response",
            ContinuityEvent::ToolExecution { .. } => "tool_execution",
            ContinuityEvent::FileChange { .. } => "file_change",
            ContinuityEvent::Reflection(_) => "reflection",
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

pub(crate) fn should_reject_experience_lesson(body: &str) -> bool {
    let body = normalize_text(body);
    body.is_empty() || should_reject_persistent_memory(&body) || is_prompt_echo_question(&body)
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
