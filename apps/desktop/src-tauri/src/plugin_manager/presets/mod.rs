pub mod claude;
pub mod codex;
pub mod hermes;

use super::PluginEntry;

/// Find a preset plugin by its ID across all agents.
pub fn find_by_id(id: &str) -> Option<PluginEntry> {
    claude::all()
        .into_iter()
        .chain(codex::all())
        .chain(hermes::all())
        .find(|e| e.id == id)
}
