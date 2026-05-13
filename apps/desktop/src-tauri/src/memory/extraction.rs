use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus, WikiMemory};
use crate::memory::risk::should_reject_persistent_memory;
use crate::memory::storage::now_string;

pub fn extract_candidates_from_user_message(
    session_id: &str,
    project_path: Option<&str>,
    text: &str,
) -> Vec<WikiMemory> {
    let body = collapse_whitespace(text);
    if body.chars().count() < 8 || should_reject_persistent_memory(&body) {
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
        ],
    ) {
        candidates.push(candidate(
            session_id,
            None,
            MemoryCategory::Preference,
            MemoryScope::UserProfile,
            &candidate_title("用户偏好", &body),
            &body,
            0.72,
            "preference",
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
    fn rejects_secret_like_content() {
        let candidates = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/project"),
            "以后默认 token: abcdefghijklmnop",
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
}
