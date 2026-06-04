use serde::{Deserialize, Serialize};

use crate::agent::turn_state::{
    AgentToolCategory, AgentToolStatus, AgentToolTrace, AgentTurnState, AgentTurnStatus,
    AgentVerificationStatus, AgentVerificationTrace,
};

use super::filters::shell_failure_is_false_positive;
use super::ReflectionOutcome;

/// A single user-input episode, aggregating all execution traces for experience formation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Episode {
    pub project_path: Option<String>,
    pub session_id: String,
    pub user_goal_summary: String,
    pub changed_files: Vec<String>,
    pub tool_count: usize,
    pub failed_tools: usize,
    pub file_changes: Vec<FileChangeRecord>,
    pub verification_status: AgentVerificationStatus,
    pub verification_command: Option<String>,
    pub verification_summary: Option<String>,
    pub outcome: ReflectionOutcome,
    pub evidence_event_ids: Vec<String>,
    pub notable_failures: Vec<ToolFailureRecord>,
    pub final_result_summary: Option<String>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChangeRecord {
    pub path: String,
    pub operation: String,
    pub tool_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolFailureRecord {
    pub tool_name: String,
    pub command: Option<String>,
    pub summary: String,
}

/// Build an Episode from an agent turn.
pub fn build_episode_from_turn(turn: &AgentTurnState) -> Episode {
    let changed_files = collect_changed_files(turn);
    let file_changes = build_file_changes(turn);
    let verification = normalized_verification_trace(turn);
    let suppress_resolved_verification_failures =
        matches!(verification.status, AgentVerificationStatus::Passed);
    let notable_failures = collect_notable_failures(turn, suppress_resolved_verification_failures);

    let final_result_summary = if matches!(verification.status, AgentVerificationStatus::Passed) {
        build_success_summary(turn)
    } else if let Some(failure) = &turn.failure {
        Some(format!("{}: {}", failure.kind, failure.message))
    } else {
        build_success_summary(turn)
    };

    Episode {
        project_path: Some(turn.workspace_path.clone()),
        session_id: turn.session_id.clone(),
        user_goal_summary: summarize_user_goal(&turn.user_goal),
        changed_files: changed_files.clone(),
        tool_count: turn.tools.len(),
        failed_tools: turn
            .tools
            .iter()
            .filter(|tool| {
                tool_is_actionable_failure(tool, suppress_resolved_verification_failures)
            })
            .count(),
        file_changes,
        verification_status: verification.status.clone(),
        verification_command: verification.command.clone(),
        verification_summary: build_verification_summary(&verification),
        outcome: normalized_turn_outcome(turn, &verification),
        evidence_event_ids: turn.tools.iter().map(|t| t.tool_call_id.clone()).collect(),
        notable_failures,
        final_result_summary,
        timestamp_ms: turn.updated_at_ms,
    }
}

fn collect_changed_files(turn: &AgentTurnState) -> Vec<String> {
    let mut files: Vec<String> = turn
        .tools
        .iter()
        .filter(|tool| tool_records_file_change(tool))
        .flat_map(|tool| tool.affected_files.clone())
        .collect();
    files.sort();
    files.dedup();
    files
}

fn build_file_changes(turn: &AgentTurnState) -> Vec<FileChangeRecord> {
    let mut changes = Vec::new();
    for tool in &turn.tools {
        if !tool_records_file_change(tool) {
            continue;
        }
        for path in &tool.affected_files {
            let operation = match tool.category {
                AgentToolCategory::Write => "modified",
                AgentToolCategory::Shell => "modified_via_shell",
                _ => "affected",
            };
            changes.push(FileChangeRecord {
                path: path.clone(),
                operation: operation.to_string(),
                tool_name: tool.name.clone(),
            });
        }
    }
    changes
}

fn tool_records_file_change(tool: &AgentToolTrace) -> bool {
    matches!(
        tool.category,
        AgentToolCategory::Write | AgentToolCategory::Shell
    ) && !tool.affected_files.is_empty()
}

fn collect_notable_failures(
    turn: &AgentTurnState,
    suppress_resolved_verification_failures: bool,
) -> Vec<ToolFailureRecord> {
    turn.tools
        .iter()
        .filter(|tool| tool_is_actionable_failure(tool, suppress_resolved_verification_failures))
        .take(3)
        .map(|tool| ToolFailureRecord {
            tool_name: tool.name.clone(),
            command: tool.command.clone(),
            summary: tool
                .result_summary
                .as_deref()
                .unwrap_or("unknown error")
                .to_string(),
        })
        .collect()
}

fn normalized_verification_trace(turn: &AgentTurnState) -> AgentVerificationTrace {
    if !matches!(
        turn.verification.status,
        AgentVerificationStatus::Failed | AgentVerificationStatus::Error
    ) {
        return turn.verification.clone();
    }

    let Some(shell) = latest_successful_verification_shell(turn) else {
        return turn.verification.clone();
    };

    AgentVerificationTrace {
        status: AgentVerificationStatus::Passed,
        command: shell
            .command
            .clone()
            .or_else(|| turn.verification.command.clone()),
        exit_code: Some(0),
        stdout_preview: shell.result_summary.clone(),
        stderr_preview: None,
        duration_ms: shell
            .ended_at_ms
            .map(|ended_at| ended_at.saturating_sub(shell.started_at_ms)),
        completed_at_ms: shell.ended_at_ms,
    }
}

fn latest_successful_verification_shell(turn: &AgentTurnState) -> Option<&AgentToolTrace> {
    turn.tools.iter().rev().find(|tool| {
        matches!(tool.category, AgentToolCategory::Shell)
            && tool
                .command
                .as_deref()
                .is_some_and(command_looks_like_verification)
            && shell_failure_is_false_positive(
                tool.command.as_deref(),
                tool.result_summary.as_deref(),
            )
    })
}

fn command_looks_like_verification(command: &str) -> bool {
    let lower = command.to_lowercase();
    [
        "npm test",
        "npm run test",
        "pnpm test",
        "yarn test",
        "bun test",
        "npx tsc",
        "tsc",
        "npx tsx",
        "vitest",
        "jest",
        "cargo test",
        "cargo check",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn tool_is_actionable_failure(
    tool: &AgentToolTrace,
    suppress_resolved_verification_failures: bool,
) -> bool {
    if !tool_is_failed(tool)
        || shell_failure_is_false_positive(tool.command.as_deref(), tool.result_summary.as_deref())
    {
        return false;
    }

    if suppress_resolved_verification_failures
        && tool
            .command
            .as_deref()
            .is_some_and(command_looks_like_verification)
    {
        return false;
    }

    true
}

fn tool_is_failed(tool: &AgentToolTrace) -> bool {
    tool.is_error
        || matches!(
            tool.status,
            AgentToolStatus::Failed | AgentToolStatus::Cancelled
        )
}

fn build_verification_summary(
    trace: &crate::agent::turn_state::AgentVerificationTrace,
) -> Option<String> {
    let mut parts = Vec::new();
    match trace.status {
        AgentVerificationStatus::Passed => parts.push("passed".to_string()),
        AgentVerificationStatus::Failed => parts.push("failed".to_string()),
        AgentVerificationStatus::Error => parts.push("error".to_string()),
        AgentVerificationStatus::Skipped => parts.push("skipped".to_string()),
        AgentVerificationStatus::NotNeeded => return None,
        AgentVerificationStatus::Running => parts.push("running".to_string()),
    }
    if let Some(cmd) = trace.command.as_deref().filter(|c| !c.trim().is_empty()) {
        parts.push(format!("cmd={}", summarize_text(cmd, 120)));
    }
    if let Some(code) = trace.exit_code {
        parts.push(format!("exit={code}"));
    }
    Some(parts.join("; "))
}

fn normalized_turn_outcome(
    turn: &AgentTurnState,
    verification: &AgentVerificationTrace,
) -> ReflectionOutcome {
    if matches!(turn.status, AgentTurnStatus::Failed)
        && matches!(verification.status, AgentVerificationStatus::Passed)
    {
        return ReflectionOutcome::Completed;
    }
    turn_status_to_outcome(&turn.status)
}

fn build_success_summary(turn: &AgentTurnState) -> Option<String> {
    if turn.tools.is_empty() {
        return None;
    }
    let write_count = turn
        .tools
        .iter()
        .filter(|t| matches!(t.category, AgentToolCategory::Write))
        .count();
    let shell_count = turn
        .tools
        .iter()
        .filter(|t| matches!(t.category, AgentToolCategory::Shell))
        .count();
    let read_count = turn
        .tools
        .iter()
        .filter(|t| matches!(t.category, AgentToolCategory::Read))
        .count();

    let mut parts = Vec::new();
    if write_count > 0 {
        parts.push(format!("{write_count} write(s)"));
    }
    if shell_count > 0 {
        parts.push(format!("{shell_count} shell command(s)"));
    }
    if read_count > 0 {
        parts.push(format!("{read_count} read(s)"));
    }
    let changed_files = collect_changed_files(turn);
    if !changed_files.is_empty() {
        parts.push(format!("{} file(s) changed", changed_files.len()));
    }

    Some(parts.join(", "))
}

fn summarize_user_goal(goal: &str) -> String {
    summarize_text(goal, 200)
}

fn summarize_text(value: &str, limit: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= limit {
        return normalized;
    }
    let truncated: String = normalized.chars().take(limit).collect();
    format!("{truncated}...")
}

fn turn_status_to_outcome(status: &AgentTurnStatus) -> ReflectionOutcome {
    match status {
        AgentTurnStatus::Completed => ReflectionOutcome::Completed,
        AgentTurnStatus::Failed => ReflectionOutcome::Failed,
        AgentTurnStatus::Cancelled => ReflectionOutcome::Cancelled,
        _ => ReflectionOutcome::Completed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::turn_state::AgentTurnState;

    fn test_turn_with_tools(tools: Vec<AgentToolTrace>) -> AgentTurnState {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/repo".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Add feature".to_string(),
        );
        turn.tools = tools;
        turn.mark_status(AgentTurnStatus::Completed);
        turn
    }

    #[test]
    fn episode_captures_changed_files_and_tool_counts() {
        let turn = test_turn_with_tools(vec![
            AgentToolTrace {
                tool_call_id: "t1".to_string(),
                name: "write_file".to_string(),
                category: AgentToolCategory::Write,
                status: AgentToolStatus::Completed,
                started_at_ms: 10,
                ended_at_ms: Some(20),
                result_summary: None,
                is_error: false,
                affected_files: vec!["src/main.rs".to_string()],
                command: None,
            },
            AgentToolTrace {
                tool_call_id: "t2".to_string(),
                name: "run_shell".to_string(),
                category: AgentToolCategory::Shell,
                status: AgentToolStatus::Failed,
                started_at_ms: 20,
                ended_at_ms: Some(30),
                result_summary: Some("npm ERR! missing script".to_string()),
                is_error: true,
                affected_files: Vec::new(),
                command: Some("npm test".to_string()),
            },
        ]);

        let episode = build_episode_from_turn(&turn);

        assert_eq!(episode.tool_count, 2);
        assert_eq!(episode.failed_tools, 1);
        assert_eq!(episode.changed_files, vec!["src/main.rs"]);
        assert_eq!(episode.file_changes.len(), 1);
        assert_eq!(episode.file_changes[0].path, "src/main.rs");
        assert_eq!(episode.notable_failures.len(), 1);
        assert_eq!(episode.notable_failures[0].tool_name, "run_shell");
    }

    #[test]
    fn episode_skips_false_positive_failures() {
        let mut turn = test_turn_with_tools(vec![AgentToolTrace {
            tool_call_id: "t1".to_string(),
            name: "run_shell".to_string(),
            category: AgentToolCategory::Shell,
            status: AgentToolStatus::Failed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some("Exit code: -1 Stdout: tests passed EXIT: 0".to_string()),
            is_error: true,
            affected_files: Vec::new(),
            command: Some("npm test".to_string()),
        }]);
        turn.verification = crate::agent::turn_state::AgentVerificationTrace {
            status: AgentVerificationStatus::Passed,
            command: Some("cargo test".to_string()),
            exit_code: Some(0),
            stdout_preview: Some("test result: ok".to_string()),
            stderr_preview: None,
            duration_ms: Some(1000),
            completed_at_ms: Some(30),
        };

        let episode = build_episode_from_turn(&turn);

        assert_eq!(episode.failed_tools, 0);
        assert!(
            episode.notable_failures.is_empty(),
            "EXIT: 0 false positive should be excluded"
        );
        assert_eq!(episode.verification_status, AgentVerificationStatus::Passed);
    }

    #[test]
    fn episode_normalizes_failed_verification_when_later_shell_shows_success() {
        let mut turn = test_turn_with_tools(vec![
            AgentToolTrace {
                tool_call_id: "write".to_string(),
                name: "write_file".to_string(),
                category: AgentToolCategory::Write,
                status: AgentToolStatus::Completed,
                started_at_ms: 10,
                ended_at_ms: Some(20),
                result_summary: None,
                is_error: false,
                affected_files: vec!["src/tasks.tsx".to_string()],
                command: None,
            },
            AgentToolTrace {
                tool_call_id: "verify".to_string(),
                name: "run_shell".to_string(),
                category: AgentToolCategory::Shell,
                status: AgentToolStatus::Failed,
                started_at_ms: 20,
                ended_at_ms: Some(30),
                result_summary: Some("Exit code: -1 Stdout: EXIT=0 Stderr:".to_string()),
                is_error: true,
                affected_files: Vec::new(),
                command: Some("npx tsc --noEmit 2>&1; echo \"EXIT=$?\"".to_string()),
            },
        ]);
        turn.verification = crate::agent::turn_state::AgentVerificationTrace {
            status: AgentVerificationStatus::Failed,
            command: Some("tsc".to_string()),
            exit_code: Some(1),
            stdout_preview: None,
            stderr_preview: Some("failed".to_string()),
            duration_ms: Some(1000),
            completed_at_ms: Some(30),
        };
        turn.mark_status(AgentTurnStatus::Failed);

        let episode = build_episode_from_turn(&turn);

        assert_eq!(episode.verification_status, AgentVerificationStatus::Passed);
        assert_eq!(episode.outcome, ReflectionOutcome::Completed);
        assert_eq!(episode.failed_tools, 0);
        assert!(
            episode
                .verification_summary
                .as_deref()
                .is_some_and(|summary| summary.contains("passed")),
            "verification summary should be normalized to passed: {:?}",
            episode.verification_summary
        );
        assert!(
            episode.notable_failures.is_empty(),
            "successful verification shell should not become notable failure"
        );
    }

    #[test]
    fn episode_suppresses_resolved_verification_failures_after_later_success() {
        let mut turn = test_turn_with_tools(vec![
            AgentToolTrace {
                tool_call_id: "write".to_string(),
                name: "write_file".to_string(),
                category: AgentToolCategory::Write,
                status: AgentToolStatus::Completed,
                started_at_ms: 10,
                ended_at_ms: Some(20),
                result_summary: None,
                is_error: false,
                affected_files: vec!["src/tasks.tsx".to_string()],
                command: None,
            },
            AgentToolTrace {
                tool_call_id: "tsc-empty-1".to_string(),
                name: "run_shell".to_string(),
                category: AgentToolCategory::Shell,
                status: AgentToolStatus::Failed,
                started_at_ms: 20,
                ended_at_ms: Some(30),
                result_summary: Some("Exit code: -1 Stdout: Stderr:".to_string()),
                is_error: true,
                affected_files: Vec::new(),
                command: Some("npx tsc --noEmit 2>&1".to_string()),
            },
            AgentToolTrace {
                tool_call_id: "tsc-version".to_string(),
                name: "run_shell".to_string(),
                category: AgentToolCategory::Shell,
                status: AgentToolStatus::Failed,
                started_at_ms: 30,
                ended_at_ms: Some(40),
                result_summary: Some("Exit code: -1 Stdout: Version 5.9.3 Stderr:".to_string()),
                is_error: true,
                affected_files: Vec::new(),
                command: Some("npx tsc --version 2>&1".to_string()),
            },
            AgentToolTrace {
                tool_call_id: "tsc-success".to_string(),
                name: "run_shell".to_string(),
                category: AgentToolCategory::Shell,
                status: AgentToolStatus::Completed,
                started_at_ms: 40,
                ended_at_ms: Some(50),
                result_summary: Some("Exit code: 0 Stdout: EXIT CODE: 0 Stderr:".to_string()),
                is_error: false,
                affected_files: Vec::new(),
                command: Some("npx tsc --noEmit 2>&1; echo \"EXIT CODE: $?\"".to_string()),
            },
        ]);
        turn.verification = crate::agent::turn_state::AgentVerificationTrace {
            status: AgentVerificationStatus::Failed,
            command: Some("tsc".to_string()),
            exit_code: Some(1),
            stdout_preview: None,
            stderr_preview: Some("failed".to_string()),
            duration_ms: Some(1000),
            completed_at_ms: Some(50),
        };
        turn.mark_status(AgentTurnStatus::Failed);

        let episode = build_episode_from_turn(&turn);

        assert_eq!(episode.verification_status, AgentVerificationStatus::Passed);
        assert_eq!(episode.outcome, ReflectionOutcome::Completed);
        assert_eq!(episode.failed_tools, 0);
        assert!(
            episode.notable_failures.is_empty(),
            "resolved verification retries should not become bug-pattern failures: {:?}",
            episode.notable_failures
        );
    }

    #[test]
    fn episode_without_file_changes_has_empty_changed_files() {
        let turn = test_turn_with_tools(vec![AgentToolTrace {
            tool_call_id: "t1".to_string(),
            name: "read_file".to_string(),
            category: AgentToolCategory::Read,
            status: AgentToolStatus::Completed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: None,
            is_error: false,
            affected_files: Vec::new(),
            command: None,
        }]);

        let episode = build_episode_from_turn(&turn);
        assert!(episode.changed_files.is_empty());
        assert!(episode.file_changes.is_empty());
    }

    #[test]
    fn episode_ignores_read_only_affected_files_as_changes() {
        let turn = test_turn_with_tools(vec![
            AgentToolTrace {
                tool_call_id: "t1".to_string(),
                name: "read_file".to_string(),
                category: AgentToolCategory::Read,
                status: AgentToolStatus::Completed,
                started_at_ms: 10,
                ended_at_ms: Some(20),
                result_summary: Some("package json content".to_string()),
                is_error: false,
                affected_files: vec!["package.json".to_string()],
                command: None,
            },
            AgentToolTrace {
                tool_call_id: "t2".to_string(),
                name: "list_directory".to_string(),
                category: AgentToolCategory::Read,
                status: AgentToolStatus::Completed,
                started_at_ms: 20,
                ended_at_ms: Some(30),
                result_summary: Some("src tasks package".to_string()),
                is_error: false,
                affected_files: vec!["src".to_string()],
                command: None,
            },
            AgentToolTrace {
                tool_call_id: "t3".to_string(),
                name: "search_files".to_string(),
                category: AgentToolCategory::Read,
                status: AgentToolStatus::Completed,
                started_at_ms: 30,
                ended_at_ms: Some(40),
                result_summary: Some("found matches".to_string()),
                is_error: false,
                affected_files: vec!["src/tasks.tsx".to_string()],
                command: None,
            },
        ]);

        let episode = build_episode_from_turn(&turn);

        assert_eq!(episode.tool_count, 3);
        assert!(episode.changed_files.is_empty());
        assert!(episode.file_changes.is_empty());
        assert_eq!(episode.final_result_summary.as_deref(), Some("3 read(s)"));
    }
}
