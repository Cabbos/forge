use std::path::Path;

pub(crate) fn find_files(dir: &Path, query: &str, limit: usize) -> Vec<String> {
    let mut results = Vec::new();
    let lower_query = query.trim().to_lowercase();
    let mut visited = 0usize;
    find_files_in_dir(dir, dir, &lower_query, limit, 0, &mut visited, &mut results);
    results.truncate(limit);
    results
}

fn find_files_in_dir(
    root: &Path,
    dir: &Path,
    lower_query: &str,
    limit: usize,
    depth: usize,
    visited: &mut usize,
    results: &mut Vec<String>,
) {
    const MAX_DEPTH: usize = 8;
    const MAX_VISITED: usize = 5000;

    if results.len() >= limit || depth > MAX_DEPTH || *visited >= MAX_VISITED {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries = entries.flatten().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        if results.len() >= limit || *visited >= MAX_VISITED {
            break;
        }
        *visited += 1;
        let Ok(metadata) = entry.file_type() else {
            continue;
        };
        if metadata.is_symlink() {
            continue;
        }

        let path = entry.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if should_skip_file_search_entry(&name) {
            continue;
        }

        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let matches_query =
            name.to_lowercase().contains(lower_query) || rel.to_lowercase().contains(lower_query);

        if path.is_dir() {
            if matches_query {
                results.push(format!("{}/", rel));
            }
            find_files_in_dir(root, &path, lower_query, limit, depth + 1, visited, results);
        } else if matches_query {
            results.push(rel);
        }
    }
}

fn should_skip_file_search_entry(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "node_modules" | "target" | "dist" | "build" | ".next" | "coverage"
        )
}
