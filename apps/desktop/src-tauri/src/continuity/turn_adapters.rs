use crate::agent::turn_state::{
    AgentToolCategory, AgentToolStatus, AgentToolTrace, AgentTurnState, AgentVerificationStatus,
    AgentVerificationTrace,
};

use super::{
    filters::shell_failure_is_false_positive, ContinuityEvent, Episode, FileOperation,
    ReflectionEvent, ReflectionOutcome,
};

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
    let mut lessons = Vec::new();

    for tool in turn
        .tools
        .iter()
        .filter(|tool| continuity_tool_failure_is_actionable(tool))
        .take(3)
    {
        lessons.push(format!(
            "Tool `{}` failed: {} -> {}",
            normalize_inline_text(&tool.name, 80),
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
                "Verification `{}` failed: {}",
                normalize_inline_text(command, 120),
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
    episode: Option<Episode>,
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
        episode,
        timestamp_ms,
    })
}

fn continuity_verification_failure_summary(trace: &AgentVerificationTrace) -> Option<String> {
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
    if matches!(tool.category, AgentToolCategory::Shell)
        && shell_failure_is_false_positive(tool.command.as_deref(), tool.result_summary.as_deref())
    {
        return false;
    }

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

    !shell_failure_is_false_positive(tool.command.as_deref(), tool.result_summary.as_deref())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::turn_state::{
        AgentTurnContextSnapshot, AgentTurnInputIntent, AgentTurnState, AgentTurnStatus,
    };

    #[test]
    fn shell_false_failure_with_exit_zero_does_not_form_lesson() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/repo".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Run tests".to_string(),
        );
        turn.record_tool(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "run_shell".to_string(),
            category: AgentToolCategory::Shell,
            status: AgentToolStatus::Failed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some(
                "Exit code: -1 Stdout: > test-app@0.1.0 test > npx tsx src/test.ts normalizeInput tests ✅ passed ✅ passed EXIT: 0 Stderr:"
                    .to_string(),
            ),
            is_error: true,
            affected_files: Vec::new(),
            command: Some("npm test".to_string()),
        });
        turn.mark_status(AgentTurnStatus::Completed);

        let lessons = continuity_lessons_from_turn(&turn);
        assert!(
            lessons.is_empty(),
            "shell with EXIT: 0 should not form a failure lesson"
        );
    }

    #[test]
    fn shell_false_failure_with_exit_zero_records_non_error_event() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/repo".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Run tsc".to_string(),
        );
        turn.record_tool(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "run_shell".to_string(),
            category: AgentToolCategory::Shell,
            status: AgentToolStatus::Failed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some("Exit code: -1 Stdout: EXIT CODE: 0 Stderr:".to_string()),
            is_error: true,
            affected_files: Vec::new(),
            command: Some("npx tsc --noEmit 2>&1; echo \"EXIT CODE: $?\"".to_string()),
        });
        turn.mark_status(AgentTurnStatus::Completed);

        let events = continuity_events_from_turn(&turn);
        let tool_event = events
            .iter()
            .find_map(|event| match event {
                ContinuityEvent::ToolExecution { is_error, .. } => Some(is_error),
                _ => None,
            })
            .expect("tool execution event");
        let assistant_summary = events
            .iter()
            .find_map(|event| match event {
                ContinuityEvent::AssistantResponse {
                    content_summary, ..
                } => Some(content_summary),
                _ => None,
            })
            .expect("assistant summary");

        assert!(
            !tool_event,
            "EXIT CODE: 0 shell output should not record as an error"
        );
        assert!(assistant_summary.contains("failed_tools=0"));
    }

    #[test]
    fn silent_tsc_false_failure_without_summary_records_non_error_event() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/repo".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Run tsc".to_string(),
        );
        turn.record_tool(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "run_shell".to_string(),
            category: AgentToolCategory::Shell,
            status: AgentToolStatus::Failed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: None,
            is_error: true,
            affected_files: Vec::new(),
            command: Some("npx tsc --noEmit".to_string()),
        });
        turn.mark_status(AgentTurnStatus::Completed);

        let events = continuity_events_from_turn(&turn);
        let tool_event = events
            .iter()
            .find_map(|event| match event {
                ContinuityEvent::ToolExecution { is_error, .. } => Some(is_error),
                _ => None,
            })
            .expect("tool execution event");
        let assistant_summary = events
            .iter()
            .find_map(|event| match event {
                ContinuityEvent::AssistantResponse {
                    content_summary, ..
                } => Some(content_summary),
                _ => None,
            })
            .expect("assistant summary");

        assert!(
            !tool_event,
            "silent tsc wrapper failure should not record as an error"
        );
        assert!(assistant_summary.contains("failed_tools=0"));
    }

    #[test]
    fn shell_false_failure_for_successful_inspection_does_not_form_lesson() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/repo".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Inspect continuity database".to_string(),
        );

        for (index, (command, summary)) in [
            (
                "cd /repo && sqlite3 .forge/continuity.db \".tables\" 2>&1",
                "Exit code: -1 Stdout: continuity_events continuity_experiences continuity_experiences_fts Stderr:",
            ),
            (
                "cd /repo && sqlite3 .forge/continuity.db \"SELECT event_type, COUNT(*) FROM continuity_events GROUP BY event_type\" 2>&1",
                "Exit code: -1 Stdout: user_message|1 tool_execution|30 Stderr:",
            ),
            (
                "cd /repo && ls -la .forge/ 2>&1; echo \"---\"; file .forge/continuity.db 2>&1; echo \"---\"; wc -c .forge/continuity.db 2>&1",
                "Exit code: -1 Stdout: total 312 drwxr-xr-x checkpoints -rw-r--r-- continuity.db --- SQLite 3.x database --- 4096 .forge/continuity.db Stderr:",
            ),
        ]
        .iter()
        .enumerate()
        {
            turn.record_tool(AgentToolTrace {
                tool_call_id: format!("tool-{index}"),
                name: "run_shell".to_string(),
                category: AgentToolCategory::Shell,
                status: AgentToolStatus::Failed,
                started_at_ms: 10 + index as u64,
                ended_at_ms: Some(20 + index as u64),
                result_summary: Some((*summary).to_string()),
                is_error: true,
                affected_files: Vec::new(),
                command: Some((*command).to_string()),
            });
        }
        turn.mark_status(AgentTurnStatus::Completed);

        let lessons = continuity_lessons_from_turn(&turn);

        assert!(
            lessons.is_empty(),
            "successful read-only inspection commands should not form failure lessons: {lessons:?}"
        );
    }

    #[test]
    fn failed_tool_lessons_do_not_echo_long_user_prompt() {
        let turn = AgentTurnState {
            turn_id: "turn-1".to_string(),
            session_id: "session-1".to_string(),
            workspace_path: "/repo/forge".to_string(),
            provider: "openai".to_string(),
            model: "gpt-test".to_string(),
            route: "direct".to_string(),
            phase: "executing".to_string(),
            user_goal:
                "我们现在在 /Users/cabbos/project/continuity-manual-test-app 做一次 Continuity 长流程人工测试，\
                 请故意更新 package.json 中的 test 命令，同时跑两个测试文件，观察候选经验是否进入 SQLite。"
                    .to_string(),
            input_intent: AgentTurnInputIntent::default(),
            execution_plan: None,
            context: AgentTurnContextSnapshot::default(),
            tools: vec![AgentToolTrace {
                tool_call_id: "tool-1".to_string(),
                name: "run_shell".to_string(),
                category: AgentToolCategory::Shell,
                status: AgentToolStatus::Failed,
                started_at_ms: 10,
                ended_at_ms: Some(20),
                result_summary: Some("npm ERR! Missing script: test".to_string()),
                is_error: true,
                affected_files: Vec::new(),
                command: Some("npm test".to_string()),
            }],
            evidence: Vec::new(),
            compact_events: Vec::new(),
            verification: AgentVerificationTrace::default(),
            failure: None,
            transition_log: Vec::new(),
            status: AgentTurnStatus::Failed,
            stop_reason: None,
            model_rounds: 0,
            tool_call_count: 1,
            failed_tool_count: 1,
            compact_saved_tokens: 0,
            created_at_ms: 1,
            updated_at_ms: 20,
        };

        let lessons = continuity_lessons_from_turn(&turn);

        assert_eq!(lessons.len(), 1);
        assert!(lessons[0].contains("run_shell"));
        assert!(lessons[0].contains("npm test"));
        assert!(lessons[0].contains("Missing script"));
        assert!(!lessons[0].contains("continuity-manual-test-app"));
        assert!(!lessons[0].contains("长流程人工测试"));
    }
}
