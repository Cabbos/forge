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
        let change = match (before.get(&path), after.get(&path)) {
            (None, Some(after)) => Some((
                "added",
                build_unified_diff(&path, None, Some(&after.contents)),
            )),
            (Some(before), None) => Some((
                "deleted",
                build_unified_diff(&path, Some(&before.contents), None),
            )),
            (Some(before), Some(after)) if before != after => Some((
                "modified",
                build_unified_diff(&path, Some(&before.contents), Some(&after.contents)),
            )),
            _ => None,
        };
        if let Some((change_type, diff)) = change {
            changed_files.push(path.clone());
            file_diffs.push(HeadlessFileDiff {
                path: path.clone(),
                change_type: change_type.to_string(),
                diff,
            });
        }
    }

    (changed_files, file_diffs)
}

fn build_unified_diff(path: &str, before: Option<&[u8]>, after: Option<&[u8]>) -> String {
    let (Ok(before_text), Ok(after_text)) = (
        std::str::from_utf8(before.unwrap_or_default()),
        std::str::from_utf8(after.unwrap_or_default()),
    ) else {
        return format!("Binary files a/{path} and b/{path} differ\n");
    };
    let old_count = line_count(before_text);
    let new_count = line_count(after_text);
    let mut diff = format!("diff --git a/{path} b/{path}\n");
    match (before, after) {
        (None, Some(_)) => {
            diff.push_str("new file mode 100644\n");
            diff.push_str("--- /dev/null\n");
            diff.push_str(&format!("+++ b/{path}\n"));
        }
        (Some(_), None) => {
            diff.push_str("deleted file mode 100644\n");
            diff.push_str(&format!("--- a/{path}\n"));
            diff.push_str("+++ /dev/null\n");
        }
        (Some(_), Some(_)) => {
            diff.push_str(&format!("--- a/{path}\n"));
            diff.push_str(&format!("+++ b/{path}\n"));
        }
        (None, None) => return diff,
    }
    diff.push_str(&format!(
        "@@ {} {} @@\n",
        hunk_range('-', old_count),
        hunk_range('+', new_count)
    ));
    append_prefixed_contents(&mut diff, '-', before_text);
    append_prefixed_contents(&mut diff, '+', after_text);
    diff
}

fn line_count(contents: &str) -> usize {
    if contents.is_empty() {
        0
    } else {
        contents.bytes().filter(|byte| *byte == b'\n').count()
            + usize::from(!contents.ends_with('\n'))
    }
}

fn hunk_range(prefix: char, count: usize) -> String {
    match count {
        0 => format!("{prefix}0,0"),
        1 => format!("{prefix}1"),
        _ => format!("{prefix}1,{count}"),
    }
}

fn append_prefixed_contents(diff: &mut String, prefix: char, contents: &str) {
    for line in contents.split_inclusive('\n') {
        diff.push(prefix);
        diff.push_str(line);
    }
    if !contents.is_empty() && !contents.ends_with('\n') {
        diff.push('\n');
        diff.push_str("\\ No newline at end of file\n");
    }
}
