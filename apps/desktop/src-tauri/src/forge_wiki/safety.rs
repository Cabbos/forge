use std::path::{Component, Path, PathBuf};

use crate::memory::risk::should_reject_persistent_memory;

pub fn wiki_dir(project_path: &str) -> PathBuf {
    Path::new(project_path).join(".forge").join("wiki")
}

pub fn resolve_wiki_page_path(project_path: &str, page_path: &str) -> Result<PathBuf, String> {
    let relative = Path::new(page_path);
    if page_path.trim().is_empty() {
        return Err("Wiki page path cannot be empty".to_string());
    }
    if relative.is_absolute() {
        return Err("Wiki page path must be relative".to_string());
    }
    if relative.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return Err("Wiki page path must be a Markdown file".to_string());
    }
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err("Wiki page path cannot leave the wiki directory".to_string());
    }

    let root = wiki_dir(project_path);
    let resolved = root.join(relative);

    if root.exists() && resolved.exists() {
        let canonical_root = root
            .canonicalize()
            .map_err(|err| format!("Failed to resolve wiki directory: {err}"))?;
        let canonical_resolved = resolved
            .canonicalize()
            .map_err(|err| format!("Failed to resolve wiki page: {err}"))?;
        if !canonical_resolved.starts_with(&canonical_root) {
            return Err("Wiki page path is outside the wiki directory".to_string());
        }
    }

    Ok(resolved)
}

pub fn should_ignore_project_entry(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            matches!(
                name,
                ".git"
                    | "node_modules"
                    | "dist"
                    | "build"
                    | "target"
                    | ".next"
                    | ".env"
                    | ".env.local"
            )
        })
        .unwrap_or(false)
}

pub fn contains_sensitive_wiki_content(text: &str) -> bool {
    should_reject_persistent_memory(text)
}
