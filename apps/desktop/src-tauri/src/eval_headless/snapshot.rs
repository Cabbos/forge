use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use super::types::{HeadlessFileDiff, SnapshotFile, WorkspaceSnapshot};

pub(crate) fn snapshot_workspace(root: &Path) -> Result<WorkspaceSnapshot, String> {
    if !root.is_dir() {
        return Err(format!(
            "Forge eval workspace does not exist or is not a directory: {}",
            root.display()
        ));
    }

    let mut snapshot = WorkspaceSnapshot::new();
    snapshot_dir(root, root, &mut snapshot).map_err(|error| {
        format!(
            "failed to snapshot Forge eval workspace {}: {error}",
            root.display()
        )
    })?;
    Ok(snapshot)
}

pub(crate) fn snapshot_dir(
    root: &Path,
    dir: &Path,
    snapshot: &mut WorkspaceSnapshot,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if file_type.is_dir() {
            if is_ignored_snapshot_dir(&name) {
                continue;
            }
            snapshot_dir(root, &path, snapshot)?;
            continue;
        }

        if !file_type.is_file() || is_ignored_snapshot_file(&name) {
            continue;
        }

        let relative_path = normalize_relative_path(root, &path)?;
        let contents = fs::read(&path)?;
        snapshot.insert(relative_path, SnapshotFile { contents });
    }
    Ok(())
}

pub(crate) fn normalize_relative_path(root: &Path, path: &Path) -> io::Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/"))
}

pub(crate) fn is_ignored_snapshot_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | ".forge" | "node_modules" | "target" | ".venv" | "__pycache__" | ".pytest_cache"
    )
}

pub(crate) fn is_ignored_snapshot_file(name: &str) -> bool {
    matches!(name, ".DS_Store")
}

pub(crate) fn is_ignored_snapshot_path(path: &str) -> bool {
    path == ".forge" || path.starts_with(".forge/")
}

pub(crate) fn diff_workspace_snapshots(
    before: &WorkspaceSnapshot,
    after: &WorkspaceSnapshot,
) -> (Vec<String>, Vec<HeadlessFileDiff>) {
    let paths = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut changed_files = Vec::new();
    let mut file_diffs = Vec::new();

    for path in paths {
        if is_ignored_snapshot_path(&path) {
            continue;
        }
        let change_type = match (before.get(&path), after.get(&path)) {
            (None, Some(_)) => Some("added"),
            (Some(_), None) => Some("deleted"),
            (Some(before), Some(after)) if before != after => Some("modified"),
            _ => None,
        };
        if let Some(change_type) = change_type {
            changed_files.push(path.clone());
            file_diffs.push(HeadlessFileDiff {
                path: path.clone(),
                change_type: change_type.to_string(),
                diff: format!("workspace snapshot detected {change_type}: {path}"),
            });
        }
    }

    (changed_files, file_diffs)
}
