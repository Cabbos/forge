use std::fs;
use std::path::{Path, PathBuf};

use crate::workspace_safety::resolve_workspace_path;

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
    let metadata =
        serde_json::from_str(&json).map_err(|e| format!("App metadata is corrupted: {e}"))?;
    Ok(normalize_app_metadata(metadata))
}

fn save_app_metadata_at(root: &Path, metadata: &AppMetadata) -> Result<(), String> {
    let path = metadata_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create app metadata dir: {e}"))?;
    }
    let metadata = normalize_app_metadata(metadata.clone());
    let json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| format!("Failed to serialize app metadata: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write app metadata: {e}"))?;
    Ok(())
}

fn normalize_app_metadata(metadata: AppMetadata) -> AppMetadata {
    let mut seen_paths = std::collections::HashSet::new();
    let mut workspaces = Vec::new();

    for workspace in metadata.workspaces {
        let Ok(canonical_path) = resolve_workspace_path(&workspace.path) else {
            continue;
        };
        let path = canonical_path.to_string_lossy().to_string();
        if !seen_paths.insert(path.clone()) {
            continue;
        }
        let name = workspace.name.trim();
        let name = if name.is_empty() {
            workspace_name_from_path(&canonical_path)
        } else {
            workspace.name
        };
        workspaces.push(AppWorkspace {
            id: path.clone(),
            name,
            path,
            last_opened_at: workspace.last_opened_at,
        });
    }

    let active_workspace_id = metadata
        .active_workspace_id
        .as_deref()
        .and_then(|active| resolve_workspace_path(active).ok())
        .map(|path| path.to_string_lossy().to_string())
        .filter(|path| seen_paths.contains(path));

    AppMetadata {
        workspaces,
        active_workspace_id,
        active_session_id: metadata.active_session_id,
        selected_provider: metadata.selected_provider,
        selected_model: metadata.selected_model,
    }
}

fn workspace_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("当前项目")
        .to_string()
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
        let workspace = temp_root("workspace-roundtrip");
        fs::create_dir_all(&workspace).expect("workspace dir");
        let workspace = workspace.canonicalize().expect("canonical workspace");
        let workspace_path = workspace.to_string_lossy().to_string();
        let metadata = AppMetadata {
            workspaces: vec![AppWorkspace {
                id: workspace_path.clone(),
                name: "demo".to_string(),
                path: workspace_path.clone(),
                last_opened_at: 42,
            }],
            active_workspace_id: Some(workspace_path.clone()),
            active_session_id: Some("session-1".to_string()),
            selected_provider: Some("deepseek".to_string()),
            selected_model: Some("deepseek-chat".to_string()),
        };

        save_app_metadata_at(&root, &metadata).expect("save metadata");
        let restored = load_app_metadata_at(&root).expect("load metadata");

        assert_eq!(
            restored.active_workspace_id.as_deref(),
            Some(workspace_path.as_str())
        );
        assert_eq!(restored.active_session_id.as_deref(), Some("session-1"));
        assert_eq!(restored.workspaces[0].last_opened_at, 42);
        assert_eq!(restored.selected_provider.as_deref(), Some("deepseek"));
        assert_eq!(restored.selected_model.as_deref(), Some("deepseek-chat"));

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn missing_metadata_loads_empty_state() {
        let root = temp_root("missing");

        let restored = load_app_metadata_at(&root).expect("load missing metadata");

        assert!(restored.workspaces.is_empty());
        assert!(restored.active_workspace_id.is_none());
        assert!(restored.active_session_id.is_none());
    }

    #[test]
    fn app_metadata_load_drops_broad_workspace_roots() {
        let root = temp_root("broad");
        let path = metadata_path(&root);
        fs::create_dir_all(path.parent().expect("metadata parent")).expect("metadata dir");
        fs::write(
            &path,
            serde_json::json!({
                "workspaces": [{
                    "id": "/",
                    "name": "Root",
                    "path": "/",
                    "lastOpenedAt": 42
                }],
                "activeWorkspaceId": "/",
                "activeSessionId": "session-1",
                "selectedProvider": "deepseek",
                "selectedModel": "deepseek-chat"
            })
            .to_string(),
        )
        .expect("write metadata");

        let restored = load_app_metadata_at(&root).expect("load metadata");

        assert!(restored.workspaces.is_empty());
        assert_eq!(restored.active_workspace_id, None);
        assert_eq!(restored.active_session_id.as_deref(), Some("session-1"));
        assert_eq!(restored.selected_provider.as_deref(), Some("deepseek"));
        assert_eq!(restored.selected_model.as_deref(), Some("deepseek-chat"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn app_metadata_save_canonicalizes_workspace_paths() {
        let root = temp_root("canonical");
        let workspace = temp_root("workspace");
        fs::create_dir_all(&workspace).expect("workspace dir");
        let input_path = workspace.join(".").to_string_lossy().to_string();
        let metadata = AppMetadata {
            workspaces: vec![AppWorkspace {
                id: input_path.clone(),
                name: "demo".to_string(),
                path: input_path.clone(),
                last_opened_at: 42,
            }],
            active_workspace_id: Some(input_path),
            active_session_id: None,
            selected_provider: None,
            selected_model: None,
        };

        save_app_metadata_at(&root, &metadata).expect("save metadata");
        let restored = load_app_metadata_at(&root).expect("load metadata");
        let canonical = workspace.canonicalize().expect("canonical workspace");
        let canonical = canonical.to_string_lossy().to_string();

        assert_eq!(restored.workspaces.len(), 1);
        assert_eq!(restored.workspaces[0].id, canonical);
        assert_eq!(restored.workspaces[0].path, canonical);
        assert_eq!(
            restored.active_workspace_id.as_deref(),
            Some(canonical.as_str())
        );

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(workspace);
    }
}
