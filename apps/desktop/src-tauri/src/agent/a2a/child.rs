use std::path::Path;
use std::sync::Arc;
use tokio::sync::Notify;

use crate::adapters::base::AiAdapter;
use crate::agent::event_sink::EventEmitter;
use crate::harness::Harness;

pub(crate) struct ChildAgentRuntime;

impl ChildAgentRuntime {
    pub(crate) async fn run_read_only(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        emitter: &dyn EventEmitter,
        cancel: Arc<Notify>,
        working_dir: &Path,
    ) -> String {
        crate::agent::sub::SubAgent::run_with_emitter(
            task,
            adapter,
            harness,
            emitter,
            cancel,
            working_dir,
        )
        .await
    }

    pub(crate) async fn run_patch_proposal(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        emitter: &dyn EventEmitter,
        cancel: Arc<Notify>,
        working_dir: &Path,
    ) -> String {
        crate::agent::sub::SubAgent::run_patch_proposal(
            task,
            adapter,
            harness,
            emitter,
            cancel,
            working_dir,
        )
        .await
    }

    /// Run a worktree-worker sub-agent in an isolated git worktree.
    ///
    /// The worker gets a fresh Harness pointing at the worktree, so it can
    /// write files and run shell commands without touching the main workspace.
    /// On completion we collect the git diff (and any test output) and return
    /// a structured summary to the parent model.
    pub(crate) async fn run_worktree_worker(
        worktree_id: &str,
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        _parent_harness: Arc<Harness>,
        emitter: &dyn EventEmitter,
        cancel: Arc<Notify>,
        working_dir: &Path,
    ) -> String {
        use crate::agent::a2a::worktree::{LeaseResult, WorktreeLease, WorktreeWorkerSummary};

        let mut lease = match WorktreeLease::create(working_dir, worktree_id) {
            LeaseResult::Ok(l) => l,
            LeaseResult::NotAGitRepo { path } => {
                return serde_json::to_string(&WorktreeWorkerSummary {
                    result: format!(
                        "Cannot create worktree: {} is not inside a git repository",
                        path.display()
                    ),
                    diff: None,
                    diff_available: false,
                    test_report: None,
                    worktree_path: path.to_string_lossy().to_string(),
                    cleaned_up: true,
                })
                .unwrap_or_else(|_| "Worktree creation failed".to_string());
            }
            LeaseResult::GitError { message } => {
                return serde_json::to_string(&WorktreeWorkerSummary {
                    result: format!("Worktree creation failed: {message}"),
                    diff: None,
                    diff_available: false,
                    test_report: None,
                    worktree_path: working_dir.to_string_lossy().to_string(),
                    cleaned_up: true,
                })
                .unwrap_or_else(|_| "Worktree creation failed".to_string());
            }
        };

        let worktree_path = lease.path().to_path_buf();

        // Create a fresh harness for the worktree so the worker has full
        // tool access inside the isolated directory.
        let worktree_harness = Arc::new(Harness::new(worktree_path.clone()));

        // Run the sub-agent in worktree-worker mode.
        let sub_result = crate::agent::sub::SubAgent::run_worktree_worker(
            task,
            adapter,
            worktree_harness,
            emitter,
            cancel,
            &worktree_path,
        )
        .await;

        // Collect diff from the worktree.
        let diff = lease.diff().ok();
        let diff_available = diff.as_ref().is_some_and(|d| !d.trim().is_empty());

        // Extract a test report from the sub-agent result if present.
        let test_report = extract_test_report(&sub_result);

        // Preserve the worktree on failure so the user can inspect.
        let sub_success = !sub_result.contains("error") && !sub_result.contains("Error");
        if !sub_success {
            lease.preserve();
        }

        let cleaned_up = if lease.is_preserved() {
            false
        } else {
            lease.cleanup().is_ok()
        };

        let summary = WorktreeWorkerSummary {
            result: sub_result,
            diff,
            diff_available,
            test_report,
            worktree_path: worktree_path.to_string_lossy().to_string(),
            cleaned_up,
        };

        serde_json::to_string(&summary).unwrap_or_else(|_| {
            format!(
                "{{\"result\": \"{}\", \"diff_available\": false, \"cleaned_up\": {}}}",
                summary.result.replace('"', "\\\""),
                cleaned_up
            )
        })
    }
}

/// Try to extract test-report content from a sub-agent JSON result.
fn extract_test_report(raw: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(raw).ok()?;
    // If the result contains an explicit test_report field, use it.
    if let Some(report) = value.get("test_report").and_then(|v| v.as_str()) {
        if !report.is_empty() {
            return Some(report.to_string());
        }
    }
    // Otherwise, if the overall result text looks like it contains test output,
    // surface the whole result as the test report.
    let result = value.get("result").and_then(|v| v.as_str())?;
    if result.contains("test")
        || result.contains("Test")
        || result.contains("cargo test")
        || result.contains("npm test")
        || result.contains("pytest")
    {
        Some(result.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_test_report_from_json_field() {
        let raw = r#"{"result": "Done", "test_report": "3 passed, 0 failed"}"#;
        assert_eq!(
            extract_test_report(raw),
            Some("3 passed, 0 failed".to_string())
        );
    }

    #[test]
    fn extract_test_report_from_result_heuristic() {
        let raw = r#"{"result": "Ran cargo test. 5 passed, 1 failed."}"#;
        assert_eq!(
            extract_test_report(raw),
            Some("Ran cargo test. 5 passed, 1 failed.".to_string())
        );
    }

    #[test]
    fn extract_test_report_returns_none_when_no_tests() {
        let raw = r#"{"result": "Fixed typo in README"}"#;
        assert_eq!(extract_test_report(raw), None);
    }

    #[test]
    fn extract_test_report_returns_none_for_plain_text() {
        assert_eq!(extract_test_report("Just plain text"), None);
    }
}
