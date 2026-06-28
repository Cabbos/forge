#[cfg(test)]
mod tests {
    use super::super::files::FileExecutor;
    use std::fs;

    fn temp_workspace(name: &str) -> std::path::PathBuf {
        let workspace = std::env::temp_dir().join(format!(
            "forge-file-exec-test-{name}-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        workspace
    }

    #[test]
    fn read_file_returns_content_and_line_count() {
        let workspace = temp_workspace("read-content");
        fs::write(workspace.join("test.txt"), "line1\nline2\nline3\n").expect("write");
        let executor = FileExecutor::new(workspace.clone());
        let result = executor.read_file("test.txt").expect("read");
        assert_eq!(result.content, "line1\nline2\nline3\n");
        assert_eq!(result.line_count, 3);
        assert!(result.path.ends_with("test.txt"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn write_file_creates_new_file() {
        let workspace = temp_workspace("write-new");
        let executor = FileExecutor::new(workspace.clone());
        let result = executor
            .write_file("new.txt", "hello world")
            .expect("write");
        assert!(result.path.ends_with("new.txt"));
        assert_eq!(result.old_content, "");
        assert_eq!(result.new_content, "hello world");
        assert_eq!(
            fs::read_to_string(workspace.join("new.txt")).unwrap(),
            "hello world"
        );
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn write_file_overwrites_existing() {
        let workspace = temp_workspace("write-overwrite");
        fs::write(workspace.join("existing.txt"), "old content").expect("write");
        let executor = FileExecutor::new(workspace.clone());
        let result = executor
            .write_file("existing.txt", "new content")
            .expect("write");
        assert_eq!(result.old_content, "old content");
        assert_eq!(result.new_content, "new content");
        assert_eq!(
            fs::read_to_string(workspace.join("existing.txt")).unwrap(),
            "new content"
        );
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn write_file_rejects_too_large_content() {
        let workspace = temp_workspace("write-large");
        let executor = FileExecutor::new(workspace.clone());
        let huge = "x".repeat(2_000_001);
        let err = executor
            .write_file("big.txt", &huge)
            .expect_err("should reject large content");
        assert!(err.contains("limit") || err.contains("Refusing"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn edit_file_replaces_old_string() {
        let workspace = temp_workspace("edit");
        fs::write(workspace.join("file.txt"), "hello world").expect("write");
        let executor = FileExecutor::new(workspace.clone());
        let result = executor
            .edit_file("file.txt", "world", "universe")
            .expect("edit");
        assert!(result.contains("edited"));
        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "hello universe"
        );
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn edit_file_rejects_missing_old_string() {
        let workspace = temp_workspace("edit-missing");
        fs::write(workspace.join("file.txt"), "hello world").expect("write");
        let executor = FileExecutor::new(workspace.clone());
        let err = executor
            .edit_file("file.txt", "nonexistent", "replacement")
            .expect_err("should reject missing old_string");
        assert!(err.contains("not found"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn list_directory_returns_sorted_entries() {
        let workspace = temp_workspace("list");
        fs::create_dir_all(workspace.join("subdir")).expect("create dir");
        fs::write(workspace.join("a.txt"), "a").expect("write");
        fs::write(workspace.join("b.txt"), "b").expect("write");
        let executor = FileExecutor::new(workspace.clone());
        let listing = executor.list_directory("").expect("list");
        assert!(listing.contains("a.txt"));
        assert!(listing.contains("b.txt"));
        assert!(listing.contains("subdir/"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn list_directory_truncates_at_limit() {
        let workspace = temp_workspace("list-truncate");
        for i in 0..350 {
            fs::write(workspace.join(format!("file{i}.txt")), "x").expect("write");
        }
        let executor = FileExecutor::new(workspace.clone());
        let listing = executor.list_directory("").expect("list");
        assert!(listing.contains("truncated"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn search_files_finds_matches() {
        let workspace = temp_workspace("search");
        fs::write(workspace.join("a.txt"), "hello world").expect("write");
        fs::write(workspace.join("b.txt"), "goodbye world").expect("write");
        let executor = FileExecutor::new(workspace.clone());
        let matches = executor.search_files("world").expect("search");
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().any(|m| m.file_path.ends_with("a.txt")));
        assert!(matches.iter().any(|m| m.file_path.ends_with("b.txt")));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn search_files_returns_empty_for_no_matches() {
        let workspace = temp_workspace("search-empty");
        fs::write(workspace.join("a.txt"), "hello world").expect("write");
        let executor = FileExecutor::new(workspace.clone());
        let matches = executor.search_files("nonexistent").expect("search");
        assert!(matches.is_empty());
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn search_files_rejects_invalid_regex() {
        let workspace = temp_workspace("search-regex");
        let executor = FileExecutor::new(workspace.clone());
        let err = executor
            .search_files("[")
            .expect_err("should reject invalid regex");
        assert!(err.contains("Invalid regex"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn read_file_rejects_oversized_file() {
        let workspace = temp_workspace("read-oversized");
        let huge = "x".repeat(2_000_001);
        fs::write(workspace.join("big.txt"), huge).expect("write");
        let executor = FileExecutor::new(workspace.clone());
        let err = executor
            .read_file("big.txt")
            .expect_err("should reject oversized file");
        assert!(err.contains("too large") || err.contains("limit"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn write_file_rejects_symlink_target() {
        let workspace = temp_workspace("write-symlink");
        fs::write(workspace.join("real.txt"), "real").expect("write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(workspace.join("real.txt"), workspace.join("link.txt")).expect("symlink");
            let executor = FileExecutor::new(workspace.clone());
            let err = executor
                .write_file("link.txt", "new content")
                .expect_err("should reject symlink");
            assert!(err.contains("symlink"));
        }
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn resolve_allows_relative_path_in_workspace() {
        let workspace = temp_workspace("resolve-relative");
        fs::create_dir_all(workspace.join("src")).expect("create dir");
        fs::write(workspace.join("src/main.rs"), "fn main() {}").expect("write");
        let executor = FileExecutor::new(workspace.clone());
        let result = executor.read_file("src/main.rs").expect("read relative");
        assert!(result.content.contains("fn main()"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn resolve_allows_absolute_path_inside_workspace() {
        let workspace = temp_workspace("resolve-abs-inside");
        fs::write(workspace.join("file.txt"), "content").expect("write");
        let executor = FileExecutor::new(workspace.clone());
        let result = executor
            .read_file(&workspace.join("file.txt").to_string_lossy())
            .expect("read absolute inside");
        assert_eq!(result.content, "content");
        let _ = fs::remove_dir_all(&workspace);
    }
}
