use crate::plugin_manager::{AgentTarget, PluginEntry, PluginStatus, PluginType};

pub fn all() -> Vec<PluginEntry> {
    let mut v = Vec::new();
    v.extend(mcp_servers());
    v.extend(hooks());
    v.extend(skills());
    v
}

fn mcp_servers() -> Vec<PluginEntry> {
    vec![
        mcp(
            "mcp__filesystem",
            "Filesystem MCP",
            "Read, write, and manage files on the local filesystem",
            "filesystem",
        ),
        mcp(
            "mcp__github",
            "GitHub MCP",
            "Manage GitHub issues, PRs, and repositories",
            "git",
        ),
        mcp(
            "mcp__postgres",
            "PostgreSQL MCP",
            "Query and manage PostgreSQL databases",
            "database",
        ),
        mcp(
            "mcp__puppeteer",
            "Puppeteer MCP",
            "Browser automation and web scraping",
            "browser",
        ),
        mcp(
            "mcp__slack",
            "Slack MCP",
            "Send messages and manage Slack workspaces",
            "communication",
        ),
        mcp(
            "mcp__git",
            "Git MCP",
            "Git operations: commit, branch, diff, log",
            "git",
        ),
        mcp(
            "mcp__fetch",
            "Fetch MCP",
            "HTTP requests and web fetching",
            "network",
        ),
        mcp(
            "mcp__docker",
            "Docker MCP",
            "Manage Docker containers and images",
            "tools",
        ),
        mcp(
            "mcp__memory",
            "Memory MCP",
            "Persistent knowledge graph for AI memory",
            "tools",
        ),
        mcp(
            "mcp__sequential-thinking",
            "Sequential Thinking MCP",
            "Multi-step reasoning for complex problems",
            "reasoning",
        ),
    ]
}

fn hooks() -> Vec<PluginEntry> {
    vec![
        hook(
            "hook__pre-commit-review",
            "Pre-commit Review",
            "Run code review before each commit",
            "automation",
        ),
        hook(
            "hook__format-on-save",
            "Format on Save",
            "Auto-format code files on save",
            "automation",
        ),
    ]
}

fn skills() -> Vec<PluginEntry> {
    vec![
        skill("skill__pdf-reader", "PDF Reader", "Read and parse PDF documents"),
        skill("skill__excel-manager", "Excel Manager", "Create and edit Excel spreadsheets"),
        skill("skill__code-review", "Code Review", "Automated code review assistant"),
        skill("skill__api-tester", "API Tester", "Test and debug REST APIs"),
    ]
}

fn mcp(id: &str, name: &str, description: &str, category: &str) -> PluginEntry {
    PluginEntry {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        plugin_type: PluginType::McpServer,
        agent: AgentTarget::Claude,
        category: category.to_string(),
        status: PluginStatus::NotInstalled,
        config_schema: None,
        current_config: None,
        homepage: None,
        author: Some("Community".to_string()),
    }
}

fn hook(id: &str, name: &str, description: &str, category: &str) -> PluginEntry {
    PluginEntry {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        plugin_type: PluginType::Hook,
        agent: AgentTarget::Claude,
        category: category.to_string(),
        status: PluginStatus::NotInstalled,
        config_schema: None,
        current_config: None,
        homepage: None,
        author: Some("Community".to_string()),
    }
}

fn skill(id: &str, name: &str, description: &str) -> PluginEntry {
    PluginEntry {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        plugin_type: PluginType::Skill,
        agent: AgentTarget::Claude,
        category: "skill".to_string(),
        status: PluginStatus::NotInstalled,
        config_schema: None,
        current_config: None,
        homepage: None,
        author: Some("Community".to_string()),
    }
}
