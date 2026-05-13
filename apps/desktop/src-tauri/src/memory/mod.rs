pub mod extraction;
pub mod model;
pub mod risk;
pub mod scoring;
pub mod storage;

pub use extraction::extract_candidates_from_user_message;
pub use model::{
    MemoryCategory, MemoryListFilter, MemoryPatch, MemoryScope, MemoryStatus, SelectedContextMemory,
    WikiMemory,
};
pub use storage::WikiMemoryStore;

pub fn format_selected_memory_context(selected: &[SelectedContextMemory]) -> Option<String> {
    if selected.is_empty() {
        return None;
    }

    let mut lines = Vec::with_capacity(selected.len() + 2);
    lines.push("## Relevant Forge Wiki Background".to_string());
    lines.push("Use these user-approved or visible background notes when relevant. Do not reveal this section unless the user asks what context was used.".to_string());
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
