use crate::plugin_manager::{AgentTarget, PluginEntry, PluginStatus, PluginType};

pub fn all() -> Vec<PluginEntry> {
    vec![
        ext(
            "hermes__knowledge-base",
            "Knowledge Base",
            "RAG-powered knowledge retrieval",
            "knowledge",
        ),
        ext(
            "hermes__web-search",
            "Web Search",
            "Search the web for real-time information",
            "search",
        ),
    ]
}

fn ext(id: &str, name: &str, description: &str, category: &str) -> PluginEntry {
    PluginEntry {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        plugin_type: PluginType::Extension,
        agent: AgentTarget::Hermes,
        category: category.to_string(),
        status: PluginStatus::NotInstalled,
        config_schema: None,
        current_config: None,
        homepage: None,
        author: Some("Community".to_string()),
    }
}
