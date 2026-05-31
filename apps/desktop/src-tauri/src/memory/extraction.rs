use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus, WikiMemory};
use crate::memory::risk::should_reject_persistent_memory;
use crate::memory::storage::now_string;

pub fn extract_candidates_from_user_message(
    session_id: &str,
    project_path: Option<&str>,
    text: &str,
) -> Vec<WikiMemory> {
    let body = collapse_whitespace(text);
    if body.chars().count() < 8
        || should_suppress_persistent_memory(&body)
        || is_low_signal_continuation(&body)
        || should_reject_persistent_memory(&body)
    {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    let now = now_string();

    if contains_any(
        &body,
        &[
            "以后",
            "我希望",
            "我喜欢",
            "不用验证",
            "我来验证",
            "默认",
            "优先",
            "default",
            "prefer",
        ],
    ) {
        let is_task_like = is_task_like_instruction(&body);
        let is_project_pref = is_project_specific_preference(&body);
        let project_scoped = (is_project_pref || is_task_like) && project_path.is_some();

        // Task-like instructions must never enter global UserProfile.
        // If no project_path is available, suppress rather than pollute global scope.
        if is_task_like && !project_scoped && project_path.is_none() {
            // skip: task instruction without project context → do not create UserProfile
        } else {
            candidates.push(candidate(
                session_id,
                if project_scoped { project_path } else { None },
                MemoryCategory::Preference,
                if project_scoped {
                    MemoryScope::Project
                } else {
                    MemoryScope::UserProfile
                },
                &candidate_title("用户偏好", &body),
                &body,
                0.72,
                "preference",
                &now,
            ));
        }
    }

    if project_path.is_some()
        && contains_any(
            &body,
            &[
                "只改",
                "只修改",
                "不要动",
                "不要改",
                "不要碰",
                "不能改",
                "不要污染",
                "工作区",
                "当前项目",
                "目标项目",
                "workspace",
                "only change",
                "do not touch",
                "don't touch",
            ],
        )
    {
        candidates.push(candidate(
            session_id,
            project_path,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            &candidate_title("项目事实", &body),
            &body,
            0.66,
            "project_fact",
            &now,
        ));
    }

    if contains_any(
        &body,
        &[
            "方向",
            "定了",
            "已定",
            "确定方向",
            "决定",
            "选择",
            "就用",
            "方案",
            "产品",
            "兼容",
        ],
    ) {
        candidates.push(candidate(
            session_id,
            project_path,
            MemoryCategory::Decision,
            MemoryScope::Project,
            &candidate_title("项目已定方案", &body),
            &body,
            0.68,
            "decision",
            &now,
        ));
    }

    if contains_any(
        &body,
        &[
            "继续",
            "接下来",
            "已经完成",
            "已经做到",
            "做到",
            "先把",
            "下一步",
        ],
    ) {
        candidates.push(candidate(
            session_id,
            project_path,
            MemoryCategory::TaskState,
            MemoryScope::Project,
            &candidate_title("当前进度", &body),
            &body,
            0.6,
            "task_state",
            &now,
        ));
    }

    candidates
}

#[allow(clippy::too_many_arguments)]
fn candidate(
    session_id: &str,
    project_path: Option<&str>,
    category: MemoryCategory,
    scope: MemoryScope,
    title: &str,
    body: &str,
    confidence: f32,
    tag: &str,
    now: &str,
) -> WikiMemory {
    WikiMemory {
        id: uuid::Uuid::now_v7().to_string(),
        category,
        scope,
        status: MemoryStatus::Candidate,
        title: title.to_string(),
        body: truncate_chars(body, 360),
        project_path: project_path.map(str::to_string),
        source_session_id: Some(session_id.to_string()),
        source_message_ids: Vec::new(),
        confidence,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        last_used_at: None,
        use_count: 0,
        tags: vec![tag.to_string()],
    }
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn contains_any(text: &str, signals: &[&str]) -> bool {
    signals.iter().any(|signal| text.contains(signal))
}

fn is_project_specific_preference(text: &str) -> bool {
    let lower = text.to_lowercase();
    contains_any(
        &lower,
        &[
            "这个项目",
            "本项目",
            "当前项目",
            "项目里",
            "项目中",
            "这个仓库",
            "本仓库",
            "当前仓库",
            "这个 repo",
            "this project",
            "this repo",
        ],
    )
}

fn is_task_like_instruction(text: &str) -> bool {
    let lower = text.to_lowercase();
    contains_any(
        &lower,
        &[
            "我想做",
            "请先做",
            "请优先",
            "优先推进",
            "优先做",
            "先把",
            "第一版",
            "可预览",
            "预览",
            "验收",
            "下一步",
            "做到哪",
            "小工具",
            "页面",
            "功能",
            "修复",
            "bug",
            "demo",
            "帮我实现",
            "帮我做",
            "帮我写",
            "i want to make",
            "i want to build",
            "please implement",
            "please build",
            "prioritize",
            "first version",
            "mvp",
        ],
    )
}

fn should_suppress_persistent_memory(text: &str) -> bool {
    contains_any(
        text,
        &[
            "不要记住",
            "别记住",
            "不要保存",
            "别保存",
            "不要作为长期偏好",
            "不是长期偏好",
            "只是临时",
            "临时测试",
            "只在本轮",
            "只在这次",
            "do not remember",
            "don't remember",
            "do not save",
            "temporary only",
            "just for this turn",
        ],
    )
}

fn is_low_signal_continuation(text: &str) -> bool {
    if text.chars().count() > 24 {
        return false;
    }
    if contains_any(
        text,
        &[
            "先把",
            "已经完成",
            "已经做到",
            "做到",
            "定了",
            "已定",
            "决定",
            "方案",
            "方向",
            "不要",
            "只改",
            "默认",
            "优先",
            "生成",
            "修复",
            "补齐",
        ],
    ) {
        return false;
    }

    contains_any(
        text,
        &[
            "继续",
            "接下来",
            "下一步",
            "往下走",
            "然后呢",
            "ok",
            "OK",
            "可以的",
            "好的",
        ],
    )
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn candidate_title(label: &str, body: &str) -> String {
    format!("{label}：{}", short_summary(body))
}

fn short_summary(body: &str) -> String {
    const MAX_CHARS: usize = 32;
    let summary = truncate_chars(body, MAX_CHARS);
    if body.chars().count() > MAX_CHARS {
        format!("{summary}...")
    } else {
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::extract_candidates_from_user_message;
    use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus};

    #[test]
    fn extracts_low_risk_preference() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/project"),
            "  以后   默认用中文回答，我来验证结果  ",
        );

        assert_eq!(candidates.len(), 1);
        let memory = &candidates[0];
        assert_eq!(memory.category, MemoryCategory::Preference);
        assert_eq!(memory.scope, MemoryScope::UserProfile);
        assert_eq!(memory.status, MemoryStatus::Candidate);
        assert!(memory.title.starts_with("用户偏好："));
        assert_eq!(memory.body, "以后 默认用中文回答，我来验证结果");
        assert_eq!(memory.project_path, None);
        assert_eq!(memory.source_session_id.as_deref(), Some("session-1"));
        assert!(memory.source_message_ids.is_empty());
        assert_eq!(memory.confidence, 0.72);
        assert_eq!(memory.tags, vec!["preference"]);
        assert_eq!(memory.use_count, 0);
        assert_eq!(memory.last_used_at, None);
        assert!(!memory.id.is_empty());
        assert!(!memory.created_at.is_empty());
        assert_eq!(memory.created_at, memory.updated_at);
    }

    #[test]
    fn project_specific_preference_stays_project_scoped() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/project"),
            "这个项目默认使用 pnpm，优先保持现在的目录结构。",
        );

        assert_eq!(candidates.len(), 1);
        let memory = &candidates[0];
        assert_eq!(memory.category, MemoryCategory::Preference);
        assert_eq!(memory.scope, MemoryScope::Project);
        assert_eq!(memory.project_path.as_deref(), Some("/tmp/project"));
    }

    #[test]
    fn task_like_priority_instruction_stays_project_scoped() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/pomodoro"),
            "我想做一个番茄钟小工具，可以开始、暂停、重置。请优先推进到一个可预览的第一版。",
        );

        assert!(candidates.iter().any(|memory| {
            memory.category == MemoryCategory::Preference
                && memory.scope == MemoryScope::Project
                && memory.project_path.as_deref() == Some("/tmp/pomodoro")
        }));
        assert!(candidates
            .iter()
            .all(|memory| memory.scope != MemoryScope::UserProfile));
    }

    #[test]
    fn english_project_preference_stays_project_scoped() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/project"),
            "This project should default to pnpm first.",
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].scope, MemoryScope::Project);
        assert_eq!(candidates[0].project_path.as_deref(), Some("/tmp/project"));
    }

    #[test]
    fn rejects_secret_like_content() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/project"),
            "以后默认 token: abcdefghijklmnop",
        );

        assert!(candidates.is_empty());
    }

    #[test]
    fn suppresses_memory_when_user_says_do_not_remember() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/project"),
            "不要记住这个，只是临时测试：以后这个项目默认用亮色主题。",
        );

        assert!(candidates.is_empty());
    }

    #[test]
    fn suppresses_memory_when_user_says_session_only() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/project"),
            "这条不要作为长期偏好，只在本轮里用：以后回答都短一点。",
        );

        assert!(candidates.is_empty());
    }

    #[test]
    fn extracts_project_decision_and_task_state() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/project"),
            "产品方向定了，就用兼容旧方案。接下来先把存储做到可用。",
        );

        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().any(|memory| {
            memory.category == MemoryCategory::Decision
                && memory.scope == MemoryScope::Project
                && memory.project_path.as_deref() == Some("/tmp/project")
                && memory.title.starts_with("项目已定方案：")
                && memory.confidence == 0.68
                && memory.tags == vec!["decision"]
        }));
        assert!(candidates.iter().any(|memory| {
            memory.category == MemoryCategory::TaskState
                && memory.scope == MemoryScope::Project
                && memory.project_path.as_deref() == Some("/tmp/project")
                && memory.title.starts_with("当前进度：")
                && memory.confidence == 0.6
                && memory.tags == vec!["task_state"]
        }));
    }

    #[test]
    fn ignores_low_signal_continuation_questions() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/project"),
            "接下来我们继续吧",
        );

        assert!(candidates.is_empty());
    }

    #[test]
    fn keeps_explicit_next_step_as_task_state() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/project"),
            "接下来先把 Agent Core 的恢复链路补齐。",
        );

        assert!(candidates.iter().any(|memory| {
            memory.category == MemoryCategory::TaskState
                && memory.scope == MemoryScope::Project
                && memory.tags == vec!["task_state"]
        }));
    }

    #[test]
    fn extracts_project_safety_boundary_as_project_fact() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/forge-test-app"),
            "这个任务只改 demo 项目，不要动 Forge 本体。",
        );

        assert_eq!(candidates.len(), 1);
        let memory = &candidates[0];
        assert_eq!(memory.category, MemoryCategory::ProjectFact);
        assert_eq!(memory.scope, MemoryScope::Project);
        assert_eq!(memory.project_path.as_deref(), Some("/tmp/forge-test-app"));
        assert!(memory.title.starts_with("项目事实："));
        assert_eq!(memory.body, "这个任务只改 demo 项目，不要动 Forge 本体。");
        assert_eq!(memory.tags, vec!["project_fact"]);
    }

    #[test]
    fn avoids_routine_test_message_as_decision() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/project"),
            "一定要先跑测试",
        );

        assert!(candidates.is_empty());
    }

    #[test]
    fn avoids_low_value_lint_status() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/project"),
            "已经修了 lint",
        );

        assert!(candidates.is_empty());
    }

    #[test]
    fn ignores_short_messages() {
        let candidates = extract_candidates_from_user_message("session-1", None, " 默认 ");

        assert!(candidates.is_empty());
    }

    #[test]
    fn task_like_instruction_without_project_does_not_create_user_profile() {
        // "我想做一个番茄钟小工具…请优先推进第一版" — the original pollution incident
        // Without project_path, this must NOT create a UserProfile preference.
        let candidates = extract_candidates_from_user_message(
            "session-1",
            None,
            "我想做一个番茄钟小工具，可以开始、暂停、重置。请优先推进到一个可预览的第一版。",
        );

        assert!(
            candidates
                .iter()
                .all(|c| c.scope != MemoryScope::UserProfile),
            "task-like instruction without project_path must not enter UserProfile, got: {:?}",
            candidates.iter().map(|c| &c.scope).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn task_like_instruction_with_project_stays_project_scoped() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/pomodoro"),
            "我想做一个番茄钟小工具，请优先推进到第一版。",
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].scope, MemoryScope::Project);
        assert_eq!(candidates[0].project_path.as_deref(), Some("/tmp/pomodoro"));
    }

    #[test]
    fn pure_long_preference_without_project_stays_user_profile() {
        // "以后所有项目都用中文回复" — a genuine long-term preference
        let candidates = extract_candidates_from_user_message(
            "session-1",
            None,
            "以后所有项目都默认用中文回复我。",
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].scope, MemoryScope::UserProfile);
        assert_eq!(candidates[0].project_path, None);
    }

    #[test]
    fn prioritize_keyword_triggers_task_like_suppression_without_project() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            None,
            "请优先推进第一版的小工具功能。",
        );

        assert!(
            candidates.is_empty(),
            "prioritize without project should not create UserProfile"
        );
    }

    #[test]
    fn english_i_want_to_build_without_project_is_suppressed() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            None,
            "I want to build a timer widget, please implement the first version.",
        );

        assert!(
            candidates.is_empty(),
            "English task instruction without project should not create UserProfile"
        );
    }

    #[test]
    fn english_i_want_to_build_with_project_stays_project_scoped() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/timer"),
            "I prefer to build a timer widget first, please implement the first version.",
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].scope, MemoryScope::Project);
        assert_eq!(candidates[0].project_path.as_deref(), Some("/tmp/timer"));
    }

    #[test]
    fn mixed_preference_and_task_without_project_is_suppressed() {
        // "默认优先推进" hits both preference keywords AND task-like
        let candidates = extract_candidates_from_user_message(
            "session-1",
            None,
            "默认优先推进番茄钟的重置功能。",
        );

        assert!(
            candidates
                .iter()
                .all(|c| c.scope != MemoryScope::UserProfile),
            "mixed signal with task-like should not create UserProfile"
        );
    }

    #[test]
    fn decision_extraction_requires_project_path() {
        // Decision without project_path should still create (it's always Project scope)
        let candidates = extract_candidates_from_user_message(
            "session-1",
            None,
            "产品方向定了，就用兼容旧方案。",
        );

        // Decision always creates with project_path (None here is ok, scope is Project)
        assert!(candidates
            .iter()
            .any(|c| c.category == MemoryCategory::Decision));
    }
}
