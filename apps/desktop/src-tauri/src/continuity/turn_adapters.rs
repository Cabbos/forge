use crate::agent::turn_state::{
    AgentToolCategory, AgentToolStatus, AgentToolTrace, AgentTurnState, AgentVerificationStatus,
    AgentVerificationTrace,
};

use super::{should_reject_experience_lesson, ContinuityEvent, FileOperation, ReflectionEvent, ReflectionOutcome};
use crate::memory::model::WikiMemory;

/// Build continuity events from an agent turn.
pub fn continuity_events_from_turn(turn: &AgentTurnState) -> Vec<ContinuityEvent> {
    let mut events = Vec::new();
    for tool in &turn.tools {
        events.push(ContinuityEvent::ToolExecution {
            session_id: turn.session_id.clone(),
            tool_name: tool.name.clone(),
            input_summary: continuity_tool_input_summary(tool),
            output_summary: continuity_tool_output_summary(tool),
            is_error: continuity_tool_is_error(tool),
            timestamp_ms: tool.ended_at_ms.unwrap_or(tool.started_at_ms),
        });

        if continuity_tool_can_change_files(tool) {
            for path in &tool.affected_files {
                events.push(ContinuityEvent::FileChange {
                    session_id: turn.session_id.clone(),
                    path: path.clone(),
                    operation: FileOperation::Modified,
                    diff_summary: continuity_file_change_summary(tool),
                    timestamp_ms: tool.ended_at_ms.unwrap_or(tool.started_at_ms),
                });
            }
        }
    }

    events.push(ContinuityEvent::AssistantResponse {
        session_id: turn.session_id.clone(),
        content_summary: continuity_assistant_response_summary(turn),
        timestamp_ms: turn.updated_at_ms,
    });
    events
}

/// Build actionable lessons from an agent turn for experience formation.
pub fn continuity_lessons_from_turn(turn: &AgentTurnState) -> Vec<String> {
    let goal = normalize_inline_text(&turn.user_goal, 120);
    let mut lessons = Vec::new();

    for tool in turn
        .tools
        .iter()
        .filter(|tool| continuity_tool_failure_is_actionable(tool))
        .take(3)
    {
        lessons.push(format!(
            "Tool `{}` failed during `{}` ({}): {}",
            normalize_inline_text(&tool.name, 80),
            goal,
            continuity_tool_input_summary(tool),
            continuity_tool_output_summary(tool)
        ));
    }

    if matches!(
        turn.verification.status,
        AgentVerificationStatus::Failed | AgentVerificationStatus::Error
    ) {
        if let Some(summary) = continuity_verification_failure_summary(&turn.verification) {
            let command = turn
                .verification
                .command
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("verification");
            lessons.push(format!(
                "Verification `{}` failed during `{}`: {}",
                normalize_inline_text(command, 120),
                goal,
                summary
            ));
        }
    }

    dedupe_lessons(lessons)
}

/// Build a reflection event for a completed user input turn.
pub fn build_send_input_reflection_event(
    session_id: &str,
    user_goal: &str,
    outcome: ReflectionOutcome,
    lessons: Vec<String>,
    timestamp_ms: u64,
) -> ContinuityEvent {
    ContinuityEvent::Reflection(ReflectionEvent {
        session_id: session_id.to_string(),
        user_goal: user_goal.to_string(),
        execution_summary: match outcome {
            ReflectionOutcome::Completed => "send_input completed successfully".to_string(),
            ReflectionOutcome::Failed => "send_input failed before completion".to_string(),
            ReflectionOutcome::Cancelled => "send_input was cancelled".to_string(),
        },
        outcome,
        verification_summary: None,
        lessons,
        timestamp_ms,
    })
}

/// Convert memory candidates into lesson strings for reflection.
pub fn continuity_lessons_from_memory_candidates(candidates: &[WikiMemory]) -> Vec<String> {
    candidates
        .iter()
        .map(|candidate| {
            let title = candidate.title.trim();
            let body = candidate.body.trim();
            if title.is_empty() || body.contains(title) {
                body.to_string()
            } else {
                format!("{title}: {body}")
            }
        })
        .filter(|lesson| !should_reject_experience_lesson(lesson))
        .collect()
}

fn continuity_verification_failure_summary(
    trace: &AgentVerificationTrace,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(exit_code) = trace.exit_code {
        parts.push(format!("exit_code={exit_code}"));
    }
    if let Some(stderr) = trace
        .stderr_preview
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("stderr={}", normalize_inline_text(stderr, 180)));
    } else if let Some(stdout) = trace
        .stdout_preview
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("stdout={}", normalize_inline_text(stdout, 180)));
    }

    (!parts.is_empty()).then(|| parts.join("; "))
}

pub fn dedupe_lessons(lessons: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    lessons
        .into_iter()
        .filter(|lesson| !lesson.trim().is_empty())
        .filter(|lesson| seen.insert(lesson.to_lowercase()))
        .collect()
}

fn continuity_tool_input_summary(tool: &AgentToolTrace) -> String {
    if let Some(command) = tool
        .command
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return format!("command={}", normalize_inline_text(command, 240));
    }
    if !tool.affected_files.is_empty() {
        return format!(
            "files={}",
            tool.affected_files
                .iter()
                .map(|path| normalize_inline_text(path, 120))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    format!("tool_call_id={}", tool.tool_call_id)
}

fn continuity_tool_output_summary(tool: &AgentToolTrace) -> String {
    tool.result_summary
        .as_deref()
        .map(|summary| normalize_inline_text(summary, 320))
        .filter(|summary| !summary.is_empty())
        .unwrap_or_else(|| format!("status={}", continuity_tool_status_label(&tool.status)))
}

fn continuity_file_change_summary(tool: &AgentToolTrace) -> String {
    let output = continuity_tool_output_summary(tool);
    format!("tool={}; {}", tool.name, output)
}

fn continuity_assistant_response_summary(turn: &AgentTurnState) -> String {
    let failed_tools = turn
        .tools
        .iter()
        .filter(|tool| continuity_tool_is_error(tool))
        .count();
    let mut parts = vec![format!(
        "turn_status={}; tools={}; failed_tools={}",
        serde_json::to_value(&turn.status)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| "unknown".to_string()),
        turn.tools.len(),
        failed_tools
    )];
    if let Some(failure) = &turn.failure {
        parts.push(format!(
            "failure={}",
            normalize_inline_text(&failure.message, 240)
        ));
    }
    parts.join("; ")
}

fn continuity_tool_can_change_files(tool: &AgentToolTrace) -> bool {
    !tool.affected_files.is_empty()
        && matches!(
            tool.category,
            AgentToolCategory::Write | AgentToolCategory::Shell
        )
}

fn continuity_tool_is_error(tool: &AgentToolTrace) -> bool {
    tool.is_error
        || matches!(
            tool.status,
            AgentToolStatus::Failed | AgentToolStatus::Cancelled
        )
}

fn continuity_tool_failure_is_actionable(tool: &AgentToolTrace) -> bool {
    if !continuity_tool_is_error(tool) {
        return false;
    }

    if matches!(tool.category, AgentToolCategory::Shell) {
        if let Some(summary) = tool.result_summary.as_deref() {
            return !shell_failure_summary_looks_successful(summary);
        }
    }

    true
}

fn shell_failure_summary_looks_successful(summary: &str) -> bool {
    let lower = summary.to_lowercase();
    let has_error_marker = lower.contains("error:")
        || lower.contains(" failed")
        || lower.contains("failed ")
        || lower.contains("panic")
        || lower.contains("not found")
        || lower.contains("cannot find")
        || lower.contains("no such file");
    let looks_like_vite_build_success = lower.contains("vite ")
        && lower.contains("building")
        && lower.contains("rendering chunks")
        && lower.contains("computing gzip size")
        && lower.contains("✓ built in");

    looks_like_vite_build_success && !has_error_marker
}

fn continuity_tool_status_label(status: &AgentToolStatus) -> &'static str {
    match status {
        AgentToolStatus::Pending => "pending",
        AgentToolStatus::Running => "running",
        AgentToolStatus::Completed => "completed",
        AgentToolStatus::Failed => "failed",
        AgentToolStatus::Cancelled => "cancelled",
    }
}

fn normalize_inline_text(value: &str, limit: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized.chars().take(limit).collect()
}
