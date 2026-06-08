use std::collections::HashSet;

use crate::memory::model::{
    MemoryCategory, MemorySelectionAudit, MemoryStatus, RejectReason, RejectedMemory,
    SelectedContextMemory, WikiMemory,
};

/// Original API — returns only selected memories, discards audit trail.
pub fn select_relevant_memories(
    memories: &[WikiMemory],
    message: &str,
    project_path: Option<&str>,
    limit: usize,
) -> Vec<SelectedContextMemory> {
    select_relevant_memories_with_audit(memories, message, project_path, limit).selected
}

/// Returns both selected and rejected memories with reasons.
pub fn select_relevant_memories_with_audit(
    memories: &[WikiMemory],
    message: &str,
    project_path: Option<&str>,
    limit: usize,
) -> MemorySelectionAudit {
    if limit == 0 {
        return MemorySelectionAudit {
            selected: Vec::new(),
            rejected: Vec::new(),
        };
    }
    if is_low_signal_memory_query(message) {
        let rejected = memories
            .iter()
            .filter(|m| m.status != MemoryStatus::Forgotten && m.status != MemoryStatus::Archived)
            .map(|m| reject(m, RejectReason::LowSignalQuery))
            .collect();
        return MemorySelectionAudit {
            selected: Vec::new(),
            rejected,
        };
    }

    let message_terms = terms(message);
    let message_lower = message.to_lowercase();

    let mut selected = Vec::new();
    let mut rejected = Vec::new();

    for memory in memories {
        // Gate 1: Forgotten / Archived
        if memory.status == MemoryStatus::Forgotten {
            rejected.push(reject(memory, RejectReason::Forgotten));
            continue;
        }
        if memory.status == MemoryStatus::Archived {
            rejected.push(reject(memory, RejectReason::Archived));
            continue;
        }

        // Gate 2: Project mismatch
        if let (Some(active_project), Some(memory_project)) =
            (project_path, memory.project_path.as_deref())
        {
            if normalize_path(active_project) != normalize_path(memory_project) {
                rejected.push(reject(memory, RejectReason::ProjectMismatch));
                continue;
            }
        }

        // Gate 3: Orphan project memory (Project scope + TaskState/Decision + no project_path)
        if memory.scope == crate::memory::model::MemoryScope::Project
            && memory.project_path.is_none()
            && matches!(
                memory.category,
                MemoryCategory::TaskState | MemoryCategory::Decision
            )
        {
            rejected.push(reject(memory, RejectReason::OrphanProjectMemory));
            continue;
        }

        let mut score = 0.0_f32;
        let mut reasons = Vec::new();
        let mut has_relevance_signal = false;

        if memory.status == MemoryStatus::Candidate {
            if memory.confidence < 0.55 {
                rejected.push(reject(memory, RejectReason::LowConfidenceCandidate));
                continue;
            }
            score -= 0.75;
            reasons.push("自动记录");
        }

        if memory.status == MemoryStatus::Pinned {
            score += 4.0;
            reasons.push("已固定");
            has_relevance_signal = true;
        }

        if let (Some(active), Some(memory_project)) = (project_path, memory.project_path.as_deref())
        {
            if normalize_path(active) == normalize_path(memory_project) {
                score += 3.0;
                reasons.push("同一项目");
            }
        }

        let memory_text = format!("{} {} {}", memory.title, memory.body, memory.tags.join(" "));
        if is_project_task_like_global_preference(memory, &memory_text) {
            rejected.push(reject(memory, RejectReason::TaskLikeGlobalPreference));
            continue;
        }
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
            rejected.push(reject(memory, RejectReason::NoRelevanceSignal));
            continue;
        }

        if score > 0.0 && memory.use_count > 0 {
            score += (memory.use_count.min(3) as f32) * 0.4;
            reasons.push("曾被使用");
        }

        if score <= 0.0 {
            rejected.push(reject(memory, RejectReason::ScoreBelowThreshold));
            continue;
        }

        selected.push(SelectedContextMemory {
            memory_id: memory.id.clone(),
            title: memory.title.clone(),
            body: memory.body.clone(),
            category: memory.category.clone(),
            scope: memory.scope.clone(),
            score,
            reason: reasons.join("、"),
            injected: true,
        });
    }

    selected.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.title.cmp(&b.title))
    });
    selected.truncate(limit);

    MemorySelectionAudit { selected, rejected }
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

fn is_project_task_like_global_preference(memory: &WikiMemory, memory_text: &str) -> bool {
    if !matches!(memory.category, MemoryCategory::Preference)
        || memory.project_path.is_some()
        || !matches!(memory.scope, crate::memory::model::MemoryScope::UserProfile)
    {
        return false;
    }

    let lower = memory_text.to_lowercase();
    [
        "我想做",
        "请先做",
        "请优先",
        "优先推进",
        "优先做",
        "先把",
        "当前项目",
        "这个项目",
        "项目档案",
        "当前进度",
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
    ]
    .iter()
    .any(|signal| lower.contains(signal))
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

fn reject(memory: &WikiMemory, reason: RejectReason) -> RejectedMemory {
    RejectedMemory {
        memory_id: memory.id.clone(),
        title: memory.title.clone(),
        scope: memory.scope.clone(),
        category: memory.category.clone(),
        project_path: memory.project_path.clone(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::{select_relevant_memories, select_relevant_memories_with_audit};
    use crate::memory::model::{
        MemoryCategory, MemoryScope, MemoryStatus, RejectReason, WikiMemory,
    };

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
    fn project_specific_memory_from_other_project_is_excluded_even_with_keyword_overlap() {
        let mut other_project = task_state_memory(
            "other-progress",
            MemoryStatus::Accepted,
            "番茄钟第一版已经完成，下一步优化交互。",
        );
        other_project.project_path = Some("/tmp/pomodoro".to_string());

        let selected = select_relevant_memories(
            &[other_project],
            "番茄钟之前做到哪了？",
            Some("/tmp/forge"),
            5,
        );

        assert!(selected.is_empty());
    }

    #[test]
    fn task_like_global_preference_is_not_injected_by_preference_signal() {
        let memories = vec![global_preference_memory(
            "old-pomodoro",
            "我想做一个番茄钟小工具，可以开始、暂停、重置。请优先推进到一个可预览的第一版。",
            18,
        )];

        let selected = select_relevant_memories(&memories, "不用重新问我这个项目是什么", None, 5);

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

    // ── Cross-project pollution regression tests ──────────────────────

    #[test]
    fn tomato_clock_global_preference_not_injected_in_different_project() {
        // The original incident: tomato clock memory as UserProfile,
        // user says "继续" in a different project context.
        let memories = vec![global_preference_memory(
            "tomato-clock",
            "我想做一个番茄钟小工具，可以开始、暂停、重置。请优先推进到一个可预览的第一版。",
            12,
        )];

        let selected = select_relevant_memories(&memories, "继续吧", Some("/tmp/forge-backend"), 5);

        assert!(
            selected.is_empty(),
            "tomato clock global preference must not leak into different project"
        );
    }

    #[test]
    fn tomato_clock_global_preference_not_injected_by_progress_signal() {
        let memories = vec![global_preference_memory(
            "tomato-clock",
            "我想做一个番茄钟小工具，优先推进第一版。",
            8,
        )];

        let selected = select_relevant_memories(
            &memories,
            "下一步做什么？",
            Some("/tmp/some-other-project"),
            5,
        );

        assert!(
            selected.is_empty(),
            "progress signal must not revive task-like global preference"
        );
    }

    #[test]
    fn tomato_clock_global_preference_not_injected_even_without_project_path() {
        let memories = vec![global_preference_memory(
            "tomato-clock",
            "我想做一个番茄钟小工具，请优先推进第一版。",
            18,
        )];

        // User says "验收一下" without any project context
        let selected = select_relevant_memories(&memories, "验收一下", None, 5);

        assert!(
            selected.is_empty(),
            "task-like global preference must not be injected even without project context"
        );
    }

    #[test]
    fn genuine_user_profile_preference_still_injected() {
        // "以后用中文" is a genuine long-term preference, NOT task-like
        let memories = vec![global_preference_memory(
            "lang-pref",
            "以后所有项目都默认用中文回复。",
            5,
        )];

        let selected =
            select_relevant_memories(&memories, "以后回复用中文", Some("/tmp/any-project"), 5);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].memory_id, "lang-pref");
    }

    #[test]
    fn same_project_keyword_overlap_isolation() {
        // Two projects with similar names but different paths
        let mut forge_memory = task_state_memory(
            "forge-progress",
            MemoryStatus::Accepted,
            "Forge 后端 agent core 测试已经补齐。",
        );
        forge_memory.project_path = Some("/tmp/forge-backend".to_string());

        let mut forge_frontend = task_state_memory(
            "forge-frontend-progress",
            MemoryStatus::Accepted,
            "Forge 前端 UI 重构进行中。",
        );
        forge_frontend.project_path = Some("/tmp/forge-frontend".to_string());

        let selected = select_relevant_memories(
            &[forge_memory, forge_frontend],
            "之前的进度做到哪了？",
            Some("/tmp/forge-frontend"),
            5,
        );

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].memory_id, "forge-frontend-progress");
    }

    #[test]
    fn low_signal_message_does_not_inject_old_project_goal() {
        let memories = vec![task_state_memory(
            "progress",
            MemoryStatus::Accepted,
            "番茄钟第一版已经完成，下一步优化交互。",
        )];

        for message in &["继续", "好的", "下一步", "ok", "嗯"] {
            let selected =
                select_relevant_memories(memories.as_slice(), message, Some("/tmp/forge"), 5);
            assert!(
                selected.is_empty(),
                "low signal '{message}' must not inject project memory from different project"
            );
        }
    }

    #[test]
    fn forgotten_memory_never_injected() {
        let mut forgotten = memory("forgotten-pref", MemoryStatus::Forgotten, "以后都用中文");
        forgotten.scope = MemoryScope::UserProfile;
        forgotten.category = MemoryCategory::Preference;
        forgotten.project_path = None;

        let selected = select_relevant_memories(&[forgotten], "以后回复用中文", None, 5);

        assert!(
            selected.is_empty(),
            "forgotten memory must never be injected"
        );
    }

    #[test]
    fn archived_memory_never_injected() {
        let mut archived = memory("archived-pref", MemoryStatus::Archived, "以后都用 Tailwind");
        archived.scope = MemoryScope::UserProfile;
        archived.category = MemoryCategory::Preference;
        archived.project_path = None;

        let selected = select_relevant_memories(&[archived], "以后用 Tailwind", None, 5);

        assert!(
            selected.is_empty(),
            "archived memory must never be injected"
        );
    }

    #[test]
    fn every_selected_memory_has_explainable_reason() {
        let memories = vec![
            memory("pinned", MemoryStatus::Pinned, "资料系统"),
            task_state_memory("progress", MemoryStatus::Accepted, "上次做到哪了？"),
        ];

        let selected = select_relevant_memories(&memories, "继续做资料系统", Some("/tmp/forge"), 5);

        for mem in &selected {
            assert!(
                !mem.reason.is_empty(),
                "every selected memory must have a non-empty reason, got empty for id={}",
                mem.memory_id,
            );
        }
    }

    // ── Audit trail tests ────────────────────────────────────────────

    #[test]
    fn audit_returns_rejected_with_task_like_global_preference_reason() {
        let memories = vec![global_preference_memory(
            "tomato-clock",
            "我想做一个番茄钟小工具，可以开始、暂停、重置。请优先推进到一个可预览的第一版。",
            12,
        )];

        let audit = select_relevant_memories_with_audit(
            &memories,
            "不用重新问我这个项目是什么",
            Some("/tmp/forge-backend"),
            5,
        );

        assert!(audit.selected.is_empty());
        assert_eq!(audit.rejected.len(), 1);
        assert_eq!(audit.rejected[0].memory_id, "tomato-clock");
        assert_eq!(
            audit.rejected[0].reason,
            RejectReason::TaskLikeGlobalPreference
        );
    }

    #[test]
    fn audit_returns_rejected_with_project_mismatch_reason() {
        let mut other = task_state_memory(
            "other-progress",
            MemoryStatus::Accepted,
            "番茄钟第一版已经完成。",
        );
        other.project_path = Some("/tmp/pomodoro".to_string());

        let audit = select_relevant_memories_with_audit(
            &[other],
            "番茄钟之前做到哪了？",
            Some("/tmp/forge"),
            5,
        );

        assert!(audit.selected.is_empty());
        assert_eq!(audit.rejected.len(), 1);
        assert_eq!(audit.rejected[0].reason, RejectReason::ProjectMismatch);
    }

    #[test]
    fn audit_returns_forgotten_reason() {
        let mut forgotten = memory("forgotten-pref", MemoryStatus::Forgotten, "以后都用中文");
        forgotten.scope = MemoryScope::UserProfile;
        forgotten.category = MemoryCategory::Preference;
        forgotten.project_path = None;

        let audit = select_relevant_memories_with_audit(&[forgotten], "以后回复用中文", None, 5);

        assert!(audit.selected.is_empty());
        assert_eq!(audit.rejected.len(), 1);
        assert_eq!(audit.rejected[0].reason, RejectReason::Forgotten);
    }

    #[test]
    fn audit_returns_archived_reason() {
        let mut archived = memory("archived-pref", MemoryStatus::Archived, "以后都用 Tailwind");
        archived.scope = MemoryScope::UserProfile;
        archived.category = MemoryCategory::Preference;
        archived.project_path = None;

        let audit = select_relevant_memories_with_audit(&[archived], "以后用 Tailwind", None, 5);

        assert!(audit.selected.is_empty());
        assert_eq!(audit.rejected.len(), 1);
        assert_eq!(audit.rejected[0].reason, RejectReason::Archived);
    }

    #[test]
    fn audit_returns_low_signal_query_reason() {
        let memories = vec![memory("m1", MemoryStatus::Accepted, "资料系统")];

        let audit = select_relevant_memories_with_audit(&memories, "继续", None, 5);

        assert!(audit.selected.is_empty());
        assert_eq!(audit.rejected.len(), 1);
        assert_eq!(audit.rejected[0].reason, RejectReason::LowSignalQuery);
    }

    #[test]
    fn audit_returns_no_relevance_signal_reason() {
        let memories = vec![memory("m1", MemoryStatus::Accepted, "资料系统")];

        let audit = select_relevant_memories_with_audit(
            &memories,
            "帮我看看按钮颜色",
            Some("/tmp/forge"),
            5,
        );

        assert!(audit.selected.is_empty());
        // Should have at least one rejected with NoRelevanceSignal
        assert!(audit
            .rejected
            .iter()
            .any(|r| r.reason == RejectReason::NoRelevanceSignal));
    }

    #[test]
    fn audit_selected_memories_still_have_reasons() {
        let memories = vec![memory("pinned", MemoryStatus::Pinned, "资料系统")];

        let audit =
            select_relevant_memories_with_audit(&memories, "继续做资料系统", Some("/tmp/forge"), 5);

        assert_eq!(audit.selected.len(), 1);
        assert_eq!(audit.selected[0].memory_id, "pinned");
        assert!(audit.selected[0].reason.contains("已固定"));
    }

    // ── Orphan project memory tests ──────────────────────────────────

    #[test]
    fn project_scoped_task_state_without_project_path_is_not_selected() {
        let mut orphan = WikiMemory {
            id: "orphan-task".to_string(),
            category: MemoryCategory::TaskState,
            scope: MemoryScope::Project,
            status: MemoryStatus::Accepted,
            title: "当前进度".to_string(),
            body: "第一步已经完成。".to_string(),
            project_path: None,
            source_session_id: Some("session-1".to_string()),
            source_message_ids: Vec::new(),
            confidence: 0.8,
            created_at: "1".to_string(),
            updated_at: "1".to_string(),
            last_used_at: None,
            use_count: 0,
            tags: vec!["task_state".to_string()],
        };
        // Ensure it would have been selected without the orphan check
        // by giving it matching keywords
        orphan.body = "第一步已经完成，接下来继续修复。".to_string();

        let selected = select_relevant_memories(&[orphan], "继续修复", Some("/tmp/forge"), 5);

        assert!(
            selected.is_empty(),
            "orphan TaskState without project_path must not be selected"
        );
    }

    #[test]
    fn project_scoped_decision_without_project_path_is_flagged_by_audit() {
        let orphan = WikiMemory {
            id: "orphan-decision".to_string(),
            category: MemoryCategory::Decision,
            scope: MemoryScope::Project,
            status: MemoryStatus::Accepted,
            title: "项目已定方案".to_string(),
            body: "产品方向定了，就用兼容旧方案。".to_string(),
            project_path: None,
            source_session_id: Some("session-1".to_string()),
            source_message_ids: Vec::new(),
            confidence: 0.8,
            created_at: "1".to_string(),
            updated_at: "1".to_string(),
            last_used_at: None,
            use_count: 0,
            tags: vec!["decision".to_string()],
        };

        let audit = select_relevant_memories_with_audit(
            &[orphan],
            "之前的方向方案是什么？",
            Some("/tmp/forge"),
            5,
        );

        assert!(audit.selected.is_empty());
        assert_eq!(audit.rejected.len(), 1);
        assert_eq!(audit.rejected[0].reason, RejectReason::OrphanProjectMemory);
    }

    #[test]
    fn valid_project_memory_with_matching_path_still_selected() {
        let valid = task_state_memory(
            "valid-task",
            MemoryStatus::Accepted,
            "第一步已经完成，接下来继续修复。",
        );

        let selected = select_relevant_memories(&[valid], "继续修复", Some("/tmp/forge"), 5);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].memory_id, "valid-task");
    }

    #[test]
    fn project_fact_without_project_path_is_not_blocked_by_orphan_gate() {
        // ProjectFact without project_path should NOT be blocked by orphan gate
        // because the orphan gate only applies to TaskState and Decision.
        let fact = WikiMemory {
            id: "fact-no-path".to_string(),
            category: MemoryCategory::ProjectFact,
            scope: MemoryScope::Project,
            status: MemoryStatus::Accepted,
            title: "项目事实".to_string(),
            body: "这个项目使用 pnpm。".to_string(),
            project_path: None,
            source_session_id: Some("session-1".to_string()),
            source_message_ids: Vec::new(),
            confidence: 0.8,
            created_at: "1".to_string(),
            updated_at: "1".to_string(),
            last_used_at: None,
            use_count: 0,
            tags: vec!["project_fact".to_string()],
        };

        let audit = select_relevant_memories_with_audit(
            &[fact],
            "这个项目的路径是什么？",
            Some("/tmp/forge"),
            5,
        );

        // Should NOT be rejected as OrphanProjectMemory
        assert!(
            !audit
                .rejected
                .iter()
                .any(|r| r.reason == RejectReason::OrphanProjectMemory),
            "ProjectFact should not be blocked by orphan gate"
        );
    }

    #[test]
    fn preference_without_project_path_is_not_blocked_by_orphan_gate() {
        let pref = global_preference_memory("lang-pref", "以后所有项目都默认用中文回复。", 5);

        let audit = select_relevant_memories_with_audit(
            &[pref],
            "以后回复用中文",
            Some("/tmp/any-project"),
            5,
        );

        // Should NOT be rejected as OrphanProjectMemory
        assert!(
            !audit
                .rejected
                .iter()
                .any(|r| r.reason == RejectReason::OrphanProjectMemory),
            "UserProfile Preference should not be blocked by orphan gate"
        );
    }
}
