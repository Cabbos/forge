use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppMetadata {
    #[serde(default)]
    pub workspaces: Vec<AppWorkspace>,
    #[serde(default)]
    pub active_workspace_id: Option<String>,
    #[serde(default)]
    pub active_session_id: Option<String>,
    #[serde(default)]
    pub selected_provider: Option<String>,
    #[serde(default)]
    pub selected_model: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppWorkspace {
    pub id: String,
    pub name: String,
    pub path: String,
    pub last_opened_at: u64,
}

#[tauri::command]
pub fn load_app_metadata() -> Result<AppMetadata, String> {
    load_app_metadata_at(&app_data_dir())
}

#[tauri::command]
pub fn save_app_metadata(metadata: AppMetadata) -> Result<(), String> {
    save_app_metadata_at(&app_data_dir(), &metadata)
}

fn load_app_metadata_at(root: &Path) -> Result<AppMetadata, String> {
    let path = metadata_path(root);
    if !path.exists() {
        return Ok(AppMetadata::default());
    }
    let json =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read app metadata: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("App metadata is corrupted: {e}"))
}

fn save_app_metadata_at(root: &Path, metadata: &AppMetadata) -> Result<(), String> {
    let path = metadata_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create app metadata dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(metadata)
        .map_err(|e| format!("Failed to serialize app metadata: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write app metadata: {e}"))?;
    Ok(())
}

fn metadata_path(root: &Path) -> PathBuf {
    root.join("app-state.json")
}

fn app_data_dir() -> PathBuf {
    home_dir().join(".forge")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "forge-app-metadata-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    #[test]
    fn app_metadata_roundtrips_workspace_and_active_selection() {
        let root = temp_root("roundtrip");
        let metadata = AppMetadata {
            workspaces: vec![AppWorkspace {
                id: "/workspace/demo".to_string(),
                name: "demo".to_string(),
                path: "/workspace/demo".to_string(),
                last_opened_at: 42,
            }],
            active_workspace_id: Some("/workspace/demo".to_string()),
            active_session_id: Some("session-1".to_string()),
            selected_provider: Some("deepseek".to_string()),
            selected_model: Some("deepseek-chat".to_string()),
        };

        save_app_metadata_at(&root, &metadata).expect("save metadata");
        let restored = load_app_metadata_at(&root).expect("load metadata");

        assert_eq!(
            restored.active_workspace_id.as_deref(),
            Some("/workspace/demo")
        );
        assert_eq!(restored.active_session_id.as_deref(), Some("session-1"));
        assert_eq!(restored.workspaces[0].last_opened_at, 42);
        assert_eq!(restored.selected_provider.as_deref(), Some("deepseek"));
        assert_eq!(restored.selected_model.as_deref(), Some("deepseek-chat"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_metadata_loads_empty_state() {
        let root = temp_root("missing");

        let restored = load_app_metadata_at(&root).expect("load missing metadata");

        assert!(restored.workspaces.is_empty());
        assert!(restored.active_workspace_id.is_none());
        assert!(restored.active_session_id.is_none());
    }
}
