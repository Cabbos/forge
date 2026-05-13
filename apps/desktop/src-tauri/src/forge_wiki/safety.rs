use std::fs;
use std::io::ErrorKind;
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

    let canonical_root = match fs::symlink_metadata(&root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err("Wiki directory cannot be a symlink".to_string());
            }
            let canonical_root = root
                .canonicalize()
                .map_err(|err| format!("Failed to resolve wiki directory: {err}"))?;
            Some(canonical_root)
        }
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => return Err(format!("Failed to inspect wiki directory: {err}")),
    };

    let mut current = root.clone();
    for component in relative.components() {
        match component {
            Component::Normal(part) => current.push(part),
            Component::CurDir => continue,
            _ => return Err("Wiki page path cannot leave the wiki directory".to_string()),
        }

        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err("Wiki page path cannot include symlinks".to_string());
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => break,
            Err(err) => return Err(format!("Failed to inspect wiki page path: {err}")),
        }
    }

    if let Some(canonical_root) = canonical_root {
        let nearest_existing = nearest_existing_ancestor(&resolved);
        let canonical_existing = nearest_existing
            .canonicalize()
            .map_err(|err| format!("Failed to resolve wiki page ancestor: {err}"))?;
        if !canonical_existing.starts_with(&canonical_root) {
            return Err("Wiki page path is outside the wiki directory".to_string());
        }
    }

    Ok(resolved)
}

fn nearest_existing_ancestor(path: &Path) -> &Path {
    let mut current = path;
    while !current.exists() {
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    current
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
