use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

pub(crate) const BROAD_WORKSPACE_REASON: &str = "请选择具体项目文件夹，不要直接使用用户主目录。";
pub(crate) const FORGE_SOURCE_WARNING: &str =
    "这是 Forge 自己的开发目录。继续操作可能修改 Forge 本体。";
pub(crate) const UNVERIFIED_WORKSPACE_WARNING: &str =
    "无法确认当前项目安全性，请确认项目文件夹后再继续。";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkspaceRisk {
    Normal,
    High,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceSafety {
    pub canonical_path: PathBuf,
    pub display_name: String,
    pub risk: WorkspaceRisk,
    pub warning: Option<String>,
    pub block_reason: Option<String>,
}

pub(crate) fn classify_workspace_path(path: &str) -> Result<WorkspaceSafety, String> {
    let requested = path.trim();
    let path = if requested.is_empty() {
        std::env::current_dir().map_err(|err| format!("无法读取当前项目文件夹：{err}"))?
    } else {
        PathBuf::from(requested)
    };

    if is_broad_workspace_path(&path) {
        return Ok(blocked_workspace(path));
    }

    let canonical_path = path
        .canonicalize()
        .map_err(|err| format!("无法打开项目文件夹：{err}"))?;
    if is_broad_workspace_path(&canonical_path) {
        return Ok(blocked_workspace(canonical_path));
    }
    if !canonical_path.is_dir() {
        return Err(format!(
            "这个路径不是项目文件夹：{}",
            canonical_path.display()
        ));
    }

    let display_name = workspace_display_name(&canonical_path);
    if is_forge_source_workspace_marker(&canonical_path) {
        return Ok(WorkspaceSafety {
            canonical_path,
            display_name,
            risk: WorkspaceRisk::High,
            warning: Some(FORGE_SOURCE_WARNING.to_string()),
            block_reason: None,
        });
    }

    Ok(WorkspaceSafety {
        canonical_path,
        display_name,
        risk: WorkspaceRisk::Normal,
        warning: None,
        block_reason: None,
    })
}

pub(crate) fn resolve_workspace_path(path: &str) -> Result<PathBuf, String> {
    let safety = classify_workspace_path(path)?;
    if safety.risk == WorkspaceRisk::Blocked {
        return Err(safety
            .block_reason
            .unwrap_or_else(|| BROAD_WORKSPACE_REASON.to_string()));
    }
    Ok(safety.canonical_path)
}

pub(crate) fn is_forge_source_workspace(path: &Path) -> bool {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    is_forge_source_workspace_marker(&canonical)
}

pub(crate) fn classify_existing_workspace_path(path: &Path) -> WorkspaceSafety {
    classify_workspace_path(path.to_string_lossy().as_ref()).unwrap_or_else(|_| WorkspaceSafety {
        canonical_path: path.to_path_buf(),
        display_name: workspace_display_name(path),
        risk: WorkspaceRisk::High,
        warning: Some(UNVERIFIED_WORKSPACE_WARNING.to_string()),
        block_reason: None,
    })
}

fn blocked_workspace(path: PathBuf) -> WorkspaceSafety {
    WorkspaceSafety {
        display_name: workspace_display_name(&path),
        canonical_path: path,
        risk: WorkspaceRisk::Blocked,
        warning: None,
        block_reason: Some(BROAD_WORKSPACE_REASON.to_string()),
    }
}

fn workspace_display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("当前项目")
        .to_string()
}

fn is_broad_workspace_path(path: &Path) -> bool {
    let components = path.components().collect::<Vec<_>>();
    if matches!(components.as_slice(), [Component::RootDir]) {
        return true;
    }
    if matches!(
        components.as_slice(),
        [Component::RootDir, Component::Normal(base)] if *base == "Users" || *base == "home"
    ) {
        return true;
    }
    matches!(
        components.as_slice(),
        [
            Component::RootDir,
            Component::Normal(base),
            Component::Normal(_user)
        ] if *base == "Users" || *base == "home"
    )
}

fn is_forge_source_workspace_marker(path: &Path) -> bool {
    if !path.join("src-tauri").is_dir() {
        return false;
    }

    let tauri_marker = tauri_config_mentions_forge(&path.join("src-tauri").join("tauri.conf.json"));
    let package_marker = package_json_name(path).as_deref() == Some("forge");
    let cargo_marker =
        cargo_package_name(&path.join("src-tauri").join("Cargo.toml")).as_deref() == Some("forge");

    tauri_marker && (package_marker || cargo_marker)
}

fn package_json_name(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path.join("package.json")).ok()?;
    serde_json::from_str::<Value>(&text)
        .ok()?
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn cargo_package_name(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package && line.starts_with("name") {
            return line
                .split_once('=')
                .map(|(_, value)| value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn tauri_config_mentions_forge(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    let product = value
        .get("productName")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let identifier = value
        .get("identifier")
        .and_then(Value::as_str)
        .unwrap_or_default();
    product == "Forge" || identifier.contains(".forge")
}

#[cfg(test)]
mod tests {
    use super::{
        classify_existing_workspace_path, classify_workspace_path, WorkspaceRisk,
        BROAD_WORKSPACE_REASON, UNVERIFIED_WORKSPACE_WARNING,
    };
    use std::path::Path;

    fn temp_workspace(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("forge-workspace-safety-{name}-{nanos}"));
        std::fs::create_dir_all(&path).expect("create temp workspace");
        path
    }

    #[test]
    fn normal_project_directory_is_allowed() {
        let workspace = temp_workspace("normal");

        let safety = classify_workspace_path(workspace.to_str().unwrap()).expect("classify");

        assert_eq!(safety.risk, WorkspaceRisk::Normal);
        assert_eq!(safety.block_reason, None);
        assert_eq!(
            safety.display_name,
            workspace.file_name().unwrap().to_string_lossy()
        );
        assert!(safety.canonical_path.is_absolute());
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn broad_workspace_roots_are_blocked() {
        for path in ["/", "/Users", "/Users/example", "/home", "/home/example"] {
            let safety = classify_workspace_path(path).expect("classify");

            assert_eq!(safety.risk, WorkspaceRisk::Blocked, "{path}");
            assert_eq!(safety.block_reason.as_deref(), Some(BROAD_WORKSPACE_REASON));
        }
    }

    #[test]
    fn forge_source_workspace_is_high_risk_from_project_markers() {
        let workspace = temp_workspace("forge-source");
        std::fs::create_dir_all(workspace.join("src-tauri")).expect("src-tauri");
        std::fs::write(workspace.join("package.json"), r#"{"name":"not-forge"}"#).expect("package");
        std::fs::write(
            workspace.join("src-tauri").join("Cargo.toml"),
            "[package]\nname = \"forge\"\nversion = \"0.1.0\"\n",
        )
        .expect("cargo");
        std::fs::write(
            workspace.join("src-tauri").join("tauri.conf.json"),
            r#"{"productName":"Forge","identifier":"com.cabbos.forge"}"#,
        )
        .expect("tauri config");

        let safety = classify_workspace_path(workspace.to_str().unwrap()).expect("classify");

        assert_eq!(safety.risk, WorkspaceRisk::High);
        assert_eq!(
            safety.warning.as_deref(),
            Some("这是 Forge 自己的开发目录。继续操作可能修改 Forge 本体。")
        );
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn forge_source_requires_tauri_config_marker() {
        let workspace = temp_workspace("forge-without-tauri-marker");
        std::fs::create_dir_all(workspace.join("src-tauri")).expect("src-tauri");
        std::fs::write(workspace.join("package.json"), r#"{"name":"forge"}"#).expect("package");
        std::fs::write(
            workspace.join("src-tauri").join("Cargo.toml"),
            "[package]\nname = \"forge\"\nversion = \"0.1.0\"\n",
        )
        .expect("cargo");

        let safety = classify_workspace_path(workspace.to_str().unwrap()).expect("classify");

        assert_eq!(safety.risk, WorkspaceRisk::Normal);
        assert_eq!(safety.warning, None);
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn existing_workspace_classification_failure_is_high_risk() {
        let missing = std::env::temp_dir().join("forge-workspace-safety-missing-project");
        let _ = std::fs::remove_dir_all(&missing);

        let safety = classify_existing_workspace_path(&missing);

        assert_eq!(safety.risk, WorkspaceRisk::High);
        assert_eq!(
            safety.warning.as_deref(),
            Some(UNVERIFIED_WORKSPACE_WARNING)
        );
    }

    #[test]
    fn canonicalized_broad_project_directory_is_blocked() {
        let home = std::env::var("HOME").expect("home");
        let safety = classify_workspace_path(&format!("{home}/.")).expect("classify");

        assert_eq!(safety.risk, WorkspaceRisk::Blocked);
        assert_eq!(safety.block_reason.as_deref(), Some(BROAD_WORKSPACE_REASON));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_to_user_home_is_blocked() {
        let workspace = temp_workspace("symlink-home");
        let link = workspace.join("home-link");
        std::os::unix::fs::symlink(std::env::var("HOME").expect("home"), &link)
            .expect("create symlink");

        let safety = classify_workspace_path(link.to_str().unwrap()).expect("classify");

        assert_eq!(safety.risk, WorkspaceRisk::Blocked);
        assert_eq!(safety.block_reason.as_deref(), Some(BROAD_WORKSPACE_REASON));
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn current_repository_is_detected_as_forge_source_when_classified() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root");

        let safety = classify_workspace_path(repo.to_str().unwrap()).expect("classify");

        assert_eq!(safety.risk, WorkspaceRisk::High);
    }
}
