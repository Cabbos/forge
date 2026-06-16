#[cfg(test)]
mod tests {
    use super::super::scoring::{select_relevant_memories, select_relevant_memories_with_audit};
    use crate::memory::model::{
        MemoryCategory, MemoryScope, MemoryStatus, RejectReason, WikiMemory,
    };

    fn make_memory(
        id: &str,
        status: MemoryStatus,
        category: MemoryCategory,
        scope: MemoryScope,
        title: &str,
        body: &str,
        project_path: Option<&str>,
    ) -> WikiMemory {
        WikiMemory {
            id: id.to_string(),
            category,
            scope,
            status,
            title: title.to_string(),
            body: body.to_string(),
            project_path: project_path.map(|s| s.to_string()),
            source_session_id: Some("session-1".to_string()),
            source_message_ids: vec![],
            confidence: 0.8,
            created_at: "1".to_string(),
            updated_at: "1".to_string(),
            last_used_at: None,
            use_count: 0,
            tags: vec![],
        }
    }

    #[test]
    fn limit_zero_returns_empty() {
        let memories = vec![make_memory(
            "m1",
            MemoryStatus::Accepted,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body",
            Some("/tmp/forge"),
        )];
        let selected = select_relevant_memories(&memories, "query", Some("/tmp/forge"), 0);
        assert!(selected.is_empty());
    }

    #[test]
    fn low_signal_query_rejects_all() {
        let memories = vec![make_memory(
            "m1",
            MemoryStatus::Accepted,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body",
            Some("/tmp/forge"),
        )];
        for message in ["继续", "好的", "ok", "嗯"] {
            let selected = select_relevant_memories(&memories, message, Some("/tmp/forge"), 5);
            assert!(
                selected.is_empty(),
                "low signal '{message}' should not select any memory"
            );
        }
    }

    #[test]
    fn explicit_context_signal_prevents_low_signal_rejection() {
        let memories = vec![make_memory(
            "m1",
            MemoryStatus::Accepted,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "修复步骤",
            "修复这个项目的具体步骤",
            Some("/tmp/forge"),
        )];
        // "继续修" matches has_explicit_context_signal → low-signal gate passes.
        // "项目" triggers contains_project_signal → relevance signal met.
        let selected =
            select_relevant_memories(&memories, "继续修这个项目bug", Some("/tmp/forge"), 5);
        assert!(
            !selected.is_empty(),
            "explicit context signal should prevent low-signal rejection, but got empty"
        );
    }

    #[test]
    fn forgotten_memory_is_always_rejected() {
        let memories = vec![make_memory(
            "m1",
            MemoryStatus::Forgotten,
            MemoryCategory::Preference,
            MemoryScope::UserProfile,
            "lang",
            "以后用中文",
            None,
        )];
        let selected = select_relevant_memories(&memories, "以后用中文", None, 5);
        assert!(selected.is_empty(), "forgotten memory should be rejected");
    }

    #[test]
    fn archived_memory_is_always_rejected() {
        let memories = vec![make_memory(
            "m1",
            MemoryStatus::Archived,
            MemoryCategory::Preference,
            MemoryScope::UserProfile,
            "lang",
            "以后用中文",
            None,
        )];
        let selected = select_relevant_memories(&memories, "以后用中文", None, 5);
        assert!(selected.is_empty(), "archived memory should be rejected");
    }

    #[test]
    fn candidate_with_low_confidence_is_rejected() {
        let mut memory = make_memory(
            "m1",
            MemoryStatus::Candidate,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body",
            Some("/tmp/forge"),
        );
        memory.confidence = 0.4;
        let memories = vec![memory];
        let selected = select_relevant_memories(&memories, "query", Some("/tmp/forge"), 5);
        assert!(
            selected.is_empty(),
            "low confidence candidate should be rejected"
        );
    }

    #[test]
    fn candidate_with_high_confidence_is_selected_with_signal() {
        let mut memory = make_memory(
            "m1",
            MemoryStatus::Candidate,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body content",
            Some("/tmp/forge"),
        );
        memory.confidence = 0.8;
        let memories = vec![memory];
        let selected = select_relevant_memories(&memories, "content", Some("/tmp/forge"), 5);
        assert_eq!(
            selected.len(),
            1,
            "high confidence candidate should be selected"
        );
        assert!(selected[0].reason.contains("自动记录"));
    }

    #[test]
    fn pinned_memory_gets_high_score() {
        let memories = vec![make_memory(
            "m1",
            MemoryStatus::Pinned,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body",
            Some("/tmp/forge"),
        )];
        let selected = select_relevant_memories(&memories, "query", Some("/tmp/forge"), 5);
        assert_eq!(selected.len(), 1);
        assert!(
            selected[0].score > 3.0,
            "pinned memory should have high score"
        );
        assert!(selected[0].reason.contains("已固定"));
    }

    #[test]
    fn project_mismatch_rejects_memory() {
        let memories = vec![make_memory(
            "m1",
            MemoryStatus::Accepted,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body",
            Some("/tmp/other"),
        )];
        let selected = select_relevant_memories(&memories, "query", Some("/tmp/forge"), 5);
        assert!(selected.is_empty(), "project mismatch should reject memory");
    }

    #[test]
    fn same_project_boosts_score() {
        let mut memory = make_memory(
            "m1",
            MemoryStatus::Accepted,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body",
            Some("/tmp/forge"),
        );
        memory.use_count = 0;
        let memories = vec![memory];
        let selected = select_relevant_memories(&memories, "body", Some("/tmp/forge"), 5);
        assert_eq!(selected.len(), 1);
        assert!(selected[0].reason.contains("同一项目"));
    }

    #[test]
    fn use_count_boosts_relevant_memory() {
        let mut used = make_memory(
            "used",
            MemoryStatus::Accepted,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body",
            Some("/tmp/forge"),
        );
        used.use_count = 3;
        let mut unused = make_memory(
            "unused",
            MemoryStatus::Accepted,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body",
            Some("/tmp/forge"),
        );
        unused.use_count = 0;
        let memories = vec![unused, used];
        let selected = select_relevant_memories(&memories, "body", Some("/tmp/forge"), 5);
        assert_eq!(selected[0].memory_id, "used");
        assert!(selected[0].reason.contains("曾被使用"));
    }

    #[test]
    fn preference_category_boosts_with_preference_signal() {
        let memories = vec![make_memory(
            "m1",
            MemoryStatus::Accepted,
            MemoryCategory::Preference,
            MemoryScope::UserProfile,
            "lang",
            "以后用中文",
            None,
        )];
        let selected = select_relevant_memories(&memories, "以后用中文", None, 5);
        assert_eq!(selected.len(), 1);
        assert!(selected[0].reason.contains("偏好相关"));
    }

    #[test]
    fn decision_category_boosts_with_direction_signal() {
        let memories = vec![make_memory(
            "m1",
            MemoryStatus::Accepted,
            MemoryCategory::Decision,
            MemoryScope::Project,
            "direction",
            "使用方案A",
            Some("/tmp/forge"),
        )];
        let selected = select_relevant_memories(&memories, "方向", Some("/tmp/forge"), 5);
        assert_eq!(selected.len(), 1);
        assert!(selected[0].reason.contains("方向相关"));
    }

    #[test]
    fn task_state_boosts_with_progress_signal() {
        let memories = vec![make_memory(
            "m1",
            MemoryStatus::Accepted,
            MemoryCategory::TaskState,
            MemoryScope::Project,
            "progress",
            "完成首页",
            Some("/tmp/forge"),
        )];
        let selected = select_relevant_memories(&memories, "进度", Some("/tmp/forge"), 5);
        assert_eq!(selected.len(), 1);
        assert!(selected[0].reason.contains("进度相关"));
    }

    #[test]
    fn project_fact_boosts_with_project_signal() {
        let memories = vec![make_memory(
            "m1",
            MemoryStatus::Accepted,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "fact",
            "使用pnpm",
            Some("/tmp/forge"),
        )];
        let selected = select_relevant_memories(&memories, "项目", Some("/tmp/forge"), 5);
        assert_eq!(selected.len(), 1);
        assert!(selected[0].reason.contains("项目相关"));
    }

    #[test]
    fn no_relevance_signal_rejects_memory() {
        let memories = vec![make_memory(
            "m1",
            MemoryStatus::Accepted,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body",
            Some("/tmp/forge"),
        )];
        let selected =
            select_relevant_memories(&memories, "completely unrelated", Some("/tmp/forge"), 5);
        assert!(
            selected.is_empty(),
            "no relevance signal should reject memory"
        );
    }

    #[test]
    fn results_are_limited() {
        let mut memories = Vec::new();
        for i in 0..10 {
            memories.push(make_memory(
                &format!("m{i}"),
                MemoryStatus::Accepted,
                MemoryCategory::ProjectFact,
                MemoryScope::Project,
                &format!("title{i}"),
                "common body",
                Some("/tmp/forge"),
            ));
        }
        let selected = select_relevant_memories(&memories, "common body", Some("/tmp/forge"), 3);
        assert_eq!(selected.len(), 3, "should limit to 3 results");
    }

    #[test]
    fn results_are_sorted_by_score_descending() {
        let mut high = make_memory(
            "high",
            MemoryStatus::Pinned,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body",
            Some("/tmp/forge"),
        );
        high.use_count = 5;
        let mut low = make_memory(
            "low",
            MemoryStatus::Accepted,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body",
            Some("/tmp/forge"),
        );
        low.use_count = 0;
        let memories = vec![low, high];
        let selected = select_relevant_memories(&memories, "body", Some("/tmp/forge"), 5);
        assert_eq!(selected[0].memory_id, "high");
        assert!(selected[0].score > selected[1].score);
    }

    #[test]
    fn audit_returns_both_selected_and_rejected() {
        let memories = vec![
            make_memory(
                "forgotten",
                MemoryStatus::Forgotten,
                MemoryCategory::Preference,
                MemoryScope::UserProfile,
                "title",
                "body",
                None,
            ),
            make_memory(
                "selected",
                MemoryStatus::Accepted,
                MemoryCategory::ProjectFact,
                MemoryScope::Project,
                "title",
                "body",
                Some("/tmp/forge"),
            ),
        ];
        let audit = select_relevant_memories_with_audit(&memories, "body", Some("/tmp/forge"), 5);
        assert_eq!(audit.selected.len(), 1);
        assert_eq!(audit.selected[0].memory_id, "selected");
        assert_eq!(audit.rejected.len(), 1);
        assert_eq!(audit.rejected[0].memory_id, "forgotten");
        assert_eq!(audit.rejected[0].reason, RejectReason::Forgotten);
    }

    #[test]
    fn orphan_project_memory_is_rejected() {
        let memory = make_memory(
            "orphan",
            MemoryStatus::Accepted,
            MemoryCategory::TaskState,
            MemoryScope::Project,
            "progress",
            "继续修复",
            None, // No project_path
        );
        let memories = vec![memory];
        let audit =
            select_relevant_memories_with_audit(&memories, "继续修复", Some("/tmp/forge"), 5);
        assert!(audit.selected.is_empty());
        assert_eq!(audit.rejected.len(), 1);
        assert_eq!(audit.rejected[0].reason, RejectReason::OrphanProjectMemory);
    }

    #[test]
    fn project_fact_without_project_path_is_not_orphan() {
        let memory = make_memory(
            "fact",
            MemoryStatus::Accepted,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "fact",
            "使用pnpm",
            None, // No project_path
        );
        let memories = vec![memory];
        let audit = select_relevant_memories_with_audit(&memories, "项目", Some("/tmp/forge"), 5);
        assert!(
            !audit
                .rejected
                .iter()
                .any(|r| r.reason == RejectReason::OrphanProjectMemory),
            "ProjectFact should not be blocked by orphan gate"
        );
    }

    #[test]
    fn task_like_global_preference_is_rejected() {
        let memory = make_memory(
            "task-like",
            MemoryStatus::Accepted,
            MemoryCategory::Preference,
            MemoryScope::UserProfile,
            "pref",
            "我想做一个番茄钟小工具，请优先推进第一版。",
            None,
        );
        let memories = vec![memory];
        let selected = select_relevant_memories(&memories, "不用重新问我这个项目是什么", None, 5);
        assert!(
            selected.is_empty(),
            "task-like global preference should be rejected"
        );
    }

    #[test]
    fn score_below_threshold_rejects_memory() {
        let memory = make_memory(
            "m1",
            MemoryStatus::Accepted,
            MemoryCategory::ProjectFact,
            MemoryScope::Project,
            "title",
            "body",
            Some("/tmp/forge"),
        );
        let memories = vec![memory];
        // Query with no matching keywords and no category signal
        let audit = select_relevant_memories_with_audit(&memories, "zzz", Some("/tmp/forge"), 5);
        assert!(audit.selected.is_empty());
        assert!(audit
            .rejected
            .iter()
            .any(|r| r.reason == RejectReason::NoRelevanceSignal));
    }
}
