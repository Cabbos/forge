use crate::agent::turn_state::AgentVerificationStatus;

use super::{episode::Episode, ReflectionOutcome};

pub(crate) enum StructuredBodyMode {
    Primary,
    FailurePattern,
    Workflow,
}

pub(crate) fn format_structured_body(episode: &Episode, mode: StructuredBodyMode) -> String {
    let mut lines = Vec::new();
    let goal = experience_goal_label(episode);

    // Problem
    let problem = match mode {
        StructuredBodyMode::Primary => {
            if episode.failed_tools > 0 && !episode.notable_failures.is_empty() {
                format!(
                    "While working on '{}', encountered {} tool failure(s).",
                    goal, episode.failed_tools
                )
            } else {
                format!("Needed to implement '{}'.", goal)
            }
        }
        StructuredBodyMode::FailurePattern => {
            let failure = &episode.notable_failures[0];
            format!(
                "Tool '{}' failed during '{}': {}.",
                failure.tool_name,
                goal,
                summarize_text(&failure.summary, 120)
            )
        }
        StructuredBodyMode::Workflow => {
            format!(
                "Multi-step change '{}' required {} tools across {} files.",
                goal,
                episode.tool_count,
                episode.changed_files.len()
            )
        }
    };
    lines.push(format!("Problem: {problem}"));

    // Cause
    if !episode.notable_failures.is_empty() {
        let cause = match mode {
            StructuredBodyMode::Primary | StructuredBodyMode::FailurePattern => {
                let failure = &episode.notable_failures[0];
                if let Some(cmd) = &failure.command {
                    format!(
                        "'{}' produced: {}",
                        summarize_text(cmd, 80),
                        summarize_text(&failure.summary, 120)
                    )
                } else {
                    summarize_text(&failure.summary, 160)
                }
            }
            StructuredBodyMode::Workflow => {
                format!(
                    "{} tools executed; {} failed.",
                    episode.tool_count, episode.failed_tools
                )
            }
        };
        lines.push(format!("Cause: {cause}"));
    } else if matches!(mode, StructuredBodyMode::Workflow) {
        lines.push("Cause: Multiple interdependent files needed coordinated updates.".to_string());
    }

    // Fix
    let fix = match mode {
        StructuredBodyMode::Primary => {
            if episode.changed_files.len() == 1 {
                format!("Modified {}.", episode.changed_files[0])
            } else {
                format!(
                    "Modified {} files: {}.",
                    episode.changed_files.len(),
                    episode
                        .changed_files
                        .iter()
                        .map(|f| summarize_text(f, 60))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        StructuredBodyMode::FailurePattern => {
            if episode.changed_files.len() == 1 {
                format!("Fixed by updating {}.", episode.changed_files[0])
            } else {
                format!("Fixed by updating {} files.", episode.changed_files.len())
            }
        }
        StructuredBodyMode::Workflow => {
            format!(
                "Applied changes across {} files using {} tool invocations.",
                episode.changed_files.len(),
                episode.tool_count
            )
        }
    };
    lines.push(format!("Fix: {fix}"));

    // Verified by
    match episode.verification_status {
        AgentVerificationStatus::Passed => {
            if let Some(vs) = &episode.verification_summary {
                if !vs.is_empty() {
                    lines.push(format!("Verified by: {vs}"));
                }
            }
        }
        AgentVerificationStatus::Failed => {
            lines.push("Verified by: failed — this is an unverified candidate".to_string());
        }
        AgentVerificationStatus::Error => {
            lines.push("Verified by: error — this is an unverified candidate".to_string());
        }
        AgentVerificationStatus::NotNeeded => {
            lines.push("Verified by: manual review (no automated verification)".to_string());
        }
        AgentVerificationStatus::Skipped => {
            lines.push("Verified by: skipped".to_string());
        }
        AgentVerificationStatus::Running => {
            lines.push("Verified by: running".to_string());
        }
    }

    // Applies when
    lines.push(format!(
        "Applies when: working on '{}' with {} tools and {} changed file(s).",
        goal,
        episode.tool_count,
        episode.changed_files.len()
    ));

    // Evidence
    let evidence = if episode.file_changes.is_empty() {
        format!(
            "turn outcome={:?}; tools={}; failed={}",
            episode.outcome, episode.tool_count, episode.failed_tools
        )
    } else {
        let changes = episode
            .file_changes
            .iter()
            .map(|fc| format!("{} ({} via {})", fc.path, fc.operation, fc.tool_name))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "file_changes=[{changes}]; outcome={:?}; tools={}; failed={}",
            episode.outcome, episode.tool_count, episode.failed_tools
        )
    };
    lines.push(format!("Evidence: {evidence}"));

    lines.join(" ")
}

fn experience_goal_label(episode: &Episode) -> String {
    let cleaned = clean_goal_summary(&episode.user_goal_summary);
    if !cleaned.is_empty() {
        return summarize_text(&cleaned, 90);
    }

    if episode.changed_files.is_empty() {
        return "this task".to_string();
    }

    if episode.changed_files.len() == 1 {
        return format!(
            "changes to {}",
            summarize_text(&episode.changed_files[0], 60)
        );
    }

    format!("changes across {} files", episode.changed_files.len())
}

fn clean_goal_summary(summary: &str) -> String {
    let normalized = summary.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return String::new();
    }

    for marker in [
        "任务目标：",
        "任务目标:",
        "目标：",
        "目标:",
        "task:",
        "goal:",
        "objective:",
    ] {
        if let Some(index) = normalized.to_lowercase().find(&marker.to_lowercase()) {
            let value = &normalized[index + marker.len()..];
            return clean_goal_fragment(value);
        }
    }

    let mut candidates = normalized
        .split(['。', '；', ';', '\n'])
        .map(clean_goal_fragment)
        .filter(|part| !part.is_empty())
        .filter(|part| !looks_like_prompt_context(part))
        .collect::<Vec<_>>();

    if let Some(candidate) = candidates.pop() {
        return candidate;
    }

    clean_goal_fragment(&normalized)
}

fn clean_goal_fragment(value: &str) -> String {
    let fragment = value
        .split(['。', '；', ';', '\n'])
        .next()
        .unwrap_or(value)
        .trim()
        .trim_matches(['"', '\'', '“', '”', '‘', '’', '.', ',', '，', ':', '：']);

    strip_prompt_prefix(fragment)
        .trim()
        .trim_matches(['"', '\'', '“', '”', '‘', '’', '.', ',', '，', ':', '：'])
        .to_string()
}

fn strip_prompt_prefix(value: &str) -> &str {
    let lower = value.to_lowercase();
    for prefix in [
        "请严格",
        "请先",
        "请优先",
        "请检查",
        "帮我实现",
        "帮我做",
        "帮我写",
        "please implement",
        "please build",
        "i want to",
    ] {
        if lower.starts_with(prefix) {
            return value[prefix.len()..].trim();
        }
    }
    value
}

fn looks_like_prompt_context(value: &str) -> bool {
    let lower = value.to_lowercase();
    [
        "我们现在在 ",
        "人工测试",
        "人工验证",
        "请严格只修改",
        "目标不是",
        "验证本地",
        "完整闭环",
        "观察候选",
    ]
    .iter()
    .any(|marker| lower.contains(&marker.to_lowercase()))
}

pub(crate) fn confidence_for_episode(episode: &Episode) -> f32 {
    let base = match episode.outcome {
        ReflectionOutcome::Completed => 0.74,
        ReflectionOutcome::Failed => 0.62,
        ReflectionOutcome::Cancelled => 0.55,
    };

    let verification_boost = match episode.verification_status {
        AgentVerificationStatus::Passed => 0.08,
        AgentVerificationStatus::Failed => -0.10,
        AgentVerificationStatus::Error => -0.12,
        _ => 0.0,
    };

    let change_boost = if episode.changed_files.len() >= 2 {
        0.03
    } else {
        0.0
    };

    let failure_penalty = if episode.failed_tools > 0 {
        -0.05 * (episode.failed_tools as f32).min(3.0)
    } else {
        0.0
    };

    (base + verification_boost + change_boost + failure_penalty).clamp(0.30, 0.95)
}

pub(crate) fn episode_tags(episode: &Episode) -> Vec<String> {
    let mut tags = Vec::new();

    for path in &episode.changed_files {
        if let Some(ext) = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
        {
            let tag = format!("ext:{ext}");
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
    }

    if episode.failed_tools > 0 {
        tags.push("has-failure".to_string());
    }

    match episode.verification_status {
        AgentVerificationStatus::Passed => tags.push("verified".to_string()),
        AgentVerificationStatus::Failed => tags.push("verify-failed".to_string()),
        AgentVerificationStatus::Error => tags.push("verify-failed".to_string()),
        AgentVerificationStatus::NotNeeded => tags.push("unverified".to_string()),
        _ => {}
    }

    tags
}

pub(crate) fn experience_id(
    project_path: Option<&str>,
    session_id: &str,
    timestamp_ms: u64,
    index: usize,
) -> String {
    let project_component = project_id_component(project_path);
    format!(
        "experience:{}:{}:{}:{}",
        project_component, session_id, timestamp_ms, index
    )
}

pub(crate) fn project_id_component(project_path: Option<&str>) -> String {
    let Some(project_path) = project_path else {
        return "global".to_string();
    };
    format!("project-{:016x}", stable_hash(project_path.as_bytes()))
}

pub(crate) fn stable_hash(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

pub(crate) fn title_from_body(body: &str) -> String {
    const MAX_TITLE_CHARS: usize = 80;
    let sentence = body
        .split(['.', '。', '!', '！', '?', '？'])
        .next()
        .unwrap_or(body)
        .trim();
    let title_source = if sentence.is_empty() { body } else { sentence };
    summarize_text(title_source, MAX_TITLE_CHARS)
}

pub(crate) fn summarize_text(value: &str, limit: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= limit {
        return normalized;
    }
    let truncated: String = normalized.chars().take(limit).collect();
    format!("{truncated}...")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::turn_state::{
        AgentToolCategory, AgentToolStatus, AgentToolTrace, AgentTurnState, AgentTurnStatus,
        AgentVerificationTrace,
    };
    use crate::continuity::episode::build_episode_from_turn;

    fn make_episode(
        goal: &str,
        _files: Vec<&str>,
        tools: Vec<AgentToolTrace>,
        outcome: ReflectionOutcome,
        verification: AgentVerificationStatus,
    ) -> Episode {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/repo".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            goal.to_string(),
        );
        turn.tools = tools;
        turn.verification.status = verification;
        turn.mark_status(match outcome {
            ReflectionOutcome::Completed => AgentTurnStatus::Completed,
            ReflectionOutcome::Failed => AgentTurnStatus::Failed,
            ReflectionOutcome::Cancelled => AgentTurnStatus::Cancelled,
        });
        build_episode_from_turn(&turn)
    }

    #[test]
    fn failed_verification_candidate_is_marked_unverified() {
        let episode = make_episode(
            "Fix build error",
            vec!["src/lib.rs"],
            vec![
                AgentToolTrace {
                    tool_call_id: "t1".to_string(),
                    name: "write_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 10,
                    ended_at_ms: Some(20),
                    result_summary: None,
                    is_error: false,
                    affected_files: vec!["src/lib.rs".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "t2".to_string(),
                    name: "run_shell".to_string(),
                    category: AgentToolCategory::Shell,
                    status: AgentToolStatus::Failed,
                    started_at_ms: 20,
                    ended_at_ms: Some(30),
                    result_summary: Some("error: unresolved import".to_string()),
                    is_error: true,
                    affected_files: vec![],
                    command: Some("cargo test".to_string()),
                },
            ],
            ReflectionOutcome::Failed,
            AgentVerificationStatus::Failed,
        );

        let body = format_structured_body(&episode, StructuredBodyMode::Primary);
        assert!(
            body.contains("unverified candidate"),
            "failed verification body should mark candidate as unverified: {body}"
        );
        assert!(
            body.contains("Verified by: failed"),
            "failed verification body should contain 'Verified by: failed': {body}"
        );

        let tags = episode_tags(&episode);
        assert!(
            tags.contains(&"verify-failed".to_string()),
            "failed verification tags should contain 'verify-failed': {tags:?}"
        );

        let confidence = confidence_for_episode(&episode);
        assert!(
            confidence < 0.6,
            "failed verification should produce low confidence, got {confidence}"
        );
    }

    #[test]
    fn error_verification_candidate_is_marked_unverified() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/repo".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Fix build".to_string(),
        );
        turn.tools = vec![AgentToolTrace {
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
        }];
        turn.verification = AgentVerificationTrace {
            status: AgentVerificationStatus::Error,
            command: Some("cargo test".to_string()),
            exit_code: Some(1),
            stdout_preview: None,
            stderr_preview: Some("compilation failed".to_string()),
            duration_ms: Some(500),
            completed_at_ms: Some(30),
        };
        turn.mark_status(AgentTurnStatus::Failed);

        let episode = build_episode_from_turn(&turn);
        let body = format_structured_body(&episode, StructuredBodyMode::Primary);
        assert!(
            body.contains("unverified candidate"),
            "error verification body should mark candidate as unverified: {body}"
        );

        let tags = episode_tags(&episode);
        assert!(tags.contains(&"verify-failed".to_string()));

        let confidence = confidence_for_episode(&episode);
        assert!(
            confidence < 0.6,
            "error verification should produce low confidence, got {confidence}"
        );
    }

    #[test]
    fn passed_verification_candidate_is_not_marked_unverified() {
        let episode = make_episode(
            "Add npm test script",
            vec!["package.json"],
            vec![AgentToolTrace {
                tool_call_id: "t1".to_string(),
                name: "write_file".to_string(),
                category: AgentToolCategory::Write,
                status: AgentToolStatus::Completed,
                started_at_ms: 10,
                ended_at_ms: Some(20),
                result_summary: None,
                is_error: false,
                affected_files: vec!["package.json".to_string()],
                command: None,
            }],
            ReflectionOutcome::Completed,
            AgentVerificationStatus::Passed,
        );

        let body = format_structured_body(&episode, StructuredBodyMode::Primary);
        assert!(
            !body.contains("unverified candidate"),
            "passed verification body should NOT contain 'unverified': {body}"
        );
        assert!(
            body.contains("Verified by:"),
            "passed verification body should contain 'Verified by:': {body}"
        );

        let tags = episode_tags(&episode);
        assert!(
            tags.contains(&"verified".to_string()),
            "passed verification tags should contain 'verified': {tags:?}"
        );
        assert!(
            !tags.contains(&"verify-failed".to_string()),
            "passed verification tags should NOT contain 'verify-failed': {tags:?}"
        );
    }

    #[test]
    fn not_needed_verification_is_tagged_unverified() {
        let episode = make_episode(
            "Update README",
            vec!["README.md"],
            vec![AgentToolTrace {
                tool_call_id: "t1".to_string(),
                name: "write_file".to_string(),
                category: AgentToolCategory::Write,
                status: AgentToolStatus::Completed,
                started_at_ms: 10,
                ended_at_ms: Some(20),
                result_summary: None,
                is_error: false,
                affected_files: vec!["README.md".to_string()],
                command: None,
            }],
            ReflectionOutcome::Completed,
            AgentVerificationStatus::NotNeeded,
        );

        let tags = episode_tags(&episode);
        assert!(
            tags.contains(&"unverified".to_string()),
            "not-needed verification should be tagged 'unverified': {tags:?}"
        );
        assert!(
            !tags.contains(&"verified".to_string()),
            "not-needed verification should NOT be tagged 'verified': {tags:?}"
        );
    }

    #[test]
    fn read_only_sqlite_inspection_does_not_become_notable_failure() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/repo".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Inspect DB".to_string(),
        );
        turn.tools = vec![
            AgentToolTrace {
                tool_call_id: "t1".to_string(),
                name: "run_shell".to_string(),
                category: AgentToolCategory::Shell,
                status: AgentToolStatus::Failed,
                started_at_ms: 10,
                ended_at_ms: Some(20),
                result_summary: Some(
                    "Exit code: -1 Stdout: continuity_events continuity_experiences Stderr:"
                        .to_string(),
                ),
                is_error: true,
                affected_files: vec![],
                command: Some("sqlite3 .forge/continuity.db .tables".to_string()),
            },
            AgentToolTrace {
                tool_call_id: "t2".to_string(),
                name: "write_file".to_string(),
                category: AgentToolCategory::Write,
                status: AgentToolStatus::Completed,
                started_at_ms: 20,
                ended_at_ms: Some(30),
                result_summary: None,
                is_error: false,
                affected_files: vec!["src/main.rs".to_string()],
                command: None,
            },
        ];
        turn.mark_status(AgentTurnStatus::Completed);

        let episode = build_episode_from_turn(&turn);
        assert!(
            episode.notable_failures.is_empty(),
            "sqlite3 .tables false failure should not become notable: {:?}",
            episode.notable_failures
        );
    }

    #[test]
    fn read_only_ls_git_inspection_does_not_become_notable_failure() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/repo".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Check repo".to_string(),
        );
        turn.tools = vec![
            AgentToolTrace {
                tool_call_id: "t1".to_string(),
                name: "run_shell".to_string(),
                category: AgentToolCategory::Shell,
                status: AgentToolStatus::Failed,
                started_at_ms: 10,
                ended_at_ms: Some(20),
                result_summary: Some(
                    "Exit code: -1 Stdout: total 312 drwxr-xr-x Stderr:".to_string(),
                ),
                is_error: true,
                affected_files: vec![],
                command: Some("ls -la".to_string()),
            },
            AgentToolTrace {
                tool_call_id: "t2".to_string(),
                name: "run_shell".to_string(),
                category: AgentToolCategory::Shell,
                status: AgentToolStatus::Failed,
                started_at_ms: 20,
                ended_at_ms: Some(30),
                result_summary: Some("Exit code: -1 Stdout: On branch main Stderr:".to_string()),
                is_error: true,
                affected_files: vec![],
                command: Some("git status".to_string()),
            },
            AgentToolTrace {
                tool_call_id: "t3".to_string(),
                name: "write_file".to_string(),
                category: AgentToolCategory::Write,
                status: AgentToolStatus::Completed,
                started_at_ms: 30,
                ended_at_ms: Some(40),
                result_summary: None,
                is_error: false,
                affected_files: vec!["src/main.rs".to_string()],
                command: None,
            },
        ];
        turn.mark_status(AgentTurnStatus::Completed);

        let episode = build_episode_from_turn(&turn);
        assert!(
            episode.notable_failures.is_empty(),
            "ls/git read-only false failures should not become notable: {:?}",
            episode.notable_failures
        );
    }
}
