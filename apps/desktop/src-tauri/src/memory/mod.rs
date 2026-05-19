pub mod extraction;
pub mod model;
pub mod risk;
pub mod scoring;
pub mod storage;

pub use extraction::extract_candidates_from_user_message;
pub use model::{
    MemoryCategory, MemoryListFilter, MemoryPatch, MemoryScope, SelectedContextMemory, WikiMemory,
};
pub use storage::WikiMemoryStore;

pub fn format_selected_memory_context(selected: &[SelectedContextMemory]) -> Option<String> {
    if selected.is_empty() {
        return None;
    }

    let mut lines = Vec::with_capacity(selected.len() + 2);
    lines.push("## Work Continuity and Project Background".to_string());
    lines.push("Use these notes to continue previous work when relevant. Combine them with the retained visible conversation, and clearly say when older details are unavailable; do not expose memory, retrieval, or context-engineering internals to the user.".to_string());
    for memory in selected {
        lines.push(format!(
            "- [{}] title={} body={}",
            memory_category_label(&memory.category),
            memory_data_text(&memory.title),
            memory_data_text(&memory.body)
        ));
    }
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::format_selected_memory_context;
    use crate::memory::model::{MemoryCategory, MemoryScope, SelectedContextMemory};

    #[test]
    fn formatted_context_guides_resume_without_exposing_memory_internals() {
        let selected = vec![SelectedContextMemory {
            memory_id: "progress".to_string(),
            title: "当前进度".to_string(),
            body: "上次已经完成 demo 首页，下一步修复检查失败。".to_string(),
            category: MemoryCategory::TaskState,
            scope: MemoryScope::Project,
            score: 3.0,
            reason: "进度相关".to_string(),
            injected: true,
        }];

        let context = format_selected_memory_context(&selected).expect("context");

        assert!(context.contains("continue previous work"));
        assert!(context.contains("do not expose memory"));
        assert!(context.contains("[task_state]"));
        assert!(context.contains("上次已经完成 demo 首页"));
    }
}

fn memory_data_text(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    serde_json::to_string(&normalized).unwrap_or_else(|_| "\"\"".to_string())
}

fn memory_category_label(category: &MemoryCategory) -> &'static str {
    match category {
        MemoryCategory::Preference => "preference",
        MemoryCategory::ProjectFact => "project_fact",
        MemoryCategory::Decision => "decision",
        MemoryCategory::TaskState => "task_state",
    }
}
