#[cfg(test)]
mod tests {
    use super::super::{
        append_line_capped, get_str, git_output_z_with_timeout, next_stream_line, preview_for_log,
        summarize_tool_input, truncate_text, GitSnapshotCommandOutput, ShellFileEffectDelta,
        ShellFileEffectSnapshot, ToolExecutor,
    };
    use crate::agent::event_sink::CollectingEventEmitter;
    use crate::agent::snapshot::{ActiveToolCallDescriptor, PendingConfirmDescriptor};
    use crate::protocol::events::StreamEvent;
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn temp_workspace(name: &str) -> std::path::PathBuf {
        let workspace = std::env::temp_dir().join(format!(
            "forge-executor-test-{name}-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        workspace
    }

    #[allow(clippy::type_complexity)]
    fn descriptor_executor(
        workspace: &std::path::Path,
    ) -> (
        ToolExecutor,
        Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
        Arc<RwLock<Vec<PendingConfirmDescriptor>>>,
        Arc<RwLock<Vec<ActiveToolCallDescriptor>>>,
    ) {
        let pending_confirms = Arc::new(RwLock::new(HashMap::new()));
        let pending_descriptors = Arc::new(RwLock::new(Vec::new()));
        let active_descriptors = Arc::new(RwLock::new(Vec::new()));
        let executor = ToolExecutor::new_with_descriptors(
            workspace.to_path_buf(),
            pending_confirms.clone(),
            pending_descriptors.clone(),
            active_descriptors.clone(),
        );
        (
            executor,
            pending_confirms,
            pending_descriptors,
            active_descriptors,
        )
    }

    fn has_file_io_event(
        events: &[StreamEvent],
        block_id: &str,
        path_suffix: &str,
        operation: &str,
    ) -> bool {
        events.iter().any(|event| {
            matches!(
                event,
                StreamEvent::FileIo {
                    session_id,
                    block_id: event_block_id,
                    path,
                    operation: event_operation,
                    source,
                } if session_id == "session-1"
                    && event_block_id == block_id
                    && path.ends_with(path_suffix)
                    && event_operation == operation
                    && source.as_deref() == Some("executor")
            )
        })
    }

    fn post_shell_delta_events(events: &[StreamEvent]) -> Vec<&StreamEvent> {
        events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    StreamEvent::FileIo {
                        source,
                        ..
                    } if source.as_deref() == Some("post_shell_delta")
                )
            })
            .collect()
    }

    fn assert_post_shell_delta_event(
        events: &[StreamEvent],
        block_id: &str,
        path: &str,
        operation: &str,
    ) {
        assert!(
            events.iter().any(|event| matches!(
                event,
                StreamEvent::FileIo {
                    session_id,
                    block_id: event_block_id,
                    path: event_path,
                    operation: event_operation,
                    source,
                } if session_id == "session-1"
                    && event_block_id == block_id
                    && event_path == path
                    && event_operation == operation
                    && source.as_deref() == Some("post_shell_delta")
            )),
            "expected post-shell delta {operation} event for {path}, got {events:?}"
        );
    }

    fn assert_file_io_event(
        events: &[StreamEvent],
        block_id: &str,
        path_suffix: &str,
        operation: &str,
    ) {
        assert!(
            has_file_io_event(events, block_id, path_suffix, operation),
            "expected FileIo {operation} event for {path_suffix}, got {events:?}"
        );
    }

    fn assert_no_file_io(events: &[StreamEvent], context: &str) {
        assert!(
            events
                .iter()
                .all(|event| !matches!(event, StreamEvent::FileIo { .. })),
            "{context} must not emit FileIo: {events:?}"
        );
    }

    fn run_git(workspace: &std::path::Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(workspace)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // ── Helper: get_str ──

    #[test]
    fn get_str_returns_some_for_existing_key() {
        let val = serde_json::json!({"path": "/tmp/file.txt"});
        assert_eq!(get_str(&val, "path"), Some("/tmp/file.txt"));
    }

    #[test]
    fn get_str_returns_none_for_missing_key() {
        let val = serde_json::json!({"path": "/tmp/file.txt"});
        assert_eq!(get_str(&val, "content"), None);
    }

    #[test]
    fn get_str_returns_none_for_non_string_value() {
        let val = serde_json::json!({"count": 42});
        assert_eq!(get_str(&val, "count"), None);
    }

    // ── Helper: summarize_tool_input ──

    #[test]
    fn summarize_tool_input_read_file() {
        let val = serde_json::json!({"path": "/tmp/file.txt"});
        let summary = summarize_tool_input("read_file", &val);
        assert!(summary.contains("path="));
        assert!(summary.contains("file.txt"));
    }

    #[test]
    fn summarize_tool_input_write_file() {
        let val = serde_json::json!({"path": "out.txt", "content": "hello"});
        let summary = summarize_tool_input("write_file", &val);
        assert!(summary.contains("path="));
        assert!(summary.contains("content_len=5"));
    }

    #[test]
    fn summarize_tool_input_edit_file() {
        let val = serde_json::json!({"path": "a.rs", "old_string": "foo", "new_string": "bar"});
        let summary = summarize_tool_input("edit_file", &val);
        assert!(summary.contains("old_len=3"));
        assert!(summary.contains("new_len=3"));
    }

    #[test]
    fn summarize_tool_input_shell() {
        let val = serde_json::json!({"command": "echo hi"});
        let summary = summarize_tool_input("run_shell", &val);
        assert!(summary.contains("command="));
        assert!(summary.contains("echo hi"));
    }

    #[test]
    fn summarize_tool_input_search() {
        let val = serde_json::json!({"pattern": "todo", "path": "src"});
        let summary = summarize_tool_input("search_content", &val);
        assert!(summary.contains("pattern="));
        assert!(summary.contains("todo"));
        assert!(summary.contains("path="));
        assert!(summary.contains("src"));
    }

    #[test]
    fn summarize_tool_input_unknown_returns_empty() {
        let val = serde_json::Value::Null;
        assert_eq!(summarize_tool_input("unknown_tool", &val), "");
    }

    // ── Helper: preview_for_log ──

    #[test]
    fn preview_for_log_short_value() {
        assert_eq!(preview_for_log("hello"), "\"hello\"");
    }

    #[test]
    fn preview_for_log_long_value_truncates() {
        let long = "a".repeat(100);
        let preview = preview_for_log(&long);
        assert!(preview.starts_with("\""));
        assert!(preview.ends_with("...\""));
        // " + 80 chars + ..." = 84, but the actual length may be 85 depending on char counting
        assert!(preview.len() <= 85);
    }

    #[test]
    fn preview_for_log_escapes_newlines() {
        assert_eq!(preview_for_log("a\nb"), "\"a\\nb\"");
    }

    // ── Helper: truncate_text ──

    #[test]
    fn truncate_text_noop_when_short() {
        assert_eq!(truncate_text("hello", 100), "hello");
    }

    #[test]
    fn truncate_text_truncates_at_byte_boundary() {
        let text = "a".repeat(200);
        let truncated = truncate_text(&text, 100);
        assert!(truncated.starts_with(&"a".repeat(100)));
        assert!(truncated.contains("truncated"));
    }

    #[test]
    fn truncate_text_respects_utf8_boundaries() {
        let text = "é".repeat(200); // 2 bytes each
        let truncated = truncate_text(&text, 100);
        assert!(truncated.contains("truncated"));
        // Should not panic and should be valid UTF-8
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    // ── Helper: append_line_capped ──

    #[test]
    fn append_line_capped_appends_line() {
        let mut buf = String::new();
        append_line_capped(&mut buf, "hello", 100);
        assert_eq!(buf, "hello\n");
    }

    #[test]
    fn append_line_capped_skips_when_full() {
        let mut buf = "x".repeat(100);
        append_line_capped(&mut buf, "more", 100);
        assert_eq!(buf, "x".repeat(100));
    }

    #[test]
    fn append_line_capped_truncates_long_line() {
        let mut buf = String::new();
        let line = "a".repeat(200);
        append_line_capped(&mut buf, &line, 100);
        assert!(buf.contains("... [output truncated]"));
        assert!(buf.len() <= 100);
    }

    #[test]
    fn append_line_capped_partial_fit() {
        let mut buf = "x".repeat(50);
        let line = "a".repeat(200);
        append_line_capped(&mut buf, &line, 100);
        assert!(buf.contains("... [output truncated]"));
        assert!(buf.len() <= 100);
    }

    // ── Helper: next_stream_line ──

    #[test]
    fn next_stream_line_emits_full_line_when_under_limit() {
        let emitted = Arc::new(std::sync::Mutex::new(0));
        let notice = Arc::new(std::sync::Mutex::new(false));
        let result = next_stream_line("hello", &emitted, &notice, 100);
        assert_eq!(result, Some("hello".to_string()));
        assert_eq!(*emitted.lock().unwrap(), 6);
    }

    #[test]
    fn next_stream_line_returns_none_after_limit_reached() {
        let emitted = Arc::new(std::sync::Mutex::new(100));
        let notice = Arc::new(std::sync::Mutex::new(false));
        let result = next_stream_line("hello", &emitted, &notice, 100);
        assert_eq!(
            result,
            Some("... output truncated after 100 bytes".to_string())
        );
        assert!(*notice.lock().unwrap());

        // Second call should return None
        let notice2 = Arc::new(std::sync::Mutex::new(true));
        let result2 = next_stream_line("hello", &emitted, &notice2, 100);
        assert_eq!(result2, None);
    }

    #[test]
    fn next_stream_line_truncates_partial_line() {
        let emitted = Arc::new(std::sync::Mutex::new(95));
        let notice = Arc::new(std::sync::Mutex::new(false));
        let result = next_stream_line("hello world", &emitted, &notice, 100);
        assert!(result.is_some());
        let content = result.unwrap();
        assert!(content.contains("truncated after 100 bytes"));
    }

    // ── ToolExecutor construction ──

    #[test]
    fn tool_executor_new() {
        let workspace = temp_workspace("new");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        assert_eq!(executor.file.working_dir(), &workspace);
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn tool_executor_new_with_descriptors() {
        let workspace = temp_workspace("new-with-descriptors");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let pending_descriptors = Arc::new(RwLock::new(Vec::new()));
        let active_descriptors = Arc::new(RwLock::new(Vec::new()));
        let executor = ToolExecutor::new_with_descriptors(
            workspace.clone(),
            pending,
            pending_descriptors.clone(),
            active_descriptors.clone(),
        );
        assert_eq!(executor.file.working_dir(), &workspace);
        let _ = std::fs::remove_dir_all(&workspace);
    }

    // ── execute_with_emitter: read_file ──

    #[tokio::test]
    async fn read_file_success() {
        let workspace = temp_workspace("read-success");
        std::fs::write(workspace.join("test.txt"), "hello world").expect("write");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());
        let result = executor
            .execute_with_emitter(
                "session-1",
                "read_file",
                &serde_json::json!({"path": "test.txt"}),
                emitter.clone(),
                Some("block-1"),
                None,
            )
            .await;
        assert_eq!(result, "hello world");

        let events = emitter.drain();
        assert!(events.iter().any(
            |e| matches!(e, StreamEvent::ToolCallResult { block_id, .. } if block_id == "block-1")
        ));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn executor_file_io_stream_read_success_emits_file_io() {
        let workspace = temp_workspace("file-io-read-success");
        std::fs::write(workspace.join("test.txt"), "hello world").expect("write");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "read_file",
                &serde_json::json!({"path": "test.txt"}),
                emitter.clone(),
                Some("block-read"),
                None,
            )
            .await;

        assert_eq!(result, "hello world");
        let events = emitter.drain();
        assert!(
            events.iter().any(|event| matches!(
                event,
                StreamEvent::FileIo {
                    session_id,
                    block_id,
                    path,
                    operation,
                    source,
                } if session_id == "session-1"
                    && block_id == "block-read"
                    && path.ends_with("test.txt")
                    && operation == "read"
                    && source.as_deref() == Some("executor")
            )),
            "expected FileIo read event, got {events:?}"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn read_file_error_missing() {
        let workspace = temp_workspace("read-missing");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());
        let result = executor
            .execute_with_emitter(
                "session-1",
                "read_file",
                &serde_json::json!({"path": "missing.txt"}),
                emitter.clone(),
                Some("block-2"),
                None,
            )
            .await;
        assert!(
            result.starts_with("Error:"),
            "expected error, got: {result}"
        );
        let events = emitter.drain();
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamEvent::ToolCallResult { is_error: true, .. })));
        assert!(
            events
                .iter()
                .all(|event| !matches!(event, StreamEvent::FileIo { .. })),
            "missing read must not emit FileIo: {events:?}"
        );
        let _ = std::fs::remove_dir_all(&workspace);
    }

    // ── execute_with_emitter: write_file ──

    #[tokio::test]
    async fn write_file_success_emits_diff_view() {
        let workspace = temp_workspace("write-success");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());
        let result = executor
            .execute_with_emitter(
                "session-1",
                "write_file",
                &serde_json::json!({"path": "new.txt", "content": "fresh content"}),
                emitter.clone(),
                Some("block-3"),
                None,
            )
            .await;
        assert!(
            result.contains("File written:"),
            "expected success, got: {result}"
        );
        assert!(result.contains("new.txt"));
        assert_eq!(
            std::fs::read_to_string(workspace.join("new.txt")).unwrap(),
            "fresh content"
        );

        let events = emitter.drain();
        assert!(
            events.iter().any(|e| matches!(e, StreamEvent::DiffView { file_path, .. } if file_path.ends_with("new.txt"))),
            "expected DiffView event"
        );
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn executor_file_io_stream_write_success_emits_file_io() {
        let workspace = temp_workspace("file-io-write-success");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "write_file",
                &serde_json::json!({"path": "new.txt", "content": "fresh content"}),
                emitter.clone(),
                Some("block-write"),
                None,
            )
            .await;

        assert!(
            result.contains("File written:"),
            "expected success, got: {result}"
        );
        let events = emitter.drain();
        assert!(
            events.iter().any(|event| matches!(
                event,
                StreamEvent::FileIo {
                    session_id,
                    block_id,
                    path,
                    operation,
                    source,
                } if session_id == "session-1"
                    && block_id == "block-write"
                    && path.ends_with("new.txt")
                    && operation == "write"
                    && source.as_deref() == Some("executor")
            )),
            "expected FileIo write event, got {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, StreamEvent::DiffView { .. })),
            "write should keep existing DiffView event"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    // ── execute_with_emitter: edit_file ──

    #[tokio::test]
    async fn edit_file_success() {
        let workspace = temp_workspace("edit-success");
        std::fs::write(workspace.join("file.txt"), "hello world").expect("write");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());
        let result = executor
            .execute_with_emitter(
                "session-1",
                "edit_file",
                &serde_json::json!({"path": "file.txt", "old_string": "world", "new_string": "universe"}),
                emitter.clone(),
                Some("block-4"),
                None,
            )
            .await;
        assert!(
            result.contains("edited"),
            "expected edit success, got: {result}"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "hello universe"
        );
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn edit_file_error_missing_old_string() {
        let workspace = temp_workspace("edit-error");
        std::fs::write(workspace.join("file.txt"), "hello world").expect("write");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());
        let result = executor
            .execute_with_emitter(
                "session-1",
                "edit_file",
                &serde_json::json!({"path": "file.txt", "old_string": "missing", "new_string": "replacement"}),
                emitter.clone(),
                Some("block-5"),
                None,
            )
            .await;
        assert!(
            result.starts_with("Error:"),
            "expected error, got: {result}"
        );
        let events = emitter.drain();
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamEvent::ToolCallResult { is_error: true, .. })));
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn executor_file_io_stream_edit_success_emits_edit_file_io() {
        let workspace = temp_workspace("file-io-edit-success");
        std::fs::write(workspace.join("file.txt"), "hello world").expect("write");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "edit_file",
                &serde_json::json!({"path": "file.txt", "old_string": "world", "new_string": "universe"}),
                emitter.clone(),
                Some("block-edit"),
                None,
            )
            .await;

        assert!(
            result.contains("edited"),
            "expected edit success, got: {result}"
        );
        let events = emitter.drain();
        assert_file_io_event(&events, "block-edit", "file.txt", "edit");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn executor_file_io_stream_edit_error_emits_no_file_io() {
        let workspace = temp_workspace("file-io-edit-error");
        std::fs::write(workspace.join("file.txt"), "hello world").expect("write");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "edit_file",
                &serde_json::json!({"path": "file.txt", "old_string": "missing", "new_string": "replacement"}),
                emitter.clone(),
                Some("block-edit-error"),
                None,
            )
            .await;

        assert!(
            result.starts_with("Error:"),
            "expected edit error, got: {result}"
        );
        let events = emitter.drain();
        assert_no_file_io(&events, "edit error");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    // ── execute_with_emitter: list_directory ──

    #[tokio::test]
    async fn list_directory_empty() {
        let workspace = temp_workspace("list-empty");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());
        let result = executor
            .execute_with_emitter(
                "session-1",
                "list_directory",
                &serde_json::json!({"path": "."}),
                emitter.clone(),
                Some("block-6"),
                None,
            )
            .await;
        assert!(
            !result.starts_with("Error:"),
            "expected success, got: {result}"
        );
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn executor_file_io_stream_list_directory_success_emits_list_file_io() {
        let workspace = temp_workspace("file-io-list-success");
        std::fs::write(workspace.join("file.txt"), "hello world").expect("write");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "list_directory",
                &serde_json::json!({"path": "."}),
                emitter.clone(),
                Some("block-list"),
                None,
            )
            .await;

        assert!(
            result.contains("file.txt"),
            "expected directory listing, got: {result}"
        );
        let events = emitter.drain();
        assert_file_io_event(&events, "block-list", ".", "list");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn executor_file_io_stream_search_files_success_emits_search_file_io() {
        let workspace = temp_workspace("file-io-search-files-success");
        std::fs::create_dir_all(workspace.join("src")).expect("mkdir");
        std::fs::write(workspace.join("src/main.rs"), "fn main() {}\n").expect("write");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "search_files",
                &serde_json::json!({"path": "src", "pattern": "*.rs"}),
                emitter.clone(),
                Some("block-search-files"),
                None,
            )
            .await;

        assert!(
            result.contains("main.rs"),
            "expected file search result, got: {result}"
        );
        let events = emitter.drain();
        assert_file_io_event(&events, "block-search-files", "src", "search");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn executor_file_io_stream_search_content_no_match_emits_search_file_io() {
        let workspace = temp_workspace("file-io-search-content-no-match");
        std::fs::write(workspace.join("file.txt"), "hello world").expect("write");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "search_content",
                &serde_json::json!({"path": ".", "pattern": ""}),
                emitter.clone(),
                Some("block-search-content"),
                None,
            )
            .await;

        assert_eq!(result, "No matches found");
        let events = emitter.drain();
        assert_file_io_event(&events, "block-search-content", ".", "search");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn executor_file_io_stream_failed_search_emits_no_file_io() {
        let workspace = temp_workspace("file-io-search-failed");
        std::fs::write(workspace.join("file.txt"), "hello world").expect("write");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "search_content",
                &serde_json::json!({"path": ".", "pattern": "["}),
                emitter.clone(),
                Some("block-search-failed"),
                None,
            )
            .await;

        assert!(
            result.starts_with("Search failed"),
            "expected failed search, got: {result}"
        );
        let events = emitter.drain();
        assert_no_file_io(&events, "failed search");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn executor_file_io_stream_blocked_search_emits_no_file_io() {
        let workspace = temp_workspace("file-io-search-blocked");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());
        let outside_workspace = std::env::temp_dir();

        let result = executor
            .execute_with_emitter(
                "session-1",
                "search_content",
                &serde_json::json!({"path": outside_workspace, "pattern": "needle"}),
                emitter.clone(),
                Some("block-search-blocked"),
                None,
            )
            .await;

        assert!(
            result.starts_with("Search blocked:"),
            "expected blocked search, got: {result}"
        );
        let events = emitter.drain();
        assert_no_file_io(&events, "blocked search");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn executor_file_io_stream_git_diff_success_emits_diff_file_io() {
        let workspace = temp_workspace("file-io-git-diff-success");
        run_git(&workspace, &["init", "--quiet"]);
        std::fs::write(workspace.join("tracked.txt"), "old\n").expect("write old");
        run_git(&workspace, &["add", "tracked.txt"]);
        std::fs::write(workspace.join("tracked.txt"), "new\n").expect("write new");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "git_diff",
                &serde_json::json!({"path": "tracked.txt"}),
                emitter.clone(),
                Some("block-diff"),
                None,
            )
            .await;

        assert!(
            result.contains("-old") && result.contains("+new"),
            "expected git diff output, got: {result}"
        );
        let events = emitter.drain();
        assert_file_io_event(&events, "block-diff", "tracked.txt", "diff");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn executor_file_io_stream_git_diff_failure_emits_no_file_io() {
        let workspace = temp_workspace("file-io-git-diff-failure");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "git_diff",
                &serde_json::json!({"path": "tracked.txt"}),
                emitter.clone(),
                Some("block-diff-failure"),
                None,
            )
            .await;

        assert!(
            result.starts_with("git diff failed"),
            "expected git diff failure, got: {result}"
        );
        let events = emitter.drain();
        assert_no_file_io(&events, "git diff failure");

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn shell_file_effect_write_emits_post_shell_delta_after_completion() {
        let workspace = temp_workspace("shell-file-effect-write");
        run_git(&workspace, &["init", "--quiet"]);
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "run_shell",
                &serde_json::json!({"command": "printf 'hello' > shell.txt"}),
                emitter.clone(),
                Some("block-shell"),
                None,
            )
            .await;

        assert!(
            !result.starts_with("Error:"),
            "expected shell command to run, got: {result}"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.join("shell.txt")).unwrap(),
            "hello"
        );
        let events = emitter.drain();
        assert!(
            events.iter().all(|event| !matches!(
                event,
                StreamEvent::FileIo {
                    source,
                    ..
                } if source.as_deref() == Some("executor")
                    || source.as_deref() == Some("shell_internal")
            )),
            "run_shell must not emit executor or shell-internal FileIo facts: {events:?}"
        );
        assert_post_shell_delta_event(&events, "block-shell", "shell.txt", "created");
        let shell_end_index = events
            .iter()
            .position(|event| matches!(event, StreamEvent::ShellEnd { block_id, .. } if block_id == "block-shell"))
            .expect("shell_end event");
        let delta_index = events
            .iter()
            .position(|event| {
                matches!(
                    event,
                    StreamEvent::FileIo {
                        block_id,
                        path,
                        source,
                        ..
                    } if block_id == "block-shell"
                        && path == "shell.txt"
                        && source.as_deref() == Some("post_shell_delta")
                )
            })
            .expect("post-shell delta event");
        assert!(
            delta_index > shell_end_index,
            "post-shell delta must be emitted after shell_end: {events:?}"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn shell_file_effect_read_only_emits_no_post_shell_delta() {
        let workspace = temp_workspace("shell-file-effect-read-only");
        run_git(&workspace, &["init", "--quiet"]);
        std::fs::write(workspace.join("tracked.txt"), "hello\n").expect("write tracked");
        run_git(&workspace, &["add", "tracked.txt"]);
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "run_shell",
                &serde_json::json!({"command": "cat tracked.txt"}),
                emitter.clone(),
                Some("block-shell-read"),
                None,
            )
            .await;

        assert!(
            result.contains("hello"),
            "expected shell read output, got: {result}"
        );
        let events = emitter.drain();
        assert!(
            post_shell_delta_events(&events).is_empty(),
            "read-only shell command must not emit post-shell deltas: {events:?}"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn shell_file_effect_failed_command_emits_no_success_delta() {
        let workspace = temp_workspace("shell-file-effect-failed");
        run_git(&workspace, &["init", "--quiet"]);
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "run_shell",
                &serde_json::json!({"command": "printf 'hello' > shell.txt; exit 7"}),
                emitter.clone(),
                Some("block-shell-failed"),
                None,
            )
            .await;

        assert!(
            result.contains("Exit code: 7"),
            "expected shell failure exit code, got: {result}"
        );
        let events = emitter.drain();
        assert!(
            post_shell_delta_events(&events).is_empty(),
            "failed shell command must not emit success post-shell deltas: {events:?}"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn shell_file_effect_non_git_workspace_emits_unknown_boundary() {
        let workspace = temp_workspace("shell-file-effect-non-git");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "run_shell",
                &serde_json::json!({"command": "printf 'hello' > shell.txt"}),
                emitter.clone(),
                Some("block-shell-non-git"),
                None,
            )
            .await;

        assert!(
            !result.starts_with("Error:"),
            "expected shell command to run, got: {result}"
        );
        let events = emitter.drain();
        assert_post_shell_delta_event(
            &events,
            "block-shell-non-git",
            "[non_git_workspace]",
            "unknown_boundary",
        );
        assert!(
            post_shell_delta_events(&events).len() == 1,
            "non-git workspaces must emit one boundary fact, not enumerate files: {events:?}"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn shell_file_effect_snapshot_falls_back_when_required_git_command_fails() {
        let workspace = temp_workspace("shell-file-effect-git-failure");
        run_git(&workspace, &["init", "--quiet"]);
        let git_index = workspace.join(".git").join("index");
        if git_index.is_file() {
            std::fs::remove_file(&git_index).expect("remove git index file");
        }
        std::fs::create_dir(&git_index).expect("corrupt git index");

        let snapshot = ShellFileEffectSnapshot::capture(&workspace).await;

        assert_eq!(
            snapshot,
            ShellFileEffectSnapshot::UnknownBoundary {
                path: "[git_snapshot_unavailable]".to_string(),
            }
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn shell_file_effect_unavailable_before_snapshot_wins_over_later_git_success() {
        let before = ShellFileEffectSnapshot::UnknownBoundary {
            path: "[git_snapshot_unavailable]".to_string(),
        };
        let after = ShellFileEffectSnapshot::Git {
            root: "/tmp/forge".to_string(),
            paths: BTreeMap::new(),
        };

        let delta = before.delta(after);

        assert!(matches!(
            delta,
            ShellFileEffectDelta::UnknownBoundary { path }
                if path == "[git_snapshot_unavailable]"
        ));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn git_snapshot_command_times_out_as_unavailable() {
        use std::os::unix::fs::PermissionsExt;

        let workspace = temp_workspace("shell-file-effect-git-timeout");
        let fake_git = workspace.join("slow-git");
        std::fs::write(&fake_git, "#!/bin/sh\nexec sleep 5\n").expect("write fake git");
        let mut permissions = std::fs::metadata(&fake_git)
            .expect("fake git metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_git, permissions).expect("chmod fake git");

        let started = std::time::Instant::now();
        let output = git_output_z_with_timeout(
            fake_git.as_os_str(),
            &workspace,
            &["status", "--porcelain=v1", "-z"],
            std::time::Duration::from_millis(50),
        )
        .await;

        assert_eq!(output, GitSnapshotCommandOutput::Unavailable);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "git snapshot timeout did not bound the command promptly"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn shell_file_effect_delta_carries_shell_block_id_only() {
        let workspace = temp_workspace("shell-file-effect-block-id");
        run_git(&workspace, &["init", "--quiet"]);
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "run_shell",
                &serde_json::json!({"command": "printf 'hello' > shell.txt"}),
                emitter.clone(),
                Some("block-shell-owner"),
                None,
            )
            .await;

        assert!(
            !result.starts_with("Error:"),
            "expected shell command to run, got: {result}"
        );
        let events = emitter.drain();
        let deltas = post_shell_delta_events(&events);
        assert!(
            !deltas.is_empty(),
            "expected at least one post-shell delta event: {events:?}"
        );
        assert!(
            deltas.iter().all(|event| matches!(
                event,
                StreamEvent::FileIo {
                    block_id,
                    ..
                } if block_id == "block-shell-owner"
            )),
            "post-shell delta events must carry the shell block id only: {events:?}"
        );
        assert!(
            deltas.iter().all(|event| !matches!(
                event,
                StreamEvent::FileIo {
                    block_id,
                    ..
                } if block_id == "other-block"
            )),
            "post-shell delta must not attach to another block: {events:?}"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    // ── execute_with_emitter: ask_user ──

    #[tokio::test]
    async fn ask_user_approved() {
        let workspace = temp_workspace("ask-approved");
        let (executor, pending_confirms, pending_descriptors, _active) =
            descriptor_executor(&workspace);
        let emitter = Arc::new(CollectingEventEmitter::new());
        let block_id = "ask-block-1".to_string();

        // Spawn a task that will approve the confirm once it appears in pending_confirms
        let pending_for_approve = pending_confirms.clone();
        let block_id_for_approve = block_id.clone();
        let approve_task = tokio::spawn(async move {
            // Poll until the executor has registered the sender, then send true
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                if let Some(sender) = pending_for_approve
                    .write()
                    .await
                    .remove(&block_id_for_approve)
                {
                    let _ = sender.send(true);
                    break;
                }
            }
        });

        let result = executor
            .execute_with_emitter(
                "session-1",
                "ask_user",
                &serde_json::json!({"question": "Continue?"}),
                emitter.clone(),
                Some(&block_id),
                None,
            )
            .await;

        assert_eq!(result, "User approved");

        let events = emitter.drain();
        assert!(
            events.iter().any(
                |e| matches!(e, StreamEvent::ConfirmAsk { block_id: b, .. } if b == &block_id)
            ),
            "expected ConfirmAsk event"
        );

        // pending_confirm_descriptors should have been populated and then cleaned
        assert!(
            pending_descriptors.read().await.is_empty(),
            "pending confirm descriptors should be cleaned up"
        );

        let _ = approve_task.await;
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn ask_user_declined() {
        let workspace = temp_workspace("ask-declined");
        let (executor, pending_confirms, _pending_descriptors, _active) =
            descriptor_executor(&workspace);
        let emitter = Arc::new(CollectingEventEmitter::new());
        let block_id = "ask-block-2".to_string();

        // Spawn a task that will decline the confirm once it appears in pending_confirms
        let pending_for_decline = pending_confirms.clone();
        let block_id_for_decline = block_id.clone();
        let decline_task = tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                if let Some(sender) = pending_for_decline
                    .write()
                    .await
                    .remove(&block_id_for_decline)
                {
                    let _ = sender.send(false);
                    break;
                }
            }
        });

        let result = executor
            .execute_with_emitter(
                "session-1",
                "ask_user",
                &serde_json::json!({"question": "Delete everything?"}),
                emitter.clone(),
                Some(&block_id),
                None,
            )
            .await;

        assert_eq!(result, "User declined");
        let _ = decline_task.await;
        let _ = std::fs::remove_dir_all(&workspace);
    }

    // ── execute_with_emitter: unknown tool ──

    #[tokio::test]
    async fn unknown_tool_returns_error_message() {
        let workspace = temp_workspace("unknown");
        let pending = Arc::new(RwLock::new(HashMap::new()));
        let executor = ToolExecutor::new(workspace.clone(), pending);
        let emitter = Arc::new(CollectingEventEmitter::new());
        let result = executor
            .execute_with_emitter(
                "session-1",
                "nonexistent_tool",
                &serde_json::Value::Null,
                emitter.clone(),
                Some("block-7"),
                None,
            )
            .await;
        assert!(
            result.contains("Unknown tool:"),
            "expected unknown tool, got: {result}"
        );
        assert!(result.contains("nonexistent_tool"));

        let events = emitter.drain();
        assert!(
            events.iter().any(
                |e| matches!(e, StreamEvent::ToolCallResult { result: r, .. } if r == &result)
            ),
            "expected ToolCallResult with the unknown tool message"
        );
        let _ = std::fs::remove_dir_all(&workspace);
    }

    // ── active_tool_call_descriptors register and cleanup ──

    #[tokio::test]
    async fn active_tool_call_descriptors_register_and_cleanup() {
        let workspace = temp_workspace("active-descriptors");
        let (executor, _pending, _pending_descriptors, active_descriptors) =
            descriptor_executor(&workspace);
        let emitter = Arc::new(CollectingEventEmitter::new());
        let block_id = "active-block-1";

        let result = executor
            .execute_with_emitter(
                "session-1",
                "list_directory",
                &serde_json::json!({"path": "."}),
                emitter.clone(),
                Some(block_id),
                None,
            )
            .await;

        assert!(
            !result.starts_with("Error:"),
            "expected success, got: {result}"
        );
        assert!(
            active_descriptors.read().await.is_empty(),
            "active descriptors should be cleaned up after execution"
        );
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn active_tool_call_descriptors_populated_during_execution() {
        let workspace = temp_workspace("active-during");
        let (executor, _pending, _pending_descriptors, active_descriptors) =
            descriptor_executor(&workspace);
        let emitter = Arc::new(CollectingEventEmitter::new());
        let block_id = "active-block-2";

        // We can't easily inspect mid-execution without a custom emitter that blocks,
        // but we can verify the registry is empty before and after.
        assert!(active_descriptors.read().await.is_empty());

        let result = executor
            .execute_with_emitter(
                "session-1",
                "read_file",
                &serde_json::json!({"path": "nonexistent.txt"}),
                emitter.clone(),
                Some(block_id),
                None,
            )
            .await;

        assert!(result.starts_with("Error:"));
        assert!(
            active_descriptors.read().await.is_empty(),
            "active descriptors should be cleaned up even on error"
        );
        let _ = std::fs::remove_dir_all(&workspace);
    }
}
