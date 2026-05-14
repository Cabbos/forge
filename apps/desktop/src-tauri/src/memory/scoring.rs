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

    let message_terms = terms(message);
    let message_lower = message.to_lowercase();

    let mut scored = memories
        .iter()
        .filter(|memory| {
            memory.status != MemoryStatus::Forgotten
                && memory.status != MemoryStatus::Archived
                && memory.status != MemoryStatus::Candidate
        })
        .filter_map(|memory| {
            let mut score = 0.0_f32;
            let mut reasons = Vec::new();

            if memory.status == MemoryStatus::Pinned {
                score += 4.0;
                reasons.push("已固定");
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
            }

            if matches!(memory.category, MemoryCategory::Preference)
                && contains_preference_signal(&message_lower)
            {
                score += 1.5;
                reasons.push("偏好相关");
            }

            if matches!(memory.category, MemoryCategory::Decision)
                && contains_direction_signal(&message_lower)
            {
                score += 1.5;
                reasons.push("方向相关");
            }

            if memory.use_count > 0 {
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

    #[test]
    fn selects_pinned_same_project_memory() {
        let memories = vec![memory("m1", MemoryStatus::Pinned, "渐进式 Project Records")];

        let selected = select_relevant_memories(&memories, "继续做资料系统", Some("/tmp/forge"), 5);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].memory_id, "m1");
        assert!(selected[0].reason.contains("已固定"));
    }

    #[test]
    fn excludes_forgotten_archived_and_candidates() {
        let memories = vec![
            memory("forgotten", MemoryStatus::Forgotten, "资料系统"),
            memory("archived", MemoryStatus::Archived, "资料系统"),
            memory("candidate", MemoryStatus::Candidate, "资料系统"),
        ];

        let selected = select_relevant_memories(&memories, "继续做资料系统", Some("/tmp/forge"), 5);

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
}
