use std::collections::HashSet;

use crate::memory::model::{MemoryCategory, MemoryStatus, SelectedContextMemory, WikiMemory};

pub fn select_relevant_memories(
    memories: &[WikiMemory],
    message: &str,
    project_path: Option<&str>,
    limit: usize,
) -> Vec<SelectedContextMemory> {
    if limit == 0 {
        return Vec::new();
    }
    if is_low_signal_memory_query(message) {
        return Vec::new();
    }

    let message_terms = terms(message);
    let message_lower = message.to_lowercase();

    let mut scored = memories
        .iter()
        .filter(|memory| {
            memory.status != MemoryStatus::Forgotten && memory.status != MemoryStatus::Archived
        })
        .filter_map(|memory| {
            let mut score = 0.0_f32;
            let mut reasons = Vec::new();
            let mut has_relevance_signal = false;

            if memory.status == MemoryStatus::Candidate {
                if memory.confidence < 0.55 {
                    return None;
                }
                score -= 0.75;
                reasons.push("自动记录");
            }

            if memory.status == MemoryStatus::Pinned {
                score += 4.0;
                reasons.push("已固定");
                has_relevance_signal = true;
            }

            if let (Some(active), Some(memory_project)) =
                (project_path, memory.project_path.as_deref())
            {
                if normalize_path(active) == normalize_path(memory_project) {
                    score += 3.0;
                    reasons.push("同一项目");
                }
            }

            let memory_text = format!("{} {} {}", memory.title, memory.body, memory.tags.join(" "));
            let memory_terms = terms(&memory_text);
            let overlap = message_terms
                .iter()
                .filter(|term| memory_terms.contains(term))
                .count();
            if overlap > 0 {
                score += overlap as f32;
                reasons.push("关键词匹配");
                has_relevance_signal = true;
            }

            if matches!(memory.category, MemoryCategory::Preference)
                && contains_preference_signal(&message_lower)
            {
                score += 1.5;
                reasons.push("偏好相关");
                has_relevance_signal = true;
            }

            if matches!(memory.category, MemoryCategory::Decision)
                && contains_direction_signal(&message_lower)
            {
                score += 1.5;
                reasons.push("方向相关");
                has_relevance_signal = true;
            }

            if matches!(memory.category, MemoryCategory::TaskState)
                && contains_progress_signal(&message_lower)
            {
                score += 2.0;
                reasons.push("进度相关");
                has_relevance_signal = true;
            }

            if matches!(memory.category, MemoryCategory::ProjectFact)
                && contains_project_signal(&message_lower)
            {
                score += 1.5;
                reasons.push("项目相关");
                has_relevance_signal = true;
            }

            if !has_relevance_signal {
                return None;
            }

            if score > 0.0 && memory.use_count > 0 {
                score += (memory.use_count.min(3) as f32) * 0.4;
                reasons.push("曾被使用");
            }

            if score <= 0.0 {
                return None;
            }

            Some(SelectedContextMemory {
                memory_id: memory.id.clone(),
                title: memory.title.clone(),
                body: memory.body.clone(),
                category: memory.category.clone(),
                scope: memory.scope.clone(),
                score,
                reason: reasons.join("、"),
                injected: true,
            })
        })
        .collect::<Vec<_>>();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.title.cmp(&b.title))
    });
    scored.truncate(limit);
    scored
}

fn contains_preference_signal(message: &str) -> bool {
    ["偏好", "习惯", "还是", "不用", "以后", "按我"]
        .iter()
        .any(|signal| message.contains(signal))
}

fn contains_direction_signal(message: &str) -> bool {
    ["方向", "方案", "之前", "继续", "按之前", "产品"]
        .iter()
        .any(|signal| message.contains(signal))
}

fn contains_progress_signal(message: &str) -> bool {
    [
        "之前",
        "上次",
        "继续",
        "接着",
        "做到哪",
        "进度",
        "完成了什么",
        "做了什么",
        "下一步",
        "resume",
        "continue",
    ]
    .iter()
    .any(|signal| message.contains(signal))
}

fn contains_project_signal(message: &str) -> bool {
    [
        "项目",
        "工作区",
        "目录",
        "路径",
        "只改",
        "不要动",
        "不要改",
        "workspace",
        "project",
    ]
    .iter()
    .any(|signal| message.contains(signal))
}

fn is_low_signal_memory_query(message: &str) -> bool {
    let message_lower = message.to_lowercase();
    if has_explicit_context_signal(&message_lower) {
        return false;
    }

    let compact = message_lower
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect::<String>();
    if compact.is_empty() {
        return true;
    }

    matches!(
        compact.as_str(),
        "继续"
            | "继续吧"
            | "继续呀"
            | "接着"
            | "接着吧"
            | "往下"
            | "往下走"
            | "下一步"
            | "下一步呢"
            | "接下来"
            | "接下来呢"
            | "然后呢"
            | "ok"
            | "okay"
            | "好"
            | "好的"
            | "可以"
            | "可以的"
            | "行"
            | "嗯"
    )
}

fn has_explicit_context_signal(message: &str) -> bool {
    [
        "刚才",
        "上次",
        "之前",
        "前面",
        "前文",
        "历史",
        "背景",
        "记忆",
        "项目记录",
        "做到哪",
        "进度",
        "失败",
        "报错",
        "错误",
        "继续做",
        "继续改",
        "继续修",
        "接着做",
        "接着改",
        "接着修",
        "我们说",
        "说了什么",
        "resume",
        "history",
        "memory",
    ]
    .iter()
    .any(|signal| message.contains(signal))
}

fn terms(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();

    text.to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter_map(|term| {
            let term = term.trim().to_string();
            if term.len() < 2 || !seen.insert(term.clone()) {
                None
            } else {
                Some(term)
            }
        })
        .collect()
}

fn normalize_path(path: &str) -> String {
    path.trim().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::select_relevant_memories;
    use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus, WikiMemory};

    fn memory(id: &str, status: MemoryStatus, body: &str) -> WikiMemory {
        WikiMemory {
            id: id.to_string(),
            category: MemoryCategory::Decision,
            scope: MemoryScope::Project,
            status,
            title: "Forge 方向".to_string(),
            body: body.to_string(),
            project_path: Some("/tmp/forge".to_string()),
            source_session_id: Some("session-1".to_string()),
            source_message_ids: Vec::new(),
            confidence: 0.8,
            created_at: "1".to_string(),
            updated_at: "1".to_string(),
            last_used_at: None,
            use_count: 0,
            tags: vec!["forge".to_string(), "资料系统".to_string()],
        }
    }

    fn global_preference_memory(id: &str, body: &str, use_count: u32) -> WikiMemory {
        WikiMemory {
            id: id.to_string(),
            category: MemoryCategory::Preference,
            scope: MemoryScope::UserProfile,
            status: MemoryStatus::Accepted,
            title: "用户偏好".to_string(),
            body: body.to_string(),
            project_path: None,
            source_session_id: Some("session-1".to_string()),
            source_message_ids: Vec::new(),
            confidence: 0.8,
            created_at: "1".to_string(),
            updated_at: "1".to_string(),
            last_used_at: Some("1".to_string()),
            use_count,
            tags: vec!["preference".to_string()],
        }
    }

    fn task_state_memory(id: &str, status: MemoryStatus, body: &str) -> WikiMemory {
        WikiMemory {
            id: id.to_string(),
            category: MemoryCategory::TaskState,
            scope: MemoryScope::Project,
            status,
            title: "当前进度".to_string(),
            body: body.to_string(),
            project_path: Some("/tmp/forge".to_string()),
            source_session_id: Some("session-1".to_string()),
            source_message_ids: Vec::new(),
            confidence: 0.8,
            created_at: "1".to_string(),
            updated_at: "1".to_string(),
            last_used_at: None,
            use_count: 0,
            tags: vec!["task_state".to_string()],
        }
    }

    #[test]
    fn selects_pinned_same_project_memory() {
        let memories = vec![memory("m1", MemoryStatus::Pinned, "渐进式 Project Records")];

        let selected = select_relevant_memories(&memories, "继续做资料系统", Some("/tmp/forge"), 5);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].memory_id, "m1");
        assert!(selected[0].reason.contains("已固定"));
    }

    #[test]
    fn excludes_forgotten_and_archived_but_keeps_relevant_candidates() {
        let memories = vec![
            memory("forgotten", MemoryStatus::Forgotten, "资料系统"),
            memory("archived", MemoryStatus::Archived, "资料系统"),
            memory("candidate", MemoryStatus::Candidate, "资料系统"),
        ];

        let selected = select_relevant_memories(&memories, "继续做资料系统", Some("/tmp/forge"), 5);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].memory_id, "candidate");
        assert!(selected[0].reason.contains("自动记录"));
    }

    #[test]
    fn candidate_task_state_can_support_resume_question() {
        let memories = vec![task_state_memory(
            "progress",
            MemoryStatus::Candidate,
            "上次已经完成 demo 首页和检查失败后的继续修复入口。",
        )];

        let selected =
            select_relevant_memories(&memories, "我们之前做到哪了？", Some("/tmp/forge"), 5);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].memory_id, "progress");
        assert!(selected[0].reason.contains("自动记录"));
        assert!(selected[0].reason.contains("进度相关"));
    }

    #[test]
    fn low_signal_continuation_does_not_select_project_memories() {
        let memories = vec![
            task_state_memory(
                "progress",
                MemoryStatus::Accepted,
                "上次已经完成 demo 首页和检查失败后的继续修复入口。",
            ),
            memory(
                "direction",
                MemoryStatus::Accepted,
                "V1 先证明本地网页小工具闭环。",
            ),
        ];

        let selected = select_relevant_memories(&memories, "继续吧", Some("/tmp/forge"), 5);

        assert!(selected.is_empty());
    }

    #[test]
    fn same_project_alone_does_not_select_memory() {
        let memories = vec![memory(
            "direction",
            MemoryStatus::Accepted,
            "V1 先证明本地网页小工具闭环。",
        )];

        let selected =
            select_relevant_memories(&memories, "帮我看看按钮颜色", Some("/tmp/forge"), 5);

        assert!(selected.is_empty());
    }

    #[test]
    fn repeated_query_terms_do_not_inflate_score() {
        let memories = vec![memory("m1", MemoryStatus::Accepted, "forge")];

        let repeated = select_relevant_memories(&memories, "forge forge forge", None, 5);
        let single = select_relevant_memories(&memories, "forge", None, 5);

        assert_eq!(repeated.len(), 1);
        assert_eq!(single.len(), 1);
        assert_eq!(repeated[0].score, single[0].score);
    }

    #[test]
    fn use_count_alone_does_not_select_unrelated_global_memory() {
        let memories = vec![global_preference_memory(
            "old-demo",
            "我想做一个番茄钟小工具，可以开始、暂停、重置。",
            12,
        )];

        let selected = select_relevant_memories(&memories, "我们之前说了什么东西", None, 5);

        assert!(selected.is_empty());
    }

    #[test]
    fn use_count_still_boosts_relevant_memory() {
        let mut used = memory("used", MemoryStatus::Accepted, "资料系统");
        used.use_count = 3;
        let unused = memory("unused", MemoryStatus::Accepted, "资料系统");

        let selected = select_relevant_memories(&[unused, used], "继续做资料系统", None, 5);

        assert_eq!(selected[0].memory_id, "used");
        assert!(selected[0].reason.contains("曾被使用"));
    }
}
