use std::sync::Arc;
use crate::state::AppState;
use serde::Serialize;

#[derive(Serialize)]
pub struct CapabilityInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub source: String,
    pub version: String,
    pub enabled: bool,
}

#[tauri::command]
pub async fn list_capabilities(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<CapabilityInfo>, String> {
    let caps = state.harness.capability_registry.all();
    Ok(caps.iter().map(|m| CapabilityInfo {
        id: m.id.clone(),
        name: m.name.clone(),
        description: m.description.clone(),
        kind: format!("{:?}", m.kind).to_lowercase(),
        source: m.source.clone(),
        version: m.version.clone(),
        enabled: true,
    }).collect())
}

#[tauri::command]
pub async fn toggle_capability(
    state: tauri::State<'_, Arc<AppState>>,
    capability_id: String,
    enabled: bool,
) -> Result<(), String> {
    state.harness.capability_registry.toggle(&capability_id, enabled)
}
