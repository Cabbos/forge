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
    use super::super::episode::{build_episode_from_turn, FileChangeRecord};
    use super::super::experience_lesson_reject_reason;
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
    fn compiler_suppresses_recovered_edit_miss_and_keeps_real_goal() {
        let episode = make_episode(
            "我们现在继续在 /Users/cabbos/project/continuity-manual-test-app 这个测试项目里做一次 Forge Continuity 召回验证。\n\n请只修改这个测试项目：\n/Users/cabbos/project/continuity-manual-test-app\n\n不要修改 Forge 主仓库：\n/Users/cabbos/project/crusted-spinning-lynx-agent\n\n不要修改任何 .forge 目录，不要清理 SQLite，不要提交 git，不要 push。\n\n这次任务目标：\n扩展已有“任务搜索”能力，让搜索框支持按任务状态关键词过滤。\n\n具体行为：todo / doing / done 大小写不敏感。",
            vec!["src/storage.ts", "src/search.test.ts"],
            vec![
                AgentToolTrace {
                    tool_call_id: "storage-edit".to_string(),
                    name: "edit_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 10,
                    ended_at_ms: Some(20),
                    result_summary: Some("File edited: src/storage.ts".to_string()),
                    is_error: false,
                    affected_files: vec!["src/storage.ts".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "test-edit-miss".to_string(),
                    name: "edit_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Failed,
                    started_at_ms: 20,
                    ended_at_ms: Some(30),
                    result_summary: Some("Error: old_string not found in file".to_string()),
                    is_error: true,
                    affected_files: vec!["src/search.test.ts".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "test-edit-success".to_string(),
                    name: "edit_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 30,
                    ended_at_ms: Some(40),
                    result_summary: Some("File edited: src/search.test.ts".to_string()),
                    is_error: false,
                    affected_files: vec!["src/search.test.ts".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "npm-test".to_string(),
                    name: "run_shell".to_string(),
                    category: AgentToolCategory::Shell,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 40,
                    ended_at_ms: Some(50),
                    result_summary: Some("Exit code: 0 Stdout: 61 passed".to_string()),
                    is_error: false,
                    affected_files: vec![],
                    command: Some("npm test".to_string()),
                },
                AgentToolTrace {
                    tool_call_id: "tsc".to_string(),
                    name: "run_shell".to_string(),
                    category: AgentToolCategory::Shell,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 50,
                    ended_at_ms: Some(60),
                    result_summary: Some("Exit code: 0 Stdout: EXIT CODE: 0".to_string()),
                    is_error: false,
                    affected_files: vec![],
                    command: Some("npx tsc --noEmit 2>&1; echo \"EXIT CODE: $?\"".to_string()),
                },
            ],
            ReflectionOutcome::Completed,
            AgentVerificationStatus::Passed,
        );

        let experiences = ExperienceCompiler::compile(&episode, Some("/repo"), 42);
        assert!(
            !experiences.is_empty(),
            "verified state-search change should produce candidates"
        );
        assert!(
            experiences
                .iter()
                .all(|experience| experience.kind != ExperienceKind::BugPattern),
            "recovered edit miss should not produce bug-pattern candidates: {experiences:?}"
        );
        assert!(
            experiences
                .iter()
                .all(|experience| !experience.tags.contains(&"has-failure".to_string())),
            "recovered edit miss should not tag candidates as failure: {experiences:?}"
        );

        let joined = experiences
            .iter()
            .map(|experience| experience.body.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("任务状态关键词过滤"),
            "body should keep the real task goal: {joined}"
        );
        assert!(
            !joined.contains("请只修改这个测试项目") && !joined.contains("不要修改 Forge 主仓库"),
            "body should not echo prompt constraints: {joined}"
        );
        assert!(
            !joined.contains("old_string not found"),
            "recovered edit miss should not be recorded as reusable experience: {joined}"
        );
    }

    #[test]
    fn compiler_keeps_pre_summarized_eval_prompt_constraints() {
        let workspace =
            "/var/folders/23/kzpccc795dd3vk7c72qhscfw0000gn/T/forge-eval-task/workspace";
        let prompt = "在当前 TypeScript 项目中新增任务状态汇总工具。新增 src/task-summary.ts，从 src/storage.ts 导入 Task，导出类型 TaskSummary = { total: number; todo: number; doing: number; done: number; active: number }，并导出 summarizeTasks(tasks: Task[]): TaskSummary。active 等于 todo + doing。新增 src/task-summary.test.ts，覆盖空列表、只有 todo、混合状态、active 计算、不会改变输入数组。更新 package.json 的 test script，让 npm test 能运行这个测试。最后确认 npm test 和 npx tsc --noEmit 都通过。不要修改 .env、.forge、package-lock.json。";
        let episode = Episode {
            project_path: Some(workspace.to_string()),
            session_id: "session-1".to_string(),
            user_goal_summary: prompt.to_string(),
            changed_files: vec![
                format!("{workspace}/package.json"),
                format!("{workspace}/src/task-summary.test.ts"),
                format!("{workspace}/src/task-summary.ts"),
            ],
            tool_count: 22,
            failed_tools: 4,
            file_changes: vec![
                FileChangeRecord {
                    path: format!("{workspace}/src/task-summary.ts"),
                    operation: "modified".to_string(),
                    tool_name: "write_to_file".to_string(),
                },
                FileChangeRecord {
                    path: format!("{workspace}/src/task-summary.test.ts"),
                    operation: "modified".to_string(),
                    tool_name: "write_to_file".to_string(),
                },
                FileChangeRecord {
                    path: format!("{workspace}/package.json"),
                    operation: "modified".to_string(),
                    tool_name: "edit_file".to_string(),
                },
            ],
            verification_status: AgentVerificationStatus::Passed,
            verification_command: Some("npx tsc --noEmit".to_string()),
            verification_summary: Some("passed; cmd=npx tsc --noEmit; exit=0".to_string()),
            outcome: ReflectionOutcome::Completed,
            evidence_event_ids: vec!["write-impl".to_string(), "tsc".to_string()],
            notable_failures: vec![],
            final_result_summary: Some("3 file(s) changed".to_string()),
            timestamp_ms: 1_778_688_000_000,
        };
        let body =
            formatters::format_structured_body(&episode, formatters::StructuredBodyMode::Primary);

        assert!(
            !should_reject_experience_lesson(&body),
            "structured eval body should not be rejected: {body}"
        );

        let experiences = ExperienceCompiler::compile(&episode, Some(workspace), 42);
        assert!(
            !experiences.is_empty(),
            "structured eval episode should produce candidates; body={body}"
        );
    }

    #[test]
    fn compiler_keeps_truncated_eval_goal_summary() {
        let workspace =
            "/var/folders/23/kzpccc795dd3vk7c72qhscfw0000gn/T/forge-eval-continuity-pipeline-task-summary-0ccz9xhz/workspace";
        let goal = "在当前 TypeScript 项目中新增任务状态汇总工具。新增 src/task-summary.ts，从 src/storage.ts 导入 Task，导出类型 TaskSummary = { total: number; todo: number; doing: number; done: number; active: number }，并导出 summarizeTasks(tasks: T...";
        let episode = Episode {
            project_path: Some(workspace.to_string()),
            session_id: "session-1".to_string(),
            user_goal_summary: goal.to_string(),
            changed_files: vec![
                format!("{workspace}/package.json"),
                format!("{workspace}/src/task-summary.test.ts"),
                format!("{workspace}/src/task-summary.ts"),
            ],
            tool_count: 15,
            failed_tools: 0,
            file_changes: vec![
                FileChangeRecord {
                    path: format!("{workspace}/src/task-summary.ts"),
                    operation: "modified".to_string(),
                    tool_name: "write_to_file".to_string(),
                },
                FileChangeRecord {
                    path: format!("{workspace}/src/task-summary.test.ts"),
                    operation: "modified".to_string(),
                    tool_name: "write_to_file".to_string(),
                },
                FileChangeRecord {
                    path: format!("{workspace}/package.json"),
                    operation: "modified".to_string(),
                    tool_name: "edit_file".to_string(),
                },
            ],
            verification_status: AgentVerificationStatus::Passed,
            verification_command: Some("npx tsc --noEmit".to_string()),
            verification_summary: Some("passed; cmd=npx tsc --noEmit; exit=0".to_string()),
            outcome: ReflectionOutcome::Completed,
            evidence_event_ids: vec!["write-impl".to_string(), "tsc".to_string()],
            notable_failures: vec![],
            final_result_summary: Some("3 file(s) changed".to_string()),
            timestamp_ms: 1_778_688_000_000,
        };
        let body =
            formatters::format_structured_body(&episode, formatters::StructuredBodyMode::Primary);

        assert!(
            !should_reject_experience_lesson(&body),
            "truncated eval body should not be rejected; reason={:?}; body={body}",
            experience_lesson_reject_reason(&body)
        );

        let experiences = ExperienceCompiler::compile(&episode, Some(workspace), 42);
        assert!(
            !experiences.is_empty(),
            "truncated eval episode should produce candidates; body={body}"
        );
    }

    #[test]
    fn compiler_keeps_verified_success_even_when_prompt_ends_with_forbidden_files() {
        let episode = make_episode(
            "在当前 TypeScript 项目中新增任务状态汇总工具。新增 src/task-summary.ts，从 src/storage.ts 导入 Task，导出类型 TaskSummary = { total: number; todo: number; doing: number; done: number; active: number }，并导出 summarizeTasks(tasks: Task[]): TaskSummary。active 等于 todo + doing。新增 src/task-summary.test.ts，覆盖空列表、只有 todo、混合状态、active 计算、不会改变输入数组。更新 package.json 的 test script，让 npm test 能运行这个测试。最后确认 npm test 和 npx tsc --noEmit 都通过。不要修改 .env、.forge、package-lock.json。",
            vec!["package.json", "src/task-summary.ts", "src/task-summary.test.ts"],
            vec![
                AgentToolTrace {
                    tool_call_id: "write-impl".to_string(),
                    name: "write_to_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 10,
                    ended_at_ms: Some(20),
                    result_summary: Some("File written: src/task-summary.ts".to_string()),
                    is_error: false,
                    affected_files: vec!["src/task-summary.ts".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "write-test".to_string(),
                    name: "write_to_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 20,
                    ended_at_ms: Some(30),
                    result_summary: Some("File written: src/task-summary.test.ts".to_string()),
                    is_error: false,
                    affected_files: vec!["src/task-summary.test.ts".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "edit-package".to_string(),
                    name: "edit_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 30,
                    ended_at_ms: Some(40),
                    result_summary: Some("File edited: package.json".to_string()),
                    is_error: false,
                    affected_files: vec!["package.json".to_string()],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "npm-test".to_string(),
                    name: "run_shell".to_string(),
                    category: AgentToolCategory::Shell,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 40,
                    ended_at_ms: Some(50),
                    result_summary: Some("Exit code: 0 Stdout: 5 pass 0 fail".to_string()),
                    is_error: false,
                    affected_files: vec![],
                    command: Some("npm test".to_string()),
                },
                AgentToolTrace {
                    tool_call_id: "tsc".to_string(),
                    name: "run_shell".to_string(),
                    category: AgentToolCategory::Shell,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 50,
                    ended_at_ms: Some(60),
                    result_summary: Some("Exit code: 0".to_string()),
                    is_error: false,
                    affected_files: vec![],
                    command: Some("npx tsc --noEmit".to_string()),
                },
            ],
            ReflectionOutcome::Completed,
            AgentVerificationStatus::Passed,
        );

        let experiences = ExperienceCompiler::compile(&episode, Some("/repo"), 42);
        assert!(
            !experiences.is_empty(),
            "verified task-summary change should produce at least one candidate"
        );

        let joined = experiences
            .iter()
            .map(|experience| experience.body.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("src/task-summary.ts"),
            "candidate should keep the implemented file as evidence: {joined}"
        );
    }

    #[test]
    fn compiler_keeps_workflow_candidate_when_sensitive_hook_failure_is_recovered() {
        let workspace =
            "/var/folders/23/kzpccc795dd3vk7c72qhscfw0000gn/T/forge-eval-task/workspace";
        let episode = make_episode(
            "在当前 TypeScript 项目中新增任务状态汇总工具。新增 src/task-summary.ts 和 src/task-summary.test.ts，更新 package.json 的 test script。最后确认 npm test 和 npx tsc --noEmit 都通过。",
            vec![],
            vec![
                AgentToolTrace {
                    tool_call_id: "write-impl".to_string(),
                    name: "write_to_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 10,
                    ended_at_ms: Some(20),
                    result_summary: Some("File written: src/task-summary.ts".to_string()),
                    is_error: false,
                    affected_files: vec![format!("{workspace}/src/task-summary.ts")],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "write-test".to_string(),
                    name: "write_to_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 20,
                    ended_at_ms: Some(30),
                    result_summary: Some("File written: src/task-summary.test.ts".to_string()),
                    is_error: false,
                    affected_files: vec![format!("{workspace}/src/task-summary.test.ts")],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "edit-package".to_string(),
                    name: "edit_file".to_string(),
                    category: AgentToolCategory::Write,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 30,
                    ended_at_ms: Some(40),
                    result_summary: Some("File edited: package.json".to_string()),
                    is_error: false,
                    affected_files: vec![format!("{workspace}/package.json")],
                    command: None,
                },
                AgentToolTrace {
                    tool_call_id: "blocked-shell".to_string(),
                    name: "run_shell".to_string(),
                    category: AgentToolCategory::Shell,
                    status: AgentToolStatus::Failed,
                    started_at_ms: 40,
                    ended_at_ms: Some(45),
                    result_summary: Some(
                        "Tool execution blocked by hook: 已阻止：工具输入中疑似包含敏感信息，请移除密钥、令牌或私钥后再继续。"
                            .to_string(),
                    ),
                    is_error: true,
                    affected_files: vec![],
                    command: Some("npm test && npx tsc --noEmit".to_string()),
                },
                AgentToolTrace {
                    tool_call_id: "npm-test".to_string(),
                    name: "run_shell".to_string(),
                    category: AgentToolCategory::Shell,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 50,
                    ended_at_ms: Some(60),
                    result_summary: Some("Exit code: 0 Stdout: 5 pass 0 fail".to_string()),
                    is_error: false,
                    affected_files: vec![],
                    command: Some("npm test".to_string()),
                },
                AgentToolTrace {
                    tool_call_id: "tsc".to_string(),
                    name: "run_shell".to_string(),
                    category: AgentToolCategory::Shell,
                    status: AgentToolStatus::Completed,
                    started_at_ms: 60,
                    ended_at_ms: Some(70),
                    result_summary: Some("Exit code: 0".to_string()),
                    is_error: false,
                    affected_files: vec![],
                    command: Some("npx tsc --noEmit".to_string()),
                },
            ],
            ReflectionOutcome::Completed,
            AgentVerificationStatus::Passed,
        );

        let experiences = ExperienceCompiler::compile(&episode, Some("/repo"), 42);
        assert!(
            !experiences.is_empty(),
            "a recovered sensitive-hook failure must not erase all verified change candidates"
        );
        assert!(
            experiences
                .iter()
                .any(|experience| experience.body.contains("src/task-summary.ts")),
            "candidate should retain changed-file evidence: {experiences:?}"
        );
    }

    #[test]
    fn compiler_keeps_truncated_typescript_goal_with_absolute_paths() {
        let workspace =
            "/var/folders/23/kzpccc795dd3vk7c72qhscfw0000gn/T/forge-eval-task/workspace";
        let episode = Episode {
            project_path: Some(workspace.to_string()),
            session_id: "session-1".to_string(),
            user_goal_summary: "在当前 TypeScript 项目中新增任务状态汇总工具。新增 src/task-summary.ts，从 src/storage.ts 导入 Task，导出类型 TaskSummary = { total: number; todo: number; doing: number; done: number; active: number }，并导出 summarizeTasks(tasks: T...".to_string(),
            changed_files: vec![
                format!("{workspace}/package.json"),
                format!("{workspace}/src/task-summary.test.ts"),
                format!("{workspace}/src/task-summary.ts"),
            ],
            tool_count: 13,
            failed_tools: 0,
            file_changes: vec![
                super::super::episode::FileChangeRecord {
                    path: format!("{workspace}/src/task-summary.ts"),
                    operation: "modified".to_string(),
                    tool_name: "write_to_file".to_string(),
                },
                super::super::episode::FileChangeRecord {
                    path: format!("{workspace}/src/task-summary.test.ts"),
                    operation: "modified".to_string(),
                    tool_name: "write_to_file".to_string(),
                },
                super::super::episode::FileChangeRecord {
                    path: format!("{workspace}/package.json"),
                    operation: "modified".to_string(),
                    tool_name: "edit_file".to_string(),
                },
            ],
            verification_status: AgentVerificationStatus::Passed,
            verification_command: Some("npx tsc --noEmit".to_string()),
            verification_summary: Some("passed; cmd=npx tsc --noEmit; exit=0".to_string()),
            outcome: ReflectionOutcome::Completed,
            evidence_event_ids: vec!["write-impl".to_string(), "tsc".to_string()],
            notable_failures: vec![],
            final_result_summary: Some("3 file(s) changed".to_string()),
            timestamp_ms: 1_778_688_000_000,
        };

        let experiences = ExperienceCompiler::compile(&episode, Some(workspace), 42);
        assert!(
            !experiences.is_empty(),
            "truncated TypeScript goal with absolute paths should still form"
        );
        assert!(
            experiences[0].body.contains("src/task-summary.ts"),
            "body should keep changed file evidence: {}",
            experiences[0].body
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
