use crate::plugin_manager::{AgentTarget, PluginEntry, PluginStatus, PluginType};

pub fn all() -> Vec<PluginEntry> {
    vec![
        ext(
            "codex__linter",
            "Linter Plugin",
            "Code linting and style checking",
            "quality",
        ),
        ext(
            "codex__formatter",
            "Formatter Plugin",
            "Automatic code formatting",
            "quality",
        ),
        ext(
            "codex__test-runner",
            "Test Runner",
            "Run and manage test suites",
            "testing",
        ),
    ]
}

fn ext(id: &str, name: &str, description: &str, category: &str) -> PluginEntry {
    PluginEntry {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        plugin_type: PluginType::Extension,
        agent: AgentTarget::Codex,
        category: category.to_string(),
        status: PluginStatus::NotInstalled,
        config_schema: None,
        current_config: None,
        homepage: None,
        author: Some("Community".to_string()),
    }
}
