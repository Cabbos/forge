use super::presets;
use super::{AgentTarget, PluginEntry};

/// Registry of discoverable plugins (presets + optional external sources).
pub struct PluginRegistry;

impl PluginRegistry {
    /// List all available plugins that can be installed for a given agent.
    pub fn discover(agent: &AgentTarget) -> Vec<PluginEntry> {
        let installed_ids: Vec<String> = super::scanner::PluginScanner::scan(agent)
            .iter()
            .map(|e| e.id.clone())
            .collect();

        let mut available = match agent {
            AgentTarget::Claude => presets::claude::all(),
            AgentTarget::Codex => presets::codex::all(),
            AgentTarget::Hermes => presets::hermes::all(),
        };

        // Mark already-installed plugins
        for entry in &mut available {
            if installed_ids.contains(&entry.id) {
                // Already installed — merge status
                entry.status = super::PluginStatus::Installed { enabled: true };
            }
        }

        available
    }

    /// Get a specific preset plugin by ID.
    pub fn get_preset(id: &str) -> Option<PluginEntry> {
        presets::find_by_id(id)
    }
}
