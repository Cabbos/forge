use super::{
    episode::Episode, filters::is_debugging_only_episode, formatters,
    should_reject_experience_lesson, ExperienceKind, ExperienceMemory, ExperienceStatus,
    ReflectionOutcome,
};

/// Episode-level experience compiler.
///
/// Produces 0–3 structured, reusable engineering experiences from a single episode.
/// Filters out debugging-only turns, turns without code changes, and low-signal failures.
pub struct ExperienceCompiler;

impl ExperienceCompiler {
    /// Compile an episode into candidate experiences.
    ///
    /// Returns an empty vector when:
    /// - the episode produced no file changes (pure inspection / chat)
    /// - the episode was cancelled
    /// - the episode looks like a debugging-only interaction
    /// - all notable failures are false positives
    pub fn compile(
        episode: &Episode,
        project_path: Option<&str>,
        now_ms: u64,
    ) -> Vec<ExperienceMemory> {
        // Filter: no code changes = no reusable experience
        if episode.changed_files.is_empty() {
            return Vec::new();
        }

        // Filter: cancelled turns are incomplete
        if matches!(episode.outcome, ReflectionOutcome::Cancelled) {
            return Vec::new();
        }

        // Filter: debugging-only interactions
        if is_debugging_only_episode(episode) {
            return Vec::new();
        }

        let mut candidates = Vec::new();

        // Primary experience: what was done and verified
        if let Some(primary) = Self::compile_primary_experience(episode, project_path, now_ms) {
            candidates.push(primary);
        }

        // Secondary experience: failure pattern if there were real failures
        if episode.failed_tools > 0 && !episode.notable_failures.is_empty() {
            if let Some(secondary) =
                Self::compile_failure_pattern_experience(episode, project_path, now_ms)
            {
                // Avoid duplicating the primary experience
                if !candidates.iter().any(|c| c.body == secondary.body) {
                    candidates.push(secondary);
                }
            }
        }

        // Tertiary experience: workflow pattern when multiple tools/file types were involved
        if episode.tool_count >= 3 && episode.changed_files.len() >= 2 {
            if let Some(tertiary) = Self::compile_workflow_experience(episode, project_path, now_ms)
            {
                if !candidates.iter().any(|c| c.body == tertiary.body) {
                    candidates.push(tertiary);
                }
            }
        }

        // Cap at 3
        candidates.truncate(3);
        candidates
    }

    fn compile_primary_experience(
        episode: &Episode,
        project_path: Option<&str>,
        now_ms: u64,
    ) -> Option<ExperienceMemory> {
        let body =
            formatters::format_structured_body(episode, formatters::StructuredBodyMode::Primary);
        if should_reject_experience_lesson(&body) {
            return None;
        }

        let kind = if episode.failed_tools > 0 && !episode.notable_failures.is_empty() {
            ExperienceKind::BugPattern
        } else {
            ExperienceKind::Lesson
        };

        let confidence = formatters::confidence_for_episode(episode);

        Some(ExperienceMemory {
            id: formatters::experience_id(
                project_path,
                &episode.session_id,
                episode.timestamp_ms,
                0,
            ),
            kind,
            status: ExperienceStatus::Candidate,
            title: formatters::title_from_body(&body),
            body,
            project_path: project_path.map(str::to_string),
            source_session_id: Some(episode.session_id.clone()),
            confidence,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            tags: formatters::episode_tags(episode),
        })
    }

    fn compile_failure_pattern_experience(
        episode: &Episode,
        project_path: Option<&str>,
        now_ms: u64,
    ) -> Option<ExperienceMemory> {
        let body = formatters::format_structured_body(
            episode,
            formatters::StructuredBodyMode::FailurePattern,
        );
        if should_reject_experience_lesson(&body) {
            return None;
        }

        Some(ExperienceMemory {
            id: formatters::experience_id(
                project_path,
                &episode.session_id,
                episode.timestamp_ms,
                1,
            ),
            kind: ExperienceKind::BugPattern,
            status: ExperienceStatus::Candidate,
            title: formatters::title_from_body(&body),
            body,
            project_path: project_path.map(str::to_string),
            source_session_id: Some(episode.session_id.clone()),
            confidence: formatters::confidence_for_episode(episode) * 0.92,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            tags: formatters::episode_tags(episode),
        })
    }

    fn compile_workflow_experience(
        episode: &Episode,
        project_path: Option<&str>,
        now_ms: u64,
    ) -> Option<ExperienceMemory> {
        let body =
            formatters::format_structured_body(episode, formatters::StructuredBodyMode::Workflow);
        if should_reject_experience_lesson(&body) {
            return None;
        }

        Some(ExperienceMemory {
            id: formatters::experience_id(
                project_path,
                &episode.session_id,
                episode.timestamp_ms,
                2,
            ),
            kind: ExperienceKind::Workflow,
            status: ExperienceStatus::Candidate,
            title: formatters::title_from_body(&body),
            body,
            project_path: project_path.map(str::to_string),
            source_session_id: Some(episode.session_id.clone()),
            confidence: formatters::confidence_for_episode(episode) * 0.88,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            tags: formatters::episode_tags(episode),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::episode::build_episode_from_turn;
    use super::*;
    use crate::agent::turn_state::{
        AgentToolCategory, AgentToolStatus, AgentToolTrace, AgentTurnState, AgentTurnStatus,
        AgentVerificationStatus,
    };

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
    fn compiler_returns_empty_for_no_file_changes() {
        let episode = make_episode(
            "Run tests",
            vec![],
            vec![AgentToolTrace {
                tool_call_id: "t1".to_string(),
                name: "run_shell".to_string(),
                category: AgentToolCategory::Shell,
                status: AgentToolStatus::Completed,
                started_at_ms: 10,
                ended_at_ms: Some(20),
                result_summary: Some("tests passed".to_string()),
                is_error: false,
                affected_files: vec![],
                command: Some("npm test".to_string()),
            }],
            ReflectionOutcome::Completed,
            AgentVerificationStatus::Passed,
        );

        let experiences = ExperienceCompiler::compile(&episode, Some("/repo"), 42);
        assert!(experiences.is_empty(), "no file changes = no experience");
    }

    #[test]
    fn compiler_returns_empty_for_read_only_affected_files() {
        let episode = make_episode(
            "Inspect task filtering behavior",
            vec!["package.json", "src", "src/tasks.tsx"],
            vec![
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
            ],
            ReflectionOutcome::Completed,
            AgentVerificationStatus::NotNeeded,
        );

        let experiences = ExperienceCompiler::compile(&episode, Some("/repo"), 42);
        assert!(
            experiences.is_empty(),
            "read-only inspected files must not produce continuity experience"
        );
    }

    #[test]
    fn compiler_returns_empty_for_cancelled_turn() {
        let episode = make_episode(
            "Add feature",
            vec!["src/main.rs"],
            vec![AgentToolTrace {
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
            }],
            ReflectionOutcome::Cancelled,
            AgentVerificationStatus::NotNeeded,
        );

        let experiences = ExperienceCompiler::compile(&episode, Some("/repo"), 42);
        assert!(experiences.is_empty(), "cancelled = no experience");
    }

    #[test]
    fn compiler_returns_empty_for_debugging_only() {
        let episode = make_episode(
            "检查一下 continuity.db",
            vec![".forge/continuity.db"],
            vec![AgentToolTrace {
                tool_call_id: "t1".to_string(),
                name: "run_shell".to_string(),
                category: AgentToolCategory::Shell,
                status: AgentToolStatus::Completed,
                started_at_ms: 10,
                ended_at_ms: Some(20),
                result_summary: Some("tables found".to_string()),
                is_error: false,
                affected_files: vec![".forge/continuity.db".to_string()],
                command: Some("sqlite3 .forge/continuity.db .tables".to_string()),
            }],
            ReflectionOutcome::Completed,
            AgentVerificationStatus::NotNeeded,
        );

        let experiences = ExperienceCompiler::compile(&episode, Some("/repo"), 42);
        assert!(experiences.is_empty(), "debugging-only = no experience");
    }

    #[test]
    fn compiler_produces_structured_body_with_all_sections() {
        let episode = make_episode(
            "Add npm test script",
            vec!["package.json"],
            vec![
                AgentToolTrace {
                    tool_call_id: "t1".to_string(),
                    name: "read_file".to_string(),
                    category: AgentToolCategory::Read,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 10,
                    ended_at_ms: Some(15),
                    result_summary: None,
                    is_error: false,
                    affected_files: vec!["package.json".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "t2".to_string(),
                    name: "write_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 15,
                    ended_at_ms: Some(20),
                    result_summary: None,
                    is_error: false,
                    affected_files: vec!["package.json".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "t3".to_string(),
                    name: "run_shell".to_string(),
                    category: AgentToolCategory::Shell,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 20,
                    ended_at_ms: Some(30),
                    result_summary: Some("tests passed".to_string()),
                    is_error: false,
                    affected_files: vec![],
                    command: Some("npm test".to_string()),
                },
            ],
            ReflectionOutcome::Completed,
            AgentVerificationStatus::Passed,
        );

        let experiences = ExperienceCompiler::compile(&episode, Some("/repo"), 42);
        assert_eq!(experiences.len(), 1, "should produce primary experience");

        let body = &experiences[0].body;
        assert!(
            body.contains("Problem:"),
            "body should have Problem section"
        );
        assert!(body.contains("Fix:"), "body should have Fix section");
        assert!(
            body.contains("Verified by:"),
            "body should have Verified by section"
        );
        assert!(
            body.contains("Applies when:"),
            "body should have Applies when section"
        );
        assert!(
            body.contains("Evidence:"),
            "body should have Evidence section"
        );
        assert!(
            body.contains("package.json"),
            "body should mention changed file"
        );
    }

    #[test]
    fn compiler_sanitizes_manual_test_prompt_goal() {
        let episode = make_episode(
            "我们现在在 /Users/cabbos/project/continuity-manual-test-app 做人工验证。请严格只修改这个测试项目，不要改 Forge 主仓库。任务目标：给任务列表增加按优先级排序能力，并补测试。",
            vec!["src/tasks.tsx", "src/sort.test.ts"],
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
                    affected_files: vec!["src/tasks.tsx".to_string()],
                    command: None,
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
                    affected_files: vec!["src/sort.test.ts".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "t3".to_string(),
                    name: "run_shell".to_string(),
                    category: AgentToolCategory::Shell,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 30,
                    ended_at_ms: Some(40),
                    result_summary: Some("tests passed".to_string()),
                    is_error: false,
                    affected_files: vec![],
                    command: Some("npm test".to_string()),
                },
            ],
            ReflectionOutcome::Completed,
            AgentVerificationStatus::Passed,
        );

        let experiences = ExperienceCompiler::compile(&episode, Some("/repo"), 42);
        assert!(
            !experiences.is_empty(),
            "manual test prompt should still produce a candidate for real verified code changes"
        );

        let body = &experiences[0].body;
        assert!(
            body.contains("按优先级排序"),
            "body should keep the task signal: {body}"
        );
        assert!(
            !body.contains("我们现在在") && !body.contains("人工验证") && !body.contains("请严格"),
            "body should not echo the manual test prompt: {body}"
        );
    }

    #[test]
    fn compiler_produces_failure_pattern_when_tools_fail() {
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

        let experiences = ExperienceCompiler::compile(&episode, Some("/repo"), 42);
        assert!(
            !experiences.is_empty(),
            "should produce at least one experience for failed turn with file changes"
        );

        let primary = &experiences[0];
        assert_eq!(primary.kind, ExperienceKind::BugPattern);
        assert!(
            primary.body.contains("Cause:"),
            "failure pattern should include Cause"
        );
        assert!(
            primary.confidence < 0.7,
            "failed verification should lower confidence"
        );
        assert!(
            primary.body.contains("unverified candidate"),
            "failed verification should mark body as unverified"
        );
        assert!(
            primary.tags.contains(&"verify-failed".to_string()),
            "failed verification should tag verify-failed"
        );
    }

    #[test]
    fn compiler_produces_multiple_candidates_for_complex_episode() {
        let episode = make_episode(
            "Refactor auth and add tests",
            vec!["src/auth.rs", "src/auth_test.rs", "src/lib.rs"],
            vec![
                AgentToolTrace {
                    tool_call_id: "t1".to_string(),
                    name: "read_file".to_string(),
                    category: AgentToolCategory::Read,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 10,
                    ended_at_ms: Some(15),
                    result_summary: None,
                    is_error: false,
                    affected_files: vec!["src/auth.rs".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "t2".to_string(),
                    name: "write_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 15,
                    ended_at_ms: Some(20),
                    result_summary: None,
                    is_error: false,
                    affected_files: vec!["src/auth.rs".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "t3".to_string(),
                    name: "write_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 20,
                    ended_at_ms: Some(25),
                    result_summary: None,
                    is_error: false,
                    affected_files: vec!["src/auth_test.rs".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "t4".to_string(),
                    name: "write_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 25,
                    ended_at_ms: Some(30),
                    result_summary: None,
                    is_error: false,
                    affected_files: vec!["src/lib.rs".to_string()],
                    command: None,
                },
            ],
            ReflectionOutcome::Completed,
            AgentVerificationStatus::Passed,
        );

        let experiences = ExperienceCompiler::compile(&episode, Some("/repo"), 42);
        assert!(
            experiences.len() >= 2,
            "complex episode should produce multiple candidates: got {:?}",
            experiences.len()
        );

        // Verify uniqueness
        let bodies: std::collections::HashSet<_> =
            experiences.iter().map(|e| e.body.clone()).collect();
        assert_eq!(
            bodies.len(),
            experiences.len(),
            "all candidates should have unique bodies"
        );
    }

    #[test]
    fn compiler_caps_at_three_candidates() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/repo".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Complex refactor".to_string(),
        );
        // Create many tools and file changes to trigger all experience types
        let mut tools = Vec::new();
        for i in 0..5 {
            tools.push(AgentToolTrace {
                tool_call_id: format!("t{i}"),
                name: "write_file".to_string(),
                category: AgentToolCategory::Write,
                status: AgentToolStatus::Completed,
                started_at_ms: 10 + i as u64,
                ended_at_ms: Some(20 + i as u64),
                result_summary: None,
                is_error: false,
                affected_files: vec![format!("src/file{i}.rs")],
                command: None,
            });
        }
        // Add a failure to trigger failure pattern
        tools.push(AgentToolTrace {
            tool_call_id: "tfail".to_string(),
            name: "run_shell".to_string(),
            category: AgentToolCategory::Shell,
            status: AgentToolStatus::Failed,
            started_at_ms: 100,
            ended_at_ms: Some(110),
            result_summary: Some("compile error".to_string()),
            is_error: true,
            affected_files: vec![],
            command: Some("cargo build".to_string()),
        });
        turn.tools = tools;
        turn.verification.status = AgentVerificationStatus::Passed;
        turn.mark_status(AgentTurnStatus::Completed);

        let episode = build_episode_from_turn(&turn);
        let experiences = ExperienceCompiler::compile(&episode, Some("/repo"), 42);
        assert!(
            experiences.len() <= 3,
            "should cap at 3 candidates, got {}",
            experiences.len()
        );
    }
}
