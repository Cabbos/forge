use serde::{Deserialize, Serialize};

use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus, WikiMemory};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuggestedAction {
    /// Memory looks like a task instruction stored as UserProfile — review and archive/forget.
    Archive,
    /// Memory is clearly stale pollution — safe to forget.
    Forget,
    /// Memory is Project-scoped but has no project_path — attach one or forget.
    AttachProject,
    /// Memory needs human review to decide action.
    Review,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuspiciousMemory {
    pub memory_id: String,
    pub title: String,
    pub scope: MemoryScope,
    pub category: MemoryCategory,
    pub project_path: Option<String>,
    pub reasons: Vec<String>,
    pub suggested_action: SuggestedAction,
}

/// Scan a memory store for suspicious/orphaned memories.
/// Returns a list of findings — does NOT modify the store.
pub fn scan_memory_hygiene(memories: &[WikiMemory]) -> Vec<SuspiciousMemory> {
    let mut findings = Vec::new();

    for memory in memories {
        // Skip already-forgotten or archived — they're already handled.
        if matches!(
            memory.status,
            MemoryStatus::Forgotten | MemoryStatus::Archived
        ) {
            continue;
        }

        let mut reasons = Vec::new();
        let mut suggested_action = SuggestedAction::Review;

        let memory_text = format!("{} {} {}", memory.title, memory.body, memory.tags.join(" "));
        let lower = memory_text.to_lowercase();

        // Rule 1: UserProfile + task-like content
        if memory.scope == MemoryScope::UserProfile
            && memory.category == MemoryCategory::Preference
            && memory.project_path.is_none()
            && has_task_like_signals(&lower)
        {
            reasons.push("UserProfile contains task-like instruction content".to_string());
            suggested_action = SuggestedAction::Forget;
        }

        // Rule 2: Project-scoped TaskState/Decision without project_path
        if memory.scope == MemoryScope::Project
            && memory.project_path.is_none()
            && matches!(
                memory.category,
                MemoryCategory::TaskState | MemoryCategory::Decision
            )
        {
            reasons
                .push("Project-scoped TaskState/Decision has no project_path (orphan)".to_string());
            suggested_action = SuggestedAction::AttachProject;
        }

        // Rule 3: UserProfile scope but body contains project goal signals
        if memory.scope == MemoryScope::UserProfile
            && memory.project_path.is_none()
            && has_project_goal_signals(&lower)
            && !reasons.iter().any(|r| r.contains("task-like"))
        {
            reasons.push(
                "UserProfile contains project goal signals that should be project-scoped"
                    .to_string(),
            );
            suggested_action = SuggestedAction::Archive;
        }

        if !reasons.is_empty() {
            findings.push(SuspiciousMemory {
                memory_id: memory.id.clone(),
                title: memory.title.clone(),
                scope: memory.scope.clone(),
                category: memory.category.clone(),
                project_path: memory.project_path.clone(),
                reasons,
                suggested_action,
            });
        }
    }

    findings
}

/// Signals that indicate task-like instruction content.
fn has_task_like_signals(text: &str) -> bool {
    const SIGNALS: &[&str] = &[
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
        "实现",
        "做到",
        "推进",
    ];
    SIGNALS.iter().any(|signal| text.contains(signal))
}

/// Signals that indicate project goal content that doesn't belong in UserProfile.
fn has_project_goal_signals(text: &str) -> bool {
    const SIGNALS: &[&str] = &[
        "当前项目",
        "这个项目",
        "项目档案",
        "当前进度",
        "小工具",
        "第一版",
        "可预览",
        "预览",
        "验收",
        "下一步",
        "做到哪",
        "页面",
        "demo",
        "this project",
        "this repo",
    ];
    SIGNALS.iter().any(|signal| text.contains(signal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::storage::now_string;

    fn make_memory(
        id: &str,
        scope: MemoryScope,
        category: MemoryCategory,
        status: MemoryStatus,
        title: &str,
        body: &str,
        project_path: Option<&str>,
    ) -> WikiMemory {
        let now = now_string();
        WikiMemory {
            id: id.to_string(),
            category,
            scope,
            status,
            title: title.to_string(),
            body: body.to_string(),
            project_path: project_path.map(str::to_string),
            source_session_id: Some("session-1".to_string()),
            source_message_ids: Vec::new(),
            confidence: 0.8,
            created_at: now.clone(),
            updated_at: now,
            last_used_at: None,
            use_count: 0,
            tags: Vec::new(),
        }
    }

    #[test]
    fn tomato_clock_user_profile_flagged_as_suspicious() {
        let memory = make_memory(
            "tomato-clock",
            MemoryScope::UserProfile,
            MemoryCategory::Preference,
            MemoryStatus::Accepted,
            "用户偏好：我想做一个番茄钟小工具",
            "我想做一个番茄钟小工具，可以开始、暂停、重置。请优先推进到一个可预览的第一版。",
            None,
        );

        let findings = scan_memory_hygiene(&[memory]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].memory_id, "tomato-clock");
        assert!(findings[0].reasons.iter().any(|r| r.contains("task-like")));
        assert_eq!(findings[0].suggested_action, SuggestedAction::Forget);
    }

    #[test]
    fn orphan_task_state_without_project_path_flagged() {
        let memory = make_memory(
            "orphan-task",
            MemoryScope::Project,
            MemoryCategory::TaskState,
            MemoryStatus::Accepted,
            "当前进度",
            "第一步已经完成。",
            None,
        );

        let findings = scan_memory_hygiene(&[memory]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].memory_id, "orphan-task");
        assert!(findings[0].reasons.iter().any(|r| r.contains("orphan")));
        assert_eq!(findings[0].suggested_action, SuggestedAction::AttachProject);
    }

    #[test]
    fn orphan_decision_without_project_path_flagged() {
        let memory = make_memory(
            "orphan-decision",
            MemoryScope::Project,
            MemoryCategory::Decision,
            MemoryStatus::Accepted,
            "项目已定方案",
            "产品方向定了，就用兼容旧方案。",
            None,
        );

        let findings = scan_memory_hygiene(&[memory]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].suggested_action, SuggestedAction::AttachProject);
    }

    #[test]
    fn valid_project_memory_with_path_not_flagged() {
        let memory = make_memory(
            "valid-task",
            MemoryScope::Project,
            MemoryCategory::TaskState,
            MemoryStatus::Accepted,
            "当前进度",
            "第一步已经完成。",
            Some("/tmp/forge"),
        );

        let findings = scan_memory_hygiene(&[memory]);

        assert!(findings.is_empty());
    }

    #[test]
    fn genuine_user_profile_preference_not_flagged() {
        let memory = make_memory(
            "lang-pref",
            MemoryScope::UserProfile,
            MemoryCategory::Preference,
            MemoryStatus::Accepted,
            "用户偏好",
            "以后所有项目都默认用中文回复。",
            None,
        );

        let findings = scan_memory_hygiene(&[memory]);

        assert!(findings.is_empty());
    }

    #[test]
    fn forgotten_memory_skipped() {
        let memory = make_memory(
            "forgotten",
            MemoryScope::UserProfile,
            MemoryCategory::Preference,
            MemoryStatus::Forgotten,
            "用户偏好",
            "我想做一个番茄钟。",
            None,
        );

        let findings = scan_memory_hygiene(&[memory]);

        assert!(findings.is_empty());
    }

    #[test]
    fn archived_memory_skipped() {
        let memory = make_memory(
            "archived",
            MemoryScope::UserProfile,
            MemoryCategory::Preference,
            MemoryStatus::Archived,
            "用户偏好",
            "我想做一个番茄钟。",
            None,
        );

        let findings = scan_memory_hygiene(&[memory]);

        assert!(findings.is_empty());
    }

    #[test]
    fn user_profile_with_project_goal_signals_flagged_as_review() {
        let memory = make_memory(
            "project-goal-in-profile",
            MemoryScope::UserProfile,
            MemoryCategory::Preference,
            MemoryStatus::Accepted,
            "用户偏好",
            "当前项目进度报告已经生成了，交互优化进行中。",
            None,
        );

        let findings = scan_memory_hygiene(&[memory]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].suggested_action, SuggestedAction::Archive);
        assert!(findings[0]
            .reasons
            .iter()
            .any(|r| r.contains("project goal")));
    }

    #[test]
    fn project_fact_without_path_not_flagged_as_orphan() {
        // ProjectFact without path should NOT be flagged as orphan
        // (only TaskState and Decision are flagged)
        let memory = make_memory(
            "fact-no-path",
            MemoryScope::Project,
            MemoryCategory::ProjectFact,
            MemoryStatus::Accepted,
            "项目事实",
            "这个项目使用 pnpm。",
            None,
        );

        let findings = scan_memory_hygiene(&[memory]);

        assert!(findings.is_empty());
    }

    #[test]
    fn multiple_suspicious_memories_detected() {
        let memories = vec![
            make_memory(
                "tomato",
                MemoryScope::UserProfile,
                MemoryCategory::Preference,
                MemoryStatus::Accepted,
                "用户偏好",
                "我想做一个番茄钟小工具。",
                None,
            ),
            make_memory(
                "orphan",
                MemoryScope::Project,
                MemoryCategory::TaskState,
                MemoryStatus::Accepted,
                "当前进度",
                "第一步完成。",
                None,
            ),
            make_memory(
                "valid",
                MemoryScope::Project,
                MemoryCategory::TaskState,
                MemoryStatus::Accepted,
                "当前进度",
                "第一步完成。",
                Some("/tmp/forge"),
            ),
        ];

        let findings = scan_memory_hygiene(&memories);

        assert_eq!(findings.len(), 2);
        assert!(findings.iter().any(|f| f.memory_id == "tomato"));
        assert!(findings.iter().any(|f| f.memory_id == "orphan"));
        assert!(!findings.iter().any(|f| f.memory_id == "valid"));
    }
}
